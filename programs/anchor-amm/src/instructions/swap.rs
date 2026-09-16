use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface},
};

use crate::{error::ErrorCode, AmmConfig, PoolConfig, SwapQuote};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mint::token_program = token_program,
    )]
    pub mint_a: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mint::token_program = token_program,
    )]
    pub mint_b: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        seeds = [b"amm", amm_config.maker.as_ref(), amm_config.id.to_le_bytes().as_ref()],
        bump = amm_config.bump,
    )]
    pub amm_config: Box<Account<'info, AmmConfig>>,

    #[account(
        seeds = [b"pool", amm_config.key().as_ref(), mint_a.key().as_ref(), mint_b.key().as_ref()],
        bump = pool_config.bump,
    )]
    pub pool_config: Box<Account<'info, PoolConfig>>,

    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = pool_config,
        associated_token::token_program = token_program,
    )]
    pub vault_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_b,
        associated_token::authority = pool_config,
        associated_token::token_program = token_program,
    )]
    pub vault_b: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_ata_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_b,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    #[account(address = anchor_spl::token::ID)]
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> Swap<'info> {
    /// Swaps an exact input amount; true selects A to B, false selects B to A.
    pub fn handler(&mut self, a_to_b: bool, amount_in: u64, min_out: u64) -> Result<()> {
        require!(
            self.mint_a.key() < self.mint_b.key(),
            ErrorCode::InvalidMintPair
        );
        require!(self.amm_config.paused == 0, ErrorCode::InvalidAmmState);

        let (input_mint, output_mint, input_vault, output_vault, user_input, user_output) =
            if a_to_b {
                (
                    &self.mint_a,
                    &self.mint_b,
                    &self.vault_a,
                    &self.vault_b,
                    &self.user_ata_a,
                    &self.user_ata_b,
                )
            } else {
                (
                    &self.mint_b,
                    &self.mint_a,
                    &self.vault_b,
                    &self.vault_a,
                    &self.user_ata_b,
                    &self.user_ata_a,
                )
            };

        let quote = SwapQuote::quote_swap_exact_in(
            amount_in,
            min_out,
            input_vault.amount,
            output_vault.amount,
            self.amm_config.fee,
        )?;

        // Collect the full input; the fee stays in the pool for LP holders.
        token_interface::transfer_checked(
            CpiContext::new(
                self.token_program.key(),
                token_interface::TransferChecked {
                    from: user_input.to_account_info(),
                    to: input_vault.to_account_info(),
                    mint: input_mint.to_account_info(),
                    authority: self.user.to_account_info(),
                },
            ),
            quote.amount_in,
            input_mint.decimals,
        )?;

        // Pool seeds retain their canonical A/B order in either swap direction.
        let amm_key = self.amm_config.key();
        let mint_a_key = self.mint_a.key();
        let mint_b_key = self.mint_b.key();
        let bump = [self.pool_config.bump];
        let pool_seeds: &[&[u8]] = &[
            b"pool",
            amm_key.as_ref(),
            mint_a_key.as_ref(),
            mint_b_key.as_ref(),
            &bump,
        ];

        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                self.token_program.key(),
                token_interface::TransferChecked {
                    from: output_vault.to_account_info(),
                    to: user_output.to_account_info(),
                    mint: output_mint.to_account_info(),
                    authority: self.pool_config.to_account_info(),
                },
                &[pool_seeds],
            ),
            quote.amount_out,
            output_mint.decimals,
        )?;

        Ok(())
    }
}
