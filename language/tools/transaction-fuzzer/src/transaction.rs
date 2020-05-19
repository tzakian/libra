// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    abstract_state::{AbstractMetadata, Constraint, Effect},
    chain_state::AbstractChainState,
    resource,
};
use libra_types::account_address::AccountAddress;
use move_core_types::{language_storage::TypeTag, transaction_argument::TransactionArgument};
use rand::{self, Rng};
use std::collections::BTreeMap;
use std::fmt;
use stdlib::transaction_scripts::StdlibScript;

pub type InstantiableEffect = Box<fn(Vec<TransactionArgument>, Vec<TypeTag>) -> Effect>;

macro_rules! ieff {
    ($args:ident, $ty_args:ident => $eff:expr) => {
        Box::new(move |$args: Vec<TransactionArgument>, $ty_args: Vec<TypeTag>| $eff)
    };
}

#[derive(Clone, PartialEq, Eq)]
pub struct AbstractTransactionArgument {
    pub preconditions: Vec<Constraint>,
    pub argument_type: TransactionArgumentType,
}

#[derive(Clone, PartialEq, Eq)]
pub enum TransactionArgumentType {
    U8,
    U64,
    U128,
    Address,
    U8Vector,
    Bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct AbstractTransaction {
    pub sender_preconditions: AbstractTransactionArgument,
    pub ty_args: Vec<AbstractMetadata>,
    pub args: Vec<AbstractTransactionArgument>,
    pub transaction: StdlibScript,
    pub effects: Vec<InstantiableEffect>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct InstantiatedTransaction {
    pub sender: AccountAddress,
    pub ty_args: Vec<TypeTag>,
    pub args: Vec<TransactionArgument>,
    pub transaction: StdlibScript,
    pub effects: Vec<Effect>,
}

impl fmt::Debug for InstantiatedTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstantiatedTransaction")
            .field("sender", &self.sender)
            .field("ty_args", &self.ty_args)
            .field("args", &self.args)
            .field("effects", &self.effects)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TransactionRegistry {
    pub transactions: BTreeMap<String, AbstractTransaction>,
}

impl TransactionRegistry {
    pub fn new() -> Self {
        Self {
            transactions: BTreeMap::new(),
        }
    }

    pub fn add_transactions(&mut self, txns: Vec<(String, AbstractTransaction)>) {
        for (name, txn) in txns {
            self.transactions.insert(name, txn);
        }
    }
}

pub fn addr(txn_arg: TransactionArgument) -> AccountAddress {
    match txn_arg {
        TransactionArgument::Address(addr) => addr,
        _ => panic!("nope!"),
    }
}

pub fn txns() -> TransactionRegistry {
    let mut registry = TransactionRegistry::new();
    let txns = vec![(
        "create_account".to_string(),
        AbstractTransaction {
            sender_preconditions: AbstractTransactionArgument {
                //preconditions: vec![Constraint::HasResource(resource!(0x0::LibraAccount::T))],
                preconditions: vec![Constraint::AccountDNE],
                argument_type: TransactionArgumentType::Address,
            },
            ty_args: vec![AbstractMetadata::IsCurrency],
            args: vec![
                AbstractTransactionArgument {
                    preconditions: vec![Constraint::AccountDNE],
                    argument_type: TransactionArgumentType::Address,
                },
                AbstractTransactionArgument {
                    preconditions: vec![Constraint::RangeConstraint {
                        lower: 32,
                        upper: 33,
                    }],
                    argument_type: TransactionArgumentType::U8Vector,
                },
            ],
            transaction: StdlibScript::CreateAccount,
            effects: vec![
                ieff!(x, _y => Effect::PublishesResource(addr(x[0].clone()), resource!(0x0::LibraAccount::T))),
                ieff!(x, _y => Effect::PublishesResource(addr(x[0].clone()), resource!(0x0::VASP::RootVASP))),
                // TODO
                //ieff!(args, ty_args => Effect::PublishesResource(args[0], ty!(0x0::LibraAccount::Balance<ty_args[0]>)))
            ],
        },
    )];
    registry.add_transactions(txns);
    registry
}

impl TransactionArgumentType {
    pub fn inhabit(&self) -> TransactionArgument {
        use TransactionArgument as TA;
        use TransactionArgumentType as TAT;
        match self {
            TAT::U8 => TA::U8(rand::random::<u8>()),
            TAT::U64 => TA::U64(rand::random::<u64>()),
            TAT::U128 => TA::U128(rand::random::<u128>()),
            TAT::U8Vector => TA::U8Vector((0..32).map(|_| rand::random::<u8>()).collect()),
            TAT::Bool => TA::Bool(rand::random::<bool>()),
            TAT::Address => TA::Address(AccountAddress::random()),
        }
    }
}

impl AbstractTransactionArgument {
    pub fn inhabit(&self, chain_state: &AbstractChainState) -> Option<TransactionArgument> {
        if self.preconditions.is_empty() {
            Some(self.argument_type.inhabit())
        } else {
            match self.argument_type {
                TransactionArgumentType::Bool => Some(self.argument_type.inhabit()),
                TransactionArgumentType::U8 => {
                    let (lower_bound, upper_bound) = self
                        .preconditions
                        .iter()
                        .fold(None, |bounds, constraint| {
                            constraint.constrain_bounds(bounds)
                        })
                        .unwrap_or((std::u8::MIN as u128, std::u8::MAX as u128));
                    // Incompatible constraints
                    if lower_bound == upper_bound {
                        return None;
                    }
                    let val = rand::thread_rng().gen_range(lower_bound, upper_bound) as u8;
                    Some(TransactionArgument::U8(val))
                }
                TransactionArgumentType::U64 => {
                    let (lower_bound, upper_bound) = self
                        .preconditions
                        .iter()
                        .fold(None, |bounds, constraint| {
                            constraint.constrain_bounds(bounds)
                        })
                        .unwrap_or((std::u64::MIN as u128, std::u64::MAX as u128));
                    // Incompatible constraints
                    if lower_bound == upper_bound {
                        return None;
                    }
                    let val = rand::thread_rng().gen_range(lower_bound, upper_bound) as u64;
                    Some(TransactionArgument::U64(val))
                }
                TransactionArgumentType::U128 => {
                    let (lower_bound, upper_bound) = self
                        .preconditions
                        .iter()
                        .fold(None, |bounds, constraint| {
                            constraint.constrain_bounds(bounds)
                        })
                        .unwrap_or((std::u128::MIN, std::u128::MAX));
                    // Incompatible constraints
                    if lower_bound == upper_bound {
                        return None;
                    }
                    let val = rand::thread_rng().gen_range(lower_bound, upper_bound);
                    Some(TransactionArgument::U128(val))
                }
                TransactionArgumentType::U8Vector => {
                    let (lower_bound, upper_bound) = self
                        .preconditions
                        .iter()
                        .fold(None, |bounds, constraint| {
                            constraint.constrain_bounds(bounds)
                        })
                        .unwrap_or((std::u128::MIN, std::u128::MAX));
                    // Incompatible constraints
                    if lower_bound == upper_bound {
                        return None;
                    }
                    let end = rand::thread_rng().gen_range(lower_bound, upper_bound);
                    let vec: Vec<_> = (0..end+1).map(|_| rand::random::<u8>()).collect();
                    Some(TransactionArgument::U8Vector(vec))
                }
                TransactionArgumentType::Address => {
                    if self
                        .preconditions
                        .iter()
                        .any(|precond| precond == &Constraint::AccountDNE)
                    {
                        return Some(TransactionArgument::Address(AccountAddress::random()));
                    }
                    let available_accounts: Vec<_> = chain_state
                        .accounts
                        .iter()
                        .filter_map(|(address, account)| {
                            if self
                                .preconditions
                                .iter()
                                .all(|precondition| precondition.constrain_account(account))
                            {
                                Some(address)
                            } else {
                                None
                            }
                        })
                        .collect();
                    if available_accounts.is_empty() {
                        None
                    } else {
                        let address =
                            available_accounts[rand::random::<usize>() % available_accounts.len()];
                        Some(TransactionArgument::Address(*address))
                    }
                }
            }
        }
    }
}

impl AbstractTransaction {
    /// Try to instantiate the transaction w.r.t. the given chain state
    pub fn instantiate(&self, chain_state: &AbstractChainState) -> Option<InstantiatedTransaction> {
        let args: Vec<_> = self
            .args
            .iter()
            .map(|txn_arg| txn_arg.inhabit(chain_state))
            .collect::<Option<Vec<_>>>()?;

        let sender = match self.sender_preconditions.inhabit(chain_state)? {
            TransactionArgument::Address(addr) => addr,
            _ => return None,
        };

        let ty_args: Vec<TypeTag> = self
            .ty_args
            .iter()
            .map(|meta| {
                chain_state
                    .type_registry
                    .get_ty_from_meta(meta)
                    .unwrap()
                    .type_
                    .clone()
            })
            .collect();

        let effects = self
            .effects
            .iter()
            .map(|effect_fn| effect_fn(args.clone(), ty_args.clone()))
            .collect();

        Some(InstantiatedTransaction {
            sender,
            ty_args,
            args,
            transaction: self.transaction,
            effects,
        })
    }
}
