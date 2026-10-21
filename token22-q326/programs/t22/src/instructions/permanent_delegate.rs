use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    approve, transfer_checked, Approve, Mint, TokenInterface, TransferChecked,
};

pub fn handle_create_seizable_mint(ctx: Context<CreateSeizableMint>, decimals: u8) -> Result<()> {
    msg!(
        "seizable mint {} with {} deccimals, permanent delegate",
        ctx.accounts.mint.key(),
        decimals,
        // ctx.accounts.payer.key()
    );
    Ok(())
}

pub fn handle_delegate_to_program(ctx: Context<DelegateToProgram>, amount: u64) -> Result<()> {
    approve(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            Approve {
                to: ctx.accounts.token_account.to_account_info(),
                delegate: ctx.accounts.delegate.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            },
        ),
        amount,
    )?;
    msg!("delegated {} to {}", amount, ctx.accounts.delegate.key());
    Ok(())
}

pub fn handle_permanent_delegate_seize(
    ctx: Context<PermanentDelegateSeize>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.source.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.destination.to_account_info(),
                authority: ctx.accounts.permanent_delegate.to_account_info(),
            },
        ),
        amount,
        decimals,
    )?;
    msg!("seized {} without holder consent", amount);
    Ok(())
}

#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct CreateSeizableMint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// `permanent_delegate` is one of the seven extensions Anchor can express
    /// as a constraint, so no manual CPI is needed here.
    #[account(
        init,
        payer = payer,
        mint::decimals = decimals,
        mint::authority = payer,
        mint::token_program = token_program,
        extensions::permanent_delegate::delegate = payer,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DelegateToProgram<'info> {
    /// CHECK: validated by Token-2022 during Approve.
    #[account(mut, owner = token_program.key())]
    pub token_account: UncheckedAccount<'info>,

    /// CHECK: any address may receive delegation; Token-2022 stores it as is.
    pub delegate: UncheckedAccount<'info>,

    pub owner: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct PermanentDelegateSeize<'info> {
    /// CHECK: validated by Token-2022. Note it is not a signer.
    #[account(mut, owner = token_program.key())]
    pub source: UncheckedAccount<'info>,

    /// CHECK: validated by Token-2022.
    #[account(owner = token_program.key())]
    pub mint: UncheckedAccount<'info>,

    /// CHECK: validated by Token-2022.
    #[account(mut, owner = token_program.key())]
    pub destination: UncheckedAccount<'info>,

    /// The mint's permanent delegate. Token-2022 checks this against the
    /// extension; the holder has no say.
    pub permanent_delegate: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}
