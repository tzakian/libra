use crate::{
    code_cache::module_cache::ModuleCache, data_cache::RemoteCache,
    loaded_data::loaded_module::LoadedModule,
};
use libra_config::config::VMPublishingOption;
use libra_types::transaction::SignatureCheckedTransaction;
use vm::errors::VMResult;
use vm_cache_map::Arena;

pub mod execute;
pub mod validate;
pub mod verify;

use validate::{ValidatedTransaction, ValidationMode};

/// The starting point for processing a transaction. All the different states involved are described
/// through the types present in submodules.
pub struct ProcessTransaction<'alloc, 'txn>
where
    'alloc: 'txn,
{
    txn: SignatureCheckedTransaction,
    module_cache: &'txn dyn ModuleCache<'alloc>,
    data_cache: &'txn dyn RemoteCache,
    allocator: &'txn Arena<LoadedModule>,
}

impl<'alloc, 'txn> ProcessTransaction<'alloc, 'txn>
where
    'alloc: 'txn,
{
    /// Creates a new instance of `ProcessTransaction`.
    pub fn new(
        txn: SignatureCheckedTransaction,
        module_cache: &'txn dyn ModuleCache<'alloc>,
        data_cache: &'txn dyn RemoteCache,
        allocator: &'txn Arena<LoadedModule>,
    ) -> Self {
        Self {
            txn,
            module_cache,
            data_cache,
            allocator,
        }
    }

    /// Validates this transaction. Returns a `ValidatedTransaction` on success or `VMStatus` on
    /// failure.
    pub fn validate(
        self,
        mode: ValidationMode,
        publishing_option: &VMPublishingOption,
    ) -> VMResult<ValidatedTransaction<'txn>> {
        ValidatedTransaction::new(self, mode, publishing_option)
    }
}
