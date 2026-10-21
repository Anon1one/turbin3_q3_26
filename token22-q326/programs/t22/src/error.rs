use anchor_lang::prelude::*;

#[error_code]
pub enum MintError {
    #[msg("mint carries an extension this program has not been written to handle")]
    UnsupportedExtension,

    #[msg("user's specific account is already unfrozen")]
    AlreadyUnfrozen,
}
