mod common;

use {
    anchor_amm::{error::ErrorCode, AmmConfig},
    anchor_lang::{
        prelude::Pubkey, solana_program::instruction::Instruction, AccountDeserialize,
        InstructionData, ToAccountMetas,
    },
    common::{liquidity::setup_deposit, pool::setup_pool, setup, TestFixture},
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::{versioned::VersionedTransaction, InstructionError, TransactionError},
};

fn config(f: &TestFixture, key: Pubkey) -> AmmConfig {
    AmmConfig::try_deserialize(&mut f.svm.get_account(&key).unwrap().data.as_slice()).unwrap()
}

#[test]
fn admin_can_pause_resume_and_repeat_state_without_changing_other_fields() {
    let mut f = setup();
    let initial = f.initialize_amm(65_537, 30, 0);
    initial.result.unwrap();
    let key = initial.amm_config;
    let original = config(&f, key);
    for paused in [1, 1, 0, 0] {
        f.svm.expire_blockhash();
        f.set_paused(key, paused).unwrap();
        let current = config(&f, key);
        assert_eq!(current.paused, paused);
        assert_eq!(current.maker, original.maker);
        assert_eq!(current.admin, original.admin);
        assert_eq!(current.id, original.id);
        assert_eq!(current.fee, original.fee);
        assert_eq!(current.bump, original.bump);
    }
}

#[test]
fn rejects_invalid_pause_values_without_state_change() {
    let mut f = setup();
    let initial = f.initialize_amm(65_537, 30, 0);
    initial.result.unwrap();
    let key = initial.amm_config;
    let before = f.svm.get_account(&key).unwrap();
    for paused in [2, 255] {
        assert_eq!(
            f.set_paused(key, paused).unwrap_err().err,
            TransactionError::InstructionError(
                0,
                InstructionError::Custom(ErrorCode::InvalidPaused.into())
            )
        );
        assert_eq!(f.svm.get_account(&key).unwrap(), before);
    }
}

#[test]
fn maker_cannot_override_a_different_admin() {
    let mut f = setup();
    let initial = f.initialize_amm(65_537, 30, 0);
    initial.result.unwrap();
    let key = initial.amm_config;
    let before = f.svm.get_account(&key).unwrap();
    let ix = Instruction::new_with_bytes(
        anchor_amm::id(),
        &anchor_amm::instruction::SetPaused { paused: 1 }.data(),
        anchor_amm::accounts::SetPaused {
            admin: f.maker.pubkey(),
            amm_config: key,
        }
        .to_account_metas(None),
    );
    let message =
        Message::new_with_blockhash(&[ix], Some(&f.maker.pubkey()), &f.svm.latest_blockhash());
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[&f.maker]).unwrap();
    assert_eq!(
        f.svm.send_transaction(tx).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(anchor_lang::error::ErrorCode::ConstraintHasOne.into())
        )
    );
    assert_eq!(f.svm.get_account(&key).unwrap(), before);
}

#[test]
fn admin_address_without_signature_is_rejected() {
    let mut f = setup();
    let initial = f.initialize_amm(65_537, 30, 0);
    initial.result.unwrap();
    let key = initial.amm_config;
    let before = f.svm.get_account(&key).unwrap();
    let mut metas = anchor_amm::accounts::SetPaused {
        admin: f.admin.pubkey(),
        amm_config: key,
    }
    .to_account_metas(None);
    metas[0].is_signer = false;
    let ix = Instruction::new_with_bytes(
        anchor_amm::id(),
        &anchor_amm::instruction::SetPaused { paused: 1 }.data(),
        metas,
    );
    let message =
        Message::new_with_blockhash(&[ix], Some(&f.maker.pubkey()), &f.svm.latest_blockhash());
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[&f.maker]).unwrap();
    assert_eq!(
        f.svm.send_transaction(tx).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(anchor_lang::error::ErrorCode::AccountNotSigner.into())
        )
    );
    assert_eq!(f.svm.get_account(&key).unwrap(), before);
}

#[test]
fn rejects_config_at_wrong_pda() {
    let mut f = setup();
    let initial = f.initialize_amm(65_537, 30, 0);
    initial.result.unwrap();
    let wrong = Pubkey::new_unique();
    let account = f.svm.get_account(&initial.amm_config).unwrap();
    f.svm.set_account(wrong, account.clone()).unwrap();
    assert_eq!(
        f.set_paused(wrong, 1).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(anchor_lang::error::ErrorCode::ConstraintSeeds.into())
        )
    );
    assert_eq!(f.svm.get_account(&wrong).unwrap(), account);
}

#[test]
fn pause_blocks_deposits_and_resume_allows_them() {
    let mut f = setup_deposit();
    f.deposit(1_000, 4_000, 2_000).unwrap();
    f.pool.base.set_paused(f.pool.amm_config, 1).unwrap();
    let before = f.snapshot();
    assert_eq!(
        f.deposit(100, 400, 200).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(ErrorCode::InvalidAmmState.into())
        )
    );
    assert_eq!(before, f.snapshot());
    f.pool.base.set_paused(f.pool.amm_config, 0).unwrap();
    f.pool.base.svm.expire_blockhash();
    f.deposit(100, 400, 200).unwrap();
    assert_eq!(f.supply(), 2_200);
}

#[test]
fn pause_blocks_new_pools_and_resume_allows_them() {
    let mut f = setup_pool(0);
    f.base.set_paused(f.amm_config, 1).unwrap();
    let (result, addresses) = f.initialize(f.mint_a, f.mint_b);
    assert_eq!(
        result.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(ErrorCode::InvalidAmmState.into())
        )
    );
    assert!(f.base.svm.get_account(&addresses.config).is_none());
    f.base.set_paused(f.amm_config, 0).unwrap();
    f.base.svm.expire_blockhash();
    f.initialize(f.mint_a, f.mint_b).0.unwrap();
}
