// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    abstract_state::{resource, AbstractAccount, AbstractMetadata, AbstractResource, Effect},
    registered_types::TypeRegistry,
    ty,
};
use anyhow::{Error, Result};
use language_e2e_tests::account::Account;
use libra_types::{
    account_address::AccountAddress,
    account_config, on_chain_config,
    write_set::{WriteOp, WriteSet},
};
use move_core_types::language_storage::TypeTag;
use resource_viewer::{MoveValueAnnotator, NullStateView};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub struct AbstractChainState {
    pub accounts: BTreeMap<AccountAddress, AbstractAccount>,
    pub type_registry: TypeRegistry,
}

impl fmt::Display for AbstractChainState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (account_address, abstract_account) in self.accounts.iter() {
            writeln!(f, "Account: {}", account_address)?;
            for resource in abstract_account.resources.iter() {
                writeln!(f, "\t{}", resource.type_)?
            }
        }
        Ok(())
    }
}

impl AbstractChainState {
    pub fn new(genesis_write_set: &WriteSet, type_registry: TypeRegistry) -> Self {
        let mut accounts = BTreeMap::new();
        let mut assoc = AbstractAccount::new_from_account(Account::new_association());
        assoc.sequence_number = 1;
        accounts.insert(account_config::association_address(), assoc);
        accounts.insert(
            account_config::transaction_fee_address(),
            AbstractAccount::new_from_account(Account::new_genesis_account(
                account_config::transaction_fee_address(),
            )),
        );
        accounts.insert(
            on_chain_config::config_address(),
            AbstractAccount::new_from_account(Account::new_genesis_account(
                on_chain_config::config_address(),
            )),
        );
        accounts.insert(
            account_config::treasury_compliance_account_address(),
            AbstractAccount::new_from_account(Account::new_genesis_account(
                account_config::treasury_compliance_account_address(),
            )),
        );

        let mut mapping = BTreeMap::new();
        let view = NullStateView::default();
        let annotator = MoveValueAnnotator::new(&view);

        for (ap, op) in genesis_write_set.iter() {
            if !accounts.contains_key(&ap.address) {
                continue;
            }
            match op {
                WriteOp::Deletion => panic!("found WriteOp::Deletion in WriteSet"),
                WriteOp::Value(blob) => {
                    let tag = ap.path.get(0).expect("empty blob in WriteSet");
                    if *tag == 1 {
                        let struct_tag = match annotator.view_access_path(ap.clone(), blob) {
                            Ok(v) => TypeTag::Struct(v.type_),
                            Err(_) => panic!("Unable to deserialize genesis type"),
                        };
                        let entry = mapping.entry(ap.address).or_insert_with(Vec::new);
                        entry.push(struct_tag);
                    }
                }
            }
        }

        for (addr, resources) in mapping.into_iter() {
            let abstract_account = accounts.get_mut(&addr).unwrap();
            for ty in resources.into_iter() {
                abstract_account.add_resource(AbstractResource::new(ty));
            }
        }

        Self {
            accounts,
            type_registry,
        }
    }

    pub fn add_account(&mut self) -> Account {
        Account::new()
    }

    pub fn get_gas_currency(&self, account_addr: &AccountAddress) -> String {
        let account_state = self
            .accounts
            .get(account_addr)
            .expect("Unable to get account for gas currency");
        let currencies = self
            .type_registry
            .meta_to_type
            .get(&AbstractMetadata::IsCurrency)
            .unwrap();
        let gas_currencies: Vec<_> = currencies
            .into_iter()
            .filter(|ty| {
                account_state.resources.contains(
                    &resource(ty!(0x0::LibraAccount::Balance)).with_ty_param(ty.type_.clone()),
                )
            })
            .collect();
        //println!("{} GAS CURRENCIES: {:?}", account_addr, gas_currencies);
        assert!(
            !gas_currencies.is_empty(),
            "Unable to find gas currency for account"
        );
        match &gas_currencies[rand::random::<usize>() % gas_currencies.len()].type_ {
            TypeTag::Struct(struct_tag) => struct_tag.module.to_string(),
            _ => panic!("Invalid currency type encountered"),
        }
    }

    pub fn apply_effect(&mut self, effect: Effect) -> Result<()> {
        match effect {
            Effect::RotatesKey(address, (new_private_key, new_public_key)) => {
                let account = self
                    .accounts
                    .get_mut(&address)
                    .ok_or_else(|| Error::msg("Unable to find account when removing resource"))?;
                account.account.rotate_key(new_private_key, new_public_key)
            }
            Effect::RemovesResource(address, resource) => {
                let account = self
                    .accounts
                    .get_mut(&address)
                    .ok_or_else(|| Error::msg("Unable to find account when removing resource"))?;
                account.resources.remove(&resource);
            }
            Effect::CreatesAccount(account) => {
                let account_addr = *account.address();
                let abstract_account = AbstractAccount::new_from_account(account);
                self.accounts.insert(account_addr, abstract_account);
            }
            Effect::PublishesResource(address, resource) => {
                //let account = self
                //.accounts
                //.entry(address)
                //.or_insert_with(|| AbstractAccount::new_from_addr(address));
                let account = self
                    .accounts
                    .get_mut(&address)
                    .ok_or_else(|| Error::msg("Unable to find account when publishing resource"))?;
                if !account.resources.insert(resource) {
                    return Err(Error::msg("Resource already published under account"));
                }
            }
        }
        Ok(())
    }
}
