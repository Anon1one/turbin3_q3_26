use anchor_spl::token_interface::spl_token_2022::extension::ExtensionType;

// Instruction arguments in lib.rs are sized with this
pub use anchor_spl::token_interface::spl_token_2022::solana_zk_sdk::encryption::ELGAMAL_CIPHERTEXT_LEN;

// The length of a ciphertext which is how a decryptable balance is represented in the account data
pub const AE_CIPHERTEXT_LEN: usize = 36;

pub const SUPPORTED_EXTENSIONS: [ExtensionType; 3] = [
    ExtensionType::MintCloseAuthority,
    ExtensionType::MetadataPointer,
    ExtensionType::TransferFeeConfig,
];
