// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0
//! Processor for a single transaction.

use crate::{
    code_cache::module_cache::{ModuleCache, VMModuleCache},
    counters::*,
    data_cache::{RemoteCache, TransactionDataCache},
    gas_meter::GasMeter,
    interpreter::Interpreter,
    loaded_data::{
        function::{FunctionRef, FunctionReference},
        loaded_module::LoadedModule,
    },
};
use bytecode_verifier::{VerifiedModule, VerifiedScript};
use libra_types::{
    account_address::AccountAddress,
    account_config,
    contract_event::ContractEvent,
    identifier::{IdentStr, Identifier},
    language_storage::ModuleId,
    transaction::{TransactionArgument, TransactionOutput, TransactionStatus},
    vm_error::{StatusCode, StatusType, VMStatus},
    write_set::WriteSet,
};
use vm::{
    errors::*,
    file_format::CompiledScript,
    gas_schedule::{GasAlgebra, GasCarrier, GasUnits},
    transaction_metadata::TransactionMetadata,
    vm_string::VMString,
};
use vm_cache_map::Arena;
use vm_runtime_types::value::Value;

// Metadata needed for resolving the account module.
lazy_static! {
    /// The ModuleId for the Account module
    pub static ref ACCOUNT_MODULE: ModuleId =
        { ModuleId::new(account_config::core_code_address(), Identifier::new("LibraAccount").unwrap()) };
    /// The ModuleId for the LibraCoin module
    pub static ref COIN_MODULE: ModuleId =
        { ModuleId::new(account_config::core_code_address(), Identifier::new("LibraCoin").unwrap()) };
    /// The ModuleId for the Event
    pub static ref EVENT_MODULE: ModuleId =
        { ModuleId::new(account_config::core_code_address(), Identifier::new("Event").unwrap()) };
    /// The ModuleId for the validator config
    pub static ref VALIDATOR_CONFIG_MODULE: ModuleId =
        { ModuleId::new(account_config::core_code_address(), Identifier::new("ValidatorConfig").unwrap()) };
    /// The ModuleId for the libra system module
    pub static ref LIBRA_SYSTEM_MODULE: ModuleId =
        { ModuleId::new(account_config::core_code_address(), Identifier::new("LibraSystem").unwrap()) };

    /// The ModuleId for the transaction fee distribution module
    pub static ref TRANSACTION_FEE_DISTRIBUTION_MODULE: ModuleId =
        { ModuleId::new(account_config::core_code_address(), Identifier::new("TransactionFeeDistribution").unwrap()) };
}

// Names for special functions.
lazy_static! {
    static ref PROLOGUE_NAME: Identifier = Identifier::new("prologue").unwrap();
    static ref EPILOGUE_NAME: Identifier = Identifier::new("epilogue").unwrap();
    static ref CREATE_ACCOUNT_NAME: Identifier = Identifier::new("make").unwrap();
    static ref ACCOUNT_STRUCT_NAME: Identifier = Identifier::new("T").unwrap();
    static ref EMIT_EVENT_NAME: Identifier = Identifier::new("write_to_event_store").unwrap();
}

/// A struct that executes one single transaction.
/// 'alloc is the lifetime for the code cache, which is the argument type P here. Hence the P should
/// live as long as alloc.
/// 'txn is the lifetime of one single transaction.
pub struct TransactionExecutor<'alloc, 'txn>
where
    'alloc: 'txn,
{
    module_cache: &'txn dyn ModuleCache<'alloc>,
    data_cache: &'txn TransactionDataCache<'txn>,
    txn_data: TransactionMetadata,
    event_data: Vec<ContractEvent>,
    gas_left: GasUnits<GasCarrier>,
}

impl<'alloc, 'txn> TransactionExecutor<'alloc, 'txn>
where
    'alloc: 'txn,
{
    /// Create a new `TransactionExecutor` to execute a single transaction. `module_cache` is the
    /// cache that stores the modules previously read from the blockchain. `data_cache` is the cache
    /// that holds read-only connection to the state store as well as the changes made by previous
    /// transactions within the same block.
    pub fn new(
        module_cache: &'txn dyn ModuleCache<'alloc>,
        data_cache: &'txn dyn RemoteCache,
        txn_data: TransactionMetadata,
    ) -> Self {
        TransactionExecutor {
            module_cache,
            data_cache: &TransactionDataCache::new(data_cache),
            txn_data,
            event_data: Vec::new(),
            gas_left: txn_data.max_gas_amount(),
        }
    }

    /// Returns the module cache for this executor.
    pub fn module_cache(&self) -> &'txn dyn ModuleCache<'alloc> {
        self.module_cache
    }

    /// Create an account on the blockchain by calling into `CREATE_ACCOUNT_NAME` function stored
    /// in the `ACCOUNT_MODULE` on chain.
    pub fn create_account(&mut self, addr: AccountAddress) -> VMResult<()> {
        let gas_meter = GasMeter::new(self.gas_left);
        Interpreter::new(
            self.module_cache,
            self.txn_data,
            &mut self.data_cache,
            &mut self.event_data,
            &mut gas_meter,
        )
        .create_account_entry(addr)?;
        self.gas_left = gas_meter.remaining_gas();
        Ok(())
    }

    /// Run the prologue of a transaction by calling into `PROLOGUE_NAME` function stored
    /// in the `ACCOUNT_MODULE` on chain.
    pub(crate) fn run_prologue(&mut self) -> VMResult<()> {
        let gas_meter = GasMeter::new(self.gas_left);
        let interpreter = Interpreter::new(
            self.module_cache,
            self.txn_data,
            &mut self.data_cache,
            &mut self.event_data,
            &mut gas_meter,
        );
        let result = record_stats! {time_hist | TXN_PROLOGUE_TIME_TAKEN | {
                interpreter.disable_metering();
                let result = interpreter.execute_function(&ACCOUNT_MODULE, &PROLOGUE_NAME, vec![]);
                interpreter.enable_metering();
                result
            }
        };
        self.gas_left = gas_meter.remaining_gas();
        result
    }

    /// Run the epilogue of a transaction by calling into `EPILOGUE_NAME` function stored
    /// in the `ACCOUNT_MODULE` on chain.
    fn run_epilogue(&mut self) -> VMResult<()> {
        let gas_meter = GasMeter::new(self.gas_left);
        let interpreter = Interpreter::new(
            self.module_cache,
            self.txn_data,
            &mut self.data_cache,
            &mut self.event_data,
            &mut gas_meter,
        );
        let result = record_stats! {time_hist | TXN_EPILOGUE_TIME_TAKEN | {
                interpreter.disable_metering();
                let result = interpreter.execute_function(&ACCOUNT_MODULE, &EPILOGUE_NAME, vec![]);
                interpreter.enable_metering();
                result
            }
        };
        self.gas_left = gas_meter.remaining_gas();
        result
    }

    /// Generate the TransactionOutput on failure. There can be two possibilities:
    /// 1. The transaction encounters some runtime error, such as out of gas, arithmetic overflow,
    /// etc. In this scenario, we are going to keep this transaction and charge proper gas to the
    /// sender. 2. The transaction encounters VM invariant violation error type which indicates some
    /// properties should have been guaranteed failed. Such transaction should be discarded for
    /// sanity but this implies a bug in the VM that we should take care of.
    pub(crate) fn failed_transaction_cleanup(&mut self, result: VMResult<()>) -> TransactionOutput {
        // Discard all the local writes, restart execution from a clean state.
        self.clear();
        match self.run_epilogue() {
            Ok(_) => self
                .make_write_set(vec![], result)
                .unwrap_or_else(error_output),
            // Running epilogue shouldn't fail here as we've already checked for enough balance in
            // the prologue
            Err(err) => error_output(err),
        }
    }

    /// Clear all the writes local to this transaction.
    fn clear(&mut self) {
        self.data_cache.clear();
        self.event_data.clear();
    }

    /// Generate the TransactionOutput for a successful transaction
    pub(crate) fn transaction_cleanup(
        &mut self,
        to_be_published_modules: Vec<(ModuleId, Vec<u8>)>,
    ) -> TransactionOutput {
        // First run the epilogue
        match self.run_epilogue() {
            // If epilogue runs successfully, try to emit the writeset.
            Ok(_) => match self.make_write_set(to_be_published_modules, Ok(())) {
                // This step could fail if the program has dangling global reference
                Ok(trans_out) => trans_out,
                // In case of failure, run the cleanup code.
                Err(err) => self.failed_transaction_cleanup(Err(err)),
            },
            // If the sender depleted its balance and can't pay for the gas, run the cleanup code.
            Err(err) => match err.status_type() {
                StatusType::InvariantViolation => error_output(err),
                _ => self.failed_transaction_cleanup(Err(err)),
            },
        }
    }

    /// Entrypoint into the interpreter. All external calls need to be routed through this
    /// function.
    pub(crate) fn interpeter_entrypoint(
        &mut self,
        func: FunctionRef<'txn>,
        args: Vec<TransactionArgument>,
    ) -> VMResult<()> {
        let gas_meter = GasMeter::new(self.gas_left);
        Interpreter::new(
            self.module_cache,
            self.txn_data,
            &mut self.data_cache,
            &mut self.event_data,
            &mut gas_meter,
        )
        .interpeter_entrypoint(func, convert_txn_args(args))?;
        self.gas_left = gas_meter.remaining_gas();
        Ok(())
    }

    /// Execute a function.
    /// `module` is an identifier for the name the module is stored in. `function_name` is the name
    /// of the function. If such function is found, the VM will execute this function with arguments
    /// `args`. The return value will be placed on the top of the value stack and abort if an error
    /// occurs.
    pub fn execute_function(
        &mut self,
        module: &ModuleId,
        function_name: &IdentStr,
        args: Vec<Value>,
    ) -> VMResult<()> {
        let gas_meter = GasMeter::new(self.gas_left);
        Interpreter::new(
            self.module_cache,
            self.txn_data,
            &mut self.data_cache,
            &mut self.event_data,
            &mut gas_meter,
        )
        .execute_function(module, function_name, args)?;
        self.gas_left = gas_meter.remaining_gas();
        Ok(())
    }

    /// Execute a function with the sender set to `sender`, restoring the original sender afterward.
    /// This should only be used in the logic for generating the genesis block.
    #[allow(non_snake_case)]
    pub fn execute_function_with_sender_FOR_GENESIS_ONLY(
        &mut self,
        address: AccountAddress,
        module: &ModuleId,
        function_name: &IdentStr,
        args: Vec<Value>,
    ) -> VMResult<()> {
        let gas_meter = GasMeter::new(self.gas_left);
        let interpreter = Interpreter::new(
            self.module_cache,
            self.txn_data,
            &mut self.data_cache,
            &mut self.event_data,
            &mut gas_meter,
        );
        let old_sender = interpreter.swap_sender(address);
        let res = interpreter.execute_function(module, function_name, args);
        interpreter.swap_sender(old_sender);
        self.gas_left = gas_meter.remaining_gas();
        res
    }

    /// Produce a write set at the end of a transaction. This will clear all the local states in
    /// the TransactionProcessor and turn them into a writeset.
    pub fn make_write_set(
        &mut self,
        to_be_published_modules: Vec<(ModuleId, Vec<u8>)>,
        result: VMResult<()>,
    ) -> VMResult<TransactionOutput> {
        // This should only be used for bookkeeping. The gas is already deducted from the sender's
        // account in the account module's epilogue.
        let gas_used: u64 = self.txn_data.max_gas_amount().sub(self.gas_left).get();
        let write_set = self.data_cache.make_write_set(to_be_published_modules)?;

        record_stats!(observe | TXN_TOTAL_GAS_USAGE | gas_used);

        Ok(TransactionOutput::new(
            write_set,
            self.event_data,
            gas_used,
            match result {
                Ok(()) => TransactionStatus::from(VMStatus::new(StatusCode::EXECUTED)),
                Err(err) => TransactionStatus::from(err),
            },
        ))
    }
}

#[inline]
fn error_output(err: VMStatus) -> TransactionOutput {
    // Since this transaction will be discarded, no writeset will be included.
    TransactionOutput::new(
        WriteSet::default(),
        vec![],
        0,
        TransactionStatus::Discard(err),
    )
}

/// Convert the transaction arguments into move values.
pub(crate) fn convert_txn_args(args: Vec<TransactionArgument>) -> Vec<Value> {
    args.into_iter()
        .map(|arg| match arg {
            TransactionArgument::U64(i) => Value::u64(i),
            TransactionArgument::Address(a) => Value::address(a),
            TransactionArgument::Bool(b) => Value::bool(b),
            TransactionArgument::ByteArray(b) => Value::byte_array(b),
            TransactionArgument::String(s) => Value::string(VMString::new(s)),
        })
        .collect()
}

/// A helper function for executing a single script. Will be deprecated once we have a better
/// testing framework for executing arbitrary script.
pub fn execute_function(
    caller_script: VerifiedScript,
    modules: Vec<VerifiedModule>,
    args: Vec<TransactionArgument>,
    data_cache: &dyn RemoteCache,
) -> VMResult<()> {
    let allocator = Arena::new();
    let module_cache = VMModuleCache::new(&allocator);
    let main_module = caller_script.into_module();
    let loaded_main = LoadedModule::new(main_module);
    let entry_func = FunctionRef::new(&loaded_main, CompiledScript::MAIN_INDEX);
    let txn_metadata = TransactionMetadata::default();
    for m in modules {
        module_cache.cache_module(m);
    }
    let gas_meter = GasMeter::new(txn_metadata.max_gas_amount());
    let mut interpreter = Interpreter::new(
        &module_cache,
        txn_metadata,
        &mut TransactionDataCache::new(data_cache),
        &mut Vec::new(),
        &mut gas_meter,
    );
    interpreter.interpeter_entrypoint(entry_func, convert_txn_args(args))
}
