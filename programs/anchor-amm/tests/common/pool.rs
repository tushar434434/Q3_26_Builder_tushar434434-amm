use {
    super::{setup, TestFixture},
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{
            instruction::Instruction, program_pack::Pack, system_instruction, system_program,
        },
        InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{self, get_associated_token_address_with_program_id},
        token::{self, spl_token},
    },
    litesvm::types::TransactionResult,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

const AMM_ID: u64 = 65_537;

pub struct PoolFixture {
    pub token_program: Pubkey,
    pub base: TestFixture,
    pub payer: Keypair,
    pub amm_config: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
}

pub struct PoolAddresses {
    pub config: Pubkey,
    pub bump: u8,
    pub vault_a: Pubkey,
    pub vault_b: Pubkey,
    pub lp_mint: Pubkey,
    pub lp_bump: u8,
}

pub fn send(
    base: &mut TestFixture,
    payer: &Keypair,
    instructions: &[Instruction],
    extra: &[&Keypair],
) -> TransactionResult {
    let message = Message::new_with_blockhash(
        instructions,
        Some(&payer.pubkey()),
        &base.svm.latest_blockhash(),
    );
    let mut signers = vec![payer];
    signers.extend_from_slice(extra);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(message), &signers).unwrap();
    base.svm.send_transaction(tx)
}

pub fn setup_pool(paused: u8) -> PoolFixture {
    setup_pool_with_program(paused, token::ID)
}

pub fn setup_pool_with_program(paused: u8, token_program: Pubkey) -> PoolFixture {
    let mut base = setup();
    let amm = base.initialize_amm(AMM_ID, 30, paused);
    amm.result.unwrap();
    // Pool creation is permissionless: the payer is distinct from the AMM maker.
    let payer = Keypair::new();
    base.svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    let mut mints = Vec::new();
    for decimals in [6, 9] {
        let mint = Keypair::new();
        let instructions = [
            system_instruction::create_account(
                &payer.pubkey(),
                &mint.pubkey(),
                base.svm
                    .minimum_balance_for_rent_exemption(spl_token::state::Mint::LEN),
                spl_token::state::Mint::LEN as u64,
                &token_program,
            ),
            anchor_spl::token_2022::spl_token_2022::instruction::initialize_mint2(
                &token_program,
                &mint.pubkey(),
                &payer.pubkey(),
                None,
                decimals,
            )
            .unwrap(),
        ];
        send(&mut base, &payer, &instructions, &[&mint]).unwrap();
        mints.push(mint.pubkey());
    }
    mints.sort();
    PoolFixture {
        token_program,
        base,
        payer,
        amm_config: amm.amm_config,
        mint_a: mints[0],
        mint_b: mints[1],
    }
}

impl PoolFixture {
    pub fn addresses(&self, mint_a: Pubkey, mint_b: Pubkey) -> PoolAddresses {
        let (config, bump) = Pubkey::find_program_address(
            &[
                b"pool",
                self.amm_config.as_ref(),
                mint_a.as_ref(),
                mint_b.as_ref(),
            ],
            &anchor_amm::id(),
        );
        let (lp_mint, lp_bump) =
            Pubkey::find_program_address(&[b"lp_mint", config.as_ref()], &anchor_amm::id());
        PoolAddresses {
            config,
            bump,
            lp_mint,
            lp_bump,
            vault_a: get_associated_token_address_with_program_id(
                &config,
                &mint_a,
                &self.token_program,
            ),
            vault_b: get_associated_token_address_with_program_id(
                &config,
                &mint_b,
                &self.token_program,
            ),
        }
    }

    pub fn initialize(
        &mut self,
        mint_a: Pubkey,
        mint_b: Pubkey,
    ) -> (TransactionResult, PoolAddresses) {
        let addresses = self.addresses(mint_a, mint_b);
        let ix = Instruction::new_with_bytes(
            anchor_amm::id(),
            &anchor_amm::instruction::InitializePool { _id: AMM_ID }.data(),
            anchor_amm::accounts::InitializePool {
                payer: self.payer.pubkey(),
                maker: self.base.maker.pubkey(),
                amm_config: self.amm_config,
                mint_a,
                mint_b,
                pool_config: addresses.config,
                vault_a: addresses.vault_a,
                vault_b: addresses.vault_b,
                lp_mint: addresses.lp_mint,
                system_program: system_program::ID,
                token_program: self.token_program,
                associated_token_program: associated_token::ID,
            }
            .to_account_metas(None),
        );
        (send(&mut self.base, &self.payer, &[ix], &[]), addresses)
    }
}
