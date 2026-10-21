use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    q3_26_vault::{STATE, VAULT_SEED},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

const DEPOSIT_LAMPORTS: u64 = 500_000_000;
const WITHDRAW_LAMPORTS: u64 = 100_000_000;

fn try_send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    ix: Instruction,
) -> litesvm::types::TransactionResult {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx)
}

fn send(svm: &mut LiteSVM, payer: &Keypair, ix: Instruction) {
    try_send(svm, payer, ix).unwrap_or_else(|err| {
        panic!("tx failed: {err:?}\nlogs: {logs:#?}", logs = err.meta.logs);
    });
}

fn withdraw_ix(user: &Keypair, vault_state: Pubkey, vault: Pubkey, amount: u64) -> Instruction {
    Instruction::new_with_bytes(
        q3_26_vault::id(),
        &q3_26_vault::instruction::Withdraw { amount }.data(),
        q3_26_vault::accounts::Withdraw {
            user: user.pubkey(),
            vault_state,
            vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

#[test]
fn test() {
    let program_id = q3_26_vault::id();
    let user = Keypair::new();
    let (vault_state, _) =
        Pubkey::find_program_address(&[STATE, user.pubkey().as_ref()], &program_id);
    let (vault, _) =
        Pubkey::find_program_address(&[VAULT_SEED, user.pubkey().as_ref()], &program_id);

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/q3_26_vault.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&user.pubkey(), 2_000_000_000).unwrap();

    send(
        &mut svm,
        &user,
        Instruction::new_with_bytes(
            program_id,
            &q3_26_vault::instruction::Initialize {}.data(),
            q3_26_vault::accounts::Initialize {
                user: user.pubkey(),
                vault_state,
                vault,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        ),
    );

    let rent_exempt = svm.minimum_balance_for_rent_exemption(0);
    assert!(
        svm.get_account(&vault_state).is_some(),
        "vault_state should exist after initialize"
    );
    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent_exempt,
        "vault should be rent-exempt after initialize"
    );

    send(
        &mut svm,
        &user,
        Instruction::new_with_bytes(
            program_id,
            &q3_26_vault::instruction::Deposit {
                amount: DEPOSIT_LAMPORTS,
            }
            .data(),
            q3_26_vault::accounts::Deposit {
                user: user.pubkey(),
                vault_state,
                vault,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        ),
    );

    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent_exempt + DEPOSIT_LAMPORTS,
        "vault should increase by the deposit"
    );

    send(
        &mut svm,
        &user,
        Instruction::new_with_bytes(
            program_id,
            &q3_26_vault::instruction::Withdraw {
                amount: WITHDRAW_LAMPORTS,
            }
            .data(),
            q3_26_vault::accounts::Withdraw {
                user: user.pubkey(),
                vault_state,
                vault,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        ),
    );

    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent_exempt + DEPOSIT_LAMPORTS - WITHDRAW_LAMPORTS,
        "vault should decrease by the withdraw"
    );

    // One lamport into the rent reserve is refused with a clear error
    let err = try_send(
        &mut svm,
        &user,
        withdraw_ix(
            &user,
            vault_state,
            vault,
            DEPOSIT_LAMPORTS - WITHDRAW_LAMPORTS + 1,
        ),
    )
    .expect_err("withdraw below rent exemption should fail");
    assert!(
        err.meta
            .logs
            .iter()
            .any(|l| l.contains("WithdrawBelowRentExempt")),
        "unexpected logs: {:#?}",
        err.meta.logs
    );

    send(
        &mut svm,
        &user,
        Instruction::new_with_bytes(
            program_id,
            &q3_26_vault::instruction::Close {}.data(),
            q3_26_vault::accounts::Close {
                user: user.pubkey(),
                vault_state,
                vault,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        ),
    );

    assert!(
        svm.get_account(&vault_state).is_none(),
        "vault_state should be closed"
    );
    assert_eq!(
        svm.get_balance(&vault).unwrap_or(0),
        0,
        "vault should be empty"
    );
}
