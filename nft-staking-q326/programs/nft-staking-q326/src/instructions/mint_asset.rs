use anchor_lang::prelude::*;
use mpl_core::{instructions::CreateV2CpiBuilder, ID as MPL_CORE_ID};

use crate::{errors::ErrorCode, state::Config, CONFIG_SEED};

#[derive(Accounts)]
pub struct MintAsset<'info> {
    // Only the config admin can mint into the collection. Otherwise anyone could
    // mint unlimited NFTs and farm staking rewards with them.
    #[account(mut)]
    pub admin: Signer<'info>,
    /// CHECK: any wallet can receive the minted NFT
    pub owner: UncheckedAccount<'info>,
    #[account(mut)]
    pub asset: Signer<'info>,
    #[account(
        has_one = admin @ ErrorCode::Unauthorized,
        has_one = collection @ ErrorCode::InvalidCollection,
        seeds = [CONFIG_SEED, collection.key().as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    /// CHECK: MPL Core collection, must match config
    pub collection: UncheckedAccount<'info>,
    #[account(
        seeds = [b"update_authority", collection.key().as_ref()],
        bump,
    )]
    pub update_authority: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
    #[account(address = MPL_CORE_ID)]
    /// CHECK: MPL Core Program ID
    pub mpl_core_program: UncheckedAccount<'info>,
}

impl<'info> MintAsset<'info> {
    pub fn mint_asset(&mut self, name: String, uri: String, bumps: &MintAssetBumps) -> Result<()> {
        let collection_key = self.collection.key();
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"update_authority",
            collection_key.as_ref(),
            &[bumps.update_authority],
        ]];

        CreateV2CpiBuilder::new(&self.mpl_core_program)
            .asset(&self.asset)
            .collection(Some(&self.collection))
            .authority(Some(&self.update_authority))
            .payer(&self.admin)
            .owner(Some(&self.owner))
            .system_program(&self.system_program)
            .name(name)
            .uri(uri)
            .invoke_signed(signer_seeds)?;

        Ok(())
    }
}
