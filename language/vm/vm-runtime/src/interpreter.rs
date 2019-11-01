// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    code_cache::module_cache::ModuleCache,
    counters::*,
    data_cache::TransactionDataCache,
    gas_meter::GasMeter,
    identifier::{create_access_path, resource_storage_key},
    loaded_data::{
        function::{FunctionRef, FunctionReference},
        loaded_module::LoadedModule,
    },
};
use libra_logger::prelude::*;
use libra_types::{
    access_path::AccessPath,
    account_address::AccountAddress,
    account_config,
    byte_array::ByteArray,
    contract_event::ContractEvent,
    event::EventKey,
    identifier::{IdentStr, Identifier},
    language_storage::{ModuleId, StructTag, TypeTag},
    transaction::MAX_TRANSACTION_SIZE_IN_BYTES,
    vm_error::{StatusCode, StatusType, VMStatus},
    write_set::WriteSet,
};
#[cfg(any(test, feature = "instruction_synthesis"))]
use std::collections::HashMap;
use std::{collections::VecDeque, convert::TryFrom, marker::PhantomData};
use vm::{
    access::ModuleAccess,
    errors::*,
    file_format::{
        Bytecode, FunctionHandleIndex, LocalIndex, LocalsSignatureIndex, SignatureToken,
        StructDefinitionIndex,
    },
    gas_schedule::{AbstractMemorySize, GasAlgebra, GasCarrier, GasUnits},
    transaction_metadata::TransactionMetadata,
    IndexKind,
};
use vm_runtime_types::{
    loaded_data::struct_def::StructDef,
    native_functions::dispatch::resolve_native_function,
    value::{Locals, ReferenceValue, Struct, Value},
};

// Data to resolve basic account and transaction flow functions and structs
lazy_static! {
    /// The ModuleId for the Account module
    pub static ref ACCOUNT_MODULE: ModuleId =
        { ModuleId::new(account_config::core_code_address(), Identifier::new("LibraAccount").unwrap()) };
    /// The ModuleId for the Event
    pub static ref EVENT_MODULE: ModuleId =
        { ModuleId::new(account_config::core_code_address(), Identifier::new("Event").unwrap()) };

}

// Names for special functions and structs
lazy_static! {
    static ref CREATE_ACCOUNT_NAME: Identifier = Identifier::new("make").unwrap();
    static ref ACCOUNT_STRUCT_NAME: Identifier = Identifier::new("T").unwrap();
    static ref EMIT_EVENT_NAME: Identifier = Identifier::new("write_to_event_store").unwrap();
}

/// `Interpreter` instances can execute Move functions.
///
/// An `Interpreter` instance is a stand alone execution context for a function.
/// It mimics execution on a single thread, with an call stack and an operand stack.
/// The `Interpreter` receives a reference to a data store used by certain opcodes
/// to do operations on data on chain and a `TransactionMetadata` which is also used to resolve
/// specific opcodes.
/// A `ModuleCache` is also provided to resolve external references to code.
// REVIEW: abstract the data store better (maybe a single Trait for both data and event?)
// The ModuleCache should be a Loader with a proper API.
// Resolve where GasMeter should live.
pub struct Interpreter<'alloc, 'txn>
where
    'alloc: 'txn,
{
    /// Operand stack, where Move `Value`s are stored for stack operations.
    operand_stack: Stack,
    /// The stack of active functions.
    call_stack: CallStack<'txn>,
    /// Gas metering to track cost of execution.
    gas_meter: GasMeter,
    /// Transaction data to resolve special bytecodes (e.g. GetTxnSequenceNumber, GetTxnPublicKey,
    /// GetTxnSenderAddress, ...)
    txn_data: TransactionMetadata,
    /// List of events "fired" during the course of an execution.
    // REVIEW: should this live outside the Interpreter?
    event_data: &'txn Vec<ContractEvent>,
    /// Data store
    // REVIEW: maybe this and the event should go together as some kind of external context?
    data_view: &'txn TransactionDataCache<'txn>,
    /// Code cache, this is effectively the loader.
    module_cache: &'txn dyn ModuleCache<'alloc>,
}

fn derive_type_tag(
    module: &impl ModuleAccess,
    type_actual_tags: &[TypeTag],
    ty: &SignatureToken,
) -> VMResult<TypeTag> {
    use SignatureToken::*;

    match ty {
        Bool => Ok(TypeTag::Bool),
        Address => Ok(TypeTag::Address),
        U64 => Ok(TypeTag::U64),
        ByteArray => Ok(TypeTag::ByteArray),
        String => Err(VMStatus::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
            .with_message("Cannot derive type tags for strings: unimplemented.".to_string())),
        TypeParameter(idx) => type_actual_tags
            .get(*idx as usize)
            .ok_or_else(|| {
                VMStatus::new(StatusCode::VERIFIER_INVARIANT_VIOLATION).with_message(
                    "Cannot derive type tag: type parameter index out of bounds.".to_string(),
                )
            })
            .map(|inner| inner.clone()),
        Reference(_) | MutableReference(_) => {
            Err(VMStatus::new(StatusCode::VERIFIER_INVARIANT_VIOLATION)
                .with_message("Cannot derive type tag for references.".to_string()))
        }
        Struct(idx, struct_type_actuals) => {
            let struct_type_actuals_tags = struct_type_actuals
                .iter()
                .map(|ty| derive_type_tag(module, type_actual_tags, ty))
                .collect::<VMResult<Vec<_>>>()?;
            let struct_handle = module.struct_handle_at(*idx);
            let struct_name = module.identifier_at(struct_handle.name);
            let module_handle = module.module_handle_at(struct_handle.module);
            let module_address = module.address_at(module_handle.address);
            let module_name = module.identifier_at(module_handle.name);
            Ok(TypeTag::Struct(StructTag {
                address: *module_address,
                module: module_name.into(),
                name: struct_name.into(),
                type_params: struct_type_actuals_tags,
            }))
        }
    }
}

impl<'alloc, 'txn> Interpreter<'alloc, 'txn>
where
    'alloc: 'txn,
{
    /// Create a new instance of an `Interpreter` in the context of a transaction with a
    /// given module cache (loader) and a data store.
    // REVIEW: it's not clear the responsibilities between the Interpreter and the outside
    // context are well defined. Obviously certain opcodes require a given context, but
    // we may be doing better here...
    pub fn new(
        module_cache: &'txn dyn ModuleCache<'alloc>,
        txn_data: TransactionMetadata,
        data_view: &'txn TransactionDataCache<'txn>,
        event_data: &'txn Vec<ContractEvent>,
        gas_meter: &'txn GasMeter,
    ) -> Self {
        Interpreter {
            operand_stack: Stack::new(),
            call_stack: CallStack::new(),
            gas_meter: GasMeter::new(txn_data.max_gas_amount()),
            txn_data,
            event_data,
            data_view,
            module_cache,
        }
    }

    //
    // The functions below should be reviewed once we clean up the Loader and the
    // transaction flow. It's not clear whether they are leaking internal of the Interpreter
    // that would be better exposed via a more proper API.
    //

    /// Returns the module cache for this interpreter.
    pub fn module_cache(&self) -> &'txn dyn ModuleCache<'alloc> {
        self.module_cache
    }

    /// Disables metering of gas.
    pub fn disable_metering(&mut self) {
        self.gas_meter.disable_metering();
    }

    /// Re-enables metering of gas.
    pub(crate) fn enable_metering(&mut self) {
        self.gas_meter.enable_metering();
    }

    /// Returns the gas used by an execution in the `Interpreter`.
    pub(crate) fn gas_used(&self) -> u64 {
        self.txn_data
            .max_gas_amount
            .sub(self.gas_meter.remaining_gas())
            .mul(self.txn_data.gas_unit_price)
            .get()
    }

    // This is used by the genesis tool and must be deleted and re-worked
    pub(crate) fn swap_sender(&mut self, address: AccountAddress) -> AccountAddress {
        let old_sender = self.txn_data.sender;
        self.txn_data.sender = address;
        old_sender
    }

    /// Clear all the writes local to this execution.
    pub(crate) fn clear(&mut self) {
        self.data_view.clear();
        self.event_data.clear();
    }

    /// Return the list of events emitted during execution.
    pub(crate) fn events(&self) -> &[ContractEvent] {
        &self.event_data
    }

    /// Generate a `WriteSet` as a result of an execution.
    pub(crate) fn make_write_set(
        &mut self,
        to_be_published_modules: Vec<(ModuleId, Vec<u8>)>,
    ) -> VMResult<WriteSet> {
        self.data_view.make_write_set(to_be_published_modules)
    }

    /// Execute a function.
    /// `module` is an identifier for the name the module is stored in. `function_name` is the name
    /// of the function. If such function is found, the VM will execute this function with arguments
    /// `args`. The return value will be placed on the top of the value stack and abort if an error
    /// occurs.
    // REVIEW: this should probably disappear or at the very least only one between
    // `execute_function` and `interpeter_entrypoint` should exist. It's a bit messy at
    // the moment given tooling and testing. Once we remove Program transactions and we
    // clean up the loader we will have a better time cleaning this up.
    pub fn execute_function(
        &mut self,
        module: &ModuleId,
        function_name: &IdentStr,
        args: Vec<Value>,
    ) -> VMResult<()> {
        let loaded_module = self
            .module_cache
            .get_loaded_module(module)?
            .ok_or_else(|| VMStatus::new(StatusCode::LINKER_ERROR))?;
        let func_idx = loaded_module
            .function_defs_table
            .get(function_name)
            .ok_or_else(|| VMStatus::new(StatusCode::LINKER_ERROR))?;
        let func = FunctionRef::new(loaded_module, *func_idx);

        self.execute(func, args)
    }

    /// Entrypoint into the interpreter. All external calls need to be routed through this
    /// function.
    pub(crate) fn interpeter_entrypoint(
        &mut self,
        func: FunctionRef<'txn>,
        args: Vec<Value>,
    ) -> VMResult<()> {
        // We charge an intrinsic amount of gas based upon the size of the transaction submitted
        // (in raw bytes).
        let txn_size = self.txn_data.transaction_size;
        // The callers of this function verify the transaction before executing it. Transaction
        // verification ensures the following condition.
        assume!(txn_size.get() <= (MAX_TRANSACTION_SIZE_IN_BYTES as u64));
        // We count the intrinsic cost of the transaction here, since that needs to also cover the
        // setup of the function.
        let starting_gas = self.gas_meter.remaining_gas().get();
        self.gas_meter.charge_transaction_gas(txn_size)?;
        let ret = self.execute(func, args);
        record_stats!(observe | TXN_EXECUTION_GAS_USAGE | starting_gas);
        ret
    }

    /// Internal execution entry point.
    fn execute(&mut self, function: FunctionRef<'txn>, args: Vec<Value>) -> VMResult<()> {
        self.execute_main(function, args, 0).or_else(|err| {
            self.operand_stack.0.clear();
            self.call_stack.0.clear();
            Err(err)
        })?;
        // TODO: assert invariants: empty operand stack, ...
        Ok(())
    }

    /// Main loop for the execution of a function.
    ///
    /// This function sets up a `Frame` and calls `execute_code_unit` to execute code of the
    /// function represented by the frame. Control comes back to this function on return or
    /// on call. When that happens the frame is changes to a new one (call) or to the one
    /// at the top of the stack (return). If the call stack is empty execution is completed.
    // REVIEW: create account will be removed in favor of a native function (no opcode) and
    // we can simplify this code quite a bit.
    fn execute_main(
        &mut self,
        function: FunctionRef<'txn>,
        args: Vec<Value>,
        create_account_marker: usize,
    ) -> VMResult<()> {
        let mut locals = Locals::new(function.local_count());
        // TODO: assert consistency of args and function formals
        for (i, value) in args.into_iter().enumerate() {
            locals.store_loc(i, value)?;
        }
        let mut current_frame = Frame::new(function, vec![], locals);
        loop {
            let code = current_frame.code_definition();
            let exit_code = self
                .execute_code_unit(&mut current_frame, code)
                .or_else(|err| Err(self.maybe_core_dump(err, &current_frame)))?;
            match exit_code {
                ExitCode::Return => {
                    // TODO: assert consistency of current frame: stack height correct
                    if create_account_marker == self.call_stack.0.len() {
                        return Ok(());
                    }
                    if let Some(frame) = self.call_stack.pop() {
                        current_frame = frame;
                    } else {
                        return Err(self.unreachable("call stack cannot be empty", &current_frame));
                    }
                }
                ExitCode::Call(idx, type_actuals_idx) => {
                    let type_actuals = &current_frame
                        .module()
                        .locals_signature_at(type_actuals_idx)
                        .0;
                    let type_actual_tags = type_actuals
                        .iter()
                        .map(|ty| {
                            derive_type_tag(
                                current_frame.module(),
                                current_frame.type_actual_tags(),
                                ty,
                            )
                        })
                        .collect::<VMResult<Vec<_>>>()?;

                    let opt_frame = self
                        .make_call_frame(current_frame.module(), idx, type_actual_tags)
                        .or_else(|err| Err(self.maybe_core_dump(err, &current_frame)))?;
                    if let Some(frame) = opt_frame {
                        self.call_stack.push(current_frame).or_else(|frame| {
                            let err = VMStatus::new(StatusCode::CALL_STACK_OVERFLOW);
                            Err(self.maybe_core_dump(err, &frame))
                        })?;
                        current_frame = frame;
                    }
                }
                ExitCode::CreateAccount => {
                    // TODO: this code will be removed but at the moment it re-enters execute_main.
                    // That creates some issue with errors and core dumps reporting which are
                    // not completely and correctly sorted out to keep the logic manageable
                    // and given this is going away soon
                    self.call_stack.push(current_frame).or_else(|_| {
                        let err = VMStatus::new(StatusCode::CALL_STACK_OVERFLOW);
                        Err(err)
                    })?;
                    self.create_account_opcode()?;
                    if let Some(frame) = self.call_stack.pop() {
                        current_frame = frame;
                    } else {
                        return Err(VMStatus::new(StatusCode::UNREACHABLE).with_message(
                            "returning from CreateAccount opcode with no call stack".to_string(),
                        ));
                    }
                }
            }
        }
    }

    /// Execute a Move function until a return or a call opcode is found.
    #[allow(clippy::cognitive_complexity)]
    fn execute_code_unit(
        &mut self,
        frame: &mut Frame<'txn, FunctionRef<'txn>>,
        code: &[Bytecode],
    ) -> VMResult<ExitCode> {
        // TODO: re-enbale this once gas metering is sorted out
        //let code = frame.code_definition();
        loop {
            for instruction in &code[frame.pc as usize..] {
                // FIXME: Once we add in memory ops, we will need to pass in the current memory size
                // to this function.
                self.gas_meter.calculate_and_consume(
                    instruction,
                    InterpreterForGasCost::new(&self.operand_stack, self.module_cache, frame),
                    AbstractMemorySize::new(1),
                )?;
                frame.pc += 1;

                match instruction {
                    Bytecode::Pop => {
                        self.operand_stack.pop()?;
                    }
                    Bytecode::Ret => {
                        return Ok(ExitCode::Return);
                    }
                    Bytecode::BrTrue(offset) => {
                        if self.operand_stack.pop_as::<bool>()? {
                            frame.pc = *offset;
                            break;
                        }
                    }
                    Bytecode::BrFalse(offset) => {
                        if !self.operand_stack.pop_as::<bool>()? {
                            frame.pc = *offset;
                            break;
                        }
                    }
                    Bytecode::Branch(offset) => {
                        frame.pc = *offset;
                        break;
                    }
                    Bytecode::LdConst(int_const) => {
                        self.operand_stack.push(Value::u64(*int_const))?;
                    }
                    Bytecode::LdAddr(idx) => {
                        self.operand_stack
                            .push(Value::address(*frame.module().address_at(*idx)))?;
                    }
                    Bytecode::LdStr(idx) => {
                        self.operand_stack
                            .push(Value::string(frame.module().user_string_at(*idx).into()))?;
                    }
                    Bytecode::LdByteArray(idx) => {
                        self.operand_stack.push(Value::byte_array(
                            frame.module().byte_array_at(*idx).clone(),
                        ))?;
                    }
                    Bytecode::LdTrue => {
                        self.operand_stack.push(Value::bool(true))?;
                    }
                    Bytecode::LdFalse => {
                        self.operand_stack.push(Value::bool(false))?;
                    }
                    Bytecode::CopyLoc(idx) => {
                        self.operand_stack.push(frame.copy_loc(*idx)?)?;
                    }
                    Bytecode::MoveLoc(idx) => {
                        self.operand_stack.push(frame.move_loc(*idx)?)?;
                    }
                    Bytecode::StLoc(idx) => {
                        frame.store_loc(*idx, self.operand_stack.pop()?)?;
                    }
                    Bytecode::Call(idx, type_actuals_idx) => {
                        return Ok(ExitCode::Call(*idx, *type_actuals_idx));
                    }
                    Bytecode::MutBorrowLoc(idx) | Bytecode::ImmBorrowLoc(idx) => {
                        self.operand_stack.push(frame.borrow_loc(*idx)?)?;
                    }
                    Bytecode::ImmBorrowField(fd_idx) | Bytecode::MutBorrowField(fd_idx) => {
                        let field_offset = frame.module().get_field_offset(*fd_idx)?;
                        let reference = self.operand_stack.pop_as::<ReferenceValue>()?;
                        let field_ref = reference.borrow_field(field_offset as usize)?;
                        self.operand_stack.push(field_ref)?;
                    }
                    Bytecode::Pack(sd_idx, _) => {
                        let struct_def = frame.module().struct_def_at(*sd_idx);
                        let field_count = struct_def.declared_field_count()?;
                        let args = self.operand_stack.popn(field_count)?;
                        self.operand_stack.push(Value::struct_(Struct::new(args)))?;
                    }
                    Bytecode::Unpack(sd_idx, _) => {
                        let struct_def = frame.module().struct_def_at(*sd_idx);
                        let field_count = struct_def.declared_field_count()?;
                        let struct_ = self.operand_stack.pop_as::<Struct>()?;
                        for idx in 0..field_count {
                            self.operand_stack
                                .push(struct_.get_field_value(idx as usize)?)?;
                        }
                    }
                    Bytecode::ReadRef => {
                        let reference = self.operand_stack.pop_as::<ReferenceValue>()?;
                        self.operand_stack.push(reference.read_ref()?)?;
                    }
                    Bytecode::WriteRef => {
                        let reference = self.operand_stack.pop_as::<ReferenceValue>()?;
                        reference.write_ref(self.operand_stack.pop()?);
                    }
                    // Arithmetic Operations
                    Bytecode::Add => self.binop_int(u64::checked_add)?,
                    Bytecode::Sub => self.binop_int(u64::checked_sub)?,
                    Bytecode::Mul => self.binop_int(u64::checked_mul)?,
                    Bytecode::Mod => self.binop_int(u64::checked_rem)?,
                    Bytecode::Div => self.binop_int(u64::checked_div)?,
                    Bytecode::BitOr => self.binop_int(|l: u64, r| Some(l | r))?,
                    Bytecode::BitAnd => self.binop_int(|l: u64, r| Some(l & r))?,
                    Bytecode::Xor => self.binop_int(|l: u64, r| Some(l ^ r))?,
                    Bytecode::Or => self.binop_bool(|l, r| l || r)?,
                    Bytecode::And => self.binop_bool(|l, r| l && r)?,
                    Bytecode::Lt => self.binop_bool(|l: u64, r| l < r)?,
                    Bytecode::Gt => self.binop_bool(|l: u64, r| l > r)?,
                    Bytecode::Le => self.binop_bool(|l: u64, r| l <= r)?,
                    Bytecode::Ge => self.binop_bool(|l: u64, r| l >= r)?,
                    Bytecode::Abort => {
                        let error_code = self.operand_stack.pop_as::<u64>()?;
                        return Err(VMStatus::new(StatusCode::ABORTED).with_sub_status(error_code));
                    }
                    Bytecode::Eq => {
                        let lhs = self.operand_stack.pop()?;
                        let rhs = self.operand_stack.pop()?;
                        self.operand_stack.push(Value::bool(lhs.equals(&rhs)?))?;
                    }
                    Bytecode::Neq => {
                        let lhs = self.operand_stack.pop()?;
                        let rhs = self.operand_stack.pop()?;
                        self.operand_stack
                            .push(Value::bool(lhs.not_equals(&rhs)?))?;
                    }
                    Bytecode::GetTxnGasUnitPrice => {
                        self.operand_stack
                            .push(Value::u64(self.txn_data.gas_unit_price().get()))?;
                    }
                    Bytecode::GetTxnMaxGasUnits => {
                        self.operand_stack
                            .push(Value::u64(self.txn_data.max_gas_amount().get()))?;
                    }
                    Bytecode::GetTxnSequenceNumber => {
                        self.operand_stack
                            .push(Value::u64(self.txn_data.sequence_number()))?;
                    }
                    Bytecode::GetTxnSenderAddress => {
                        self.operand_stack
                            .push(Value::address(self.txn_data.sender()))?;
                    }
                    Bytecode::GetTxnPublicKey => {
                        let byte_array =
                            ByteArray::new(self.txn_data.public_key().to_bytes().to_vec());
                        self.operand_stack.push(Value::byte_array(byte_array))?;
                    }
                    Bytecode::MutBorrowGlobal(idx, _) | Bytecode::ImmBorrowGlobal(idx, _) => {
                        let addr = self.operand_stack.pop_as::<AccountAddress>()?;
                        let size =
                            self.global_data_op(addr, *idx, frame.module(), Self::borrow_global)?;

                        self.gas_meter.calculate_and_consume(
                            &instruction,
                            InterpreterForGasCost::new(
                                &self.operand_stack,
                                self.module_cache,
                                frame,
                            ),
                            size,
                        )?;
                    }
                    Bytecode::Exists(idx, _) => {
                        let addr = self.operand_stack.pop_as::<AccountAddress>()?;
                        let size = self.global_data_op(addr, *idx, frame.module(), Self::exists)?;
                        self.gas_meter.calculate_and_consume(
                            &instruction,
                            InterpreterForGasCost::new(
                                &self.operand_stack,
                                self.module_cache,
                                frame,
                            ),
                            size,
                        )?;
                    }
                    Bytecode::MoveFrom(idx, _) => {
                        let addr = self.operand_stack.pop_as::<AccountAddress>()?;
                        let size =
                            self.global_data_op(addr, *idx, frame.module(), Self::move_from)?;
                        self.gas_meter.calculate_and_consume(
                            &instruction,
                            InterpreterForGasCost::new(
                                &self.operand_stack,
                                self.module_cache,
                                frame,
                            ),
                            size,
                        )?;
                    }
                    Bytecode::MoveToSender(idx, _) => {
                        let addr = self.txn_data.sender();
                        let size =
                            self.global_data_op(addr, *idx, frame.module(), Self::move_to_sender)?;
                        self.gas_meter.calculate_and_consume(
                            &instruction,
                            InterpreterForGasCost::new(
                                &self.operand_stack,
                                self.module_cache,
                                frame,
                            ),
                            size,
                        )?;
                    }
                    Bytecode::CreateAccount => {
                        return Ok(ExitCode::CreateAccount);
                    }
                    Bytecode::FreezeRef => {
                        // FreezeRef should just be a null op as we don't distinguish between mut
                        // and immut ref at runtime.
                    }
                    Bytecode::Not => {
                        let value = !self.operand_stack.pop_as::<bool>()?;
                        self.operand_stack.push(Value::bool(value))?;
                    }
                    Bytecode::GetGasRemaining => {
                        let remaining_gas = self.gas_meter.remaining_gas().get();
                        self.operand_stack.push(Value::u64(remaining_gas))?;
                    }
                }
            }
            // ok we are out, it's a branch, check the pc for good luck
            // TODO: re-work the logic here. Cost synthesis and tests should have a more
            // natural way to plug in
            if frame.pc as usize >= code.len() {
                if cfg!(test) || cfg!(feature = "instruction_synthesis") {
                    // In order to test the behavior of an instruction stream, hitting end of the
                    // code should report no error so that we can check the
                    // locals.
                    return Ok(ExitCode::Return);
                } else {
                    return Err(VMStatus::new(StatusCode::PC_OVERFLOW));
                }
            }
        }
    }

    /// Returns a `Frame` if the call is to a Move function. Calls to native functions are
    /// "inlined" and this returns `None`.
    ///
    /// Native functions do not push a frame at the moment and as such errors from a native
    /// function are incorrectly attributed to the caller.
    fn make_call_frame(
        &mut self,
        module: &LoadedModule,
        idx: FunctionHandleIndex,
        type_actual_tags: Vec<TypeTag>,
    ) -> VMResult<Option<Frame<'txn, FunctionRef<'txn>>>> {
        let func = self
            .module_cache
            .resolve_function_ref(module, idx)?
            .ok_or_else(|| VMStatus::new(StatusCode::LINKER_ERROR))?;
        if func.is_native() {
            self.call_native(func, type_actual_tags)?;
            Ok(None)
        } else {
            let mut locals = Locals::new(func.local_count());
            let arg_count = func.arg_count();
            for i in 0..arg_count {
                locals.store_loc(arg_count - i - 1, self.operand_stack.pop()?)?;
            }
            Ok(Some(Frame::new(func, type_actual_tags, locals)))
        }
    }

    /// Call a native functions.
    fn call_native(
        &mut self,
        function: FunctionRef<'txn>,
        type_actual_tags: Vec<TypeTag>,
    ) -> VMResult<()> {
        let module = function.module();
        let module_id = module.self_id();
        let function_name = function.name();
        let native_function = resolve_native_function(&module_id, function_name)
            .ok_or_else(|| VMStatus::new(StatusCode::LINKER_ERROR))?;
        if module_id == *EVENT_MODULE && function_name == EMIT_EVENT_NAME.as_ident_str() {
            self.call_emit_event(type_actual_tags)
        } else {
            let mut arguments = VecDeque::new();
            let expected_args = native_function.num_args();
            // REVIEW: this is checked again in every functions, rationalize it!
            if function.arg_count() != expected_args {
                // Should not be possible due to bytecode verifier but this
                // assertion is here to make sure
                // the view the type checker had lines up with the
                // execution of the native function
                return Err(VMStatus::new(StatusCode::LINKER_ERROR));
            }
            for _ in 0..expected_args {
                arguments.push_front(self.operand_stack.pop()?);
            }
            let result = (native_function.dispatch)(arguments)?;
            self.gas_meter.consume_gas(GasUnits::new(result.cost))?;
            result.result.and_then(|values| {
                for value in values {
                    self.operand_stack.push(value)?;
                }
                Ok(())
            })
        }
    }

    /// Emit an event if the native function was `write_to_event_store`.
    fn call_emit_event(&mut self, mut type_actual_tags: Vec<TypeTag>) -> VMResult<()> {
        if type_actual_tags.len() != 1 {
            return Err(
                VMStatus::new(StatusCode::VERIFIER_INVARIANT_VIOLATION).with_message(format!(
                    "write_to_event_storage expects 1 argument got {}.",
                    type_actual_tags.len()
                )),
            );
        }
        let type_tag = type_actual_tags.pop().unwrap();

        let msg = self
            .operand_stack
            .pop()?
            .simple_serialize()
            .ok_or_else(|| VMStatus::new(StatusCode::DATA_FORMAT_ERROR))?;
        let count = self.operand_stack.pop_as::<u64>()?;
        let key = self.operand_stack.pop_as::<ByteArray>()?;
        let guid = EventKey::try_from(key.as_bytes())
            .map_err(|_| VMStatus::new(StatusCode::EVENT_KEY_MISMATCH))?;
        self.event_data
            .push(ContractEvent::new(guid, count, type_tag, msg));
        Ok(())
    }

    /// Perform a binary operation to two values at the top of the stack.
    fn binop<F, T>(&mut self, f: F) -> VMResult<()>
    where
        VMResult<T>: From<Value>,
        F: FnOnce(T, T) -> Option<Value>,
    {
        let rhs = self.operand_stack.pop_as::<T>()?;
        let lhs = self.operand_stack.pop_as::<T>()?;
        let result = f(lhs, rhs);
        if let Some(v) = result {
            self.operand_stack.push(v)?;
            Ok(())
        } else {
            Err(VMStatus::new(StatusCode::ARITHMETIC_ERROR))
        }
    }

    /// Perform a binary operation for integer values.
    fn binop_int<F, T>(&mut self, f: F) -> VMResult<()>
    where
        VMResult<T>: From<Value>,
        F: FnOnce(T, T) -> Option<u64>,
    {
        self.binop(|lhs, rhs| f(lhs, rhs).map(Value::u64))
    }

    /// Perform a binary operation for boolean values.
    fn binop_bool<F, T>(&mut self, f: F) -> VMResult<()>
    where
        VMResult<T>: From<Value>,
        F: FnOnce(T, T) -> bool,
    {
        self.binop(|lhs, rhs| Some(Value::bool(f(lhs, rhs))))
    }

    /// Entry point for all global store operations (effectively opcodes).
    ///
    /// This performs common operation on the data store and then executes the specific
    /// opcode.
    fn global_data_op<F>(
        &mut self,
        address: AccountAddress,
        idx: StructDefinitionIndex,
        module: &LoadedModule,
        op: F,
    ) -> VMResult<AbstractMemorySize<GasCarrier>>
    where
        F: FnOnce(&mut Self, AccessPath, StructDef) -> VMResult<AbstractMemorySize<GasCarrier>>,
    {
        let ap = Self::make_access_path(module, idx, address);
        if let Some(struct_def) =
            self.module_cache
                .resolve_struct_def(module, idx, &self.gas_meter)?
        {
            op(self, ap, struct_def)
        } else {
            Err(VMStatus::new(StatusCode::LINKER_ERROR))
        }
    }

    /// BorrowGlobal (mutable and not) opcode.
    fn borrow_global(
        &mut self,
        ap: AccessPath,
        struct_def: StructDef,
    ) -> VMResult<AbstractMemorySize<GasCarrier>> {
        let global_ref = self.data_view.borrow_global(&ap, struct_def)?;
        let size = global_ref.size();
        self.operand_stack.push(Value::global_ref(global_ref))?;
        Ok(size)
    }

    /// Exists opcode.
    fn exists(
        &mut self,
        ap: AccessPath,
        struct_def: StructDef,
    ) -> VMResult<AbstractMemorySize<GasCarrier>> {
        let (exists, mem_size) = self.data_view.resource_exists(&ap, struct_def)?;
        self.operand_stack.push(Value::bool(exists))?;
        Ok(mem_size)
    }

    /// MoveFrom opcode.
    fn move_from(
        &mut self,
        ap: AccessPath,
        struct_def: StructDef,
    ) -> VMResult<AbstractMemorySize<GasCarrier>> {
        let resource = self.data_view.move_resource_from(&ap, struct_def)?;
        let size = resource.size();
        self.operand_stack.push(resource)?;
        Ok(size)
    }

    /// MoveToSender opcode.
    fn move_to_sender(
        &mut self,
        ap: AccessPath,
        struct_def: StructDef,
    ) -> VMResult<AbstractMemorySize<GasCarrier>> {
        let resource = self.operand_stack.pop_as::<Struct>()?;
        let size = resource.size();
        self.data_view.move_resource_to(&ap, struct_def, resource)?;
        Ok(size)
    }

    /// Helper to create a resource storage key (`AccessPath`) for global storage operations.
    fn make_access_path(
        module: &impl ModuleAccess,
        idx: StructDefinitionIndex,
        address: AccountAddress,
    ) -> AccessPath {
        let struct_tag = resource_storage_key(module, idx);
        create_access_path(&address, struct_tag)
    }

    //
    // Create account opcode. This will all go away once we make create account a native function.
    //

    fn create_account_opcode(&mut self) -> VMResult<()> {
        let account_module = self
            .module_cache
            .get_loaded_module(&ACCOUNT_MODULE)?
            .ok_or_else(|| VMStatus::new(StatusCode::LINKER_ERROR))?;
        let create_account_name: &IdentStr = &CREATE_ACCOUNT_NAME;
        let create_account_idx = account_module
            .function_defs_table
            .get(create_account_name)
            .ok_or_else(|| VMStatus::new(StatusCode::LINKER_ERROR))?;
        let create_account_fn = FunctionRef::new(account_module, *create_account_idx);
        let addr = self.operand_stack.pop_as::<AccountAddress>()?;
        self.gas_meter.disable_metering();
        self.execute_main(
            create_account_fn,
            vec![Value::byte_array(ByteArray::new(addr.to_vec()))],
            self.call_stack.0.len(),
        )?;
        self.gas_meter.enable_metering();
        self.save_account(account_module, addr)
    }

    fn save_account(
        &mut self,
        account_module: &LoadedModule,
        addr: AccountAddress,
    ) -> VMResult<()> {
        let account_struct_id = account_module
            .struct_defs_table
            .get(&*ACCOUNT_STRUCT_NAME)
            .ok_or_else(|| VMStatus::new(StatusCode::LINKER_ERROR))?;
        let account_struct_def = self
            .module_cache
            .resolve_struct_def(account_module, *account_struct_id, &self.gas_meter)?
            .ok_or_else(|| VMStatus::new(StatusCode::LINKER_ERROR))?;

        let account_resource = self.operand_stack.pop_as::<Struct>()?;
        // TODO: Adding the freshly created account's expiration date to the TransactionOutput here.
        let account_path = Self::make_access_path(account_module, *account_struct_id, addr);
        self.data_view
            .move_resource_to(&account_path, account_struct_def, account_resource)
    }

    /// Create an account on the blockchain by calling into `CREATE_ACCOUNT_NAME` function stored
    /// in the `ACCOUNT_MODULE` on chain.
    // REVIEW: this should not live here
    pub fn create_account_entry(&mut self, addr: AccountAddress) -> VMResult<()> {
        let account_module = self
            .module_cache
            .get_loaded_module(&ACCOUNT_MODULE)?
            .ok_or_else(|| VMStatus::new(StatusCode::LINKER_ERROR))?;

        // TODO: Currently the event counter will cause the gas cost for create account be flexible.
        //       We either need to fix the gas stability test cases in tests or we need to come up
        //       with some better ideas for the event counter creation.
        self.gas_meter.disable_metering();
        // Address will be used as the initial authentication key.
        self.execute_function(
            &ACCOUNT_MODULE,
            &CREATE_ACCOUNT_NAME,
            vec![Value::byte_array(ByteArray::new(addr.to_vec()))],
        )?;
        self.gas_meter.enable_metering();

        self.save_account(account_module, addr)
    }

    //
    // Debugging and logging helpers.
    //

    /// Given an `VMStatus` generate a core dump if the error is an `InvariantViolation`.
    fn maybe_core_dump(
        &self,
        err: VMStatus,
        current_frame: &Frame<'txn, FunctionRef<'txn>>,
    ) -> VMStatus {
        if err.is(StatusType::InvariantViolation) {
            crit!(
                "Error: {:?}\nCORE DUMP: >>>>>>>>>>>>\n{}\n<<<<<<<<<<<<\n",
                err,
                self.get_internal_state(current_frame),
            );
        }
        err
    }

    /// Generate a string which is the status of the interpreter: call stack, current bytecode
    /// stream, locals and operand stack.
    ///
    /// It is used when generating a core dump but can be used for debugging of the interpreter.
    /// It will be exposed via a debug module to give developers a way to print the internals
    /// of an execution.
    fn get_internal_state(&self, current_frame: &Frame<'txn, FunctionRef<'txn>>) -> String {
        let mut internal_state = "Call stack:\n".to_string();
        for (i, frame) in self.call_stack.0.iter().enumerate() {
            internal_state.push_str(
                format!(
                    " frame #{}: {} [pc = {}]\n",
                    i,
                    frame.function.pretty_string(),
                    frame.pc,
                )
                .as_str(),
            );
        }
        internal_state.push_str(
            format!(
                "*frame #{}: {} [pc = {}]:\n",
                self.call_stack.0.len(),
                current_frame.function.pretty_string(),
                current_frame.pc,
            )
            .as_str(),
        );
        let code = current_frame.code_definition();
        let pc = current_frame.pc as usize;
        if pc < code.len() {
            let mut i = 0;
            for bytecode in &code[0..pc] {
                internal_state.push_str(format!("{}> {:?}\n", i, bytecode).as_str());
                i += 1;
            }
            internal_state.push_str(format!("{}* {:?}\n", i, code[pc]).as_str());
        }
        internal_state
            .push_str(format!("Locals:\n{}", current_frame.locals.pretty_string()).as_str());
        internal_state.push_str("Operand Stack:\n");
        for value in &self.operand_stack.0 {
            internal_state.push_str(format!("{}\n", value.pretty_string()).as_str());
        }
        internal_state
    }

    /// Generate a core dump and an `UNREACHABLE` invariant violation.
    fn unreachable(&self, msg: &str, current_frame: &Frame<'txn, FunctionRef<'txn>>) -> VMStatus {
        let err = VMStatus::new(StatusCode::UNREACHABLE).with_message(msg.to_string());
        self.maybe_core_dump(err, current_frame)
    }
}

// TODO Determine stack size limits based on gas limit
const OPERAND_STACK_SIZE_LIMIT: usize = 1024;
const CALL_STACK_SIZE_LIMIT: usize = 1024;

/// The operand stack.
struct Stack(Vec<Value>);

impl Stack {
    /// Create a new empty operand stack.
    fn new() -> Self {
        Stack(vec![])
    }

    /// Push a `Value` on the stack if the max stack size has not been reached. Abort execution
    /// otherwise.
    fn push(&mut self, value: Value) -> VMResult<()> {
        if self.0.len() < OPERAND_STACK_SIZE_LIMIT {
            self.0.push(value);
            Ok(())
        } else {
            Err(VMStatus::new(StatusCode::EXECUTION_STACK_OVERFLOW))
        }
    }

    /// Pop a `Value` off the stack or abort execution if the stack is empty.
    fn pop(&mut self) -> VMResult<Value> {
        self.0
            .pop()
            .ok_or_else(|| VMStatus::new(StatusCode::EMPTY_VALUE_STACK))
    }

    /// Pop a `Value` of a given type off the stack. Abort if the value is not of the given
    /// type or if the stack is empty.
    fn pop_as<T>(&mut self) -> VMResult<T>
    where
        VMResult<T>: From<Value>,
    {
        self.pop()?.value_as()
    }

    /// Pop n values off the stack.
    fn popn(&mut self, n: u16) -> VMResult<Vec<Value>> {
        let remaining_stack_size = self
            .0
            .len()
            .checked_sub(n as usize)
            .ok_or_else(|| VMStatus::new(StatusCode::EMPTY_VALUE_STACK))?;
        let args = self.0.split_off(remaining_stack_size);
        Ok(args)
    }
}

/// A call stack.
#[derive(Debug)]
struct CallStack<'txn>(Vec<Frame<'txn, FunctionRef<'txn>>>);

impl<'txn> CallStack<'txn> {
    /// Create a new empty call stack.
    fn new() -> Self {
        CallStack(vec![])
    }

    /// Push a `Frame` on the call stack.
    fn push(
        &mut self,
        frame: Frame<'txn, FunctionRef<'txn>>,
    ) -> ::std::result::Result<(), Frame<'txn, FunctionRef<'txn>>> {
        if self.0.len() < CALL_STACK_SIZE_LIMIT {
            self.0.push(frame);
            Ok(())
        } else {
            Err(frame)
        }
    }

    /// Pop a `Frame` off the call stack.
    fn pop(&mut self) -> Option<Frame<'txn, FunctionRef<'txn>>> {
        self.0.pop()
    }
}

/// A `Frame` is the execution context for a function. It holds the locals of the function and
/// the function itself.
#[derive(Debug)]
struct Frame<'txn, F: 'txn> {
    pc: u16,
    locals: Locals,
    function: F,
    type_actual_tags: Vec<TypeTag>,
    phantom: PhantomData<&'txn F>,
}

/// An `ExitCode` from `execute_code_unit`.
#[derive(Debug)]
enum ExitCode {
    /// A `Return` opcode was found.
    Return,
    /// A `Call` opcode was found.
    Call(FunctionHandleIndex, LocalsSignatureIndex),
    /// A `CreateAccount` opcode was found.
    CreateAccount,
}

impl<'txn, F> Frame<'txn, F>
where
    F: FunctionReference<'txn>,
{
    /// Create a new `Frame` given a `FunctionReference` and the function `Locals`.
    ///
    /// The locals must be loaded before calling this.
    fn new(function: F, type_actual_tags: Vec<TypeTag>, locals: Locals) -> Self {
        Frame {
            pc: 0,
            locals,
            function,
            type_actual_tags,
            phantom: PhantomData,
        }
    }

    /// Return the code stream of this function.
    fn code_definition(&self) -> &'txn [Bytecode] {
        self.function.code_definition()
    }

    /// Return the `LoadedModule` this function lives in.
    fn module(&self) -> &'txn LoadedModule {
        self.function.module()
    }

    /// Copy a local from this frame at the given index. Return an error if the index is
    /// out of bounds or the local is `Invalid`.
    fn copy_loc(&self, idx: LocalIndex) -> VMResult<Value> {
        self.locals.copy_loc(idx as usize)
    }

    /// Move a local from this frame at the given index. Return an error if the index is
    /// out of bounds or the local is `Invalid`.
    fn move_loc(&mut self, idx: LocalIndex) -> VMResult<Value> {
        self.locals.move_loc(idx as usize)
    }

    /// Store a `Value` into a local at the given index. Return an error if the index is
    /// out of bounds.
    fn store_loc(&mut self, idx: LocalIndex, value: Value) -> VMResult<()> {
        self.locals.store_loc(idx as usize, value)
    }

    /// Borrow a local from this frame at the given index. Return an error if the index is
    /// out of bounds or the local is `Invalid`.
    fn borrow_loc(&mut self, idx: LocalIndex) -> VMResult<Value> {
        self.locals.borrow_loc(idx as usize)
    }

    fn type_actual_tags(&self) -> &[TypeTag] {
        &self.type_actual_tags
    }
}

//
// Below are all the functions needed for gas synthesis and gas cost.
// The story is going to change given those functions expose internals of the Interpreter that
// should never leak out.
// For now they are grouped in a couple of temporary struct and impl that can be used
// to determine what the needs of gas logic has to be.
//

pub struct InterpreterForGasCost<'a, 'alloc, 'txn>
where
    'alloc: 'txn,
{
    operand_stack: &'a Stack,
    module_cache: &'a dyn ModuleCache<'alloc>,
    frame: &'a Frame<'txn, FunctionRef<'txn>>,
}

impl<'a, 'alloc, 'txn> InterpreterForGasCost<'a, 'alloc, 'txn>
where
    'alloc: 'txn,
{
    fn new(
        operand_stack: &'a Stack,
        module_cache: &'a dyn ModuleCache<'alloc>,
        frame: &'a Frame<'txn, FunctionRef<'txn>>,
    ) -> Self {
        InterpreterForGasCost {
            operand_stack,
            module_cache,
            frame,
        }
    }

    pub fn peek(&self) -> VMResult<&Value> {
        Ok(self
            .operand_stack
            .0
            .last()
            .ok_or_else(|| VMStatus::new(StatusCode::EMPTY_VALUE_STACK))?)
    }

    pub fn peek_at(&self, index: usize) -> VMResult<&Value> {
        let size = self.operand_stack.0.len();
        if let Some(valid_index) = size
            .checked_sub(index)
            .and_then(|index| index.checked_sub(1))
        {
            Ok(self
                .operand_stack
                .0
                .get(valid_index)
                .ok_or_else(|| VMStatus::new(StatusCode::EMPTY_VALUE_STACK))?)
        } else {
            let msg = format!(
                "Index {} out of bounds for {} while indexing {}",
                index,
                size,
                IndexKind::LocalPool,
            );
            Err(VMStatus::new(StatusCode::INDEX_OUT_OF_BOUNDS).with_message(msg))
        }
    }

    pub fn module_cache(&self) -> &'a dyn ModuleCache<'alloc> {
        self.module_cache
    }

    pub fn module(&self) -> &'txn LoadedModule {
        self.frame.module()
    }

    pub fn copy_loc(&self, idx: LocalIndex) -> VMResult<Value> {
        self.frame.copy_loc(idx)
    }
}

#[cfg(any(test, feature = "instruction_synthesis"))]
pub struct InterpreterForCostSynthesis<'alloc, 'txn, P>(Interpreter<'alloc, 'txn, P>)
where
    'alloc: 'txn,
    P: ModuleCache<'alloc>;

#[cfg(any(test, feature = "instruction_synthesis"))]
impl<'alloc, 'txn, P> InterpreterForCostSynthesis<'alloc, 'txn, P>
where
    'alloc: 'txn,
    P: ModuleCache<'alloc>,
{
    pub fn new(
        module_cache: P,
        txn_data: TransactionMetadata,
        data_view: TransactionDataCache<'txn>,
    ) -> Self {
        let interpreter = Interpreter::new(module_cache, txn_data, data_view);
        InterpreterForCostSynthesis(interpreter)
    }

    pub fn turn_off_gas_metering(&mut self) {
        self.0.gas_meter.disable_metering();
    }

    pub fn clear_writes(&mut self) {
        self.0.clear();
    }

    pub fn set_stack(&mut self, stack: Vec<Value>) {
        self.0.operand_stack.0 = stack;
    }

    pub fn call_stack_height(&self) -> usize {
        self.0.call_stack.0.len()
    }

    pub fn pop_call(&mut self) {
        self.0
            .call_stack
            .pop()
            .expect("call stack must not be empty");
    }

    pub fn push_frame(&mut self, func: FunctionRef<'txn>, type_actual_tags: Vec<TypeTag>) {
        let count = func.local_count();
        self.0
            .call_stack
            .push(Frame::new(func, type_actual_tags, Locals::new(count)))
            .expect("Call stack limit reached");
    }

    pub fn load_call(&mut self, args: HashMap<LocalIndex, Value>) {
        let mut current_frame = self.0.call_stack.pop().expect("frame must exist");
        for (local_index, local) in args.into_iter() {
            current_frame
                .store_loc(local_index, local)
                .expect("local must exist");
        }
        self.0
            .call_stack
            .push(current_frame)
            .expect("Call stack limit reached");
    }

    pub fn execute_code_snippet(&mut self, code: &[Bytecode]) -> VMResult<()> {
        let mut current_frame = self.0.call_stack.pop().expect("frame must exist");
        self.0.execute_code_unit(&mut current_frame, code)?;
        self.0
            .call_stack
            .push(current_frame)
            .expect("Call stack limit reached");
        Ok(())
    }
}
