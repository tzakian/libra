// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    chain_state::AbstractChainState,
    transaction::{InstantiatedTransaction, TransactionRegistry},
};
use language_e2e_tests::{account::Account, executor::FakeExecutor, gas_costs};
use libra_types::{
    transaction::{SignedTransaction, TransactionOutput, TransactionStatus},
    vm_error::StatusCode,
};

pub struct Generator {
    allowed_transactions: TransactionRegistry,
    block_size: u64,
    executor: FakeExecutor,
}

impl Generator {
    pub fn new(allowed_transactions: TransactionRegistry, block_size: u64) -> Self {
        Self {
            allowed_transactions,
            block_size,
            executor: FakeExecutor::from_genesis_file(),
        }
    }

    fn sign_txn(
        chain_state: &mut AbstractChainState,
        txn: InstantiatedTransaction,
    ) -> SignedTransaction {
        let account = chain_state.accounts.get(&txn.sender).unwrap();
        let signed_txn = account.account.create_signed_txn_with_args(
            txn.transaction.compiled_bytes().into_vec(),
            txn.ty_args,
            txn.args,
            account.sequence_number,
            gas_costs::TXN_RESERVED * 2,
            0,
            chain_state.get_gas_currency(&txn.sender),
        );
        let account = chain_state.accounts.get_mut(&txn.sender).unwrap();
        account.sequence_number += 1;
        signed_txn
    }

    pub fn generate_block_and_apply(
        &self,
        chain_state: &mut AbstractChainState,
    ) -> Vec<(String, SignedTransaction)> {
        (0..self.block_size)
            .filter_map(|_| {
                let mut allowed: Vec<_> = self
                    .allowed_transactions
                    .transactions
                    .iter()
                    .filter_map(|(name, txn)| {
                        txn.instantiate(chain_state)
                            .map(|txn| (name.to_string(), txn))
                    })
                    .collect();
                if allowed.is_empty() {
                    None
                } else {
                    let (name, txn) = allowed.remove(rand::random::<usize>() % allowed.len());
                    let signed_txn = Self::sign_txn(chain_state, txn.clone());
                    txn.apply_transaction(chain_state)
                        .expect("Unable to apply effect");
                    Some((name, signed_txn))
                }
            })
            .collect()
    }

    pub fn exec(&mut self, block: Vec<(String, SignedTransaction)>) -> Vec<TransactionOutput> {
        let (names, block): (Vec<_>, Vec<_>) = block.into_iter().unzip();
        let result = self
            .executor
            .execute_block(block)
            .expect("Unable to execute block");

        for (output, name) in result.iter().zip(names.iter()) {
            println!("Ran: {}", name);
            match output.status() {
                TransactionStatus::Keep(status) => {
                    self.executor.apply_write_set(output.write_set());
                    assert!(
                        status.major_status == StatusCode::EXECUTED,
                        "transaction failed with {:?}",
                        status
                    );
                }
                TransactionStatus::Discard(status) => {
                    panic!("transaction discarded with {:?}", status)
                }
                TransactionStatus::Retry => panic!("transaction status is retry"),
            }
        }

        result
    }
}
