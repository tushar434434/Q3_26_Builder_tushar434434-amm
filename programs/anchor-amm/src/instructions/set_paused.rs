use anchor_lang::prelude::*;

use crate::{error::ErrorCode, AmmConfig};

#[derive(Accounts)]
pub struct SetPaused<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"amm", amm_config.maker.as_ref(), amm_config.id.to_le_bytes().as_ref()],
        bump = amm_config.bump,
        has_one = admin,
    )]
    pub amm_config: Account<'info, AmmConfig>,
}

impl<'info> SetPaused<'info> {
    /// Sets the AMM pause flag. Withdrawals remain available while paused.
    pub fn handler(&mut self, paused: u8) -> Result<()> {
        require!(paused <= 1, ErrorCode::InvalidPaused);
        self.amm_config.paused = paused;
        Ok(())
    }
}
