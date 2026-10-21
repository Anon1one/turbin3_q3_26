use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod instructions;
pub mod state;
pub mod utils;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("F2EenmddwQTMzbpAiqkCvwfkpcSvHi8e5F9286CcuzMo");

#[program]
pub mod nft_staking_q326 {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        rewards_bps: u64,
        freeze_period: u16,
    ) -> Result<()> {
        ctx.accounts
            .initialize(rewards_bps, freeze_period, &ctx.bumps)
    }

    pub fn create_collection(
        ctx: Context<CreateCollection>,
        name: String,
        uri: String,
    ) -> Result<()> {
        ctx.accounts.create_collection(name, uri, &ctx.bumps)
    }

    pub fn mint_asset(ctx: Context<MintAsset>, name: String, uri: String) -> Result<()> {
        ctx.accounts.mint_asset(name, uri, &ctx.bumps)
    }

    pub fn stake(ctx: Context<Stake>) -> Result<()> {
        ctx.accounts.stake(&ctx.bumps)
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        ctx.accounts.unstake(&ctx.bumps)
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        ctx.accounts.claim_rewards(&ctx.bumps)
    }

    pub fn burn_staked_nft(ctx: Context<BurnStakedNft>) -> Result<()> {
        ctx.accounts.burn_staked_nft(&ctx.bumps)
    }
}
