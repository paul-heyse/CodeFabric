//! Workspace admission for actual native allocation and retained owner APIs.
//! Staged against exact native sources before synchronized dependency adoption.
use std::any::Any;
use std::fmt;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use crate::resource_budget::{
    ResourceAmounts, ResourceBudget, ResourceBudgetError, ResourceClass, ResourceReservation,
    ResourceScopeKind,
};
use arrow_schema::resource::{
    ResourceAllocationRequest, ResourceOwnerError, RetainedResourceOwner,
};
use buoyant_kernel::resource as kernel;

static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeResourceLimits {
    pub kernel_allocations: usize,
    pub original_owners: usize,
    pub original_owner_depth: usize,
    pub kernel_json: kernel::JsonResourceLimits,
    pub url_reference_bytes: usize,
    pub json: arrow_json::resource::ReaderResourceLimits,
    pub parquet: parquet::resource::ReaderResourceLimits,
}

#[derive(Debug)]
struct NativeReceipt {
    bytes: usize,
    _reservation: ResourceReservation,
}
impl kernel::AllocationReceipt for NativeReceipt {
    fn bytes(&self) -> usize {
        self.bytes
    }
}
impl arrow_json::resource::ResourceReceipt for NativeReceipt {
    fn bytes(&self) -> usize {
        self.bytes
    }
}
impl parquet::resource::ResourceReceipt for NativeReceipt {
    fn bytes(&self) -> usize {
        self.bytes
    }
}

struct NativeAdmission {
    budget: ResourceBudget,
    class: ResourceClass,
    scope: Mutex<Weak<kernel::NativeResourceScope>>,
    _reservation: ResourceReservation,
}
impl fmt::Debug for NativeAdmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NativeAdmission { .. }")
    }
}

fn exhausted(
    error: ResourceBudgetError,
    kind: &'static str,
    requested: usize,
) -> kernel::ResourceExhausted {
    let limit = match error {
        ResourceBudgetError::Exhausted { limit, .. } => {
            usize::try_from(limit).unwrap_or(usize::MAX)
        }
        _ => 0,
    };
    kernel::ResourceExhausted {
        kind,
        requested,
        limit,
    }
}
fn kernel_failure(error: buoyant_kernel::Error) -> kernel::ResourceExhausted {
    match error {
        buoyant_kernel::Error::ResourceExhausted(error) => error,
        _ => kernel::ResourceExhausted {
            kind: "native policy construction",
            requested: 1,
            limit: 0,
        },
    }
}
fn layout(bytes: usize, kind: &'static str) -> Result<usize, kernel::ResourceExhausted> {
    bytes
        .checked_add(2 * size_of::<usize>())
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or(kernel::ResourceExhausted {
            kind,
            requested: usize::MAX,
            limit: isize::MAX as usize,
        })
}
fn reserve(
    budget: &ResourceBudget,
    class: ResourceClass,
    bytes: usize,
    kind: &'static str,
) -> Result<ResourceReservation, kernel::ResourceExhausted> {
    let memory_bytes = u64::try_from(bytes).map_err(|_| kernel::ResourceExhausted {
        kind,
        requested: bytes,
        limit: 0,
    })?;
    budget
        .try_reserve(
            class,
            ResourceAmounts {
                memory_bytes,
                ..ResourceAmounts::default()
            },
        )
        .map_err(|error| exhausted(error, kind, bytes))
}
impl NativeAdmission {
    fn charge(
        &self,
        bytes: usize,
        kind: &'static str,
    ) -> Result<Arc<NativeReceipt>, kernel::ResourceExhausted> {
        let scope = self
            .scope
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .upgrade();
        if let Some(error) = scope.as_ref().and_then(|scope| scope.failure()) {
            return Err(error);
        }
        if scope
            .as_ref()
            .is_some_and(|scope| scope.allocations_sealed())
        {
            return Err(kernel::ResourceExhausted {
                kind: "native allocation scope has joined",
                requested: bytes,
                limit: 0,
            });
        }
        let result = (|| {
            // The receipt's own concrete Arc allocation is admitted before Arc::new.
            let full = bytes
                .checked_add(layout(size_of::<NativeReceipt>(), kind)?)
                .filter(|bytes| *bytes <= isize::MAX as usize)
                .ok_or(kernel::ResourceExhausted {
                    kind,
                    requested: usize::MAX,
                    limit: isize::MAX as usize,
                })?;
            let reservation = reserve(&self.budget, self.class, full, kind)?;
            Ok(Arc::new(NativeReceipt {
                bytes,
                _reservation: reservation,
            }))
        })();
        if let (Err(error), Some(scope)) = (&result, scope) {
            scope.record_failure(*error);
        }
        result
    }
}
impl kernel::AllocationAdmission for NativeAdmission {
    fn try_reserve(
        &self,
        request: kernel::AllocationRequest,
    ) -> Result<Arc<dyn kernel::AllocationReceipt>, kernel::ResourceExhausted> {
        Ok(self.charge(request.bytes, request.kind)?)
    }
}
impl arrow_json::resource::ResourceAdmission for NativeAdmission {
    fn try_reserve(
        &self,
        request: arrow_json::resource::ResourceRequest,
    ) -> Result<
        Arc<dyn arrow_json::resource::ResourceReceipt>,
        arrow_json::resource::ResourceExhausted,
    > {
        self.charge(request.bytes, request.kind)
            .map(|receipt| receipt as Arc<dyn arrow_json::resource::ResourceReceipt>)
            .map_err(|error| arrow_json::resource::ResourceExhausted {
                kind: error.kind,
                requested: error.requested,
                limit: error.limit,
            })
    }
}
impl parquet::resource::ResourceAdmission for NativeAdmission {
    fn try_reserve(
        &self,
        request: parquet::resource::ResourceRequest,
    ) -> Result<Arc<dyn parquet::resource::ResourceReceipt>, parquet::resource::ResourceExhausted>
    {
        self.charge(request.bytes, request.kind)
            .map(|receipt| receipt as Arc<dyn parquet::resource::ResourceReceipt>)
            .map_err(|error| parquet::resource::ResourceExhausted {
                kind: error.kind,
                requested: error.requested,
                limit: error.limit,
            })
    }
}

/// An immutable operation identity with a bounded, acyclic list of original
/// input owners. It retains complete policy objects, including receipt banks.
pub(crate) struct NativeResourceOwner {
    id: u64,
    operation_claimed: AtomicBool,
    budget: ResourceBudget,
    scope: Arc<kernel::NativeResourceScope>,
    kernel: kernel::NativeResourceThreadPolicy,
    json: arrow_json::resource::ReaderResourcePolicy,
    parquet: parquet::resource::ReaderResourcePolicy,
    url: buoyant_kernel::path::url_resource::NativeUrlThreadPolicy,
    originals: Mutex<OriginalOwners>,
    max_original_depth: usize,
    _reservation: ResourceReservation,
}
struct OriginalOwners {
    values: Box<[Option<Arc<dyn RetainedResourceOwner>>]>,
    depth: usize,
    joined: bool,
}
impl fmt::Debug for NativeResourceOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeResourceOwner")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl NativeResourceOwner {
    pub(crate) fn try_new(
        budget: ResourceBudget,
        class: ResourceClass,
        limits: NativeResourceLimits,
        authorized_directories: &[url::Url],
    ) -> Result<Arc<Self>, kernel::ResourceExhausted> {
        if budget
            .ancestor_owner(ResourceScopeKind::Workspace)
            .is_none()
        {
            return Err(kernel::ResourceExhausted {
                kind: "native workspace owner required",
                requested: 1,
                limit: 0,
            });
        }
        let kind = "native original owner registry";
        if limits.original_owners == 0 || limits.original_owner_depth == 0 {
            return Err(kernel::ResourceExhausted {
                kind,
                requested: 0,
                limit: 1,
            });
        }
        let registry = limits
            .original_owners
            .checked_mul(size_of::<Option<Arc<dyn RetainedResourceOwner>>>())
            .and_then(|bytes| bytes.checked_add(size_of::<Self>()))
            .ok_or(kernel::ResourceExhausted {
                kind,
                requested: usize::MAX,
                limit: isize::MAX as usize,
            })?;
        let reservation = reserve(&budget, class, layout(registry, kind)?, kind)?;
        let mut originals = Vec::new();
        originals
            .try_reserve_exact(limits.original_owners)
            .map_err(|_| kernel::ResourceExhausted {
                kind,
                requested: registry,
                limit: 0,
            })?;
        originals.resize_with(limits.original_owners, || None);
        let admission_reservation = reserve(
            &budget,
            class,
            layout(size_of::<NativeAdmission>(), kind)?,
            kind,
        )?;
        let admission = Arc::new(NativeAdmission {
            budget: budget.clone(),
            class,
            scope: Mutex::new(Weak::new()),
            _reservation: admission_reservation,
        });
        let scope =
            kernel::NativeResourceScope::try_new(admission.clone(), limits.kernel_allocations)
                .map_err(kernel_failure)?;
        *admission
            .scope
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Arc::downgrade(&scope);
        let kernel = kernel::NativeResourceThreadPolicy::try_new(scope.clone(), limits.kernel_json)
            .map_err(kernel_failure)?;
        let json =
            arrow_json::resource::ReaderResourcePolicy::try_new(admission.clone(), limits.json)
                .map_err(|error| kernel::ResourceExhausted {
                    kind: error.kind,
                    requested: error.requested,
                    limit: error.limit,
                })?;
        let parquet = parquet::resource::ReaderResourcePolicy::try_new(admission, limits.parquet)
            .map_err(|error| kernel::ResourceExhausted {
            kind: error.kind,
            requested: error.requested,
            limit: error.limit,
        })?;
        let local_urls = buoyant_kernel::path::url_resource::LocalUrlJoinAdmission::try_new(
            scope.clone(),
            authorized_directories,
            limits.url_reference_bytes,
        )
        .map_err(kernel_failure)?;
        let url = buoyant_kernel::path::url_resource::NativeUrlThreadPolicy::new(
            scope.clone(),
            local_urls,
        );
        let id = NEXT_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| kernel::ResourceExhausted {
                kind: "native owner identities",
                requested: usize::MAX,
                limit: 0,
            })?;
        Ok(Arc::new(Self {
            id,
            operation_claimed: AtomicBool::new(false),
            budget,
            scope,
            kernel,
            json,
            parquet,
            url,
            originals: Mutex::new(OriginalOwners {
                values: originals.into_boxed_slice(),
                depth: 0,
                joined: false,
            }),
            max_original_depth: limits.original_owner_depth,
            _reservation: reservation,
        }))
    }

    pub(crate) fn claim_operation(&self) -> Result<(), kernel::ResourceExhausted> {
        self.check_available()?;
        if self.scope.allocations_sealed() || self.operation_claimed.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
            return Err(kernel::ResourceExhausted { kind: "native resource owner already bound", requested: 1, limit: 0 });
        }
        Ok(())
    }
    pub(crate) fn belongs_to(&self, budget: &ResourceBudget) -> bool {
        self.budget.same_root(budget)
            && self.budget.ancestor_owner(ResourceScopeKind::Workspace).is_some()
            && self.budget.ancestor_owner(ResourceScopeKind::Workspace) == budget.ancestor_owner(ResourceScopeKind::Workspace)
    }

    pub(crate) fn scope(&self) -> &Arc<kernel::NativeResourceScope> {
        &self.scope
    }
    pub(crate) fn check_available(&self) -> Result<(), kernel::ResourceExhausted> {
        match self.scope.failure() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Called only after the operation's native runtime has joined. Freezing
    /// its ancestry prevents later mutations from defeating the depth bound.
    pub(crate) fn mark_joined(&self) -> Result<(), kernel::ResourceExhausted> {
        self.check_available()?;
        self.originals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .joined = true;
        self.scope.seal_allocations();
        Ok(())
    }

    /// All five guards remain on this dedicated OS thread for its full lifetime.
    pub(crate) fn enter_thread(
        self: &Arc<Self>,
    ) -> Result<NativeResourceThreadGuard, kernel::ResourceExhausted> {
        // Install even after another worker has latched failure: a late-starting
        // blocking thread must still reject native allocations under this owner.
        // Leaving that thread ungoverned would turn pressure into an escape.
        if kernel::current_resource_scope().is_some()
            || arrow_schema::resource::current_resource_owner().is_some()
            || arrow_json::resource::ReaderResourcePolicy::current().is_some()
            || parquet::resource::ReaderResourcePolicy::current().is_some()
        {
            let error = kernel::ResourceExhausted {
                kind: "native worker already has a resource owner",
                requested: 1,
                limit: 0,
            };
            self.scope.record_failure(error);
            return Err(error);
        }
        let kernel = self.kernel.enter_thread();
        let arrow = arrow_schema::resource::enter_resource_owner(self.clone());
        let json = self.json.enter_thread().map_err(|error| {
            let error = kernel::ResourceExhausted {
                kind: error.kind,
                requested: error.requested,
                limit: error.limit,
            };
            self.scope.record_failure(error);
            error
        })?;
        let parquet = self.parquet.enter_thread();
        let url = self.url.enter_thread().map_err(kernel_failure)?;
        Ok(NativeResourceThreadGuard {
            _url: url,
            _parquet: parquet,
            _json: json,
            _arrow: arrow,
            _kernel: kernel,
        })
    }
}

impl RetainedResourceOwner for NativeResourceOwner {
    fn try_reserve_allocation(
        &self,
        request: ResourceAllocationRequest,
    ) -> Result<(), ResourceOwnerError> {
        if self
            .originals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .joined
        {
            let error = ResourceOwnerError {
                kind: "native joined owner cannot allocate",
                requested: request.bytes,
                limit: 0,
            };
            self.record_failure(error);
            return Err(error);
        }
        if std::alloc::Layout::from_size_align(request.bytes, request.alignment).is_err() {
            let error = ResourceOwnerError {
                kind: request.kind,
                requested: request.bytes,
                limit: isize::MAX as usize,
            };
            self.record_failure(error);
            return Err(error);
        }
        self.scope
            .reserve(kernel::AllocationRequest {
                kind: request.kind,
                bytes: request.bytes,
            })
            .map_err(|error| {
                let failure = self.scope.failure().unwrap_or(kernel::ResourceExhausted {
                    kind: request.kind,
                    requested: request.bytes,
                    limit: 0,
                });
                let _ = error;
                ResourceOwnerError {
                    kind: failure.kind,
                    requested: failure.requested,
                    limit: failure.limit,
                }
            })
    }

    fn try_adopt(
        &self,
        original: Arc<dyn RetainedResourceOwner>,
    ) -> Result<(), ResourceOwnerError> {
        let reject = |kind, requested, limit| {
            let error = ResourceOwnerError {
                kind,
                requested,
                limit,
            };
            self.record_failure(error);
            error
        };
        self.check_available().map_err(|error| ResourceOwnerError {
            kind: error.kind,
            requested: error.requested,
            limit: error.limit,
        })?;
        let Some(owner) = (original.as_ref() as &dyn Any).downcast_ref::<Self>() else {
            return Err(reject("native foreign owner type", 1, 0));
        };
        if owner.id == self.id {
            return Ok(());
        }
        // Strictly decreasing IDs along every edge forbid cycles without a
        // recursive traversal or allocation. Original owner IDs never change.
        if owner.id >= self.id {
            return Err(reject("native owner cycle", 1, 0));
        }
        if !self.budget.same_root(&owner.budget)
            || self.budget.ancestor_owner(ResourceScopeKind::Workspace)
                != owner.budget.ancestor_owner(ResourceScopeKind::Workspace)
        {
            return Err(reject("native foreign workspace owner", 1, 0));
        }
        let original_depth = {
            let originals = owner
                .originals
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !originals.joined {
                return Err(reject("native input owner has not joined", 1, 0));
            }
            originals.depth
        };
        let depth = original_depth
            .checked_add(1)
            .filter(|depth| *depth <= self.max_original_depth)
            .ok_or_else(|| {
                reject(
                    "native original owner depth",
                    original_depth.saturating_add(1),
                    self.max_original_depth,
                )
            })?;
        let mut originals = self
            .originals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if originals.joined {
            return Err(reject("native joined owner cannot adopt", 1, 0));
        }
        if originals
            .values
            .iter()
            .flatten()
            .any(|existing| Arc::ptr_eq(existing, &original))
        {
            return Ok(());
        }
        let limit = originals.values.len();
        let slot = originals
            .values
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or_else(|| {
                reject(
                    "native original owner slots",
                    limit.saturating_add(1),
                    limit,
                )
            })?;
        *slot = Some(original);
        originals.depth = originals.depth.max(depth);
        Ok(())
    }

    fn record_failure(&self, error: ResourceOwnerError) {
        self.scope.record_failure(kernel::ResourceExhausted {
            kind: error.kind,
            requested: error.requested,
            limit: error.limit,
        });
    }
}

pub(crate) struct NativeResourceThreadGuard {
    _url: buoyant_kernel::path::url_resource::NativeUrlThreadGuard,
    _parquet: parquet::resource::ReaderThreadGuard,
    _json: arrow_json::resource::ReaderThreadGuard,
    _arrow: arrow_schema::resource::ResourceOwnerThreadGuard,
    _kernel: kernel::NativeResourceThreadGuard,
}
