use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface},
};

use crate::{error::ErrorCode, AmmConfig, PoolConfig, WithdrawQuote};

#[derive(Accounts)]
pub struct Withdraw<'info> {
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
        seeds = [b"lp_mint", pool_config.key().as_ref()],
        bump = pool_config.lp_bump,
        mint::token_program = token_program
    )]
    pub mint_lp: Box<InterfaceAccount<'info, Mint>>,

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
        associated_token::mint = mint_lp,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_ata_l: Box<InterfaceAccount<'info, TokenAccount>>,

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

impl<'info> Withdraw<'info> {
    pub fn handler(&mut self, lp_to_burn: u64, min_a: u64, min_b: u64) -> Result<()> {
        require!(
            self.mint_a.key() < self.mint_b.key(),
            ErrorCode::InvalidMintPair
        );

        let quote = WithdrawQuote::quote_withdraw(
            lp_to_burn,
            min_a,
            min_b,
            self.vault_a.amount,
            self.vault_b.amount,
            self.mint_lp.supply,
        )?;

        token_interface::burn(
            CpiContext::new(
                self.token_program.key(),
                token_interface::Burn {
                    mint: self.mint_lp.to_account_info(),
                    from: self.user_ata_l.to_account_info(),
                    authority: self.user.to_account_info(),
                },
            ),
            lp_to_burn,
        )?;

        let amm_config = self.amm_config.key();
        let mint_a = self.mint_a.key();
        let mint_b = self.mint_b.key();

        let pool_seeds = [
            b"pool",
            amm_config.as_ref(),
            mint_a.as_ref(),
            mint_b.as_ref(),
            &[self.pool_config.bump],
        ];

        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                self.token_program.key(),
                token_interface::TransferChecked {
                    from: self.vault_a.to_account_info(),
                    to: self.user_ata_a.to_account_info(),
                    mint: self.mint_a.to_account_info(),
                    authority: self.pool_config.to_account_info(),
                },
                &[&pool_seeds],
            ),
            quote.amount_a,
            self.mint_a.decimals,
        )?;
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                self.token_program.key(),
                token_interface::TransferChecked {
                    from: self.vault_b.to_account_info(),
                    to: self.user_ata_b.to_account_info(),
                    mint: self.mint_b.to_account_info(),
                    authority: self.pool_config.to_account_info(),
                },
                &[&pool_seeds],
            ),
            quote.amount_b,
            self.mint_b.decimals,
        )?;

        Ok(())
    }
}
