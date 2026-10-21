use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, Mint, MintTo, Token, TokenAccount},
};
use mpl_core::{
    accounts::BaseAssetV1,
    fetch_plugin,
    instructions::{AddPluginV1CpiBuilder, BurnV1CpiBuilder, UpdatePluginV1CpiBuilder},
    types::{
        Attributes, BurnDelegate, FreezeDelegate, Plugin, PluginAuthority, PluginType,
        UpdateAuthority,
    },
    ID as MPL_CORE_ID,
};

use crate::{errors::ErrorCode, state::Config, utils::update_total_staked, BURN_BONUS_TOKENS};

#[derive(Accounts)]
pub struct BurnStakedNft<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        seeds = [b"config", collection.key().as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,
    #[account(mut, owner = MPL_CORE_ID)]
    /// CHECK: MPL Core asset, owner and update authority checked in handler
    pub asset: UncheckedAccount<'info>,
    #[account(
        mut,
        constraint = collection.key() == config.collection @ ErrorCode::InvalidCollection,
    )]
    /// CHECK: MPL Core collection, must match config
    pub collection: UncheckedAccount<'info>,
    #[account(
        seeds = [b"update_authority", collection.key().as_ref()],
        bump,
    )]
    pub update_authority: SystemAccount<'info>,
    #[account(
        mut,
        seeds = [b"rewards", config.key().as_ref()],
        bump = config.rewards_bump,
    )]
    pub rewards_mint: Account<'info, Mint>,
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = rewards_mint,
        associated_token::authority = owner,
    )]
    pub user_rewards_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    #[account(address = MPL_CORE_ID)]
    /// CHECK: MPL Core Program ID
    pub mpl_core_program: UncheckedAccount<'info>,
}

impl<'info> BurnStakedNft<'info> {
    pub fn burn_staked_nft(&mut self, bumps: &BurnStakedNftBumps) -> Result<()> {
        let collection_key = self.collection.key();
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"update_authority",
            collection_key.as_ref(),
            &[bumps.update_authority],
        ]];

        let asset = BaseAssetV1::from_bytes(&self.asset.data.borrow())
            .map_err(|_| ProgramError::InvalidAccountData)?;
        require_keys_eq!(asset.owner, self.owner.key(), ErrorCode::InvalidOwner);
        require!(
            asset.update_authority == UpdateAuthority::Collection(self.collection.key()),
            ErrorCode::InvalidUpdateAuthority
        );

        let attributes_fetched =
            fetch_plugin::<BaseAssetV1, Attributes>(&self.asset, PluginType::Attributes);

        let mut is_staked = false;
        let mut staked_at_timestamp: i64 = 0;
        let mut last_claimed_timestamp: Option<i64> = None;

        if let Ok((_, attributes, _)) = attributes_fetched {
            for attribute in attributes.attribute_list {
                if attribute.key == "staked" {
                    is_staked = attribute.value == "true";
                } else if attribute.key == "staked_at" {
                    staked_at_timestamp = attribute.value.parse::<i64>().unwrap_or(0);
                } else if attribute.key == "last_claimed_at" {
                    last_claimed_timestamp = attribute.value.parse::<i64>().ok();
                }
            }
        }

        if !is_staked {
            return Err(ErrorCode::NotStaked.into());
        }

        // Burning ends the stake too, so it waits out the same freeze period as unstake
        let staked_days = Clock::get()?
            .unix_timestamp
            .checked_sub(staked_at_timestamp)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(86400)
            .ok_or(ErrorCode::MathOverflow)?;

        if staked_days < i64::from(self.config.freeze_period) {
            return Err(ErrorCode::FreezePeriodNotElapsed.into());
        }

        // Unclaimed rewards are paid out along with the burn bonus
        let pending_days = Clock::get()?
            .unix_timestamp
            .checked_sub(last_claimed_timestamp.unwrap_or(staked_at_timestamp))
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(86400)
            .ok_or(ErrorCode::MathOverflow)?;

        let decimals_factor = 10u64.pow(self.rewards_mint.decimals as u32);

        let pending_reward = (pending_days as u64)
            .checked_mul(self.config.rewards_bps)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_mul(decimals_factor)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::MathOverflow)?;

        let reward_amount = BURN_BONUS_TOKENS
            .checked_mul(decimals_factor)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_add(pending_reward)
            .ok_or(ErrorCode::MathOverflow)?;

        // A frozen asset cannot be burned, so thaw it first
        UpdatePluginV1CpiBuilder::new(&self.mpl_core_program)
            .asset(&self.asset)
            .collection(Some(&self.collection))
            .payer(&self.owner)
            .authority(Some(&self.update_authority))
            .system_program(&self.system_program)
            .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: false }))
            .invoke_signed(signer_seeds)?;

        // Owner grants the program burn rights through BurnDelegate
        AddPluginV1CpiBuilder::new(&self.mpl_core_program)
            .asset(&self.asset)
            .collection(Some(&self.collection))
            .payer(&self.owner)
            .authority(Some(&self.owner))
            .system_program(&self.system_program)
            .plugin(Plugin::BurnDelegate(BurnDelegate {}))
            .init_authority(PluginAuthority::Address {
                address: self.update_authority.key(),
            })
            .invoke()?;

        BurnV1CpiBuilder::new(&self.mpl_core_program)
            .asset(&self.asset)
            .collection(Some(&self.collection))
            .payer(&self.owner)
            .authority(Some(&self.update_authority))
            .system_program(Some(&self.system_program))
            .invoke_signed(signer_seeds)?;

        update_total_staked(
            &self.collection,
            &self.update_authority,
            &self.owner,
            &self.system_program,
            &self.mpl_core_program,
            signer_seeds,
            -1,
        )?;

        let config_seeds: &[&[&[u8]]] =
            &[&[b"config", collection_key.as_ref(), &[self.config.bump]]];

        let cpi_accounts = MintTo {
            mint: self.rewards_mint.to_account_info(),
            to: self.user_rewards_ata.to_account_info(),
            authority: self.config.to_account_info(),
        };

        let cpi_ctx =
            CpiContext::new_with_signer(self.token_program.key(), cpi_accounts, config_seeds);

        mint_to(cpi_ctx, reward_amount)?;

        Ok(())
    }
}
