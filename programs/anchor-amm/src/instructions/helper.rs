use anchor_lang::prelude::*;

use crate::error::ErrorCode;

pub struct DepositQuote {
    pub amount_a: u64,
    pub amount_b: u64,
    pub lp_out: u64,
}
impl DepositQuote {
    /// Quotes the initial deposit and LP allocation as floor(sqrt(x * y)).
    /// Rejects zero deposit amounts. Does not account for existing vault
    /// balances or deduct locked liquidity.
    pub fn initial_liquidity(x: u64, y: u64) -> Result<Self> {
        require!(x > 0 && y > 0, ErrorCode::InvalidDepositAmount);

        let k: u128 = x as u128 * y as u128;

        Ok(Self {
            amount_a: x,
            amount_b: y,
            lp_out: k.isqrt() as u64,
        })
    }

    /// Quotes deposit amounts within the user's limits and the LP tokens to mint.
    /// Uses pre-deposit reserves and LP supply. Integer divisions round down.
    /// Requires an existing pool with nonzero reserves and LP supply.
    pub fn quote_deposit(max_a: u64, max_b: u64, x: u64, y: u64, l: u64) -> Result<Self> {
        require!(max_a > 0 && max_b > 0, ErrorCode::InvalidDepositAmount);
        require!(x > 0 && y > 0 && l > 0, ErrorCode::InvalidLiquidityState);

        // How much B would accompany the user's maximum A?
        let b_for_max_a = (max_a as u128 * y as u128) / x as u128;

        let (amount_a, amount_b) = if b_for_max_a <= max_b as u128 {
            // B fits within the limit, so A limits this deposit.
            (max_a, b_for_max_a as u64)
        } else {
            // B limits this deposit. Calculate its matching A.
            let a_for_max_b = (max_b as u128 * x as u128) / y as u128;

            let amount_a =
                u64::try_from(a_for_max_b).map_err(|_| error!(ErrorCode::ArithmeticOverflow))?;

            (amount_a, max_b)
        };

        // Integer division can round a matching amount down to zero.
        require!(
            amount_a > 0 && amount_b > 0,
            ErrorCode::InvalidDepositAmount
        );

        let lp_from_a = (amount_a as u128 * l as u128) / x as u128;
        let lp_from_b = (amount_b as u128 * l as u128) / y as u128;

        let lp_out = u64::try_from(lp_from_a.min(lp_from_b))
            .map_err(|_| error!(ErrorCode::ArithmeticOverflow))?;

        require!(lp_out > 0, ErrorCode::InsufficientLiquidityMinted);

        // The resulting total LP supply must also fit in u64.
        l.checked_add(lp_out)
            .ok_or_else(|| error!(ErrorCode::ArithmeticOverflow))?;

        // Check if resulting balance fits u64
        x.checked_add(amount_a)
            .ok_or_else(|| error!(ErrorCode::ArithmeticOverflow))?;

        y.checked_add(amount_b)
            .ok_or_else(|| error!(ErrorCode::ArithmeticOverflow))?;

        Ok(Self {
            amount_a,
            amount_b,
            lp_out,
        })
    }
}

pub struct WithdrawQuote {
    pub amount_a: u64,
    pub amount_b: u64,
}

impl WithdrawQuote {
    /// Quotes token amounts returned for burning a specified LP amount.
    /// Uses pre-withdrawal reserves and LP supply, rounding outputs down.
    /// Rejects zero outputs or outputs below the supplied minimum amounts.
    pub fn quote_withdraw(
        lp_to_burn: u64,
        min_a: u64,
        min_b: u64,
        x: u64,
        y: u64,
        l: u64,
    ) -> Result<Self> {
        require!(lp_to_burn > 0, ErrorCode::InvalidWithdrawAmount);
        require!(l > 0, ErrorCode::InvalidLiquidityState);
        require!(lp_to_burn <= l, ErrorCode::InvalidWithdrawAmount);

        let amount_a = ((x as u128 * lp_to_burn as u128) / l as u128) as u64;
        let amount_b = ((y as u128 * lp_to_burn as u128) / l as u128) as u64;

        require!(
            amount_a > 0 && amount_b > 0,
            ErrorCode::InsufficientWithdrawOutput
        );
        require!(
            amount_a >= min_a && amount_b >= min_b,
            ErrorCode::SlippageExceeded
        );
        Ok(Self { amount_a, amount_b })
    }
}

pub struct SwapQuote {
    pub amount_in: u64,
    pub amount_out: u64,
}

impl SwapQuote {
    /// Quotes the output for an exact-input swap, rounding down.
    /// Reserves are pre-swap balances ordered by input and output token.
    /// Assumes ordinary transfers and a fee retained in the pool.
    /// Rejects if the output token is lesser than user provided min output
    pub fn quote_swap_exact_in(
        amount_in: u64,
        min_out: u64,
        reserve_in: u64,
        reserve_out: u64,
        fee_bps: u16,
    ) -> Result<Self> {
        require!(
            amount_in > 0 && reserve_in > 0 && reserve_out > 0,
            ErrorCode::InvalidSwapAmount
        );
        require!(fee_bps < 10_000, ErrorCode::InvalidFee);

        const BPS_DENOMINATOR: u128 = 10_000;

        let adjusted_input = amount_in as u128 * (BPS_DENOMINATOR - fee_bps as u128);

        let numerator = adjusted_input
            .checked_mul(reserve_out as u128)
            .ok_or_else(|| error!(ErrorCode::ArithmeticOverflow))?;

        let denominator = reserve_in as u128 * BPS_DENOMINATOR + adjusted_input;

        let amount_out = u64::try_from(numerator / denominator)
            .map_err(|_| error!(ErrorCode::ArithmeticOverflow))?;

        // Check if resulting balance fits u64
        reserve_in
            .checked_add(amount_in)
            .ok_or_else(|| error!(ErrorCode::ArithmeticOverflow))?;

        require!(amount_out > 0, ErrorCode::InsufficientSwapOutput);
        require!(
            amount_out < reserve_out,
            ErrorCode::InsufficientSwapLiquidity
        );
        require!(amount_out >= min_out, ErrorCode::SlippageExceeded);

        Ok(Self {
            amount_in,
            amount_out,
        })
    }
}
