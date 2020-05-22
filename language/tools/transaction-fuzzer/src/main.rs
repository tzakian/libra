use transaction_fuzzer::{
    chain_state::AbstractChainState,
    summaries::{txns, type_registry},
};

fn main() {
    let type_registry = type_registry();
    let mut chain_state = AbstractChainState::new(type_registry);
    let txns = txns();
    for (name, txn) in txns.transactions.into_iter() {
        for _ in 0..5 {
            if let Some(t) = txn.instantiate(&chain_state) {
                println!("running! {}", name);
                for effect in t.effects.into_iter() {
                    //println!("EFF: {:#?}", effect);
                    chain_state.apply_effect(effect).unwrap();
                }
            } else {
                println!("{} nope", name);
            }
        }
    }
    println!("{:#?}", chain_state);
}
