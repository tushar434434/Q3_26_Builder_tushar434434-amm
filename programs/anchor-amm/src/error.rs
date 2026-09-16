use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Fee must be less than 10,000 basis points")]
    InvalidFee,
    #[msg("Paused must be 0 or 1")]
    InvalidPaused,
    #[msg("Provided mints must be ordered correctly, and not be identical")]
    InvalidMintPair,
    #[msg("Cannot initialize pool with a paused AMM Config")]
    InvalidAmmState,
    #[msg("Deposit amounts must be greater than zero")]
    InvalidDepositAmount,
    #[msg("Expected nonzero reserves and LP supply")]
    InvalidLiquidityState,
    #[msg("Deposit is too small to mint LP tokens")]
    InsufficientLiquidityMinted,
    #[msg("Arithmetic result exceeds the supported range")]
    ArithmeticOverflow,
    #[msg("LP amount must be positive and cannot exceed total supply")]
    InvalidWithdrawAmount,
    #[msg("Withdrawal must return at least one raw unit of each token")]
    InsufficientWithdrawOutput,
    #[msg("Swap input amount must be greater than zero")]
    InvalidSwapAmount,
    #[msg("Swap output rounds to zero")]
    InsufficientSwapOutput,
    #[msg("Swap output must be less than the output reserve")]
    InsufficientSwapLiquidity,
    #[msg("Output amount is below the user's minimum")]
    SlippageExceeded,
}
