use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenInterface};
use mpl_core::{accounts::BaseCollectionV1, ID as MPL_CORE_ID};

use crate::errors::ErrorCode;
use crate::state::Config;
use crate::{CONFIG_SEED, REWARDS_SEED, UPDATE_AUTHORITY_SEED};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + Config::INIT_SPACE,
        seeds = [CONFIG_SEED, collection.key().as_ref()],
        bump,
    )]
    pub config: Account<'info, Config>,

    /// The collection keypair must sign, so only whoever created the collection
    /// can set its reward terms. Update authority checked in handler.
    #[account(owner = MPL_CORE_ID)]
    pub collection: Signer<'info>,

    #[account(
        seeds = [UPDATE_AUTHORITY_SEED, collection.key().as_ref()],
        bump
    )]
    /// CHECK: PDA, only used as the collection update authority
    pub update_authority: UncheckedAccount<'info>,

    #[account(
        init,
        payer = admin,
        mint::decimals = 6,
        mint::authority = config,
        seeds = [REWARDS_SEED, config.key().as_ref()],
        bump
    )]
    pub rewards_mint: InterfaceAccount<'info, Mint>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> Initialize<'info> {
    pub fn initialize(
        &mut self,
        rewards_bps: u64,
        freeze_period: u16,
        bumps: &InitializeBumps,
    ) -> Result<()> {
        let collection = BaseCollectionV1::from_bytes(&self.collection.data.borrow())
            .map_err(|_| ProgramError::InvalidAccountData)?;
        require_keys_eq!(
            collection.update_authority,
            self.update_authority.key(),
            ErrorCode::InvalidUpdateAuthority
        );

        self.config.set_inner(Config {
            collection: self.collection.key(),
            admin: self.admin.key(),
            rewards_bps,
            freeze_period,
            rewards_bump: bumps.rewards_mint,
            bump: bumps.config,
        });

        Ok(())
    }
}
