// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

pub mod abstract_state;
pub mod chain_state;
pub mod registered_types;
pub mod transaction;

// A transaction is represented as a:
// txn:
//   args: [ty0, ty1, ...]
//   ty_args: [ty0, ty1, ...]
//   preconditions: [Precondition]
//   effects: [Effect]

// Txn decls
// decl_txn!{ name,
//      ty_args: [...],
//      args: [...],
//      preconditions: [...],
//      effects: [...]
//  }
