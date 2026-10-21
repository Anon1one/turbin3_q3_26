use {
    anchor_lang::solana_program::program_pack::Pack,
    anchor_spl::associated_token,
    litesvm::LiteSVM,
    litesvm_token::{
        spl_token::state::Account as TokenAccountState, CreateAssociatedTokenAccount, CreateMint,
    },
    solana_keypair::Keypair,
    solana_message::{Instruction, Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

mod ix_handlers;
use ix_handlers::*;

fn send(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> litesvm::types::TransactionResult {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

// Setup function to initialize LiteSVM and create a payer keypair
fn setup() -> (
    LiteSVM,
    Keypair,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
) {
    let program_id = amm_video::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/amm_video.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Create two mints (Mint A and Mint B) with 6 decimal places and the maker as the authority
    // This done using litesvm-token's CreateMint utility which creates the mint in the LiteSVM environment
    let mint_x = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let mint_y = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let config =
        Pubkey::find_program_address(&[b"config", &123u64.to_le_bytes()], &amm_video::id()).0;
    let mint_lp = Pubkey::find_program_address(&[b"lp", config.as_ref()], &amm_video::id()).0;
    let treasury = Pubkey::new_unique();

    // Derive the PDA for the vault associated token account using the config PDA and Mint A
    let vault_x = associated_token::get_associated_token_address(&config, &mint_x);
    let vault_y = associated_token::get_associated_token_address(&config, &mint_y);

    (
        svm, payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y,
    )
}

#[test]
fn test_initialize() {
    let (mut svm, payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y) = setup();

    let instruction = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y,
    );
    let res = send(&mut svm, &[instruction], &payer, &[&payer]);
    assert!(res.is_ok(), "initialize failed: {:?}", res);
}

#[test]
pub fn test_deposit() {
    let (mut svm, payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "deposit failed: {:?}", res);
}

#[test]
pub fn test_withdraw() {
    let (mut svm, payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let withdraw_ix = create_withdraw_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(
        &mut svm,
        &[init_ix, deposit_ix, withdraw_ix],
        &payer,
        &[&payer],
    );
    assert!(res.is_ok(), "withdraw failed: {:?}", res);
}

#[test]
pub fn test_swap() {
    let (mut svm, payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let swap_ix = create_swap_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y, treasury,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix, swap_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "swap failed: {:?}", res);

    let treasury_x = associated_token::get_associated_token_address(&treasury, &mint_x);
    let treasury_account = svm
        .get_account(&treasury_x)
        .expect("treasury x ATA should exist after swap");
    let treasury_x_state = TokenAccountState::unpack_from_slice(&treasury_account.data)
        .expect("treasury x ATA should decode as token account");

    assert!(treasury_x_state.amount > 0, "treasury x ATA should receive swap fees");
}

fn assert_fails_with(res: litesvm::types::TransactionResult, code: &str) {
    let err = res.expect_err("transaction should have failed");
    assert!(
        err.meta.logs.iter().any(|l| l.contains(code)),
        "expected {code}, got logs: {:#?}",
        err.meta.logs
    );
}

#[test]
pub fn test_swap_rejects_foreign_treasury() {
    let (mut svm, payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "setup failed: {:?}", res);

    // The swapper names a wallet of their own as treasury, so the fee would land there
    let own_treasury = Keypair::new();
    for mint in [&mint_x, &mint_y] {
        CreateAssociatedTokenAccount::new(&mut svm, &payer, mint)
            .owner(&own_treasury.pubkey())
            .send()
            .unwrap();
    }
    let swap_ix = create_swap_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y, own_treasury.pubkey(),
    );
    let res = send(&mut svm, &[swap_ix], &payer, &[&payer]);
    assert_fails_with(res, "InvalidTreasury");
}

#[test]
pub fn test_locked_pool_rejects_swap_until_unlocked() {
    let (mut svm, payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let lock_ix = create_set_locked_ix(payer.pubkey(), config, true);
    let res = send(&mut svm, &[init_ix, deposit_ix, lock_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "setup failed: {:?}", res);

    let swap_ix = create_swap_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y, treasury,
    );
    let res = send(&mut svm, &[swap_ix.clone()], &payer, &[&payer]);
    assert_fails_with(res, "PoolLocked");

    let unlock_ix = create_set_locked_ix(payer.pubkey(), config, false);
    let res = send(&mut svm, &[unlock_ix, swap_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "swap after unlock failed: {:?}", res);
}

#[test]
pub fn test_lock_rejects_non_authority() {
    let (mut svm, payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y,
    );
    let res = send(&mut svm, &[init_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "initialize failed: {:?}", res);

    let stranger = Keypair::new();
    svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();
    let lock_ix = create_set_locked_ix(stranger.pubkey(), config, true);
    let res = send(&mut svm, &[lock_ix], &stranger, &[&stranger]);
    assert_fails_with(res, "InvalidAuthority");
}

#[test]
pub fn test_initialize_rejects_fee_above_100_percent() {
    let (mut svm, payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix_with_fee(
        &mut svm, &payer, mint_x, mint_y, treasury, config, mint_lp, vault_x, vault_y, 10_001,
    );
    let res = send(&mut svm, &[init_ix], &payer, &[&payer]);
    assert_fails_with(res, "InvalidFee");
}

#[test]
pub fn test_initialize_rejects_same_mint_twice() {
    let (mut svm, payer, mint_x, _mint_y, treasury, config, mint_lp, vault_x, _vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_x, treasury, config, mint_lp, vault_x, vault_x,
    );
    // vault_y would be the same ATA as vault_x, so its init fails before the handler runs
    let res = send(&mut svm, &[init_ix], &payer, &[&payer]);
    assert!(res.is_err(), "a pool with the same mint on both sides should not initialize");
}
