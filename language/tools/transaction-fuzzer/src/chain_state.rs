// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    abstract_state::{AbstractAccount, Effect},
    registered_types::TypeRegistry,
};
use anyhow::{Error, Result};
use libra_types::account_address::AccountAddress;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub struct AbstractChainState {
    pub accounts: BTreeMap<AccountAddress, AbstractAccount>,
    pub type_registry: TypeRegistry,
}

impl AbstractChainState {
    pub fn new(type_registry: TypeRegistry) -> Self {
        Self {
            accounts: BTreeMap::new(),
            type_registry,
        }
    }

    pub fn apply_effect(&mut self, effect: Effect) -> Result<()> {
        match effect {
            Effect::RemovesResource(address, resource) => {
                let account = self
                    .accounts
                    .get_mut(&address)
                    .ok_or_else(|| Error::msg("Unable to find account when removing resource"))?;
                account.resources.remove(&resource);
            }
            Effect::PublishesResource(address, resource) => {
                let account = self
                    .accounts
                    .entry(address)
                    .or_insert_with(|| AbstractAccount::new_from_addr(address));

                if !account.resources.insert(resource) {
                    return Err(Error::msg("Resource already published under account"));
                }
            }
        }
        Ok(())
    }
}
