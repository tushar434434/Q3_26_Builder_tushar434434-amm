mod common;

use {
    anchor_amm::{error::ErrorCode, AmmConfig},
    anchor_lang::{AccountDeserialize, Discriminator, Space},
    common::setup,
    solana_signer::Signer,
    solana_transaction::{InstructionError, TransactionError},
};

#[test]
fn initialize_amm_creates_config() {
    let mut fixture = setup();
    // Exercise the full u64 ID rather than the old u16 seed encoding.
    let id = 65_537;
    let fee = 30;
    let paused = 1;
    let outcome = fixture.initialize_amm(id, fee, paused);
    outcome.result.expect("AMM initialization should succeed");

    let account = fixture
        .svm
        .get_account(&outcome.amm_config)
        .expect("config should exist");
    assert_eq!(account.owner, anchor_amm::id());
    assert_eq!(
        account.data.len(),
        AmmConfig::DISCRIMINATOR.len() + AmmConfig::INIT_SPACE,
    );
    let config = AmmConfig::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(config.maker, fixture.maker.pubkey());
    assert_eq!(config.admin, fixture.admin.pubkey());
    assert_eq!(config.id, id);
    assert_eq!(config.fee, fee);
    assert_eq!(config.paused, paused);
    assert_eq!(config.bump, outcome.bump);
}

#[test]
fn rejects_invalid_fee() {
    let mut fixture = setup();
    let outcome = fixture.initialize_amm(65_537, 10_001, 0);
    let failure = outcome.result.expect_err("fee above 10,000 must fail");
    assert_eq!(
        failure.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(ErrorCode::InvalidFee.into())
        ),
    );
    assert!(fixture.svm.get_account(&outcome.amm_config).is_none());
}

#[test]
fn rejects_invalid_paused() {
    let mut fixture = setup();
    let outcome = fixture.initialize_amm(65_537, 30, 2);
    let failure = outcome
        .result
        .expect_err("paused other than 0 or 1 must fail");
    assert_eq!(
        failure.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(ErrorCode::InvalidPaused.into()),
        ),
    );
    assert!(fixture.svm.get_account(&outcome.amm_config).is_none());
}
