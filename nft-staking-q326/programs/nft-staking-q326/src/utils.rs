use anchor_lang::prelude::*;
use mpl_core::{
    accounts::BaseCollectionV1,
    fetch_plugin,
    instructions::{AddCollectionPluginV1CpiBuilder, UpdateCollectionPluginV1CpiBuilder},
    types::{Attribute, Attributes, Plugin, PluginType},
};

use crate::errors::ErrorCode;

/// Adds `delta` to the "total_staked" Attribute on the collection,
/// creating the Attributes plugin on the first stake.
pub fn update_total_staked<'info>(
    collection: &AccountInfo<'info>,
    update_authority: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    mpl_core_program: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
    delta: i64,
) -> Result<()> {
    let attributes_fetched =
        fetch_plugin::<BaseCollectionV1, Attributes>(collection, PluginType::Attributes);
    let has_attributes = attributes_fetched.is_ok();

    let mut attribute_list: Vec<Attribute> = Vec::new();
    let mut total_staked: u64 = 0;

    if let Ok((_, attributes, _)) = attributes_fetched {
        for attribute in attributes.attribute_list {
            if attribute.key == "total_staked" {
                total_staked = attribute.value.parse::<u64>().unwrap_or(0);
            } else {
                attribute_list.push(attribute);
            }
        }
    }

    let total_staked = if delta >= 0 {
        total_staked.checked_add(delta as u64)
    } else {
        total_staked.checked_sub(delta.unsigned_abs())
    }
    .ok_or(ErrorCode::MathOverflow)?;

    attribute_list.push(Attribute {
        key: "total_staked".to_string(),
        value: total_staked.to_string(),
    });

    if !has_attributes {
        AddCollectionPluginV1CpiBuilder::new(mpl_core_program)
            .collection(collection)
            .payer(payer)
            .authority(Some(update_authority))
            .system_program(system_program)
            .plugin(Plugin::Attributes(Attributes { attribute_list }))
            .invoke_signed(signer_seeds)?;
    } else {
        UpdateCollectionPluginV1CpiBuilder::new(mpl_core_program)
            .collection(collection)
            .payer(payer)
            .authority(Some(update_authority))
            .system_program(system_program)
            .plugin(Plugin::Attributes(Attributes { attribute_list }))
            .invoke_signed(signer_seeds)?;
    }

    Ok(())
}
