mod common;

use {
    anchor_amm::{error::ErrorCode, AmmConfig},
    anchor_lang::{
        prelude::Pubkey, solana_program::instruction::Instruction, AccountDeserialize,
        InstructionData, ToAccountMetas,
    },
    common::{setup, TestFixture},
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::{versioned::VersionedTransaction, InstructionError, TransactionError},
};

fn config(f: &TestFixture, key: Pubkey) -> AmmConfig {
    AmmConfig::try_deserialize(&mut f.svm.get_account(&key).unwrap().data.as_slice()).unwrap()
}

#[test]
fn admin_can_update_fee_and_preserve_other_fields() {
    let mut f = setup();
    let initial = f.initialize_amm(65_537, 30, 0);
    initial.result.unwrap();
    let key = initial.amm_config;
    let original = config(&f, key);
    for fee in [0, 9_999, 30, 30] {
        f.svm.expire_blockhash();
        f.update_fee(key, fee).unwrap();
        let current = config(&f, key);
        assert_eq!(current.fee, fee);
        assert_eq!(current.maker, original.maker);
        assert_eq!(current.admin, original.admin);
        assert_eq!(current.id, original.id);
        assert_eq!(current.paused, original.paused);
        assert_eq!(current.bump, original.bump);
    }
}

#[test]
fn rejects_invalid_fee_values_without_state_change() {
    let mut f = setup();
    let initial = f.initialize_amm(65_537, 30, 0);
    initial.result.unwrap();
    let key = initial.amm_config;
    let before = f.svm.get_account(&key).unwrap();
    for fee in [10_000, u16::MAX] {
        assert_eq!(
            f.update_fee(key, fee).unwrap_err().err,
            TransactionError::InstructionError(
                0,
                InstructionError::Custom(ErrorCode::InvalidFee.into())
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
        &anchor_amm::instruction::UpdateFee { fee: 1 }.data(),
        anchor_amm::accounts::UpdateFee {
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
    let mut metas = anchor_amm::accounts::UpdateFee {
        admin: f.admin.pubkey(),
        amm_config: key,
    }
    .to_account_metas(None);
    metas[0].is_signer = false;
    let ix = Instruction::new_with_bytes(
        anchor_amm::id(),
        &anchor_amm::instruction::UpdateFee { fee: 1 }.data(),
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
        f.update_fee(wrong, 1).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(anchor_lang::error::ErrorCode::ConstraintSeeds.into())
        )
    );
    assert_eq!(f.svm.get_account(&wrong).unwrap(), account);
}

#[test]
fn initialization_uses_the_same_fee_bounds() {
    for fee in [0, 9_999, 10_000, u16::MAX] {
        let mut f = setup();
        let result = f.initialize_amm(65_537, fee, 0);
        if fee < 10_000 {
            result.result.unwrap();
            assert_eq!(config(&f, result.amm_config).fee, fee);
        } else {
            assert_eq!(
                result.result.unwrap_err().err,
                TransactionError::InstructionError(
                    0,
                    InstructionError::Custom(ErrorCode::InvalidFee.into())
                )
            );
            assert!(f.svm.get_account(&result.amm_config).is_none());
        }
    }
}

#[test]
fn admin_can_update_fee_while_paused() {
    let mut f = setup();
    let initial = f.initialize_amm(65_537, 30, 1);
    initial.result.unwrap();
    f.update_fee(initial.amm_config, 100).unwrap();
    let current = config(&f, initial.amm_config);
    assert_eq!(current.fee, 100);
    assert_eq!(current.paused, 1);
}
