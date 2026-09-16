mod common;

use {
    anchor_amm::error::ErrorCode,
    anchor_lang::{
        solana_program::{instruction::Instruction, program_pack::Pack, system_program},
        InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{self, get_associated_token_address_with_program_id},
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

fn setup_withdraw() -> DepositFixture {
    let mut f = setup_deposit();
    f.deposit(1_000, 4_000, 2_000).unwrap();
    f
}

fn instruction(f: &DepositFixture, lp_to_burn: u64, min_a: u64, min_b: u64) -> Instruction {
    Instruction::new_with_bytes(
        anchor_amm::id(),
        &anchor_amm::instruction::WithdrawFromPool {
            lp_to_burn,
            min_a,
            min_b,
        }
        .data(),
        anchor_amm::accounts::Withdraw {
            user: f.pool.payer.pubkey(),
            mint_a: f.pool.mint_a,
            mint_b: f.pool.mint_b,
            amm_config: f.pool.amm_config,
            pool_config: f.addresses.config,
            mint_lp: f.addresses.lp_mint,
            vault_a: f.addresses.vault_a,
            vault_b: f.addresses.vault_b,
            user_ata_l: f.user_lp,
            user_ata_a: f.user_a,
            user_ata_b: f.user_b,
            system_program: system_program::ID,
            token_program: token::ID,
            associated_token_program: associated_token::ID,
        }
        .to_account_metas(None),
    )
}

fn withdraw(f: &mut DepositFixture, lp: u64, min_a: u64, min_b: u64) -> TransactionResult {
    let ix = instruction(f, lp, min_a, min_b);
    send(&mut f.pool.base, &f.pool.payer, &[ix], &[])
}

#[test]
fn partial_withdrawal_accepts_exact_minimums() {
    let mut f = setup_withdraw();
    withdraw(&mut f, 500, 250, 1_000).unwrap();
    assert_eq!(f.balance(f.user_lp), 1_500);
    assert_eq!(f.supply(), 1_500);
    assert_eq!(f.balance(f.user_a), 9_250);
    assert_eq!(f.balance(f.user_b), 7_000);
    assert_eq!(f.balance(f.addresses.vault_a), 750);
    assert_eq!(f.balance(f.addresses.vault_b), 3_000);
}

#[test]
fn full_withdrawal_returns_reserves_and_burns_all_lp() {
    let mut f = setup_withdraw();
    withdraw(&mut f, 2_000, 1_000, 4_000).unwrap();
    assert_eq!(f.balance(f.user_lp), 0);
    assert_eq!(f.supply(), 0);
    assert_eq!(f.balance(f.user_a), 10_000);
    assert_eq!(f.balance(f.user_b), 10_000);
    assert_eq!(f.balance(f.addresses.vault_a), 0);
    assert_eq!(f.balance(f.addresses.vault_b), 0);
}

#[test]
fn rejects_either_output_below_its_minimum() {
    for (min_a, min_b) in [(251, 1_000), (250, 1_001)] {
        let mut f = setup_withdraw();
        let before = f.snapshot();
        assert_eq!(
            withdraw(&mut f, 500, min_a, min_b).unwrap_err().err,
            TransactionError::InstructionError(
                0,
                InstructionError::Custom(ErrorCode::SlippageExceeded.into())
            )
        );
        assert_eq!(before, f.snapshot());
    }
}

#[test]
fn rejects_zero_and_excessive_burn_amounts() {
    for lp in [0, 2_001] {
        let mut f = setup_withdraw();
        let before = f.snapshot();
        assert_eq!(
            withdraw(&mut f, lp, 0, 0).unwrap_err().err,
            TransactionError::InstructionError(
                0,
                InstructionError::Custom(ErrorCode::InvalidWithdrawAmount.into())
            )
        );
        assert_eq!(before, f.snapshot());
    }
}

#[test]
fn rejects_insufficient_lp_before_vault_transfers() {
    let mut f = setup_withdraw();
    let other = f.pool.base.maker.pubkey();
    let mint = f.addresses.lp_mint;
    let ata = get_associated_token_address_with_program_id(&other, &mint, &token::ID);
    let ixs=[associated_token::spl_associated_token_account::instruction::create_associated_token_account(&f.pool.payer.pubkey(),&other,&mint,&token::ID),
        spl_token::instruction::transfer(&token::ID,&f.user_lp,&ata,&f.pool.payer.pubkey(),&[],2_000).unwrap()];
    send(&mut f.pool.base, &f.pool.payer, &ixs, &[]).unwrap();
    let before = f.snapshot();
    let failure = withdraw(&mut f, 500, 250, 1_000).unwrap_err();
    assert_eq!(
        failure.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(spl_token::error::TokenError::InsufficientFunds as u32)
        )
    );
    assert!(!failure
        .meta
        .logs
        .iter()
        .any(|l| l.contains("Instruction: TransferChecked")));
    assert_eq!(before, f.snapshot());
}

#[test]
fn second_transfer_failure_rolls_back_burn_and_first_transfer() {
    let mut f = setup_withdraw();
    // Inject a frozen destination to exercise a failure inside the second CPI.
    let mut account = f.pool.base.svm.get_account(&f.user_b).unwrap();
    let mut state = spl_token::state::Account::unpack(&account.data).unwrap();
    state.state = spl_token::state::AccountState::Frozen;
    spl_token::state::Account::pack(state, &mut account.data).unwrap();
    f.pool.base.svm.set_account(f.user_b, account).unwrap();
    let before = f.snapshot();
    let failure = withdraw(&mut f, 500, 250, 1_000).unwrap_err();
    assert_eq!(
        failure.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(spl_token::error::TokenError::AccountFrozen as u32)
        )
    );
    assert!(failure
        .meta
        .logs
        .iter()
        .any(|l| l.contains("Instruction: Burn")));
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

#[test]
fn allows_withdrawal_while_amm_is_paused() {
    let mut f = setup_withdraw();
    f.pool.base.set_paused(f.pool.amm_config, 1).unwrap();
    withdraw(&mut f, 500, 250, 1_000).unwrap();
    assert_eq!(f.supply(), 1_500);
    assert_eq!(f.balance(f.user_a), 9_250);
    assert_eq!(f.balance(f.user_b), 7_000);
}

#[test]
fn missing_lp_account_is_not_created() {
    let mut f = setup_withdraw();
    let user = f.pool.base.maker.pubkey();
    let lp = get_associated_token_address_with_program_id(&user, &f.addresses.lp_mint, &token::ID);
    let mut ix = instruction(&f, 500, 250, 1_000);
    for meta in &mut ix.accounts {
        if meta.pubkey == f.pool.payer.pubkey() {
            meta.pubkey = user;
        } else if meta.pubkey == f.user_lp {
            meta.pubkey = lp;
        }
    }
    let message = solana_message::Message::new_with_blockhash(
        &[ix],
        Some(&user),
        &f.pool.base.svm.latest_blockhash(),
    );
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(message),
        &[&f.pool.base.maker],
    )
    .unwrap();
    assert_eq!(
        f.pool.base.svm.send_transaction(tx).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(anchor_lang::error::ErrorCode::AccountNotInitialized.into())
        )
    );
    assert!(f.pool.base.svm.get_account(&lp).is_none());
}
