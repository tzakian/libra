use transaction_fuzzer::{
    abstract_state::AbstractMetadata,
    chain_state::AbstractChainState,
    registered_types::{self, TypeRegistry},
    transaction::{txns, AbstractTransaction},
    ty,
};

pub fn create_type_registry() -> TypeRegistry {
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

fn main() {
    let type_registry = create_type_registry();
    let mut chain_state = AbstractChainState::new(type_registry);
    let txns = txns();
    for (name, txn) in txns.transactions.into_iter() {
        for _ in 0..5 {
            if let Some(t) = txn.instantiate(&chain_state) {
                println!("yup! {} {:#?}", name, t);
                for effect in t.effects.into_iter() {
                    println!("EFF: {:#?}", effect);
                    chain_state.apply_effect(effect).unwrap();
                }
            } else {
                println!("{} nope", name);
            }
        }
    }
    println!("{:#?}", chain_state);
}
