// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{assert_status_eq, executor::FakeExecutor};
use crypto::ed25519::*;
use libra_types::{
    access_path::AccessPath,
    account_config,
    test_helpers::transaction_test_helpers,
    vm_error::StatusCode,
    write_set::{WriteOp, WriteSetMut},
};

// TODO: Writesets need to go through a special path in the VM in order to avoid trying to load the
// gas schedule from chain. Once this is done, change this test back.
#[test]
fn invalid_genesis_write_set() {
    let executor = FakeExecutor::no_genesis();
    // Genesis write sets are not allowed to contain deletions.
    let write_op = (AccessPath::default(), WriteOp::Deletion);
    let write_set = WriteSetMut::new(vec![write_op]).freeze().unwrap();
    let address = account_config::association_address();
    let (private_key, public_key) = compat::generate_keypair(None);
    let signed_txn = transaction_test_helpers::get_write_set_txn(
        address,
        0,
        private_key,
        public_key,
        Some(write_set),
    )
    .into_inner();
    let verify_status = executor.verify_transaction(signed_txn.clone()).unwrap();
    let exec_block_status = executor.execute_block(vec![signed_txn]).unwrap_err();
    assert_status_eq(&verify_status, &exec_block_status);
    assert!(exec_block_status.major_status == StatusCode::VM_STARTUP_FAILURE);
}

// #[test]
// fn invalid_genesis_write_set() {
//     let executor = FakeExecutor::no_genesis();
//     // Genesis write sets are not allowed to contain deletions.
//     let write_op = (AccessPath::default(), WriteOp::Deletion);
//     let write_set = WriteSetMut::new(vec![write_op]).freeze().unwrap();
//     let address = account_config::association_address();
//     let (private_key, public_key) = compat::generate_keypair(None);
//     let signed_txn = transaction_test_helpers::get_write_set_txn(
//         address,
//         0,
//         private_key,
//         public_key,
//         Some(write_set),
//     )
//     .into_inner();
//     assert_prologue_parity!(
//         executor.verify_transaction(signed_txn.clone()),
//         executor.execute_transaction(signed_txn).status(),
//         VMStatus::new(StatusCode::INVALID_WRITE_SET)
//     );
// }
