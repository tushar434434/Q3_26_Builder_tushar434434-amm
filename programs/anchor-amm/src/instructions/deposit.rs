use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface},
};

use crate::{error::ErrorCode, AmmConfig, DepositQuote, PoolConfig};

#[derive(Accounts)]
pub struct Deposit<'info> {
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
        init_if_needed,
        payer = user,
        associated_token::mint = mint_lp,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_ata_l: Box<InterfaceAccount<'info, TokenAccount>>,

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

impl<'info> Deposit<'info> {
    pub fn handler(&mut self, max_a: u64, max_b: u64, min_lp_out: u64) -> Result<()> {
        require!(
            self.mint_a.key() < self.mint_b.key(),
            ErrorCode::InvalidMintPair
        );
        require!(self.amm_config.paused == 0, ErrorCode::InvalidAmmState);

        let quote = if self.mint_lp.supply == 0 {
            DepositQuote::initial_liquidity(max_a, max_b)?
        } else {
            DepositQuote::quote_deposit(
                max_a,
                max_b,
                self.vault_a.amount,
                self.vault_b.amount,
                self.mint_lp.supply,
            )?
        };

        require!(quote.lp_out >= min_lp_out, ErrorCode::SlippageExceeded);

        token_interface::transfer_checked(
            CpiContext::new(
                self.token_program.key(),
                token_interface::TransferChecked {
                    from: self.user_ata_a.to_account_info(),
                    to: self.vault_a.to_account_info(),
                    mint: self.mint_a.to_account_info(),
                    authority: self.user.to_account_info(),
                },
            ),
            quote.amount_a,
            self.mint_a.decimals,
        )?;
        token_interface::transfer_checked(
            CpiContext::new(
                self.token_program.key(),
                token_interface::TransferChecked {
                    from: self.user_ata_b.to_account_info(),
                    to: self.vault_b.to_account_info(),
                    mint: self.mint_b.to_account_info(),
                    authority: self.user.to_account_info(),
                },
            ),
            quote.amount_b,
            self.mint_b.decimals,
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

        token_interface::mint_to_checked(
            CpiContext::new_with_signer(
                self.token_program.key(),
                token_interface::MintToChecked {
                    mint: self.mint_lp.to_account_info(),
                    authority: self.pool_config.to_account_info(),
                    to: self.user_ata_l.to_account_info(),
                },
                &[&pool_seeds],
            ),
            quote.lp_out,
            self.mint_lp.decimals,
        )?;

        Ok(())
    }
}
