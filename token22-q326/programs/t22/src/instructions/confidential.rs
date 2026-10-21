use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_interface::{
    initialize_mint2, spl_token_2022, transfer_fee_initialize, InitializeMint2, TokenInterface,
    TransferFeeInitialize,
};
use proofext::instruction::ProofLocation;
use spl_token_2022::solana_zk_sdk::encryption::{
    pod::elgamal::PodElGamalCiphertext, ELGAMAL_CIPHERTEXT_LEN,
};
use spl_token_2022::{
    extension::{
        confidential_transfer::{instruction as confidential_instruction, DecryptableBalance},
        confidential_transfer_fee::instruction as confidential_fee_instruction,
        ExtensionType,
    },
    state::Mint as MintState,
};

use crate::constants::*;

pub fn handle_create_confidential_mint(
    ctx: Context<CreateConfidentialMint>,
    decimals: u8,
    auto_approve_new_accounts: bool,
) -> Result<()> {
    let space = ExtensionType::try_calculate_account_len::<MintState>(&[
        ExtensionType::ConfidentialTransferMint,
    ])?;
    let lamports = Rent::get()?.minimum_balance(space);

    anchor_lang::system_program::create_account(
        CpiContext::new(
            ctx.accounts.system_program.key(),
            anchor_lang::system_program::CreateAccount {
                from: ctx.accounts.payer.to_account_info(),
                to: ctx.accounts.mint.to_account_info(),
            },
        ),
        lamports,
        space as u64,
        &ctx.accounts.token_program.key(),
    )?;

    // No auditor key here. Passing Some(pubkey) would let its holder
    // decrypt every transfer amount for this mint, which is the usual
    // compliance escape hatch.
    let ix = confidential_instruction::initialize_mint(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.mint.key(),
        Some(ctx.accounts.payer.key()),
        auto_approve_new_accounts,
        None,
    )?;
    invoke(
        &ix,
        &[
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
    )?;

    initialize_mint2(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            InitializeMint2 {
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        decimals,
        &ctx.accounts.payer.key(),
        None,
    )?;

    msg!(
        "confidential mint {} at {} bytes",
        ctx.accounts.mint.key(),
        space
    );
    Ok(())
}

pub fn handle_create_confidential_fee_mint(
    ctx: Context<CreateConfidentialFeeMint>,
    decimals: u8,
    basis_points: u16,
    maximum_fee: u64,
    withdraw_withheld_authority_elgamal_pubkey: [u8; 32],
) -> Result<()> {
    let space = ExtensionType::try_calculate_account_len::<MintState>(&[
        ExtensionType::TransferFeeConfig,
        ExtensionType::ConfidentialTransferMint,
        ExtensionType::ConfidentialTransferFeeConfig,
    ])?;
    let lamports = Rent::get()?.minimum_balance(space);

    anchor_lang::system_program::create_account(
        CpiContext::new(
            ctx.accounts.system_program.key(),
            anchor_lang::system_program::CreateAccount {
                from: ctx.accounts.payer.to_account_info(),
                to: ctx.accounts.mint.to_account_info(),
            },
        ),
        lamports,
        space as u64,
        &ctx.accounts.token_program.key(),
    )?;

    let mint_info = ctx.accounts.mint.to_account_info();
    let program_info = ctx.accounts.token_program.to_account_info();
    let infos = [mint_info.clone(), program_info.clone()];

    transfer_fee_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferFeeInitialize {
                token_program_id: program_info.clone(),
                mint: mint_info.clone(),
            },
        ),
        Some(&ctx.accounts.payer.key()),
        Some(&ctx.accounts.payer.key()),
        basis_points,
        maximum_fee,
    )?;

    invoke(
        &confidential_instruction::initialize_mint(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.mint.key(),
            Some(ctx.accounts.payer.key()),
            true,
            None,
        )?,
        &infos,
    )?;

    // The fee extension must come after ConfidentialTransferMint, because
    // Token-2022 checks that the confidential mint config already exists.
    invoke(
        &confidential_fee_instruction::initialize_confidential_transfer_fee_config(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.mint.key(),
            Some(ctx.accounts.payer.key()),
            &withdraw_withheld_authority_elgamal_pubkey.into(),
        )?,
        &infos,
    )?;

    initialize_mint2(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            InitializeMint2 { mint: mint_info },
        ),
        decimals,
        &ctx.accounts.payer.key(),
        None,
    )?;

    msg!(
        "confidential fee mint {} at {} bytes",
        ctx.accounts.mint.key(),
        space
    );
    Ok(())
}

pub fn handle_deposit_confidential(
    ctx: Context<DepositConfidential>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    let ix = confidential_instruction::deposit(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.token_account.key(),
        &ctx.accounts.mint.key(),
        amount,
        decimals,
        &ctx.accounts.authority.key(),
        &[],
    )?;
    invoke(
        &ix,
        &[
            ctx.accounts.token_account.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
    )?;
    Ok(())
}

pub fn handle_apply_pending_balance(
    ctx: Context<ApplyPendingBalance>,
    expected_pending_balance_credit_counter: u64,
    new_decryptable_available_balance: [u8; AE_CIPHERTEXT_LEN],
) -> Result<()> {
    let balance = DecryptableBalance::from(new_decryptable_available_balance);
    let ix = confidential_instruction::apply_pending_balance(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.token_account.key(),
        expected_pending_balance_credit_counter,
        &balance,
        &ctx.accounts.authority.key(),
        &[],
    )?;
    invoke(
        &ix,
        &[
            ctx.accounts.token_account.to_account_info(),
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
    )?;
    Ok(())
}

pub fn handle_configure_account(
    ctx: Context<ConfigureAccount>,
    decryptable_zero_balance: [u8; AE_CIPHERTEXT_LEN],
    maximum_pending_balance_credit_counter: u64,
) -> Result<()> {
    let balance = DecryptableBalance::from(decryptable_zero_balance);

    // `ProofLocation` borrows, so it can't be an Anchor instruction argument.
    // The proof is pre-verified into a context state account instead.
    let proof_key = ctx.accounts.proof_account.key();
    let proof_data_location =
        ProofLocation::<confidential_instruction::PubkeyValidityProofData>::ContextStateAccount(
            &proof_key,
        );

    let ixs = confidential_instruction::configure_account(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.account.key(),
        &ctx.accounts.mint.key(),
        &balance,
        maximum_pending_balance_credit_counter,
        &ctx.accounts.authority.key(),
        &[],
        proof_data_location,
    )?;

    invoke(
        &ixs[0],
        &[
            ctx.accounts.account.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.proof_account.to_account_info(),
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
    )?;

    Ok(())
}

pub fn handle_confidential_transfer(
    ctx: Context<ConfidentialTransfer>,
    new_source_decryptable_available_balance: [u8; AE_CIPHERTEXT_LEN],
    transfer_amount_auditor_ciphertext_lo: [u8; ELGAMAL_CIPHERTEXT_LEN],
    transfer_amount_auditor_ciphertext_hi: [u8; ELGAMAL_CIPHERTEXT_LEN],
) -> Result<()> {
    let source_balance = DecryptableBalance::from(new_source_decryptable_available_balance);
    let auditor_ciphertext_lo = PodElGamalCiphertext::from(transfer_amount_auditor_ciphertext_lo);
    let auditor_ciphertext_hi = PodElGamalCiphertext::from(transfer_amount_auditor_ciphertext_hi);

    let equality_proof_key = ctx.accounts.equality_proof_account.key();
    let ciphertext_validity_proof_key = ctx.accounts.ciphertext_validity_proof_account.key();
    let range_proof_key = ctx.accounts.range_proof_account.key();

    let equality_proof_data_location = ProofLocation::<
        confidential_instruction::CiphertextCommitmentEqualityProofData,
    >::ContextStateAccount(&equality_proof_key);
    let ciphertext_validity_proof_data_location = ProofLocation::<
        confidential_instruction::BatchedGroupedCiphertext3HandlesValidityProofData,
    >::ContextStateAccount(
        &ciphertext_validity_proof_key
    );
    let range_proof_data_location = ProofLocation::<
        confidential_instruction::BatchedRangeProofU128Data,
    >::ContextStateAccount(&range_proof_key);

    let ixs = confidential_instruction::transfer(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.source.key(),
        &ctx.accounts.mint.key(),
        &ctx.accounts.destination.key(),
        &source_balance,
        &auditor_ciphertext_lo,
        &auditor_ciphertext_hi,
        &ctx.accounts.authority.key(),
        &[],
        equality_proof_data_location,
        ciphertext_validity_proof_data_location,
        range_proof_data_location,
    )?;

    invoke(
        &ixs[0],
        &[
            ctx.accounts.source.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination.to_account_info(),
            ctx.accounts.equality_proof_account.to_account_info(),
            ctx.accounts
                .ciphertext_validity_proof_account
                .to_account_info(),
            ctx.accounts.range_proof_account.to_account_info(),
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
    )?;

    Ok(())
}

pub fn handle_withdraw_confidential(
    ctx: Context<WithdrawConfidential>,
    amount: u64,
    decimals: u8,
    new_decryptable_available_balance: [u8; AE_CIPHERTEXT_LEN],
) -> Result<()> {
    let balance = DecryptableBalance::from(new_decryptable_available_balance);

    let equality_proof_key = ctx.accounts.equality_proof_account.key();
    let range_proof_key = ctx.accounts.range_proof_account.key();

    let equality_proof_data_location = ProofLocation::<
        confidential_instruction::CiphertextCommitmentEqualityProofData,
    >::ContextStateAccount(&equality_proof_key);
    let range_proof_data_location = ProofLocation::<
        confidential_instruction::BatchedRangeProofU64Data,
    >::ContextStateAccount(&range_proof_key);

    let ixs = confidential_instruction::withdraw(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.token_account.key(),
        &ctx.accounts.mint.key(),
        amount,
        decimals,
        &balance,
        &ctx.accounts.authority.key(),
        &[],
        equality_proof_data_location,
        range_proof_data_location,
    )?;

    invoke(
        &ixs[0],
        &[
            ctx.accounts.token_account.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.equality_proof_account.to_account_info(),
            ctx.accounts.range_proof_account.to_account_info(),
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct CreateConfidentialMint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: created and initialized in the handler, signs because the
    /// account is made at its own address.
    #[account(mut, signer)]
    pub mint: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateConfidentialFeeMint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: created and initialized in the handler.
    #[account(mut, signer)]
    pub mint: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DepositConfidential<'info> {
    /// CHECK: validated by Token-2022, which rejects any account that is not
    /// a token account for this mint configured for confidential transfers.
    #[account(mut, owner = token_program.key())]
    pub token_account: UncheckedAccount<'info>,

    /// CHECK: validated by Token-2022 during the deposit.
    #[account(owner = token_program.key())]
    pub mint: UncheckedAccount<'info>,

    pub authority: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ApplyPendingBalance<'info> {
    /// CHECK: validated by Token-2022.
    #[account(mut, owner = token_program.key())]
    pub token_account: UncheckedAccount<'info>,

    pub authority: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ConfigureAccount<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        owner = token_program.key()
    )]
    pub account: UncheckedAccount<'info>,

    #[account(owner = token_program.key())]
    pub mint: UncheckedAccount<'info>,

    /// CHECK: pre-verified pubkey-validity proof context state account,
    /// validated by the ZK ElGamal proof program during the CPI.
    pub proof_account: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ConfidentialTransfer<'info> {
    #[account(mut, owner = token_program.key())]
    pub source: UncheckedAccount<'info>,

    #[account(owner = token_program.key())]
    pub mint: UncheckedAccount<'info>,

    #[account(mut, owner = token_program.key())]
    pub destination: UncheckedAccount<'info>,

    /// CHECK: pre-verified equality proof context state account.
    pub equality_proof_account: UncheckedAccount<'info>,
    /// CHECK: pre-verified ciphertext-validity proof context state account.
    pub ciphertext_validity_proof_account: UncheckedAccount<'info>,
    /// CHECK: pre-verified range proof context state account.
    pub range_proof_account: UncheckedAccount<'info>,

    pub authority: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct WithdrawConfidential<'info> {
    #[account(mut, owner = token_program.key())]
    pub token_account: UncheckedAccount<'info>,

    #[account(owner = token_program.key())]
    pub mint: UncheckedAccount<'info>,

    /// CHECK: pre-verified equality proof context state account.
    pub equality_proof_account: UncheckedAccount<'info>,
    /// CHECK: pre-verified range proof context state account.
    pub range_proof_account: UncheckedAccount<'info>,

    pub authority: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}
