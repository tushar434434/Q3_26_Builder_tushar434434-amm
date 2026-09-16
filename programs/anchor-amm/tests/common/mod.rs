#![allow(dead_code)]

use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    litesvm::{types::TransactionResult, LiteSVM},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

pub struct TestFixture {
    pub svm: LiteSVM,
    pub maker: Keypair,
    pub admin: Keypair,
}

pub struct InitializeOutcome {
    pub result: TransactionResult,
    pub amm_config: Pubkey,
    pub bump: u8,
}

pub fn setup() -> TestFixture {
    let maker = Keypair::new();
    let admin = Keypair::new();
    let mut svm = LiteSVM::new();
    // Build the SBF program before running these tests with `anchor test`.
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/anchor_amm.so"
    ));
    svm.add_program(anchor_amm::id(), bytes).unwrap();
    svm.airdrop(&maker.pubkey(), 1_000_000_000).unwrap();
    TestFixture { svm, maker, admin }
}

impl TestFixture {
    pub fn set_paused(&mut self, amm_config: Pubkey, paused: u8) -> TransactionResult {
        let ix = Instruction::new_with_bytes(
            anchor_amm::id(),
            &anchor_amm::instruction::SetPaused { paused }.data(),
            anchor_amm::accounts::SetPaused {
                admin: self.admin.pubkey(),
                amm_config,
            }
            .to_account_metas(None),
        );
        let message = Message::new_with_blockhash(
            &[ix],
            Some(&self.maker.pubkey()),
            &self.svm.latest_blockhash(),
        );
        let tx = VersionedTransaction::try_new(
            VersionedMessage::Legacy(message),
            &[&self.maker, &self.admin],
        )
        .unwrap();
        self.svm.send_transaction(tx)
    }

    pub fn update_fee(&mut self, amm_config: Pubkey, fee: u16) -> TransactionResult {
        let ix = Instruction::new_with_bytes(
            anchor_amm::id(),
            &anchor_amm::instruction::UpdateFee { fee }.data(),
            anchor_amm::accounts::UpdateFee {
                admin: self.admin.pubkey(),
                amm_config,
            }
            .to_account_metas(None),
        );
        let message = Message::new_with_blockhash(
            &[ix],
            Some(&self.maker.pubkey()),
            &self.svm.latest_blockhash(),
        );
        let tx = VersionedTransaction::try_new(
            VersionedMessage::Legacy(message),
            &[&self.maker, &self.admin],
        )
        .unwrap();
        self.svm.send_transaction(tx)
    }

    pub fn initialize_amm(&mut self, id: u64, fee: u16, paused: u8) -> InitializeOutcome {
        let program_id = anchor_amm::id();
        let (amm_config, bump) = Pubkey::find_program_address(
            &[b"amm", self.maker.pubkey().as_ref(), &id.to_le_bytes()],
            &program_id,
        );
        let instruction = Instruction::new_with_bytes(
            program_id,
            &anchor_amm::instruction::InitializeAmm { id, fee, paused }.data(),
            anchor_amm::accounts::InitializeAmm {
                maker: self.maker.pubkey(),
                admin: self.admin.pubkey(),
                amm_config,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        );
        let message = Message::new_with_blockhash(
            &[instruction],
            Some(&self.maker.pubkey()),
            &self.svm.latest_blockhash(),
        );
        let transaction = VersionedTransaction::try_new(
            VersionedMessage::Legacy(message),
            &[&self.maker, &self.admin],
        )
        .unwrap();
        InitializeOutcome {
            result: self.svm.send_transaction(transaction),
            amm_config,
            bump,
        }
    }
}

pub mod pool;

pub mod liquidity;
