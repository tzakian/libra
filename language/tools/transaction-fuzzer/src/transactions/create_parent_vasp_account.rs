// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    abstract_state::{resource, AbstractMetadata, Constraint, Effect},
    chain_state::AbstractChainState,
    eff,
    transaction::{
        self, AbstractTransaction, AbstractTransactionArgument, EffectInstantiationArg,
        InstantiatedTransaction, Transaction, TransactionArgumentType,
    },
    ty, ty_constraint,
};
use move_core_types::transaction_argument::TransactionArgument;
use stdlib::transaction_scripts::StdlibScript;

pub struct CreateParentVASPAccount;
impl Transaction for CreateParentVASPAccount {
    fn name(&self) -> String {
        "create_parent_vasp_account".to_string()
    }
    fn abstract_(&self) -> AbstractTransaction {
        use AbstractTransactionArgument as Arg;
        use Constraint as C;
        use Effect as E;
        use TransactionArgumentType as ArgType;
        AbstractTransaction {
            sender_preconditions: Arg {
                preconditions: vec![
                    C::HasResource(resource(ty!(0x0::LibraAccount::T))),
                    C::HasResource(resource(
                        ty!(0x0::Association::PrivilegedCapability<0x0::Association::T>),
                    )),
                ],
                argument_type: ArgType::Address,
            },
            ty_args: vec![(AbstractMetadata::IsCurrency, ty_constraint!(_x => vec![]))],
            args: vec![
                Arg {
                    preconditions: vec![C::AccountDNE],
                    argument_type: ArgType::Address,
                },
                Arg {
                    preconditions: vec![C::RangeConstraint {
                        lower: 32,
                        upper: 33,
                    }],
                    argument_type: ArgType::U8Vector,
                },
                Arg {
                    preconditions: vec![C::RangeConstraint {
                        lower: 0,
                        upper: 64,
                    }],
                    argument_type: ArgType::U8Vector,
                },
                Arg {
                    preconditions: vec![C::RangeConstraint {
                        lower: 0,
                        upper: 64,
                    }],
                    argument_type: ArgType::U8Vector,
                },
                Arg {
                    preconditions: vec![C::RangeConstraint {
                        lower: 64,
                        upper: 65,
                    }],
                    argument_type: ArgType::U8Vector,
                },
                Arg {
                    preconditions: vec![],
                    argument_type: ArgType::Bool,
                },
            ],
            transaction: StdlibScript::CreateParentVaspAccount,
            effects: eff! {_sender, args, ty_args => {
                let new_account = args[0].account();
                let new_addr = *new_account.address();
                let publish_all_currencies = transaction::bool(args[5].txn());
                let mut effects = vec![
                    E::CreatesAccount(new_account),
                    E::PublishesResource(
                        new_addr,
                        resource(ty!(0x0::LibraAccount::T))
                    ),
                    E::PublishesResource(
                        new_addr,
                        resource(ty!(0x0::LibraAccount::Role<0x0::VASP::ParentVASP>))
                    ),
                    E::PublishesResource(
                        new_addr,
                        resource(ty!(0x0::Event::EventHandleGenerator))
                    ),
                ];
                if publish_all_currencies {
                    effects.push(E::PublishesResource(
                            new_addr,
                            resource(ty!(0x0::LibraAccount::Balance<0x0::Coin1::T>))
                        ));
                    effects.push(E::PublishesResource(
                            new_addr,
                            resource(ty!(0x0::LibraAccount::Balance<0x0::Coin2::T>))
                        ));
                    effects.push(E::PublishesResource(
                            new_addr,
                            resource(ty!(0x0::LibraAccount::Balance<0x0::LBR::T>))
                        ));
                } else {
                    effects.push(E::PublishesResource(
                            new_addr,
                            resource(ty!(0x0::LibraAccount::Balance))
                            .with_ty_param(ty_args[0].clone())
                        ));
                }
                effects
            }
            },
        }
    }
    fn instantiate(&self, chain_state: &mut AbstractChainState) -> Option<InstantiatedTransaction> {
        let atxn = self.abstract_();
        let mut args: Vec<_> = atxn
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

        let account = chain_state.add_account();
        let mut effect_args = EffectInstantiationArg::project(args.clone());

        args[0] = TransactionArgument::Address(*account.address());
        args[1] = TransactionArgument::U8Vector(account.auth_key_prefix());
        effect_args[0] = EffectInstantiationArg::Account(account);

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
