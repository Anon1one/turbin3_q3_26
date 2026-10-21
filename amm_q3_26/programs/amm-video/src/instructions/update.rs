use anchor_lang::prelude::*;

use crate::{error::AmmError, state::Config};

#[derive(Accounts)]
pub struct Update<'info> {
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
    )]
    pub config: Account<'info, Config>,
}

impl<'info> Update<'info> {
    pub fn set_locked(&mut self, locked: bool) -> Result<()> {
        // A pool created without an authority can never be locked or unlocked
        let authority = self.config.authority.ok_or(AmmError::NoAuthoritySet)?;
        require_keys_eq!(authority, self.authority.key(), AmmError::InvalidAuthority);

        self.config.locked = locked;
        Ok(())
    }
}
