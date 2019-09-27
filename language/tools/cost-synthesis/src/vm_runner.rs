// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Defines the VM context for running instruction synthesis.

use vm::{
    file_format::{
        AddressPoolIndex, ByteArrayPoolIndex, Bytecode, FieldDefinitionIndex, FunctionHandleIndex,
        StructDefinitionIndex, UserStringIndex, NO_TYPE_ACTUALS,
    },
    gas_schedule::{CostTable, GasCost},
};

pub fn bogus_gas_schedule() -> CostTable {
    use Bytecode::*;
    // The actual costs for the instructions in this table _DO NOT MATTER_. This is only used
    // for cost synthesis, and for this we don't need to worry about the actual gas for instructions.
    // The only thing we care about is having an entry in the gas schedule for each
    // instruction.
    let instrs = vec![
        (
            MoveToSender(StructDefinitionIndex::new(0), NO_TYPE_ACTUALS),
            GasCost::new(0, 0),
        ),
        (GetTxnSenderAddress, GasCost::new(0, 0)),
        (
            MoveFrom(StructDefinitionIndex::new(0), NO_TYPE_ACTUALS),
            GasCost::new(0, 0),
        ),
        (BrTrue(0), GasCost::new(0, 0)),
        (WriteRef, GasCost::new(0, 0)),
        (Mul, GasCost::new(0, 0)),
        (MoveLoc(0), GasCost::new(0, 0)),
        (And, GasCost::new(0, 0)),
        (GetTxnPublicKey, GasCost::new(0, 0)),
        (Pop, GasCost::new(0, 0)),
        (BitAnd, GasCost::new(0, 0)),
        (ReadRef, GasCost::new(0, 0)),
        (Sub, GasCost::new(0, 0)),
        (
            MutBorrowField(FieldDefinitionIndex::new(0)),
            GasCost::new(0, 0),
        ),
        (
            ImmBorrowField(FieldDefinitionIndex::new(0)),
            GasCost::new(0, 0),
        ),
        (Add, GasCost::new(0, 0)),
        (CopyLoc(0), GasCost::new(0, 0)),
        (StLoc(0), GasCost::new(0, 0)),
        (Ret, GasCost::new(0, 0)),
        (Lt, GasCost::new(0, 0)),
        (LdConst(0), GasCost::new(0, 0)),
        (Abort, GasCost::new(0, 0)),
        (MutBorrowLoc(0), GasCost::new(0, 0)),
        (ImmBorrowLoc(0), GasCost::new(0, 0)),
        (LdStr(UserStringIndex::new(0)), GasCost::new(0, 0)),
        (LdAddr(AddressPoolIndex::new(0)), GasCost::new(0, 0)),
        (Ge, GasCost::new(0, 0)),
        (Xor, GasCost::new(0, 0)),
        (Neq, GasCost::new(0, 0)),
        (Not, GasCost::new(0, 0)),
        (
            Call(FunctionHandleIndex::new(0), NO_TYPE_ACTUALS),
            GasCost::new(0, 0),
        ),
        (Le, GasCost::new(0, 0)),
        (CreateAccount, GasCost::new(0, 0)),
        (Branch(0), GasCost::new(0, 0)),
        (
            Unpack(StructDefinitionIndex::new(0), NO_TYPE_ACTUALS),
            GasCost::new(0, 0),
        ),
        (Or, GasCost::new(0, 0)),
        (LdFalse, GasCost::new(0, 0)),
        (LdTrue, GasCost::new(0, 0)),
        (GetTxnGasUnitPrice, GasCost::new(0, 0)),
        (Mod, GasCost::new(0, 0)),
        (BrFalse(0), GasCost::new(0, 0)),
        (
            Exists(StructDefinitionIndex::new(0), NO_TYPE_ACTUALS),
            GasCost::new(0, 0),
        ),
        (GetGasRemaining, GasCost::new(0, 0)),
        (BitOr, GasCost::new(0, 0)),
        (GetTxnMaxGasUnits, GasCost::new(0, 0)),
        (GetTxnSequenceNumber, GasCost::new(0, 0)),
        (FreezeRef, GasCost::new(0, 0)),
        (
            MutBorrowGlobal(StructDefinitionIndex::new(0), NO_TYPE_ACTUALS),
            GasCost::new(0, 0),
        ),
        (
            ImmBorrowGlobal(StructDefinitionIndex::new(0), NO_TYPE_ACTUALS),
            GasCost::new(0, 0),
        ),
        (Div, GasCost::new(0, 0)),
        (Eq, GasCost::new(0, 0)),
        (LdByteArray(ByteArrayPoolIndex::new(0)), GasCost::new(0, 0)),
        (Gt, GasCost::new(0, 0)),
        (
            Pack(StructDefinitionIndex::new(0), NO_TYPE_ACTUALS),
            GasCost::new(0, 0),
        ),
    ];
    CostTable::new(instrs)
}

/// Create a VM loaded with the modules defined by the module generator passed in.
///
/// Returns back handles that can be used to reference the created VM, the root_module, and the
/// module cache of all loaded modules in the VM.
#[macro_export]
macro_rules! with_loaded_vm {
    ($module_generator:expr, $root_account:expr => $vm:ident, $mod:ident, $module_cache:ident) => {
        use vm::access::ModuleAccess;

        let mut modules = ::stdlib::stdlib_modules().to_vec();
        let mut generated_modules = $module_generator.collect();
        modules.append(&mut generated_modules);
        // The last module is the root module based upon how we generate modules.
        let root_module = modules
            .last()
            .expect("[VM Setup] Unable to get root module");
        let allocator = Arena::new();
        let module_id = root_module.self_id();
        let $module_cache = VMModuleCache::new(&allocator);
        let entry_idx = FunctionDefinitionIndex::new(0);
        let mut data_cache = FakeDataStore::default();
        $module_cache.cache_module(root_module.clone());
        let $mod = $module_cache
            .get_loaded_module(&module_id)
            .expect("[Module Lookup] Runtime error while looking up module")
            .expect("[Module Cache] Unable to find module in module cache.");
        for m in modules.clone() {
            $module_cache.cache_module(m);
        }
        let entry_func = FunctionRef::new(&$mod, entry_idx);
        // Create the inhabitor to build the resources that have been published
        let mut inhabitor = RandomInhabitor::new(&$mod, &$module_cache);
        $root_account.modules = modules;
        for (access_path, blob) in $root_account.generate_resources(&mut inhabitor).into_iter() {
            data_cache.set(access_path, blob);
        }
        let gas_schedule = bogus_gas_schedule();
        let mut $vm = TransactionExecutor::new(
            &$module_cache,
            &gas_schedule,
            &data_cache,
            TransactionMetadata::default(),
        );
        $vm.turn_off_gas_metering();
        match $vm.execution_stack.push_frame(entry_func) {
            Ok(_) => {}
            Err(e) => panic!("Unexpected Runtime Error: {:?}", e),
        }
    };
}
