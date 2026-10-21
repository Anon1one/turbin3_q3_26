use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid Owner")]
    InvalidOwner,
    #[msg("Invalid Collection")]
    InvalidCollection,
    #[msg("Invalid Update Authority")]
    InvalidUpdateAuthority,
    #[msg("Asset is already staked")]
    AlreadyStaked,
    #[msg("Asset is not staked")]
    NotStaked,
    #[msg("Freeze period has not elapsed")]
    FreezePeriodNotElapsed,
    #[msg("Math Overflow")]
    MathOverflow,
    #[msg("Signer is not the config admin")]
    Unauthorized,
}
