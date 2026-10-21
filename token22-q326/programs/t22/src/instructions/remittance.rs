use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_2022::spl_token_2022::extension::transfer_fee::instruction::transfer_checked_with_fee;
use anchor_spl::token_interface::{
    initialize_mint2, mint_close_authority_initialize, permanent_delegate_initialize,
    spl_token_2022, transfer_fee_initialize, InitializeMint2, MintCloseAuthorityInitialize,
    PermanentDelegateInitialize, TokenInterface, TransferFeeInitialize,
};
use spl_token_2022::solana_zk_sdk::encryption::pod::elgamal::PodElGamalPubkey;
use spl_token_2022::{
    extension::{
        confidential_transfer::instruction as confidential_instruction,
        confidential_transfer_fee::instruction as confidential_fee_instruction,
        transfer_fee::TransferFeeConfig, BaseStateWithExtensions, ExtensionType,
        StateWithExtensions,
    },
    state::Mint as MintState,
};

pub fn handle_create_remittance_mint(
    ctx: Context<CreateRemittanceMint>,
    decimals: u8,
    basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {
    let extensions = [
        ExtensionType::TransferFeeConfig,
        ExtensionType::MetadataPointer,
        ExtensionType::DefaultAccountState,
        ExtensionType::MintCloseAuthority,
    ];

    // calculating space upfront becuase of extensions
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

    invoke(
        &spl_token_2022::extension::metadata_pointer::instruction::initialize(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.mint.key(),
            Some(ctx.accounts.payer.key()), // Metadata update authority
            Some(ctx.accounts.mint.key()),  // Metadata address = mint itself
        )?,
        &[
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
    )?;
    invoke(
&spl_token_2022::extension::default_account_state::instruction::initialize_default_account_state(
    &ctx.accounts.token_program.key(),
    &ctx.accounts.mint.key(),
    &spl_token_2022::state::AccountState::Frozen,
)?,
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
        Some(&ctx.accounts.payer.key()),
    )?;

    msg!(
        "remittance mint {} created with {} bytes",
        ctx.accounts.mint.key(),
        space
    );
    Ok(())
}

pub fn handle_safe_transfer_with_fee(
    ctx: Context<SafeTransferWithFee>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    let fee = {
        let data = ctx.accounts.mint.try_borrow_data()?;
        StateWithExtensions::<MintState>::unpack(&data)?
            .get_extension::<TransferFeeConfig>()?
            .calculate_epoch_fee(Clock::get()?.epoch, amount)
            .ok_or(ProgramError::InvalidArgument)?
    };

    invoke(
        &transfer_checked_with_fee(
            ctx.accounts.token_program.key,
            ctx.accounts.source.key,
            ctx.accounts.mint.key,
            ctx.accounts.destination.key,
            ctx.accounts.payer.key,
            &[],
            amount,
            decimals,
            fee,
        )?,
        &[
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.source.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination.to_account_info(),
            ctx.accounts.payer.to_account_info(),
        ],
    )?;

    Ok(())
}

pub fn handle_create_remittance_mint_with_confidential_transfer_and_permanent_delegate(
    ctx: Context<CreateRemittanceMint>,
    decimals: u8,
    basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {
    let extensions = [
        ExtensionType::TransferFeeConfig,
        ExtensionType::MetadataPointer,
        ExtensionType::DefaultAccountState,
        ExtensionType::MintCloseAuthority,
        ExtensionType::ConfidentialTransferMint,
        // Token-2022 rejects TransferFeeConfig + ConfidentialTransferMint
        // unless the confidential fee config is present as well.
        ExtensionType::ConfidentialTransferFeeConfig,
        ExtensionType::PermanentDelegate,
    ];

    // calculating space upfront becuase of extensions
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

    invoke(
        &spl_token_2022::extension::metadata_pointer::instruction::initialize(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.mint.key(),
            Some(ctx.accounts.payer.key()), // Metadata update authority
            Some(ctx.accounts.mint.key()),  // Metadata address = mint itself
        )?,
        &[
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
    )?;
    invoke(
&spl_token_2022::extension::default_account_state::instruction::initialize_default_account_state(
    &ctx.accounts.token_program.key(),
    &ctx.accounts.mint.key(),
    &spl_token_2022::state::AccountState::Frozen,
)?,
&[
    ctx.accounts.mint.to_account_info(),
    ctx.accounts.token_program.to_account_info(),
],
)?;
    permanent_delegate_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            PermanentDelegateInitialize {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        &ctx.accounts.payer.key(),
    )?;

    invoke(
        &confidential_instruction::initialize_mint(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.mint.key(),
            Some(ctx.accounts.payer.key()),
            false,
            None,
        )?,
        &[
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.mint.to_account_info(),
        ],
    )?;

    invoke(
        &confidential_fee_instruction::initialize_confidential_transfer_fee_config(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.mint.key(),
            Some(ctx.accounts.payer.key()),
            &PodElGamalPubkey::default(),
        )?,
        &[
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.mint.to_account_info(),
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
        Some(&ctx.accounts.payer.key()),
    )?;

    msg!(
        "Reissued confidential remittance mint initialized: {}",
        ctx.accounts.mint.key()
    );
    Ok(())
}

#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct CreateRemittanceMint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut, signer)]
    pub mint: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SafeTransferWithFee<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: validated by Token-2022 when checking the fee-bearing transfer.
    #[account(mut, owner = token_program.key())]
    pub source: UncheckedAccount<'info>,

    /// CHECK: validated by Token-2022 and the transfer fee logic in the handler.
    #[account(owner = token_program.key())]
    pub mint: UncheckedAccount<'info>,

    /// CHECK: validated by Token-2022 before the transfer completes.
    #[account(mut, owner = token_program.key())]
    pub destination: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}
