// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    abstract_state::{AbstractMetadata, AbstractType, Constraint, Effect},
    chain_state::AbstractChainState,
};
use anyhow::Result;
use language_e2e_tests::account::Account;
use libra_crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey};
use libra_types::account_address::AccountAddress;
use move_core_types::{language_storage::TypeTag, transaction_argument::TransactionArgument};
use rand::{self, Rng};
use std::{collections::HashMap, fmt};
use stdlib::transaction_scripts::StdlibScript;

pub type InstantiableEffects =
    Box<fn(AccountAddress, Vec<EffectInstantiationArg>, Vec<TypeTag>) -> Vec<Effect>>;

pub type TyConstraint = Box<fn(AbstractType) -> Vec<Constraint>>;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectInstantiationArg {
    TxnArg(TransactionArgument),
    Account(Account),
    NewKey(Ed25519PrivateKey, Ed25519PublicKey),
}

pub trait Transaction {
    fn name(&self) -> String;
    fn abstract_(&self) -> AbstractTransaction;
    fn instantiate(&self, chain_state: &mut AbstractChainState) -> Option<InstantiatedTransaction>;
}

#[derive(Clone, PartialEq, Eq)]
pub struct AbstractTransaction {
    pub sender_preconditions: AbstractTransactionArgument,
    pub ty_args: Vec<(AbstractMetadata, TyConstraint)>,
    pub args: Vec<AbstractTransactionArgument>,
    pub transaction: StdlibScript,
    pub effects: InstantiableEffects,
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

impl EffectInstantiationArg {
    pub fn project(args: Vec<TransactionArgument>) -> Vec<EffectInstantiationArg> {
        args.into_iter()
            .map(EffectInstantiationArg::TxnArg)
            .collect()
    }

    pub fn txn(&self) -> TransactionArgument {
        match self {
            EffectInstantiationArg::TxnArg(arg) => arg.clone(),
            _ => panic!("Invalid effect argument encountered"),
        }
    }
    pub fn account(&self) -> Account {
        match self {
            EffectInstantiationArg::Account(account) => account.clone(),
            _ => panic!("Invalid effect argument encountered"),
        }
    }
    pub fn keys(&self) -> (Ed25519PrivateKey, Ed25519PublicKey) {
        match self {
            EffectInstantiationArg::NewKey(priv_key, pub_key) => {
                (priv_key.clone(), pub_key.clone())
            }
            _ => panic!("Invalid effect argument encountered"),
        }
    }
}

impl InstantiatedTransaction {
    pub fn apply_transaction(&self, chain_state: &mut AbstractChainState) -> Result<()> {
        for effect in &self.effects {
            //println!("Applying effect: {:#?}", effect);
            chain_state.apply_effect(effect.clone())?;
        }
        Ok(())
    }
}

pub struct TransactionRegistry {
    pub transactions: HashMap<String, Box<dyn Transaction>>,
}

impl TransactionRegistry {
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
        }
    }

    pub fn add_transactions(&mut self, txns: Vec<(String, Box<dyn Transaction>)>) {
        for (name, txn) in txns {
            self.transactions.insert(name, txn);
        }
    }
}

impl TransactionArgumentType {
    pub fn inhabit(&self) -> TransactionArgument {
        use TransactionArgument as TA;
        use TransactionArgumentType as TAT;
        match self {
            TAT::U8 => TA::U8(rand::random::<u8>()),
            //TAT::U64 => TA::U64(rand::random::<u64>()),
            TAT::U64 => TA::U64(0),
            TAT::U128 => TA::U128(rand::random::<u128>()),
            TAT::U8Vector => TA::U8Vector((0..32).map(|_| rand::random::<u8>()).collect()),
            TAT::Bool => TA::Bool(rand::random::<bool>()),
            TAT::Address => TA::Address(AccountAddress::random()),
        }
    }
}

impl AbstractTransactionArgument {
    pub fn add_constraints(mut self, mut constraints: Vec<Constraint>) -> Self {
        self.preconditions.append(&mut constraints);
        self
    }

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
                    let vec: Vec<_> = (0..end + 1).map(|_| rand::random::<u8>()).collect();
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
    pub fn get_tys_and_constraints(
        tys: &[(AbstractMetadata, TyConstraint)],
        chain_state: &AbstractChainState,
    ) -> (Vec<TypeTag>, Vec<Constraint>) {
        let mut ty_tags = Vec::new();
        let mut constraints = Vec::new();
        for (meta, ty_constraint) in tys.iter() {
            let typ = chain_state.type_registry.get_ty_from_meta(meta).unwrap();
            let mut constraint = ty_constraint(typ.clone());
            ty_tags.push(typ.type_.clone());
            constraints.append(&mut constraint);
        }
        (ty_tags, constraints)
    }
}

pub fn addr(txn_arg: TransactionArgument) -> AccountAddress {
    match txn_arg {
        TransactionArgument::Address(addr) => addr,
        _ => panic!("Invalid transactioon argument"),
    }
}

pub fn bool(txn_arg: TransactionArgument) -> bool {
    match txn_arg {
        TransactionArgument::Bool(b) => b,
        _ => panic!("Invalid transactioon argument"),
    }
}
