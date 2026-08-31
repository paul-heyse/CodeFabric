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
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use arrow_schema::SchemaRef;
use datafusion::common::tree_node::{TreeNode, TreeNodeRecursion};
use datafusion::execution::SessionState;
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::logical_expr::{Expr, LogicalPlan, TableScan, WindowFunctionDefinition};

use crate::relational_program::{CompilationObservations, RelationalProgram};
use crate::schema_contract::SchemaContract;

use super::activation::TableVersionSetRef;
use super::command::EpochId;

const COMPILED_PLAN_DIGEST_DOMAIN: &[u8] = b"codefabric.logical-plan.compiled.v1";
const OPTIMIZED_PLAN_DIGEST_DOMAIN: &[u8] = b"codefabric.logical-plan.optimized.v1";
const LOGICAL_PLAN_AUTHORITY_DOMAIN: &[u8] = b"codefabric.logical-plan.semantic-authority.v1";

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

    pub(super) fn frame_debug(&mut self, value: &impl Debug) {
        self.frame_str(&format!("{value:?}"));
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
        builder.frame_debug(cast.logical_data_type());
        builder.frame_debug(cast.storage_data_type());
    }
    builder.frame_debug(contract.constraints());
    builder.frame_debug(&contract.compatibility());
    builder.frame_debug(&contract.column_mapping_mode());
    builder.frame_debug(&contract.deletion_vector_behavior());
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

    builder.frame_usize(state.analyzer().function_rewrites().len());
    for rewrite in state.analyzer().function_rewrites() {
        builder.frame_debug(rewrite);
    }
    builder.frame_usize(state.analyzer().rules.len());
    for rule in &state.analyzer().rules {
        builder.frame_str(rule.name());
        builder.frame_debug(rule);
    }
    builder.frame_usize(state.optimizers().len());
    for rule in state.optimizers() {
        builder.frame_str(rule.name());
        builder.frame_debug(rule);
    }
    builder.frame_usize(state.expr_planners().len());
    for planner in state.expr_planners() {
        builder.frame_debug(planner);
    }
    builder.frame_usize(state.relation_planners().len());
    for planner in state.relation_planners() {
        builder.frame_debug(planner);
    }

    let mut scalar = state.scalar_functions().iter().collect::<Vec<_>>();
    scalar.sort_by(|(left, _), (right, _)| left.cmp(right));
    builder.frame_usize(scalar.len());
    for (installed_name, function) in scalar {
        builder.frame_str(installed_name);
        builder.frame_str(function.name());
        builder.frame_debug(function.signature());
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
        builder.frame_debug(function.signature());
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
        builder.frame_debug(function.signature());
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

/// Explicit limits for DataFusion's runtime caches and CodeFabric's logical-plan cache.
///
/// The object-list TTL is always finite. Exact Delta snapshots remain the source of table-file
/// authority; the listing cache may accelerate other epoch-local providers without hiding an
/// indefinitely stale mutable prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataFusionCachePolicy {
    metadata_cache_bytes: NonZeroUsize,
    file_statistics_cache_bytes: NonZeroUsize,
    object_list_cache_bytes: NonZeroUsize,
    object_list_cache_ttl_seconds: NonZeroU64,
    logical_plan_entries: NonZeroUsize,
}

impl DataFusionCachePolicy {
    /// Derive a bounded cache profile from the runtime's explicit memory envelope.
    ///
    /// DataFusion cache limits are independent of its execution memory pool. Keeping their total
    /// limit at 13/32 of the pool prevents a small child runtime from silently inheriting the
    /// workstation-sized cache profile while retaining useful space for repeated Parquet work.
    #[must_use]
    pub fn proportional_to(memory_limit_bytes: usize) -> Self {
        let metadata = (memory_limit_bytes / 4).max(1);
        let statistics = (memory_limit_bytes / 8).max(1);
        let object_lists = (memory_limit_bytes / 32).max(1);
        let plan_entries = (memory_limit_bytes / (1024 * 1024)).clamp(1, 256);
        Self {
            metadata_cache_bytes: NonZeroUsize::new(metadata).unwrap_or(NonZeroUsize::MIN),
            file_statistics_cache_bytes: NonZeroUsize::new(statistics).unwrap_or(NonZeroUsize::MIN),
            object_list_cache_bytes: NonZeroUsize::new(object_lists).unwrap_or(NonZeroUsize::MIN),
            object_list_cache_ttl_seconds: NonZeroU64::new(30).unwrap_or(NonZeroU64::MIN),
            logical_plan_entries: NonZeroUsize::new(plan_entries).unwrap_or(NonZeroUsize::MIN),
        }
    }

    /// Construct one fully bounded cache policy.
    ///
    /// # Errors
    ///
    /// Rejects a zero byte, TTL, or entry bound. Disabling a cache is a distinct release policy,
    /// not an accidental zero hidden in an otherwise enabled profile.
    pub fn try_new(
        metadata_cache_bytes: usize,
        file_statistics_cache_bytes: usize,
        object_list_cache_bytes: usize,
        object_list_cache_ttl_seconds: u64,
        logical_plan_entries: usize,
    ) -> Result<Self, DataFusionCachePolicyError> {
        Ok(Self {
            metadata_cache_bytes: NonZeroUsize::new(metadata_cache_bytes).ok_or(
                DataFusionCachePolicyError::ZeroBound("metadata_cache_bytes"),
            )?,
            file_statistics_cache_bytes: NonZeroUsize::new(file_statistics_cache_bytes).ok_or(
                DataFusionCachePolicyError::ZeroBound("file_statistics_cache_bytes"),
            )?,
            object_list_cache_bytes: NonZeroUsize::new(object_list_cache_bytes).ok_or(
                DataFusionCachePolicyError::ZeroBound("object_list_cache_bytes"),
            )?,
            object_list_cache_ttl_seconds: NonZeroU64::new(object_list_cache_ttl_seconds).ok_or(
                DataFusionCachePolicyError::ZeroBound("object_list_cache_ttl_seconds"),
            )?,
            logical_plan_entries: NonZeroUsize::new(logical_plan_entries).ok_or(
                DataFusionCachePolicyError::ZeroBound("logical_plan_entries"),
            )?,
        })
    }

    #[must_use]
    pub const fn metadata_cache_bytes(&self) -> usize {
        self.metadata_cache_bytes.get()
    }

    #[must_use]
    pub const fn file_statistics_cache_bytes(&self) -> usize {
        self.file_statistics_cache_bytes.get()
    }

    #[must_use]
    pub const fn object_list_cache_bytes(&self) -> usize {
        self.object_list_cache_bytes.get()
    }

    #[must_use]
    pub const fn object_list_cache_ttl_seconds(&self) -> u64 {
        self.object_list_cache_ttl_seconds.get()
    }

    #[must_use]
    pub const fn logical_plan_entries(&self) -> usize {
        self.logical_plan_entries.get()
    }

    pub(super) fn configure_runtime(&self, builder: RuntimeEnvBuilder) -> RuntimeEnvBuilder {
        builder
            .with_metadata_cache_limit(self.metadata_cache_bytes())
            .with_file_statistics_cache_limit(self.file_statistics_cache_bytes())
            .with_object_list_cache_limit(self.object_list_cache_bytes())
            .with_object_list_cache_ttl(Some(Duration::from_secs(
                self.object_list_cache_ttl_seconds(),
            )))
    }

    pub(super) fn identity_fragment(&self) -> String {
        format!(
            "metadata-cache={}:file-statistics-cache={}:object-list-cache={}:object-list-ttl-seconds={}:logical-plan-entries={}",
            self.metadata_cache_bytes(),
            self.file_statistics_cache_bytes(),
            self.object_list_cache_bytes(),
            self.object_list_cache_ttl_seconds(),
            self.logical_plan_entries(),
        )
    }
}

impl Default for DataFusionCachePolicy {
    fn default() -> Self {
        Self::proportional_to(256 * 1024 * 1024)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DataFusionCachePolicyError {
    #[error("DataFusion cache bound {0} must be non-zero")]
    ZeroBound(&'static str),
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

/// Reconstructible native plans and their causal compiler observations.
#[derive(Clone, Debug)]
pub(super) struct CachedLogicalPlan {
    compiled_plan: LogicalPlan,
    optimized_plan: LogicalPlan,
    output_schema: SchemaRef,
    observations: CompilationObservations,
    compiled_plan_digest: [u8; 32],
    optimized_plan_digest: [u8; 32],
}

impl CachedLogicalPlan {
    pub(super) fn new(
        compiled_plan: LogicalPlan,
        optimized_plan: LogicalPlan,
        output_schema: SchemaRef,
        observations: CompilationObservations,
    ) -> Self {
        let compiled_plan_digest = logical_plan_digest(COMPILED_PLAN_DIGEST_DOMAIN, &compiled_plan);
        let optimized_plan_digest =
            logical_plan_digest(OPTIMIZED_PLAN_DIGEST_DOMAIN, &optimized_plan);
        Self {
            compiled_plan,
            optimized_plan,
            output_schema,
            observations,
            compiled_plan_digest,
            optimized_plan_digest,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalPlanCacheObservation {
    pub capacity_entries: usize,
    pub resident_entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

#[derive(Debug)]
struct ResidentPlan {
    entry: Arc<CachedLogicalPlan>,
    last_used: u64,
}

#[derive(Debug, Default)]
struct LogicalPlanCacheState {
    clock: u64,
    hits: u64,
    misses: u64,
    evictions: u64,
    entries: HashMap<LogicalPlanCacheKey, ResidentPlan>,
}

/// Epoch-owned LRU for compiled and optimized logical plans.
///
/// Physical plans are intentionally excluded: DataFusion rebuilds and physically optimizes one
/// for the current task/runtime on every execution. Results are also never cached here.
#[derive(Debug)]
pub(super) struct EpochLogicalPlanCache {
    capacity: NonZeroUsize,
    state: Mutex<LogicalPlanCacheState>,
}

impl EpochLogicalPlanCache {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            capacity: NonZeroUsize::new(capacity)
                .expect("cache policy construction guarantees a non-zero capacity"),
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

    pub(super) fn insert(
        &self,
        key: LogicalPlanCacheKey,
        entry: CachedLogicalPlan,
    ) -> Arc<CachedLogicalPlan> {
        let mut state = self.lock_state();
        state.clock = state.clock.saturating_add(1);
        let clock = state.clock;
        if let Some(resident) = state.entries.get_mut(&key) {
            resident.last_used = clock;
            return Arc::clone(&resident.entry);
        }
        if state.entries.len() == self.capacity.get()
            && let Some(eviction_key) = state
                .entries
                .iter()
                .min_by_key(|(_, resident)| resident.last_used)
                .map(|(key, _)| key.clone())
        {
            state.entries.remove(&eviction_key);
            state.evictions = state.evictions.saturating_add(1);
        }
        let entry = Arc::new(entry);
        state.entries.insert(
            key,
            ResidentPlan {
                entry: Arc::clone(&entry),
                last_used: clock,
            },
        );
        entry
    }

    #[must_use]
    pub(super) fn observation(&self) -> LogicalPlanCacheObservation {
        let state = self.lock_state();
        LogicalPlanCacheObservation {
            capacity_entries: self.capacity.get(),
            resident_entries: state.entries.len(),
            hits: state.hits,
            misses: state.misses,
            evictions: state.evictions,
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

fn logical_plan_digest(domain: &[u8], plan: &LogicalPlan) -> [u8; 32] {
    let rendered = plan.display_indent_schema().to_string();
    let mut digest = blake3::Hasher::new();
    digest.update(&(domain.len() as u64).to_be_bytes());
    digest.update(domain);
    digest.update(&(rendered.len() as u64).to_be_bytes());
    digest.update(rendered.as_bytes());
    *digest.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use datafusion::logical_expr::LogicalPlanBuilder;

    use super::*;
    use crate::relational_program::{FieldId, RelationId, RelationalExpression};

    #[test]
    fn cache_policy_rejects_unbounded_zeroes() {
        assert_eq!(
            DataFusionCachePolicy::try_new(1, 1, 1, 0, 1),
            Err(DataFusionCachePolicyError::ZeroBound(
                "object_list_cache_ttl_seconds"
            ))
        );
        assert_eq!(
            DataFusionCachePolicy::try_new(1, 1, 1, 1, 0),
            Err(DataFusionCachePolicyError::ZeroBound(
                "logical_plan_entries"
            ))
        );
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

        let cache = EpochLogicalPlanCache::new(1);
        let first = key(1);
        let second = key(2);
        assert!(cache.get(&first).is_none());
        cache.insert(first.clone(), entry());
        assert!(cache.get(&first).is_some());
        cache.insert(second, entry());
        assert!(cache.get(&first).is_none());
        assert_eq!(
            cache.observation(),
            LogicalPlanCacheObservation {
                capacity_entries: 1,
                resident_entries: 1,
                hits: 1,
                misses: 2,
                evictions: 1,
            }
        );
    }
}
