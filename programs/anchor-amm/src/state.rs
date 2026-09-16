use anchor_lang::prelude::*;

#[account(discriminator = [1])]
#[derive(InitSpace)]
pub struct AmmConfig {
    pub maker: Pubkey,
    pub admin: Pubkey,
    pub id: u64,
    pub fee: u16, // In basis points
    pub paused: u8,
    pub bump: u8,
}

#[account(discriminator = [2])]
#[derive(InitSpace)]
pub struct PoolConfig {
    pub amm_config: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub bump: u8,
    pub lp_bump: u8,
}
