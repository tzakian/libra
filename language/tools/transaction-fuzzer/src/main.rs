use language_e2e_tests::data_store::GENESIS_CHANGE_SET;
use transaction_fuzzer::{
    chain_state::AbstractChainState,
    execution::Generator,
    transactions::{txns, type_registry},
};

fn main() {
    let type_registry = type_registry();
    let mut chain_state = AbstractChainState::new(GENESIS_CHANGE_SET.write_set(), type_registry);
    println!("{}", chain_state);
    let mut generator = Generator::new(txns(), 1000);
    let block = generator.generate_block_and_apply(&mut chain_state);
    println!("NUM: {}", block.len());
    generator.exec(block);
    //for txn in &block {
    //println!("{:#?}", txn);
    //}
    //for output in generator.exec(block) {
    //println!("OUTPUT: {:#?}", output);
    //}
    //println!("{}", chain_state);
}
