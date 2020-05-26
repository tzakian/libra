// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    abstract_state::{resource, AbstractMetadata, Constraint, Effect},
    chain_state::AbstractChainState,
    eff,
    transaction::{
        AbstractTransaction, AbstractTransactionArgument, EffectInstantiationArg,
        InstantiatedTransaction, Transaction, TransactionArgumentType,
    },
    ty, ty_constraint,
};

use move_core_types::transaction_argument::TransactionArgument;
use stdlib::transaction_scripts::StdlibScript;

pub struct AddCurrency;

impl Transaction for AddCurrency {
    fn name(&self) -> String {
        "add_currency".to_string()
    }
    fn abstract_(&self) -> AbstractTransaction {
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
    fn instantiate(&self, chain_state: &mut AbstractChainState) -> Option<InstantiatedTransaction> {
        let atxn = self.abstract_();
        let args: Vec<_> = atxn
            .args
            .iter()
            .map(|txn_arg| txn_arg.inhabit(chain_state))
            .collect::<Option<Vec<_>>>()?;

        let (ty_args, constraints) =
            AbstractTransaction::get_tys_and_constraints(&atxn.ty_args, chain_state);

        let sender = match atxn
            .sender_preconditions
            .clone()
            .add_constraints(constraints)
            .inhabit(chain_state)?
        {
            TransactionArgument::Address(addr) => addr,
            _ => return None,
        };

        let effect_args = EffectInstantiationArg::project(args.clone());
        let effects = (atxn.effects)(sender, effect_args, ty_args.clone());

        Some(InstantiatedTransaction {
            sender,
            ty_args,
            args,
            transaction: atxn.transaction,
            effects,
        })
    }
}
