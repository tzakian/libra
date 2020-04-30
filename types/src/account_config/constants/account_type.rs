// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    account_config::constants::CORE_CODE_ADDRESS,
    language_storage::{ModuleId, StructTag, TypeTag},
};
use move_core_types::identifier::{IdentStr, Identifier};
use once_cell::sync::Lazy;

static ACCOUNT_TYPE_MODULE_NAME: Lazy<Identifier> =
    Lazy::new(|| Identifier::new("AccountType").unwrap());
pub static ACCOUNT_TYPE_MODULE: Lazy<ModuleId> =
    Lazy::new(|| ModuleId::new(CORE_CODE_ADDRESS, ACCOUNT_TYPE_MODULE_NAME.clone()));
pub static ACCOUNT_TYPE_STRUCT_NAME: Lazy<Identifier> = Lazy::new(|| Identifier::new("T").unwrap());

static VASP_TYPE_MODULE_NAME: Lazy<Identifier> = Lazy::new(|| Identifier::new("VASP").unwrap());
pub static VASP_TYPE_MODULE: Lazy<ModuleId> =
    Lazy::new(|| ModuleId::new(CORE_CODE_ADDRESS, VASP_TYPE_MODULE_NAME.clone()));
pub static ROOT_VASP_STRUCT_NAME: Lazy<Identifier> =
    Lazy::new(|| Identifier::new("RootVASP").unwrap());

static EMPTY_ACCOUNT_TYPE_MODULE_NAME: Lazy<Identifier> =
    Lazy::new(|| Identifier::new("Empty").unwrap());
pub static EMPTY_ACCOUNT_TYPE_MODULE: Lazy<ModuleId> =
    Lazy::new(|| ModuleId::new(CORE_CODE_ADDRESS, EMPTY_ACCOUNT_TYPE_MODULE_NAME.clone()));
pub static EMPTY_ACCOUNT_STRUCT_NAME: Lazy<Identifier> =
    Lazy::new(|| Identifier::new("T").unwrap());

pub fn account_type_module_name() -> &'static IdentStr {
    &*ACCOUNT_TYPE_MODULE_NAME
}

pub fn account_type_struct_name() -> &'static IdentStr {
    &*ACCOUNT_TYPE_STRUCT_NAME
}

pub fn vasp_type_module_name() -> &'static IdentStr {
    &*VASP_TYPE_MODULE_NAME
}

pub fn root_vasp_type_struct_name() -> &'static IdentStr {
    &*ROOT_VASP_STRUCT_NAME
}

pub fn empty_account_type_module_name() -> &'static IdentStr {
    &*EMPTY_ACCOUNT_TYPE_MODULE_NAME
}

pub fn empty_account_type_struct_name() -> &'static IdentStr {
    &*EMPTY_ACCOUNT_STRUCT_NAME
}

pub fn account_type_struct_tag(is_empty_account_type: bool) -> StructTag {
    let inner_struct_tag = if is_empty_account_type {
        StructTag {
            address: CORE_CODE_ADDRESS,
            module: empty_account_type_module_name().to_owned(),
            type_params: vec![],
            name: empty_account_type_struct_name().to_owned(),
        }
    } else {
        StructTag {
            address: CORE_CODE_ADDRESS,
            module: vasp_type_module_name().to_owned(),
            type_params: vec![],
            name: root_vasp_type_struct_name().to_owned(),
        }
    };
    StructTag {
        address: CORE_CODE_ADDRESS,
        module: account_type_module_name().to_owned(),
        type_params: vec![TypeTag::Struct(inner_struct_tag)],
        name: account_type_struct_name().to_owned(),
    }
}
