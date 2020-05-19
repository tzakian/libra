// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use libra_types::account_address::AccountAddress;
use move_core_types::language_storage::TypeTag;
use std::collections::BTreeSet;

// A type can sometimes represent something else, such as a privilege, or be treated as a currency.
#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub enum AbstractMetadata {
    IsCurrency,
    IsPrivilege,
    IsAccountType,
}

// An `AbstractType` is a TypeTag, along with any metadata that might pertain
#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub struct AbstractType {
    pub type_: TypeTag,
    pub meta: BTreeSet<AbstractMetadata>,
}

// An AbstractResource is a
#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub struct AbstractResource {
    pub abstract_type: AbstractType,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub struct AbstractAccount {
    pub addr: AccountAddress,
    pub resources: BTreeSet<AbstractResource>,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub enum Constraint {
    HasResource(AbstractResource),
    DoesNotHaveResource(AbstractResource),
    RangeConstraint { lower: u128, upper: u128 },
    AccountDNE,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub enum Effect {
    PublishesResource(AccountAddress, AbstractResource),
    RemovesResource(AccountAddress, AbstractResource),
}

impl AbstractType {
    pub fn new(type_: TypeTag) -> Self {
        Self {
            type_,
            meta: BTreeSet::new(),
        }
    }

    pub fn add_meta(&mut self, meta: AbstractMetadata) {
        self.meta.insert(meta);
    }

    pub fn has_meta(&self, meta: &AbstractMetadata) -> bool {
        self.meta.contains(meta)
    }
}

impl AbstractResource {
    pub fn new(abstract_type: AbstractType) -> Self {
        Self { abstract_type }
    }
}

impl AbstractAccount {
    pub fn new_from_addr(addr: AccountAddress) -> Self {
        Self {
            addr,
            resources: BTreeSet::new(),
        }
    }
}

impl Constraint {
    pub fn constrain_account(&self, account: &AbstractAccount) -> bool {
        match self {
            Constraint::HasResource(resource) => account.resources.contains(resource),
            Constraint::DoesNotHaveResource(resource) => !account.resources.contains(resource),
            Constraint::AccountDNE => panic!("Contradictory constraint found"),
            Constraint::RangeConstraint { .. } => panic!("Invalid constraint found for address"),
        }
    }

    pub fn constrain_bounds(&self, bounds: Option<(u128, u128)>) -> Option<(u128, u128)> {
        match self {
            Constraint::RangeConstraint { lower, upper } => match bounds {
                None => Some((*lower, *upper)),
                Some((other_lower, other_upper)) => Some((
                    std::cmp::max(*lower, other_lower),
                    std::cmp::min(*upper, other_upper),
                )),
            },
            Constraint::HasResource(_)
            | Constraint::DoesNotHaveResource(_)
            | Constraint::AccountDNE => panic!("Invalid range constraint encountered"),
        }
    }
}
