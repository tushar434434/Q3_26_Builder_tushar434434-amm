mod common;

use {
    anchor_amm::error::ErrorCode,
    anchor_lang::{
        solana_program::{instruction::Instruction, program_pack::Pack, system_program},
        InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token,
        token::{self, spl_token},
    },
    common::{
        liquidity::{setup_deposit, DepositFixture},
        pool::send,
    },
    litesvm::types::TransactionResult,
    solana_signer::Signer,
    solana_transaction::{InstructionError, TransactionError},
};

fn setup_swap() -> DepositFixture {
    let mut f = setup_deposit();
    f.deposit(1_000, 4_000, 2_000).unwrap();
    f
}

fn swap(f: &mut DepositFixture, a_to_b: bool, amount_in: u64, min_out: u64) -> TransactionResult {
    let ix = Instruction::new_with_bytes(
        anchor_amm::id(),
        &anchor_amm::instruction::Swap {
            a_to_b,
            amount_in,
            min_out,
        }
        .data(),
        anchor_amm::accounts::Swap {
            user: f.pool.payer.pubkey(),
            mint_a: f.pool.mint_a,
            mint_b: f.pool.mint_b,
            amm_config: f.pool.amm_config,
            pool_config: f.addresses.config,
            vault_a: f.addresses.vault_a,
            vault_b: f.addresses.vault_b,
            user_ata_a: f.user_a,
            user_ata_b: f.user_b,
            system_program: system_program::ID,
            token_program: token::ID,
            associated_token_program: associated_token::ID,
        }
        .to_account_metas(None),
    );
    send(&mut f.pool.base, &f.pool.payer, &[ix], &[])
}

#[test]
fn swaps_a_to_b_at_exact_minimum_and_retains_fee() {
    let mut f = setup_swap();
    // At 30 bps: floor(100 * 9970 * 4000 / (1000 * 10000 + 100 * 9970)) = 362.
    swap(&mut f, true, 100, 362).unwrap();
    assert_eq!(f.balance(f.user_a), 8_900);
    assert_eq!(f.balance(f.user_b), 6_362);
    assert_eq!(f.balance(f.addresses.vault_a), 1_100);
    assert_eq!(f.balance(f.addresses.vault_b), 3_638);
    assert_eq!(f.balance(f.user_lp), 2_000);
    assert_eq!(f.supply(), 2_000);
    assert!(1_100_u128 * 3_638 > 1_000_u128 * 4_000);
}

#[test]
fn swaps_b_to_a_at_exact_minimum() {
    let mut f = setup_swap();
    // A 400 B input at 30 bps returns floor(90.661...) = 90 A.
    swap(&mut f, false, 400, 90).unwrap();
    assert_eq!(f.balance(f.user_a), 9_090);
    assert_eq!(f.balance(f.user_b), 5_600);
    assert_eq!(f.balance(f.addresses.vault_a), 910);
    assert_eq!(f.balance(f.addresses.vault_b), 4_400);
    assert_eq!(f.balance(f.user_lp), 2_000);
    assert_eq!(f.supply(), 2_000);
}

#[test]
fn rejects_slippage_in_both_directions() {
    for (direction, amount, min) in [(true, 100, 363), (false, 400, 91)] {
        let mut f = setup_swap();
        let before = f.snapshot();
        assert_eq!(
            swap(&mut f, direction, amount, min).unwrap_err().err,
            TransactionError::InstructionError(
                0,
                InstructionError::Custom(ErrorCode::SlippageExceeded.into())
            )
        );
        assert_eq!(before, f.snapshot());
    }
}

#[test]
fn rejects_paused_amm() {
    let mut f = setup_swap();
    f.pool.base.set_paused(f.pool.amm_config, 1).unwrap();
    let before = f.snapshot();
    assert_eq!(
        swap(&mut f, true, 100, 0).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(ErrorCode::InvalidAmmState.into())
        )
    );
    assert_eq!(before, f.snapshot());
}

#[test]
fn failed_output_transfer_rolls_back_input_in_both_directions() {
    for direction in [true, false] {
        let mut f = setup_swap();
        let destination = if direction { f.user_b } else { f.user_a };
        // Inject a frozen destination to force failure after input collection.
        let mut account = f.pool.base.svm.get_account(&destination).unwrap();
        let mut state = spl_token::state::Account::unpack(&account.data).unwrap();
        state.state = spl_token::state::AccountState::Frozen;
        spl_token::state::Account::pack(state, &mut account.data).unwrap();
        f.pool.base.svm.set_account(destination, account).unwrap();
        let before = f.snapshot();
        let failure = swap(&mut f, direction, 100, 0).unwrap_err();
        assert_eq!(
            failure.err,
            TransactionError::InstructionError(
                0,
                InstructionError::Custom(spl_token::error::TokenError::AccountFrozen as u32)
            )
        );
        assert_eq!(
            failure
                .meta
                .logs
                .iter()
                .filter(|l| l.contains("Instruction: TransferChecked"))
                .count(),
            2
        );
        assert_eq!(before, f.snapshot());
    }
}

#[test]
fn insufficient_input_does_not_pay_output() {
    for direction in [true, false] {
        let mut f = setup_swap();
        let before = f.snapshot();
        assert_eq!(
            swap(&mut f, direction, 10_001, 0).unwrap_err().err,
            TransactionError::InstructionError(
                0,
                InstructionError::Custom(spl_token::error::TokenError::InsufficientFunds as u32)
            )
        );
        assert_eq!(before, f.snapshot());
    }
}

#[test]
fn rejects_zero_input() {
    let mut f = setup_swap();
    let before = f.snapshot();
    assert_eq!(
        swap(&mut f, true, 0, 0).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(ErrorCode::InvalidSwapAmount.into())
        )
    );
    assert_eq!(before, f.snapshot());
}

#[test]
fn fee_updates_apply_to_existing_pool_in_both_directions() {
    // Independent expected outputs for reserves 1000 A / 4000 B.
    for (fee, direction, amount, out) in [
        (0, true, 100, 363),
        (100, true, 100, 360),
        (0, false, 400, 90),
        (100, false, 400, 90),
    ] {
        let mut f = setup_swap();
        let before = f.snapshot();
        f.pool.base.update_fee(f.pool.amm_config, fee).unwrap();
        assert_eq!(before, f.snapshot());
        swap(&mut f, direction, amount, out).unwrap();
        if direction {
            assert_eq!(f.balance(f.user_b), 6_000 + out);
            assert_eq!(f.balance(f.addresses.vault_a), 1_100);
        } else {
            assert_eq!(f.balance(f.user_a), 9_000 + out);
            assert_eq!(f.balance(f.addresses.vault_b), 4_400);
        }
        assert_eq!(f.supply(), 2_000);
    }
}
