//! Request-scoped, least-authority DataFusion sessions derived from a sealed epoch.
//!
//! A child is constructed from application-owned table grants and exact access,
//! query, resource, and epoch pins. Construction copies only verified provider
//! capabilities into a fresh reduced catalog graph. The returned facade owns no
//! public `SessionContext`, `SessionState`, catalog, schema, provider, runtime,
//! function-registry, or object-store-registry handle.

pub mod resource_governance;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use datafusion::catalog::{
    CatalogProvider as _, CatalogProviderList as _, MemoryCatalogProvider,
    MemoryCatalogProviderList, MemorySchemaProvider, SchemaProvider as _, Session as _,
};
use datafusion::common::tree_node::{Transformed, TreeNodeRecursion};
use datafusion::common::{DataFusionError, TableReference};
use datafusion::datasource::{provider_as_source, source_as_provider};
use datafusion::execution::memory_pool::{FairSpillPool, TrackConsumersPool};
use datafusion::execution::object_store::{ObjectStoreRegistry, ObjectStoreUrl};
use datafusion::execution::runtime_env::{RuntimeEnv, RuntimeEnvBuilder};
use datafusion::execution::{SessionState, SessionStateBuilder};
use datafusion::logical_expr::registry::FunctionRegistry as _;
use datafusion::logical_expr::{
    AggregateUDF, LogicalPlan, LogicalPlanBuilder, ScalarUDF, TableScanBuilder, TableType,
    WindowUDF,
};
use datafusion::physical_plan::collect;
use datafusion::prelude::{SessionConfig, SessionContext};
use datafusion::variable::{VarProvider, VarType};
use object_store::ObjectStore;
use url::Url;

use crate::relational_program::{
    CompilationObservations, ProgramBindings, RelationInput, RelationalProgram,
    RelationalProgramCompiler, RelationalProgramError,
};
use crate::schema_contract::SchemaContract;

use super::datafusion_cache::{
    CachedLogicalPlan, DataFusionCachePolicy, EpochLogicalPlanCache, LogicalPlanAuthorityBuilder,
    LogicalPlanAuthorityFingerprint, LogicalPlanCacheError, LogicalPlanCacheKey,
    LogicalPlanCacheObservation, LogicalPlanCacheOutcome, LogicalPlanCacheScope,
    LogicalPlanExecutionObservation, execution_observation, frame_schema_contract,
    frame_session_logical_authority, validate_logical_plan_references,
};
use super::epoch_runtime::{FABRIC_CATALOG, FabricEpochId, FabricSchemaRole};
use super::programmatic_epoch::{ProgrammaticFabricEpoch, ProgrammaticFabricEpochError};
use super::programmatic_schema::registered_view_logical_plan;
use super::programmatic_schema::{IdentityPreservingViewTable, ProgrammaticRelationId};
#[cfg(feature = "daemon")]
use super::request_owned_relation::{RequestOwnedRelationCollection, RequestOwnedRelationError};
use resource_governance::{EpochResourceCoordinator, EpochResourceError};

/// Exact authorization for one stable relation in the pinned parent epoch.
///
/// The grant carries no catalog spelling or parallel Arrow schema. Those are
/// resolved from the sealed programmatic binding selected by the epoch pin.
#[derive(Clone, Debug)]
pub struct ChildTableGrant {
    relation_id: ProgrammaticRelationId,
}

impl ChildTableGrant {
    /// Authorize one stable relation identity.
    ///
    /// # Errors
    ///
    /// Rejects an empty relation identity.
    pub fn try_new(relation_id: ProgrammaticRelationId) -> Result<Self, ChildSessionError> {
        if relation_id.as_str().trim().is_empty() {
            return Err(ChildSessionError::InvalidTableGrant(
                "relation identity is empty".into(),
            ));
        }
        Ok(Self { relation_id })
    }

    #[must_use]
    pub const fn relation_id(&self) -> &ProgrammaticRelationId {
        &self.relation_id
    }
}

/// Exact identities consumed when constructing one authorized child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChildSessionPins {
    epoch_id: FabricEpochId,
    access_scope: [u8; 32],
    query_policy: [u8; 32],
    resource_policy: [u8; 32],
}

impl ChildSessionPins {
    /// Construct exact epoch/access/query/resource pins.
    ///
    /// # Errors
    ///
    /// Rejects all-zero policy identities, which are not released references.
    pub fn try_new(
        epoch_id: FabricEpochId,
        access_scope: [u8; 32],
        query_policy: [u8; 32],
        resource_policy: [u8; 32],
    ) -> Result<Self, ChildSessionError> {
        for (kind, value) in [
            ("access", access_scope),
            ("query", query_policy),
            ("resource", resource_policy),
        ] {
            if value == [0; 32] {
                return Err(ChildSessionError::InvalidPin(kind));
            }
        }
        Ok(Self {
            epoch_id,
            access_scope,
            query_policy,
            resource_policy,
        })
    }

    #[must_use]
    pub const fn epoch_id(&self) -> FabricEpochId {
        self.epoch_id
    }

    #[must_use]
    pub const fn access_scope(&self) -> &[u8; 32] {
        &self.access_scope
    }

    #[must_use]
    pub const fn query_policy(&self) -> &[u8; 32] {
        &self.query_policy
    }

    #[must_use]
    pub const fn resource_policy(&self) -> &[u8; 32] {
        &self.resource_policy
    }
}

/// Bounded physical resources owned by one child session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChildResourceLimits {
    memory_limit_bytes: usize,
    max_spill_bytes: u64,
    max_spill_merge_fan_in: usize,
    tracked_consumer_count: NonZeroUsize,
    batch_size: NonZeroUsize,
    target_partitions: NonZeroUsize,
    cache_policy: DataFusionCachePolicy,
}

impl ChildResourceLimits {
    /// Construct a fully bounded child runtime profile.
    ///
    /// # Errors
    ///
    /// Rejects any implicit/unbounded zero value.
    pub fn try_new(
        memory_limit_bytes: usize,
        max_spill_bytes: u64,
        max_spill_merge_fan_in: usize,
        tracked_consumer_count: usize,
        batch_size: usize,
        target_partitions: usize,
    ) -> Result<Self, ChildSessionError> {
        let tracked_consumer_count = NonZeroUsize::new(tracked_consumer_count);
        let batch_size = NonZeroUsize::new(batch_size);
        let target_partitions = NonZeroUsize::new(target_partitions);
        if memory_limit_bytes == 0
            || max_spill_bytes == 0
            || max_spill_merge_fan_in == 0
            || tracked_consumer_count.is_none()
            || batch_size.is_none()
            || target_partitions.is_none()
        {
            return Err(ChildSessionError::InvalidResourceLimits(
                "memory, spill, fan-in, consumer, batch, and partition bounds must be non-zero"
                    .into(),
            ));
        }
        Ok(Self {
            memory_limit_bytes,
            max_spill_bytes,
            max_spill_merge_fan_in,
            tracked_consumer_count: tracked_consumer_count.expect("validated non-zero"),
            batch_size: batch_size.expect("validated non-zero"),
            target_partitions: target_partitions.expect("validated non-zero"),
            cache_policy: DataFusionCachePolicy::proportional_to(memory_limit_bytes),
        })
    }

    /// Replace the bounded DataFusion cache profile owned by the epoch resource domain.
    #[must_use]
    pub fn with_cache_policy(mut self, cache_policy: DataFusionCachePolicy) -> Self {
        self.cache_policy = cache_policy;
        self
    }

    #[must_use]
    pub const fn cache_policy(&self) -> &DataFusionCachePolicy {
        &self.cache_policy
    }

    fn session_config(&self) -> SessionConfig {
        SessionConfig::new()
            .with_default_catalog_and_schema(FABRIC_CATALOG, FabricSchemaRole::Public.as_str())
            .with_create_default_catalog_and_schema(false)
            .with_information_schema(false)
            .with_batch_size(self.batch_size.get())
            .with_target_partitions(self.target_partitions.get())
            .set_bool("datafusion.execution.collect_statistics", false)
            .set_bool(
                "datafusion.execution.parquet.schema_force_view_types",
                false,
            )
    }

    fn runtime_env(&self) -> Result<Arc<RuntimeEnv>, DataFusionError> {
        self.cache_policy
            .configure_runtime(RuntimeEnvBuilder::new())
            .with_object_store_registry(Arc::new(ClosedObjectStoreRegistry))
            .with_memory_pool(Arc::new(TrackConsumersPool::new(
                FairSpillPool::new(self.memory_limit_bytes),
                self.tracked_consumer_count,
            )))
            .with_max_temp_directory_size(self.max_spill_bytes)
            .with_max_spill_merge_fan_in(self.max_spill_merge_fan_in)
            .build_arc()
    }
}

/// One explicitly supplied DataFusion function and its complete allowed name set.
///
/// The expected names must equal the implementation's canonical name plus every alias. This
/// prevents a policy from authorizing one spelling while DataFusion silently installs another.
#[derive(Clone, Debug)]
pub struct ChildFunctionGrant {
    expected_names: BTreeSet<String>,
    capability: ChildFunctionCapability,
}

#[derive(Clone, Debug)]
enum ChildFunctionCapability {
    Scalar { function: Arc<ScalarUDF> },
    Aggregate { function: Arc<AggregateUDF> },
    Window { function: Arc<WindowUDF> },
}

impl ChildFunctionGrant {
    /// Authorize one exact scalar UDF implementation under its complete name set.
    ///
    /// # Errors
    ///
    /// Rejects blank, duplicate, missing, or additional canonical/alias names.
    pub fn try_scalar(
        expected_names: BTreeSet<String>,
        function: Arc<ScalarUDF>,
    ) -> Result<Self, ChildSessionError> {
        validate_function_names(
            "scalar",
            &expected_names,
            function.name(),
            function.aliases(),
        )?;
        Ok(Self {
            expected_names,
            capability: ChildFunctionCapability::Scalar { function },
        })
    }

    /// Authorize one exact aggregate UDF implementation under its complete name set.
    ///
    /// # Errors
    ///
    /// Rejects blank, duplicate, missing, or additional canonical/alias names.
    pub fn try_aggregate(
        expected_names: BTreeSet<String>,
        function: Arc<AggregateUDF>,
    ) -> Result<Self, ChildSessionError> {
        validate_function_names(
            "aggregate",
            &expected_names,
            function.name(),
            function.aliases(),
        )?;
        Ok(Self {
            expected_names,
            capability: ChildFunctionCapability::Aggregate { function },
        })
    }

    /// Authorize one exact window UDF implementation under its complete name set.
    ///
    /// # Errors
    ///
    /// Rejects blank, duplicate, missing, or additional canonical/alias names.
    pub fn try_window(
        expected_names: BTreeSet<String>,
        function: Arc<WindowUDF>,
    ) -> Result<Self, ChildSessionError> {
        validate_function_names(
            "window",
            &expected_names,
            function.name(),
            function.aliases(),
        )?;
        Ok(Self {
            expected_names,
            capability: ChildFunctionCapability::Window { function },
        })
    }

    const fn family(&self) -> &'static str {
        match &self.capability {
            ChildFunctionCapability::Scalar { .. } => "scalar",
            ChildFunctionCapability::Aggregate { .. } => "aggregate",
            ChildFunctionCapability::Window { .. } => "window",
        }
    }

    fn expected_names(&self) -> &BTreeSet<String> {
        &self.expected_names
    }
}

/// Exact multipart variable reference presented by DataFusion to a [`VarProvider`].
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ChildVariableReference(Vec<String>);

impl ChildVariableReference {
    /// Construct one exact DataFusion variable lookup key.
    ///
    /// # Errors
    ///
    /// Rejects empty keys and empty or whitespace-padded parts.
    pub fn try_new(parts: Vec<String>) -> Result<Self, ChildSessionError> {
        if parts.is_empty()
            || parts
                .iter()
                .any(|part| part.is_empty() || part.trim() != part)
        {
            return Err(ChildSessionError::InvalidRegistryAuthority("variable"));
        }
        Ok(Self(parts))
    }

    #[must_use]
    pub fn parts(&self) -> &[String] {
        &self.0
    }
}

/// One variable-provider capability restricted to exact variable lookup keys.
#[derive(Clone, Debug)]
pub struct ChildVariableProviderGrant {
    variable_type: VarType,
    variables: BTreeSet<ChildVariableReference>,
    installation: Arc<dyn VarProvider + Send + Sync>,
}

impl ChildVariableProviderGrant {
    /// Bind one DataFusion variable namespace to an explicit provider and exact key set.
    ///
    /// # Errors
    ///
    /// Rejects an empty set, a key whose `@`/`@@` prefix disagrees with `variable_type`, or a
    /// duplicate provider for the same namespace when added to an allowlist.
    pub fn try_new(
        variable_type: VarType,
        variables: BTreeSet<ChildVariableReference>,
        provider: Arc<dyn VarProvider + Send + Sync>,
    ) -> Result<Self, ChildSessionError> {
        if variables.is_empty()
            || variables.iter().any(|variable| {
                let first = variable.parts().first().map(String::as_str);
                match variable_type {
                    VarType::System => first.is_none_or(|part| !part.starts_with("@@")),
                    VarType::UserDefined => {
                        first.is_none_or(|part| !part.starts_with('@') || part.starts_with("@@"))
                    }
                }
            })
        {
            return Err(ChildSessionError::InvalidRegistryAuthority("variable"));
        }
        let installation: Arc<dyn VarProvider + Send + Sync> =
            Arc::new(AllowlistedVariableProvider {
                variables: variables.clone(),
                delegate: provider,
            });
        Ok(Self {
            variable_type,
            variables,
            installation,
        })
    }

    #[must_use]
    pub const fn variable_type(&self) -> &VarType {
        &self.variable_type
    }

    pub fn variables(
        &self,
    ) -> impl ExactSizeIterator<Item = &ChildVariableReference> + DoubleEndedIterator {
        self.variables.iter()
    }
}

/// One exact, host-bearing object-store origin and its explicitly supplied capability.
#[derive(Clone)]
pub struct ChildObjectStoreGrant {
    origin: ObjectStoreUrl,
    store: Arc<dyn ObjectStore>,
}

impl fmt::Debug for ChildObjectStoreGrant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChildObjectStoreGrant")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

impl ChildObjectStoreGrant {
    /// Bind one exact origin to an explicitly supplied object-store implementation.
    ///
    /// # Errors
    ///
    /// Rejects hostless origins (including `file://`) and embedded credentials. DataFusion's
    /// registry key is origin-wide, so accepting either would grant authority broader than this
    /// boundary can truthfully prove.
    pub fn try_new(
        origin: ObjectStoreUrl,
        store: Arc<dyn ObjectStore>,
    ) -> Result<Self, ChildSessionError> {
        let url: &Url = origin.as_ref();
        if url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
            return Err(ChildSessionError::InvalidRegistryAuthority("object-store"));
        }
        Ok(Self { origin, store })
    }

    #[must_use]
    pub const fn origin(&self) -> &ObjectStoreUrl {
        &self.origin
    }
}

/// Explicit capabilities installed into fresh child-owned DataFusion registries.
///
/// Implementations are supplied directly; no name is resolved through or copied from the parent
/// session. Construction validates collision-free names and namespace/origin uniqueness before
/// the child state exists.
#[derive(Clone, Debug, Default)]
pub struct ChildRegistryAllowlist {
    functions: Vec<ChildFunctionGrant>,
    variables: Vec<ChildVariableProviderGrant>,
    object_stores: BTreeMap<ObjectStoreUrl, ChildObjectStoreGrant>,
}

impl ChildRegistryAllowlist {
    /// Construct one complete child registry installation plan.
    ///
    /// # Errors
    ///
    /// Rejects function-name collisions within a DataFusion function family, more than one
    /// provider for either variable namespace, or duplicate object-store origins.
    pub fn try_new(
        functions: impl IntoIterator<Item = ChildFunctionGrant>,
        variables: impl IntoIterator<Item = ChildVariableProviderGrant>,
        object_stores: impl IntoIterator<Item = ChildObjectStoreGrant>,
    ) -> Result<Self, ChildSessionError> {
        let functions = functions.into_iter().collect::<Vec<_>>();
        let mut scalar_names = BTreeSet::new();
        let mut aggregate_names = BTreeSet::new();
        let mut window_names = BTreeSet::new();
        for function in &functions {
            let names = match &function.capability {
                ChildFunctionCapability::Scalar { .. } => &mut scalar_names,
                ChildFunctionCapability::Aggregate { .. } => &mut aggregate_names,
                ChildFunctionCapability::Window { .. } => &mut window_names,
            };
            for name in function.expected_names() {
                if !names.insert(name.clone()) {
                    return Err(ChildSessionError::DuplicateRegistryAuthority {
                        kind: function.family(),
                        name: name.clone(),
                    });
                }
            }
        }

        let variables = variables.into_iter().collect::<Vec<_>>();
        for variable_type in [VarType::System, VarType::UserDefined] {
            if variables
                .iter()
                .filter(|grant| grant.variable_type == variable_type)
                .count()
                > 1
            {
                return Err(ChildSessionError::DuplicateRegistryAuthority {
                    kind: "variable",
                    name: format!("{variable_type:?}"),
                });
            }
        }

        let mut by_origin = BTreeMap::new();
        for grant in object_stores {
            let origin = grant.origin.clone();
            if by_origin.insert(origin.clone(), grant).is_some() {
                return Err(ChildSessionError::DuplicateRegistryAuthority {
                    kind: "object-store",
                    name: origin.to_string(),
                });
            }
        }

        Ok(Self {
            functions,
            variables,
            object_stores: by_origin,
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty() && self.variables.is_empty() && self.object_stores.is_empty()
    }
}

/// Complete immutable policy input for one child construction.
#[derive(Clone, Debug)]
pub struct ChildSessionPolicy {
    pins: ChildSessionPins,
    tables: BTreeMap<ProgrammaticRelationId, ChildTableGrant>,
    resources: ChildResourceLimits,
    max_output_rows: NonZeroUsize,
    registries: ChildRegistryAllowlist,
}

impl ChildSessionPolicy {
    /// Construct a policy from exact grants and bounded resources.
    ///
    /// # Errors
    ///
    /// Rejects duplicate table grants or a zero output-row bound.
    pub fn try_new(
        pins: ChildSessionPins,
        tables: impl IntoIterator<Item = ChildTableGrant>,
        resources: ChildResourceLimits,
        max_output_rows: usize,
        registries: ChildRegistryAllowlist,
    ) -> Result<Self, ChildSessionError> {
        let max_output_rows = NonZeroUsize::new(max_output_rows).ok_or_else(|| {
            ChildSessionError::InvalidResourceLimits("max_output_rows must be non-zero".into())
        })?;
        let mut by_table = BTreeMap::new();
        for grant in tables {
            let key = grant.relation_id.clone();
            if by_table.insert(key.clone(), grant).is_some() {
                return Err(ChildSessionError::DuplicateTableGrant(key));
            }
        }
        Ok(Self {
            pins,
            tables: by_table,
            resources,
            max_output_rows,
            registries,
        })
    }

    #[must_use]
    pub const fn pins(&self) -> &ChildSessionPins {
        &self.pins
    }
}

/// One bounded scan accepted by the sealed child facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChildTableScan {
    relation_id: ProgrammaticRelationId,
    projection: Option<Vec<usize>>,
    limit: Option<usize>,
}

impl ChildTableScan {
    #[must_use]
    pub const fn all(relation_id: ProgrammaticRelationId) -> Self {
        Self {
            relation_id,
            projection: None,
            limit: None,
        }
    }

    #[must_use]
    pub fn with_projection(mut self, projection: Vec<usize>) -> Self {
        self.projection = Some(projection);
        self
    }

    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Arrow result that retains the validated logical schema when execution emits
/// zero batches or a zero-row batch.
#[derive(Clone, Debug)]
pub struct ChildQueryResult {
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
    truncated: bool,
}

impl ChildQueryResult {
    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }

    #[must_use]
    pub fn row_count(&self) -> usize {
        self.batches.iter().map(RecordBatch::num_rows).sum()
    }

    /// Whether an independently observed overflow-probe row proved that more
    /// rows existed than the caller's exact bound allowed returning.
    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.truncated
    }
}

/// Exact output and causal compiler observations from one authorized relational program.
///
/// Unlike [`ChildQueryResult`], this result is never semantically truncated. A program whose
/// observed output exceeds the child resource envelope fails before a result is returned.
#[derive(Clone, Debug)]
pub struct ChildProgramResult {
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
    observations: CompilationObservations,
    plan: LogicalPlanExecutionObservation,
    cache_use: ChildProgramCacheUse,
}

/// Exact shared logical-plan-cache disposition for one child program execution.
///
/// Request-owned providers are not epoch-stable capabilities. Until a cache key and retained-plan
/// validator include their full concrete authority, their plans are compiled and optimized anew
/// and represented as an explicit bypass rather than a synthetic cache miss.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildProgramCacheUse {
    SharedCache(LogicalPlanCacheOutcome),
    #[cfg(feature = "daemon")]
    RequestOwnedAuthorityBypass,
}

impl ChildProgramResult {
    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }

    #[must_use]
    pub fn row_count(&self) -> usize {
        self.batches.iter().map(RecordBatch::num_rows).sum()
    }

    /// Causal model dependencies and native DataFusion selections observed by compilation.
    #[must_use]
    pub const fn observations(&self) -> &CompilationObservations {
        &self.observations
    }

    #[must_use]
    pub const fn plan_observation(&self) -> LogicalPlanExecutionObservation {
        self.plan
    }

    /// Whether the epoch cache was used, missed, or deliberately bypassed for query-local input.
    #[must_use]
    pub const fn cache_use(&self) -> ChildProgramCacheUse {
        self.cache_use
    }
}

/// Read-only child resource observations that do not expose runtime authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChildResourceObservation {
    pub memory_limit_bytes: usize,
    pub memory_reserved_bytes: usize,
    pub max_spill_bytes: u64,
    pub spilled_bytes: u64,
    pub active_spill_files: usize,
    pub batch_size: usize,
    pub target_partitions: usize,
    pub max_output_rows: usize,
    pub metadata_cache_limit_bytes: usize,
    pub file_statistics_cache_limit_bytes: usize,
    pub object_list_cache_limit_bytes: usize,
    pub object_list_cache_ttl_seconds: Option<u64>,
    pub logical_plan_cache_capacity_entries: usize,
}

#[derive(Clone, Debug)]
struct SealedTableContract {
    table_reference: TableReference,
    contract: Arc<SchemaContract>,
    provider: Arc<dyn datafusion::catalog::TableProvider>,
    logical_plan: Option<Arc<LogicalPlan>>,
}

fn rebuild_child_provider_graph(
    parent_tables: &BTreeMap<ProgrammaticRelationId, SealedTableContract>,
    relations_by_reference: &BTreeMap<TableReference, ProgrammaticRelationId>,
) -> Result<
    BTreeMap<ProgrammaticRelationId, Arc<dyn datafusion::catalog::TableProvider>>,
    ChildSessionError,
> {
    let mut rebuilt = BTreeMap::new();
    let mut active = BTreeSet::new();
    for relation_id in parent_tables.keys() {
        rebuild_child_provider(
            relation_id,
            parent_tables,
            relations_by_reference,
            &mut rebuilt,
            &mut active,
        )?;
    }
    Ok(rebuilt)
}

fn rebuild_child_provider(
    relation_id: &ProgrammaticRelationId,
    parent_tables: &BTreeMap<ProgrammaticRelationId, SealedTableContract>,
    relations_by_reference: &BTreeMap<TableReference, ProgrammaticRelationId>,
    rebuilt: &mut BTreeMap<ProgrammaticRelationId, Arc<dyn datafusion::catalog::TableProvider>>,
    active: &mut BTreeSet<ProgrammaticRelationId>,
) -> Result<Arc<dyn datafusion::catalog::TableProvider>, ChildSessionError> {
    if let Some(provider) = rebuilt.get(relation_id) {
        return Ok(Arc::clone(provider));
    }
    if !active.insert(relation_id.clone()) {
        return Err(ChildSessionError::ProviderDependencyCycle(
            relation_id.clone(),
        ));
    }

    let parent = parent_tables
        .get(relation_id)
        .expect("provider rebuild starts from an exact granted relation");
    let parent_provider = Arc::clone(&parent.provider);
    let table_type = parent_provider.table_type();
    let definition = parent_provider.get_table_definition().map(str::to_owned);
    let logical_plan = parent
        .logical_plan
        .as_ref()
        .map(|plan| plan.as_ref().clone());

    let result = if let Some(logical_plan) = logical_plan {
        let dependencies = provider_plan_dependencies(&logical_plan)?;
        let mut child_dependencies = BTreeMap::new();
        for (dependency_reference, retained_provider) in dependencies {
            let dependency_relation = relations_by_reference
                .get(&dependency_reference)
                .ok_or_else(|| ChildSessionError::DeniedProviderDependency {
                    relation_id: relation_id.clone(),
                    dependency: dependency_reference.clone(),
                })?;
            let expected_parent = &parent_tables
                .get(dependency_relation)
                .expect("reference index contains only granted relations")
                .provider;
            if !Arc::ptr_eq(&retained_provider, expected_parent) {
                return Err(ChildSessionError::ProviderDependencyCapabilityMismatch {
                    relation_id: relation_id.clone(),
                    dependency: dependency_reference,
                });
            }
            let child_provider = rebuild_child_provider(
                dependency_relation,
                parent_tables,
                relations_by_reference,
                rebuilt,
                active,
            )?;
            child_dependencies.insert(dependency_reference, child_provider);
        }

        if table_type == TableType::View {
            let plan = rebind_child_view_plan(relation_id, logical_plan, &child_dependencies)?;
            Arc::new(IdentityPreservingViewTable::with_definition(
                plan, definition,
            )) as Arc<dyn datafusion::catalog::TableProvider>
        } else {
            parent_provider
        }
    } else if table_type == TableType::View {
        return Err(ChildSessionError::ProviderLogicalPlanUnavailable(
            relation_id.clone(),
        ));
    } else {
        parent_provider
    };

    active.remove(relation_id);
    let expected_schema = parent_tables
        .get(relation_id)
        .expect("provider rebuild starts from an exact granted relation")
        .contract
        .logical_schema();
    if result.schema().as_ref() != expected_schema.as_ref() {
        return Err(ChildSessionError::ParentProviderCapabilityMismatch(
            relation_id.clone(),
        ));
    }
    rebuilt.insert(relation_id.clone(), Arc::clone(&result));
    Ok(result)
}

fn provider_plan_dependencies(
    plan: &LogicalPlan,
) -> Result<Vec<(TableReference, Arc<dyn datafusion::catalog::TableProvider>)>, ChildSessionError> {
    let mut dependencies = Vec::new();
    plan.apply_with_subqueries(|node| {
        if let LogicalPlan::TableScan(scan) = node {
            dependencies.push((scan.table_name.clone(), source_as_provider(&scan.source)?));
        }
        Ok(TreeNodeRecursion::Continue)
    })?;
    Ok(dependencies)
}

fn rebind_child_view_plan(
    relation_id: &ProgrammaticRelationId,
    plan: LogicalPlan,
    child_dependencies: &BTreeMap<TableReference, Arc<dyn datafusion::catalog::TableProvider>>,
) -> Result<LogicalPlan, ChildSessionError> {
    Ok(plan
        .transform_up_with_subqueries(|node| {
            let LogicalPlan::TableScan(scan) = node else {
                return Ok(Transformed::no(node));
            };
            let dependency_reference = scan.table_name.clone();
            let provider = child_dependencies
                .get(&dependency_reference)
                .ok_or_else(|| {
                    DataFusionError::Plan(format!(
                        "view {} retained unproved dependency {dependency_reference}",
                        relation_id.as_str()
                    ))
                })?;
            let expected_projected_schema = Arc::clone(&scan.projected_schema);
            let rebuilt = TableScanBuilder::new(
                dependency_reference,
                provider_as_source(Arc::clone(provider)),
            )
            .with_projection(scan.projection)
            .with_filters(scan.filters)
            .with_fetch(scan.fetch)
            .with_statistics_requests(scan.statistics_requests)
            .build()?;
            if rebuilt.projected_schema.as_ref() != expected_projected_schema.as_ref() {
                return Err(DataFusionError::Plan(format!(
                    "view {} dependency schema changed while rebinding into the child",
                    relation_id.as_str()
                )));
            }
            Ok(Transformed::yes(LogicalPlan::TableScan(rebuilt)))
        })?
        .data)
}

fn validate_child_provider_graph(
    tables: &BTreeMap<ProgrammaticRelationId, SealedTableContract>,
    state: &SessionState,
) -> Result<(), ChildSessionError> {
    let relations_by_reference = tables
        .iter()
        .map(|(relation_id, table)| (table.table_reference.clone(), relation_id.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut validated = BTreeSet::new();
    let mut active = BTreeSet::new();
    for relation_id in tables.keys() {
        validate_child_provider(
            relation_id,
            tables,
            &relations_by_reference,
            state,
            &mut validated,
            &mut active,
        )?;
    }
    Ok(())
}

fn validate_child_provider(
    relation_id: &ProgrammaticRelationId,
    tables: &BTreeMap<ProgrammaticRelationId, SealedTableContract>,
    relations_by_reference: &BTreeMap<TableReference, ProgrammaticRelationId>,
    state: &SessionState,
    validated: &mut BTreeSet<ProgrammaticRelationId>,
    active: &mut BTreeSet<ProgrammaticRelationId>,
) -> Result<(), ChildSessionError> {
    if validated.contains(relation_id) {
        return Ok(());
    }
    if !active.insert(relation_id.clone()) {
        return Err(ChildSessionError::ProviderDependencyCycle(
            relation_id.clone(),
        ));
    }
    let table = tables
        .get(relation_id)
        .expect("provider validation starts from an exact child relation");
    let logical_plan = table
        .logical_plan
        .as_ref()
        .map(|plan| plan.as_ref().clone());
    if let Some(logical_plan) = logical_plan {
        let mut dependencies = BTreeSet::new();
        validate_logical_plan_references(&logical_plan, state, false, |scan| {
            let dependency_relation = relations_by_reference
                .get(&scan.table_name)
                .ok_or_else(|| format!("ungranted table dependency {}", scan.table_name))?;
            let expected_provider = &tables
                .get(dependency_relation)
                .expect("reference index contains only child relations")
                .provider;
            let retained_provider = source_as_provider(&scan.source).map_err(|error| {
                format!(
                    "dependency {} has a non-provider table source: {error}",
                    scan.table_name
                )
            })?;
            if !Arc::ptr_eq(&retained_provider, expected_provider) {
                return Err(format!(
                    "dependency {} retained a parent or substituted provider capability",
                    scan.table_name
                ));
            }
            dependencies.insert(dependency_relation.clone());
            Ok(())
        })
        .map_err(|detail| ChildSessionError::ProviderLogicalPlanClosure {
            relation_id: relation_id.clone(),
            detail,
        })?;
        for dependency in dependencies {
            validate_child_provider(
                &dependency,
                tables,
                relations_by_reference,
                state,
                validated,
                active,
            )?;
        }
    } else if table.provider.table_type() == TableType::View {
        return Err(ChildSessionError::ProviderLogicalPlanUnavailable(
            relation_id.clone(),
        ));
    }
    active.remove(relation_id);
    validated.insert(relation_id.clone());
    Ok(())
}

/// Read-only contract observation for one table admitted to a child session.
#[derive(Clone, Copy, Debug)]
pub struct ChildTableContractObservation<'a> {
    relation_id: &'a ProgrammaticRelationId,
    table_reference: &'a TableReference,
    contract: &'a Arc<SchemaContract>,
}

impl ChildTableContractObservation<'_> {
    #[must_use]
    pub const fn relation_id(&self) -> &ProgrammaticRelationId {
        self.relation_id
    }

    #[must_use]
    pub const fn table_reference(&self) -> &TableReference {
        self.table_reference
    }

    #[must_use]
    pub fn source_schema_identity(&self) -> &str {
        self.contract.source_schema_identity()
    }

    #[must_use]
    pub fn logical_schema(&self) -> &SchemaRef {
        self.contract.logical_schema()
    }
}

/// Sealed query-only child session.
///
/// The only executable operation is a bounded scan over an application-owned
/// granted table reference. The concrete state and every registration handle
/// remain private.
///
/// ```compile_fail
/// use codefabric::fabric::child_session::AuthorizedChildSession;
/// fn cannot_register(session: &AuthorizedChildSession) {
///     session.register_table();
/// }
/// ```
pub struct AuthorizedChildSession {
    pins: ChildSessionPins,
    program_bindings: Arc<ProgramBindings>,
    resources: ChildResourceLimits,
    max_output_rows: NonZeroUsize,
    tables: BTreeMap<ProgrammaticRelationId, SealedTableContract>,
    state: SessionState,
    table_versions: super::activation::TableVersionSetRef,
    runtime_configuration: Arc<str>,
    logical_plan_authority: LogicalPlanAuthorityFingerprint,
    logical_plan_cache: Arc<EpochLogicalPlanCache>,
}

impl fmt::Debug for AuthorizedChildSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthorizedChildSession")
            .field("pins", &self.pins)
            .field("table_count", &self.tables.len())
            .field("max_output_rows", &self.max_output_rows)
            .finish_non_exhaustive()
    }
}

fn derive_child_logical_plan_authority(
    epoch: &ProgrammaticFabricEpoch,
    policy: &ChildSessionPolicy,
    tables: &BTreeMap<ProgrammaticRelationId, SealedTableContract>,
    state: &SessionState,
) -> Result<LogicalPlanAuthorityFingerprint, ChildSessionError> {
    let mut authority = LogicalPlanAuthorityBuilder::new(b"authorized-child-session-authority.v1");
    authority.frame(epoch.logical_plan_authority().as_bytes());
    authority.frame(policy.pins.epoch_id.as_bytes());
    authority.frame(&policy.pins.access_scope);
    authority.frame(&policy.pins.query_policy);
    authority.frame(&policy.pins.resource_policy);
    authority.frame_str(epoch.program_bindings().authority_id());
    authority.frame(epoch.table_version_set_ref().as_bytes());

    authority.frame_usize(tables.len());
    for (relation_id, table) in tables {
        frame_schema_contract(&mut authority, relation_id.as_str(), &table.contract)
            .map_err(ChildSessionError::LogicalPlanAuthority)?;
        authority.frame_str(&table.table_reference.to_string());
        authority.frame_arc_identity(&table.provider);
    }

    authority.frame_usize(policy.resources.memory_limit_bytes);
    authority.frame_u64(policy.resources.max_spill_bytes);
    authority.frame_usize(policy.resources.max_spill_merge_fan_in);
    authority.frame_usize(policy.resources.tracked_consumer_count.get());
    authority.frame_usize(policy.resources.batch_size.get());
    authority.frame_usize(policy.resources.target_partitions.get());
    authority.frame_str(&policy.resources.cache_policy.identity_fragment());
    authority.frame_usize(policy.max_output_rows.get());

    frame_session_logical_authority(
        &mut authority,
        state,
        "datafusion::physical_planner::DefaultPhysicalPlanner@55.0.0",
    );

    let mut variables = policy.registries.variables.iter().collect::<Vec<_>>();
    variables.sort_by_key(|grant| variable_type_identity(&grant.variable_type));
    authority.frame_usize(variables.len());
    for grant in variables {
        authority.frame_str(variable_type_identity(&grant.variable_type));
        authority.frame_usize(grant.variables.len());
        for variable in &grant.variables {
            authority.frame_usize(variable.parts().len());
            for part in variable.parts() {
                authority.frame_str(part);
            }
        }
        let installed = state
            .execution_props()
            .get_var_provider(grant.variable_type.clone())
            .ok_or_else(|| ChildSessionError::RegistryInstallationDrift {
                kind: "variable",
                name: format!("{:?}", grant.variable_type),
            })?;
        authority.frame_arc_identity(&installed);
    }

    authority.frame_usize(policy.registries.object_stores.len());
    for origin in policy.registries.object_stores.keys() {
        authority.frame_str(&origin.to_string());
        let installed = state
            .runtime_env()
            .object_store_registry
            .get_store(origin.as_ref())?;
        authority.frame_arc_identity(&installed);
    }
    Ok(authority.finish())
}

const fn variable_type_identity(variable_type: &VarType) -> &'static str {
    match variable_type {
        VarType::System => "system",
        VarType::UserDefined => "user-defined",
    }
}

impl AuthorizedChildSession {
    async fn try_from_epoch(
        epoch: &ProgrammaticFabricEpoch,
        policy: ChildSessionPolicy,
        resources: &EpochResourceCoordinator,
    ) -> Result<Self, ChildSessionError> {
        if policy.pins.epoch_id != *epoch.identity() {
            return Err(ChildSessionError::EpochPinMismatch {
                expected: *epoch.identity(),
                actual: policy.pins.epoch_id,
            });
        }
        let resource_runtime = resources.child_runtime_env(
            *epoch.identity(),
            policy.pins.resource_policy(),
            &policy.resources,
        )?;
        let object_store_registry: Arc<dyn ObjectStoreRegistry> = Arc::new(
            AllowlistedObjectStoreRegistry::from_grants(&policy.registries.object_stores),
        );
        let runtime = RuntimeEnvBuilder::from_runtime_env(&resource_runtime)
            .with_object_store_registry(Arc::clone(&object_store_registry))
            .build_arc()?;
        if !Arc::ptr_eq(&runtime.object_store_registry, &object_store_registry) {
            return Err(ChildSessionError::RegistryInstallationDrift {
                kind: "object-store-registry",
                name: "child runtime".into(),
            });
        }
        validate_object_store_installations(
            runtime.object_store_registry.as_ref(),
            &policy.registries.object_stores,
        )?;

        // Resolve and validate every grant before creating the reduced catalog. View plans retain
        // concrete provider Arcs, so their complete dependency graph must be proved against this
        // exact grant set and rebound bottom-up before any child table becomes visible.
        let mut parent_tables = BTreeMap::new();
        let mut relations_by_reference = BTreeMap::new();
        for relation_id in policy.tables.keys() {
            let (table_reference, provider, parent_contract, logical_plan) =
                epoch.resolve_sealed_relation(relation_id).await?;
            if full_table_parts(&table_reference).is_none() {
                return Err(ChildSessionError::CatalogClosure(format!(
                    "sealed relation {} has non-full table reference {table_reference}",
                    relation_id.as_str()
                )));
            }
            if parent_contract.qualifier() != &table_reference
                || provider.schema().as_ref() != parent_contract.logical_schema().as_ref()
            {
                return Err(ChildSessionError::ParentSchemaContractMismatch(
                    relation_id.clone(),
                ));
            }
            let contract_relation_id = parent_contract
                .relation_id(crate::schema_contract::SchemaRole::Logical)
                .map_err(|error| {
                    ChildSessionError::CatalogClosure(format!(
                        "sealed relation {} has no executable identity: {error}",
                        relation_id.as_str()
                    ))
                })?;
            if contract_relation_id != relation_id.as_str() {
                return Err(ChildSessionError::ParentSchemaContractMismatch(
                    relation_id.clone(),
                ));
            }
            if relations_by_reference
                .insert(table_reference.clone(), relation_id.clone())
                .is_some()
            {
                return Err(ChildSessionError::CatalogClosure(format!(
                    "sealed table reference {table_reference} is granted more than once"
                )));
            }
            parent_tables.insert(
                relation_id.clone(),
                SealedTableContract {
                    table_reference,
                    contract: parent_contract,
                    provider,
                    logical_plan,
                },
            );
        }
        let mut child_providers =
            rebuild_child_provider_graph(&parent_tables, &relations_by_reference)?;

        let catalog_list = Arc::new(MemoryCatalogProviderList::new());
        let mut catalogs = BTreeMap::<String, Arc<MemoryCatalogProvider>>::new();
        let mut schemas = BTreeMap::<(String, String), Arc<MemorySchemaProvider>>::new();
        let mut contracts = BTreeMap::new();
        for (relation_id, parent) in parent_tables {
            let provider = child_providers
                .remove(&relation_id)
                .expect("every granted provider was rebuilt or admitted as an opaque leaf");
            let table_reference = parent.table_reference;
            let parent_contract = parent.contract;
            let (catalog_name, schema_name, table_name) = full_table_parts(&table_reference)
                .expect("full table references were validated before provider rebuilding");

            let catalog = if let Some(catalog) = catalogs.get(catalog_name) {
                Arc::clone(catalog)
            } else {
                let catalog = Arc::new(MemoryCatalogProvider::new());
                if catalog_list
                    .register_catalog(catalog_name.to_owned(), Arc::clone(&catalog) as _)
                    .is_some()
                {
                    return Err(ChildSessionError::CatalogClosure(format!(
                        "fresh child catalog {catalog_name} was replaced"
                    )));
                }
                catalogs.insert(catalog_name.to_owned(), Arc::clone(&catalog));
                catalog
            };
            let schema_key = (catalog_name.to_owned(), schema_name.to_owned());
            let schema = if let Some(schema) = schemas.get(&schema_key) {
                Arc::clone(schema)
            } else {
                let schema = Arc::new(MemorySchemaProvider::new());
                if catalog
                    .register_schema(schema_name, Arc::clone(&schema) as _)?
                    .is_some()
                {
                    return Err(ChildSessionError::CatalogClosure(format!(
                        "fresh child schema {catalog_name}.{schema_name} was replaced"
                    )));
                }
                schemas.insert(schema_key, Arc::clone(&schema));
                schema
            };
            if schema
                .register_table(table_name.to_owned(), Arc::clone(&provider))?
                .is_some()
            {
                return Err(ChildSessionError::CatalogClosure(format!(
                    "fresh child table {table_reference} was replaced"
                )));
            }
            let installed = schema.table(table_name).await?.ok_or_else(|| {
                ChildSessionError::CatalogClosure(format!(
                    "child provider {table_reference} disappeared during construction"
                ))
            })?;
            if !Arc::ptr_eq(&provider, &installed)
                || installed.schema().as_ref() != parent_contract.logical_schema().as_ref()
            {
                return Err(ChildSessionError::ParentProviderCapabilityMismatch(
                    relation_id.clone(),
                ));
            }
            let logical_plan = registered_view_logical_plan(provider.as_ref()).map(Arc::new);
            contracts.insert(
                relation_id,
                SealedTableContract {
                    table_reference,
                    contract: parent_contract,
                    provider,
                    logical_plan,
                },
            );
        }

        validate_reduced_catalog(&catalog_list, &catalogs, &schemas, &contracts)?;
        let catalog_authority: Arc<dyn datafusion::catalog::CatalogProviderList> =
            Arc::clone(&catalog_list) as _;
        if !epoch.child_authorities_are_distinct(&runtime, &catalog_authority) {
            return Err(ChildSessionError::ParentAuthorityRetained);
        }

        // `new()` is intentionally used without `with_default_features()`:
        // no built-in functions, file formats, table factories, expression
        // planners, or canonical extension types cross this boundary.
        let mut state = SessionStateBuilder::new()
            .with_config(policy.resources.session_config())
            .with_runtime_env(Arc::clone(&runtime))
            .with_catalog_list(catalog_authority)
            .build();
        validate_empty_registries(&state)?;
        install_allowlisted_registries(&mut state, &policy.registries)?;
        validate_allowlisted_registries(&state, &policy.registries)?;
        validate_child_provider_graph(&contracts, &state)?;
        let logical_plan_authority =
            derive_child_logical_plan_authority(epoch, &policy, &contracts, &state)?;

        Ok(Self {
            pins: policy.pins,
            program_bindings: Arc::clone(epoch.program_bindings()),
            resources: policy.resources,
            max_output_rows: policy.max_output_rows,
            tables: contracts,
            state,
            table_versions: epoch.table_version_set_ref(),
            runtime_configuration: Arc::from(epoch.runtime_configuration_identity()),
            logical_plan_authority,
            logical_plan_cache: Arc::clone(epoch.logical_plan_cache()),
        })
    }

    #[must_use]
    pub const fn pins(&self) -> &ChildSessionPins {
        &self.pins
    }

    /// Enumerate only granted stable relation identities. No DataFusion
    /// catalog/schema/provider handle is returned.
    pub fn allowed_tables(
        &self,
    ) -> impl ExactSizeIterator<Item = &ProgrammaticRelationId> + DoubleEndedIterator {
        self.tables.keys()
    }

    #[must_use]
    pub fn logical_plan_cache_observation(&self) -> LogicalPlanCacheObservation {
        self.logical_plan_cache.observation()
    }

    /// Enumerate the exact parent schema contracts that causally admitted each table.
    ///
    /// The observation contains identity and Arrow type metadata only; it does not expose a
    /// provider, catalog, session, runtime, or mutation capability.
    pub fn table_contracts(
        &self,
    ) -> impl ExactSizeIterator<Item = ChildTableContractObservation<'_>> + DoubleEndedIterator
    {
        self.tables
            .iter()
            .map(|(relation_id, sealed)| ChildTableContractObservation {
                relation_id,
                table_reference: &sealed.table_reference,
                contract: &sealed.contract,
            })
    }

    /// Observe bounded resources without exposing a mutable `RuntimeEnv`.
    #[must_use]
    pub fn resource_observation(&self) -> ChildResourceObservation {
        let runtime = self.state.runtime_env();
        let spilling = runtime.spilling_progress();
        ChildResourceObservation {
            memory_limit_bytes: self.resources.memory_limit_bytes,
            memory_reserved_bytes: runtime.memory_pool.reserved(),
            max_spill_bytes: self.resources.max_spill_bytes,
            spilled_bytes: spilling.current_bytes,
            active_spill_files: spilling.active_files_count,
            batch_size: self.resources.batch_size.get(),
            target_partitions: self.resources.target_partitions.get(),
            max_output_rows: self.max_output_rows.get(),
            metadata_cache_limit_bytes: runtime.cache_manager.get_metadata_cache_limit(),
            file_statistics_cache_limit_bytes: runtime
                .cache_manager
                .get_file_statistic_cache_limit(),
            object_list_cache_limit_bytes: runtime.cache_manager.get_list_files_cache_limit(),
            object_list_cache_ttl_seconds: runtime
                .cache_manager
                .get_list_files_cache_ttl()
                .map(|ttl| ttl.as_secs()),
            logical_plan_cache_capacity_entries: self.resources.cache_policy.logical_plan_entries(),
        }
    }

    /// Execute one bounded projection against an allowed child table.
    ///
    /// # Errors
    ///
    /// Rejects denied tables, invalid/duplicate projections, limits above the
    /// pinned query bound, planning/execution failures, and output-schema drift.
    pub(crate) async fn scan(
        &self,
        request: &ChildTableScan,
    ) -> Result<ChildQueryResult, ChildSessionError> {
        let contract = self
            .tables
            .get(&request.relation_id)
            .ok_or_else(|| ChildSessionError::DeniedTable(request.relation_id.clone()))?;
        let effective_limit = request.limit.unwrap_or(self.max_output_rows.get());
        if effective_limit > self.max_output_rows.get() {
            return Err(ChildSessionError::OutputRowLimitExceeded {
                requested: effective_limit,
                maximum: self.max_output_rows.get(),
            });
        }

        let expected_schema = if let Some(projection) = request.projection.as_deref() {
            let unique = projection.iter().copied().collect::<BTreeSet<_>>();
            if unique.len() != projection.len() {
                return Err(ChildSessionError::DuplicateProjectionIndex);
            }
            Arc::new(contract.contract.logical_schema().project(projection)?)
        } else {
            Arc::clone(contract.contract.logical_schema())
        };
        let mut frame = self
            .context()
            .table(contract.table_reference.clone())
            .await?;
        if let Some(projection) = request.projection.as_deref() {
            let names = projection
                .iter()
                .map(|index| {
                    contract
                        .contract
                        .logical_schema()
                        .fields()
                        .get(*index)
                        .map(|field| field.name().as_str())
                        .ok_or_else(|| ChildSessionError::InvalidProjectionIndex {
                            relation_id: request.relation_id.clone(),
                            index: *index,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            frame = frame.select_columns(&names)?;
        }
        let probe_limit = effective_limit
            .checked_add(1)
            .ok_or(ChildSessionError::OutputRowProbeOverflow)?;
        let probed = frame.limit(0, Some(probe_limit))?.collect().await?;
        let probed_row_count = probed.iter().map(RecordBatch::num_rows).sum::<usize>();
        let truncated = probed_row_count > effective_limit;
        let batches = truncate_batches(probed, effective_limit);
        for batch in &batches {
            if batch.schema_ref().as_ref() != expected_schema.as_ref() {
                return Err(ChildSessionError::QuerySchemaDrift {
                    relation_id: request.relation_id.clone(),
                    expected: Arc::clone(&expected_schema),
                    actual: batch.schema(),
                });
            }
        }
        Ok(ChildQueryResult {
            schema: expected_schema,
            batches,
            truncated,
        })
    }

    /// Compile and execute one relational program exclusively through this reduced child.
    ///
    /// Every input resolved by the exact sealed [`ProgramBindings`] must already be present in the
    /// child grant set and must resolve to the same fully qualified table. Concrete scans and final
    /// execution use the child `SessionContext`; the sealed parent session is never consulted. A
    /// one-row overflow probe bounds materialization without silently truncating program meaning.
    ///
    /// # Errors
    ///
    /// Rejects any input absent from the child catalog, binding/table drift, relational typing or
    /// planning failure, output-schema drift, and output above the pinned row envelope.
    pub(crate) async fn execute_relational_program(
        &self,
        program: &RelationalProgram,
    ) -> Result<ChildProgramResult, ChildSessionError> {
        let context = self.context();
        let probe_limit = self
            .max_output_rows
            .get()
            .checked_add(1)
            .ok_or(ChildSessionError::OutputRowProbeOverflow)?;
        let allowed_relations = self
            .tables
            .keys()
            .map(|relation_id| Arc::<str>::from(relation_id.as_str()))
            .collect::<Vec<_>>();
        let cache_key = LogicalPlanCacheKey::new(
            self.pins.epoch_id,
            self.table_versions,
            self.program_bindings.authority_id(),
            Arc::clone(&self.runtime_configuration),
            self.logical_plan_authority,
            LogicalPlanCacheScope::Authorized {
                access_scope: self.pins.access_scope,
                query_policy: self.pins.query_policy,
                resource_policy: self.pins.resource_policy,
                allowed_relations: Arc::from(allowed_relations),
                max_output_rows: self.max_output_rows.get(),
            },
            program,
        );
        let (cached, cache_outcome) = if let Some(cached) = self.logical_plan_cache.get(&cache_key)
        {
            (cached, LogicalPlanCacheOutcome::Hit)
        } else {
            let bindings = RelationalProgramCompiler::bind_catalog_inputs_with_bindings(
                &self.program_bindings,
                program,
            )?;
            let mut inputs = Vec::with_capacity(bindings.len());
            for binding in bindings {
                let relation_id = ProgrammaticRelationId::new(binding.relation_id.as_str());
                let Some(authorized) = self.tables.get(&relation_id) else {
                    return Err(ChildSessionError::DeniedProgramRelation {
                        relation: binding.relation_id.as_str().to_owned(),
                    });
                };
                if authorized.table_reference != binding.table_reference {
                    return Err(ChildSessionError::ProgramBindingDrift {
                        relation: binding.relation_id.as_str().to_owned(),
                        expected: authorized.table_reference.clone(),
                        actual: binding.table_reference,
                    });
                }
                let plan = context
                    .table(authorized.table_reference.clone())
                    .await?
                    .into_unoptimized_plan();
                inputs.push(RelationInput {
                    relation_id: binding.relation_id,
                    plan,
                });
            }
            let compiled = RelationalProgramCompiler::compile_with_bindings(
                &self.program_bindings,
                inputs,
                program,
            )?;
            let expected_schema = Arc::new(compiled.plan.schema().as_arrow().clone());
            let bounded_plan = LogicalPlanBuilder::from(compiled.plan)
                .limit(0, Some(probe_limit))?
                .build()?;
            let optimized = self.state.optimize(&bounded_plan)?;
            let cached = self.logical_plan_cache.try_insert(
                cache_key,
                CachedLogicalPlan::new(
                    bounded_plan,
                    optimized,
                    expected_schema,
                    compiled.observations,
                ),
            )?;
            (cached, LogicalPlanCacheOutcome::Miss)
        };
        self.validate_cached_plan_authority(&cached)?;
        let physical_plan = self
            .state
            .query_planner()
            .create_physical_plan(cached.optimized_plan(), &self.state)
            .await?;
        let batches = collect(physical_plan, self.state.task_ctx()).await?;
        let expected_schema = Arc::clone(cached.output_schema());
        let observations = cached.observations().clone();
        let plan = execution_observation(&cached, cache_outcome);
        let row_count = batches.iter().map(RecordBatch::num_rows).sum::<usize>();
        if row_count > self.max_output_rows.get() {
            return Err(ChildSessionError::ProgramOutputRowLimitExceeded {
                observed: row_count,
                maximum: self.max_output_rows.get(),
            });
        }
        for batch in &batches {
            if batch.schema_ref().as_ref() != expected_schema.as_ref() {
                return Err(ChildSessionError::ProgramSchemaDrift {
                    expected: Arc::clone(&expected_schema),
                    actual: batch.schema(),
                });
            }
        }
        Ok(ChildProgramResult {
            schema: expected_schema,
            batches,
            observations,
            plan,
            cache_use: ChildProgramCacheUse::SharedCache(cache_outcome),
        })
    }

    /// Compile and execute one program with exact request-owned Arrow relations.
    ///
    /// Request relations extend a clone of the epoch's immutable [`ProgramBindings`] and enter the
    /// compiler as their already-verified direct scan plans. They are never registered in the
    /// reduced child catalog or its sealed parent. Because these concrete provider capabilities
    /// are query-local, this path deliberately bypasses the shared epoch logical-plan cache and
    /// constructs a fresh optimized and physical plan on every invocation.
    ///
    /// # Errors
    ///
    /// Rejects unused or shadowing request relations, binding/table drift, any compiled or
    /// optimized scan that does not retain the exact request/provider capability, relational
    /// typing or planning failure, output-schema drift, and output above the pinned row envelope.
    #[cfg(feature = "daemon")]
    pub(crate) async fn execute_relational_program_with_request_inputs(
        &self,
        program: &RelationalProgram,
        request_inputs: &RequestOwnedRelationCollection,
    ) -> Result<ChildProgramResult, ChildSessionError> {
        if request_inputs.is_empty() {
            return self.execute_relational_program(program).await;
        }

        let query_bindings = self
            .program_bindings
            .with_supplemental_relations(request_inputs.supplemental_program_bindings()?)?;
        let bindings =
            RelationalProgramCompiler::bind_catalog_inputs_with_bindings(&query_bindings, program)?;
        let context = self.context();
        let mut inputs = Vec::with_capacity(bindings.len());
        let mut consumed_request_relations = BTreeSet::new();
        for binding in bindings {
            if let Some(request_input) = request_inputs.get(&binding.relation_id) {
                if request_input.table_reference() != &binding.table_reference {
                    return Err(ChildSessionError::ProgramBindingDrift {
                        relation: binding.relation_id.as_str().to_owned(),
                        expected: request_input.table_reference().clone(),
                        actual: binding.table_reference,
                    });
                }
                let input = request_input.relation_input();
                request_input.validate_exact_input(&input)?;
                consumed_request_relations.insert(binding.relation_id);
                inputs.push(input);
                continue;
            }

            let relation_id = ProgrammaticRelationId::new(binding.relation_id.as_str());
            let Some(authorized) = self.tables.get(&relation_id) else {
                return Err(ChildSessionError::DeniedProgramRelation {
                    relation: binding.relation_id.as_str().to_owned(),
                });
            };
            if authorized.table_reference != binding.table_reference {
                return Err(ChildSessionError::ProgramBindingDrift {
                    relation: binding.relation_id.as_str().to_owned(),
                    expected: authorized.table_reference.clone(),
                    actual: binding.table_reference,
                });
            }
            let plan = context
                .table(authorized.table_reference.clone())
                .await?
                .into_unoptimized_plan();
            inputs.push(RelationInput {
                relation_id: binding.relation_id,
                plan,
            });
        }

        if consumed_request_relations.len() != request_inputs.len() {
            let unused = request_inputs
                .iter()
                .find(|input| !consumed_request_relations.contains(input.relation_id()))
                .expect("different request relation counts imply one unused relation");
            return Err(ChildSessionError::UnusedRequestOwnedRelation(
                unused.relation_id().as_str().to_owned(),
            ));
        }

        let compiled =
            RelationalProgramCompiler::compile_with_bindings(&query_bindings, inputs, program)?;
        let expected_schema = Arc::new(compiled.plan.schema().as_arrow().clone());
        let probe_limit = self
            .max_output_rows
            .get()
            .checked_add(1)
            .ok_or(ChildSessionError::OutputRowProbeOverflow)?;
        let bounded_plan = LogicalPlanBuilder::from(compiled.plan)
            .limit(0, Some(probe_limit))?
            .build()?;
        let optimized = self.state.optimize(&bounded_plan)?;
        let query_local_plan = CachedLogicalPlan::new(
            bounded_plan,
            optimized,
            expected_schema,
            compiled.observations,
        );
        self.validate_request_owned_plan_authority(&query_local_plan, request_inputs)?;

        // Physical planning and execution are intentionally fresh. `query_local_plan` is a local
        // digest/validation carrier and is never looked up in or inserted into the epoch cache.
        let physical_plan = self
            .state
            .query_planner()
            .create_physical_plan(query_local_plan.optimized_plan(), &self.state)
            .await?;
        let batches = collect(physical_plan, self.state.task_ctx()).await?;
        let expected_schema = Arc::clone(query_local_plan.output_schema());
        let observations = query_local_plan.observations().clone();
        let plan = execution_observation(&query_local_plan, LogicalPlanCacheOutcome::Miss);
        let row_count = batches.iter().map(RecordBatch::num_rows).sum::<usize>();
        if row_count > self.max_output_rows.get() {
            return Err(ChildSessionError::ProgramOutputRowLimitExceeded {
                observed: row_count,
                maximum: self.max_output_rows.get(),
            });
        }
        for batch in &batches {
            if batch.schema_ref().as_ref() != expected_schema.as_ref() {
                return Err(ChildSessionError::ProgramSchemaDrift {
                    expected: Arc::clone(&expected_schema),
                    actual: batch.schema(),
                });
            }
        }
        Ok(ChildProgramResult {
            schema: expected_schema,
            batches,
            observations,
            plan,
            cache_use: ChildProgramCacheUse::RequestOwnedAuthorityBypass,
        })
    }

    #[cfg(feature = "daemon")]
    fn validate_request_owned_plan_authority(
        &self,
        plan: &CachedLogicalPlan,
        request_inputs: &RequestOwnedRelationCollection,
    ) -> Result<(), ChildSessionError> {
        if plan.compiled_plan().schema().as_arrow() != plan.output_schema().as_ref()
            || plan.optimized_plan().schema().as_arrow() != plan.output_schema().as_ref()
        {
            return Err(ChildSessionError::RequestOwnedPlanAuthorityDrift(
                "query-local logical-plan schema differs from its compiled output contract".into(),
            ));
        }

        for (phase, logical_plan) in [
            ("compiled", plan.compiled_plan()),
            ("optimized", plan.optimized_plan()),
        ] {
            let mut observed_request_relations = BTreeSet::new();
            validate_logical_plan_references(logical_plan, &self.state, true, |scan| {
                if let Some(request_input) = request_inputs
                    .iter()
                    .find(|input| input.table_reference() == &scan.table_name)
                {
                    let provider = source_as_provider(&scan.source).map_err(|error| {
                        format!(
                            "{phase} request scan {} has a non-provider source: {error}",
                            scan.table_name
                        )
                    })?;
                    if !Arc::ptr_eq(&provider, request_input.provider_capability()) {
                        return Err(format!(
                            "{phase} request scan {} retains a different provider capability",
                            scan.table_name
                        ));
                    }
                    if provider.schema().as_ref() != request_input.schema().as_ref() {
                        return Err(format!(
                            "{phase} request scan {} retains a different Arrow schema",
                            scan.table_name
                        ));
                    }
                    observed_request_relations.insert(request_input.relation_id().clone());
                    return Ok(());
                }

                let Some(authorized) = self
                    .tables
                    .values()
                    .find(|table| table.table_reference == scan.table_name)
                else {
                    return Err(format!(
                        "{phase} scan {} is absent from the reduced child and request authority",
                        scan.table_name
                    ));
                };
                match source_as_provider(&scan.source) {
                    Ok(provider) if Arc::ptr_eq(&provider, &authorized.provider) => Ok(()),
                    Ok(_) => Err(format!(
                        "{phase} scan {} retains a different child provider capability",
                        scan.table_name
                    )),
                    Err(error) => Err(format!(
                        "{phase} scan {} has a non-provider table source: {error}",
                        scan.table_name
                    )),
                }
            })
            .map_err(ChildSessionError::RequestOwnedPlanAuthorityDrift)?;

            if observed_request_relations.len() != request_inputs.len() {
                let missing = request_inputs
                    .iter()
                    .find(|input| !observed_request_relations.contains(input.relation_id()))
                    .expect("different request relation counts imply one missing relation");
                return Err(ChildSessionError::RequestOwnedPlanAuthorityDrift(format!(
                    "{phase} plan no longer retains request provider {}",
                    missing.relation_id().as_str()
                )));
            }
        }
        Ok(())
    }

    fn validate_cached_plan_authority(
        &self,
        cached: &CachedLogicalPlan,
    ) -> Result<(), ChildSessionError> {
        if cached.compiled_plan().schema().as_arrow() != cached.output_schema().as_ref()
            || cached.optimized_plan().schema().as_arrow() != cached.output_schema().as_ref()
        {
            return Err(ChildSessionError::CachedPlanAuthorityDrift(
                "cached logical-plan schema differs from its admitted output contract".into(),
            ));
        }
        for plan in [cached.compiled_plan(), cached.optimized_plan()] {
            validate_logical_plan_references(plan, &self.state, true, |scan| {
                let Some(authorized) = self
                    .tables
                    .values()
                    .find(|table| table.table_reference == scan.table_name)
                else {
                    return Err(format!(
                        "cached scan {} is absent from the reduced child catalog",
                        scan.table_name
                    ));
                };
                match source_as_provider(&scan.source) {
                    Ok(provider) if Arc::ptr_eq(&provider, &authorized.provider) => Ok(()),
                    Ok(_) => Err(format!(
                        "cached scan {} retains a different provider capability",
                        scan.table_name
                    )),
                    Err(error) => Err(format!(
                        "cached scan {} has a non-provider table source: {error}",
                        scan.table_name
                    )),
                }
            })
            .map_err(ChildSessionError::CachedPlanAuthorityDrift)?;
        }
        Ok(())
    }

    fn context(&self) -> SessionContext {
        SessionContext::new_with_state(self.state.clone())
    }
}

impl ProgrammaticFabricEpoch {
    /// Derive one query-only child session from this exact sealed epoch.
    ///
    /// # Errors
    ///
    /// Fails closed on pin disagreement, unavailable registry installations,
    /// parent contract/provider drift, catalog leakage, or resource leakage.
    pub(crate) async fn authorized_child_session(
        &self,
        policy: ChildSessionPolicy,
        resources: &EpochResourceCoordinator,
    ) -> Result<AuthorizedChildSession, ChildSessionError> {
        AuthorizedChildSession::try_from_epoch(self, policy, resources).await
    }
}

/// Fail-closed child construction and query errors.
#[derive(Debug, thiserror::Error)]
pub enum ChildSessionError {
    #[error("invalid child table grant: {0}")]
    InvalidTableGrant(String),
    #[error("child {0} pin is all-zero")]
    InvalidPin(&'static str),
    #[error("invalid child resource limits: {0}")]
    InvalidResourceLimits(String),
    #[error("invalid child {0} registry authority")]
    InvalidRegistryAuthority(&'static str),
    #[error("child {kind} registry authority {name:?} is duplicated")]
    DuplicateRegistryAuthority { kind: &'static str, name: String },
    #[error(
        "child {family} function names differ: expected {expected:?}, implementation {actual:?}"
    )]
    FunctionNameMismatch {
        family: &'static str,
        expected: BTreeSet<String>,
        actual: BTreeSet<String>,
    },
    #[error("child {kind} registry installation drifted for {name:?}")]
    RegistryInstallationDrift { kind: &'static str, name: String },
    #[error("duplicate child table grant {0:?}")]
    DuplicateTableGrant(ProgrammaticRelationId),
    #[error("child epoch pin differs: expected {expected:?}, actual {actual:?}")]
    EpochPinMismatch {
        expected: FabricEpochId,
        actual: FabricEpochId,
    },
    #[error("child parent schema contract differs for relation {0:?}")]
    ParentSchemaContractMismatch(ProgrammaticRelationId),
    #[error("child did not retain the exact allowed provider capability for relation {0:?}")]
    ParentProviderCapabilityMismatch(ProgrammaticRelationId),
    #[error("granted relation {relation_id:?} retains denied provider dependency {dependency}")]
    DeniedProviderDependency {
        relation_id: ProgrammaticRelationId,
        dependency: TableReference,
    },
    #[error(
        "granted relation {relation_id:?} retains a substituted provider for dependency {dependency}"
    )]
    ProviderDependencyCapabilityMismatch {
        relation_id: ProgrammaticRelationId,
        dependency: TableReference,
    },
    #[error("granted provider dependency graph contains a cycle at relation {0:?}")]
    ProviderDependencyCycle(ProgrammaticRelationId),
    #[error("granted view relation {0:?} exposes no logical plan for closure validation")]
    ProviderLogicalPlanUnavailable(ProgrammaticRelationId),
    #[error("granted relation {relation_id:?} has invalid logical-plan closure: {detail}")]
    ProviderLogicalPlanClosure {
        relation_id: ProgrammaticRelationId,
        detail: String,
    },
    #[error("child retained a parent catalog/runtime/registry/resource authority")]
    ParentAuthorityRetained,
    #[error("child catalog closure failed: {0}")]
    CatalogClosure(String),
    #[error("child table is denied or absent: {0:?}")]
    DeniedTable(ProgrammaticRelationId),
    #[error("program relation {relation} is absent from the authorized child catalog")]
    DeniedProgramRelation { relation: String },
    #[error("request-owned relation {0} is not consumed by this relational program")]
    #[cfg(feature = "daemon")]
    UnusedRequestOwnedRelation(String),
    #[error(
        "program relation {relation} resolves to {actual}, but the authorized child owns {expected}"
    )]
    ProgramBindingDrift {
        relation: String,
        expected: TableReference,
        actual: TableReference,
    },
    #[error("cached logical plan escaped its admitted child authority: {0}")]
    CachedPlanAuthorityDrift(String),
    #[error(transparent)]
    LogicalPlanCache(#[from] LogicalPlanCacheError),
    #[error("request-owned logical plan escaped its exact query-local authority: {0}")]
    #[cfg(feature = "daemon")]
    RequestOwnedPlanAuthorityDrift(String),
    #[error("child logical-plan semantic authority could not be derived: {0}")]
    LogicalPlanAuthority(String),
    #[error("duplicate child projection index")]
    DuplicateProjectionIndex,
    #[error("invalid projection index {index} for child relation {relation_id:?}")]
    InvalidProjectionIndex {
        relation_id: ProgrammaticRelationId,
        index: usize,
    },
    #[error("child output-row limit {requested} exceeds pinned maximum {maximum}")]
    OutputRowLimitExceeded { requested: usize, maximum: usize },
    #[error("child output-row overflow probe cannot be represented")]
    OutputRowProbeOverflow,
    #[error("child relational-program output rows {observed} exceed pinned maximum {maximum}")]
    ProgramOutputRowLimitExceeded { observed: usize, maximum: usize },
    #[error(
        "child query schema drift for {relation_id:?}: expected {expected:?}, actual {actual:?}"
    )]
    QuerySchemaDrift {
        relation_id: ProgrammaticRelationId,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("child relational-program schema drift: expected {expected:?}, actual {actual:?}")]
    ProgramSchemaDrift {
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error(transparent)]
    ParentEpoch(#[from] ProgrammaticFabricEpochError),
    #[error(transparent)]
    Arrow(#[from] arrow_schema::ArrowError),
    #[error(transparent)]
    DataFusion(#[from] DataFusionError),
    #[error(transparent)]
    RelationalProgram(#[from] RelationalProgramError),
    #[error(transparent)]
    #[cfg(feature = "daemon")]
    RequestOwnedRelation(#[from] RequestOwnedRelationError),
    #[error(transparent)]
    ResourceGovernance(#[from] EpochResourceError),
}

fn truncate_batches(batches: Vec<RecordBatch>, row_limit: usize) -> Vec<RecordBatch> {
    let mut remaining = row_limit;
    let mut selected = Vec::new();
    for batch in batches {
        if remaining == 0 {
            break;
        }
        let selected_rows = remaining.min(batch.num_rows());
        if selected_rows == batch.num_rows() {
            selected.push(batch);
        } else {
            selected.push(batch.slice(0, selected_rows));
        }
        remaining -= selected_rows;
    }
    selected
}

fn full_table_parts(table_reference: &TableReference) -> Option<(&str, &str, &str)> {
    Some((
        table_reference.catalog()?,
        table_reference.schema()?,
        table_reference.table(),
    ))
}

fn validate_reduced_catalog(
    catalog_list: &MemoryCatalogProviderList,
    catalogs: &BTreeMap<String, Arc<MemoryCatalogProvider>>,
    schemas: &BTreeMap<(String, String), Arc<MemorySchemaProvider>>,
    tables: &BTreeMap<ProgrammaticRelationId, SealedTableContract>,
) -> Result<(), ChildSessionError> {
    let actual_catalogs = catalog_list
        .catalog_names()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let expected_catalogs = catalogs.keys().cloned().collect::<BTreeSet<_>>();
    if actual_catalogs != expected_catalogs {
        return Err(ChildSessionError::CatalogClosure(format!(
            "catalog names differ: expected {expected_catalogs:?}, actual {actual_catalogs:?}"
        )));
    }

    for (catalog_name, catalog) in catalogs {
        let actual_schemas = catalog.schema_names().into_iter().collect::<BTreeSet<_>>();
        let expected_schemas = schemas
            .keys()
            .filter(|(candidate_catalog, _)| candidate_catalog == catalog_name)
            .map(|(_, schema)| schema.clone())
            .collect::<BTreeSet<_>>();
        if actual_schemas != expected_schemas {
            return Err(ChildSessionError::CatalogClosure(format!(
                "schemas in {catalog_name} differ: expected {expected_schemas:?}, actual {actual_schemas:?}"
            )));
        }
    }
    for ((catalog_name, schema_name), schema) in schemas {
        let actual = schema.table_names().into_iter().collect::<BTreeSet<_>>();
        let expected = tables
            .values()
            .filter_map(|sealed| {
                let (catalog, schema, table) = full_table_parts(&sealed.table_reference)?;
                (catalog == catalog_name && schema == schema_name).then(|| table.to_owned())
            })
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(ChildSessionError::CatalogClosure(format!(
                "tables in {catalog_name}.{schema_name} differ: expected {expected:?}, actual {actual:?}"
            )));
        }
    }
    Ok(())
}

fn validate_function_names(
    family: &'static str,
    expected: &BTreeSet<String>,
    canonical: &str,
    aliases: &[String],
) -> Result<(), ChildSessionError> {
    let mut actual = BTreeSet::new();
    for name in std::iter::once(canonical).chain(aliases.iter().map(String::as_str)) {
        if name.is_empty() || name.trim() != name || !actual.insert(name.to_owned()) {
            return Err(ChildSessionError::InvalidRegistryAuthority("function"));
        }
    }
    if expected != &actual {
        return Err(ChildSessionError::FunctionNameMismatch {
            family,
            expected: expected.clone(),
            actual,
        });
    }
    Ok(())
}

#[derive(Debug)]
struct AllowlistedVariableProvider {
    variables: BTreeSet<ChildVariableReference>,
    delegate: Arc<dyn VarProvider + Send + Sync>,
}

impl AllowlistedVariableProvider {
    fn allows(&self, variable_names: &[String]) -> bool {
        self.variables
            .contains(&ChildVariableReference(variable_names.to_vec()))
    }
}

impl VarProvider for AllowlistedVariableProvider {
    fn get_value(
        &self,
        var_names: Vec<String>,
    ) -> datafusion::common::Result<datafusion::common::ScalarValue> {
        if !self.allows(&var_names) {
            return Err(DataFusionError::Plan(format!(
                "variable authority is absent from this child session: {var_names:?}"
            )));
        }
        self.delegate.get_value(var_names)
    }

    fn get_type(&self, var_names: &[String]) -> Option<arrow_schema::DataType> {
        self.allows(var_names)
            .then(|| self.delegate.get_type(var_names))
            .flatten()
    }
}

fn install_allowlisted_registries(
    state: &mut SessionState,
    allowlist: &ChildRegistryAllowlist,
) -> Result<(), ChildSessionError> {
    for grant in &allowlist.functions {
        let replaced = match &grant.capability {
            ChildFunctionCapability::Scalar { function } => {
                state.register_udf(Arc::clone(function))?.is_some()
            }
            ChildFunctionCapability::Aggregate { function } => {
                state.register_udaf(Arc::clone(function))?.is_some()
            }
            ChildFunctionCapability::Window { function } => {
                state.register_udwf(Arc::clone(function))?.is_some()
            }
        };
        if replaced {
            return Err(ChildSessionError::RegistryInstallationDrift {
                kind: grant.family(),
                name: grant
                    .expected_names()
                    .iter()
                    .next()
                    .cloned()
                    .unwrap_or_default(),
            });
        }
    }
    for grant in &allowlist.variables {
        if state
            .execution_props_mut()
            .add_var_provider(grant.variable_type.clone(), Arc::clone(&grant.installation))
            .is_some()
        {
            return Err(ChildSessionError::RegistryInstallationDrift {
                kind: "variable",
                name: format!("{:?}", grant.variable_type),
            });
        }
    }
    Ok(())
}

fn validate_allowlisted_registries(
    state: &SessionState,
    allowlist: &ChildRegistryAllowlist,
) -> Result<(), ChildSessionError> {
    let mut expected_scalar = BTreeMap::<String, &Arc<ScalarUDF>>::new();
    let mut expected_aggregate = BTreeMap::<String, &Arc<AggregateUDF>>::new();
    let mut expected_window = BTreeMap::<String, &Arc<WindowUDF>>::new();
    for grant in &allowlist.functions {
        match &grant.capability {
            ChildFunctionCapability::Scalar { function } => {
                for name in &grant.expected_names {
                    expected_scalar.insert(name.clone(), function);
                }
            }
            ChildFunctionCapability::Aggregate { function } => {
                for name in &grant.expected_names {
                    expected_aggregate.insert(name.clone(), function);
                }
            }
            ChildFunctionCapability::Window { function } => {
                for name in &grant.expected_names {
                    expected_window.insert(name.clone(), function);
                }
            }
        }
    }

    validate_function_installations("scalar", &expected_scalar, state.scalar_functions())?;
    validate_function_installations(
        "aggregate",
        &expected_aggregate,
        state.aggregate_functions(),
    )?;
    validate_function_installations("window", &expected_window, state.window_functions())?;

    for variable_type in [VarType::System, VarType::UserDefined] {
        let expected = allowlist
            .variables
            .iter()
            .find(|grant| grant.variable_type == variable_type)
            .map(|grant| &grant.installation);
        let actual = state
            .execution_props()
            .get_var_provider(variable_type.clone());
        match (expected, actual) {
            (Some(expected), Some(actual)) if Arc::ptr_eq(expected, &actual) => {}
            (None, None) => {}
            _ => {
                return Err(ChildSessionError::RegistryInstallationDrift {
                    kind: "variable",
                    name: format!("{variable_type:?}"),
                });
            }
        }
    }

    let extensions_empty = state
        .extension_type_registry()
        .extension_type_registrations()
        .is_empty();
    if !state.higher_order_functions().is_empty()
        || !state.table_functions().is_empty()
        || !state.expr_planners().is_empty()
        || !state.relation_planners().is_empty()
        || !extensions_empty
        || state.get_file_format_factory("csv").is_some()
        || state.get_file_format_factory("parquet").is_some()
    {
        return Err(ChildSessionError::CatalogClosure(
            "fresh child state contains an unrequested higher-order/table function, extension, planner, or file format"
                .into(),
        ));
    }
    Ok(())
}

fn validate_function_installations<T>(
    family: &'static str,
    expected: &BTreeMap<String, &Arc<T>>,
    actual: &std::collections::HashMap<String, Arc<T>>,
) -> Result<(), ChildSessionError>
where
    T: ?Sized,
{
    let actual_names = actual.keys().cloned().collect::<BTreeSet<_>>();
    let expected_names = expected.keys().cloned().collect::<BTreeSet<_>>();
    if actual_names != expected_names {
        return Err(ChildSessionError::RegistryInstallationDrift {
            kind: family,
            name: format!("expected {expected_names:?}, actual {actual_names:?}"),
        });
    }
    for (name, expected_function) in expected {
        let actual_function =
            actual
                .get(name)
                .ok_or_else(|| ChildSessionError::RegistryInstallationDrift {
                    kind: family,
                    name: name.clone(),
                })?;
        if !Arc::ptr_eq(expected_function, &actual_function) {
            return Err(ChildSessionError::RegistryInstallationDrift {
                kind: family,
                name: name.clone(),
            });
        }
    }
    Ok(())
}

struct AllowlistedObjectStoreRegistry {
    stores: BTreeMap<ObjectStoreUrl, Arc<dyn ObjectStore>>,
}

impl AllowlistedObjectStoreRegistry {
    fn from_grants(grants: &BTreeMap<ObjectStoreUrl, ChildObjectStoreGrant>) -> Self {
        Self {
            stores: grants
                .iter()
                .map(|(origin, grant)| (origin.clone(), Arc::clone(&grant.store)))
                .collect(),
        }
    }
}

impl fmt::Debug for AllowlistedObjectStoreRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AllowlistedObjectStoreRegistry")
            .field("origins", &self.stores.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ObjectStoreRegistry for AllowlistedObjectStoreRegistry {
    fn register_store(
        &self,
        _url: &Url,
        store: Arc<dyn ObjectStore>,
    ) -> Option<Arc<dyn ObjectStore>> {
        Some(store)
    }

    fn get_store(&self, url: &Url) -> datafusion::common::Result<Arc<dyn ObjectStore>> {
        if !url.username().is_empty() || url.password().is_some() {
            return Err(DataFusionError::Plan(
                "object-store URLs containing credentials are denied".into(),
            ));
        }
        let origin = object_store_origin(url)?;
        self.stores.get(&origin).cloned().ok_or_else(|| {
            DataFusionError::Plan(format!(
                "object-store origin {origin} is absent from this child session"
            ))
        })
    }
}

fn object_store_origin(url: &Url) -> datafusion::common::Result<ObjectStoreUrl> {
    let authority = &url[url::Position::BeforeHost..url::Position::AfterPort];
    ObjectStoreUrl::parse(format!("{}://{authority}", url.scheme()))
}

fn validate_object_store_installations(
    registry: &dyn ObjectStoreRegistry,
    expected: &BTreeMap<ObjectStoreUrl, ChildObjectStoreGrant>,
) -> Result<(), ChildSessionError> {
    for (origin, grant) in expected {
        let actual = registry.get_store(origin.as_ref())?;
        if !Arc::ptr_eq(&grant.store, &actual) {
            return Err(ChildSessionError::RegistryInstallationDrift {
                kind: "object-store",
                name: origin.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_empty_registries(state: &SessionState) -> Result<(), ChildSessionError> {
    let variables_empty = state
        .execution_props()
        .get_var_provider(VarType::System)
        .is_none()
        && state
            .execution_props()
            .get_var_provider(VarType::UserDefined)
            .is_none();
    let extensions_empty = state
        .extension_type_registry()
        .extension_type_registrations()
        .is_empty();
    if !state.scalar_functions().is_empty()
        || !state.higher_order_functions().is_empty()
        || !state.aggregate_functions().is_empty()
        || !state.window_functions().is_empty()
        || !state.table_functions().is_empty()
        || !state.expr_planners().is_empty()
        || !state.relation_planners().is_empty()
        || !variables_empty
        || !extensions_empty
        || state.get_file_format_factory("csv").is_some()
        || state.get_file_format_factory("parquet").is_some()
    {
        return Err(ChildSessionError::CatalogClosure(
            "fresh child state contains an unrequested function, variable, extension, planner, or file format"
                .into(),
        ));
    }
    Ok(())
}

/// Immutable empty object-store registry. Registration attempts are refused by
/// retaining no capability; lookup therefore always fails closed.
#[derive(Debug)]
struct ClosedObjectStoreRegistry;

impl ObjectStoreRegistry for ClosedObjectStoreRegistry {
    fn register_store(
        &self,
        _url: &Url,
        store: Arc<dyn ObjectStore>,
    ) -> Option<Arc<dyn ObjectStore>> {
        Some(store)
    }

    fn get_store(&self, _url: &Url) -> datafusion::common::Result<Arc<dyn ObjectStore>> {
        Err(DataFusionError::Plan(
            "object-store authority is absent from this child session".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow_array::{RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::common::{Column, DFSchema, ScalarValue};
    use datafusion::datasource::{MemTable, ViewTable, provider_as_source};
    use datafusion::logical_expr::{ColumnarValue, Expr, Projection, Volatility, create_udf};
    use datafusion::prelude::col;
    use object_store::memory::InMemory;

    use super::*;
    use crate::fabric::epoch_runtime::FabricEpochRuntimeConfig;
    use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
    use crate::fabric::programmatic_schema::ProviderInput;
    use crate::relational_program::{
        CompilationDependency, FieldId, RelationId, RelationalExpression,
    };
    use crate::schema_contract::{
        FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SchemaRole,
    };

    #[derive(Debug)]
    struct TestVariableProvider;

    impl VarProvider for TestVariableProvider {
        fn get_value(&self, var_names: Vec<String>) -> datafusion::common::Result<ScalarValue> {
            Ok(ScalarValue::Utf8(Some(var_names.join("."))))
        }

        fn get_type(&self, _var_names: &[String]) -> Option<DataType> {
            Some(DataType::Utf8)
        }
    }

    fn function_names(canonical: &str, aliases: &[String]) -> BTreeSet<String> {
        std::iter::once(canonical.to_owned())
            .chain(aliases.iter().cloned())
            .collect()
    }

    fn test_scalar_udf() -> Arc<ScalarUDF> {
        Arc::new(
            create_udf(
                "child_identity",
                vec![DataType::Utf8],
                DataType::Utf8,
                Volatility::Immutable,
                Arc::new(|args: &[ColumnarValue]| Ok(args[0].clone())),
            )
            .with_aliases(["child_identity_alias"]),
        )
    }

    fn test_aggregate_udf() -> Arc<AggregateUDF> {
        datafusion::functions_aggregate::all_default_aggregate_functions()
            .into_iter()
            .find(|function| function.name() == "count")
            .expect("DataFusion 55 count UDAF")
    }

    fn test_window_udf() -> Arc<WindowUDF> {
        datafusion::functions_window::all_default_window_functions()
            .into_iter()
            .find(|function| function.name() == "row_number")
            .expect("DataFusion 55 row_number UDWF")
    }

    fn scalar_registry(function: Arc<ScalarUDF>) -> ChildRegistryAllowlist {
        ChildRegistryAllowlist::try_new(
            vec![
                ChildFunctionGrant::try_scalar(
                    function_names(function.name(), function.aliases()),
                    function,
                )
                .unwrap(),
            ],
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    }

    const ALLOWED_RELATION: &str = "test.allowed_relation";
    const DENIED_RELATION: &str = "test.denied_relation";
    const INNER_VIEW_RELATION: &str = "test.inner_view_relation";
    const OUTER_VIEW_RELATION: &str = "test.outer_view_relation";

    fn identified_schema(relation_id: &str, fields: &[(&str, &str)]) -> SchemaRef {
        let fields = fields
            .iter()
            .map(|(name, field_id)| {
                Field::new(*name, DataType::Utf8, false).with_metadata(HashMap::from([(
                    FIELD_ID_METADATA_KEY.to_owned(),
                    (*field_id).to_owned(),
                )]))
            })
            .collect::<Vec<_>>();
        Arc::new(Schema::new_with_metadata(
            fields,
            HashMap::from([(RELATION_ID_METADATA_KEY.to_owned(), relation_id.to_owned())]),
        ))
    }

    fn provider_input(
        relation_id: &str,
        table_reference: TableReference,
        fields: &[(&str, &str)],
        rows: &[Vec<&str>],
    ) -> ProviderInput {
        provider_input_with_capability(relation_id, table_reference, fields, rows).0
    }

    fn provider_input_with_capability(
        relation_id: &str,
        table_reference: TableReference,
        fields: &[(&str, &str)],
        rows: &[Vec<&str>],
    ) -> (ProviderInput, Arc<dyn datafusion::catalog::TableProvider>) {
        let schema = identified_schema(relation_id, fields);
        let columns = (0..fields.len())
            .map(|column| {
                Arc::new(StringArray::from(
                    rows.iter().map(|row| row[column]).collect::<Vec<_>>(),
                )) as _
            })
            .collect::<Vec<_>>();
        let batch = RecordBatch::try_new(Arc::clone(&schema), columns).unwrap();
        let contract = Arc::new(
            SchemaContract::try_new(
                format!("provider:{relation_id}:v1"),
                table_reference.clone(),
                Arc::clone(&schema),
                Arc::clone(&schema),
                (0..fields.len())
                    .map(|index| FieldIndexMapping::direct(index, index))
                    .collect(),
            )
            .unwrap(),
        );
        let provider: Arc<dyn datafusion::catalog::TableProvider> =
            Arc::new(MemTable::try_new(schema, vec![vec![batch]]).unwrap());
        (
            ProviderInput::new(
                ProgrammaticRelationId::new(relation_id),
                table_reference,
                contract,
                Arc::clone(&provider),
            ),
            provider,
        )
    }

    fn view_provider_input(
        relation_id: &str,
        table_reference: TableReference,
        dependency_reference: TableReference,
        dependency_provider: Arc<dyn datafusion::catalog::TableProvider>,
        field_name: &str,
        field_id: &str,
    ) -> (ProviderInput, Arc<dyn datafusion::catalog::TableProvider>) {
        let schema = identified_schema(relation_id, &[(field_name, field_id)]);
        let input = LogicalPlanBuilder::scan(
            dependency_reference,
            provider_as_source(dependency_provider),
            None,
        )
        .unwrap()
        .build()
        .unwrap();
        let expressions = input
            .schema()
            .iter()
            .map(|(qualifier, field)| {
                Expr::Column(Column::from((qualifier, field))).alias(field_name.to_owned())
            })
            .collect::<Vec<_>>();
        let qualified_schema = Arc::new(
            DFSchema::try_from_qualified_schema(table_reference.clone(), schema.as_ref()).unwrap(),
        );
        let plan = LogicalPlan::Projection(
            Projection::try_new_with_schema(expressions, Arc::new(input), qualified_schema)
                .unwrap(),
        );
        let provider: Arc<dyn datafusion::catalog::TableProvider> =
            Arc::new(ViewTable::new(plan, None));
        let contract = Arc::new(
            SchemaContract::try_new(
                format!("provider:{relation_id}:v1"),
                table_reference.clone(),
                Arc::clone(&schema),
                schema,
                vec![FieldIndexMapping::direct(0, 0)],
            )
            .unwrap(),
        );
        (
            ProviderInput::new(
                ProgrammaticRelationId::new(relation_id),
                table_reference,
                contract,
                Arc::clone(&provider),
            ),
            provider,
        )
    }

    async fn sealed_epoch() -> ProgrammaticFabricEpoch {
        let resources = FabricEpochRuntimeConfig::default();
        let mut builder = ProgrammaticFabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([0xA5; 16]),
            resources,
        )
        .unwrap();
        builder
            .register_provider(provider_input(
                ALLOWED_RELATION,
                TableReference::full(FABRIC_CATALOG, "fact", "allowed_rows"),
                &[
                    ("entity_id", "test.allowed.entity_id"),
                    ("kind", "test.allowed.kind"),
                ],
                &[vec!["entity-1", "function"], vec!["entity-2", "class"]],
            ))
            .unwrap();
        builder
            .register_provider(provider_input(
                DENIED_RELATION,
                TableReference::full(FABRIC_CATALOG, "derived", "denied_rows"),
                &[("entity_id", "test.denied.entity_id")],
                &[vec!["entity-3"]],
            ))
            .unwrap();
        builder.seal_for_test().await.unwrap()
    }

    async fn sealed_view_epoch() -> ProgrammaticFabricEpoch {
        let mut builder = ProgrammaticFabricEpochBuilder::try_new(
            FabricEpochId::from_bytes([0xB6; 16]),
            FabricEpochRuntimeConfig::default(),
        )
        .unwrap();
        let denied_reference = TableReference::full(FABRIC_CATALOG, "derived", "denied_rows");
        let (denied_input, denied_provider) = provider_input_with_capability(
            DENIED_RELATION,
            denied_reference.clone(),
            &[("entity_id", "test.denied.entity_id")],
            &[vec!["entity-3"]],
        );
        builder.register_provider(denied_input).unwrap();

        let inner_reference = TableReference::full(FABRIC_CATALOG, "derived", "inner_allowed_view");
        let (inner_input, inner_provider) = view_provider_input(
            INNER_VIEW_RELATION,
            inner_reference.clone(),
            denied_reference,
            denied_provider,
            "entity_id",
            "test.inner_view.entity_id",
        );
        builder.register_provider(inner_input).unwrap();

        let outer_reference = TableReference::full(FABRIC_CATALOG, "derived", "outer_allowed_view");
        let (outer_input, _) = view_provider_input(
            OUTER_VIEW_RELATION,
            outer_reference,
            inner_reference,
            inner_provider,
            "entity_id",
            "test.outer_view.entity_id",
        );
        builder.register_provider(outer_input).unwrap();
        builder.seal_for_test().await.unwrap()
    }

    fn grant(relation_id: &str) -> ChildTableGrant {
        ChildTableGrant::try_new(ProgrammaticRelationId::new(relation_id)).unwrap()
    }

    fn policy(
        epoch: &ProgrammaticFabricEpoch,
        grants: Vec<ChildTableGrant>,
        registries: ChildRegistryAllowlist,
    ) -> ChildSessionPolicy {
        ChildSessionPolicy::try_new(
            ChildSessionPins::try_new(*epoch.identity(), [0x11; 32], [0x22; 32], [0x33; 32])
                .unwrap(),
            grants,
            child_resources(),
            128,
            registries,
        )
        .unwrap()
    }

    fn child_resources() -> ChildResourceLimits {
        ChildResourceLimits::try_new(8 * 1024 * 1024, 32 * 1024 * 1024, 4, 2, 128, 1).unwrap()
    }

    fn resource_coordinator(epoch: &ProgrammaticFabricEpoch) -> EpochResourceCoordinator {
        let policy = resource_governance::EpochResourcePolicy::try_new(
            child_resources(),
            resource_governance::test_lifecycle_work_class_policies(),
            4,
            1,
            8,
            30_000,
            1,
            2,
            8,
            64 * 1024 * 1024,
            60_000,
        )
        .unwrap();
        EpochResourceCoordinator::try_new(*epoch.identity(), [0x33; 32], policy).unwrap()
    }

    fn input_program(
        epoch: &ProgrammaticFabricEpoch,
        relation_id: &str,
    ) -> (RelationId, RelationalProgram) {
        let stable_id = ProgrammaticRelationId::new(relation_id);
        let sealed = epoch.relation(&stable_id).unwrap();
        let relation_id = RelationId::new(relation_id).unwrap();
        let output_fields = (0..sealed.contract.logical_schema().fields().len())
            .map(|ordinal| {
                FieldId::new(
                    sealed
                        .contract
                        .field_id_at(SchemaRole::Logical, ordinal)
                        .unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let program = RelationalProgram {
            root: RelationalExpression::Input(relation_id.clone()),
            output_fields,
        };
        (relation_id, program)
    }

    #[tokio::test]
    async fn reduced_catalog_exposes_allowed_table_and_physically_omits_denied_table() {
        let epoch = sealed_epoch().await;
        let resources = resource_coordinator(&epoch);
        let relation_grant = grant(ALLOWED_RELATION);
        let allowed = relation_grant.relation_id().clone();
        let child = epoch
            .authorized_child_session(
                policy(
                    &epoch,
                    vec![relation_grant],
                    ChildRegistryAllowlist::default(),
                ),
                &resources,
            )
            .await
            .unwrap();

        assert_eq!(child.allowed_tables().collect::<Vec<_>>(), vec![&allowed]);
        let contracts = child.table_contracts().collect::<Vec<_>>();
        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0].relation_id(), &allowed);
        assert!(
            contracts[0]
                .source_schema_identity()
                .starts_with("provider:test.allowed_relation:")
        );
        assert_eq!(
            contracts[0].logical_schema().as_ref(),
            epoch
                .relation(&ProgrammaticRelationId::new(ALLOWED_RELATION))
                .unwrap()
                .contract
                .logical_schema()
                .as_ref()
        );
        let result = child
            .scan(&ChildTableScan::all(allowed.clone()))
            .await
            .unwrap();
        assert_eq!(result.row_count(), 2);
        assert!(!result.truncated());

        let denied = ProgrammaticRelationId::new(DENIED_RELATION);
        assert!(matches!(
            child.scan(&ChildTableScan::all(denied.clone())).await,
            Err(ChildSessionError::DeniedTable(table)) if table == denied
        ));
        assert!(
            child
                .context()
                .table(epoch.relation(&denied).unwrap().table_reference.clone())
                .await
                .is_err()
        );
        assert!(
            child
                .context()
                .table(TableReference::full(
                    FABRIC_CATALOG,
                    "information_schema",
                    "tables",
                ))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn granted_view_graph_rejects_a_transitively_denied_parent_provider() {
        let epoch = sealed_view_epoch().await;
        let resources = resource_coordinator(&epoch);
        let error = epoch
            .authorized_child_session(
                policy(
                    &epoch,
                    vec![grant(INNER_VIEW_RELATION), grant(OUTER_VIEW_RELATION)],
                    ChildRegistryAllowlist::default(),
                ),
                &resources,
            )
            .await
            .expect_err("inner view must not retain its denied parent provider");

        assert!(matches!(
            error,
            ChildSessionError::DeniedProviderDependency {
                relation_id,
                dependency,
            } if relation_id == ProgrammaticRelationId::new(INNER_VIEW_RELATION)
                && dependency
                    == TableReference::full(FABRIC_CATALOG, "derived", "denied_rows")
        ));
    }

    #[tokio::test]
    async fn fully_granted_view_graph_is_rebuilt_against_child_providers() {
        let epoch = sealed_view_epoch().await;
        assert!(
            epoch
                .relation(&ProgrammaticRelationId::new(INNER_VIEW_RELATION))
                .expect("sealed inner view binding")
                .logical_plan
                .is_some(),
            "the sealed epoch must retain the application-owned inner view plan"
        );
        let resources = resource_coordinator(&epoch);
        let child = epoch
            .authorized_child_session(
                policy(
                    &epoch,
                    vec![
                        grant(DENIED_RELATION),
                        grant(INNER_VIEW_RELATION),
                        grant(OUTER_VIEW_RELATION),
                    ],
                    ChildRegistryAllowlist::default(),
                ),
                &resources,
            )
            .await
            .unwrap();

        let inner_id = ProgrammaticRelationId::new(INNER_VIEW_RELATION);
        let outer_id = ProgrammaticRelationId::new(OUTER_VIEW_RELATION);
        let base_id = ProgrammaticRelationId::new(DENIED_RELATION);
        let (_, parent_inner, _, _) = epoch.resolve_sealed_relation(&inner_id).await.unwrap();
        let (_, parent_outer, _, _) = epoch.resolve_sealed_relation(&outer_id).await.unwrap();
        let child_inner = Arc::clone(&child.tables.get(&inner_id).unwrap().provider);
        let child_outer = Arc::clone(&child.tables.get(&outer_id).unwrap().provider);
        let child_base = Arc::clone(&child.tables.get(&base_id).unwrap().provider);
        assert!(!Arc::ptr_eq(&parent_inner, &child_inner));
        assert!(!Arc::ptr_eq(&parent_outer, &child_outer));

        let inner_plan =
            registered_view_logical_plan(child_inner.as_ref()).expect("rebuilt inner view plan");
        let inner_dependencies = provider_plan_dependencies(&inner_plan).unwrap();
        assert_eq!(inner_dependencies.len(), 1);
        assert!(Arc::ptr_eq(&inner_dependencies[0].1, &child_base));
        let outer_plan =
            registered_view_logical_plan(child_outer.as_ref()).expect("rebuilt outer view plan");
        let outer_dependencies = provider_plan_dependencies(&outer_plan).unwrap();
        assert_eq!(outer_dependencies.len(), 1);
        // DataFusion 55 inlines an unfiltered ViewTable scan while building the outer
        // plan, so its exposed capability edge is directly to the base provider.
        assert!(
            Arc::ptr_eq(&outer_dependencies[0].1, &child_base),
            "outer view must retain the rebuilt child base provider",
        );
        assert_eq!(
            outer_dependencies[0].0,
            TableReference::full(FABRIC_CATALOG, "derived", "denied_rows")
        );

        assert_eq!(child_inner.schema(), parent_inner.schema());
        assert_eq!(child_outer.schema(), parent_outer.schema());

        let executed = child
            .scan(&ChildTableScan::all(outer_id))
            .await
            .expect("rebuilt nested child view must execute through its reduced catalog");
        assert!(executed.row_count() > 0);
        assert_eq!(executed.schema().as_ref(), parent_outer.schema().as_ref());
        assert!(
            executed
                .batches()
                .iter()
                .all(|batch| batch.schema().as_ref() == parent_outer.schema().as_ref()),
            "analyzed, optimized, physical, and batch schemas must retain exact view identity",
        );
    }

    #[tokio::test]
    async fn relational_program_executes_only_through_authorized_child_inputs() {
        let epoch = sealed_epoch().await;
        let resources = resource_coordinator(&epoch);
        let child = epoch
            .authorized_child_session(
                policy(
                    &epoch,
                    vec![grant(ALLOWED_RELATION)],
                    ChildRegistryAllowlist::default(),
                ),
                &resources,
            )
            .await
            .unwrap();
        let (allowed_relation, allowed_program) = input_program(&epoch, ALLOWED_RELATION);
        let result = child
            .execute_relational_program(&allowed_program)
            .await
            .unwrap();
        assert_eq!(result.row_count(), 2);
        assert_eq!(
            result.plan_observation().outcome,
            LogicalPlanCacheOutcome::Miss
        );
        assert_eq!(
            result.schema().as_ref(),
            epoch
                .relation(&ProgrammaticRelationId::new(ALLOWED_RELATION))
                .unwrap()
                .contract
                .logical_schema()
                .as_ref()
        );
        assert!(
            result
                .observations()
                .dependencies
                .contains(&CompilationDependency::Relation(allowed_relation))
        );

        let repeated_child = epoch
            .authorized_child_session(
                policy(
                    &epoch,
                    vec![grant(ALLOWED_RELATION)],
                    ChildRegistryAllowlist::default(),
                ),
                &resources,
            )
            .await
            .unwrap();
        let repeated = repeated_child
            .execute_relational_program(&allowed_program)
            .await
            .unwrap();
        assert_eq!(repeated.row_count(), 2);
        assert_eq!(
            repeated.plan_observation().outcome,
            LogicalPlanCacheOutcome::Hit
        );
        assert_eq!(
            repeated.plan_observation().optimized_plan_digest,
            result.plan_observation().optimized_plan_digest
        );

        let (denied_relation, denied_program) = input_program(&epoch, DENIED_RELATION);
        assert!(matches!(
            child.execute_relational_program(&denied_program).await,
            Err(ChildSessionError::DeniedProgramRelation { relation })
                if relation == denied_relation.as_str()
        ));
    }

    #[tokio::test]
    async fn shared_cache_tracks_concrete_registry_authority_not_only_opaque_pins() {
        let epoch = sealed_epoch().await;
        let resources = resource_coordinator(&epoch);
        let (_, program) = input_program(&epoch, ALLOWED_RELATION);

        let first_function = test_scalar_udf();
        let first = epoch
            .authorized_child_session(
                policy(
                    &epoch,
                    vec![grant(ALLOWED_RELATION)],
                    scalar_registry(first_function),
                ),
                &resources,
            )
            .await
            .unwrap()
            .execute_relational_program(&program)
            .await
            .unwrap();
        assert_eq!(
            first.plan_observation().outcome,
            LogicalPlanCacheOutcome::Miss
        );

        // Same epoch/access/query/resource pins and the same function names/signature are not
        // enough: a different concrete implementation capability must causally miss.
        let replacement_function = test_scalar_udf();
        let replacement_child = epoch
            .authorized_child_session(
                policy(
                    &epoch,
                    vec![grant(ALLOWED_RELATION)],
                    scalar_registry(Arc::clone(&replacement_function)),
                ),
                &resources,
            )
            .await
            .unwrap();
        let replacement = replacement_child
            .execute_relational_program(&program)
            .await
            .unwrap();
        assert_eq!(
            replacement.plan_observation().outcome,
            LogicalPlanCacheOutcome::Miss
        );

        // Reinstalling the exact same capability is semantically identical and may reuse the
        // shared logical plan. Physical planning and execution still happen afresh.
        let exact_reinstallation = epoch
            .authorized_child_session(
                policy(
                    &epoch,
                    vec![grant(ALLOWED_RELATION)],
                    scalar_registry(replacement_function),
                ),
                &resources,
            )
            .await
            .unwrap()
            .execute_relational_program(&program)
            .await
            .unwrap();
        assert_eq!(
            exact_reinstallation.plan_observation().outcome,
            LogicalPlanCacheOutcome::Hit
        );
    }

    #[tokio::test]
    async fn cached_reference_validation_reaches_subqueries_and_function_capabilities() {
        let epoch = sealed_epoch().await;
        let resources = resource_coordinator(&epoch);
        let installed_function = test_scalar_udf();
        let child = epoch
            .authorized_child_session(
                policy(
                    &epoch,
                    vec![grant(ALLOWED_RELATION)],
                    scalar_registry(installed_function),
                ),
                &resources,
            )
            .await
            .unwrap();
        let allowed_reference = epoch
            .relation(&ProgrammaticRelationId::new(ALLOWED_RELATION))
            .unwrap()
            .table_reference
            .clone();

        let stale_function = test_scalar_udf();
        let function_plan = LogicalPlanBuilder::from(
            child
                .context()
                .table(allowed_reference.clone())
                .await
                .unwrap()
                .into_unoptimized_plan(),
        )
        .project(vec![stale_function.call(vec![col("kind")]).alias("kind")])
        .unwrap()
        .build()
        .unwrap();
        let function_schema = Arc::new(function_plan.schema().as_arrow().clone());
        let function_cached = CachedLogicalPlan::new(
            function_plan.clone(),
            function_plan,
            function_schema,
            CompilationObservations::default(),
        );
        assert!(matches!(
            child.validate_cached_plan_authority(&function_cached),
            Err(ChildSessionError::CachedPlanAuthorityDrift(message))
                if message.contains("scalar function child_identity")
        ));

        let foreign_schema = identified_schema(
            "test.foreign_subquery",
            &[("entity_id", "test.foreign_subquery.entity_id")],
        );
        let foreign_batch = RecordBatch::new_empty(Arc::clone(&foreign_schema));
        let foreign_provider: Arc<dyn datafusion::catalog::TableProvider> =
            Arc::new(MemTable::try_new(foreign_schema, vec![vec![foreign_batch]]).unwrap());
        let foreign_plan = LogicalPlanBuilder::scan(
            TableReference::full(FABRIC_CATALOG, "fact", "foreign_subquery"),
            provider_as_source(foreign_provider),
            None,
        )
        .unwrap()
        .build()
        .unwrap();
        let subquery_plan = LogicalPlanBuilder::from(
            child
                .context()
                .table(allowed_reference)
                .await
                .unwrap()
                .into_unoptimized_plan(),
        )
        .filter(datafusion::logical_expr::expr_fn::exists(Arc::new(
            foreign_plan,
        )))
        .unwrap()
        .build()
        .unwrap();
        let subquery_schema = Arc::new(subquery_plan.schema().as_arrow().clone());
        let subquery_cached = CachedLogicalPlan::new(
            subquery_plan.clone(),
            subquery_plan,
            subquery_schema,
            CompilationObservations::default(),
        );
        assert!(matches!(
            child.validate_cached_plan_authority(&subquery_cached),
            Err(ChildSessionError::CachedPlanAuthorityDrift(message))
                if message.contains("foreign_subquery")
        ));
    }

    #[tokio::test]
    async fn child_owns_fresh_closed_registries_and_resources() {
        let epoch = sealed_epoch().await;
        let resources = resource_coordinator(&epoch);
        let child = epoch
            .authorized_child_session(
                policy(
                    &epoch,
                    vec![grant(ALLOWED_RELATION)],
                    ChildRegistryAllowlist::default(),
                ),
                &resources,
            )
            .await
            .unwrap();

        assert!(
            epoch.child_authorities_are_distinct(
                child.state.runtime_env(),
                child.state.catalog_list(),
            )
        );
        validate_empty_registries(&child.state).unwrap();
        assert!(
            child
                .state
                .runtime_env()
                .object_store_registry
                .get_store(&Url::parse("file:///not-authorized").unwrap())
                .is_err()
        );
        assert_eq!(
            child.resource_observation().memory_limit_bytes,
            8 * 1024 * 1024
        );
        assert_eq!(child.resource_observation().max_output_rows, 128);
        let observation = child.resource_observation();
        assert_eq!(
            observation.metadata_cache_limit_bytes,
            child.resources.cache_policy().metadata_cache_bytes()
        );
        assert_eq!(
            observation.file_statistics_cache_limit_bytes,
            child.resources.cache_policy().file_statistics_cache_bytes()
        );
        assert_eq!(
            observation.object_list_cache_limit_bytes,
            child.resources.cache_policy().object_list_cache_bytes()
        );
        assert_eq!(
            observation.object_list_cache_ttl_seconds,
            Some(
                child
                    .resources
                    .cache_policy()
                    .object_list_cache_ttl_seconds()
            )
        );
    }

    #[tokio::test]
    async fn empty_result_retains_exact_projected_schema() {
        let epoch = sealed_epoch().await;
        let resources = resource_coordinator(&epoch);
        let table_grant = grant(ALLOWED_RELATION);
        let table = table_grant.relation_id().clone();
        let expected = Arc::new(
            epoch
                .relation(&table)
                .unwrap()
                .contract
                .logical_schema()
                .project(&[0])
                .unwrap(),
        );
        let child = epoch
            .authorized_child_session(
                policy(&epoch, vec![table_grant], ChildRegistryAllowlist::default()),
                &resources,
            )
            .await
            .unwrap();

        let result = child
            .scan(
                &ChildTableScan::all(table)
                    .with_projection(vec![0])
                    .with_limit(0),
            )
            .await
            .unwrap();
        assert_eq!(result.row_count(), 0);
        assert_eq!(result.schema().as_ref(), expected.as_ref());
        assert!(result.truncated());
    }

    #[tokio::test]
    async fn unresolved_relation_and_function_name_authority_fail_closed() {
        let epoch = sealed_epoch().await;
        let resources = resource_coordinator(&epoch);
        let unresolved = grant("test.relation_not_sealed");
        assert!(matches!(
            epoch
                .authorized_child_session(
                    policy(&epoch, vec![unresolved], ChildRegistryAllowlist::default(),),
                    &resources
                )
                .await,
            Err(ChildSessionError::ParentEpoch(
                ProgrammaticFabricEpochError::CatalogClosure(_)
            ))
        ));

        assert!(matches!(
            ChildFunctionGrant::try_scalar(
                BTreeSet::from(["child_identity".to_owned()]),
                test_scalar_udf(),
            ),
            Err(ChildSessionError::FunctionNameMismatch { .. })
        ));
    }

    #[tokio::test]
    async fn installs_only_exact_supplied_function_variable_and_store_capabilities() {
        let epoch = sealed_epoch().await;
        let resources = resource_coordinator(&epoch);
        let scalar = test_scalar_udf();
        let aggregate = test_aggregate_udf();
        let window = test_window_udf();
        let scalar_names = function_names(scalar.name(), scalar.aliases());
        let aggregate_names = function_names(aggregate.name(), aggregate.aliases());
        let window_names = function_names(window.name(), window.aliases());
        let variable = ChildVariableProviderGrant::try_new(
            VarType::UserDefined,
            BTreeSet::from([ChildVariableReference::try_new(vec!["@allowed".into()]).unwrap()]),
            Arc::new(TestVariableProvider),
        )
        .unwrap();
        let expected_variable_installation = Arc::clone(&variable.installation);
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let origin = ObjectStoreUrl::parse("memory://allowed").unwrap();
        let registry = ChildRegistryAllowlist::try_new(
            vec![
                ChildFunctionGrant::try_scalar(scalar_names.clone(), Arc::clone(&scalar)).unwrap(),
                ChildFunctionGrant::try_aggregate(aggregate_names.clone(), Arc::clone(&aggregate))
                    .unwrap(),
                ChildFunctionGrant::try_window(window_names.clone(), Arc::clone(&window)).unwrap(),
            ],
            vec![variable],
            vec![ChildObjectStoreGrant::try_new(origin.clone(), Arc::clone(&store)).unwrap()],
        )
        .unwrap();

        let child = epoch
            .authorized_child_session(
                policy(&epoch, vec![grant(ALLOWED_RELATION)], registry),
                &resources,
            )
            .await
            .unwrap();

        assert_eq!(
            child
                .state
                .scalar_functions()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            scalar_names
        );
        for name in function_names(scalar.name(), scalar.aliases()) {
            assert!(Arc::ptr_eq(
                child.state.scalar_functions().get(&name).unwrap(),
                &scalar
            ));
        }
        assert_eq!(
            child
                .state
                .aggregate_functions()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            aggregate_names
        );
        for name in function_names(aggregate.name(), aggregate.aliases()) {
            assert!(Arc::ptr_eq(
                child.state.aggregate_functions().get(&name).unwrap(),
                &aggregate
            ));
        }
        assert_eq!(
            child
                .state
                .window_functions()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            window_names
        );
        for name in function_names(window.name(), window.aliases()) {
            assert!(Arc::ptr_eq(
                child.state.window_functions().get(&name).unwrap(),
                &window
            ));
        }
        assert!(!child.state.scalar_functions().contains_key("sqrt"));

        let installed_variable = child
            .state
            .execution_props()
            .get_var_provider(VarType::UserDefined)
            .unwrap();
        assert!(Arc::ptr_eq(
            &installed_variable,
            &expected_variable_installation
        ));
        assert_eq!(
            installed_variable.get_type(&["@allowed".into()]),
            Some(DataType::Utf8)
        );
        assert_eq!(
            installed_variable
                .get_value(vec!["@allowed".into()])
                .unwrap(),
            ScalarValue::Utf8(Some("@allowed".into()))
        );
        assert_eq!(installed_variable.get_type(&["@denied".into()]), None);
        assert!(
            installed_variable
                .get_value(vec!["@denied".into()])
                .is_err()
        );

        let installed_store = child
            .state
            .runtime_env()
            .object_store_registry
            .get_store(&Url::parse("memory://allowed/path").unwrap())
            .unwrap();
        assert!(Arc::ptr_eq(&store, &installed_store));
        assert!(
            child
                .state
                .runtime_env()
                .object_store_registry
                .get_store(&Url::parse("memory://denied/path").unwrap())
                .is_err()
        );
        assert!(matches!(
            ChildObjectStoreGrant::try_new(
                ObjectStoreUrl::local_filesystem(),
                Arc::new(InMemory::new()),
            ),
            Err(ChildSessionError::InvalidRegistryAuthority("object-store"))
        ));
    }
}
