mod common;

use {
    anchor_amm::AmmConfig,
    anchor_lang::{
        prelude::Pubkey, solana_program::instruction::Instruction, AccountDeserialize,
        InstructionData, ToAccountMetas,
    },
    common::{setup, TestFixture},
    litesvm::types::TransactionResult,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::{versioned::VersionedTransaction, InstructionError, TransactionError},
};

fn setup_admin(paused: u8) -> (TestFixture, Pubkey) {
    let mut f = setup();
    let initial = f.initialize_amm(65_537, 30, paused);
    initial.result.unwrap();
    (f, initial.amm_config)
}

fn instruction(config: Pubkey, admin: Pubkey, new_admin: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        anchor_amm::id(),
        &anchor_amm::instruction::TransferAdmin {}.data(),
        anchor_amm::accounts::TransferAdmin {
            admin,
            new_admin,
            amm_config: config,
        }
        .to_account_metas(None),
    )
}

fn submit(f: &mut TestFixture, ix: Instruction, extra: &[&Keypair]) -> TransactionResult {
    let mut signers = vec![&f.maker];
    if ix
        .accounts
        .iter()
        .any(|m| m.pubkey == f.admin.pubkey() && m.is_signer)
    {
        signers.push(&f.admin);
    }
    signers.extend_from_slice(extra);
    let message =
        Message::new_with_blockhash(&[ix], Some(&f.maker.pubkey()), &f.svm.latest_blockhash());
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(message), &signers).unwrap();
    f.svm.send_transaction(tx)
}

fn read_config(f: &TestFixture, key: Pubkey) -> AmmConfig {
    AmmConfig::try_deserialize(&mut f.svm.get_account(&key).unwrap().data.as_slice()).unwrap()
}

#[test]
fn transfers_admin_and_preserves_other_fields_even_while_paused() {
    for paused in [0, 1] {
        let (mut f, key) = setup_admin(paused);
        let before = read_config(&f, key);
        let new_admin = Keypair::new();
        let ix = instruction(key, f.admin.pubkey(), new_admin.pubkey());
        submit(&mut f, ix, &[&new_admin]).unwrap();
        let after = read_config(&f, key);
        assert_eq!(after.admin, new_admin.pubkey());
        assert_eq!(after.maker, before.maker);
        assert_eq!(after.id, before.id);
        assert_eq!(after.fee, before.fee);
        assert_eq!(after.paused, before.paused);
        assert_eq!(after.bump, before.bump);
    }
}

#[test]
fn rejects_missing_signature_from_either_admin() {
    for missing in [0, 1] {
        let (mut f, key) = setup_admin(0);
        let before = f.svm.get_account(&key).unwrap();
        let new_admin = Keypair::new();
        let mut ix = instruction(key, f.admin.pubkey(), new_admin.pubkey());
        // Deliberately remove the signer meta so Anchor must reject the unsigned account.
        ix.accounts[missing].is_signer = false;
        let extra = if missing == 0 {
            vec![&new_admin]
        } else {
            vec![]
        };
        assert_eq!(
            submit(&mut f, ix, &extra).unwrap_err().err,
            TransactionError::InstructionError(
                0,
                InstructionError::Custom(anchor_lang::error::ErrorCode::AccountNotSigner.into())
            )
        );
        assert_eq!(f.svm.get_account(&key).unwrap(), before);
    }
}

#[test]
fn rejects_maker_when_maker_is_not_admin() {
    let (mut f, key) = setup_admin(0);
    let before = f.svm.get_account(&key).unwrap();
    let new_admin = Keypair::new();
    let ix = instruction(key, f.maker.pubkey(), new_admin.pubkey());
    assert_eq!(
        submit(&mut f, ix, &[&new_admin]).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(anchor_lang::error::ErrorCode::ConstraintHasOne.into())
        )
    );
    assert_eq!(f.svm.get_account(&key).unwrap(), before);
}

#[test]
fn rejects_config_at_wrong_pda() {
    let (mut f, key) = setup_admin(0);
    let before = f.svm.get_account(&key).unwrap();
    let wrong = Pubkey::new_unique();
    f.svm.set_account(wrong, before.clone()).unwrap();
    let new_admin = Keypair::new();
    let ix = instruction(wrong, f.admin.pubkey(), new_admin.pubkey());
    assert_eq!(
        submit(&mut f, ix, &[&new_admin]).unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(anchor_lang::error::ErrorCode::ConstraintSeeds.into())
        )
    );
    assert_eq!(f.svm.get_account(&wrong).unwrap(), before);
    assert_eq!(f.svm.get_account(&key).unwrap(), before);
}

#[test]
fn same_admin_transfer_is_a_valid_no_op() {
    let (mut f, key) = setup_admin(0);
    let before = f.svm.get_account(&key).unwrap();
    let ix = instruction(key, f.admin.pubkey(), f.admin.pubkey());
    submit(&mut f, ix, &[]).unwrap();
    assert_eq!(f.svm.get_account(&key).unwrap(), before);
}

#[test]
fn old_admin_loses_controls_and_new_admin_can_manage_and_transfer_again() {
    let (mut f, key) = setup_admin(0);
    let new_admin = Keypair::new();
    let ix = instruction(key, f.admin.pubkey(), new_admin.pubkey());
    submit(&mut f, ix, &[&new_admin]).unwrap();
    let before = f.svm.get_account(&key).unwrap();
    let expected = TransactionError::InstructionError(
        0,
        InstructionError::Custom(anchor_lang::error::ErrorCode::ConstraintHasOne.into()),
    );
    assert_eq!(f.set_paused(key, 1).unwrap_err().err, expected);
    assert_eq!(f.update_fee(key, 100).unwrap_err().err, expected);
    let third_admin = Keypair::new();
    let ix = instruction(key, f.admin.pubkey(), third_admin.pubkey());
    assert_eq!(
        submit(&mut f, ix, &[&third_admin]).unwrap_err().err,
        expected
    );
    assert_eq!(f.svm.get_account(&key).unwrap(), before);

    let pause_ix = Instruction::new_with_bytes(
        anchor_amm::id(),
        &anchor_amm::instruction::SetPaused { paused: 1 }.data(),
        anchor_amm::accounts::SetPaused {
            admin: new_admin.pubkey(),
            amm_config: key,
        }
        .to_account_metas(None),
    );
    submit(&mut f, pause_ix, &[&new_admin]).unwrap();
    let fee_ix = Instruction::new_with_bytes(
        anchor_amm::id(),
        &anchor_amm::instruction::UpdateFee { fee: 100 }.data(),
        anchor_amm::accounts::UpdateFee {
            admin: new_admin.pubkey(),
            amm_config: key,
        }
        .to_account_metas(None),
    );
    submit(&mut f, fee_ix, &[&new_admin]).unwrap();
    assert_eq!(read_config(&f, key).paused, 1);
    assert_eq!(read_config(&f, key).fee, 100);
    let ix = instruction(key, new_admin.pubkey(), third_admin.pubkey());
    submit(&mut f, ix, &[&new_admin, &third_admin]).unwrap();
    assert_eq!(read_config(&f, key).admin, third_admin.pubkey());
}
