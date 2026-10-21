use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    initialize_mint2, mint_close_authority_initialize, spl_token_2022, transfer_fee_initialize,
    InitializeMint2, Mint, MintCloseAuthorityInitialize, TokenInterface, TransferFeeInitialize,
};
use spl_token_2022::{
    extension::{
        transfer_fee::TransferFeeConfig, BaseStateWithExtensions, ExtensionType,
        StateWithExtensions,
    },
    state::{Account as TokenAccountState, Mint as MintState},
};

use crate::{constants::*, error::*};

pub fn handle_create_mint_declarative(
    ctx: Context<CreateMintDeclarative>,
    decimals: u8,
) -> Result<()> {
    msg!(
        "mint {} created with {} decimals",
        ctx.accounts.mint.key(),
        decimals
    );
    Ok(())
}

pub fn handle_create_mint_with_fee(
    ctx: Context<CreateMintWithFee>,
    decimals: u8,
    basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {
    let extensions = [
        ExtensionType::MintCloseAuthority,
        ExtensionType::TransferFeeConfig,
    ];

    // Phase 1: allocate at the full extended length. Getting this number
    // from anywhere other than `try_calculate_account_len` is how mints
    // end up too small to initialize.
    let space = ExtensionType::try_calculate_account_len::<MintState>(&extensions)?;
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

    // Phase 2: initialize each extension, before the mint itself exists.
    mint_close_authority_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            MintCloseAuthorityInitialize {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        Some(&ctx.accounts.payer.key()),
    )?;

    transfer_fee_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferFeeInitialize {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        Some(&ctx.accounts.payer.key()),
        Some(&ctx.accounts.payer.key()),
        basis_points,
        maximum_fee,
    )?;

    // Phase 3: seal the mint. Nothing can be added after this point, and
    // most mint extensions cannot be added later at all, so a mistake here
    // is permanent rather than recoverable.
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
        "mint {} created with {} bytes",
        ctx.accounts.mint.key(),
        space
    );
    Ok(())
}

pub fn handle_assert_supported_mint(ctx: Context<AssertSupportedMint>) -> Result<()> {
    // Not available from the typed account. Drop to the raw bytes.
    let account_info = ctx.accounts.mint.to_account_info();
    let data = account_info.try_borrow_data()?;
    let state = StateWithExtensions::<MintState>::unpack(&data)?;

    // Available from the typed account, no extension awareness needed.
    let decimals = state.base.decimals;

    for extension in state.get_extension_types()? {
        require!(
            SUPPORTED_EXTENSIONS.contains(&extension),
            MintError::UnsupportedExtension
        );
    }

    // A transfer fee means the amount credited is not the amount debited.
    // Any accounting that assumes otherwise is wrong against this mint, so
    // read the live fee rather than assuming zero.
    //
    // Fees are epoch scheduled: `newer_transfer_fee` may not be in force
    // yet, which is why `get_epoch_fee` takes the current epoch.
    let basis_points = match state.get_extension::<TransferFeeConfig>() {
        Ok(config) => u16::from(
            config
                .get_epoch_fee(Clock::get()?.epoch)
                .transfer_fee_basis_points,
        ),
        Err(_) => 0,
    };

    msg!(
        "mint accepted: {} decimals, {} bps fee",
        decimals,
        basis_points
    );
    Ok(())
}

pub fn handle_verify_state(ctx: Context<VerifyState>) -> Result<()> {
    let account_info = ctx.accounts.mint.to_account_info();
    let data = account_info.try_borrow_data()?;
    let state = StateWithExtensions::<MintState>::unpack(&data)?;

    let decimals = state.base.decimals;
    let supply = state.base.supply;

    let fee_config = state.get_extension::<TransferFeeConfig>()?;
    let current_fee = fee_config.get_epoch_fee(Clock::get()?.epoch);

    msg!(
        "Mint supply: {}, decimals: {}, current fee basis points: {}",
        supply,
        decimals,
        u16::from(current_fee.transfer_fee_basis_points)
    );

    let account_data = ctx.accounts.token_account.try_borrow_data()?;
    let account_state = StateWithExtensions::<TokenAccountState>::unpack(&account_data)?;

    let balance = account_state.base.amount;
    let is_frozen = account_state.base.is_frozen();

    msg!(
        "Token Account balance: {}, is_frozen: {}",
        balance,
        is_frozen
    );
    Ok(())
}

pub fn handle_unfreeze_account(ctx: Context<UnfreezeAccount>) -> Result<()> {
    {
        let account_data = ctx.accounts.account.try_borrow_data()?;
        let account_state = StateWithExtensions::<TokenAccountState>::unpack(&account_data)?;

        // Ensure the account is actually frozen before attempting to thaw.
        require!(account_state.base.is_frozen(), MintError::AlreadyUnfrozen);
    } // account_data borrow is dropped here

    anchor_spl::token_2022::thaw_account(CpiContext::new(
        ctx.accounts.token_program.key(),
        anchor_spl::token_2022::ThawAccount {
            account: ctx.accounts.account.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            authority: ctx.accounts.freeze_authority.to_account_info(),
        },
    ))?;

    msg!("Account {} thawed successfully", ctx.accounts.account.key());
    Ok(())
}

#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct CreateMintDeclarative<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// each extension constraints below adds an
    /// `ExtensionType` to the size calculation and a CPI to the init sequence.
    #[account(
        init,
        payer = payer,
        mint::decimals = decimals,
        mint::authority = payer,
        mint::token_program = token_program,
        extensions::close_authority::authority = payer,
        extensions::metadata_pointer::authority = payer,
        extensions::metadata_pointer::metadata_address = payer,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateMintWithFee<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Unchecked because the account does not exist yet and Anchor has no
    /// constraint that can describe a transfer fee mint. The instruction body
    /// creates and initializes it.
    ///
    /// CHECK: created and initialized in the handler, and required to sign
    /// because the account is made at its own address.
    #[account(mut, signer)]
    pub mint: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AssertSupportedMint<'info> {
    /// Unchecked so the account is parsed exactly once, in the handler.
    ///
    /// The `owner` constraint is not optional. `StateWithExtensions::unpack`
    /// receives a byte slice and validates only the layout, so without this
    /// any account from any program whose bytes look like an initialized mint
    /// would be accepted.
    ///
    /// CHECK: ownership enforced below, contents allowlisted in the handler.
    #[account(owner = token_program.key())]
    pub mint: UncheckedAccount<'info>,

    /// Constrains `owner` above to SPL Token or Token-2022, and nothing else.
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct VerifyState<'info> {
    #[account(owner = token_program.key())]
    pub mint: UncheckedAccount<'info>,
    #[account(owner = token_program.key())]
    pub token_account: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct UnfreezeAccount<'info> {
    pub freeze_authority: Signer<'info>,

    #[account(
        mut,
        owner = token_program.key()
    )]
    pub account: UncheckedAccount<'info>,

    #[account(owner = token_program.key())]
    pub mint: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}
