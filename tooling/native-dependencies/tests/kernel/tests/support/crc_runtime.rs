//! A phase-injectable fixture owner. All native families charge the same original
//! kernel scope and common ResourceBudget; only the admission fault hook differs
//! from NativeResourceOwner. This is not a production profile certification.
use arrow_schema::resource::{
    ResourceAllocationRequest, ResourceOwnerError, RetainedResourceOwner,
};
use buoyant_kernel::path::url_resource::{
    LocalUrlJoinAdmission, NativeUrlThreadGuard, NativeUrlThreadPolicy,
};
use buoyant_kernel::resource::*;
use std::sync::Arc;
#[path = "../../../support/native_runtime.rs"]
mod native_runtime;
#[derive(Debug)]
struct Bank(Arc<NativeResourceScope>);
#[derive(Debug)]
struct Receipt {
    bytes: usize,
    _scope: Arc<NativeResourceScope>,
}
impl arrow_json::resource::ResourceReceipt for Receipt {
    fn bytes(&self) -> usize {
        self.bytes
    }
}
impl parquet::resource::ResourceReceipt for Receipt {
    fn bytes(&self) -> usize {
        self.bytes
    }
}
impl Bank {
    fn reserve(&self, bytes: usize, kind: &'static str) -> Result<Arc<Receipt>, ResourceExhausted> {
        self.0
            .reserve(AllocationRequest {
                kind,
                bytes: bytes
                    .checked_add(std::mem::size_of::<Receipt>() + 2 * std::mem::size_of::<usize>())
                    .unwrap(),
            })
            .map_err(|error| match error {
                buoyant_kernel::Error::ResourceExhausted(error) => error,
                _ => unreachable!(),
            })?;
        Ok(Arc::new(Receipt {
            bytes,
            _scope: self.0.clone(),
        }))
    }
}
impl arrow_json::resource::ResourceAdmission for Bank {
    fn try_reserve(
        &self,
        request: arrow_json::resource::ResourceRequest,
    ) -> Result<
        Arc<dyn arrow_json::resource::ResourceReceipt>,
        arrow_json::resource::ResourceExhausted,
    > {
        self.reserve(request.bytes, request.kind)
            .map(|value| value as _)
            .map_err(|error| arrow_json::resource::ResourceExhausted {
                kind: error.kind,
                requested: error.requested,
                limit: error.limit,
            })
    }
}
impl parquet::resource::ResourceAdmission for Bank {
    fn try_reserve(
        &self,
        request: parquet::resource::ResourceRequest,
    ) -> Result<Arc<dyn parquet::resource::ResourceReceipt>, parquet::resource::ResourceExhausted>
    {
        self.reserve(request.bytes, request.kind)
            .map(|value| value as _)
            .map_err(|error| parquet::resource::ResourceExhausted {
                kind: error.kind,
                requested: error.requested,
                limit: error.limit,
            })
    }
}
pub struct Owner {
    scope: Arc<NativeResourceScope>,
    kernel: NativeResourceThreadPolicy,
    json: arrow_json::resource::ReaderResourcePolicy,
    parquet: parquet::resource::ReaderResourcePolicy,
    url: NativeUrlThreadPolicy,
}
impl std::fmt::Debug for Owner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("FaultInjectableOwner")
    }
}
impl RetainedResourceOwner for Owner {
    fn try_reserve_allocation(
        &self,
        request: ResourceAllocationRequest,
    ) -> Result<(), ResourceOwnerError> {
        self.scope
            .reserve(AllocationRequest {
                kind: request.kind,
                bytes: request.bytes,
            })
            .map_err(|error| match error {
                buoyant_kernel::Error::ResourceExhausted(error) => ResourceOwnerError {
                    kind: error.kind,
                    requested: error.requested,
                    limit: error.limit,
                },
                _ => unreachable!(),
            })
    }
    fn try_adopt(
        &self,
        original: Arc<dyn RetainedResourceOwner>,
    ) -> Result<(), ResourceOwnerError> {
        if (original.as_ref() as &dyn std::any::Any)
            .downcast_ref::<Self>()
            .is_some_and(|original| std::ptr::eq(self, original))
        {
            Ok(())
        } else {
            Err(ResourceOwnerError {
                kind: "foreign fixture owner",
                requested: 1,
                limit: 0,
            })
        }
    }
    fn record_failure(&self, error: ResourceOwnerError) {
        self.scope.record_failure(ResourceExhausted {
            kind: error.kind,
            requested: error.requested,
            limit: error.limit,
        });
    }
}
pub struct Guard {
    _url: NativeUrlThreadGuard,
    _parquet: parquet::resource::ReaderThreadGuard,
    _json: arrow_json::resource::ReaderThreadGuard,
    _arrow: arrow_schema::resource::ResourceOwnerThreadGuard,
    _kernel: NativeResourceThreadGuard,
}
impl Owner {
    pub fn new(
        scope: Arc<NativeResourceScope>,
        root: &url::Url,
        json_limits: JsonResourceLimits,
    ) -> Arc<Self> {
        let bank = Arc::new(Bank(scope.clone()));
        let kernel = NativeResourceThreadPolicy::try_new(scope.clone(), json_limits).unwrap();
        let json = arrow_json::resource::ReaderResourcePolicy::try_new(
            bank.clone(),
            arrow_json::resource::ReaderResourceLimits {
                // Original nested replay buffers each retain a receipt. This
                // operation may read a projection twice before a phase fault.
                allocations: 1024,
                collection_entries: 4096,
                string_bytes: 65536,
                nesting: 32,
            },
        )
        .unwrap();
        let parquet = parquet::resource::ReaderResourcePolicy::try_new(
            bank,
            parquet::resource::ReaderResourceLimits {
                allocations: 128,
                collection_entries: 4096,
                string_bytes: 65536,
                footer_bytes: 65536,
                page_bytes: 65536,
                page_values: 4096,
                output_values: 4096,
                output_bytes: 65536,
                schema_depth: 32,
                codec_bytes: 1 << 20,
            },
        )
        .unwrap();
        let urls = LocalUrlJoinAdmission::try_new(scope.clone(), std::slice::from_ref(root), 65536)
            .unwrap();
        let url = NativeUrlThreadPolicy::new(scope.clone(), urls);
        Arc::new(Self {
            scope,
            kernel,
            json,
            parquet,
            url,
        })
    }
    pub fn enter(self: &Arc<Self>) -> Guard {
        let kernel = self.kernel.enter_thread();
        let arrow = arrow_schema::resource::enter_resource_owner(self.clone());
        let json = self.json.enter_thread().unwrap();
        let parquet = self.parquet.enter_thread();
        let url = self.url.enter_thread().unwrap();
        Guard {
            _url: url,
            _parquet: parquet,
            _json: json,
            _arrow: arrow,
            _kernel: kernel,
        }
    }
    pub fn runtime(self: &Arc<Self>) -> tokio::runtime::Runtime {
        let owner = self.clone();
        native_runtime::runtime(
            self.scope.clone(),
            move || WORKER.with(|slot| *slot.borrow_mut() = Some(owner.enter())),
            || {
                WORKER.with(|slot| {
                    slot.borrow_mut().take();
                })
            },
        )
    }
}
thread_local! { static WORKER: std::cell::RefCell<Option<Guard>> = const { std::cell::RefCell::new(None) }; }
