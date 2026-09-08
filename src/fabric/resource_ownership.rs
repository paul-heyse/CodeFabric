//! One workspace-native resource domain shared by candidate, current, and leased epochs.
//!
//! Runtime copies may install separate authorized object-store registries and session catalogs,
//! but share pool, disk manager, and native caches. Native operator reservations join the neutral
//! process ledger. DataFusion's infallible growth remains infallible and visibly overdrawn; its
//! pool is not an RSS cap, a provider allocator, or an OS containment mechanism.

use std::fmt::{Display, Formatter};
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, OnceLock};

use datafusion::common::DataFusionError;
use datafusion::execution::cache::cache_manager::CacheManager;
use datafusion::execution::disk_manager::DiskManager;
use datafusion::execution::memory_pool::{
    FairSpillPool, MemoryConsumer, MemoryLimit, MemoryPool, MemoryReservation, TrackConsumersPool,
};
use datafusion::execution::object_store::ObjectStoreRegistry;
use datafusion::execution::runtime_env::{RuntimeEnv, RuntimeEnvBuilder};
use object_store::ObjectStore;
use thiserror::Error;
use url::Url;

use crate::resource_budget::{
    NativeMemoryAccount, ResourceAmounts, ResourceBudget, ResourceBudgetError, ResourceClass,
    ResourceObservation, ResourceReservation, ResourceScopeKind,
};

use super::datafusion_cache::DataFusionCachePolicy;

/// Release-owned numeric settings only. No second full-budget epoch default is supplied here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFabricResourceConfig {
    pub memory_limit_bytes: usize,
    pub max_spill_bytes: u64,
    pub max_spill_merge_fan_in: usize,
    pub tracked_consumer_count: NonZeroUsize,
    pub cache_policy: DataFusionCachePolicy,
}

#[derive(Debug, Error)]
pub enum FabricResourceOwnershipError {
    #[error(transparent)]
    Budget(#[from] ResourceBudgetError),
    #[error(transparent)]
    DataFusion(#[from] DataFusionError),
    #[error("invalid native resource configuration: {0}")]
    Invalid(&'static str),
}

/// Separates admitted capacity from measured native reservations and spill consumption.
#[derive(Clone, Debug)]
pub struct FabricResourceObservation {
    pub process: ResourceObservation,
    pub workspace: ResourceObservation,
    pub native_reserved_bytes: usize,
    pub native_limit_bytes: usize,
    pub native_overdraft_bytes: usize,
    pub spill_used_bytes: u64,
    pub spill_reserved_capacity_bytes: u64,
    pub cache_reserved_capacity_bytes: u64,
}

/// The one injectable native owner. Cloning it or deriving an authorized runtime does not mint
/// any capacity. A derived runtime retains the pool's backing guards even after this handle drops.
#[derive(Clone, Debug)]
pub struct WorkspaceFabricResources {
    workspace: ResourceBudget,
    config: NativeFabricResourceConfig,
    runtime: Arc<RuntimeEnv>,
    cache_capacity_bytes: u64,
    domain: Arc<NativeResourceDomain>,
}

impl WorkspaceFabricResources {
    /// Construct one bounded domain, reserving cache and disk capacity before native construction.
    /// Native operator memory remains dynamic; it competes with application reservations at the
    /// same workspace/process root. Cache/disk capacity is charged once, not per epoch or entry.
    ///
    /// # Errors
    /// Rejects a non-workspace owner, zero/native-overlarge bounds, or unavailable capacity.
    pub fn try_new(
        workspace: ResourceBudget,
        config: NativeFabricResourceConfig,
        object_stores: Arc<dyn ObjectStoreRegistry>,
    ) -> Result<Self, FabricResourceOwnershipError> {
        if workspace.owner().kind != ResourceScopeKind::Workspace {
            return Err(FabricResourceOwnershipError::Invalid(
                "native resources require a workspace scope",
            ));
        }
        if config.memory_limit_bytes == 0
            || config.max_spill_bytes == 0
            || config.max_spill_merge_fan_in == 0
        {
            return Err(FabricResourceOwnershipError::Invalid(
                "native memory, disk and merge bounds must be positive",
            ));
        }
        let native_memory =
            u64::try_from(config.memory_limit_bytes).map_err(|_| ResourceBudgetError::Overflow)?;
        if native_memory > workspace.policy().limits.memory_bytes {
            return Err(FabricResourceOwnershipError::Invalid(
                "native pool exceeds workspace memory ceiling",
            ));
        }
        let control_memory = usize::try_from(workspace.policy().control_reserve.memory_bytes)
            .map_err(|_| ResourceBudgetError::Overflow)?;
        if config.memory_limit_bytes <= control_memory {
            return Err(FabricResourceOwnershipError::Invalid(
                "native pool must preserve configured control headroom",
            ));
        }
        let cache_capacity_bytes = [
            config.cache_policy.metadata_cache_bytes(),
            config.cache_policy.file_statistics_cache_bytes(),
            config.cache_policy.object_list_cache_bytes(),
        ]
        .into_iter()
        .try_fold(0_u64, |sum, bytes| {
            sum.checked_add(u64::try_from(bytes).map_err(|_| ResourceBudgetError::Overflow)?)
                .ok_or(ResourceBudgetError::Overflow)
        })?;
        // These are reserved capacities, not claimed measurements of live cache/disk usage.
        let capacities = workspace.try_reserve(
            ResourceClass::Data,
            ResourceAmounts {
                memory_bytes: cache_capacity_bytes,
                disk_bytes: config.max_spill_bytes,
                ..ResourceAmounts::default()
            },
        )?;
        let domain = Arc::new(NativeResourceDomain {
            native: FairSpillPool::new(config.memory_limit_bytes),
            limit_bytes: config.memory_limit_bytes,
            data_limit_bytes: config.memory_limit_bytes - control_memory,
            accounts: Mutex::new(NativeAccounts {
                data: workspace.native_memory_account(ResourceClass::Data),
                control: workspace.native_memory_account(ResourceClass::Control),
                data_bytes: 0,
            }),
            backing: OnceLock::new(),
            _capacities: capacities,
        });
        let pool = domain.pool(ResourceClass::Data, config.tracked_consumer_count);
        let runtime = config
            .cache_policy
            .configure_runtime(RuntimeEnvBuilder::new())
            .with_memory_pool(pool)
            .with_max_temp_directory_size(config.max_spill_bytes)
            .with_max_spill_merge_fan_in(config.max_spill_merge_fan_in)
            .with_object_store_registry(object_stores)
            .build_arc()?;
        // RuntimeEnv drops its pool field before disk/cache fields. Keeping native backing here
        // ensures the capacity guard cannot release first when the final runtime disappears.
        let _ = domain.backing.set(NativeBacking {
            _disk: Arc::clone(&runtime.disk_manager),
            _cache: Arc::clone(&runtime.cache_manager),
        });
        Ok(Self {
            workspace,
            config,
            runtime,
            cache_capacity_bytes,
            domain,
        })
    }

    #[must_use]
    pub fn workspace_budget(&self) -> &ResourceBudget {
        &self.workspace
    }

    /// Exact numeric policy, suitable for the caller's released policy identity construction.
    #[must_use]
    pub fn config(&self) -> &NativeFabricResourceConfig {
        &self.config
    }

    #[must_use]
    pub fn runtime_env(&self) -> Arc<RuntimeEnv> {
        Arc::clone(&self.runtime)
    }

    /// Share native capacities, while giving this epoch/query only its authorized store registry.
    /// This does not share mutable session state or catalogs.
    ///
    /// # Errors
    /// Returns a native runtime construction error.
    pub fn runtime_with_registry(
        &self,
        registry: Arc<dyn ObjectStoreRegistry>,
    ) -> Result<Arc<RuntimeEnv>, DataFusionError> {
        let mut runtime = RuntimeEnvBuilder::from_runtime_env(&self.runtime)
            .with_object_store_registry(registry)
            .build()?;
        // The native builder shares cache entries but constructs a new manager. Keep the exact
        // workspace manager as well, so every runtime has one observable cache domain.
        runtime.cache_manager = Arc::clone(&self.runtime.cache_manager);
        Ok(Arc::new(runtime))
    }

    /// Use reserved control capacity with the same native pool, disk, caches and root ledger.
    /// Only the caller authorized for control/status/release work may select this runtime.
    ///
    /// # Errors
    /// Returns a native runtime construction error.
    pub fn control_runtime_with_registry(
        &self,
        registry: Arc<dyn ObjectStoreRegistry>,
    ) -> Result<Arc<RuntimeEnv>, DataFusionError> {
        let mut runtime = RuntimeEnvBuilder::from_runtime_env(&self.runtime)
            .with_memory_pool(
                self.domain
                    .pool(ResourceClass::Control, self.config.tracked_consumer_count),
            )
            .with_object_store_registry(registry)
            .build()?;
        runtime.cache_manager = Arc::clone(&self.runtime.cache_manager);
        Ok(Arc::new(runtime))
    }

    #[must_use]
    pub fn observation(&self) -> FabricResourceObservation {
        let native_reserved_bytes = self.runtime.memory_pool.reserved();
        FabricResourceObservation {
            process: self.workspace.process_observation(),
            workspace: self.workspace.observation(),
            native_reserved_bytes,
            native_limit_bytes: self.config.memory_limit_bytes,
            native_overdraft_bytes: native_reserved_bytes
                .saturating_sub(self.config.memory_limit_bytes),
            spill_used_bytes: self.runtime.disk_manager.used_disk_space(),
            spill_reserved_capacity_bytes: self.config.max_spill_bytes,
            cache_reserved_capacity_bytes: self.cache_capacity_bytes,
        }
    }
}

/// The native fairness implementation remains authoritative for spillable consumer shares.
/// The second, application-owned admission condition enforces aggregate ancestor capacity.
#[derive(Debug)]
struct NativeResourceDomain {
    native: FairSpillPool,
    limit_bytes: usize,
    data_limit_bytes: usize,
    accounts: Mutex<NativeAccounts>,
    backing: OnceLock<NativeBacking>,
    // Runtime clones retain this through their shared pool. Native resource references exported
    // separately from RuntimeEnv must retain their workspace owner as part of their caller contract.
    _capacities: ResourceReservation,
}

#[derive(Debug)]
struct NativeBacking {
    _disk: Arc<DiskManager>,
    _cache: Arc<CacheManager>,
}

impl NativeResourceDomain {
    fn pool(self: &Arc<Self>, class: ResourceClass, tracked: NonZeroUsize) -> Arc<dyn MemoryPool> {
        Arc::new(TrackConsumersPool::new(
            HierarchicalMemoryPool {
                domain: Arc::clone(self),
                class,
            },
            tracked,
        ))
    }
}

#[derive(Debug)]
struct NativeAccounts {
    data: NativeMemoryAccount,
    control: NativeMemoryAccount,
    data_bytes: u128,
}

impl NativeAccounts {
    fn account(&mut self, class: ResourceClass) -> &mut NativeMemoryAccount {
        match class {
            ResourceClass::Data => &mut self.data,
            ResourceClass::Control => &mut self.control,
        }
    }
}

#[derive(Debug)]
struct HierarchicalMemoryPool {
    domain: Arc<NativeResourceDomain>,
    class: ResourceClass,
}

impl Display for HierarchicalMemoryPool {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CodeFabricHierarchy({:?}, {})",
            self.class, self.domain.native
        )
    }
}

impl MemoryPool for HierarchicalMemoryPool {
    fn name(&self) -> &'static str {
        "codefabric_hierarchical_fair_spill"
    }
    fn register(&self, consumer: &MemoryConsumer) {
        self.domain.native.register(consumer);
    }
    fn unregister(&self, consumer: &MemoryConsumer) {
        self.domain.native.unregister(consumer);
    }

    fn grow(&self, reservation: &MemoryReservation, additional: usize) {
        let mut accounts = self
            .domain
            .accounts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Native MemoryPool::grow MUST succeed. Account actual debt; never panic on a budget breach,
        // pretend a rejected allocation succeeded, or silently omit its charge.
        accounts.account(self.class).grow_infallible(additional);
        if self.class == ResourceClass::Data {
            accounts.data_bytes += additional as u128;
        }
        self.domain.native.grow(reservation, additional);
    }

    fn shrink(&self, reservation: &MemoryReservation, shrink: usize) {
        let mut accounts = self
            .domain
            .accounts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.domain.native.shrink(reservation, shrink);
        accounts.account(self.class).shrink(shrink);
        if self.class == ResourceClass::Data {
            accounts.data_bytes -= shrink as u128;
        }
    }

    fn try_grow(
        &self,
        reservation: &MemoryReservation,
        additional: usize,
    ) -> Result<(), DataFusionError> {
        let mut accounts = self
            .domain
            .accounts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // A new spillable consumer can reduce fair shares after older consumers already reserved
        // more than the new share. Preserve an aggregate fallible bound as well as native fairness.
        if self.domain.native.reserved() as u128 + additional as u128
            > self.domain.limit_bytes as u128
        {
            return Err(DataFusionError::ResourcesExhausted(
                "shared native pool capacity exhausted".into(),
            ));
        }
        if self.class == ResourceClass::Data
            && accounts.data_bytes + additional as u128 > self.domain.data_limit_bytes as u128
        {
            return Err(DataFusionError::ResourcesExhausted(
                "native data admission preserves reserved control capacity".into(),
            ));
        }
        accounts
            .account(self.class)
            .try_grow(additional)
            .map_err(|error| DataFusionError::ResourcesExhausted(error.to_string()))?;
        if let Err(error) = self.domain.native.try_grow(reservation, additional) {
            accounts.account(self.class).shrink(additional);
            return Err(error);
        }
        if self.class == ResourceClass::Data {
            accounts.data_bytes += additional as u128;
        }
        Ok(())
    }

    fn reserved(&self) -> usize {
        self.domain.native.reserved()
    }
    fn memory_limit(&self) -> MemoryLimit {
        self.domain.native.memory_limit()
    }
}

#[derive(Debug)]
struct ClosedResourceObjectStoreRegistry;

impl ObjectStoreRegistry for ClosedResourceObjectStoreRegistry {
    fn register_store(
        &self,
        _url: &Url,
        _store: Arc<dyn ObjectStore>,
    ) -> Option<Arc<dyn ObjectStore>> {
        None
    }
    fn get_store(&self, _url: &Url) -> Result<Arc<dyn ObjectStore>, DataFusionError> {
        Err(DataFusionError::Plan(
            "workspace resource owner has no ambient object-store authority".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_budget::{ResourceBudgetPolicy, ResourceUsage};
    use datafusion::execution::session_state::SessionStateBuilder;
    use datafusion::prelude::SessionContext;

    fn policy() -> ResourceBudgetPolicy {
        ResourceBudgetPolicy {
            limits: ResourceAmounts {
                memory_bytes: 256,
                disk_bytes: 1_024,
                running_jobs: 8,
                queued_jobs: 16,
                retained_generations: 4,
                retained_bytes: 1_024,
                rows: 1_024,
                pages: 16,
            },
            control_reserve: ResourceAmounts {
                memory_bytes: 16,
                running_jobs: 1,
                queued_jobs: 1,
                ..ResourceAmounts::default()
            },
        }
    }

    fn config() -> NativeFabricResourceConfig {
        NativeFabricResourceConfig {
            memory_limit_bytes: 128,
            max_spill_bytes: 128,
            max_spill_merge_fan_in: 2,
            tracked_consumer_count: NonZeroUsize::new(4).unwrap(),
            cache_policy: DataFusionCachePolicy::try_new_with_entry_limits(
                1, 1, 1, 1, 1, 1, 1, 1, 1,
            )
            .unwrap(),
        }
    }

    fn resources() -> (ResourceBudget, WorkspaceFabricResources) {
        let root = ResourceBudget::try_process([1; 16], policy()).unwrap();
        let workspace = root.workspace([2; 16], policy()).unwrap();
        (
            root,
            WorkspaceFabricResources::try_new(
                workspace,
                config(),
                Arc::new(ClosedResourceObjectStoreRegistry),
            )
            .unwrap(),
        )
    }

    #[test]
    fn rt_cpg_wp79_native_epochs_share_pool_disk_cache_but_not_authority() {
        let (root, resources) = resources();
        let candidate = resources
            .runtime_with_registry(Arc::new(ClosedResourceObjectStoreRegistry))
            .unwrap();
        let current = resources
            .runtime_with_registry(Arc::new(ClosedResourceObjectStoreRegistry))
            .unwrap();
        assert!(Arc::ptr_eq(&candidate.memory_pool, &current.memory_pool));
        assert!(Arc::ptr_eq(&candidate.disk_manager, &current.disk_manager));
        assert!(Arc::ptr_eq(
            &candidate.cache_manager,
            &current.cache_manager
        ));
        assert!(!Arc::ptr_eq(
            &candidate.object_store_registry,
            &current.object_store_registry
        ));
        let first = SessionContext::from(
            SessionStateBuilder::new()
                .with_runtime_env(candidate)
                .with_default_features()
                .build(),
        );
        let second = SessionContext::from(
            SessionStateBuilder::new()
                .with_runtime_env(current)
                .with_default_features()
                .build(),
        );
        let current_state =
            MemoryConsumer::new("current epoch").register(&first.runtime_env().memory_pool);
        let candidate_state =
            MemoryConsumer::new("candidate epoch").register(&second.runtime_env().memory_pool);
        current_state.try_grow(60).unwrap();
        candidate_state.try_grow(52).unwrap();
        assert!(candidate_state.try_grow(1).is_err());
        assert_eq!(resources.observation().native_reserved_bytes, 112);
        assert_eq!(root.observation().used.memory_bytes, 115); // one three-byte native cache envelope
        drop(current_state);
        candidate_state.try_grow(60).unwrap();
        drop(candidate_state);
        drop(first);
        drop(second);
        drop(resources);
        assert_eq!(root.observation().used, ResourceUsage::default());
    }

    #[test]
    fn rt_cpg_wp79_native_control_uses_reserved_headroom_in_the_same_pool() {
        let (root, resources) = resources();
        let data = resources.runtime_env();
        let control = resources
            .control_runtime_with_registry(Arc::new(ClosedResourceObjectStoreRegistry))
            .unwrap();
        assert!(Arc::ptr_eq(&data.disk_manager, &control.disk_manager));
        assert!(Arc::ptr_eq(&data.cache_manager, &control.cache_manager));
        let heavy = MemoryConsumer::new("heavy data")
            .with_can_spill(true)
            .register(&data.memory_pool);
        heavy.try_grow(112).unwrap();
        // Late registration changes native fair shares but must not create total pool capacity.
        let cancel = MemoryConsumer::new("activation control")
            .with_can_spill(true)
            .register(&control.memory_pool);
        assert!(heavy.try_grow(1).is_err());
        cancel.try_grow(16).unwrap();
        assert_eq!(data.memory_pool.reserved(), 128);
        assert_eq!(control.memory_pool.reserved(), 128);
        assert_eq!(root.observation().data_used.memory_bytes, 115);
        assert_eq!(root.observation().used.memory_bytes, 131);
        assert!(cancel.try_grow(1).is_err());
        assert_eq!(root.observation().used.memory_bytes, 131); // native rejection rolled back ledger
        drop(heavy);
        drop(cancel);
        drop(data);
        drop(control);
        drop(resources);
        assert_eq!(root.observation().used, ResourceUsage::default());
    }

    #[test]
    fn rt_cpg_wp79_native_and_application_allocations_compete_at_parent() {
        let (root, resources) = resources();
        let sibling = root.workspace([3; 16], policy()).unwrap();
        let external = sibling
            .try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: 150,
                    ..ResourceAmounts::default()
                },
            )
            .unwrap();
        let runtime = resources.runtime_env();
        let native = MemoryConsumer::new("native join").register(&runtime.memory_pool);
        native.try_grow(87).unwrap();
        assert!(native.try_grow(1).is_err());
        assert_eq!(root.observation().used.memory_bytes, 240);
        drop(external);
        native.try_grow(25).unwrap();
        assert_eq!(root.observation().used.memory_bytes, 115);
        drop(native);
        drop(resources);
        assert_eq!(root.observation().used.memory_bytes, 3); // escaped runtime retains its cache guard
        assert_eq!(root.observation().used.disk_bytes, 128);
        drop(runtime);
        assert_eq!(root.observation().used, ResourceUsage::default());
    }

    #[test]
    fn rt_cpg_wp79_native_infallible_grow_is_accounted_overdraft_not_hard_limit() {
        // Independent probe of the exact pinned library contract, not merely our wrapper.
        let pool: Arc<dyn MemoryPool> = Arc::new(FairSpillPool::new(8));
        let native = MemoryConsumer::new("pinned grow probe").register(&pool);
        assert!(native.try_grow(9).is_err());
        native.grow(9);
        assert_eq!(pool.reserved(), 9);
        drop(native);
        assert_eq!(pool.reserved(), 0);

        let (root, resources) = resources();
        let runtime = resources.runtime_env();
        let native = MemoryConsumer::new("infallible native path").register(&runtime.memory_pool);
        native.grow(300); // must not panic or pretend the finite limit rejected this allocation
        assert_eq!(resources.observation().native_overdraft_bytes, 172);
        assert_eq!(root.observation().used.memory_bytes, 303);
        assert_eq!(root.observation().exceeded.memory_bytes, 47);
        assert!(
            resources
                .workspace_budget()
                .try_reserve(
                    ResourceClass::Data,
                    ResourceAmounts {
                        memory_bytes: 1,
                        ..ResourceAmounts::default()
                    }
                )
                .is_err()
        );
        native.shrink(300);
        assert_eq!(root.observation().exceeded.memory_bytes, 0);
        native.try_grow(1).unwrap();
        drop(native);
        drop(runtime);
        drop(resources);
        assert_eq!(root.observation().used, ResourceUsage::default());
    }

    #[test]
    fn rt_cpg_wp79_native_fair_spill_and_invalid_owner_are_preserved() {
        let (root, resources) = resources();
        assert!(
            WorkspaceFabricResources::try_new(
                root.clone(),
                config(),
                Arc::new(ClosedResourceObjectStoreRegistry)
            )
            .is_err()
        );
        let runtime = resources.runtime_env();
        let first = MemoryConsumer::new("spill first")
            .with_can_spill(true)
            .register(&runtime.memory_pool);
        let second = MemoryConsumer::new("spill second")
            .with_can_spill(true)
            .register(&runtime.memory_pool);
        assert!(first.try_grow(65).is_err()); // native fair share is 128/2, even with parent headroom
        assert_eq!(root.observation().used.memory_bytes, 3);
        first.try_grow(64).unwrap();
        second.try_grow(48).unwrap();
        assert!(second.try_grow(1).is_err());
        drop(first);
        drop(second);
        drop(runtime);
        drop(resources);
        assert_eq!(root.observation().used, ResourceUsage::default());
    }
}
