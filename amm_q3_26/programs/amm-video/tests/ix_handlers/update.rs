use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    solana_pubkey::Pubkey,
};

pub fn create_set_locked_ix(authority: Pubkey, config: Pubkey, locked: bool) -> Instruction {
    let data = if locked {
        amm_video::instruction::Lock {}.data()
    } else {
        amm_video::instruction::Unlock {}.data()
    };

    Instruction::new_with_bytes(
        amm_video::id(),
        &data,
        amm_video::accounts::Update { authority, config }.to_account_metas(None),
    )
}
