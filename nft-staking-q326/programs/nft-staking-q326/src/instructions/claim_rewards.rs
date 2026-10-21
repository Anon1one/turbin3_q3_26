use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, Mint, MintTo, Token, TokenAccount},
};
use mpl_core::{
    accounts::BaseAssetV1,
    fetch_plugin,
    instructions::UpdatePluginV1CpiBuilder,
    types::{Attribute, Attributes, Plugin, PluginType, UpdateAuthority},
    ID as MPL_CORE_ID,
};

use crate::{errors::ErrorCode, state::Config};

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
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

impl<'info> ClaimRewards<'info> {
    pub fn claim_rewards(&mut self, bumps: &ClaimRewardsBumps) -> Result<()> {
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

        if attributes_fetched.is_err() {
            return Err(ErrorCode::NotStaked.into());
        }

        let mut attribute_list: Vec<Attribute> = Vec::new();
        let current_timestamp = Clock::get()?.unix_timestamp;
        let mut is_staked = false;
        let mut staked_at_timestamp: i64 = 0;
        let mut last_claimed_timestamp: Option<i64> = None;

        if let Ok((_, attributes, _)) = attributes_fetched {
            for attribute in attributes.attribute_list {
                if attribute.key == "staked" {
                    if attribute.value == "true" {
                        is_staked = true;
                    }
                    attribute_list.push(attribute);
                } else if attribute.key == "staked_at" {
                    staked_at_timestamp = attribute.value.parse::<i64>().unwrap_or(0);
                    attribute_list.push(attribute);
                } else if attribute.key == "last_claimed_at" {
                    last_claimed_timestamp = attribute.value.parse::<i64>().ok();
                } else {
                    attribute_list.push(attribute);
                }
            }
        }

        if !is_staked {
            return Err(ErrorCode::NotStaked.into());
        }

        // Rewards accrue from the last claim, or from staking if never claimed
        let claim_from = last_claimed_timestamp.unwrap_or(staked_at_timestamp);

        let elapsed_days = current_timestamp
            .checked_sub(claim_from)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(86400)
            .ok_or(ErrorCode::MathOverflow)?;

        // Only move the checkpoint by whole days so partial days keep accruing
        let new_last_claimed = claim_from
            .checked_add(
                elapsed_days
                    .checked_mul(86400)
                    .ok_or(ErrorCode::MathOverflow)?,
            )
            .ok_or(ErrorCode::MathOverflow)?;

        attribute_list.push(Attribute {
            key: "last_claimed_at".to_string(),
            value: new_last_claimed.to_string(),
        });

        // Only the Attributes plugin is updated, FreezeDelegate stays frozen
        UpdatePluginV1CpiBuilder::new(&self.mpl_core_program)
            .asset(&self.asset)
            .collection(Some(&self.collection))
            .payer(&self.owner)
            .authority(Some(&self.update_authority))
            .system_program(&self.system_program)
            .plugin(Plugin::Attributes(Attributes { attribute_list }))
            .invoke_signed(signer_seeds)?;

        let reward_amount = (elapsed_days as u64)
            .checked_mul(self.config.rewards_bps)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_mul(10u64.pow(self.rewards_mint.decimals as u32))
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::MathOverflow)?;

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
