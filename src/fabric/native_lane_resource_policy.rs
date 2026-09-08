//! Concrete native resource policies bound to one fully joined operation.
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use crate::fabric::native_execution_lane::{NativeLaneJoined, NativeLaneOutput, NativeLaneResourcePolicy, NativeResourceFailure, output_seal};
use crate::fabric::native_resource_policy::{NativeResourceOwner, NativeResourceThreadGuard};
use crate::resource_budget::{ResourceBudget, ResourceBudgetError};
use buoyant_kernel::resource::{AllocationRequest, ResourceExhausted};
use buoyant_kernel::engine::arrow_data::NativeOwnedRecordBatch;

thread_local! {
    static NATIVE_POLICIES: RefCell<Option<NativeResourceThreadGuard>> = const { RefCell::new(None) };
}
fn failure(error: ResourceExhausted) -> NativeResourceFailure {
    NativeResourceFailure { kind: error.kind, requested: error.requested, limit: error.limit }
}
pub(crate) struct NativeOperationResourcePolicy {
    owner: Arc<NativeResourceOwner>,
    runtime_admission: Arc<crate::fabric::native_tokio_resource::NativeTokioAdmission>,
    joined_runtime: Mutex<Option<tokio::runtime::Id>>,
}
impl NativeOperationResourcePolicy {
    pub(crate) fn try_new(owner: Arc<NativeResourceOwner>) -> Result<Arc<Self>, NativeResourceFailure> {
        if owner.scope().allocations_sealed() {
            return Err(NativeResourceFailure { kind: "native resource owner already joined", requested: 1, limit: 0 });
        }
        let layout = std::alloc::Layout::new::<(usize,usize)>().extend(std::alloc::Layout::new::<Self>())
            .map_err(|_| NativeResourceFailure { kind: "native resource bridge layout", requested: usize::MAX, limit: isize::MAX as usize })?.0.pad_to_align();
        owner.scope().reserve(AllocationRequest { kind: "native resource bridge", bytes: layout.size() })
            .map_err(|_| failure(owner.scope().failure().unwrap_or(ResourceExhausted { kind: "native resource bridge", requested: layout.size(), limit: 0 })))?;
        let runtime_admission = crate::fabric::native_tokio_resource::NativeTokioAdmission::try_new(owner.scope().clone()).map_err(|error| NativeResourceFailure { kind: error.kind, requested: error.requested, limit: error.limit })?;
        Ok(Arc::new(Self { owner, runtime_admission, joined_runtime: Mutex::new(None) }))
    }
    pub(crate) fn joined_runtime(&self) -> Option<tokio::runtime::Id> {
        *self.joined_runtime.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
impl NativeLaneResourcePolicy for NativeOperationResourcePolicy {
    fn runtime_admission(&self) -> Option<Arc<dyn tokio::runtime::resource::RuntimeAllocationAdmission>> { Some(self.runtime_admission.clone()) }
    fn begin_operation(&self) -> Result<(), NativeResourceFailure> {
        self.owner.claim_operation().map_err(failure)
    }
    fn enter_thread(&self) {
        // All fallible policy construction occurred before this operation. A
        // runtime lifecycle violation must terminate the worker before its first
        // native task; it must never leave a runnable worker without policies.
        let guard = self.owner.enter_thread().expect("native worker policy lifecycle invariant");
        NATIVE_POLICIES.with(|slot| {
            let mut slot = slot.borrow_mut();
            assert!(slot.is_none(), "native worker has duplicate resource policies");
            *slot = Some(guard);
        });
    }
    fn exit_thread(&self) { NATIVE_POLICIES.with(|slot| { slot.borrow_mut().take(); }); }
    fn check_available(&self) -> Result<(), NativeResourceFailure> { self.owner.check_available().map_err(failure) }
    fn joined(&self, proof: NativeLaneJoined) -> Result<(), NativeResourceFailure> {
        *self.joined_runtime.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(proof.runtime_id());
        self.owner.mark_joined().map_err(failure)
    }
}

// Audited output: original immutable batches, native receipt policies and their
// input ancestry. This type contains no runtime, engine, future or stream.
impl output_seal::Sealed for NativeOwnedRecordBatch {}
impl NativeLaneOutput for NativeOwnedRecordBatch {
    fn validate_retained_owner(&self, budget: &ResourceBudget) -> Result<(), ResourceBudgetError> {
        let owner = self.record_batch().resource_owner().owner()
            .and_then(|owner| (owner.as_ref() as &dyn std::any::Any).downcast_ref::<NativeResourceOwner>())
            .ok_or(ResourceBudgetError::ForeignOwner)?;
        if !owner.belongs_to(budget) || !self.owners().native_scope().is_some_and(|scope| Arc::ptr_eq(scope, owner.scope())) {
            return Err(ResourceBudgetError::ForeignOwner);
        }
        Ok(())
    }
}
