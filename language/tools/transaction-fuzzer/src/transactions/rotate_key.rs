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
use language_e2e_tests::keygen::KeyGen;
use libra_types::transaction::authenticator::AuthenticationKey;
use move_core_types::transaction_argument::TransactionArgument;
use stdlib::transaction_scripts::StdlibScript;

pub struct RotateAuthenticationKey;

impl Transaction for RotateAuthenticationKey {
    fn name(&self) -> String {
        "rotate_authentication_key".to_string()
    }
    fn abstract_(&self) -> AbstractTransaction {
        use AbstractTransactionArgument as Arg;
        use Constraint as C;
        use Effect as E;
        use TransactionArgumentType as ArgType;
        AbstractTransaction {
            sender_preconditions: Arg {
                preconditions: vec![C::HasResource(resource(ty!(0x0::LibraAccount::T)))],
                argument_type: ArgType::Address,
            },
            ty_args: vec![],
            args: vec![Arg {
                preconditions: vec![C::RangeConstraint {
                    lower: 32,
                    upper: 33,
                }],
                argument_type: ArgType::U8Vector,
            }],
            transaction: StdlibScript::RotateAuthenticationKey,
            effects: eff! {sender, args, _ty_args => vec![
                    E::RotatesKey(
                        sender,
                        args[0].keys(),
                    ),
                ]
            },
        }
    }
    fn instantiate(&self, chain_state: &mut AbstractChainState) -> Option<InstantiatedTransaction> {
        let atxn = self.abstract_();

        let ty_args = vec![];
        let sender = match atxn.sender_preconditions.clone().inhabit(chain_state)? {
            TransactionArgument::Address(addr) => addr,
            _ => return None,
        };

        let (priv_key, pub_key) = KeyGen::from_os_rng().generate_keypair();
        let args = vec![TransactionArgument::U8Vector(
            AuthenticationKey::ed25519(&pub_key).to_vec(),
        )];

        let effect_args = vec![EffectInstantiationArg::NewKey(priv_key, pub_key)];
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
