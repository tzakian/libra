// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::abstract_state::{AbstractMetadata, AbstractType};
use move_core_types::language_storage::TypeTag;
use std::collections::BTreeMap;

#[macro_export]
macro_rules! ty {
    ($addr:literal::$mod_name:ident::$struct_name:ident) => {{
        ty!($addr::$mod_name::$struct_name <>)
    }};
    ($addr:literal::$mod_name:ident::$struct_name:ident
     <$($addrs:literal::$mod_names:ident::$struct_names:ident),*>) => {{
        use move_core_types::{
            identifier::Identifier,
            language_storage::{StructTag, TypeTag},
        };
        use libra_types::account_address::AccountAddress;
        let type_params = vec![
            $(ty!($addrs::$mod_names::$struct_names),)*
        ];
        TypeTag::Struct(StructTag {
            address: AccountAddress::from_hex_literal(stringify!($addr)).unwrap(),
            module: Identifier::new(stringify!($mod_name)).unwrap(),
            name: Identifier::new(stringify!($struct_name)).unwrap(),
            type_params,
        })
    }}
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub struct TypeRegistry {
    pub meta_to_type: BTreeMap<AbstractMetadata, Vec<AbstractType>>,
    pub abstract_types: BTreeMap<TypeTag, AbstractType>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self {
            meta_to_type: BTreeMap::new(),
            abstract_types: BTreeMap::new(),
        }
    }

    pub fn add_ty(&mut self, ty: AbstractType) {
        for meta in ty.meta.iter() {
            let entry = self
                .meta_to_type
                .entry(meta.clone())
                .or_insert_with(Vec::new);
            entry.push(ty.clone());
        }
        self.abstract_types.insert(ty.type_.clone(), ty);
    }

    pub fn abstract_(&self, typ: &TypeTag) -> AbstractType {
        self.abstract_types.get(typ).unwrap().clone()
    }

    pub fn get_ty_from_meta(&self, meta: &AbstractMetadata) -> Option<&AbstractType> {
        self.meta_to_type.get(meta).and_then(|tys| {
            let index = rand::random::<usize>() % tys.len();
            tys.get(index)
        })
    }
}

pub fn build_type_registry(registries: Vec<(TypeTag, Vec<AbstractMetadata>)>) -> TypeRegistry {
    let mut type_registry = TypeRegistry::new();
    for (tag, metas) in registries.into_iter() {
        let mut ty = AbstractType::new(tag);
        for meta in metas {
            ty.add_meta(meta)
        }
        type_registry.add_ty(ty);
    }
    type_registry
}
