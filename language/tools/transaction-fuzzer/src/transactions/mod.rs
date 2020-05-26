// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0
use crate::{
    abstract_state::AbstractMetadata,
    registered_types::{self, TypeRegistry},
    transaction::{Transaction, TransactionRegistry},
    ty,
};

#[macro_use]
pub mod macros;
pub mod add_currency;
pub mod create_child_vasp_account;
pub mod create_parent_vasp_account;
pub mod rotate_key;

use add_currency::AddCurrency;
use create_child_vasp_account::CreateChildVASPAccount;
use create_parent_vasp_account::CreateParentVASPAccount;
use rotate_key::RotateAuthenticationKey;

pub fn type_registry() -> TypeRegistry {
    registered_types::build_type_registry(vec![
        (ty!(0x0::LBR::T), vec![AbstractMetadata::IsCurrency]),
        (ty!(0x0::Coin1::T), vec![AbstractMetadata::IsCurrency]),
        (ty!(0x0::Coin2::T), vec![AbstractMetadata::IsCurrency]),
        (
            ty!(0x0::AccountType::T<0x0::Empty::T>),
            vec![AbstractMetadata::IsAccountType],
        ),
        (
            ty!(0x0::AccountType::T<0x0::Unhosted::T>),
            vec![AbstractMetadata::IsAccountType],
        ),
        (
            ty!(0x0::AccountType::T<0x0::VASP::RootVASP>),
            vec![AbstractMetadata::IsAccountType],
        ),
        (
            ty!(0x0::AccountType::T<0x0::VASP::ChildVASP>),
            vec![AbstractMetadata::IsAccountType],
        ),
    ])
}

macro_rules! register_txn {
    ($registry:ident, $ty:expr) => {
        $registry.transactions.insert($ty.name(), Box::new($ty))
    };
}

pub fn txns() -> TransactionRegistry {
    let mut registry = TransactionRegistry::new();
    register_txn!(registry, AddCurrency);
    register_txn!(registry, CreateParentVASPAccount);
    register_txn!(registry, CreateChildVASPAccount);
    register_txn!(registry, RotateAuthenticationKey);
    registry
}
