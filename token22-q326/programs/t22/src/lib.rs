use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod instructions;

pub use constants::*;
pub use error::*;
pub use instructions::*;

declare_id!("37Nfd4hYKZSqL2RFwkQRucfsb96XAeT77fwWSZg7onF8");

#[program]
pub mod t22 {
    use super::*;

    ///the declarative path.
    /// Everything happens in the `#[account(init, ...)]` attribute on the
    /// `mint` field. The macro expands to exactly the sequence you would write
    /// by hand:
    ///
    ///   create_account -> each extension initializer -> initialize_mint2
    ///
    ///sizes the allocation with `find_mint_account_size`, which wraps
    /// `ExtensionType::try_calculate_account_len`
    pub fn create_mint_declarative(
        ctx: Context<CreateMintDeclarative>,
        decimals: u8,
    ) -> Result<()> {
        instructions::mint::handle_create_mint_declarative(ctx, decimals)
    }

    /// Anchor's `extensions::` constraints cover a closed set of seven:
    /// group_pointer, group_member_pointer, metadata_pointer, close_authority,
    /// permanent_delegate, transfer_hook and pausable. TransferFeeConfig is
    /// not among them, so a mint that charges a transfer fee cannot be
    /// expressed as a constraint at all.
    ///
    /// The fallback is to take the mint as an unchecked account and drive the
    /// three phases yourself with CPIs. The ordering discipline does not
    /// change, only who writes it.
    pub fn create_mint_with_fee(
        ctx: Context<CreateMintWithFee>,
        decimals: u8,
        basis_points: u16,
        maximum_fee: u64,
    ) -> Result<()> {
        instructions::mint::handle_create_mint_with_fee(ctx, decimals, basis_points, maximum_fee)
    }

    /// `InterfaceAccount<'info, Mint>` looks like it gives you the whole mint.
    /// It does not. Anchor's deserializer runs
    /// `StateWithExtensions::unpack(buf).map(|t| Mint(t.base))`, which parses
    /// the TLV region and then discards it, keeping only the base struct.
    ///
    /// So the typed account has decimals, supply and authorities, and knows
    /// nothing about extensions. To see them you re-borrow the raw bytes and
    /// run `StateWithExtensions` yourself. That is the same call the plain
    /// Rust client makes.
    ///
    /// Confidential TF: creating the mint.
    ///
    /// ConfidentialTransferMint is not one of Anchor's seven `extensions::`
    /// constraints, and anchor-spl ships no CPI helper for it either. So this
    /// builds the raw instruction and invokes it directly. Same three phases.
    pub fn create_confidential_mint(
        ctx: Context<CreateConfidentialMint>,
        decimals: u8,
        auto_approve_new_accounts: bool,
    ) -> Result<()> {
        instructions::confidential::handle_create_confidential_mint(
            ctx,
            decimals,
            auto_approve_new_accounts,
        )
    }

    /// Confidential transfer fees.
    ///
    /// A fee on a confidential transfer is a contradiction that has to be
    /// resolved: the fee is a percentage of an amount nobody can see. The
    /// resolution is that the withheld fee is itself an ElGamal ciphertext,
    /// encrypted under a key belonging to the withdraw withheld authority, so
    /// only that authority can total up what it is owed.
    ///
    /// That is why this extension takes an ElGamal public key rather than just
    /// an address, and why it requires both TransferFeeConfig and
    /// ConfidentialTransferMint to already be on the mint. Three extensions,
    /// one ordering, all before InitializeMint2.
    pub fn create_confidential_fee_mint(
        ctx: Context<CreateConfidentialFeeMint>,
        decimals: u8,
        basis_points: u16,
        maximum_fee: u64,
        withdraw_withheld_authority_elgamal_pubkey: [u8; 32],
    ) -> Result<()> {
        instructions::confidential::handle_create_confidential_fee_mint(
            ctx,
            decimals,
            basis_points,
            maximum_fee,
            withdraw_withheld_authority_elgamal_pubkey,
        )
    }

    /// Confidential: deposit.
    ///
    /// The only step of the confidential lifecycle a program can drive on its
    /// own. Moving tokens from the public balance into the pending
    /// confidential balance needs no zero knowledge proof, because the amount
    /// was already public before the move.
    pub fn deposit_confidential(
        ctx: Context<DepositConfidential>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        instructions::confidential::handle_deposit_confidential(ctx, amount, decimals)
    }

    /// Confidential: apply pending balance.
    ///
    /// Incoming deposits and transfers land in a pending balance that cannot
    /// be spent. Moving it to available requires the owner to supply the new
    /// available balance already encrypted under their AES key.
    ///
    /// Note what that means: the program cannot compute this value. It has no
    /// access to the owner's key. The ciphertext is an instruction argument,
    /// and the program is a pass through.
    pub fn apply_pending_balance(
        ctx: Context<ApplyPendingBalance>,
        expected_pending_balance_credit_counter: u64,
        new_decryptable_available_balance: [u8; AE_CIPHERTEXT_LEN],
    ) -> Result<()> {
        instructions::confidential::handle_apply_pending_balance(
            ctx,
            expected_pending_balance_credit_counter,
            new_decryptable_available_balance,
        )
    }

    /// Creates a mint whose permanent delegate is the payer.
    pub fn create_seizable_mint(ctx: Context<CreateSeizableMint>, decimals: u8) -> Result<()> {
        instructions::permanent_delegate::handle_create_seizable_mint(ctx, decimals)
    }

    /// CPI guard.
    ///
    /// Delegates authority over a token account to this program's PDA by
    /// cross program invoking Approve.
    ///
    /// This is the exact pattern a lending or escrow protocol uses, and it is
    /// also the exact pattern a malicious program uses to drain an account it
    /// tricked a user into signing for. Token-2022 cannot tell them apart, so
    /// it lets the account owner decide: with the CpiGuard extension enabled,
    /// Approve issued through a CPI fails outright. The owner can still
    /// approve by signing a top level instruction.
    pub fn delegate_to_program(ctx: Context<DelegateToProgram>, amount: u64) -> Result<()> {
        instructions::permanent_delegate::handle_delegate_to_program(ctx, amount)
    }

    /// Permanent delegate.
    ///
    /// Moves tokens out of an account using the mint's permanent delegate
    /// authority. Note what is missing: no Approve was ever issued by the
    /// holder, and the holder is not a signer here.
    ///
    /// A permanent delegate can move or burn tokens from every account of its
    /// mint, forever, without consent and with no way for a holder to revoke
    /// it. Any protocol accepting arbitrary Token-2022 mints must treat this
    /// extension as a reason to reject the mint, not a feature to support.
    pub fn permanent_delegate_seize(
        ctx: Context<PermanentDelegateSeize>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        instructions::permanent_delegate::handle_permanent_delegate_seize(ctx, amount, decimals)
    }

    pub fn assert_supported_mint(ctx: Context<AssertSupportedMint>) -> Result<()> {
        instructions::mint::handle_assert_supported_mint(ctx)
    }

    pub fn create_remittance_mint(
        ctx: Context<CreateRemittanceMint>,
        decimals: u8,
        basis_points: u16,
        maximum_fee: u64,
    ) -> Result<()> {
        instructions::remittance::handle_create_remittance_mint(
            ctx,
            decimals,
            basis_points,
            maximum_fee,
        )
    }

    pub fn safe_transfer_with_fee(
        ctx: Context<SafeTransferWithFee>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        instructions::remittance::handle_safe_transfer_with_fee(ctx, amount, decimals)
    }

    pub fn verify_state(ctx: Context<VerifyState>) -> Result<()> {
        instructions::mint::handle_verify_state(ctx)
    }

    pub fn unfreeze_account(ctx: Context<UnfreezeAccount>) -> Result<()> {
        instructions::mint::handle_unfreeze_account(ctx)
    }

    pub fn create_remittance_mint_with_confidential_transfer_and_permanent_delegate(
        ctx: Context<CreateRemittanceMint>,
        decimals: u8,
        basis_points: u16,
        maximum_fee: u64,
    ) -> Result<()> {
        instructions::remittance::handle_create_remittance_mint_with_confidential_transfer_and_permanent_delegate(ctx, decimals, basis_points, maximum_fee)
    }

    pub fn configure_account(
        ctx: Context<ConfigureAccount>,
        decryptable_zero_balance: [u8; AE_CIPHERTEXT_LEN],
        maximum_pending_balance_credit_counter: u64,
    ) -> Result<()> {
        instructions::confidential::handle_configure_account(
            ctx,
            decryptable_zero_balance,
            maximum_pending_balance_credit_counter,
        )
    }

    pub fn confidential_transfer(
        ctx: Context<ConfidentialTransfer>,
        new_source_decryptable_available_balance: [u8; AE_CIPHERTEXT_LEN],
        transfer_amount_auditor_ciphertext_lo: [u8; ELGAMAL_CIPHERTEXT_LEN],
        transfer_amount_auditor_ciphertext_hi: [u8; ELGAMAL_CIPHERTEXT_LEN],
    ) -> Result<()> {
        instructions::confidential::handle_confidential_transfer(
            ctx,
            new_source_decryptable_available_balance,
            transfer_amount_auditor_ciphertext_lo,
            transfer_amount_auditor_ciphertext_hi,
        )
    }

    pub fn withdraw_confidential(
        ctx: Context<WithdrawConfidential>,
        amount: u64,
        decimals: u8,
        new_decryptable_available_balance: [u8; AE_CIPHERTEXT_LEN],
    ) -> Result<()> {
        instructions::confidential::handle_withdraw_confidential(
            ctx,
            amount,
            decimals,
            new_decryptable_available_balance,
        )
    }
}
