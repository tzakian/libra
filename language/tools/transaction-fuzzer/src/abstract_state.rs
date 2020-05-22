// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use language_e2e_tests::account::Account;
use libra_types::account_address::AccountAddress;
use move_core_types::language_storage::TypeTag;
use std::{cmp::Ordering, collections::BTreeSet};

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
    pub type_: TypeTag,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractAccount {
    pub account: Account,
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
    // TODO: Replace with Account instead of AbstractAccount
    CreatesAccount(AbstractAccount),
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

pub fn resource(type_: TypeTag) -> AbstractResource {
    AbstractResource::new(type_)
}

impl AbstractResource {
    pub fn new(type_: TypeTag) -> Self {
        Self { type_ }
    }
    pub fn with_ty_param(mut self, ty_param: TypeTag) -> Self {
        match &mut self.type_ {
            TypeTag::Struct(struct_tag) => {
                struct_tag.type_params.push(ty_param);
            }
            _ => panic!("Invalid type tag for resource"),
        }
        self
    }
}

impl AbstractAccount {
    pub fn new_from_addr(addr: AccountAddress) -> Self {
        Self {
            account: Account::new_with_address(addr),
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

impl PartialOrd for AbstractAccount {
    fn partial_cmp(&self, other: &AbstractAccount) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AbstractAccount {
    fn cmp(&self, other: &AbstractAccount) -> Ordering {
        self.resources.cmp(&other.resources)
    }
}

//impl PartialOrd for Effect {
//    fn partial_cmp(&self, other: &Effect) -> Option<Ordering> {
//        Some(self.cmp(other))
//    }
//}
//
//impl Ord for Effect {
//    fn cmp(&self, other: &Effect) -> Ordering {
//        match (self, other) {
//            (Effect::CreatesAccount(a), Effect::CreatesAccount(b)) => {
//                a.address().cmp(b.address())
//            }
//            (_, Effect::CreatesAccount(_)) => Ordering::Less,
//            (Effect::CreatesAccount(_), _) => Ordering::Greater,
//        }
//    }
//}
