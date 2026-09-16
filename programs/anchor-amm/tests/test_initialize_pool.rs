mod common;
use {
    anchor_amm::{error::ErrorCode, PoolConfig},
    anchor_lang::{
        solana_program::{program_option::COption, program_pack::Pack},
        AccountDeserialize, Discriminator, Space,
    },
    anchor_spl::{
        associated_token,
        token::{self, spl_token},
    },
    common::pool::{send, setup_pool},
    solana_signer::Signer,
    solana_transaction::{InstructionError, TransactionError},
};

#[test]
fn creates_pool_vaults_and_lp_mint() {
    let mut fixture = setup_pool(0);
    let (result, addresses) = fixture.initialize(fixture.mint_a, fixture.mint_b);
    result.expect("pool initialization should succeed");
    let account = fixture.base.svm.get_account(&addresses.config).unwrap();
    assert_eq!(account.owner, anchor_amm::id());
    assert_eq!(
        account.data.len(),
        PoolConfig::DISCRIMINATOR.len() + PoolConfig::INIT_SPACE
    );
    let config = PoolConfig::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(config.amm_config, fixture.amm_config);
    assert_eq!(config.mint_a, fixture.mint_a);
    assert_eq!(config.mint_b, fixture.mint_b);
    assert_eq!(config.bump, addresses.bump);
    assert_eq!(config.lp_bump, addresses.lp_bump);
    for (address, mint) in [
        (addresses.vault_a, fixture.mint_a),
        (addresses.vault_b, fixture.mint_b),
    ] {
        let account = fixture.base.svm.get_account(&address).unwrap();
        assert_eq!(account.owner, token::ID);
        let vault = spl_token::state::Account::unpack(&account.data).unwrap();
        assert_eq!(vault.mint, mint);
        assert_eq!(vault.owner, addresses.config);
        assert_eq!(vault.amount, 0);
    }
    let account = fixture.base.svm.get_account(&addresses.lp_mint).unwrap();
    assert_eq!(account.owner, token::ID);
    let mint = spl_token::state::Mint::unpack(&account.data).unwrap();
    assert_eq!(mint.mint_authority, COption::Some(addresses.config));
    assert_eq!(mint.freeze_authority, COption::None);
    assert_eq!(mint.decimals, 6);
    assert_eq!(mint.supply, 0);
}

#[test]
fn rejects_reversed_mints() {
    let mut fixture = setup_pool(0);
    let (result, addresses) = fixture.initialize(fixture.mint_b, fixture.mint_a);
    assert_eq!(
        result.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(ErrorCode::InvalidMintPair.into())
        )
    );
    for key in [
        addresses.config,
        addresses.vault_a,
        addresses.vault_b,
        addresses.lp_mint,
    ] {
        assert!(fixture.base.svm.get_account(&key).is_none());
    }
}

#[test]
fn rejects_identical_mints() {
    let mut fixture = setup_pool(0);
    let (result, addresses) = fixture.initialize(fixture.mint_a, fixture.mint_a);
    // Duplicate mutable vaults may be rejected by Anchor before the handler runs.
    assert!(result.is_err());
    for key in [addresses.config, addresses.vault_a, addresses.lp_mint] {
        assert!(fixture.base.svm.get_account(&key).is_none());
    }
}

#[test]
fn rejects_paused_amm() {
    let mut fixture = setup_pool(1);
    let (result, addresses) = fixture.initialize(fixture.mint_a, fixture.mint_b);
    assert_eq!(
        result.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(ErrorCode::InvalidAmmState.into())
        )
    );
    for key in [
        addresses.config,
        addresses.vault_a,
        addresses.vault_b,
        addresses.lp_mint,
    ] {
        assert!(fixture.base.svm.get_account(&key).is_none());
    }
}

#[test]
fn rejects_duplicate_pool_without_changing_state() {
    let mut fixture = setup_pool(0);
    let (result, addresses) = fixture.initialize(fixture.mint_a, fixture.mint_b);
    result.unwrap();
    let keys = [
        addresses.config,
        addresses.vault_a,
        addresses.vault_b,
        addresses.lp_mint,
    ];
    let before: Vec<_> = keys
        .iter()
        .map(|key| fixture.base.svm.get_account(key).unwrap())
        .collect();
    // Ensure this reaches account validation instead of duplicate-transaction detection.
    fixture.base.svm.expire_blockhash();
    let (result, _) = fixture.initialize(fixture.mint_a, fixture.mint_b);
    assert_eq!(
        result.unwrap_err().err,
        TransactionError::InstructionError(0, InstructionError::Custom(0))
    );
    let after: Vec<_> = keys
        .iter()
        .map(|key| fixture.base.svm.get_account(key).unwrap())
        .collect();
    assert_eq!(before, after);
}

#[test]
fn reuses_preexisting_vaults() {
    let mut fixture = setup_pool(0);
    let addresses = fixture.addresses(fixture.mint_a, fixture.mint_b);
    let instructions: Vec<_> = [fixture.mint_a, fixture.mint_b].iter().map(|mint|
        associated_token::spl_associated_token_account::instruction::create_associated_token_account(
            &fixture.payer.pubkey(), &addresses.config, mint, &token::ID)
    ).collect();
    send(&mut fixture.base, &fixture.payer, &instructions, &[]).unwrap();
    let before_a = fixture.base.svm.get_account(&addresses.vault_a).unwrap();
    let before_b = fixture.base.svm.get_account(&addresses.vault_b).unwrap();
    let (result, _) = fixture.initialize(fixture.mint_a, fixture.mint_b);
    result.unwrap();
    assert_eq!(
        fixture.base.svm.get_account(&addresses.vault_a).unwrap(),
        before_a
    );
    assert_eq!(
        fixture.base.svm.get_account(&addresses.vault_b).unwrap(),
        before_b
    );
}
