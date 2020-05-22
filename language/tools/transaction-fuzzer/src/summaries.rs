// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    abstract_state::{resource, AbstractMetadata, AbstractType, Constraint, Effect},
    registered_types::{self, TypeRegistry},
    transaction::{
        self, AbstractTransaction, AbstractTransactionArgument, TransactionArgumentType,
        TransactionRegistry,
    },
    ty,
};
use libra_types::account_address::AccountAddress;
use move_core_types::{language_storage::TypeTag, transaction_argument::TransactionArgument};
use stdlib::transaction_scripts::StdlibScript;

macro_rules! ty_constraint {
    ($x:ident => $constraint:expr) => {{
        Box::new(move |$x: AbstractType| $constraint)
    }};
}

macro_rules! eff {
    ($sender:ident, $args:ident, $ty_args:ident => $eff:expr) => {
        Box::new(
            move |$sender: AccountAddress,
                  $args: Vec<TransactionArgument>,
                  $ty_args: Vec<TypeTag>| $eff,
        )
    };
}

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

pub fn add_currency() -> AbstractTransaction {
    AbstractTransaction {
        sender_preconditions: AbstractTransactionArgument {
            preconditions: vec![Constraint::HasResource(resource(ty!(0x0::LibraAccount::T)))],
            argument_type: TransactionArgumentType::Address,
        },
        ty_args: vec![(
            AbstractMetadata::IsCurrency,
            ty_constraint!(x => vec![
                Constraint::DoesNotHaveResource(resource(ty!(0x0::LibraAccount::Balance)).with_ty_param(x.type_))
            ]),
        )],
        args: vec![],
        transaction: StdlibScript::AddCurrencyToAccount,
        effects: eff! {sender, _args, ty_args => vec![
                Effect::PublishesResource(
                    sender,
                    resource(ty!(0x0::LibraAccount::Balance))
                        .with_ty_param(ty_args[0].clone())
                ),
            ]
        },
    }
}

pub fn create_account() -> AbstractTransaction {
    AbstractTransaction {
        sender_preconditions: AbstractTransactionArgument {
            //preconditions: vec![Constraint::HasResource(resource(ty!(0x0::LibraAccount::T)))],
            preconditions: vec![Constraint::AccountDNE],
            argument_type: TransactionArgumentType::Address,
        },
        ty_args: vec![(AbstractMetadata::IsCurrency, ty_constraint!(_x => vec![]))],
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
        effects: eff! {_sender, args, ty_args =>
            vec![
                Effect::PublishesResource(
                    transaction::addr(args[0].clone()),
                    resource(ty!(0x0::LibraAccount::T))
                ),
                Effect::PublishesResource(
                    transaction::addr(args[0].clone()),
                    resource(ty!(0x0::VASP::RootVASP))
                ),
                Effect::PublishesResource(
                    transaction::addr(args[0].clone()),
                    resource(ty!(0x0::LibraAccount::Balance))
                        .with_ty_param(ty_args[0].clone())
                ),
            ]
        },
    }
}

pub fn apply_for_association_address() -> AbstractTransaction {
    AbstractTransaction {
        sender_preconditions: AbstractTransactionArgument {
            preconditions: vec![
                Constraint::HasResource(resource(ty!(0x0::LibraAccount::T))),
                Constraint::DoesNotHaveResource(resource(
                    ty!(0x0::Association::AssociationPrivilege<0x0::Association::T>),
                )),
            ],
            argument_type: TransactionArgumentType::Address,
        },
        ty_args: vec![],
        args: vec![],
        transaction: StdlibScript::ApplyForAssociationAddress,
        effects: eff! {sender, _args, _ty_args => vec![
            Effect::PublishesResource(
                sender,
                resource(ty!(0x0::Association::AssociationPrivilege<0x0::Association::T>))
            )
        ]},
    }
}

pub fn txns() -> TransactionRegistry {
    let mut registry = TransactionRegistry::new();
    let txns = vec![
        ("zadd_currency".to_string(), add_currency()),
        (
            "zapply_for_association_address".to_string(),
            apply_for_association_address(),
        ),
        ("create_account".to_string(), create_account()),
    ];
    registry.add_transactions(txns);
    registry
}
