//! Adapter for the exact native Tokio profile and the original operation bank.
//! Staged until the synchronized source-authority transition selects this API.
use std::alloc::Layout;
use std::sync::Arc;
use buoyant_kernel::resource::{AllocationRequest, NativeResourceScope, ResourceExhausted};
use tokio::runtime::resource::{ResourceLayoutError, RuntimeAllocationAdmission, RuntimeAllocationReceipt, RuntimeAllocationRequest};

fn convert(error: ResourceExhausted) -> ResourceLayoutError {
    ResourceLayoutError { kind: error.kind, requested: error.requested, limit: error.limit }
}
fn arc_bytes<T>() -> Result<usize, ResourceLayoutError> {
    Layout::new::<[usize; 2]>().extend(Layout::new::<T>()).map(|(layout, _)| layout.pad_to_align().size())
        .map_err(|_| ResourceLayoutError { kind: "native Tokio receipt layout", requested: usize::MAX, limit: isize::MAX as usize })
}
fn reserve(scope: &NativeResourceScope, kind: &'static str, bytes: usize) -> Result<(), ResourceLayoutError> {
    scope.reserve(AllocationRequest { kind, bytes }).map_err(|_| convert(scope.failure().unwrap_or(ResourceExhausted { kind, requested: bytes, limit: 0 })))
}

/// Holding the actual bank preserves reservations made by caller-side native
/// JoinSet and upload admission, including entry Wakers retained after join.
/// The bank holds ordinary budget receipts, never these Tokio owner wrappers,
/// so this edge cannot form a bank -> runtime -> bank ownership cycle.
#[derive(Debug)]
pub(crate) struct NativeTokioAdmission { scope: Arc<NativeResourceScope> }
impl NativeTokioAdmission {
    pub(crate) fn try_new(scope: Arc<NativeResourceScope>) -> Result<Arc<Self>, ResourceLayoutError> {
        reserve(&scope, "native Tokio admission owner", arc_bytes::<Self>()?)?;
        Ok(Arc::new(Self { scope }))
    }
}
#[derive(Debug)]
struct BankReceipt { bytes: usize, _bank: Arc<NativeResourceScope> }
impl RuntimeAllocationReceipt for BankReceipt { fn bytes(&self) -> usize { self.bytes } }
impl RuntimeAllocationAdmission for NativeTokioAdmission {
    fn try_reserve(&self, request: RuntimeAllocationRequest) -> Result<Arc<dyn RuntimeAllocationReceipt>, ResourceLayoutError> {
        let bytes = request.bytes.checked_add(arc_bytes::<BankReceipt>()?).filter(|bytes| *bytes <= isize::MAX as usize)
            .ok_or(ResourceLayoutError { kind: request.kind, requested: usize::MAX, limit: isize::MAX as usize })?;
        reserve(&self.scope, request.kind, bytes)?;
        Ok(Arc::new(BankReceipt { bytes: request.bytes, _bank: self.scope.clone() }))
    }
    fn record_failure(&self, error: ResourceLayoutError) {
        self.scope.record_failure(ResourceExhausted { kind: error.kind, requested: error.requested, limit: error.limit });
    }
}
