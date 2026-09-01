//! Bounded, reconstructible DataFusion caches for one exact fabric epoch.
//!
//! Runtime file caches are DataFusion-owned accelerators. Logical plans are
//! CodeFabric-owned materializations keyed by the complete semantic program,
//! exact Delta version vector, session authority, runtime identity, and access
//! policy. None of these caches is an authority: dropping the cache changes
//! latency only, and every entry can be rebuilt from the sealed epoch.

use std::collections::HashMap;
use std::fmt::Debug;
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use arrow_schema::{DataType, Field, Schema, SchemaRef};
use datafusion::common::HashMap as DataFusionHashMap;
use datafusion::common::instant::Instant;
use datafusion::common::tree_node::{TreeNode, TreeNodeRecursion};
use datafusion::common::{Constraint, TableReference};
use datafusion::datasource::source_as_provider;
use datafusion::execution::SessionState;
use datafusion::execution::cache::cache_manager::{
    CacheManagerConfig, CachedFileList, CachedFileMetadata, CachedFileMetadataEntry,
    FileMetadataCache, FileStatisticsCache, ListFilesCache, TableScopedPath,
};
use datafusion::execution::cache::{Cache, CacheEntryInfo, CacheKey, CacheValue};
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::logical_expr::{Expr, LogicalPlan, TableScan, WindowFunctionDefinition};
use object_store::path::Path as ObjectStorePath;

use crate::relational_program::{CompilationObservations, RelationalProgram};
use crate::schema_contract::{
    ColumnMappingMode, DeletionVectorBehavior, SchemaCompatibility, SchemaContract,
};

use super::activation::TableVersionSetRef;
use super::command::EpochId;
use super::programmatic_schema::registered_view_logical_plan;

const COMPILED_PLAN_DIGEST_DOMAIN: &[u8] = b"codefabric.logical-plan.compiled.v1";
const OPTIMIZED_PLAN_DIGEST_DOMAIN: &[u8] = b"codefabric.logical-plan.optimized.v1";
const LOGICAL_PLAN_AUTHORITY_DOMAIN: &[u8] = b"codefabric.logical-plan.semantic-authority.v1";
/// Maximum delay before an object listing is refreshed. This bounds cache reuse; it does not
/// establish listing validity or authority.
const OBJECT_LIST_CACHE_MAX_REFRESH_SECONDS: u64 = 30;
const FILE_METADATA_TARGET_BYTES_PER_ENTRY: usize = 64 * 1024;
const FILE_STATISTICS_TARGET_BYTES_PER_ENTRY: usize = 64 * 1024;
const OBJECT_LIST_TARGET_BYTES_PER_ENTRY: usize = 256 * 1024;
const NATIVE_CACHE_MAX_ENTRIES: usize = 4_096;

/// Versioned, execution-local identity of every semantic capability that may affect a cached
/// logical plan.
///
/// The fingerprint is intentionally not durable provenance and never substitutes for the exact
/// authorities framed into it. Capability addresses distinguish concrete in-memory providers and
/// registry implementations conservatively; reconstructing an equivalent capability may miss the
/// cache, but can never reuse a plan bound to a different implementation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct LogicalPlanAuthorityFingerprint([u8; 32]);

impl LogicalPlanAuthorityFingerprint {
    pub(super) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Length-framed constructor for [`LogicalPlanAuthorityFingerprint`].
pub(super) struct LogicalPlanAuthorityBuilder(blake3::Hasher);

impl LogicalPlanAuthorityBuilder {
    pub(super) fn new(authority_kind: &[u8]) -> Self {
        let mut builder = Self(blake3::Hasher::new());
        builder.frame(LOGICAL_PLAN_AUTHORITY_DOMAIN);
        builder.frame(authority_kind);
        builder.frame(env!("CARGO_PKG_VERSION").as_bytes());
        builder.frame(datafusion::DATAFUSION_VERSION.as_bytes());
        builder.frame(arrow::ARROW_VERSION.as_bytes());
        builder.frame(blake3::hash(include_bytes!("../../Cargo.lock")).as_bytes());
        builder
    }

    pub(super) fn frame(&mut self, bytes: &[u8]) {
        self.0.update(&(bytes.len() as u64).to_be_bytes());
        self.0.update(bytes);
    }

    pub(super) fn frame_str(&mut self, value: &str) {
        self.frame(value.as_bytes());
    }

    pub(super) fn frame_usize(&mut self, value: usize) {
        self.frame(&(value as u128).to_be_bytes());
    }

    pub(super) fn frame_u64(&mut self, value: u64) {
        self.frame(&value.to_be_bytes());
    }

    pub(super) fn frame_arc_identity<T: ?Sized>(&mut self, capability: &Arc<T>) {
        let data_address = Arc::as_ptr(capability) as *const () as usize;
        self.frame_usize(data_address);
    }

    pub(super) fn frame_schema(&mut self, schema: &SchemaRef) -> Result<(), String> {
        let canonical = serde_json_canonicalizer::to_vec(schema.as_ref())
            .map_err(|error| format!("canonical Arrow schema: {error}"))?;
        self.frame(&canonical);
        Ok(())
    }

    fn frame_data_type(&mut self, data_type: &DataType) -> Result<(), String> {
        // Arrow's schema serde contract is the pinned, executable datatype encoding. A one-field
        // schema avoids binding cache identity to the non-contractual Rust `Debug` rendering.
        let schema = Arc::new(Schema::new(vec![Field::new(
            "__codefabric_datatype__",
            data_type.clone(),
            true,
        )]));
        self.frame_schema(&schema)
    }

    pub(super) fn finish(self) -> LogicalPlanAuthorityFingerprint {
        LogicalPlanAuthorityFingerprint(*self.0.finalize().as_bytes())
    }
}

/// Frame every executable part of an application-owned schema contract.
pub(super) fn frame_schema_contract(
    builder: &mut LogicalPlanAuthorityBuilder,
    relation_id: &str,
    contract: &SchemaContract,
) -> Result<(), String> {
    builder.frame_str(relation_id);
    builder.frame_str(&contract.qualifier().to_string());
    builder.frame_str(contract.source_schema_identity());
    builder.frame_schema(contract.logical_schema())?;
    builder.frame_schema(contract.storage_schema())?;
    builder.frame_schema(contract.empty_stream_schema())?;
    builder.frame_usize(contract.mappings().len());
    for mapping in contract.mappings() {
        builder.frame_usize(mapping.logical_index());
        builder.frame_usize(mapping.storage_index());
        builder.frame_usize(mapping.projection_index());
        builder.frame_usize(mapping.filter_index());
        builder.frame_usize(mapping.statistics_index());
    }
    builder.frame_usize(contract.casts().len());
    for cast in contract.casts() {
        builder.frame_str(cast.logical_field_id());
        builder.frame_str(cast.storage_field_id());
        builder.frame_usize(cast.logical_index());
        builder.frame_usize(cast.storage_index());
        builder.frame_data_type(cast.logical_data_type())?;
        builder.frame_data_type(cast.storage_data_type())?;
    }
    let constraints = contract
        .constraints()
        .as_ref()
        .clone()
        .into_iter()
        .collect::<Vec<_>>();
    builder.frame_usize(constraints.len());
    for constraint in constraints {
        match constraint {
            Constraint::PrimaryKey(indices) => {
                builder.frame(b"primary-key");
                builder.frame_usize(indices.len());
                for index in indices {
                    builder.frame_usize(index);
                }
            }
            Constraint::Unique(indices) => {
                builder.frame(b"unique");
                builder.frame_usize(indices.len());
                for index in indices {
                    builder.frame_usize(index);
                }
            }
        }
    }
    builder.frame(match contract.compatibility() {
        SchemaCompatibility::Exact => b"exact",
        SchemaCompatibility::Contains => b"contains",
    });
    builder.frame(match contract.column_mapping_mode() {
        ColumnMappingMode::Positional => b"positional",
        ColumnMappingMode::Name => b"name",
        ColumnMappingMode::FieldId => b"field-id",
    });
    builder.frame(match contract.deletion_vector_behavior() {
        DeletionVectorBehavior::Forbidden => b"forbidden",
        DeletionVectorBehavior::AppliedByProvider => b"applied-by-provider",
        DeletionVectorBehavior::ExposedVisibilityColumn => b"exposed-visibility-column",
    });
    Ok(())
}

/// Frame DataFusion's actual logical-planning configuration and installed function capabilities.
pub(super) fn frame_session_logical_authority(
    builder: &mut LogicalPlanAuthorityBuilder,
    state: &SessionState,
    query_planner_identity: &str,
) {
    builder.frame_str(query_planner_identity);

    let mut entries = state.config_options().entries();
    entries.sort_by(|left, right| left.key.cmp(&right.key));
    builder.frame_usize(entries.len());
    for entry in entries {
        builder.frame_str(&entry.key);
        match entry.value {
            Some(value) => {
                builder.frame(&[1]);
                builder.frame_str(&value);
            }
            None => builder.frame(&[0]),
        }
    }

    // Analyzer, optimizer, and expression/relation planner sets are selected only by the
    // compiled release. Their ordered names/counts plus the pinned crate and lock identities
    // frame their semantics. Heap addresses would turn equivalent fresh child sessions into
    // accidental cache misses. Function registries below are operational capabilities and retain
    // exact Arc identity because policy may install a same-named replacement implementation.
    builder.frame_usize(state.analyzer().function_rewrites().len());
    for rewrite in state.analyzer().function_rewrites() {
        builder.frame_str(rewrite.name());
    }
    builder.frame_usize(state.analyzer().rules.len());
    for rule in &state.analyzer().rules {
        builder.frame_str(rule.name());
    }
    builder.frame_usize(state.optimizers().len());
    for rule in state.optimizers() {
        builder.frame_str(rule.name());
    }
    builder.frame_usize(state.expr_planners().len());
    builder.frame_usize(state.relation_planners().len());

    let mut scalar = state.scalar_functions().iter().collect::<Vec<_>>();
    scalar.sort_by(|(left, _), (right, _)| left.cmp(right));
    builder.frame_usize(scalar.len());
    for (installed_name, function) in scalar {
        builder.frame_str(installed_name);
        builder.frame_str(function.name());
        let mut aliases = function.aliases().to_vec();
        aliases.sort();
        builder.frame_usize(aliases.len());
        for alias in aliases {
            builder.frame_str(&alias);
        }
        builder.frame_arc_identity(function);
    }

    let mut aggregate = state.aggregate_functions().iter().collect::<Vec<_>>();
    aggregate.sort_by(|(left, _), (right, _)| left.cmp(right));
    builder.frame_usize(aggregate.len());
    for (installed_name, function) in aggregate {
        builder.frame_str(installed_name);
        builder.frame_str(function.name());
        let mut aliases = function.aliases().to_vec();
        aliases.sort();
        builder.frame_usize(aliases.len());
        for alias in aliases {
            builder.frame_str(&alias);
        }
        builder.frame_arc_identity(function);
    }

    let mut window = state.window_functions().iter().collect::<Vec<_>>();
    window.sort_by(|(left, _), (right, _)| left.cmp(right));
    builder.frame_usize(window.len());
    for (installed_name, function) in window {
        builder.frame_str(installed_name);
        builder.frame_str(function.name());
        let mut aliases = function.aliases().to_vec();
        aliases.sort();
        builder.frame_usize(aliases.len());
        for alias in aliases {
            builder.frame_str(&alias);
        }
        builder.frame_arc_identity(function);
    }
}

/// Revalidate every table and function capability retained by a cached plan, including plans
/// nested in subquery expressions.
pub(super) fn validate_logical_plan_references(
    plan: &LogicalPlan,
    state: &SessionState,
    allow_relational_compiler_aggregates: bool,
    mut validate_scan: impl FnMut(&TableScan) -> Result<(), String>,
) -> Result<(), String> {
    let mut drift = None;
    plan.apply_with_subqueries(|node| {
        if drift.is_some() {
            return Ok(TreeNodeRecursion::Stop);
        }
        if let LogicalPlan::TableScan(scan) = node
            && let Err(error) = validate_scan(scan)
        {
            drift = Some(error);
            return Ok(TreeNodeRecursion::Stop);
        }
        if let LogicalPlan::Extension(extension) = node {
            drift = Some(format!(
                "cached logical plan retains unauthorized extension capability {}",
                extension.node.name()
            ));
            return Ok(TreeNodeRecursion::Stop);
        }
        for expression in node.expressions() {
            if let Some(error) =
                expression_function_drift(&expression, state, allow_relational_compiler_aggregates)?
            {
                drift = Some(error);
                return Ok(TreeNodeRecursion::Stop);
            }
        }
        Ok(TreeNodeRecursion::Continue)
    })
    .map_err(|error| format!("cached logical-plan reference traversal failed: {error}"))?;
    drift.map_or(Ok(()), Err)
}

fn expression_function_drift(
    expression: &Expr,
    state: &SessionState,
    allow_relational_compiler_aggregates: bool,
) -> datafusion::common::Result<Option<String>> {
    let mut drift = None;
    expression.apply(|candidate| {
        let invalid = match candidate {
            Expr::ScalarFunction(function) => (!state
                .scalar_functions()
                .values()
                .any(|installed| Arc::ptr_eq(installed, &function.func)))
            .then(|| {
                format!(
                    "cached scalar function {} retains an uninstalled capability",
                    function.func.name()
                )
            }),
            Expr::AggregateFunction(function) => (!aggregate_capability_is_authorized(
                &function.func,
                state,
                allow_relational_compiler_aggregates,
            ))
            .then(|| {
                format!(
                    "cached aggregate function {} retains an uninstalled capability",
                    function.func.name()
                )
            }),
            Expr::WindowFunction(function) => match &function.fun {
                WindowFunctionDefinition::AggregateUDF(aggregate) => {
                    (!aggregate_capability_is_authorized(
                        aggregate,
                        state,
                        allow_relational_compiler_aggregates,
                    ))
                    .then(|| {
                        format!(
                            "cached aggregate window function {} retains an uninstalled capability",
                            aggregate.name()
                        )
                    })
                }
                WindowFunctionDefinition::WindowUDF(window) => (!state
                    .window_functions()
                    .values()
                    .any(|installed| Arc::ptr_eq(installed, window)))
                .then(|| {
                    format!(
                        "cached window function {} retains an uninstalled capability",
                        window.name()
                    )
                }),
            },
            _ => None,
        };
        if invalid.is_some() {
            drift = invalid;
            Ok(TreeNodeRecursion::Stop)
        } else {
            Ok(TreeNodeRecursion::Continue)
        }
    })?;
    Ok(drift)
}

fn aggregate_capability_is_authorized(
    function: &Arc<datafusion::logical_expr::AggregateUDF>,
    state: &SessionState,
    allow_relational_compiler_aggregates: bool,
) -> bool {
    state
        .aggregate_functions()
        .values()
        .any(|installed| Arc::ptr_eq(installed, function))
        || (allow_relational_compiler_aggregates
            && [
                datafusion::functions_aggregate::count::count_udaf(),
                datafusion::functions_aggregate::sum::sum_udaf(),
                datafusion::functions_aggregate::average::avg_udaf(),
                datafusion::functions_aggregate::min_max::min_udaf(),
                datafusion::functions_aggregate::min_max::max_udaf(),
            ]
            .iter()
            .any(|compiler_owned| Arc::ptr_eq(compiler_owned, function)))
}

#[derive(Debug)]
struct NativeCacheEntry<V> {
    value: V,
    size_bytes: usize,
    hits: usize,
    expires: Option<Instant>,
    last_used: u64,
}

#[derive(Debug)]
struct NativeCacheState<K, V> {
    clock: u64,
    resident_bytes: usize,
    entries: DataFusionHashMap<K, NativeCacheEntry<V>>,
}

impl<K, V> Default for NativeCacheState<K, V> {
    fn default() -> Self {
        Self {
            clock: 0,
            resident_bytes: 0,
            entries: DataFusionHashMap::new(),
        }
    }
}

/// DataFusion-native cache implementation with independent entry and byte bounds.
///
/// DataFusion 55 exposes public cache traits and a configurable cache manager, while its default
/// caches enforce only their byte budgets. This adapter retains the native key/value sizing,
/// table invalidation, TTL, and inspection contracts and adds a deterministic LRU entry bound.
/// Cache contents remain reconstructible accelerators; eviction and oversize bypass never change
/// query semantics.
struct DualBoundDataFusionCache<K, V> {
    name: Arc<str>,
    capacity_entries: NonZeroUsize,
    capacity_bytes: AtomicUsize,
    ttl: Mutex<Option<Duration>>,
    state: Mutex<NativeCacheState<K, V>>,
}

impl<K, V> Debug for DualBoundDataFusionCache<K, V> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DualBoundDataFusionCache")
            .field("name", &self.name)
            .field("capacity_entries", &self.capacity_entries)
            .field(
                "capacity_bytes",
                &self.capacity_bytes.load(Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

impl<K, V> DualBoundDataFusionCache<K, V>
where
    K: CacheKey,
    V: CacheValue,
{
    fn new(
        name: impl Into<Arc<str>>,
        capacity_entries: usize,
        capacity_bytes: usize,
        ttl: Option<Duration>,
    ) -> Self {
        Self {
            name: name.into(),
            capacity_entries: NonZeroUsize::new(capacity_entries)
                .expect("cache policy validates a non-zero entry bound"),
            capacity_bytes: AtomicUsize::new(capacity_bytes),
            ttl: Mutex::new(ttl),
            state: Mutex::new(NativeCacheState::default()),
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, NativeCacheState<K, V>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn prune_expired(state: &mut NativeCacheState<K, V>) {
        let now = Instant::now();
        let expired = state
            .entries
            .iter()
            .filter_map(|(key, entry)| {
                entry
                    .expires
                    .is_some_and(|expiry| expiry <= now)
                    .then(|| key.clone())
            })
            .collect::<Vec<_>>();
        for key in expired {
            if let Some(entry) = state.entries.remove(&key) {
                state.resident_bytes = state.resident_bytes.saturating_sub(entry.size_bytes);
            }
        }
    }

    fn evict_to_limits(&self, state: &mut NativeCacheState<K, V>) {
        let byte_limit = self.capacity_bytes.load(Ordering::Relaxed);
        while state.entries.len() > self.capacity_entries.get() || state.resident_bytes > byte_limit
        {
            let Some(key) = state
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(entry) = state.entries.remove(&key) {
                state.resident_bytes = state.resident_bytes.saturating_sub(entry.size_bytes);
            }
        }
    }

    fn remove_entry(state: &mut NativeCacheState<K, V>, key: &K) -> Option<V> {
        state.entries.remove(key).map(|entry| {
            state.resident_bytes = state.resident_bytes.saturating_sub(entry.size_bytes);
            entry.value
        })
    }
}

impl<K, V> Cache<K, V> for DualBoundDataFusionCache<K, V>
where
    K: CacheKey + 'static,
    V: CacheValue + 'static,
{
    fn get(&self, key: &K) -> Option<V> {
        let mut state = self.lock_state();
        Self::prune_expired(&mut state);
        state.clock = state.clock.saturating_add(1);
        let clock = state.clock;
        state.entries.get_mut(key).map(|entry| {
            entry.hits = entry.hits.saturating_add(1);
            entry.last_used = clock;
            entry.value.clone()
        })
    }

    fn put(&self, key: &K, value: V) -> Option<V> {
        let mut state = self.lock_state();
        Self::prune_expired(&mut state);
        let previous = Self::remove_entry(&mut state, key);
        let size_bytes = key.size().saturating_add(value.size());
        let byte_limit = self.capacity_bytes.load(Ordering::Relaxed);
        if size_bytes > byte_limit {
            return previous;
        }
        while state.entries.len() >= self.capacity_entries.get()
            || state.resident_bytes.saturating_add(size_bytes) > byte_limit
        {
            let Some(eviction_key) = state
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(candidate, _)| candidate.clone())
            else {
                break;
            };
            let _ = Self::remove_entry(&mut state, &eviction_key);
        }
        state.clock = state.clock.saturating_add(1);
        let last_used = state.clock;
        let expires = *self
            .ttl
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let expires = expires.map(|ttl| Instant::now() + ttl);
        state.resident_bytes = state.resident_bytes.saturating_add(size_bytes);
        state.entries.insert(
            key.clone(),
            NativeCacheEntry {
                value,
                size_bytes,
                hits: 0,
                expires,
                last_used,
            },
        );
        previous
    }

    fn remove(&self, key: &K) -> Option<V> {
        Self::remove_entry(&mut self.lock_state(), key)
    }

    fn contains_key(&self, key: &K) -> bool {
        let mut state = self.lock_state();
        Self::prune_expired(&mut state);
        state.entries.contains_key(key)
    }

    fn len(&self) -> usize {
        let mut state = self.lock_state();
        Self::prune_expired(&mut state);
        state.entries.len()
    }

    fn clear(&self) {
        let mut state = self.lock_state();
        state.entries.clear();
        state.resident_bytes = 0;
    }

    fn name(&self) -> String {
        self.name.to_string()
    }

    fn cache_limit(&self) -> usize {
        self.capacity_bytes.load(Ordering::Relaxed)
    }

    fn update_cache_limit(&self, limit: usize) {
        self.capacity_bytes.store(limit, Ordering::Relaxed);
        let mut state = self.lock_state();
        self.evict_to_limits(&mut state);
    }

    fn cache_ttl(&self) -> Option<Duration> {
        *self
            .ttl
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn update_cache_ttl(&self, ttl: Option<Duration>) {
        *self
            .ttl
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ttl;
    }

    fn drop_table_entries(&self, table_ref: &TableReference) -> datafusion::common::Result<()> {
        let mut state = self.lock_state();
        let matching = state
            .entries
            .keys()
            .filter(|key| key.table_ref() == Some(table_ref))
            .cloned()
            .collect::<Vec<_>>();
        for key in matching {
            let _ = Self::remove_entry(&mut state, &key);
        }
        Ok(())
    }

    fn list_entries(&self) -> DataFusionHashMap<K, CacheEntryInfo<V>> {
        let mut state = self.lock_state();
        Self::prune_expired(&mut state);
        state
            .entries
            .iter()
            .map(|(key, entry)| {
                (
                    key.clone(),
                    CacheEntryInfo {
                        value: entry.value.clone(),
                        size_bytes: entry.size_bytes,
                        hits: entry.hits,
                        expires: entry.expires,
                    },
                )
            })
            .collect()
    }
}

/// Explicit limits for DataFusion's runtime caches and CodeFabric's logical-plan cache.
///
/// The object-list TTL is always finite. Exact Delta snapshots remain the source of table-file
/// authority; the listing cache may accelerate other epoch-local providers without hiding an
/// indefinitely stale mutable prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataFusionCachePolicy {
    metadata_cache_bytes: NonZeroUsize,
    metadata_cache_entries: NonZeroUsize,
    file_statistics_cache_bytes: NonZeroUsize,
    file_statistics_cache_entries: NonZeroUsize,
    object_list_cache_bytes: NonZeroUsize,
    object_list_cache_entries: NonZeroUsize,
    object_list_cache_ttl_seconds: NonZeroU64,
    logical_plan_entries: NonZeroUsize,
    logical_plan_bytes: NonZeroUsize,
}

impl DataFusionCachePolicy {
    /// Derive a bounded cache profile from the runtime's explicit memory envelope.
    ///
    /// DataFusion cache limits are independent of its execution memory pool. Keeping their total
    /// limit at 15/32 of the pool prevents a small child runtime from silently inheriting the
    /// workstation-sized cache profile while retaining useful space for repeated Parquet work.
    #[must_use]
    pub fn proportional_to(memory_limit_bytes: usize) -> Self {
        let metadata = (memory_limit_bytes / 4).max(1);
        let statistics = (memory_limit_bytes / 8).max(1);
        let object_lists = (memory_limit_bytes / 32).max(1);
        let plan_entries = (memory_limit_bytes / (1024 * 1024)).clamp(1, 256);
        let plan_bytes = (memory_limit_bytes / 16).max(1);
        Self {
            metadata_cache_bytes: NonZeroUsize::new(metadata).unwrap_or(NonZeroUsize::MIN),
            metadata_cache_entries: native_entry_bound(
                metadata,
                FILE_METADATA_TARGET_BYTES_PER_ENTRY,
            ),
            file_statistics_cache_bytes: NonZeroUsize::new(statistics).unwrap_or(NonZeroUsize::MIN),
            file_statistics_cache_entries: native_entry_bound(
                statistics,
                FILE_STATISTICS_TARGET_BYTES_PER_ENTRY,
            ),
            object_list_cache_bytes: NonZeroUsize::new(object_lists).unwrap_or(NonZeroUsize::MIN),
            object_list_cache_entries: native_entry_bound(
                object_lists,
                OBJECT_LIST_TARGET_BYTES_PER_ENTRY,
            ),
            object_list_cache_ttl_seconds: NonZeroU64::new(OBJECT_LIST_CACHE_MAX_REFRESH_SECONDS)
                .unwrap_or(NonZeroU64::MIN),
            logical_plan_entries: NonZeroUsize::new(plan_entries).unwrap_or(NonZeroUsize::MIN),
            logical_plan_bytes: NonZeroUsize::new(plan_bytes).unwrap_or(NonZeroUsize::MIN),
        }
    }

    /// Construct one fully bounded cache policy.
    ///
    /// # Errors
    ///
    /// Rejects a zero byte, TTL, or entry bound, and an object-list TTL above the 30-second
    /// refresh bound. Disabling a cache is a distinct release policy, not an accidental zero
    /// hidden in an otherwise enabled profile. TTL controls refresh only; it never establishes
    /// cache validity or semantic authority.
    pub fn try_new(
        metadata_cache_bytes: usize,
        file_statistics_cache_bytes: usize,
        object_list_cache_bytes: usize,
        object_list_cache_ttl_seconds: u64,
        logical_plan_entries: usize,
        logical_plan_bytes: usize,
    ) -> Result<Self, DataFusionCachePolicyError> {
        Self::try_new_with_entry_limits(
            metadata_cache_bytes,
            native_entry_bound(metadata_cache_bytes, FILE_METADATA_TARGET_BYTES_PER_ENTRY).get(),
            file_statistics_cache_bytes,
            native_entry_bound(
                file_statistics_cache_bytes,
                FILE_STATISTICS_TARGET_BYTES_PER_ENTRY,
            )
            .get(),
            object_list_cache_bytes,
            native_entry_bound(object_list_cache_bytes, OBJECT_LIST_TARGET_BYTES_PER_ENTRY).get(),
            object_list_cache_ttl_seconds,
            logical_plan_entries,
            logical_plan_bytes,
        )
    }

    /// Construct one cache policy with exact entry and byte bounds for every cache family.
    ///
    /// This is the release-policy constructor. [`Self::try_new`] remains a compact constructor
    /// that derives conservative native-cache entry bounds from the supplied byte budgets.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new_with_entry_limits(
        metadata_cache_bytes: usize,
        metadata_cache_entries: usize,
        file_statistics_cache_bytes: usize,
        file_statistics_cache_entries: usize,
        object_list_cache_bytes: usize,
        object_list_cache_entries: usize,
        object_list_cache_ttl_seconds: u64,
        logical_plan_entries: usize,
        logical_plan_bytes: usize,
    ) -> Result<Self, DataFusionCachePolicyError> {
        let policy = Self {
            metadata_cache_bytes: NonZeroUsize::new(metadata_cache_bytes).ok_or(
                DataFusionCachePolicyError::ZeroBound("metadata_cache_bytes"),
            )?,
            metadata_cache_entries: NonZeroUsize::new(metadata_cache_entries).ok_or(
                DataFusionCachePolicyError::ZeroBound("metadata_cache_entries"),
            )?,
            file_statistics_cache_bytes: NonZeroUsize::new(file_statistics_cache_bytes).ok_or(
                DataFusionCachePolicyError::ZeroBound("file_statistics_cache_bytes"),
            )?,
            file_statistics_cache_entries: NonZeroUsize::new(file_statistics_cache_entries).ok_or(
                DataFusionCachePolicyError::ZeroBound("file_statistics_cache_entries"),
            )?,
            object_list_cache_bytes: NonZeroUsize::new(object_list_cache_bytes).ok_or(
                DataFusionCachePolicyError::ZeroBound("object_list_cache_bytes"),
            )?,
            object_list_cache_entries: NonZeroUsize::new(object_list_cache_entries).ok_or(
                DataFusionCachePolicyError::ZeroBound("object_list_cache_entries"),
            )?,
            object_list_cache_ttl_seconds: NonZeroU64::new(object_list_cache_ttl_seconds).ok_or(
                DataFusionCachePolicyError::ZeroBound("object_list_cache_ttl_seconds"),
            )?,
            logical_plan_entries: NonZeroUsize::new(logical_plan_entries).ok_or(
                DataFusionCachePolicyError::ZeroBound("logical_plan_entries"),
            )?,
            logical_plan_bytes: NonZeroUsize::new(logical_plan_bytes)
                .ok_or(DataFusionCachePolicyError::ZeroBound("logical_plan_bytes"))?,
        };
        if policy.object_list_cache_ttl_seconds() > OBJECT_LIST_CACHE_MAX_REFRESH_SECONDS {
            return Err(
                DataFusionCachePolicyError::ObjectListTtlExceedsRefreshBound {
                    requested_seconds: policy.object_list_cache_ttl_seconds(),
                    max_seconds: OBJECT_LIST_CACHE_MAX_REFRESH_SECONDS,
                },
            );
        }
        Ok(policy)
    }

    #[must_use]
    pub const fn metadata_cache_bytes(&self) -> usize {
        self.metadata_cache_bytes.get()
    }

    #[must_use]
    pub const fn metadata_cache_entries(&self) -> usize {
        self.metadata_cache_entries.get()
    }

    #[must_use]
    pub const fn file_statistics_cache_bytes(&self) -> usize {
        self.file_statistics_cache_bytes.get()
    }

    #[must_use]
    pub const fn file_statistics_cache_entries(&self) -> usize {
        self.file_statistics_cache_entries.get()
    }

    #[must_use]
    pub const fn object_list_cache_bytes(&self) -> usize {
        self.object_list_cache_bytes.get()
    }

    #[must_use]
    pub const fn object_list_cache_entries(&self) -> usize {
        self.object_list_cache_entries.get()
    }

    #[must_use]
    pub const fn object_list_cache_ttl_seconds(&self) -> u64 {
        self.object_list_cache_ttl_seconds.get()
    }

    #[must_use]
    pub const fn logical_plan_entries(&self) -> usize {
        self.logical_plan_entries.get()
    }

    #[must_use]
    pub const fn logical_plan_bytes(&self) -> usize {
        self.logical_plan_bytes.get()
    }

    pub(super) fn configure_runtime(&self, builder: RuntimeEnvBuilder) -> RuntimeEnvBuilder {
        let file_metadata_cache: Arc<FileMetadataCache> = Arc::new(DualBoundDataFusionCache::<
            ObjectStorePath,
            CachedFileMetadataEntry,
        >::new(
            "CodeFabricFileMetadataCache",
            self.metadata_cache_entries(),
            self.metadata_cache_bytes(),
            None,
        ));
        let file_statistics_cache: Arc<FileStatisticsCache> = Arc::new(DualBoundDataFusionCache::<
            TableScopedPath,
            CachedFileMetadata,
        >::new(
            "CodeFabricFileStatisticsCache",
            self.file_statistics_cache_entries(),
            self.file_statistics_cache_bytes(),
            None,
        ));
        let object_list_ttl = Duration::from_secs(self.object_list_cache_ttl_seconds());
        let list_files_cache: Arc<ListFilesCache> = Arc::new(DualBoundDataFusionCache::<
            TableScopedPath,
            CachedFileList,
        >::new(
            "CodeFabricListFilesCache",
            self.object_list_cache_entries(),
            self.object_list_cache_bytes(),
            Some(object_list_ttl),
        ));
        let cache_manager = CacheManagerConfig {
            file_statistics_cache: Some(file_statistics_cache),
            file_statistics_cache_limit: self.file_statistics_cache_bytes(),
            list_files_cache: Some(list_files_cache),
            list_files_cache_limit: self.object_list_cache_bytes(),
            list_files_cache_ttl: Some(object_list_ttl),
            file_metadata_cache: Some(file_metadata_cache),
            metadata_cache_limit: self.metadata_cache_bytes(),
        };
        builder.with_cache_manager(cache_manager)
    }

    pub(super) fn identity_fragment(&self) -> String {
        format!(
            "metadata-cache-bytes={}:metadata-cache-entries={}:file-statistics-cache-bytes={}:file-statistics-cache-entries={}:object-list-cache-bytes={}:object-list-cache-entries={}:object-list-ttl-seconds={}:logical-plan-entries={}:logical-plan-bytes={}",
            self.metadata_cache_bytes(),
            self.metadata_cache_entries(),
            self.file_statistics_cache_bytes(),
            self.file_statistics_cache_entries(),
            self.object_list_cache_bytes(),
            self.object_list_cache_entries(),
            self.object_list_cache_ttl_seconds(),
            self.logical_plan_entries(),
            self.logical_plan_bytes(),
        )
    }
}

fn native_entry_bound(bytes: usize, target_bytes_per_entry: usize) -> NonZeroUsize {
    let entries = bytes
        .div_ceil(target_bytes_per_entry)
        .clamp(1, NATIVE_CACHE_MAX_ENTRIES);
    NonZeroUsize::new(entries).unwrap_or(NonZeroUsize::MIN)
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DataFusionCachePolicyError {
    #[error("DataFusion cache bound {0} must be non-zero")]
    ZeroBound(&'static str),
    #[error(
        "DataFusion object-list cache TTL {requested_seconds}s exceeds the {max_seconds}s refresh bound; TTL never establishes cache validity or authority"
    )]
    ObjectListTtlExceedsRefreshBound {
        requested_seconds: u64,
        max_seconds: u64,
    },
}

/// Exact authority scope under which a cached plan may be reused.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) enum LogicalPlanCacheScope {
    Epoch,
    Authorized {
        access_scope: [u8; 32],
        query_policy: [u8; 32],
        resource_policy: [u8; 32],
        allowed_relations: Arc<[Arc<str>]>,
        max_output_rows: usize,
    },
}

/// Collision-safe cache identity. The complete typed program participates in equality; the cache
/// never treats a digest alone as semantic identity.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct LogicalPlanCacheKey {
    epoch_id: EpochId,
    table_versions: TableVersionSetRef,
    session_authority: Arc<str>,
    runtime_configuration: Arc<str>,
    semantic_authority: LogicalPlanAuthorityFingerprint,
    scope: LogicalPlanCacheScope,
    program: RelationalProgram,
}

impl LogicalPlanCacheKey {
    pub(super) fn new(
        epoch_id: EpochId,
        table_versions: TableVersionSetRef,
        session_authority: impl Into<Arc<str>>,
        runtime_configuration: impl Into<Arc<str>>,
        semantic_authority: LogicalPlanAuthorityFingerprint,
        scope: LogicalPlanCacheScope,
        program: &RelationalProgram,
    ) -> Self {
        Self {
            epoch_id,
            table_versions,
            session_authority: session_authority.into(),
            runtime_configuration: runtime_configuration.into(),
            semantic_authority,
            scope,
            program: program.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RetainedLogicalCapability {
    TableProvider {
        table_name: TableReference,
        data_address: usize,
    },
    TableSource {
        table_name: TableReference,
        data_address: usize,
    },
    ViewDefinition {
        table_name: TableReference,
        plan: Box<LogicalPlan>,
    },
    ScalarFunction {
        name: Arc<str>,
        data_address: usize,
    },
    AggregateFunction {
        name: Arc<str>,
        data_address: usize,
    },
    WindowFunction {
        name: Arc<str>,
        data_address: usize,
    },
}

impl RetainedLogicalCapability {
    fn accounted_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::TableProvider { table_name, .. } | Self::TableSource { table_name, .. } => {
                    table_name.to_string().len()
                }
                Self::ViewDefinition { table_name, plan } => table_name
                    .to_string()
                    .len()
                    .saturating_add(plan.display_indent_schema().to_string().len()),
                Self::ScalarFunction { name, .. }
                | Self::AggregateFunction { name, .. }
                | Self::WindowFunction { name, .. } => name.len(),
            }
    }
}

fn arc_data_address<T: ?Sized>(capability: &Arc<T>) -> usize {
    Arc::as_ptr(capability) as *const () as usize
}

fn retained_logical_capabilities(plan: &LogicalPlan) -> Vec<RetainedLogicalCapability> {
    fn collect(
        plan: &LogicalPlan,
        capabilities: &mut Vec<RetainedLogicalCapability>,
        provider_stack: &mut Vec<usize>,
    ) -> datafusion::common::Result<()> {
        plan.apply_with_subqueries(|node| {
            if let LogicalPlan::TableScan(scan) = node {
                if let Ok(provider) = source_as_provider(&scan.source) {
                    let provider_address = arc_data_address(&provider);
                    if !provider_stack.contains(&provider_address)
                        && let Some(definition) = registered_view_logical_plan(provider.as_ref())
                    {
                        capabilities.push(RetainedLogicalCapability::ViewDefinition {
                            table_name: scan.table_name.clone(),
                            plan: Box::new(definition.clone()),
                        });
                        provider_stack.push(provider_address);
                        collect(&definition, capabilities, provider_stack)?;
                        let removed = provider_stack.pop();
                        debug_assert_eq!(removed, Some(provider_address));
                    } else {
                        capabilities.push(RetainedLogicalCapability::TableProvider {
                            table_name: scan.table_name.clone(),
                            data_address: provider_address,
                        });
                    }
                } else {
                    capabilities.push(RetainedLogicalCapability::TableSource {
                        table_name: scan.table_name.clone(),
                        data_address: arc_data_address(&scan.source),
                    });
                }
            }
            for expression in node.expressions() {
                expression.apply(|candidate| {
                    match candidate {
                        Expr::ScalarFunction(function) => {
                            capabilities.push(RetainedLogicalCapability::ScalarFunction {
                                name: Arc::from(function.func.name()),
                                data_address: arc_data_address(&function.func),
                            });
                        }
                        Expr::AggregateFunction(function) => {
                            capabilities.push(RetainedLogicalCapability::AggregateFunction {
                                name: Arc::from(function.func.name()),
                                data_address: arc_data_address(&function.func),
                            });
                        }
                        Expr::WindowFunction(function) => match &function.fun {
                            WindowFunctionDefinition::AggregateUDF(aggregate) => {
                                capabilities.push(RetainedLogicalCapability::AggregateFunction {
                                    name: Arc::from(aggregate.name()),
                                    data_address: arc_data_address(aggregate),
                                });
                            }
                            WindowFunctionDefinition::WindowUDF(window) => {
                                capabilities.push(RetainedLogicalCapability::WindowFunction {
                                    name: Arc::from(window.name()),
                                    data_address: arc_data_address(window),
                                });
                            }
                        },
                        _ => {}
                    }
                    Ok(TreeNodeRecursion::Continue)
                })?;
            }
            Ok(TreeNodeRecursion::Continue)
        })?;
        Ok(())
    }

    let mut capabilities = Vec::new();
    collect(plan, &mut capabilities, &mut Vec::new())
        .expect("logical capability collection is an infallible tree traversal");
    capabilities
}

/// Reconstructible native plans and their causal compiler observations.
///
/// `accounted_bytes` is a deterministic cache-pressure estimate over the owned plan values, their
/// complete schema-bearing renderings, output schema, and compiler observations. It is deliberately
/// not presented as allocator-observed resident memory: shared provider capabilities are authority
/// references rather than cache-owned allocations and their heap allocations are not charged here.
#[derive(Clone, Debug)]
pub(super) struct CachedLogicalPlan {
    compiled_plan: LogicalPlan,
    optimized_plan: LogicalPlan,
    output_schema: SchemaRef,
    observations: CompilationObservations,
    compiled_plan_digest: [u8; 32],
    optimized_plan_digest: [u8; 32],
    compiled_capabilities: Arc<[RetainedLogicalCapability]>,
    optimized_capabilities: Arc<[RetainedLogicalCapability]>,
    accounted_bytes: usize,
}

impl CachedLogicalPlan {
    pub(super) fn new(
        compiled_plan: LogicalPlan,
        optimized_plan: LogicalPlan,
        output_schema: SchemaRef,
        observations: CompilationObservations,
    ) -> Self {
        let compiled_plan_encoding: Arc<str> =
            compiled_plan.display_indent_schema().to_string().into();
        let optimized_plan_encoding: Arc<str> =
            optimized_plan.display_indent_schema().to_string().into();
        let compiled_plan_digest =
            logical_plan_digest(COMPILED_PLAN_DIGEST_DOMAIN, &compiled_plan_encoding);
        let optimized_plan_digest =
            logical_plan_digest(OPTIMIZED_PLAN_DIGEST_DOMAIN, &optimized_plan_encoding);
        let compiled_capabilities: Arc<[RetainedLogicalCapability]> =
            retained_logical_capabilities(&compiled_plan).into();
        let optimized_capabilities: Arc<[RetainedLogicalCapability]> =
            retained_logical_capabilities(&optimized_plan).into();
        let accounted_bytes = std::mem::size_of::<Self>()
            .saturating_add(compiled_plan_encoding.len())
            .saturating_add(optimized_plan_encoding.len())
            .saturating_add(
                compiled_capabilities
                    .iter()
                    .map(RetainedLogicalCapability::accounted_bytes)
                    .sum::<usize>(),
            )
            .saturating_add(
                optimized_capabilities
                    .iter()
                    .map(RetainedLogicalCapability::accounted_bytes)
                    .sum::<usize>(),
            )
            .saturating_add(format!("{output_schema:?}").len())
            .saturating_add(format!("{observations:?}").len());
        Self {
            compiled_plan,
            optimized_plan,
            output_schema,
            observations,
            compiled_plan_digest,
            optimized_plan_digest,
            compiled_capabilities,
            optimized_capabilities,
            accounted_bytes,
        }
    }

    #[must_use]
    pub(super) const fn compiled_plan(&self) -> &LogicalPlan {
        &self.compiled_plan
    }

    #[must_use]
    pub(super) const fn optimized_plan(&self) -> &LogicalPlan {
        &self.optimized_plan
    }

    #[must_use]
    pub(super) const fn output_schema(&self) -> &SchemaRef {
        &self.output_schema
    }

    #[must_use]
    pub(super) const fn observations(&self) -> &CompilationObservations {
        &self.observations
    }

    #[must_use]
    pub(super) const fn compiled_plan_digest(&self) -> [u8; 32] {
        self.compiled_plan_digest
    }

    #[must_use]
    pub(super) const fn optimized_plan_digest(&self) -> [u8; 32] {
        self.optimized_plan_digest
    }

    #[must_use]
    pub(super) const fn accounted_bytes(&self) -> usize {
        self.accounted_bytes
    }

    fn same_materialization(&self, other: &Self) -> bool {
        self.compiled_plan == other.compiled_plan
            && self.optimized_plan == other.optimized_plan
            && self.compiled_capabilities == other.compiled_capabilities
            && self.optimized_capabilities == other.optimized_capabilities
            && self.output_schema == other.output_schema
            && self.observations == other.observations
    }
}

/// Whether this execution reused a cached normalized/optimized logical plan pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalPlanCacheOutcome {
    Hit,
    Miss,
}

/// Query-local plan materialization evidence. Digests are diagnostic identity at the pinned
/// DataFusion release; the typed program and exact cache key remain the collision-safe key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalPlanExecutionObservation {
    pub outcome: LogicalPlanCacheOutcome,
    pub compiled_plan_digest: [u8; 32],
    pub optimized_plan_digest: [u8; 32],
}

/// Read-only aggregate cache counters suitable for a system relation.
///
/// The byte fields are deterministic pressure-accounting units, not allocator-observed memory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalPlanCacheObservation {
    pub capacity_entries: usize,
    pub accounting_capacity_bytes: usize,
    pub resident_entries: usize,
    pub accounted_bytes: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub oversized_bypasses: u64,
    pub collisions: u64,
}

#[derive(Debug)]
struct ResidentPlan {
    entry: Arc<CachedLogicalPlan>,
    last_used: u64,
    accounted_bytes: usize,
}

#[derive(Debug, Default)]
struct LogicalPlanCacheState {
    clock: u64,
    hits: u64,
    misses: u64,
    evictions: u64,
    oversized_bypasses: u64,
    collisions: u64,
    accounted_bytes: usize,
    entries: HashMap<LogicalPlanCacheKey, ResidentPlan>,
}

/// A complete typed cache key produced a different logical materialization.
///
/// Reusing either value would let an accelerator choose semantics, so the cache rejects the
/// insertion and leaves its resident entry unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LogicalPlanCacheError {
    #[error("logical-plan cache key collision produced a different materialization")]
    MaterializationCollision,
}

/// Epoch-owned LRU for compiled and optimized logical plans.
///
/// Physical plans are intentionally excluded: DataFusion rebuilds and physically optimizes one
/// for the current task/runtime on every execution. Results are also never cached here.
#[derive(Debug)]
pub(super) struct EpochLogicalPlanCache {
    capacity_entries: NonZeroUsize,
    accounting_capacity_bytes: NonZeroUsize,
    state: Mutex<LogicalPlanCacheState>,
}

impl EpochLogicalPlanCache {
    pub(super) fn new(capacity_entries: usize, accounting_capacity_bytes: usize) -> Self {
        Self {
            capacity_entries: NonZeroUsize::new(capacity_entries)
                .expect("cache policy construction guarantees a non-zero entry capacity"),
            accounting_capacity_bytes: NonZeroUsize::new(accounting_capacity_bytes)
                .expect("cache policy construction guarantees a non-zero accounting capacity"),
            state: Mutex::new(LogicalPlanCacheState::default()),
        }
    }

    pub(super) fn get(&self, key: &LogicalPlanCacheKey) -> Option<Arc<CachedLogicalPlan>> {
        let mut state = self.lock_state();
        state.clock = state.clock.saturating_add(1);
        let clock = state.clock;
        if let Some(resident) = state.entries.get_mut(key) {
            resident.last_used = clock;
            let entry = Arc::clone(&resident.entry);
            state.hits = state.hits.saturating_add(1);
            Some(entry)
        } else {
            state.misses = state.misses.saturating_add(1);
            None
        }
    }

    pub(super) fn try_insert(
        &self,
        key: LogicalPlanCacheKey,
        entry: CachedLogicalPlan,
    ) -> Result<Arc<CachedLogicalPlan>, LogicalPlanCacheError> {
        let mut state = self.lock_state();
        state.clock = state.clock.saturating_add(1);
        let clock = state.clock;
        if let Some(resident) = state.entries.get_mut(&key) {
            if !resident.entry.same_materialization(&entry) {
                state.collisions = state.collisions.saturating_add(1);
                return Err(LogicalPlanCacheError::MaterializationCollision);
            }
            resident.last_used = clock;
            return Ok(Arc::clone(&resident.entry));
        }
        let entry = Arc::new(entry);
        let entry_bytes = entry.accounted_bytes();
        if entry_bytes > self.accounting_capacity_bytes.get() {
            state.oversized_bypasses = state.oversized_bypasses.saturating_add(1);
            return Ok(entry);
        }
        while state.entries.len() >= self.capacity_entries.get()
            || state.accounted_bytes.saturating_add(entry_bytes)
                > self.accounting_capacity_bytes.get()
        {
            let Some(eviction_key) = state
                .entries
                .iter()
                .min_by_key(|(_, resident)| resident.last_used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(evicted) = state.entries.remove(&eviction_key) {
                state.accounted_bytes = state
                    .accounted_bytes
                    .saturating_sub(evicted.accounted_bytes);
            }
            state.evictions = state.evictions.saturating_add(1);
        }
        state.accounted_bytes = state.accounted_bytes.saturating_add(entry_bytes);
        state.entries.insert(
            key,
            ResidentPlan {
                entry: Arc::clone(&entry),
                last_used: clock,
                accounted_bytes: entry_bytes,
            },
        );
        Ok(entry)
    }

    #[must_use]
    pub(super) fn observation(&self) -> LogicalPlanCacheObservation {
        let state = self.lock_state();
        LogicalPlanCacheObservation {
            capacity_entries: self.capacity_entries.get(),
            accounting_capacity_bytes: self.accounting_capacity_bytes.get(),
            resident_entries: state.entries.len(),
            accounted_bytes: state.accounted_bytes,
            hits: state.hits,
            misses: state.misses,
            evictions: state.evictions,
            oversized_bypasses: state.oversized_bypasses,
            collisions: state.collisions,
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, LogicalPlanCacheState> {
        // A panic cannot make cached plans authoritative or corrupt durable state. Recover the
        // derived map so cache failure never changes query semantics.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

pub(super) fn execution_observation(
    entry: &CachedLogicalPlan,
    outcome: LogicalPlanCacheOutcome,
) -> LogicalPlanExecutionObservation {
    LogicalPlanExecutionObservation {
        outcome,
        compiled_plan_digest: entry.compiled_plan_digest(),
        optimized_plan_digest: entry.optimized_plan_digest(),
    }
}

fn logical_plan_digest(domain: &[u8], rendered: &str) -> [u8; 32] {
    let mut digest = blake3::Hasher::new();
    digest.update(&(domain.len() as u64).to_be_bytes());
    digest.update(domain);
    digest.update(&(rendered.len() as u64).to_be_bytes());
    digest.update(rendered.as_bytes());
    *digest.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering as CmpOrdering;
    use std::fmt;

    use arrow_array::RecordBatch;
    use datafusion::common::{Constraints, DFSchema, DFSchemaRef};
    use datafusion::datasource::{MemTable, TableProvider, provider_as_source};
    use datafusion::logical_expr::{Extension, LogicalPlanBuilder, UserDefinedLogicalNodeCore};
    use datafusion::prelude::SessionContext;

    use super::*;
    use crate::relational_program::{FieldId, RelationId, RelationalExpression};
    use crate::schema_contract::{FieldIndexMapping, SchemaContractOptions};

    #[derive(Clone, Debug, Eq, Hash, PartialEq)]
    struct TestNativeCacheKey(u8);

    #[derive(Eq, Hash, PartialEq)]
    struct OpaqueTestExtension {
        schema: DFSchemaRef,
    }

    impl fmt::Debug for OpaqueTestExtension {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("OpaqueTestExtension")
        }
    }

    impl PartialOrd for OpaqueTestExtension {
        fn partial_cmp(&self, _other: &Self) -> Option<CmpOrdering> {
            Some(CmpOrdering::Equal)
        }
    }

    impl UserDefinedLogicalNodeCore for OpaqueTestExtension {
        fn name(&self) -> &str {
            "OpaqueTestExtension"
        }

        fn inputs(&self) -> Vec<&LogicalPlan> {
            Vec::new()
        }

        fn schema(&self) -> &DFSchemaRef {
            &self.schema
        }

        fn expressions(&self) -> Vec<Expr> {
            Vec::new()
        }

        fn fmt_for_explain(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.name())
        }

        fn with_exprs_and_inputs(
            &self,
            expressions: Vec<Expr>,
            inputs: Vec<LogicalPlan>,
        ) -> datafusion::common::Result<Self> {
            if !expressions.is_empty() || !inputs.is_empty() {
                return datafusion::common::internal_err!(
                    "opaque test extension accepts no expressions or inputs"
                );
            }
            Ok(Self {
                schema: Arc::clone(&self.schema),
            })
        }
    }

    impl CacheKey for TestNativeCacheKey {
        fn size(&self) -> usize {
            1
        }

        fn table_ref(&self) -> Option<&TableReference> {
            None
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct TestNativeCacheValue {
        id: u8,
        bytes: usize,
    }

    impl CacheValue for TestNativeCacheValue {
        fn size(&self) -> usize {
            self.bytes
        }
    }

    fn native_value(id: u8, bytes: usize) -> TestNativeCacheValue {
        TestNativeCacheValue { id, bytes }
    }

    fn schema_contract_authority(options: SchemaContractOptions) -> [u8; 32] {
        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::UInt64, false)]));
        let contract = SchemaContract::try_new_with_options(
            "test.schema.v1",
            TableReference::bare("facts"),
            Arc::clone(&schema),
            schema,
            vec![FieldIndexMapping::direct(0, 0)],
            options,
        )
        .unwrap();
        let mut builder = LogicalPlanAuthorityBuilder::new(b"schema-contract-test");
        frame_schema_contract(&mut builder, "facts", &contract).unwrap();
        *builder.finish().as_bytes()
    }

    #[test]
    fn schema_contract_cache_identity_frames_typed_policy_without_debug_text() {
        let exact = schema_contract_authority(SchemaContractOptions::new(
            Constraints::default(),
            SchemaCompatibility::Exact,
            ColumnMappingMode::Positional,
            DeletionVectorBehavior::Forbidden,
        ));
        let constrained = schema_contract_authority(SchemaContractOptions::new(
            Constraints::new_unverified(vec![Constraint::PrimaryKey(vec![0])]),
            SchemaCompatibility::Exact,
            ColumnMappingMode::Positional,
            DeletionVectorBehavior::Forbidden,
        ));
        let contains = schema_contract_authority(SchemaContractOptions::new(
            Constraints::default(),
            SchemaCompatibility::Contains,
            ColumnMappingMode::Positional,
            DeletionVectorBehavior::Forbidden,
        ));
        let field_id = schema_contract_authority(SchemaContractOptions::new(
            Constraints::default(),
            SchemaCompatibility::Exact,
            ColumnMappingMode::FieldId,
            DeletionVectorBehavior::Forbidden,
        ));
        let deletion_vectors = schema_contract_authority(SchemaContractOptions::new(
            Constraints::default(),
            SchemaCompatibility::Exact,
            ColumnMappingMode::Positional,
            DeletionVectorBehavior::AppliedByProvider,
        ));

        assert_ne!(exact, constrained);
        assert_ne!(exact, contains);
        assert_ne!(exact, field_id);
        assert_ne!(exact, deletion_vectors);
    }

    #[test]
    fn cached_plan_reference_validation_rejects_opaque_extension_capabilities() {
        let plan = LogicalPlan::Extension(Extension {
            node: Arc::new(OpaqueTestExtension {
                schema: Arc::new(DFSchema::empty()),
            }),
        });
        let state = SessionContext::new().state();

        let error = validate_logical_plan_references(&plan, &state, false, |_| Ok(()))
            .expect_err("an unregistered extension node must fail closed");

        assert_eq!(
            error,
            "cached logical plan retains unauthorized extension capability OpaqueTestExtension"
        );
    }

    #[test]
    fn native_datafusion_cache_enforces_entry_and_byte_bounds_with_lru_eviction() {
        let cache = DualBoundDataFusionCache::<TestNativeCacheKey, TestNativeCacheValue>::new(
            "test-native-cache",
            2,
            10,
            None,
        );
        let first = TestNativeCacheKey(1);
        let second = TestNativeCacheKey(2);
        let third = TestNativeCacheKey(3);

        assert_eq!(cache.put(&first, native_value(1, 3)), None);
        assert_eq!(cache.put(&second, native_value(2, 3)), None);
        assert_eq!(cache.get(&first), Some(native_value(1, 3)));
        assert_eq!(cache.put(&third, native_value(3, 3)), None);

        assert!(cache.contains_key(&first));
        assert!(!cache.contains_key(&second));
        assert!(cache.contains_key(&third));
        assert_eq!(cache.len(), 2);

        cache.update_cache_limit(5);
        assert_eq!(cache.len(), 1);
        assert!(cache.contains_key(&third));
        assert_eq!(cache.list_entries()[&third].size_bytes, 4);
    }

    #[test]
    fn native_datafusion_cache_bypasses_an_oversized_entry() {
        let cache = DualBoundDataFusionCache::<TestNativeCacheKey, TestNativeCacheValue>::new(
            "test-native-cache",
            4,
            4,
            None,
        );

        assert_eq!(cache.put(&TestNativeCacheKey(1), native_value(1, 4)), None);
        assert!(cache.is_empty());
    }

    #[test]
    fn cache_policy_rejects_unbounded_zeroes() {
        assert_eq!(
            DataFusionCachePolicy::try_new(1, 1, 1, 0, 1, 1),
            Err(DataFusionCachePolicyError::ZeroBound(
                "object_list_cache_ttl_seconds"
            ))
        );
        assert_eq!(
            DataFusionCachePolicy::try_new(1, 1, 1, 1, 0, 1),
            Err(DataFusionCachePolicyError::ZeroBound(
                "logical_plan_entries"
            ))
        );
        assert_eq!(
            DataFusionCachePolicy::try_new(1, 1, 1, 1, 1, 0),
            Err(DataFusionCachePolicyError::ZeroBound("logical_plan_bytes"))
        );
        assert_eq!(
            DataFusionCachePolicy::try_new_with_entry_limits(1, 0, 1, 1, 1, 1, 1, 1, 1),
            Err(DataFusionCachePolicyError::ZeroBound(
                "metadata_cache_entries"
            ))
        );
    }

    #[test]
    fn object_list_ttl_accepts_exact_refresh_bound_and_lower_values() {
        let lower = DataFusionCachePolicy::try_new(1, 1, 1, 1, 1, 1).unwrap();
        let boundary =
            DataFusionCachePolicy::try_new(1, 1, 1, OBJECT_LIST_CACHE_MAX_REFRESH_SECONDS, 1, 1)
                .unwrap();

        assert_eq!(lower.object_list_cache_ttl_seconds(), 1);
        assert_eq!(
            boundary.object_list_cache_ttl_seconds(),
            OBJECT_LIST_CACHE_MAX_REFRESH_SECONDS
        );
    }

    #[test]
    fn object_list_ttl_is_a_refresh_bound_not_validity_or_authority() {
        let error = DataFusionCachePolicy::try_new(
            1,
            1,
            1,
            OBJECT_LIST_CACHE_MAX_REFRESH_SECONDS + 1,
            1,
            1,
        )
        .unwrap_err();

        assert_eq!(
            error,
            DataFusionCachePolicyError::ObjectListTtlExceedsRefreshBound {
                requested_seconds: OBJECT_LIST_CACHE_MAX_REFRESH_SECONDS + 1,
                max_seconds: OBJECT_LIST_CACHE_MAX_REFRESH_SECONDS,
            }
        );
        assert_eq!(
            error.to_string(),
            "DataFusion object-list cache TTL 31s exceeds the 30s refresh bound; TTL never establishes cache validity or authority"
        );
    }

    #[test]
    fn cache_policy_installs_the_dual_bound_datafusion_cache_manager() {
        let policy =
            DataFusionCachePolicy::try_new_with_entry_limits(11, 2, 13, 3, 17, 4, 5, 6, 19)
                .unwrap();
        let runtime = policy
            .configure_runtime(RuntimeEnvBuilder::new())
            .build()
            .unwrap();
        let metadata = runtime.cache_manager.get_file_metadata_cache();
        let statistics = runtime.cache_manager.get_file_statistic_cache().unwrap();
        let object_lists = runtime.cache_manager.get_list_files_cache().unwrap();

        assert_eq!(metadata.name(), "CodeFabricFileMetadataCache");
        assert_eq!(metadata.cache_limit(), 11);
        assert_eq!(statistics.name(), "CodeFabricFileStatisticsCache");
        assert_eq!(statistics.cache_limit(), 13);
        assert_eq!(object_lists.name(), "CodeFabricListFilesCache");
        assert_eq!(object_lists.cache_limit(), 17);
        assert_eq!(object_lists.cache_ttl(), Some(Duration::from_secs(5)));
        assert_eq!(policy.metadata_cache_entries(), 2);
        assert_eq!(policy.file_statistics_cache_entries(), 3);
        assert_eq!(policy.object_list_cache_entries(), 4);
    }

    #[test]
    fn logical_plan_cache_is_collision_safe_bounded_and_lru() {
        fn program(seed: &str) -> RelationalProgram {
            RelationalProgram {
                root: RelationalExpression::Input(RelationId::new(seed).unwrap()),
                output_fields: vec![FieldId::new(format!("{seed}.field")).unwrap()],
            }
        }

        fn key(seed: u8) -> LogicalPlanCacheKey {
            let mut authority = LogicalPlanAuthorityBuilder::new(b"cache-test.v1");
            authority.frame(&[seed]);
            LogicalPlanCacheKey::new(
                EpochId::from_bytes([seed; 16]),
                TableVersionSetRef::from_bytes([seed; 32]),
                format!("authority-{seed}"),
                format!("runtime-{seed}"),
                authority.finish(),
                LogicalPlanCacheScope::Epoch,
                &program(&format!("relation-{seed}")),
            )
        }

        fn entry() -> CachedLogicalPlan {
            let plan = LogicalPlanBuilder::empty(false).build().unwrap();
            CachedLogicalPlan::new(
                plan.clone(),
                plan.clone(),
                Arc::new(plan.schema().as_arrow().clone()),
                CompilationObservations::default(),
            )
        }

        let cache = EpochLogicalPlanCache::new(1, usize::MAX);
        let first = key(1);
        let second = key(2);
        assert!(cache.get(&first).is_none());
        cache.try_insert(first.clone(), entry()).unwrap();
        assert!(cache.get(&first).is_some());
        cache.try_insert(second, entry()).unwrap();
        assert!(cache.get(&first).is_none());
        assert_eq!(
            cache.observation(),
            LogicalPlanCacheObservation {
                capacity_entries: 1,
                accounting_capacity_bytes: usize::MAX,
                resident_entries: 1,
                accounted_bytes: entry().accounted_bytes(),
                hits: 1,
                misses: 2,
                evictions: 1,
                oversized_bypasses: 0,
                collisions: 0,
            }
        );
    }

    #[test]
    fn logical_plan_cache_bypasses_oversized_entries_without_changing_semantics() {
        let mut authority = LogicalPlanAuthorityBuilder::new(b"cache-byte-bound.v1");
        authority.frame(b"one");
        let program = RelationalProgram {
            root: RelationalExpression::Input(RelationId::new("relation").unwrap()),
            output_fields: vec![FieldId::new("relation.field").unwrap()],
        };
        let key = LogicalPlanCacheKey::new(
            EpochId::from_bytes([3; 16]),
            TableVersionSetRef::from_bytes([3; 32]),
            "authority",
            "runtime",
            authority.finish(),
            LogicalPlanCacheScope::Epoch,
            &program,
        );
        let plan = LogicalPlanBuilder::empty(false).build().unwrap();
        let entry = CachedLogicalPlan::new(
            plan.clone(),
            plan.clone(),
            Arc::new(plan.schema().as_arrow().clone()),
            CompilationObservations::default(),
        );
        assert!(entry.accounted_bytes() > 1);
        let cache = EpochLogicalPlanCache::new(4, 1);

        let returned = cache.try_insert(key.clone(), entry).unwrap();
        assert!(returned.accounted_bytes() > 1);
        assert!(cache.get(&key).is_none());
        let observation = cache.observation();
        assert_eq!(observation.resident_entries, 0);
        assert_eq!(observation.accounted_bytes, 0);
        assert_eq!(observation.oversized_bypasses, 1);
    }

    #[test]
    fn logical_plan_cache_rejects_materialization_collision_for_complete_key() {
        let mut authority = LogicalPlanAuthorityBuilder::new(b"cache-collision.v1");
        authority.frame(b"one");
        let program = RelationalProgram {
            root: RelationalExpression::Input(RelationId::new("relation").unwrap()),
            output_fields: vec![FieldId::new("relation.field").unwrap()],
        };
        let key = LogicalPlanCacheKey::new(
            EpochId::from_bytes([4; 16]),
            TableVersionSetRef::from_bytes([4; 32]),
            "authority",
            "runtime",
            authority.finish(),
            LogicalPlanCacheScope::Epoch,
            &program,
        );
        let first = LogicalPlanBuilder::empty(false).build().unwrap();
        let different = LogicalPlanBuilder::empty(true).build().unwrap();
        let cache = EpochLogicalPlanCache::new(2, usize::MAX);
        cache
            .try_insert(
                key.clone(),
                CachedLogicalPlan::new(
                    first.clone(),
                    first.clone(),
                    Arc::new(first.schema().as_arrow().clone()),
                    CompilationObservations::default(),
                ),
            )
            .unwrap();

        let error = cache
            .try_insert(
                key,
                CachedLogicalPlan::new(
                    different.clone(),
                    different.clone(),
                    Arc::new(different.schema().as_arrow().clone()),
                    CompilationObservations::default(),
                ),
            )
            .unwrap_err();
        assert_eq!(error, LogicalPlanCacheError::MaterializationCollision);
        assert_eq!(cache.observation().collisions, 1);
    }

    #[test]
    fn logical_plan_cache_does_not_treat_equal_renderings_as_equal_capabilities() {
        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::UInt64, false)]));
        let first_provider: Arc<dyn TableProvider> = Arc::new(
            MemTable::try_new(Arc::clone(&schema), vec![Vec::new()])
                .expect("one-empty-partition table"),
        );
        let second_provider: Arc<dyn TableProvider> = Arc::new(
            MemTable::try_new(
                Arc::clone(&schema),
                vec![vec![RecordBatch::new_empty(Arc::clone(&schema))]],
            )
            .expect("one-empty-batch table"),
        );
        let first = LogicalPlanBuilder::scan("facts", provider_as_source(first_provider), None)
            .unwrap()
            .build()
            .unwrap();
        let second = LogicalPlanBuilder::scan("facts", provider_as_source(second_provider), None)
            .unwrap()
            .build()
            .unwrap();

        assert_eq!(
            first.display_indent_schema().to_string(),
            second.display_indent_schema().to_string(),
            "the diagnostic rendering deliberately cannot expose provider capability identity"
        );
        assert_eq!(
            first, second,
            "DataFusion plan equality deliberately omits provider capability identity"
        );
        assert_ne!(
            retained_logical_capabilities(&first),
            retained_logical_capabilities(&second),
            "CodeFabric collision equality must retain exact provider capabilities"
        );

        let program = RelationalProgram {
            root: RelationalExpression::Input(RelationId::new("facts").unwrap()),
            output_fields: vec![FieldId::new("facts.id").unwrap()],
        };
        let mut authority = LogicalPlanAuthorityBuilder::new(b"cache-render-collision.v1");
        authority.frame(b"same-complete-key");
        let key = LogicalPlanCacheKey::new(
            EpochId::from_bytes([5; 16]),
            TableVersionSetRef::from_bytes([5; 32]),
            "authority",
            "runtime",
            authority.finish(),
            LogicalPlanCacheScope::Epoch,
            &program,
        );
        let cache = EpochLogicalPlanCache::new(2, usize::MAX);
        cache
            .try_insert(
                key.clone(),
                CachedLogicalPlan::new(
                    first.clone(),
                    first,
                    Arc::clone(&schema),
                    CompilationObservations::default(),
                ),
            )
            .unwrap();

        let error = cache
            .try_insert(
                key,
                CachedLogicalPlan::new(
                    second.clone(),
                    second,
                    schema,
                    CompilationObservations::default(),
                ),
            )
            .unwrap_err();
        assert_eq!(error, LogicalPlanCacheError::MaterializationCollision);
        assert_eq!(cache.observation().collisions, 1);
    }
}
