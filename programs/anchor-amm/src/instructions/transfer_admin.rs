use anchor_lang::prelude::*;

use crate::AmmConfig;

#[derive(Accounts)]
pub struct TransferAdmin<'info> {
    pub admin: Signer<'info>,
    pub new_admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"amm", amm_config.maker.as_ref(), amm_config.id.to_le_bytes().as_ref()],
        bump = amm_config.bump,
        has_one = admin,
    )]
    pub amm_config: Account<'info, AmmConfig>,
}

impl<'info> TransferAdmin<'info> {
    /// Transfers AMM administration with approval from both admins.
    pub fn handler(&mut self) -> Result<()> {
        self.amm_config.admin = self.new_admin.key();
        Ok(())
    }
}
