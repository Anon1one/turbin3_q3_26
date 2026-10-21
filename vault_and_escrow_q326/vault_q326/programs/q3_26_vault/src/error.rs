use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Deposit amount must be greater than zero")]
    InvalidAmount,
    #[msg("Withdraw would leave the vault below rent exemption; use close to empty it")]
    WithdrawBelowRentExempt,
}
