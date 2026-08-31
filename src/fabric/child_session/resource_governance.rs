//! Epoch-scoped resource admission and shared DataFusion execution resources.
//!
//! The coordinator is an application-owned overlay around DataFusion's native
//! [`RuntimeEnv`] resource authorities. Every reduced child session gets a closed
//! object-store registry but shares the epoch's exact memory pool, disk manager,
//! and caches. Scheduling remains application-owned because DataFusion does not
//! provide daemon-wide agent fairness, update reservations, result-lease quotas,
//! or admission backpressure.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use datafusion::execution::runtime_env::{RuntimeEnv, RuntimeEnvBuilder};
use tokio::sync::oneshot;
use tokio::time::Instant;

use crate::cancellation::Cancellation;
use crate::fabric::arrow_result_resource::{
    ArrowResultResourceError, ArrowResultResourcePackage, ResultResourceLease,
};
use crate::fabric::command::{EpochId, LeaseId, PrincipalId};

use super::{ChildResourceLimits, ClosedObjectStoreRegistry};

/// Static work-class discriminants from the lifecycle execution protocol.
///
/// Priority and reserved-headroom eligibility are deliberately absent: those
/// are epoch policy facts supplied through [`EpochResourcePolicy`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EpochWorkClass {
    SecurityRecovery,
    SourceReconciliation,
    StrictCurrentUpdate,
    SourceUpdate,
    InteractiveQuery,
    SemanticDerived,
    DurableFlushArtifact,
    Maintenance,
}

impl EpochWorkClass {
    const ALL: [Self; 8] = [
        Self::SecurityRecovery,
        Self::SourceReconciliation,
        Self::StrictCurrentUpdate,
        Self::SourceUpdate,
        Self::InteractiveQuery,
        Self::SemanticDerived,
        Self::DurableFlushArtifact,
        Self::Maintenance,
    ];

    fn protocol_index(self) -> usize {
        Self::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .expect("work class is a member of the closed protocol discriminant")
    }
}

/// One model/configuration-owned scheduling fact for a static work class.
///
/// A policy supplies exactly one rule per [`EpochWorkClass`] and a unique,
/// dense priority rank. Lower ranks receive service first before bounded aging.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochWorkClassPolicy {
    class: EpochWorkClass,
    priority_rank: u8,
    reserved_headroom_eligible: bool,
}

impl EpochWorkClassPolicy {
    #[must_use]
    pub const fn new(
        class: EpochWorkClass,
        priority_rank: u8,
        reserved_headroom_eligible: bool,
    ) -> Self {
        Self {
            class,
            priority_rank,
            reserved_headroom_eligible,
        }
    }

    #[must_use]
    pub const fn class(self) -> EpochWorkClass {
        self.class
    }

    #[must_use]
    pub const fn priority_rank(self) -> u8 {
        self.priority_rank
    }

    #[must_use]
    pub const fn reserved_headroom_eligible(self) -> bool {
        self.reserved_headroom_eligible
    }
}

/// Explicit coordinator policy. Row and payload-byte authority deliberately
/// remain in `ChildSessionPolicy` and `ArrowResultResourceLimits`; this type
/// adds only aggregate scheduling and retained-resource bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochResourcePolicy {
    datafusion_resources: ChildResourceLimits,
    work_class_policies: BTreeMap<EpochWorkClass, EpochWorkClassPolicy>,
    max_concurrent_work: NonZeroUsize,
    reserved_update_slots: usize,
    max_queued_work: NonZeroUsize,
    max_execution_millis: NonZeroU64,
    cancellation_poll_millis: NonZeroU64,
    aging_grants_per_priority_step: NonZeroU64,
    max_live_result_leases: NonZeroUsize,
    max_retained_result_bytes: NonZeroU64,
    max_result_lease_millis: NonZeroU64,
}

impl EpochResourcePolicy {
    /// Construct one fully bounded epoch policy.
    ///
    /// # Errors
    ///
    /// Rejects zero bounds and a reservation that leaves no general-purpose
    /// slot for query/provider/maintenance work.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        datafusion_resources: ChildResourceLimits,
        work_class_policies: Vec<EpochWorkClassPolicy>,
        max_concurrent_work: usize,
        reserved_update_slots: usize,
        max_queued_work: usize,
        max_execution_millis: u64,
        cancellation_poll_millis: u64,
        aging_grants_per_priority_step: u64,
        max_live_result_leases: usize,
        max_retained_result_bytes: u64,
        max_result_lease_millis: u64,
    ) -> Result<Self, EpochResourceError> {
        let max_concurrent_work = NonZeroUsize::new(max_concurrent_work)
            .ok_or(EpochResourceError::InvalidPolicy("max_concurrent_work"))?;
        if reserved_update_slots >= max_concurrent_work.get() {
            return Err(EpochResourceError::InvalidPolicy(
                "reserved_update_slots must leave one general slot",
            ));
        }
        let mut policies = BTreeMap::new();
        let mut priorities = BTreeMap::new();
        for class_policy in work_class_policies {
            if usize::from(class_policy.priority_rank) >= EpochWorkClass::ALL.len() {
                return Err(EpochResourceError::InvalidWorkClassPriority {
                    class: class_policy.class,
                    priority_rank: class_policy.priority_rank,
                    class_count: EpochWorkClass::ALL.len(),
                });
            }
            if policies.insert(class_policy.class, class_policy).is_some() {
                return Err(EpochResourceError::DuplicateWorkClassPolicy(
                    class_policy.class,
                ));
            }
            if let Some(existing) =
                priorities.insert(class_policy.priority_rank, class_policy.class)
            {
                return Err(EpochResourceError::DuplicateWorkClassPriority {
                    priority_rank: class_policy.priority_rank,
                    first: existing,
                    second: class_policy.class,
                });
            }
        }
        if let Some(missing) = EpochWorkClass::ALL
            .into_iter()
            .find(|class| !policies.contains_key(class))
        {
            return Err(EpochResourceError::MissingWorkClassPolicy(missing));
        }
        if reserved_update_slots > 0
            && !policies
                .values()
                .any(|class_policy| class_policy.reserved_headroom_eligible)
        {
            return Err(EpochResourceError::ReservedHeadroomWithoutEligibleClass);
        }
        Ok(Self {
            datafusion_resources,
            work_class_policies: policies,
            max_concurrent_work,
            reserved_update_slots,
            max_queued_work: NonZeroUsize::new(max_queued_work)
                .ok_or(EpochResourceError::InvalidPolicy("max_queued_work"))?,
            max_execution_millis: NonZeroU64::new(max_execution_millis)
                .ok_or(EpochResourceError::InvalidPolicy("max_execution_millis"))?,
            cancellation_poll_millis: NonZeroU64::new(cancellation_poll_millis).ok_or(
                EpochResourceError::InvalidPolicy("cancellation_poll_millis"),
            )?,
            aging_grants_per_priority_step: NonZeroU64::new(aging_grants_per_priority_step).ok_or(
                EpochResourceError::InvalidPolicy("aging_grants_per_priority_step"),
            )?,
            max_live_result_leases: NonZeroUsize::new(max_live_result_leases)
                .ok_or(EpochResourceError::InvalidPolicy("max_live_result_leases"))?,
            max_retained_result_bytes: NonZeroU64::new(max_retained_result_bytes).ok_or(
                EpochResourceError::InvalidPolicy("max_retained_result_bytes"),
            )?,
            max_result_lease_millis: NonZeroU64::new(max_result_lease_millis)
                .ok_or(EpochResourceError::InvalidPolicy("max_result_lease_millis"))?,
        })
    }

    #[must_use]
    pub const fn datafusion_resources(&self) -> &ChildResourceLimits {
        &self.datafusion_resources
    }

    fn work_class_policy(&self, class: EpochWorkClass) -> EpochWorkClassPolicy {
        *self
            .work_class_policies
            .get(&class)
            .expect("complete work-class coverage is validated at construction")
    }
}

/// One epoch/resource/principal-pinned admission request.
#[derive(Clone, Debug)]
pub struct EpochWorkRequest {
    pub epoch_id: EpochId,
    pub principal_id: PrincipalId,
    pub class: EpochWorkClass,
    pub cancellation: Cancellation,
}

/// Read-only resource state suitable for system relations and tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochResourceObservation {
    pub epoch_id: EpochId,
    pub resource_policy: [u8; 32],
    pub memory_limit_bytes: usize,
    pub memory_reserved_bytes: usize,
    pub max_spill_bytes: u64,
    pub spilled_bytes: u64,
    pub active_spill_files: usize,
    pub metadata_cache_limit_bytes: usize,
    pub file_statistics_cache_limit_bytes: usize,
    pub object_list_cache_limit_bytes: usize,
    pub object_list_cache_ttl_seconds: Option<u64>,
    pub logical_plan_cache_capacity_entries: usize,
    pub active_work: usize,
    pub queued_work: usize,
    pub active_by_class: BTreeMap<EpochWorkClass, usize>,
    pub queued_by_class: BTreeMap<EpochWorkClass, usize>,
    pub live_result_leases: usize,
    pub retained_result_bytes: u64,
}

/// Shared epoch resource authority. Cloning preserves the same scheduler and
/// DataFusion memory/spill domain.
#[derive(Clone)]
pub struct EpochResourceCoordinator {
    inner: Arc<ResourceInner>,
}

impl fmt::Debug for EpochResourceCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EpochResourceCoordinator")
            .field("epoch_id", &self.inner.epoch_id)
            .field("resource_policy", &"REDACTED_IDENTITY")
            .finish_non_exhaustive()
    }
}

struct ResourceInner {
    epoch_id: EpochId,
    resource_policy: [u8; 32],
    policy: EpochResourcePolicy,
    datafusion_runtime: Arc<RuntimeEnv>,
    state: Mutex<SchedulerState>,
}

struct SchedulerState {
    queues: BTreeMap<EpochWorkClass, FairClassQueue>,
    active_by_class: BTreeMap<EpochWorkClass, usize>,
    queued_work: usize,
    active_work: usize,
    active_non_reserved: usize,
    next_waiter_id: u64,
    dispatch_sequence: u64,
    next_class_index: usize,
    live_result_leases: usize,
    retained_result_bytes: u64,
}

impl Default for SchedulerState {
    fn default() -> Self {
        Self {
            queues: BTreeMap::new(),
            active_by_class: BTreeMap::new(),
            queued_work: 0,
            active_work: 0,
            active_non_reserved: 0,
            next_waiter_id: 1,
            dispatch_sequence: 0,
            next_class_index: 0,
            live_result_leases: 0,
            retained_result_bytes: 0,
        }
    }
}

#[derive(Default)]
struct FairClassQueue {
    by_principal: BTreeMap<PrincipalId, VecDeque<Waiter>>,
    principal_ring: VecDeque<PrincipalId>,
}

struct Waiter {
    id: u64,
    principal_id: PrincipalId,
    class: EpochWorkClass,
    enqueue_sequence: u64,
    deadline: Instant,
    cancellation: Cancellation,
    sender: oneshot::Sender<EpochWorkPermit>,
}

impl FairClassQueue {
    fn push(&mut self, waiter: Waiter) {
        let principal = waiter.principal_id;
        let queue = self.by_principal.entry(principal).or_default();
        if queue.is_empty() {
            self.principal_ring.push_back(principal);
        }
        queue.push_back(waiter);
    }

    fn len(&self) -> usize {
        self.by_principal.values().map(VecDeque::len).sum()
    }

    fn oldest_sequence(&self) -> Option<u64> {
        self.by_principal
            .values()
            .filter_map(|queue| queue.front().map(|waiter| waiter.enqueue_sequence))
            .min()
    }

    fn pop_fair(&mut self) -> Option<Waiter> {
        while let Some(principal) = self.principal_ring.pop_front() {
            let Some(queue) = self.by_principal.get_mut(&principal) else {
                continue;
            };
            let waiter = queue.pop_front();
            if queue.is_empty() {
                self.by_principal.remove(&principal);
            } else {
                self.principal_ring.push_back(principal);
            }
            if waiter.is_some() {
                return waiter;
            }
        }
        None
    }

    fn remove(&mut self, waiter_id: u64) -> bool {
        let principal = self.by_principal.iter_mut().find_map(|(principal, queue)| {
            queue
                .iter()
                .position(|waiter| waiter.id == waiter_id)
                .map(|position| (*principal, position))
        });
        let Some((principal, position)) = principal else {
            return false;
        };
        let queue = self
            .by_principal
            .get_mut(&principal)
            .expect("principal was selected from this map");
        queue.remove(position);
        if queue.is_empty() {
            self.by_principal.remove(&principal);
            self.principal_ring.retain(|queued| *queued != principal);
        }
        true
    }
}

impl EpochResourceCoordinator {
    /// Construct an epoch-pinned coordinator and its sole shared DataFusion
    /// memory/spill domain.
    ///
    /// # Errors
    ///
    /// Rejects sentinel epoch/policy identities and DataFusion runtime setup
    /// failures.
    pub fn try_new(
        epoch_id: EpochId,
        resource_policy: [u8; 32],
        policy: EpochResourcePolicy,
    ) -> Result<Self, EpochResourceError> {
        if all_zero(epoch_id.as_bytes()) {
            return Err(EpochResourceError::InvalidEpoch);
        }
        if all_zero(&resource_policy) {
            return Err(EpochResourceError::InvalidResourcePolicy);
        }
        let datafusion_runtime = policy.datafusion_resources.runtime_env()?;
        Ok(Self {
            inner: Arc::new(ResourceInner {
                epoch_id,
                resource_policy,
                policy,
                datafusion_runtime,
                state: Mutex::new(SchedulerState::default()),
            }),
        })
    }

    #[must_use]
    pub fn epoch_id(&self) -> EpochId {
        self.inner.epoch_id
    }

    #[must_use]
    pub fn resource_policy(&self) -> &[u8; 32] {
        &self.inner.resource_policy
    }

    #[must_use]
    pub fn policy(&self) -> &EpochResourcePolicy {
        &self.inner.policy
    }

    /// Admit one bounded operation through priority, aging, agent fairness,
    /// update headroom, queue capacity, cancellation, and deadline policy.
    pub async fn admit(
        &self,
        request: EpochWorkRequest,
    ) -> Result<EpochWorkPermit, EpochResourceError> {
        self.validate_request(&request)?;
        if request.cancellation.is_cancelled() {
            return Err(EpochResourceError::Cancelled);
        }
        let deadline =
            Instant::now() + Duration::from_millis(self.inner.policy.max_execution_millis.get());
        let (sender, mut receiver) = oneshot::channel();
        let waiter_id = {
            let mut state = self.lock_state()?;
            if state.queued_work >= self.inner.policy.max_queued_work.get() {
                return Err(EpochResourceError::Backpressure {
                    queued: state.queued_work,
                    limit: self.inner.policy.max_queued_work.get(),
                });
            }
            let waiter_id = state.next_waiter_id;
            let next_waiter_id = waiter_id
                .checked_add(1)
                .ok_or(EpochResourceError::CounterOverflow("next_waiter_id"))?;
            state.next_waiter_id = next_waiter_id;
            let enqueue_sequence = state.dispatch_sequence;
            state.queues.entry(request.class).or_default().push(Waiter {
                id: waiter_id,
                principal_id: request.principal_id,
                class: request.class,
                enqueue_sequence,
                deadline,
                cancellation: request.cancellation.clone(),
                sender,
            });
            state.queued_work += 1;
            self.dispatch_locked(&mut state);
            waiter_id
        };

        let cancellation_poll =
            Duration::from_millis(self.inner.policy.cancellation_poll_millis.get());
        loop {
            tokio::select! {
                biased;
                result = &mut receiver => {
                    return result.map_err(|_| EpochResourceError::StateUnavailable);
                }
                () = tokio::time::sleep_until(deadline) => {
                    self.remove_waiter(waiter_id);
                    return Err(EpochResourceError::DeadlineExceeded {
                        limit_millis: self.inner.policy.max_execution_millis.get(),
                    });
                }
                () = tokio::time::sleep(cancellation_poll) => {
                    if request.cancellation.is_cancelled() {
                        self.remove_waiter(waiter_id);
                        return Err(EpochResourceError::Cancelled);
                    }
                }
            }
        }
    }

    /// Reserve aggregate retained-result capacity. The returned permit must be
    /// moved into the result registry so release/expiry drops the reservation.
    pub fn retain_result(
        &self,
        principal_id: PrincipalId,
        lease: ResultResourceLease,
        package: &ArrowResultResourcePackage,
        observed_at_unix_ms: i64,
    ) -> Result<EpochResultLeasePermit, EpochResourceError> {
        if all_zero(principal_id.as_bytes()) {
            return Err(EpochResourceError::InvalidPrincipal);
        }
        if package.metadata().epoch_id() != self.inner.epoch_id {
            return Err(EpochResourceError::EpochMismatch {
                expected: self.inner.epoch_id,
                actual: package.metadata().epoch_id(),
            });
        }
        if package.lease() != lease {
            return Err(EpochResourceError::ResultLeaseMismatch);
        }
        if observed_at_unix_ms < lease.issued_at_unix_ms()
            || observed_at_unix_ms >= lease.expires_at_unix_ms()
        {
            return Err(EpochResourceError::ResultLeaseOutsideWindow);
        }
        let lease_millis = u64::try_from(
            lease
                .expires_at_unix_ms()
                .saturating_sub(lease.issued_at_unix_ms()),
        )
        .map_err(|_| EpochResourceError::ResultLeaseOutsideWindow)?;
        if lease_millis > self.inner.policy.max_result_lease_millis.get() {
            return Err(EpochResourceError::ResultLeaseDurationExceeded {
                requested_millis: lease_millis,
                limit_millis: self.inner.policy.max_result_lease_millis.get(),
            });
        }
        let retained_bytes = package.retained_resource_bytes()?;
        let mut state = self.lock_state()?;
        if state.live_result_leases >= self.inner.policy.max_live_result_leases.get() {
            return Err(EpochResourceError::ResultLeaseBackpressure {
                live: state.live_result_leases,
                limit: self.inner.policy.max_live_result_leases.get(),
            });
        }
        let next_bytes = state
            .retained_result_bytes
            .checked_add(retained_bytes)
            .ok_or(EpochResourceError::CounterOverflow("retained_result_bytes"))?;
        if next_bytes > self.inner.policy.max_retained_result_bytes.get() {
            return Err(EpochResourceError::ResultByteBackpressure {
                requested: retained_bytes,
                retained: state.retained_result_bytes,
                limit: self.inner.policy.max_retained_result_bytes.get(),
            });
        }
        state.live_result_leases += 1;
        state.retained_result_bytes = next_bytes;
        Ok(EpochResultLeasePermit {
            coordinator: self.clone(),
            epoch_id: self.inner.epoch_id,
            principal_id,
            lease_id: lease.lease_id(),
            retained_bytes,
            released: false,
        })
    }

    /// Observe scheduler and the exact shared DataFusion memory/spill domain.
    pub fn observation(&self) -> Result<EpochResourceObservation, EpochResourceError> {
        let state = self.lock_state()?;
        let spilling = self.inner.datafusion_runtime.spilling_progress();
        let queued_by_class = EpochWorkClass::ALL
            .into_iter()
            .map(|class| {
                (
                    class,
                    state.queues.get(&class).map_or(0, FairClassQueue::len),
                )
            })
            .collect();
        Ok(EpochResourceObservation {
            epoch_id: self.inner.epoch_id,
            resource_policy: self.inner.resource_policy,
            memory_limit_bytes: self.inner.policy.datafusion_resources.memory_limit_bytes,
            memory_reserved_bytes: self.inner.datafusion_runtime.memory_pool.reserved(),
            max_spill_bytes: self.inner.policy.datafusion_resources.max_spill_bytes,
            spilled_bytes: spilling.current_bytes,
            active_spill_files: spilling.active_files_count,
            metadata_cache_limit_bytes: self
                .inner
                .datafusion_runtime
                .cache_manager
                .get_metadata_cache_limit(),
            file_statistics_cache_limit_bytes: self
                .inner
                .datafusion_runtime
                .cache_manager
                .get_file_statistic_cache_limit(),
            object_list_cache_limit_bytes: self
                .inner
                .datafusion_runtime
                .cache_manager
                .get_list_files_cache_limit(),
            object_list_cache_ttl_seconds: self
                .inner
                .datafusion_runtime
                .cache_manager
                .get_list_files_cache_ttl()
                .map(|ttl| ttl.as_secs()),
            logical_plan_cache_capacity_entries: self
                .inner
                .policy
                .datafusion_resources
                .cache_policy()
                .logical_plan_entries(),
            active_work: state.active_work,
            queued_work: state.queued_work,
            active_by_class: state.active_by_class.clone(),
            queued_by_class,
            live_result_leases: state.live_result_leases,
            retained_result_bytes: state.retained_result_bytes,
        })
    }

    pub(super) fn child_runtime_env(
        &self,
        epoch_id: EpochId,
        resource_policy: &[u8; 32],
        requested: &ChildResourceLimits,
    ) -> Result<Arc<RuntimeEnv>, EpochResourceError> {
        if epoch_id != self.inner.epoch_id {
            return Err(EpochResourceError::EpochMismatch {
                expected: self.inner.epoch_id,
                actual: epoch_id,
            });
        }
        if resource_policy != &self.inner.resource_policy {
            return Err(EpochResourceError::ResourcePolicyMismatch);
        }
        if requested != &self.inner.policy.datafusion_resources {
            return Err(EpochResourceError::DataFusionResourceMismatch);
        }
        RuntimeEnvBuilder::from_runtime_env(&self.inner.datafusion_runtime)
            .with_object_store_registry(Arc::new(ClosedObjectStoreRegistry))
            .build_arc()
            .map_err(EpochResourceError::DataFusion)
    }

    fn validate_request(&self, request: &EpochWorkRequest) -> Result<(), EpochResourceError> {
        if request.epoch_id != self.inner.epoch_id {
            return Err(EpochResourceError::EpochMismatch {
                expected: self.inner.epoch_id,
                actual: request.epoch_id,
            });
        }
        if all_zero(request.principal_id.as_bytes()) {
            return Err(EpochResourceError::InvalidPrincipal);
        }
        Ok(())
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, SchedulerState>, EpochResourceError> {
        self.inner
            .state
            .lock()
            .map_err(|_| EpochResourceError::StateUnavailable)
    }

    fn class_has_capacity(&self, state: &SchedulerState, class: EpochWorkClass) -> bool {
        if state.active_work >= self.inner.policy.max_concurrent_work.get() {
            return false;
        }
        self.inner
            .policy
            .work_class_policy(class)
            .reserved_headroom_eligible
            || state.active_non_reserved
                < self.inner.policy.max_concurrent_work.get()
                    - self.inner.policy.reserved_update_slots
    }

    fn dispatch_locked(&self, state: &mut SchedulerState) {
        while state.active_work < self.inner.policy.max_concurrent_work.get() {
            let Some(class) = self.select_class(state) else {
                break;
            };
            let Some(waiter) = state
                .queues
                .get_mut(&class)
                .and_then(FairClassQueue::pop_fair)
            else {
                break;
            };
            state.queued_work = state.queued_work.saturating_sub(1);
            state.active_work += 1;
            if !self
                .inner
                .policy
                .work_class_policy(class)
                .reserved_headroom_eligible
            {
                state.active_non_reserved += 1;
            }
            *state.active_by_class.entry(class).or_default() += 1;
            state.dispatch_sequence = state.dispatch_sequence.saturating_add(1);
            state.next_class_index = (class.protocol_index() + 1) % EpochWorkClass::ALL.len();
            let permit = EpochWorkPermit {
                coordinator: self.clone(),
                principal_id: waiter.principal_id,
                class: waiter.class,
                deadline: waiter.deadline,
                cancellation: waiter.cancellation,
                released: false,
            };
            if let Err(mut permit) = waiter.sender.send(permit) {
                permit.released = true;
                self.release_active_locked(state, class);
            }
        }
    }

    fn select_class(&self, state: &SchedulerState) -> Option<EpochWorkClass> {
        EpochWorkClass::ALL
            .into_iter()
            .filter(|class| self.class_has_capacity(state, *class))
            .filter_map(|class| {
                let oldest = state.queues.get(&class)?.oldest_sequence()?;
                let age_steps = state.dispatch_sequence.saturating_sub(oldest)
                    / self.inner.policy.aging_grants_per_priority_step.get();
                let effective_priority = self
                    .inner
                    .policy
                    .work_class_policy(class)
                    .priority_rank
                    .saturating_sub(u8::try_from(age_steps).unwrap_or(u8::MAX));
                let rotation = (class.protocol_index() + EpochWorkClass::ALL.len()
                    - state.next_class_index)
                    % EpochWorkClass::ALL.len();
                Some(((effective_priority, rotation), class))
            })
            .min_by_key(|(key, _)| *key)
            .map(|(_, class)| class)
    }

    fn remove_waiter(&self, waiter_id: u64) {
        let Ok(mut state) = self.inner.state.lock() else {
            return;
        };
        let removed = state
            .queues
            .values_mut()
            .any(|queue| queue.remove(waiter_id));
        if removed {
            state.queued_work = state.queued_work.saturating_sub(1);
            self.dispatch_locked(&mut state);
        }
    }

    fn release_work(&self, class: EpochWorkClass) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.release_active_locked(&mut state, class);
        self.dispatch_locked(&mut state);
    }

    fn release_active_locked(&self, state: &mut SchedulerState, class: EpochWorkClass) {
        state.active_work = state.active_work.saturating_sub(1);
        if !self
            .inner
            .policy
            .work_class_policy(class)
            .reserved_headroom_eligible
        {
            state.active_non_reserved = state.active_non_reserved.saturating_sub(1);
        }
        if let Some(active) = state.active_by_class.get_mut(&class) {
            *active = active.saturating_sub(1);
        }
    }

    fn release_result(&self, retained_bytes: u64) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.live_result_leases = state.live_result_leases.saturating_sub(1);
        state.retained_result_bytes = state.retained_result_bytes.saturating_sub(retained_bytes);
    }
}

/// Active work reservation. Dropping it releases capacity and dispatches the
/// next fair waiter.
pub struct EpochWorkPermit {
    coordinator: EpochResourceCoordinator,
    principal_id: PrincipalId,
    class: EpochWorkClass,
    deadline: Instant,
    cancellation: Cancellation,
    released: bool,
}

impl fmt::Debug for EpochWorkPermit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EpochWorkPermit")
            .field("epoch_id", &self.coordinator.epoch_id())
            .field("principal_id", &self.principal_id)
            .field("class", &self.class)
            .finish_non_exhaustive()
    }
}

impl EpochWorkPermit {
    #[must_use]
    pub const fn class(&self) -> EpochWorkClass {
        self.class
    }

    #[must_use]
    pub const fn principal_id(&self) -> PrincipalId {
        self.principal_id
    }

    /// Fail at explicit synchronous phase boundaries after cancellation or
    /// deadline. This complements `run`, which drops an in-flight async stream.
    pub fn checkpoint(&self) -> Result<(), EpochResourceError> {
        if self.cancellation.is_cancelled() {
            return Err(EpochResourceError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(EpochResourceError::DeadlineExceeded {
                limit_millis: self.coordinator.inner.policy.max_execution_millis.get(),
            });
        }
        Ok(())
    }

    /// Run one async execution under the admission deadline and cooperative
    /// cancellation handle. Dropping the selected future is DataFusion's task/
    /// stream cancellation boundary.
    pub async fn run<T>(&self, future: impl Future<Output = T>) -> Result<T, EpochResourceError> {
        self.checkpoint()?;
        let poll =
            Duration::from_millis(self.coordinator.inner.policy.cancellation_poll_millis.get());
        tokio::pin!(future);
        loop {
            tokio::select! {
                biased;
                output = &mut future => return Ok(output),
                () = tokio::time::sleep_until(self.deadline) => {
                    return Err(EpochResourceError::DeadlineExceeded {
                        limit_millis: self.coordinator.inner.policy.max_execution_millis.get(),
                    });
                }
                () = tokio::time::sleep(poll) => {
                    if self.cancellation.is_cancelled() {
                        return Err(EpochResourceError::Cancelled);
                    }
                }
            }
        }
    }
}

impl Drop for EpochWorkPermit {
    fn drop(&mut self) {
        if !self.released {
            self.released = true;
            self.coordinator.release_work(self.class);
        }
    }
}

/// Aggregate result-storage reservation retained beside the epoch and package
/// leases until terminal release or expiry.
pub struct EpochResultLeasePermit {
    coordinator: EpochResourceCoordinator,
    epoch_id: EpochId,
    principal_id: PrincipalId,
    lease_id: LeaseId,
    retained_bytes: u64,
    released: bool,
}

impl fmt::Debug for EpochResultLeasePermit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EpochResultLeasePermit")
            .field("epoch_id", &self.epoch_id)
            .field("principal_id", &self.principal_id)
            .field("lease_id", &self.lease_id)
            .field("retained_bytes", &self.retained_bytes)
            .finish_non_exhaustive()
    }
}

impl EpochResultLeasePermit {
    #[must_use]
    pub const fn epoch_id(&self) -> EpochId {
        self.epoch_id
    }

    #[must_use]
    pub const fn principal_id(&self) -> PrincipalId {
        self.principal_id
    }

    #[must_use]
    pub const fn lease_id(&self) -> LeaseId {
        self.lease_id
    }

    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }
}

impl Drop for EpochResultLeasePermit {
    fn drop(&mut self) {
        if !self.released {
            self.released = true;
            self.coordinator.release_result(self.retained_bytes);
        }
    }
}

const fn all_zero<const N: usize>(value: &[u8; N]) -> bool {
    let mut index = 0;
    while index < N {
        if value[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

/// Fail-closed resource admission and retention outcomes.
#[derive(Debug, thiserror::Error)]
pub enum EpochResourceError {
    #[error("INVALID_EPOCH_RESOURCE_POLICY:{0}")]
    InvalidPolicy(&'static str),
    #[error("INVALID_EPOCH_WORK_CLASS_POLICY:DUPLICATE_CLASS:{0:?}")]
    DuplicateWorkClassPolicy(EpochWorkClass),
    #[error("INVALID_EPOCH_WORK_CLASS_POLICY:MISSING_CLASS:{0:?}")]
    MissingWorkClassPolicy(EpochWorkClass),
    #[error(
        "INVALID_EPOCH_WORK_CLASS_POLICY:PRIORITY_OUT_OF_RANGE:class={class:?}:priority={priority_rank}:class_count={class_count}"
    )]
    InvalidWorkClassPriority {
        class: EpochWorkClass,
        priority_rank: u8,
        class_count: usize,
    },
    #[error(
        "INVALID_EPOCH_WORK_CLASS_POLICY:DUPLICATE_PRIORITY:priority={priority_rank}:first={first:?}:second={second:?}"
    )]
    DuplicateWorkClassPriority {
        priority_rank: u8,
        first: EpochWorkClass,
        second: EpochWorkClass,
    },
    #[error("INVALID_EPOCH_WORK_CLASS_POLICY:RESERVED_HEADROOM_WITHOUT_ELIGIBLE_CLASS")]
    ReservedHeadroomWithoutEligibleClass,
    #[error("INVALID_EPOCH_RESOURCE_IDENTITY")]
    InvalidEpoch,
    #[error("INVALID_EPOCH_RESOURCE_POLICY_IDENTITY")]
    InvalidResourcePolicy,
    #[error("INVALID_EPOCH_RESOURCE_PRINCIPAL")]
    InvalidPrincipal,
    #[error("EPOCH_RESOURCE_PIN_MISMATCH:expected={expected:?}:actual={actual:?}")]
    EpochMismatch { expected: EpochId, actual: EpochId },
    #[error("EPOCH_RESOURCE_POLICY_PIN_MISMATCH")]
    ResourcePolicyMismatch,
    #[error("EPOCH_DATAFUSION_RESOURCE_PROFILE_MISMATCH")]
    DataFusionResourceMismatch,
    #[error("EPOCH_RESOURCE_BACKPRESSURE:queued={queued}:limit={limit}")]
    Backpressure { queued: usize, limit: usize },
    #[error("EPOCH_RESOURCE_CANCELLED")]
    Cancelled,
    #[error("EPOCH_RESOURCE_DEADLINE_EXCEEDED:{limit_millis}ms")]
    DeadlineExceeded { limit_millis: u64 },
    #[error("EPOCH_RESULT_LEASE_MISMATCH")]
    ResultLeaseMismatch,
    #[error("EPOCH_RESULT_LEASE_OUTSIDE_WINDOW")]
    ResultLeaseOutsideWindow,
    #[error(
        "EPOCH_RESULT_LEASE_DURATION_EXCEEDED:requested={requested_millis}:limit={limit_millis}"
    )]
    ResultLeaseDurationExceeded {
        requested_millis: u64,
        limit_millis: u64,
    },
    #[error("EPOCH_RESULT_LEASE_BACKPRESSURE:live={live}:limit={limit}")]
    ResultLeaseBackpressure { live: usize, limit: usize },
    #[error(
        "EPOCH_RESULT_BYTE_BACKPRESSURE:requested={requested}:retained={retained}:limit={limit}"
    )]
    ResultByteBackpressure {
        requested: u64,
        retained: u64,
        limit: u64,
    },
    #[error("EPOCH_RESOURCE_COUNTER_OVERFLOW:{0}")]
    CounterOverflow(&'static str),
    #[error("EPOCH_RESOURCE_STATE_UNAVAILABLE")]
    StateUnavailable,
    #[error(transparent)]
    DataFusion(#[from] datafusion::common::DataFusionError),
    #[error(transparent)]
    ArrowResult(#[from] ArrowResultResourceError),
}

#[cfg(test)]
pub(crate) fn test_lifecycle_work_class_policies() -> Vec<EpochWorkClassPolicy> {
    vec![
        EpochWorkClassPolicy::new(EpochWorkClass::SecurityRecovery, 0, true),
        EpochWorkClassPolicy::new(EpochWorkClass::SourceReconciliation, 1, true),
        EpochWorkClassPolicy::new(EpochWorkClass::StrictCurrentUpdate, 2, true),
        EpochWorkClassPolicy::new(EpochWorkClass::SourceUpdate, 3, true),
        EpochWorkClassPolicy::new(EpochWorkClass::InteractiveQuery, 4, false),
        EpochWorkClassPolicy::new(EpochWorkClass::SemanticDerived, 5, false),
        EpochWorkClassPolicy::new(EpochWorkClass::DurableFlushArtifact, 6, false),
        EpochWorkClassPolicy::new(EpochWorkClass::Maintenance, 7, false),
    ]
}

#[cfg(test)]
mod tests {
    use datafusion::execution::memory_pool::MemoryConsumer;

    use super::*;

    fn child_resources() -> ChildResourceLimits {
        ChildResourceLimits::try_new(1_024 * 1_024, 4 * 1_024 * 1_024, 4, 4, 128, 1).unwrap()
    }

    fn policy(
        concurrent: usize,
        reserved_updates: usize,
        queued: usize,
        deadline_millis: u64,
    ) -> EpochResourcePolicy {
        policy_with_work_class_policies(
            test_lifecycle_work_class_policies(),
            concurrent,
            reserved_updates,
            queued,
            deadline_millis,
        )
    }

    fn policy_with_work_class_policies(
        work_class_policies: Vec<EpochWorkClassPolicy>,
        concurrent: usize,
        reserved_updates: usize,
        queued: usize,
        deadline_millis: u64,
    ) -> EpochResourcePolicy {
        EpochResourcePolicy::try_new(
            child_resources(),
            work_class_policies,
            concurrent,
            reserved_updates,
            queued,
            deadline_millis,
            1,
            2,
            2,
            2 * 1_024 * 1_024,
            5_000,
        )
        .unwrap()
    }

    fn coordinator(policy: EpochResourcePolicy) -> EpochResourceCoordinator {
        EpochResourceCoordinator::try_new(EpochId::from_bytes([0xA5; 16]), [0x33; 32], policy)
            .unwrap()
    }

    fn request(
        principal: u8,
        class: EpochWorkClass,
        cancellation: Cancellation,
    ) -> EpochWorkRequest {
        EpochWorkRequest {
            epoch_id: EpochId::from_bytes([0xA5; 16]),
            principal_id: PrincipalId::from_bytes([principal; 16]),
            class,
            cancellation,
        }
    }

    fn reconfigured_work_class_policies() -> Vec<EpochWorkClassPolicy> {
        test_lifecycle_work_class_policies()
            .into_iter()
            .map(|class_policy| match class_policy.class() {
                EpochWorkClass::SourceUpdate => {
                    EpochWorkClassPolicy::new(EpochWorkClass::SourceUpdate, 7, false)
                }
                EpochWorkClass::Maintenance => {
                    EpochWorkClassPolicy::new(EpochWorkClass::Maintenance, 3, true)
                }
                _ => class_policy,
            })
            .collect()
    }

    #[test]
    fn work_class_policy_is_complete_unique_dense_and_headroom_capable() {
        let construct = |work_class_policies| {
            EpochResourcePolicy::try_new(
                child_resources(),
                work_class_policies,
                2,
                1,
                4,
                1_000,
                1,
                2,
                2,
                2 * 1_024 * 1_024,
                5_000,
            )
        };

        let mut missing = test_lifecycle_work_class_policies();
        missing.pop();
        assert!(matches!(
            construct(missing),
            Err(EpochResourceError::MissingWorkClassPolicy(
                EpochWorkClass::Maintenance
            ))
        ));

        let mut duplicate_class = test_lifecycle_work_class_policies();
        duplicate_class[1] = EpochWorkClassPolicy::new(EpochWorkClass::SecurityRecovery, 1, true);
        assert!(matches!(
            construct(duplicate_class),
            Err(EpochResourceError::DuplicateWorkClassPolicy(
                EpochWorkClass::SecurityRecovery
            ))
        ));

        let mut duplicate_priority = test_lifecycle_work_class_policies();
        duplicate_priority[7] = EpochWorkClassPolicy::new(EpochWorkClass::Maintenance, 6, false);
        assert!(matches!(
            construct(duplicate_priority),
            Err(EpochResourceError::DuplicateWorkClassPriority {
                priority_rank: 6,
                ..
            })
        ));

        let mut out_of_range = test_lifecycle_work_class_policies();
        out_of_range[7] = EpochWorkClassPolicy::new(EpochWorkClass::Maintenance, 8, false);
        assert!(matches!(
            construct(out_of_range),
            Err(EpochResourceError::InvalidWorkClassPriority {
                class: EpochWorkClass::Maintenance,
                priority_rank: 8,
                ..
            })
        ));

        let no_headroom_class = test_lifecycle_work_class_policies()
            .into_iter()
            .map(|class_policy| {
                EpochWorkClassPolicy::new(class_policy.class(), class_policy.priority_rank(), false)
            })
            .collect();
        assert!(matches!(
            construct(no_headroom_class),
            Err(EpochResourceError::ReservedHeadroomWithoutEligibleClass)
        ));
    }

    async fn selected_after_capacity_release(
        work_class_policies: Vec<EpochWorkClassPolicy>,
    ) -> EpochWorkClass {
        let coordinator = coordinator(policy_with_work_class_policies(
            work_class_policies,
            1,
            0,
            4,
            1_000,
        ));
        let blocker = coordinator
            .admit(request(
                1,
                EpochWorkClass::InteractiveQuery,
                Cancellation::default(),
            ))
            .await
            .unwrap();
        let source_cancellation = Cancellation::with_check_interval(1);
        let source = {
            let coordinator = coordinator.clone();
            let cancellation = source_cancellation.clone();
            tokio::spawn(async move {
                coordinator
                    .admit(request(2, EpochWorkClass::SourceUpdate, cancellation))
                    .await
            })
        };
        let maintenance_cancellation = Cancellation::with_check_interval(1);
        let maintenance = {
            let coordinator = coordinator.clone();
            let cancellation = maintenance_cancellation.clone();
            tokio::spawn(async move {
                coordinator
                    .admit(request(3, EpochWorkClass::Maintenance, cancellation))
                    .await
            })
        };
        for _ in 0..100 {
            if coordinator.observation().unwrap().queued_work == 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(coordinator.observation().unwrap().queued_work, 2);
        drop(blocker);
        for _ in 0..100 {
            let observation = coordinator.observation().unwrap();
            if observation.active_work == 1 && observation.queued_work == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
        let observation = coordinator.observation().unwrap();
        let selected = [EpochWorkClass::SourceUpdate, EpochWorkClass::Maintenance]
            .into_iter()
            .find(|class| observation.active_by_class.get(class) == Some(&1))
            .expect("one configured class must receive the released slot");
        source_cancellation.cancel();
        maintenance_cancellation.cancel();
        drop(source.await.unwrap());
        drop(maintenance.await.unwrap());
        selected
    }

    async fn selected_for_reserved_headroom(
        work_class_policies: Vec<EpochWorkClassPolicy>,
    ) -> EpochWorkClass {
        let coordinator = coordinator(policy_with_work_class_policies(
            work_class_policies,
            2,
            1,
            4,
            1_000,
        ));
        let blocker = coordinator
            .admit(request(
                1,
                EpochWorkClass::InteractiveQuery,
                Cancellation::default(),
            ))
            .await
            .unwrap();
        let source_cancellation = Cancellation::with_check_interval(1);
        let source = {
            let coordinator = coordinator.clone();
            let cancellation = source_cancellation.clone();
            tokio::spawn(async move {
                coordinator
                    .admit(request(2, EpochWorkClass::SourceUpdate, cancellation))
                    .await
            })
        };
        let maintenance_cancellation = Cancellation::with_check_interval(1);
        let maintenance = {
            let coordinator = coordinator.clone();
            let cancellation = maintenance_cancellation.clone();
            tokio::spawn(async move {
                coordinator
                    .admit(request(3, EpochWorkClass::Maintenance, cancellation))
                    .await
            })
        };
        for _ in 0..100 {
            if coordinator.observation().unwrap().active_work == 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        let observation = coordinator.observation().unwrap();
        assert_eq!(observation.active_work, 2);
        assert_eq!(observation.queued_work, 1);
        let selected = [EpochWorkClass::SourceUpdate, EpochWorkClass::Maintenance]
            .into_iter()
            .find(|class| observation.active_by_class.get(class) == Some(&1))
            .expect("one configured class must receive reserved headroom");
        source_cancellation.cancel();
        maintenance_cancellation.cancel();
        drop(source.await.unwrap());
        drop(maintenance.await.unwrap());
        drop(blocker);
        selected
    }

    #[tokio::test]
    async fn model_policy_changes_priority_and_headroom_without_code_changes() {
        assert_eq!(
            selected_after_capacity_release(test_lifecycle_work_class_policies()).await,
            EpochWorkClass::SourceUpdate
        );
        assert_eq!(
            selected_after_capacity_release(reconfigured_work_class_policies()).await,
            EpochWorkClass::Maintenance
        );
        assert_eq!(
            selected_for_reserved_headroom(test_lifecycle_work_class_policies()).await,
            EpochWorkClass::SourceUpdate
        );
        assert_eq!(
            selected_for_reserved_headroom(reconfigured_work_class_policies()).await,
            EpochWorkClass::Maintenance
        );
    }

    #[tokio::test]
    async fn waiter_identity_overflow_rejects_before_enqueuing() {
        let coordinator = coordinator(policy(1, 0, 2, 1_000));
        coordinator.inner.state.lock().unwrap().next_waiter_id = u64::MAX;

        assert!(matches!(
            coordinator
                .admit(request(
                    1,
                    EpochWorkClass::InteractiveQuery,
                    Cancellation::default(),
                ))
                .await,
            Err(EpochResourceError::CounterOverflow("next_waiter_id"))
        ));
        let observation = coordinator.observation().unwrap();
        assert_eq!(observation.active_work, 0);
        assert_eq!(observation.queued_work, 0);
    }

    #[tokio::test]
    async fn shared_datafusion_domain_closes_registries_and_accounts_one_pool() {
        let coordinator = coordinator(policy(2, 1, 4, 1_000));
        let first = coordinator
            .child_runtime_env(
                coordinator.epoch_id(),
                coordinator.resource_policy(),
                coordinator.policy().datafusion_resources(),
            )
            .unwrap();
        let second = coordinator
            .child_runtime_env(
                coordinator.epoch_id(),
                coordinator.resource_policy(),
                coordinator.policy().datafusion_resources(),
            )
            .unwrap();
        assert!(Arc::ptr_eq(&first.memory_pool, &second.memory_pool));
        assert!(Arc::ptr_eq(&first.disk_manager, &second.disk_manager));
        assert!(Arc::ptr_eq(
            &first.cache_manager.get_file_metadata_cache(),
            &second.cache_manager.get_file_metadata_cache()
        ));
        assert!(Arc::ptr_eq(
            &first
                .cache_manager
                .get_file_statistic_cache()
                .expect("statistics cache is enabled"),
            &second
                .cache_manager
                .get_file_statistic_cache()
                .expect("statistics cache is enabled")
        ));
        assert!(Arc::ptr_eq(
            &first
                .cache_manager
                .get_list_files_cache()
                .expect("object-list cache is enabled"),
            &second
                .cache_manager
                .get_list_files_cache()
                .expect("object-list cache is enabled")
        ));
        assert!(!Arc::ptr_eq(
            &first.object_store_registry,
            &second.object_store_registry
        ));

        let reservation = MemoryConsumer::new("wp19-shared-domain").register(&first.memory_pool);
        reservation.try_grow(4_096).unwrap();
        assert_eq!(
            coordinator.observation().unwrap().memory_reserved_bytes,
            4_096
        );
        reservation.free();
        assert_eq!(coordinator.observation().unwrap().memory_reserved_bytes, 0);
    }

    #[tokio::test]
    async fn update_headroom_agent_fairness_backpressure_and_cancellation_are_causal() {
        let coordinator = coordinator(policy(2, 1, 2, 1_000));
        let first_query = coordinator
            .admit(request(
                1,
                EpochWorkClass::InteractiveQuery,
                Cancellation::default(),
            ))
            .await
            .unwrap();

        let queued_query = {
            let coordinator = coordinator.clone();
            tokio::spawn(async move {
                coordinator
                    .admit(request(
                        2,
                        EpochWorkClass::InteractiveQuery,
                        Cancellation::default(),
                    ))
                    .await
            })
        };
        tokio::task::yield_now().await;
        let update = coordinator
            .admit(request(
                3,
                EpochWorkClass::SourceUpdate,
                Cancellation::default(),
            ))
            .await
            .unwrap();
        let observed = coordinator.observation().unwrap();
        assert_eq!(observed.active_work, 2);
        assert_eq!(observed.queued_work, 1);
        assert_eq!(
            observed.active_by_class[&EpochWorkClass::InteractiveQuery],
            1
        );
        assert_eq!(observed.active_by_class[&EpochWorkClass::SourceUpdate], 1);

        let cancelled = Cancellation::with_check_interval(1);
        let rejected = {
            let coordinator = coordinator.clone();
            let cancellation = cancelled.clone();
            tokio::spawn(async move {
                coordinator
                    .admit(request(4, EpochWorkClass::SemanticDerived, cancellation))
                    .await
            })
        };
        tokio::task::yield_now().await;
        assert!(matches!(
            coordinator
                .admit(request(
                    5,
                    EpochWorkClass::Maintenance,
                    Cancellation::default(),
                ))
                .await,
            Err(EpochResourceError::Backpressure { .. })
        ));
        cancelled.cancel();
        assert!(matches!(
            rejected.await.unwrap(),
            Err(EpochResourceError::Cancelled)
        ));

        drop(update);
        drop(first_query);
        let second_query = queued_query.await.unwrap().unwrap();
        assert_eq!(
            second_query.principal_id(),
            PrincipalId::from_bytes([2; 16])
        );
        drop(second_query);
        assert_eq!(coordinator.observation().unwrap().active_work, 0);
    }

    #[tokio::test]
    async fn execution_timeout_and_inflight_cancellation_drop_actual_future() {
        let coordinator = coordinator(policy(1, 0, 2, 15));
        let permit = coordinator
            .admit(request(
                1,
                EpochWorkClass::InteractiveQuery,
                Cancellation::default(),
            ))
            .await
            .unwrap();
        assert!(matches!(
            permit.run(tokio::time::sleep(Duration::from_secs(1))).await,
            Err(EpochResourceError::DeadlineExceeded { .. })
        ));
        drop(permit);

        let cancellation = Cancellation::with_check_interval(1);
        let permit = coordinator
            .admit(request(
                2,
                EpochWorkClass::InteractiveQuery,
                cancellation.clone(),
            ))
            .await
            .unwrap();
        cancellation.cancel();
        assert!(matches!(
            permit.run(tokio::time::sleep(Duration::from_secs(1))).await,
            Err(EpochResourceError::Cancelled)
        ));
        drop(permit);
        assert_eq!(coordinator.observation().unwrap().active_work, 0);
    }
}
