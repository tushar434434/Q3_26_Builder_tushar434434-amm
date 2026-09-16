use {
    super::pool::{send, setup_pool, PoolAddresses, PoolFixture},
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, program_pack::Pack, system_program},
        InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{self, get_associated_token_address_with_program_id},
        token::{self, spl_token},
    },
    litesvm::types::TransactionResult,
    solana_signer::Signer,
};

pub struct DepositFixture {
    pub pool: PoolFixture,
    pub addresses: PoolAddresses,
    pub user_a: Pubkey,
    pub user_b: Pubkey,
    pub user_lp: Pubkey,
}

pub fn setup_deposit() -> DepositFixture {
    let mut pool = setup_pool(0);
    let (result, addresses) = pool.initialize(pool.mint_a, pool.mint_b);
    result.unwrap();
    let user = pool.payer.pubkey();
    let mut user_accounts = Vec::new();
    for mint in [pool.mint_a, pool.mint_b] {
        let ata = get_associated_token_address_with_program_id(&user, &mint, &token::ID);
        let instructions = [
            associated_token::spl_associated_token_account::instruction::create_associated_token_account(&user, &user, &mint, &token::ID),
            spl_token::instruction::mint_to(&token::ID, &mint, &ata, &user, &[], 10_000).unwrap(),
        ];
        send(&mut pool.base, &pool.payer, &instructions, &[]).unwrap();
        user_accounts.push(ata);
    }
    let user_lp =
        get_associated_token_address_with_program_id(&user, &addresses.lp_mint, &token::ID);
    DepositFixture {
        pool,
        addresses,
        user_a: user_accounts[0],
        user_b: user_accounts[1],
        user_lp,
    }
}

impl DepositFixture {
    pub fn instruction(&self, a: u64, b: u64, min_lp_out: u64) -> Instruction {
        Instruction::new_with_bytes(
            anchor_amm::id(),
            &anchor_amm::instruction::DepositToPool {
                max_a: a,
                max_b: b,
                min_lp_out,
            }
            .data(),
            anchor_amm::accounts::Deposit {
                user: self.pool.payer.pubkey(),
                mint_a: self.pool.mint_a,
                mint_b: self.pool.mint_b,
                amm_config: self.pool.amm_config,
                pool_config: self.addresses.config,
                mint_lp: self.addresses.lp_mint,
                vault_a: self.addresses.vault_a,
                vault_b: self.addresses.vault_b,
                user_ata_a: self.user_a,
                user_ata_b: self.user_b,
                user_ata_l: self.user_lp,
                system_program: system_program::ID,
                token_program: token::ID,
                associated_token_program: associated_token::ID,
            }
            .to_account_metas(None),
        )
    }
    pub fn deposit(&mut self, a: u64, b: u64, min_lp: u64) -> TransactionResult {
        let ix = self.instruction(a, b, min_lp);
        send(&mut self.pool.base, &self.pool.payer, &[ix], &[])
    }
    pub fn balance(&self, address: Pubkey) -> u64 {
        spl_token::state::Account::unpack(&self.pool.base.svm.get_account(&address).unwrap().data)
            .unwrap()
            .amount
    }
    pub fn supply(&self) -> u64 {
        spl_token::state::Mint::unpack(
            &self
                .pool
                .base
                .svm
                .get_account(&self.addresses.lp_mint)
                .unwrap()
                .data,
        )
        .unwrap()
        .supply
    }
    pub fn snapshot(&self) -> Vec<Option<Vec<u8>>> {
        [
            self.user_a,
            self.user_b,
            self.user_lp,
            self.addresses.vault_a,
            self.addresses.vault_b,
            self.addresses.lp_mint,
        ]
        .iter()
        .map(|key| self.pool.base.svm.get_account(key).map(|a| a.data))
        .collect()
    }
}
