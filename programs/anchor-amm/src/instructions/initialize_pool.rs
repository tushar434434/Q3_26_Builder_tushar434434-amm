use anchor_lang::prelude::{Account, *};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{error::ErrorCode, AmmConfig, PoolConfig};

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct InitializePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub maker: SystemAccount<'info>,

    #[account(
        seeds = [b"amm", maker.key().as_ref(), id.to_le_bytes().as_ref()],
        bump = amm_config.bump,
    )]
    pub amm_config: Account<'info, AmmConfig>,

    #[account(
        mint::token_program = token_program
    )]
    pub mint_a: InterfaceAccount<'info, Mint>,

    #[account(
        mint::token_program = token_program
    )]
    pub mint_b: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = payer,
        space = 1 + PoolConfig::INIT_SPACE,
        seeds = [b"pool", amm_config.key().as_ref(), mint_a.key().as_ref(), mint_b.key().as_ref()],
        bump
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint_a,
        associated_token::authority = pool_config,
        associated_token::token_program = token_program,
    )]
    pub vault_a: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint_b,
        associated_token::authority = pool_config,
        associated_token::token_program = token_program,
    )]
    pub vault_b: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init,
        payer = payer,
        seeds = [b"lp_mint", pool_config.key().as_ref()],
        bump,
        mint::decimals = 6,
        mint::authority = pool_config,
        mint::token_program = token_program
    )]
    pub lp_mint: InterfaceAccount<'info, Mint>,

    pub system_program: Program<'info, System>,
    #[account(address = anchor_spl::token::ID)]
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> InitializePool<'info> {
    pub fn handler(&mut self, bumps: &InitializePoolBumps) -> Result<()> {
        require!(
            self.mint_a.key() != self.mint_b.key(),
            ErrorCode::InvalidMintPair
        );

        require!(self.amm_config.paused == 0, ErrorCode::InvalidAmmState);
        require!(
            self.mint_a.key() < self.mint_b.key(),
            ErrorCode::InvalidMintPair
        );

        self.pool_config.set_inner(PoolConfig {
            mint_a: self.mint_a.key(),
            mint_b: self.mint_b.key(),
            amm_config: self.amm_config.key(),
            bump: bumps.pool_config,
            lp_bump: bumps.lp_mint,
        });
        Ok(())
    }
}
