use anchor_lang::prelude::*;
use mpl_core::{instructions::CreateCollectionV2CpiBuilder, ID as MPL_CORE_ID};


#[derive(Accounts)]
pub struct CreateCollection<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub collection: Signer<'info>,
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

impl<'info> CreateCollection<'info> {
    pub fn create_collection(
        &mut self,
        name: String,
        uri: String,
        bumps: &CreateCollectionBumps,
    ) -> Result<()> {
        let collection_key = self.collection.key();
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"update_authority",
            collection_key.as_ref(),
            &[bumps.update_authority],
        ]];

        CreateCollectionV2CpiBuilder::new(&self.mpl_core_program)
            .collection(&self.collection)
            .payer(&self.payer)
            .update_authority(Some(&self.update_authority))
            .system_program(&self.system_program)
            .name(name)
            .uri(uri)
            .invoke_signed(signer_seeds)?;

        Ok(())
    }
}
