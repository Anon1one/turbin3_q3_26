use anchor_lang::prelude::*;
use mpl_core::{
    accounts::BaseAssetV1,
    fetch_plugin,
    instructions::{AddPluginV1CpiBuilder, UpdatePluginV1CpiBuilder},
    types::{Attribute, Attributes, FreezeDelegate, Plugin, PluginAuthority, PluginType, UpdateAuthority},
    ID as MPL_CORE_ID,
};

use crate::{errors::ErrorCode, state::Config, utils::update_total_staked};

#[derive(Accounts)]
pub struct Stake<'info> {
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
    pub system_program: Program<'info, System>,
    #[account(address = MPL_CORE_ID)]
    /// CHECK: MPL Core Program ID
    pub mpl_core_program: UncheckedAccount<'info>,
}

impl<'info> Stake<'info> {
    pub fn stake(&mut self, bumps: &StakeBumps) -> Result<()> {
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

        let has_attributes = attributes_fetched.is_ok();
        let mut attribute_list: Vec<Attribute> = Vec::new();

        if let Ok((_, attributes, _)) = attributes_fetched {
            for attribute in attributes.attribute_list {
                if attribute.key == "staked" {
                    if attribute.value == "true" {
                        return Err(ErrorCode::AlreadyStaked.into());
                    }
                } else if attribute.key != "staked_at" && attribute.key != "last_claimed_at" {
                    attribute_list.push(attribute);
                }
            }
        }

        attribute_list.push(Attribute {
            key: "staked".to_string(),
            value: "true".to_string(),
        });

        attribute_list.push(Attribute {
            key: "staked_at".to_string(),
            value: Clock::get()?.unix_timestamp.to_string(),
        });

        if !has_attributes {
            AddPluginV1CpiBuilder::new(&self.mpl_core_program)
                .asset(&self.asset)
                .collection(Some(&self.collection))
                .payer(&self.owner)
                .authority(Some(&self.update_authority))
                .system_program(&self.system_program)
                .plugin(Plugin::Attributes(Attributes { attribute_list }))
                .invoke_signed(signer_seeds)?;
        } else {
            UpdatePluginV1CpiBuilder::new(&self.mpl_core_program)
                .asset(&self.asset)
                .collection(Some(&self.collection))
                .payer(&self.owner)
                .authority(Some(&self.update_authority))
                .system_program(&self.system_program)
                .plugin(Plugin::Attributes(Attributes { attribute_list }))
                .invoke_signed(signer_seeds)?;
        }

        // An NFT that was staked before still carries the FreezeDelegate plugin from
        // last time (unstake only thaws it), so freeze it again instead of re-adding it
        let freeze_fetched =
            fetch_plugin::<BaseAssetV1, FreezeDelegate>(&self.asset, PluginType::FreezeDelegate);

        if freeze_fetched.is_ok() {
            UpdatePluginV1CpiBuilder::new(&self.mpl_core_program)
                .asset(&self.asset)
                .collection(Some(&self.collection))
                .payer(&self.owner)
                .authority(Some(&self.update_authority))
                .system_program(&self.system_program)
                .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
                .invoke_signed(signer_seeds)?;
        } else {
            AddPluginV1CpiBuilder::new(&self.mpl_core_program)
                .asset(&self.asset)
                .collection(Some(&self.collection))
                .payer(&self.owner)
                .authority(Some(&self.owner))
                .system_program(&self.system_program)
                .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
                .init_authority(PluginAuthority::Address {
                    address: self.update_authority.key(),
                })
                .invoke_signed(signer_seeds)?;
        }

        update_total_staked(
            &self.collection,
            &self.update_authority,
            &self.owner,
            &self.system_program,
            &self.mpl_core_program,
            signer_seeds,
            1,
        )?;

        Ok(())
    }
}
