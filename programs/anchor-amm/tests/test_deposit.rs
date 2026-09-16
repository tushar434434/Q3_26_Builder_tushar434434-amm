mod common;

use {
    anchor_amm::error::ErrorCode,
    anchor_spl::token::{self, spl_token},
    common::{
        liquidity::setup_deposit,
        pool::{send, setup_pool_with_program},
    },
    solana_transaction::{InstructionError, TransactionError},
};

#[test]
fn initial_deposit_accepts_exact_minimum() {
    let mut f = setup_deposit();
    f.deposit(1_000, 4_000, 2_000).unwrap();
    assert_eq!(f.balance(f.addresses.vault_a), 1_000);
    assert_eq!(f.balance(f.addresses.vault_b), 4_000);
    assert_eq!(f.balance(f.user_a), 9_000);
    assert_eq!(f.balance(f.user_b), 6_000);
    assert_eq!(f.balance(f.user_lp), 2_000);
    assert_eq!(f.supply(), 2_000);
}

#[test]
fn subsequent_deposits_respect_both_maximums() {
    let mut f = setup_deposit();
    f.deposit(1_000, 4_000, 2_000).unwrap();
    // A limits this deposit: accept 100 A and 400 B, not 500 B.
    f.deposit(100, 500, 200).unwrap();
    // B limits this deposit: accept 50 A and 200 B, not 100 A.
    f.deposit(100, 200, 100).unwrap();
    assert_eq!(f.balance(f.addresses.vault_a), 1_150);
    assert_eq!(f.balance(f.addresses.vault_b), 4_600);
    assert_eq!(f.balance(f.user_a), 8_850);
    assert_eq!(f.balance(f.user_b), 5_400);
    assert_eq!(f.balance(f.user_lp), 2_300);
    assert_eq!(f.supply(), 2_300);
}

#[test]
fn rejects_initial_deposit_below_minimum() {
    let mut f = setup_deposit();
    let before = f.snapshot();
    assert_eq!(
        f.deposit(1_000, 4_000, 2_001).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(ErrorCode::SlippageExceeded.into())
        )
    );
    assert_eq!(before, f.snapshot());
}

#[test]
fn rejects_subsequent_deposit_below_minimum() {
    let mut f = setup_deposit();
    f.deposit(1_000, 4_000, 2_000).unwrap();
    let before = f.snapshot();
    assert_eq!(
        f.deposit(100, 400, 201).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(ErrorCode::SlippageExceeded.into())
        )
    );
    assert_eq!(before, f.snapshot());
}

#[test]
fn second_transfer_failure_rolls_back_first_transfer_and_lp_account() {
    let mut f = setup_deposit();
    let before = f.snapshot();
    // A is funded, but the second transfer needs more B than the user owns.
    let failure = f.deposit(1_000, 10_001, 1).unwrap_err();
    assert_eq!(
        failure.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(spl_token::error::TokenError::InsufficientFunds as u32)
        )
    );
    assert!(
        failure
            .meta
            .logs
            .iter()
            .filter(|line| line.contains("Instruction: TransferChecked"))
            .count()
            >= 2
    );
    assert_eq!(before, f.snapshot());
}

#[test]
fn rejects_zero_initial_deposit() {
    let mut f = setup_deposit();
    let before = f.snapshot();
    assert_eq!(
        f.deposit(0, 1_000, 0).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(ErrorCode::InvalidDepositAmount.into())
        )
    );
    assert_eq!(before, f.snapshot());
}

#[test]
fn rejects_token_2022_pool_creation() {
    let mut f = setup_pool_with_program(0, anchor_spl::token_2022::ID);
    let (result, addresses) = f.initialize(f.mint_a, f.mint_b);
    assert_eq!(
        result.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(anchor_lang::error::ErrorCode::ConstraintAddress.into())
        )
    );
    for key in [
        addresses.config,
        addresses.vault_a,
        addresses.vault_b,
        addresses.lp_mint,
    ] {
        assert!(f.base.svm.get_account(&key).is_none());
    }
}

#[test]
fn rejects_token_2022_deposit_program() {
    let mut f = setup_deposit();
    let before = f.snapshot();
    let mut ix = f.instruction(1_000, 4_000, 2_000);
    let meta = ix
        .accounts
        .iter_mut()
        .find(|m| m.pubkey == token::ID)
        .unwrap();
    meta.pubkey = anchor_spl::token_2022::ID;
    assert!(send(&mut f.pool.base, &f.pool.payer, &[ix], &[]).is_err());
    assert_eq!(before, f.snapshot());
}
