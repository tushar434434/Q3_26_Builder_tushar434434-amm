use anchor_lang::prelude::*;

use crate::{error::ErrorCode, AmmConfig};

#[derive(Accounts)]
pub struct UpdateFee<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"amm", amm_config.maker.as_ref(), amm_config.id.to_le_bytes().as_ref()],
        bump = amm_config.bump,
        has_one = admin,
    )]
    pub amm_config: Account<'info, AmmConfig>,
}

impl<'info> UpdateFee<'info> {
    /// Updates the swap fee in basis points for every pool under the AMM.
    pub fn handler(&mut self, fee: u16) -> Result<()> {
        require!(fee < 10_000, ErrorCode::InvalidFee);
        self.amm_config.fee = fee;
        Ok(())
    }
}
