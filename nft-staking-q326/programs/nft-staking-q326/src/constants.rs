use anchor_lang::prelude::*;

pub const CONFIG_SEED: &[u8] = b"config";
pub const REWARDS_SEED: &[u8] = b"rewards";
pub const UPDATE_AUTHORITY_SEED: &[u8] = b"update_authority";

// Whole reward tokens paid once when a staked NFT is burned
pub const BURN_BONUS_TOKENS: u64 = 1_000;
