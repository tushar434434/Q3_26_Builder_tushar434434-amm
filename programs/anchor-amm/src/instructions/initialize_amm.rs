use anchor_lang::prelude::*;

use crate::{error::ErrorCode, AmmConfig};

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct InitializeAmm<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    #[account()]
    pub admin: Signer<'info>,

    #[account(
        init,
        space = 1 + AmmConfig::INIT_SPACE,
        payer = maker,
        seeds = [b"amm", maker.key().as_ref(), id.to_le_bytes().as_ref()],
        bump
    )]
    pub amm_config: Account<'info, AmmConfig>,

    pub system_program: Program<'info, System>,
}

impl<'info> InitializeAmm<'info> {
    pub fn handler(
        &mut self,
        id: u64,
        fee: u16,
        paused: u8,
        bumps: &InitializeAmmBumps,
    ) -> Result<()> {
        require!(fee < 10_000, ErrorCode::InvalidFee);
        require!(paused <= 1, ErrorCode::InvalidPaused);

        self.amm_config.set_inner(AmmConfig {
            maker: self.maker.key(),
            admin: self.admin.key(),
            id,
            fee,
            paused,
            bump: bumps.amm_config,
        });

        Ok(())
    }
}
