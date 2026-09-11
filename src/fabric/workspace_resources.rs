//! One process/workspace resource lineage established before production source or epoch work.
//!
//! The initial finite profile is admission policy, not a measured optimal tuning result (WP103).
//! Native pool reservations, application allocations, retained generations, and durable result
//! capacities all compete at this owner. It does not pretend that allocator RSS is instrumented.

use std::num::{NonZeroU64, NonZeroUsize};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::child_session::resource_governance::WorkspaceResourceCoordinator;
use super::command::{ResourceEnvelopeRef, WorkspaceId};
use super::native_execution_lane::{NativeAdmissionFailure, NativeLaneEnvelope, NativeLaneError};
use super::owned_local_store::{OwnedLocalStore, OwnedLocalStoreError, OwnedLocalStoreLimits};
use super::programmatic_active_workspace_builder::ProductionActiveWorkspaceConfig;
use super::resource_ownership::WorkspaceFabricResources;
use super::workspace_native_execution::{WorkspaceNativeExecution, WorkspaceNativeProfile};
use crate::resource_budget::{
    ChargedValue, ResourceAmounts, ResourceBudget, ResourceBudgetPolicy, ResourceClass,
};

#[derive(Clone)]
pub(crate) struct ProductionWorkspaceResources {
    budget: ResourceBudget,
    operational_writer: Arc<std::sync::Mutex<()>>,
    syntax_cache:
        Arc<std::sync::Mutex<super::production_workspace_startup::syntax_cache::SyntaxCache>>,
    pyrefly_cache:
        Arc<std::sync::Mutex<super::production_workspace_startup::pyrefly_cache::PyreflyCache>>,
    rust_toolchain_cache: Arc<
        std::sync::Mutex<
            super::production_workspace_startup::rustc::toolchain_cache::ToolchainCache,
        >,
    >,
    rust_unit_graph_cache:
        Arc<std::sync::Mutex<super::production_workspace_startup::rustc::unit_graph::Cache>>,
    native: WorkspaceFabricResources,
    scheduler: WorkspaceResourceCoordinator,
    config: ProductionActiveWorkspaceConfig,
    policy_ref: ResourceEnvelopeRef,
    disk_headroom: crate::disk_headroom::LocalDiskHeadroom,
    source_blob_disk: crate::source_image::SourceBlobDiskLedger,
    local_store: Arc<OwnedLocalStore>,
    local_store_state_root: ChargedValue<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum WorkspaceStoreBootstrapError {
    #[error("workspace physical bootstrap was cancelled before starting")]
    Cancelled,
    #[error(transparent)]
    Budget(#[from] crate::resource_budget::ResourceBudgetError),
    #[error(transparent)]
    Task(#[from] crate::cancellation::StructuredTaskError),
    #[error(transparent)]
    Store(#[from] OwnedLocalStoreError),
}

impl ProductionWorkspaceResources {
    /// The daemon has one workspace; accepting an existing process owner also prevents future
    /// multi-workspace attachment from silently multiplying the process ceiling.
    pub(crate) fn try_new(
        process: &ResourceBudget,
        workspace_id: WorkspaceId,
        state_root: &Path,
    ) -> Result<Self, String> {
        let policy = local_resource_policy()
            .validate()
            .map_err(|error| error.to_string())?;
        let disk_headroom = crate::disk_headroom::LocalDiskHeadroom::open(state_root)
            .map_err(|error| error.to_string())?;
        let budget = process
            .workspace(*workspace_id.as_bytes(), policy)
            .map_err(|error| error.to_string())?;
        let source_blob_disk = crate::source_image::SourceBlobDiskLedger::try_new(
            budget.clone(),
            disk_headroom.clone(),
        )
        .map_err(|error| error.to_string())?;
        let config = ProductionActiveWorkspaceConfig::bounded_local_workstation()?;
        if !config
            .resource_policy()
            .matches_native_resources(&config.epoch_runtime().native_config())
        {
            return Err("child and epoch native resource policies differ".to_owned());
        }
        budget
            .reserve_scope_bookkeeping()
            .map_err(|error| error.to_string())?;
        let store_limits = local_store_limits();
        let root_bytes = state_root.as_os_str().as_bytes().len();
        if root_bytes > store_limits.max_path_bytes {
            return Err("workspace local store path exceeds its finite bound".to_owned());
        }
        let root_charge = budget
            .try_reserve(
                ResourceClass::Control,
                ResourceAmounts {
                    memory_bytes: u64::try_from(root_bytes + std::mem::size_of::<PathBuf>())
                        .map_err(|error| error.to_string())?,
                    ..ResourceAmounts::default()
                },
            )
            .map_err(|error| error.to_string())?;
        let local_store_state_root = root_charge.into_charged_value(state_root.to_owned());
        let local_store = Arc::new(
            OwnedLocalStore::try_new(budget.clone(), disk_headroom.clone(), store_limits.clone())
                .map_err(|error| error.to_string())?,
        );
        // Every epoch/child/control runtime resolves the exact same finite physical owner.
        // bootstrap_local_store must finish before Delta work. A failed bootstrap retains
        // its owner here, whereas turning its failure into the constructor's String would
        // release uncertain physical directory charges before retry/reconciliation.
        let native = WorkspaceFabricResources::try_new(
            budget.clone(),
            config.epoch_runtime().native_config(),
            super::programmatic_epoch::local_fabric_object_store_registry(local_store.clone()),
        )
        .map_err(|error| error.to_string())?;
        let policy_ref = resource_policy_identity(policy, &config, &store_limits);
        let scheduler = WorkspaceResourceCoordinator::try_new(
            *policy_ref.as_bytes(),
            config.resource_policy().clone(),
            native.runtime_env(),
            budget.clone(),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            operational_writer: Arc::new(std::sync::Mutex::new(())),
            syntax_cache: Arc::new(std::sync::Mutex::new(
                super::production_workspace_startup::syntax_cache::SyntaxCache::new(budget.clone()),
            )),
            pyrefly_cache: Arc::new(std::sync::Mutex::new(
                super::production_workspace_startup::pyrefly_cache::PyreflyCache::new(
                    budget.clone(),
                ),
            )),
            rust_toolchain_cache: Arc::new(std::sync::Mutex::new(
                super::production_workspace_startup::rustc::toolchain_cache::ToolchainCache::new(
                    budget.clone(),
                ),
            )),
            rust_unit_graph_cache: Arc::new(std::sync::Mutex::new(
                super::production_workspace_startup::rustc::unit_graph::Cache::new(budget.clone()),
            )),
            budget,
            native,
            scheduler,
            config,
            policy_ref,
            disk_headroom,
            source_blob_disk,
            local_store,
            local_store_state_root,
        })
    }

    pub(in crate::fabric) fn syntax_cache(
        &self,
    ) -> &Arc<std::sync::Mutex<super::production_workspace_startup::syntax_cache::SyntaxCache>>
    {
        &self.syntax_cache
    }

    pub(in crate::fabric) fn pyrefly_cache(
        &self,
    ) -> &Arc<std::sync::Mutex<super::production_workspace_startup::pyrefly_cache::PyreflyCache>>
    {
        &self.pyrefly_cache
    }

    /// Immutable deployment captures share one owner across compiler targets and generations.
    pub(in crate::fabric) fn rust_toolchain_cache(
        &self,
    ) -> &Arc<
        std::sync::Mutex<
            super::production_workspace_startup::rustc::toolchain_cache::ToolchainCache,
        >,
    > {
        &self.rust_toolchain_cache
    }

    pub(in crate::fabric) fn rust_unit_graph_cache(
        &self,
    ) -> &Arc<std::sync::Mutex<super::production_workspace_startup::rustc::unit_graph::Cache>> {
        &self.rust_unit_graph_cache
    }

    /// Serializes short operational writes; provider computation never retains this gate.
    pub(crate) fn operational_writer(&self) -> &Arc<std::sync::Mutex<()>> {
        &self.operational_writer
    }

    /// Complete the fixed physical directory bootstrap while retaining this workspace owner
    /// on failure. This synchronous, retryable phase precedes the native Delta operation lane.
    pub(crate) fn bootstrap_local_store(&self) -> Result<(), OwnedLocalStoreError> {
        self.local_store
            .bootstrap_workspace_roots(&self.local_store_state_root, self.budget.owner().id)
    }

    /// Run the bounded synchronous physical bootstrap under joined ownership.
    /// The blocking closure captures shared original roots/store, without a deep
    /// workspace-config copy. Caller cancellation cannot detach its filesystem work.
    pub(crate) async fn bootstrap_local_store_owned(
        &self,
        scope: &crate::cancellation::StructuredCancellationScope,
    ) -> Result<(), WorkspaceStoreBootstrapError> {
        struct BootstrapInput {
            store: Arc<OwnedLocalStore>,
            root: ChargedValue<PathBuf>,
            workspace: [u8; 16],
        }
        let reservation = self.budget.try_reserve(
            ResourceClass::Control,
            ResourceAmounts {
                memory_bytes: std::mem::size_of::<BootstrapInput>() as u64,
                running_jobs: 1,
                ..ResourceAmounts::default()
            },
        )?;
        let input = BootstrapInput {
            store: Arc::clone(&self.local_store),
            root: self.local_store_state_root.clone(),
            workspace: self.budget.owner().id,
        };
        let bootstrap_scope = scope.child_control("physical-store-bootstrap")?;
        let operation = bootstrap_scope
            .spawn_blocking_owned("directories", reservation, move |cancellation| {
                if cancellation.is_cancelled() {
                    return Err(WorkspaceStoreBootstrapError::Cancelled);
                }
                input
                    .store
                    .bootstrap_workspace_roots(&input.root, input.workspace)
                    .map_err(WorkspaceStoreBootstrapError::Store)
            })
            .await?;
        operation.wait().await?
    }

    pub(crate) fn local_store(&self) -> &Arc<OwnedLocalStore> {
        &self.local_store
    }

    /// Bind native phases to the caller's workspace lifecycle after physical bootstrap.
    /// Clone the returned executor to share its operation identities and data/control scopes.
    pub(crate) fn native_execution(
        &self,
        scope: &crate::cancellation::StructuredCancellationScope,
    ) -> Result<WorkspaceNativeExecution, NativeLaneError> {
        let observed = self
            .local_store
            .observe()
            .map_err(WorkspaceNativeExecution::store_admission_failure)?;
        if !observed.ready || observed.bootstrap_pending || observed.roots != 2 {
            return Err(NativeAdmissionFailure::StoreNotReconciled.into());
        }
        WorkspaceNativeExecution::try_new(
            self.budget.clone(),
            scope,
            self.local_store.clone(),
            local_native_profile(ResourceClass::Data),
            local_native_profile(ResourceClass::Control),
        )
    }

    pub(crate) fn budget(&self) -> &ResourceBudget {
        &self.budget
    }
    pub(crate) fn disk_headroom(&self) -> &crate::disk_headroom::LocalDiskHeadroom {
        &self.disk_headroom
    }
    pub(crate) fn source_blob_disk(&self) -> &crate::source_image::SourceBlobDiskLedger {
        &self.source_blob_disk
    }
    pub(crate) fn native(&self) -> &WorkspaceFabricResources {
        &self.native
    }
    pub(crate) fn scheduler(&self) -> &WorkspaceResourceCoordinator {
        &self.scheduler
    }
    pub(crate) fn config(&self) -> &ProductionActiveWorkspaceConfig {
        &self.config
    }
    pub(crate) const fn policy_ref(&self) -> ResourceEnvelopeRef {
        self.policy_ref
    }
}

/// Selected large-workstation envelope. These are ceilings, not eager allocations.
/// Retained bytes and disk are independent of live
/// memory, while every in-memory retained object must also carry a memory reservation.
pub(crate) const fn local_resource_policy() -> ResourceBudgetPolicy {
    ResourceBudgetPolicy {
        limits: ResourceAmounts {
            memory_bytes: 64 * 1024 * 1024 * 1024,
            // The shared 64 GiB spill reservation must leave room for source,
            // durable tables, and control records under this same owner.
            disk_bytes: 128 * 1024 * 1024 * 1024,
            running_jobs: 32,
            queued_jobs: 512,
            // Includes epoch generations, parser owners/revisions and native process owners.
            // A serial TS/Ruff runner can peak at six; this is not an epoch-count-only limit.
            retained_generations: 128,
            retained_bytes: 48 * 1024 * 1024 * 1024,
            rows: 200_000_000,
            pages: 1_048_576,
        },
        control_reserve: ResourceAmounts {
            memory_bytes: 256 * 1024 * 1024,
            disk_bytes: 64 * 1024 * 1024,
            running_jobs: 4,
            queued_jobs: 16,
            retained_generations: 1,
            retained_bytes: 64 * 1024 * 1024,
            rows: 100_000,
            pages: 1024,
        },
    }
}

/// Initial finite native I/O profile. These are checked admission bounds, with calibration
/// still owned by WP103. Paths name only two roots; retained files share the common disk budget.
pub(crate) const fn local_store_limits() -> OwnedLocalStoreLimits {
    OwnedLocalStoreLimits {
        max_roots: 2,
        max_object_bytes: 256 * 1024 * 1024,
        max_read_bytes: 256 * 1024 * 1024,
        max_path_bytes: 4096,
        max_list_entries: 65_536,
        max_directory_depth: 32,
        max_pending_operations: 4096,
        max_multipart_parts: 1024,
    }
}

/// Finite native worker and job envelope. Native allocator RSS is outside this reservation;
/// this reserves scheduling capacity, not a receipt for every library allocation.
/// Control uses an independent runtime so data pressure cannot consume its reserve.
pub(crate) fn local_native_profile(class: ResourceClass) -> WorkspaceNativeProfile {
    let (workers, tasks) = match class {
        ResourceClass::Data => (16, 4096),
        ResourceClass::Control => (1, 128),
    };
    WorkspaceNativeProfile {
        lane: NativeLaneEnvelope {
            worker_threads: NonZeroUsize::new(workers).expect("finite workers"),
            blocking_threads: NonZeroUsize::new(workers * 4).expect("nested blocking headroom"),
            thread_stack_bytes: NonZeroUsize::new(2 * 1024 * 1024).unwrap(),
            runtime_memory_bytes: NonZeroU64::new(1024 * 1024).unwrap(),
            // Shared DataFusion pools and bounded store reads govern their own buffers.
            native_buffer_bytes: 0,
            native_task_slots: NonZeroU64::new(tasks).unwrap(),
            task_memory_bytes: NonZeroU64::new(512).unwrap(),
            parallel_blocking_roots: NonZeroUsize::new(workers).unwrap(),
            blocking_nesting: NonZeroUsize::new(2).unwrap(),
        },
    }
}

fn resource_policy_identity(
    policy: ResourceBudgetPolicy,
    config: &ProductionActiveWorkspaceConfig,
    store: &OwnedLocalStoreLimits,
) -> ResourceEnvelopeRef {
    let mut hash = blake3::Hasher::new();
    hash.update(b"codefabric.aggregate-resource-envelope.v1\0");
    hash.update(b"owned-local-store.workspace-bootstrap.v1\0");
    hash.update(b"workspace-native-execution.bounded-owned.v2\0");
    hash.update(&crate::process_memory::RSS_PAUSE_BYTES.to_be_bytes());
    hash.update(&crate::process_memory::RSS_RESUME_BYTES.to_be_bytes());
    for class in [ResourceClass::Data, ResourceClass::Control] {
        let profile = local_native_profile(class);
        let lane = profile.lane;
        for value in [
            lane.worker_threads.get() as u64,
            lane.blocking_threads.get() as u64,
            lane.thread_stack_bytes.get() as u64,
            lane.runtime_memory_bytes.get(),
            lane.native_buffer_bytes,
            lane.native_task_slots.get(),
            lane.task_memory_bytes.get(),
            lane.parallel_blocking_roots.get() as u64,
            lane.blocking_nesting.get() as u64,
        ] {
            hash.update(&value.to_be_bytes());
        }
    }
    for value in [
        store.max_roots as u64,
        store.max_object_bytes,
        store.max_read_bytes,
        store.max_path_bytes as u64,
        store.max_list_entries as u64,
        store.max_directory_depth as u64,
        store.max_pending_operations as u64,
        store.max_multipart_parts as u64,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&crate::disk_headroom::DATA_HEADROOM_BYTES.to_be_bytes());
    hash.update(&crate::disk_headroom::CLEANUP_HEADROOM_BYTES.to_be_bytes());
    hash.update(&(super::native_operations::MAX_NATIVE_TASKS as u64).to_be_bytes());
    hash.update(&super::native_operations::NATIVE_BOOKKEEPING_BYTES.to_be_bytes());
    hash.update(
        &super::native_operations::CLEANUP_RESERVE
            .as_millis()
            .to_be_bytes(),
    );
    for amounts in [policy.limits, policy.control_reserve] {
        for value in [
            amounts.memory_bytes,
            amounts.disk_bytes,
            amounts.running_jobs,
            amounts.queued_jobs,
            amounts.retained_generations,
            amounts.retained_bytes,
            amounts.rows,
            amounts.pages,
        ] {
            hash.update(&value.to_be_bytes());
        }
    }
    hash.update(&config.resource_policy().identity());
    let runtime = config.epoch_runtime().identity();
    hash.update(&(runtime.len() as u64).to_be_bytes());
    hash.update(runtime.as_bytes());
    ResourceEnvelopeRef::from_bytes(*hash.finalize().as_bytes())
}

#[cfg(test)]
pub(crate) fn test_workspace_budget() -> ResourceBudget {
    let process = ResourceBudget::try_process([101; 16], local_resource_policy()).unwrap();
    process
        .workspace([0x22; 16], local_resource_policy())
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_identity_binds_every_local_store_limit() {
        let config = ProductionActiveWorkspaceConfig::bounded_local_workstation().unwrap();
        let policy = local_resource_policy();
        let original = resource_policy_identity(policy, &config, &local_store_limits());
        for dimension in 0..8 {
            let mut changed = local_store_limits();
            match dimension {
                0 => changed.max_roots += 1,
                1 => changed.max_object_bytes += 1,
                2 => changed.max_read_bytes += 1,
                3 => changed.max_path_bytes += 1,
                4 => changed.max_list_entries += 1,
                5 => changed.max_directory_depth += 1,
                6 => changed.max_pending_operations += 1,
                _ => changed.max_multipart_parts += 1,
            }
            assert_ne!(
                original,
                resource_policy_identity(policy, &config, &changed)
            );
        }
    }

    #[test]
    fn resource_identity_binds_numeric_envelope_not_workspace_label() {
        let config = ProductionActiveWorkspaceConfig::bounded_local_workstation().unwrap();
        let policy = local_resource_policy();
        let original = resource_policy_identity(policy, &config, &local_store_limits());
        let mut changed = policy;
        changed.limits.memory_bytes -= 1;
        assert_ne!(
            original,
            resource_policy_identity(changed, &config, &local_store_limits())
        );
        changed = policy;
        changed.control_reserve.running_jobs += 1;
        assert_ne!(
            original,
            resource_policy_identity(changed, &config, &local_store_limits())
        );
    }
}
