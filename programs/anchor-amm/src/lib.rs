pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use instructions::*;
pub use state::*;

declare_id!("CNKWbEdckorAEQJVoDnC5HNLgsqGsotqMnCbV9h4jVne");

#[program]
pub mod anchor_amm {
    use super::*;

    #[instruction(discriminator = [1])]
    pub fn initialize_amm(
        ctx: Context<InitializeAmm>,
        id: u64,
        fee: u16,
        paused: u8,
    ) -> Result<()> {
        ctx.accounts.handler(id, fee, paused, &ctx.bumps)
    }

    #[instruction(discriminator = [2])]
    pub fn initialize_pool(ctx: Context<InitializePool>, _id: u64) -> Result<()> {
        ctx.accounts.handler(&ctx.bumps)
    }

    #[instruction(discriminator = [3])]
    pub fn deposit_to_pool(
        ctx: Context<Deposit>,
        max_a: u64,
        max_b: u64,
        min_lp_out: u64,
    ) -> Result<()> {
        ctx.accounts.handler(max_a, max_b, min_lp_out)
    }

    #[instruction(discriminator = [4])]
    pub fn withdraw_from_pool(
        ctx: Context<Withdraw>,
        lp_to_burn: u64,
        min_a: u64,
        min_b: u64,
    ) -> Result<()> {
        ctx.accounts.handler(lp_to_burn, min_a, min_b)
    }
    #[instruction(discriminator = [5])]
    pub fn swap(ctx: Context<Swap>, a_to_b: bool, amount_in: u64, min_out: u64) -> Result<()> {
        ctx.accounts.handler(a_to_b, amount_in, min_out)
    }
    #[instruction(discriminator = [6])]
    pub fn set_paused(ctx: Context<SetPaused>, paused: u8) -> Result<()> {
        ctx.accounts.handler(paused)
    }
    #[instruction(discriminator = [7])]
    pub fn update_fee(ctx: Context<UpdateFee>, fee: u16) -> Result<()> {
        ctx.accounts.handler(fee)
    }
    #[instruction(discriminator = [8])]
    pub fn transfer_admin(ctx: Context<TransferAdmin>) -> Result<()> {
        ctx.accounts.handler()
    }
}
