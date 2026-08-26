//! Leased-snapshot-only DataFusion catalogs, views, and read-only query execution.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use arrow_array::builder::{BinaryBuilder, Float64Builder, Int64Builder, StringBuilder};
use arrow_array::{ArrayRef, BinaryArray, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use async_trait::async_trait;
use datafusion::catalog::{CatalogProvider, SchemaProvider, TableProvider};
use datafusion::common::tree_node::{Transformed, TreeNode, TreeNodeRecursion};
use datafusion::datasource::{MemTable, ViewTable, provider_as_source};
use datafusion::execution::memory_pool::{
    FairSpillPool, MemoryConsumer, MemoryReservation, TrackConsumersPool,
};
#[cfg(test)]
use datafusion::execution::memory_pool::{MemoryPool, UnboundedMemoryPool};
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::logical_expr::expr::Placeholder;
use datafusion::logical_expr::expr_fn::cast;
use datafusion::logical_expr::{Expr, JoinType, LogicalPlan, LogicalPlanBuilder, col, lit};
#[cfg(test)]
use datafusion::physical_plan::SendableRecordBatchStream;
use datafusion::physical_plan::metrics::MetricValue;
use datafusion::physical_plan::{
    ExecutionPlan, ExecutionPlanProperties, displayable, execute_stream,
};
use datafusion::prelude::{SQLOptions, SessionConfig, SessionContext};
use futures::StreamExt as _;
#[cfg(test)]
use rusqlite::types::Value;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{FabricError, SnapshotProviderCatalog};
use crate::operational_store::{OperationalReaderFactory, OperationalStoreError};
use crate::schema_registry::{
    ControlProjectionRole, MaterializationRole, OperationalTableSpec, OperationalWorkspaceScope,
    OverlayMutationPolicy, TableSpec, control_projection_specs, operational_table_spec,
    serving_projection_specs, serving_resource_profile, table_spec,
};
use crate::snapshot_runtime::SnapshotLeaseGuard;

const CATALOG_NAME: &str = "codefabric";
const BASE_SCHEMA: &str = "cpg_base";
const CONTROL_SCHEMA: &str = "cpg_control";
const SERVING_SCHEMA: &str = "cpg_serving";
const TRACKED_CONSUMER_COUNT: NonZeroUsize = NonZeroUsize::new(5).unwrap();
const EMPTY_SCHEMAS: [&str; 3] = ["cpg_python", "cpg_rust", "cpg_derived"];
const ALLOWED_SCALAR_FUNCTIONS: [&str; 9] = [
    "abs",
    "coalesce",
    "lower",
    "nullif",
    "octet_length",
    "substr",
    "substring",
    "trim",
    "upper",
];
const ALLOWED_AGGREGATE_FUNCTIONS: [&str; 7] =
    ["avg", "count", "max", "min", "sum", "bool_and", "bool_or"];

/// Stable failures at the leased serving-query boundary.
#[derive(Debug, Error)]
pub enum ServingQueryError {
    #[error("INVALID_REQUEST_SCHEMA:SERVING_CONFIGURATION:{0}")]
    Configuration(String),
    #[error("INVALID_REQUEST_SCHEMA:SERVING_PLAN_REJECTED:{0}")]
    PlanRejected(String),
    #[error("INTERNAL_INVARIANT_VIOLATION:SERVING_OPERATIONAL_CAPTURE:{0}")]
    OperationalCapture(String),
    #[error("QUERY_HARD_LIMIT_EXCEEDED:SERVING_RESOURCE_LIMIT:{0}")]
    ResourceLimit(String),
    #[error("serving I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    DataFusion(datafusion::error::DataFusionError),
    #[error(transparent)]
    Arrow(#[from] arrow_schema::ArrowError),
    #[error(transparent)]
    Operational(#[from] OperationalStoreError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Fabric(#[from] FabricError),
    #[error(transparent)]
    ResultChecksum(#[from] super::result_checksum::ResultChecksumError),
}

impl From<datafusion::error::DataFusionError> for ServingQueryError {
    fn from(error: datafusion::error::DataFusionError) -> Self {
        if matches!(
            error,
            datafusion::error::DataFusionError::ResourcesExhausted(_)
        ) {
            return Self::ResourceLimit(error.to_string());
        }
        let detail = error.to_string();
        if detail.contains("Resources exhausted") || detail.contains("allocation failed") {
            Self::ResourceLimit(detail)
        } else {
            Self::DataFusion(error)
        }
    }
}

/// Closed bounded runtime configuration for one leased query session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServingRuntimeConfig {
    memory_limit_bytes: usize,
    max_spill_bytes: u64,
    spill_directory: PathBuf,
    batch_size: usize,
    target_partitions: usize,
    max_output_rows: usize,
    max_output_bytes: usize,
    max_output_batches: usize,
    max_control_rows: usize,
    max_control_bytes: usize,
    max_control_batches: usize,
}

impl ServingRuntimeConfig {
    /// Validate a bounded runtime configuration before a DataFusion context exists.
    ///
    /// # Errors
    ///
    /// Rejects zero limits/partitions or a relative spill path.
    pub fn new(
        memory_limit_bytes: usize,
        max_spill_bytes: u64,
        spill_directory: PathBuf,
        target_partitions: usize,
    ) -> Result<Self, ServingQueryError> {
        if memory_limit_bytes == 0 || max_spill_bytes == 0 || target_partitions == 0 {
            return Err(ServingQueryError::Configuration(
                "memory, spill, and partition bounds must be positive".into(),
            ));
        }
        if !spill_directory.is_absolute() {
            return Err(ServingQueryError::Configuration(
                "spill directory must be absolute".into(),
            ));
        }
        let limits = serving_resource_profile();
        Ok(Self {
            memory_limit_bytes,
            max_spill_bytes,
            spill_directory,
            batch_size: limits.batch_size,
            target_partitions,
            max_output_rows: limits.max_output_rows,
            max_output_bytes: limits.max_output_bytes,
            max_output_batches: limits.max_output_batches,
            max_control_rows: limits.max_control_rows,
            max_control_bytes: limits.max_control_bytes,
            max_control_batches: limits.max_control_batches,
        })
    }

    #[cfg(test)]
    fn with_result_limits(mut self, rows: usize, bytes: usize, batches: usize) -> Self {
        self.max_output_rows = rows;
        self.max_output_bytes = bytes;
        self.max_output_batches = batches;
        self
    }

    #[cfg(test)]
    fn with_control_limits(mut self, rows: usize, bytes: usize, batches: usize) -> Self {
        self.max_control_rows = rows;
        self.max_control_bytes = bytes;
        self.max_control_batches = batches;
        self
    }

    #[cfg(test)]
    fn with_batch_size(mut self, batch_size: usize) -> Self {
        assert!(batch_size > 0, "test batch size must be positive");
        self.batch_size = batch_size;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServingRuntimeEvidence {
    pub memory_pool: String,
    pub memory_limit_bytes: usize,
    pub max_spill_bytes: u64,
    pub spill_directory: PathBuf,
    pub batch_size: usize,
    pub target_partitions: usize,
    pub observed_query_count: u64,
    pub observed_pruning_metric_count: u64,
    pub observed_pruned_row_groups: u64,
    pub observed_repartition_operator_count: u64,
    pub observed_repartition_output_rows: u64,
}

/// Identity allocated by the query boundary before any planning begins.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryExecutionContext {
    pub execution_id: String,
    pub semantic_request_id: String,
    pub mcp_call_id: String,
}

/// One emitted §110 query proof artifact without non-deterministic timing observations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryPlanArtifact {
    pub artifact_schema_version: String,
    pub execution_id: String,
    pub semantic_request_id: String,
    pub mcp_call_id: String,
    pub plan_template_id: String,
    pub bound_query_id: String,
    pub datafusion_version: String,
    pub arrow_version: String,
    pub bundle_ids: crate::snapshot::SnapshotBundles,
    pub snapshot_id: String,
    pub publication_id: String,
    pub source_table_versions: BTreeMap<u16, u64>,
    pub overlay_generation: u64,
    pub overlay_digest: String,
    pub overlay_table_versions: BTreeMap<u16, u64>,
    pub control_schema_generation_fingerprint: String,
    pub logical_plan: String,
    pub optimized_logical_plan: String,
    pub physical_plan: String,
    pub physical_plan_with_full_metrics: String,
    pub physical_plan_pg_json: String,
    pub output_schema: Vec<String>,
    pub output_partition_count: usize,
    pub output_row_count: usize,
    pub result_checksum_version: String,
    pub canonical_output_schema_digest: String,
    pub result_checksum: String,
    pub reproducibility: Reproducibility,
    pub execution_metrics: BTreeMap<String, u64>,
}

/// Machine-derived replay posture for one exact plan and pinned environment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reproducibility {
    pub deterministic: bool,
    pub inputs_pinned: bool,
    pub volatile_functions: Vec<String>,
    pub environment_recorded: bool,
}

/// Query rows and their exact non-timing plan artifact.
#[derive(Clone, Debug)]
pub struct ServingQueryResult {
    pub batches: Vec<RecordBatch>,
    pub artifact: QueryPlanArtifact,
    _reservation: Arc<MemoryReservation>,
}

/// One private DataFusion context that owns one durable snapshot lease.
pub struct ServingQuerySession {
    lease: SnapshotLeaseGuard,
    context: SessionContext,
    evidence: RwLock<ServingRuntimeEvidence>,
    _control_reservation: Arc<MemoryReservation>,
    control_schema_generation_fingerprint: String,
    limits: ServingRuntimeConfig,
}

impl std::fmt::Debug for ServingQuerySession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ServingQuerySession")
            .field("lease", &self.lease.record())
            .field("evidence", &self.runtime_evidence())
            .finish_non_exhaustive()
    }
}

impl ServingQuerySession {
    /// Build a private immutable catalog solely from one retained snapshot lease.
    ///
    /// # Errors
    ///
    /// Rejects invalid runtime bounds, missing snapshot providers/dimensions, malformed
    /// generated view metadata, operational capture drift, or DataFusion catalog failures.
    pub fn from_lease(
        lease: SnapshotLeaseGuard,
        operational: &OperationalReaderFactory,
        config: ServingRuntimeConfig,
    ) -> Result<Self, ServingQueryError> {
        std::fs::create_dir_all(&config.spill_directory).map_err(|source| {
            ServingQueryError::Io {
                path: config.spill_directory.clone(),
                source,
            }
        })?;
        let memory_pool = Arc::new(TrackConsumersPool::new(
            FairSpillPool::new(config.memory_limit_bytes),
            TRACKED_CONSUMER_COUNT,
        ));
        let runtime = Arc::new(
            RuntimeEnvBuilder::new()
                .with_memory_pool(memory_pool)
                .with_temp_file_path(&config.spill_directory)
                .with_max_temp_directory_size(config.max_spill_bytes)
                .build()?,
        );
        let session_config = SessionConfig::new()
            .with_default_catalog_and_schema(CATALOG_NAME, SERVING_SCHEMA)
            .with_batch_size(config.batch_size)
            .with_target_partitions(config.target_partitions)
            .with_parquet_pruning(true)
            .set_bool("datafusion.execution.parquet.pushdown_filters", false)
            .set_bool("datafusion.execution.parquet.reorder_filters", false)
            .with_repartition_joins(true)
            .with_repartition_aggregations(true);
        let context = SessionContext::new_with_config_rt(session_config, Arc::clone(&runtime));
        let snapshot = lease.snapshot();
        let workspace_id = snapshot
            .manifest()
            .raw_workspace_id()
            .map_err(|error| ServingQueryError::Configuration(error.to_string()))?;
        let (control, control_reservation, control_schema_generation_fingerprint) =
            capture_control_schema(
                operational,
                workspace_id,
                &lease,
                &runtime.memory_pool,
                &config,
            )?;
        let providers = snapshot.providers();
        let source_catalog = providers.catalog();
        let base = source_catalog.schema(BASE_SCHEMA).ok_or_else(|| {
            ServingQueryError::Configuration("snapshot has no cpg_base schema".into())
        })?;
        let serving = build_serving_schema(&providers)?;
        let mut schemas = BTreeMap::from([
            (BASE_SCHEMA.to_owned(), base),
            (CONTROL_SCHEMA.to_owned(), control),
            (SERVING_SCHEMA.to_owned(), serving),
        ]);
        for name in EMPTY_SCHEMAS {
            schemas.insert(
                name.to_owned(),
                Arc::new(ImmutableSchemaProvider::default()) as Arc<dyn SchemaProvider>,
            );
        }
        let catalog: Arc<dyn CatalogProvider> = Arc::new(ImmutableCatalogProvider { schemas });
        // DataFusion installs an empty provider for the configured default catalog.
        // Replacing that context-local placeholder is the public registration contract;
        // the installed catalog and schemas reject all subsequent mutation.
        context.register_catalog(CATALOG_NAME, catalog);
        let actual = context.state().config().clone();
        let evidence = ServingRuntimeEvidence {
            memory_pool: runtime.memory_pool.name().to_owned(),
            memory_limit_bytes: config.memory_limit_bytes,
            max_spill_bytes: config.max_spill_bytes,
            spill_directory: config.spill_directory.clone(),
            batch_size: actual.batch_size(),
            target_partitions: actual.target_partitions(),
            observed_query_count: 0,
            observed_pruning_metric_count: 0,
            observed_pruned_row_groups: 0,
            observed_repartition_operator_count: 0,
            observed_repartition_output_rows: 0,
        };
        Ok(Self {
            lease,
            context,
            evidence: RwLock::new(evidence),
            _control_reservation: Arc::new(control_reservation),
            control_schema_generation_fingerprint,
            limits: config,
        })
    }

    #[must_use]
    pub fn runtime_evidence(&self) -> ServingRuntimeEvidence {
        self.evidence
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    #[must_use]
    pub fn snapshot_id(&self) -> [u8; 16] {
        self.lease.record().snapshot_id
    }

    /// Immutable manifest that supplies the client-visible snapshot projection.
    #[must_use]
    pub fn snapshot_manifest(&self) -> crate::snapshot::ServingSnapshotManifest {
        self.lease.snapshot().manifest().clone()
    }

    /// Stable digest of every execution setting that may affect physical realization.
    pub(crate) fn execution_config_digest(&self) -> Result<String, ServingQueryError> {
        execution_config_digest(&self.limits)
    }

    /// Plan, verify, execute, and describe one read-only SQL query.
    ///
    /// # Errors
    ///
    /// Rejects non-query statements, unknown providers/functions, direct-file scans,
    /// planning failures, resource-limit failures, or execution failures.
    pub async fn query(&self, sql: &str) -> Result<ServingQueryResult, ServingQueryError> {
        let plan = self.context.state().create_logical_plan(sql).await?;
        read_only_options().verify_plan(&plan)?;
        validate_plan_allowlist(&plan)?;
        let execution = QueryExecutionContext {
            execution_id: format!("direct:{}", crate::integrity::framed_digest(sql.as_bytes())),
            semantic_request_id: "direct-sql".to_owned(),
            mcp_call_id: "not-applicable".to_owned(),
        };
        self.execute_plan(sql, plan, &execution).await
    }

    /// Resolve one immutable serving table into an application-owned logical-plan input.
    ///
    /// # Errors
    ///
    /// Returns an error when the table is absent from the snapshot-bound catalog.
    pub async fn table_plan(&self, table: &str) -> Result<LogicalPlan, ServingQueryError> {
        Ok(self.context.table(table).await?.into_unoptimized_plan())
    }

    /// Validate and execute a native DataFusion logical plan produced by the semantic compiler.
    ///
    /// # Errors
    ///
    /// Rejects plans that escape the immutable serving allowlist, fail optimization or physical
    /// planning, exceed resource limits, or fail during Arrow stream execution.
    pub async fn query_plan(
        &self,
        plan_identity: &str,
        plan: LogicalPlan,
    ) -> Result<ServingQueryResult, ServingQueryError> {
        let execution = QueryExecutionContext {
            execution_id: format!("direct:{plan_identity}"),
            semantic_request_id: plan_identity.to_owned(),
            mcp_call_id: "not-applicable".to_owned(),
        };
        self.query_plan_in_execution(plan_identity, plan, &execution)
            .await
    }

    /// Execute a native plan under a boundary-allocated execution identity.
    ///
    /// # Errors
    ///
    /// Rejects the same plan and execution failures as [`Self::query_plan`].
    pub async fn query_plan_in_execution(
        &self,
        plan_identity: &str,
        plan: LogicalPlan,
        execution: &QueryExecutionContext,
    ) -> Result<ServingQueryResult, ServingQueryError> {
        validate_plan_allowlist(&plan)?;
        self.execute_plan(plan_identity, plan, execution).await
    }

    /// Apply the post-lowering structural policy without executing the plan.
    ///
    /// # Errors
    ///
    /// Rejects unapproved tables, functions, or mutable/external plan families.
    pub fn validate_query_plan(&self, plan: &LogicalPlan) -> Result<(), ServingQueryError> {
        validate_plan_allowlist(plan)
    }

    #[allow(clippy::too_many_lines)] // Keeps one execution, metric capture, and result-accounting lifetime explicit.
    async fn execute_plan(
        &self,
        _request_label: &str,
        plan: LogicalPlan,
        execution: &QueryExecutionContext,
    ) -> Result<ServingQueryResult, ServingQueryError> {
        let state = self.context.state();
        let logical_plan = format!("{}", plan.display_indent_schema());
        let plan_template_id = logical_plan_template_id(&plan)?;
        let optimized = state.optimize(&plan)?;
        validate_plan_allowlist(&optimized)?;
        let physical = state.create_physical_plan(&optimized).await?;
        let output_partition_count = physical.output_partitioning().partition_count();
        let physical_plan = displayable(physical.as_ref()).indent(true).to_string();
        let output_schema = physical_output_schema(physical.as_ref());
        let reservation = Arc::new(
            MemoryConsumer::new("serving-query-result")
                .register(&self.context.runtime_env().memory_pool),
        );
        let task_context = datafusion::execution::TaskContext::from(&state)
            .with_task_id(execution.execution_id.clone());
        let mut stream = execute_stream(Arc::clone(&physical), Arc::new(task_context))?;
        let mut batches = Vec::new();
        let mut output_row_count = 0_usize;
        let mut output_bytes = 0_usize;
        while let Some(batch) = stream.next().await.transpose()? {
            output_row_count = output_row_count
                .checked_add(batch.num_rows())
                .ok_or_else(|| {
                    ServingQueryError::ResourceLimit("output row counter overflow".into())
                })?;
            output_bytes = output_bytes
                .checked_add(batch.get_array_memory_size())
                .ok_or_else(|| {
                    ServingQueryError::ResourceLimit("output byte counter overflow".into())
                })?;
            let batch_count = batches.len() + 1;
            if output_row_count > self.limits.max_output_rows
                || output_bytes > self.limits.max_output_bytes
                || batch_count > self.limits.max_output_batches
            {
                drop(stream);
                return Err(ServingQueryError::ResourceLimit(format!(
                    "query output exceeds generated rows/bytes/batches budget: \
                     {output_row_count}/{output_bytes}/{batch_count}"
                )));
            }
            reservation.try_grow(batch.get_array_memory_size())?;
            batches.push(batch);
        }
        // The output-byte limit governs delivered Arrow data. Checksum encoding is a distinct,
        // bounded working set, so charge it against the remaining session memory instead of
        // making exact-boundary result delivery fail because canonical schema bytes are additive.
        let checksum_encoding_budget = self
            .limits
            .memory_limit_bytes
            .checked_sub(output_bytes)
            .ok_or_else(|| {
                ServingQueryError::ResourceLimit(
                    "query output exhausts checksum working memory".to_owned(),
                )
            })?;
        let result = super::result_checksum::result_checksum_v1(
            physical.schema().as_ref(),
            &batches,
            checksum_encoding_budget,
        )?;
        let snapshot = self.lease.snapshot();
        let manifest = snapshot.manifest();
        let bound_query_id = bound_query_id(
            &plan_template_id,
            &logical_plan,
            &manifest.manifest_digest,
            &execution_config_digest(&self.limits)?,
        );
        let operator_metrics = physical_metrics(physical.as_ref());
        // Metrics are read only after the exact served stream is exhausted. Rendering this same
        // physical-plan instance avoids AnalyzeExec and diagnostic re-execution.
        let physical_plan_with_full_metrics =
            datafusion::physical_plan::display::DisplayableExecutionPlan::with_full_metrics(
                physical.as_ref(),
            )
            .set_show_schema(true)
            .indent(true)
            .to_string();
        let physical_plan_pg_json =
            datafusion::physical_plan::display::DisplayableExecutionPlan::with_full_metrics(
                physical.as_ref(),
            )
            .set_show_schema(true)
            .pgjson(true)
            .to_string();
        {
            let mut evidence = self
                .evidence
                .write()
                .expect("serving runtime evidence lock is not poisoned");
            evidence.observed_query_count = evidence.observed_query_count.saturating_add(1);
            evidence.observed_pruning_metric_count = evidence
                .observed_pruning_metric_count
                .saturating_add(operator_metrics.pruning_metric_count);
            evidence.observed_pruned_row_groups = evidence
                .observed_pruned_row_groups
                .saturating_add(operator_metrics.pruned_row_groups);
            evidence.observed_repartition_operator_count = evidence
                .observed_repartition_operator_count
                .saturating_add(operator_metrics.repartition_operator_count);
            evidence.observed_repartition_output_rows = evidence
                .observed_repartition_output_rows
                .saturating_add(operator_metrics.repartition_output_rows);
        }
        let artifact = QueryPlanArtifact {
            artifact_schema_version: "codefabric.query-plan-artifact.v1".to_owned(),
            execution_id: execution.execution_id.clone(),
            semantic_request_id: execution.semantic_request_id.clone(),
            mcp_call_id: execution.mcp_call_id.clone(),
            plan_template_id,
            bound_query_id,
            datafusion_version: datafusion::DATAFUSION_VERSION.to_owned(),
            arrow_version: arrow::ARROW_VERSION.to_owned(),
            bundle_ids: manifest.body.bundles.clone(),
            snapshot_id: manifest.snapshot_id.clone(),
            publication_id: manifest.body.base_publication.publication_id.clone(),
            source_table_versions: manifest
                .body
                .base_publication
                .tables
                .iter()
                .map(|table| (table.table_code, table.delta_version))
                .collect(),
            overlay_generation: manifest.body.overlay.overlay_generation,
            overlay_digest: manifest.body.overlay.overlay_digest.clone(),
            overlay_table_versions: manifest
                .body
                .overlay
                .tables
                .iter()
                .map(|table| (table.table_code, manifest.body.overlay.overlay_generation))
                .collect(),
            control_schema_generation_fingerprint: self
                .control_schema_generation_fingerprint
                .clone(),
            logical_plan,
            optimized_logical_plan: format!("{}", optimized.display_indent_schema()),
            physical_plan,
            physical_plan_with_full_metrics,
            physical_plan_pg_json,
            output_schema,
            output_partition_count,
            output_row_count,
            result_checksum_version: super::result_checksum::RESULT_CHECKSUM_VERSION.to_owned(),
            canonical_output_schema_digest: crate::integrity::framed_digest(
                &result.canonical_schema,
            ),
            result_checksum: result.checksum,
            reproducibility: Reproducibility {
                deterministic: true,
                inputs_pinned: true,
                volatile_functions: Vec::new(),
                environment_recorded: true,
            },
            execution_metrics: BTreeMap::from([
                ("output_partitions".into(), output_partition_count as u64),
                ("output_rows".into(), output_row_count as u64),
                ("output_bytes".into(), output_bytes as u64),
                ("output_batches".into(), batches.len() as u64),
                ("operator_output_rows".into(), operator_metrics.output_rows),
                ("spill_count".into(), operator_metrics.spill_count),
                ("spilled_bytes".into(), operator_metrics.spilled_bytes),
                (
                    "pruning_metric_count".into(),
                    operator_metrics.pruning_metric_count,
                ),
                (
                    "pruned_row_groups".into(),
                    operator_metrics.pruned_row_groups,
                ),
                (
                    "repartition_operator_count".into(),
                    operator_metrics.repartition_operator_count,
                ),
                (
                    "repartition_output_rows".into(),
                    operator_metrics.repartition_output_rows,
                ),
                (
                    "memory_reserved_after_execution".into(),
                    self.context.runtime_env().memory_pool.reserved() as u64,
                ),
            ]),
        };
        Ok(ServingQueryResult {
            batches,
            artifact,
            _reservation: reservation,
        })
    }

    #[cfg(test)]
    async fn query_with_barrier(
        &self,
        sql: &str,
        planned: &tokio::sync::Barrier,
        resume: &tokio::sync::Barrier,
    ) -> Result<ServingQueryResult, ServingQueryError> {
        let plan = self.context.state().create_logical_plan(sql).await?;
        read_only_options().verify_plan(&plan)?;
        validate_plan_allowlist(&plan)?;
        planned.wait().await;
        resume.wait().await;
        let execution = QueryExecutionContext {
            execution_id: format!("direct:{}", crate::integrity::framed_digest(sql.as_bytes())),
            semantic_request_id: "direct-sql".to_owned(),
            mcp_call_id: "not-applicable".to_owned(),
        };
        self.execute_plan(sql, plan, &execution).await
    }

    #[cfg(test)]
    async fn start_query_stream(
        &self,
        sql: &str,
    ) -> Result<SendableRecordBatchStream, ServingQueryError> {
        let plan = self.context.state().create_logical_plan(sql).await?;
        read_only_options().verify_plan(&plan)?;
        validate_plan_allowlist(&plan)?;
        let optimized = self.context.state().optimize(&plan)?;
        let physical = self
            .context
            .state()
            .create_physical_plan(&optimized)
            .await?;
        Ok(execute_stream(physical, self.context.task_ctx())?)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ObservedPhysicalMetrics {
    output_rows: u64,
    spill_count: u64,
    spilled_bytes: u64,
    pruning_metric_count: u64,
    pruned_row_groups: u64,
    repartition_operator_count: u64,
    repartition_output_rows: u64,
}

impl ObservedPhysicalMetrics {
    fn add(self, other: Self) -> Self {
        Self {
            output_rows: self.output_rows.saturating_add(other.output_rows),
            spill_count: self.spill_count.saturating_add(other.spill_count),
            spilled_bytes: self.spilled_bytes.saturating_add(other.spilled_bytes),
            pruning_metric_count: self
                .pruning_metric_count
                .saturating_add(other.pruning_metric_count),
            pruned_row_groups: self
                .pruned_row_groups
                .saturating_add(other.pruned_row_groups),
            repartition_operator_count: self
                .repartition_operator_count
                .saturating_add(other.repartition_operator_count),
            repartition_output_rows: self
                .repartition_output_rows
                .saturating_add(other.repartition_output_rows),
        }
    }
}

fn physical_metrics(plan: &dyn ExecutionPlan) -> ObservedPhysicalMetrics {
    let metrics = plan.metrics();
    let mut local = metrics
        .as_ref()
        .map_or_else(ObservedPhysicalMetrics::default, |metrics| {
            ObservedPhysicalMetrics {
                output_rows: metrics.output_rows().unwrap_or_default() as u64,
                spill_count: metrics.spill_count().unwrap_or_default() as u64,
                spilled_bytes: metrics.spilled_bytes().unwrap_or_default() as u64,
                ..ObservedPhysicalMetrics::default()
            }
        });
    if let Some(metrics) = metrics {
        for metric in metrics.iter() {
            if let MetricValue::PruningMetrics {
                pruning_metrics, ..
            } = metric.value()
            {
                local.pruning_metric_count = local.pruning_metric_count.saturating_add(1);
                local.pruned_row_groups = local
                    .pruned_row_groups
                    .saturating_add(pruning_metrics.pruned() as u64);
            }
        }
    }
    if plan.name() == "RepartitionExec" {
        local.repartition_operator_count = 1;
        local.repartition_output_rows = local.output_rows;
    }
    plan.children().into_iter().fold(local, |total, child| {
        total.add(physical_metrics(child.as_ref()))
    })
}

fn physical_output_schema(plan: &dyn ExecutionPlan) -> Vec<String> {
    plan.schema()
        .fields()
        .iter()
        .map(|field| field.name().clone())
        .collect()
}

fn read_only_options() -> SQLOptions {
    SQLOptions::new()
        .with_allow_ddl(false)
        .with_allow_dml(false)
        .with_allow_statements(false)
}

#[derive(Debug, Default)]
struct ImmutableSchemaProvider {
    tables: BTreeMap<String, Arc<dyn TableProvider>>,
}

#[async_trait]
impl SchemaProvider for ImmutableSchemaProvider {
    fn table_names(&self) -> Vec<String> {
        self.tables.keys().cloned().collect()
    }

    async fn table(&self, name: &str) -> datafusion::error::Result<Option<Arc<dyn TableProvider>>> {
        Ok(self.tables.get(name).cloned())
    }

    fn register_table(
        &self,
        name: String,
        _table: Arc<dyn TableProvider>,
    ) -> datafusion::error::Result<Option<Arc<dyn TableProvider>>> {
        Err(datafusion::error::DataFusionError::Plan(format!(
            "SERVING_CATALOG_FROZEN:cannot register table {name}"
        )))
    }

    fn deregister_table(
        &self,
        name: &str,
    ) -> datafusion::error::Result<Option<Arc<dyn TableProvider>>> {
        Err(datafusion::error::DataFusionError::Plan(format!(
            "SERVING_CATALOG_FROZEN:cannot deregister table {name}"
        )))
    }

    fn table_exist(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }
}

#[derive(Debug)]
struct ImmutableCatalogProvider {
    schemas: BTreeMap<String, Arc<dyn SchemaProvider>>,
}

impl CatalogProvider for ImmutableCatalogProvider {
    fn schema_names(&self) -> Vec<String> {
        self.schemas.keys().cloned().collect()
    }

    fn schema(&self, name: &str) -> Option<Arc<dyn SchemaProvider>> {
        self.schemas.get(name).cloned()
    }

    fn register_schema(
        &self,
        name: &str,
        _schema: Arc<dyn SchemaProvider>,
    ) -> datafusion::error::Result<Option<Arc<dyn SchemaProvider>>> {
        Err(datafusion::error::DataFusionError::Plan(format!(
            "SERVING_CATALOG_FROZEN:cannot register schema {name}"
        )))
    }

    fn deregister_schema(
        &self,
        name: &str,
        _cascade: bool,
    ) -> datafusion::error::Result<Option<Arc<dyn SchemaProvider>>> {
        Err(datafusion::error::DataFusionError::Plan(format!(
            "SERVING_CATALOG_FROZEN:cannot deregister schema {name}"
        )))
    }
}

fn build_serving_schema(
    providers: &SnapshotProviderCatalog,
) -> Result<Arc<dyn SchemaProvider>, ServingQueryError> {
    let enum_provider = providers.provider(11).ok_or_else(|| {
        ServingQueryError::Configuration("snapshot lacks enum_catalog dimension".into())
    })?;
    let mut tables = BTreeMap::new();
    for projection in serving_projection_specs() {
        let table_code = projection.source_table_code;
        let provider = providers.provider(table_code).ok_or_else(|| {
            ServingQueryError::Configuration(format!(
                "snapshot lacks required serving table {table_code}"
            ))
        })?;
        let spec = table_spec(table_code).ok_or_else(|| {
            ServingQueryError::Configuration(format!("unknown serving table {table_code}"))
        })?;
        validate_serving_spec(spec)?;
        let plan = serving_view_plan(spec, provider, &enum_provider)?;
        tables.insert(
            projection.view_name.to_owned(),
            Arc::new(ViewTable::new(plan, None)) as Arc<dyn TableProvider>,
        );
    }
    Ok(Arc::new(ImmutableSchemaProvider { tables }))
}

fn validate_serving_spec(spec: &TableSpec) -> Result<(), ServingQueryError> {
    if spec.materialization_role != MaterializationRole::DurableEffective {
        return Err(ServingQueryError::Configuration(format!(
            "table {} is not eligible for an effective serving view",
            spec.table_code
        )));
    }
    if spec.overlay_mutation == OverlayMutationPolicy::NotApplicable {
        return Err(ServingQueryError::Configuration(format!(
            "table {} has no closed overlay mutation policy",
            spec.table_code
        )));
    }
    Ok(())
}

fn serving_view_plan(
    spec: &crate::schema_registry::TableSpec,
    provider: Arc<dyn TableProvider>,
    enum_provider: &Arc<dyn TableProvider>,
) -> Result<LogicalPlan, ServingQueryError> {
    let source = format!("{}_source", spec.name);
    let mut builder = LogicalPlanBuilder::scan(source.clone(), provider_as_source(provider), None)?;
    let mut projection = spec
        .arrow_schema
        .fields()
        .iter()
        .filter(|field| {
            field
                .metadata()
                .get("com.codefabric.cpg.hidden_operational")
                .is_none_or(|value| value != "true")
        })
        .map(|field| col(format!("{source}.{}", field.name())))
        .collect::<Vec<_>>();
    for field in spec.arrow_schema.fields() {
        let Some(domain) = field
            .metadata()
            .get("com.codefabric.cpg.semantic_type")
            .and_then(|value| value.strip_prefix("enum:"))
        else {
            continue;
        };
        let alias = format!("enum_{}", field.name());
        let right = LogicalPlanBuilder::scan(
            alias.clone(),
            provider_as_source(Arc::clone(enum_provider)),
            None,
        )?
        .build()?;
        let code = cast(col(format!("{source}.{}", field.name())), DataType::Int32)
            .eq(col(format!("{alias}.code")));
        let domain_match = col(format!("{alias}.domain")).eq(lit(domain.to_owned()));
        builder = builder.join_on(right, JoinType::Left, [code.and(domain_match)])?;
        projection.push(col(format!("{alias}.name")).alias(format!("{}_name", field.name())));
    }
    Ok(builder.project(projection)?.build()?)
}

fn capture_control_schema(
    factory: &OperationalReaderFactory,
    workspace_id: [u8; 16],
    lease: &SnapshotLeaseGuard,
    memory_pool: &Arc<dyn datafusion::execution::memory_pool::MemoryPool>,
    config: &ServingRuntimeConfig,
) -> Result<(Arc<dyn SchemaProvider>, MemoryReservation, String), ServingQueryError> {
    let reader = factory.open()?;
    let reservation = MemoryConsumer::new("serving-control-capture").register(memory_pool);
    let mut budget = CaptureBudget {
        rows: 0,
        bytes: 0,
        batches: 0,
        max_rows: config.max_control_rows,
        max_bytes: config.max_control_bytes,
        max_batches: config.max_control_batches,
        reservation: &reservation,
    };
    let raw = reader.with_connection_result(|connection| -> Result<_, ServingQueryError> {
        connection.execute_batch("BEGIN DEFERRED")?;
        let result = control_projection_specs()
            .iter()
            .filter(|projection| {
                projection.projection_role == ControlProjectionRole::OperationalSource
            })
            .map(|projection| {
                let name = projection.source_table.ok_or_else(|| {
                    rusqlite::Error::InvalidParameterName(projection.view_name.to_owned())
                })?;
                let spec = operational_table_spec(name)
                    .ok_or_else(|| rusqlite::Error::InvalidParameterName(name.to_owned()))?;
                capture_raw_operational_table(
                    connection,
                    spec,
                    workspace_id,
                    serving_resource_profile().batch_size,
                    &mut budget,
                )
                .map(|rows| (projection.view_name.to_owned(), rows))
            })
            .collect::<Result<BTreeMap<_, _>, ServingQueryError>>();
        match result {
            Ok(tables) => {
                connection.execute_batch("COMMIT")?;
                Ok(tables)
            }
            Err(error) => {
                let _ = connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    })?;
    let active = active_snapshot_batch(lease)?;
    let mut fingerprint_input = Vec::new();
    fingerprint_input.extend_from_slice(
        &lease
            .snapshot()
            .manifest()
            .body
            .source
            .source_generation
            .to_be_bytes(),
    );
    for (name, batches) in &raw {
        fingerprint_input.extend_from_slice(&(name.len() as u64).to_be_bytes());
        fingerprint_input.extend_from_slice(name.as_bytes());
        for batch in batches {
            fingerprint_input.extend_from_slice(
                &crate::fabric::batch_checksum(batch)
                    .map_err(|error| ServingQueryError::Configuration(error.to_string()))?,
            );
        }
    }
    fingerprint_input.extend_from_slice(
        &crate::fabric::batch_checksum(&active)
            .map_err(|error| ServingQueryError::Configuration(error.to_string()))?,
    );
    let control_schema_generation_fingerprint = crate::integrity::framed_digest(&fingerprint_input);
    let mut tables = BTreeMap::new();
    for (name, batches) in raw {
        let spec = operational_table_spec(&name).expect("captured generated table");
        tables.insert(
            name,
            mem_table_batches(Arc::clone(&spec.arrow_schema), batches)?,
        );
    }
    install_derived_control_views(&mut tables)?;
    budget.retain(&active)?;
    for projection in control_projection_specs().iter().filter(|projection| {
        projection.projection_role == ControlProjectionRole::ActiveServingSnapshot
    }) {
        tables.insert(projection.view_name.to_owned(), mem_table(active.clone())?);
    }
    Ok((
        Arc::new(ImmutableSchemaProvider { tables }),
        reservation,
        control_schema_generation_fingerprint,
    ))
}

struct CaptureBudget<'a> {
    rows: usize,
    bytes: usize,
    batches: usize,
    max_rows: usize,
    max_bytes: usize,
    max_batches: usize,
    reservation: &'a MemoryReservation,
}

impl CaptureBudget<'_> {
    fn retain(&mut self, batch: &RecordBatch) -> Result<(), ServingQueryError> {
        self.rows = self.rows.checked_add(batch.num_rows()).ok_or_else(|| {
            ServingQueryError::ResourceLimit("control row counter overflow".into())
        })?;
        let bytes = batch.get_array_memory_size();
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(|| {
            ServingQueryError::ResourceLimit("control byte counter overflow".into())
        })?;
        self.batches = self.batches.checked_add(1).ok_or_else(|| {
            ServingQueryError::ResourceLimit("control batch counter overflow".into())
        })?;
        if self.rows > self.max_rows
            || self.bytes > self.max_bytes
            || self.batches > self.max_batches
        {
            return Err(ServingQueryError::ResourceLimit(format!(
                "control capture exceeds generated rows/bytes/batches budget: {}/{}/{}",
                self.rows, self.bytes, self.batches
            )));
        }
        self.reservation.try_grow(bytes)?;
        Ok(())
    }
}

fn capture_raw_operational_table(
    connection: &Connection,
    spec: &OperationalTableSpec,
    workspace_id: [u8; 16],
    batch_size: usize,
    budget: &mut CaptureBudget<'_>,
) -> Result<Vec<RecordBatch>, ServingQueryError> {
    let select = spec
        .arrow_schema
        .fields()
        .iter()
        .map(|field| format!("c.\"{}\"", field.name()))
        .collect::<Vec<_>>()
        .join(", ");
    let order = spec
        .primary_key
        .iter()
        .map(|field| format!("c.\"{field}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = match spec.workspace_scope.ok_or_else(|| {
        rusqlite::Error::InvalidParameterName(format!(
            "{} has no generated workspace scope",
            spec.name
        ))
    })? {
        OperationalWorkspaceScope::Direct { workspace_column } => format!(
            "SELECT {select} FROM \"{}\" AS c WHERE c.\"{workspace_column}\"=?1 \
             ORDER BY {order}",
            spec.name
        ),
        OperationalWorkspaceScope::ViaParent {
            parent_table,
            child_column,
            parent_column,
            workspace_column,
        } => format!(
            "SELECT {select} FROM \"{}\" AS c JOIN \"{parent_table}\" AS p \
             ON p.\"{parent_column}\"=c.\"{child_column}\" \
             WHERE p.\"{workspace_column}\"=?1 ORDER BY {order}",
            spec.name
        ),
    };
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(params![workspace_id.as_slice()])?;
    let mut batches = Vec::new();
    let mut builders = operational_builders(spec)?;
    let mut pending_rows = 0_usize;
    while let Some(row) = rows.next()? {
        for (index, (builder, field)) in builders
            .iter_mut()
            .zip(spec.arrow_schema.fields())
            .enumerate()
        {
            builder.append(field, row.get_ref(index)?)?;
        }
        pending_rows += 1;
        if capture_batch_is_full(pending_rows, batch_size) {
            let batch = finish_operational_batch(spec, &mut builders, pending_rows)?;
            budget.retain(&batch)?;
            batches.push(batch);
            pending_rows = 0;
        }
    }
    if pending_rows != 0 {
        let batch = finish_operational_batch(spec, &mut builders, pending_rows)?;
        budget.retain(&batch)?;
        batches.push(batch);
    }
    if batches.is_empty() {
        batches.push(RecordBatch::new_empty(Arc::clone(&spec.arrow_schema)));
    }
    Ok(batches)
}

const fn capture_batch_is_full(pending_rows: usize, batch_size: usize) -> bool {
    pending_rows == batch_size
}

enum OperationalBuilder {
    Int64(Int64Builder),
    Float64(Float64Builder),
    Utf8(StringBuilder),
    Binary(BinaryBuilder),
}

impl OperationalBuilder {
    fn append(&mut self, field: &Field, value: ValueRef<'_>) -> Result<(), ServingQueryError> {
        match (self, value) {
            (Self::Int64(builder), ValueRef::Integer(value)) => builder.append_value(value),
            (Self::Float64(builder), ValueRef::Real(value)) => builder.append_value(value),
            (Self::Utf8(builder), ValueRef::Text(value)) => builder.append_value(
                std::str::from_utf8(value)
                    .map_err(|error| ServingQueryError::OperationalCapture(error.to_string()))?,
            ),
            (Self::Binary(builder), ValueRef::Blob(value)) => builder.append_value(value),
            (Self::Int64(builder), ValueRef::Null) if field.is_nullable() => builder.append_null(),
            (Self::Float64(builder), ValueRef::Null) if field.is_nullable() => {
                builder.append_null();
            }
            (Self::Utf8(builder), ValueRef::Null) if field.is_nullable() => builder.append_null(),
            (Self::Binary(builder), ValueRef::Null) if field.is_nullable() => {
                builder.append_null();
            }
            _ => return Err(operational_type_error(field)),
        }
        Ok(())
    }

    fn finish(&mut self) -> ArrayRef {
        match self {
            Self::Int64(builder) => Arc::new(builder.finish()),
            Self::Float64(builder) => Arc::new(builder.finish()),
            Self::Utf8(builder) => Arc::new(builder.finish()),
            Self::Binary(builder) => Arc::new(builder.finish()),
        }
    }
}

fn operational_builders(
    spec: &OperationalTableSpec,
) -> Result<Vec<OperationalBuilder>, ServingQueryError> {
    spec.arrow_schema
        .fields()
        .iter()
        .map(|field| match field.data_type() {
            DataType::Int64 => Ok(OperationalBuilder::Int64(Int64Builder::new())),
            DataType::Float64 => Ok(OperationalBuilder::Float64(Float64Builder::new())),
            DataType::Utf8 => Ok(OperationalBuilder::Utf8(StringBuilder::new())),
            DataType::Binary => Ok(OperationalBuilder::Binary(BinaryBuilder::new())),
            data_type => Err(ServingQueryError::OperationalCapture(format!(
                "generated operational type {data_type:?} is unsupported"
            ))),
        })
        .collect()
}

fn finish_operational_batch(
    spec: &OperationalTableSpec,
    builders: &mut [OperationalBuilder],
    rows: usize,
) -> Result<RecordBatch, ServingQueryError> {
    let columns = builders
        .iter_mut()
        .map(OperationalBuilder::finish)
        .collect::<Vec<_>>();
    let batch = RecordBatch::try_new(Arc::clone(&spec.arrow_schema), columns)?;
    if batch.num_rows() != rows {
        return Err(ServingQueryError::OperationalCapture(
            "generated operational builders disagree on row count".into(),
        ));
    }
    Ok(batch)
}

#[cfg(test)]
fn values_to_array(field: &Field, values: &[&Value]) -> Result<ArrayRef, ServingQueryError> {
    match field.data_type() {
        DataType::Int64 => {
            let mut builder = Int64Builder::with_capacity(values.len());
            for value in values {
                match value {
                    Value::Integer(value) => builder.append_value(*value),
                    Value::Null if field.is_nullable() => builder.append_null(),
                    _ => return Err(operational_type_error(field)),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Float64 => {
            let mut builder = Float64Builder::with_capacity(values.len());
            for value in values {
                match value {
                    Value::Real(value) => builder.append_value(*value),
                    Value::Null if field.is_nullable() => builder.append_null(),
                    _ => return Err(operational_type_error(field)),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Utf8 => {
            let mut builder = StringBuilder::new();
            for value in values {
                match value {
                    Value::Text(value) => builder.append_value(value),
                    Value::Null if field.is_nullable() => builder.append_null(),
                    _ => return Err(operational_type_error(field)),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        DataType::Binary => {
            let mut builder = BinaryBuilder::new();
            for value in values {
                match value {
                    Value::Blob(value) => builder.append_value(value),
                    Value::Null if field.is_nullable() => builder.append_null(),
                    _ => return Err(operational_type_error(field)),
                }
            }
            Ok(Arc::new(builder.finish()))
        }
        data_type => Err(ServingQueryError::OperationalCapture(format!(
            "generated operational type {data_type:?} is unsupported"
        ))),
    }
}

fn operational_type_error(field: &Field) -> ServingQueryError {
    ServingQueryError::OperationalCapture(format!(
        "SQLite value violates generated field {}:{:?}",
        field.name(),
        field.data_type()
    ))
}

fn mem_table(batch: RecordBatch) -> Result<Arc<dyn TableProvider>, ServingQueryError> {
    Ok(Arc::new(MemTable::try_new(
        batch.schema(),
        vec![vec![batch]],
    )?))
}

fn mem_table_batches(
    schema: Arc<Schema>,
    batches: Vec<RecordBatch>,
) -> Result<Arc<dyn TableProvider>, ServingQueryError> {
    Ok(Arc::new(MemTable::try_new(schema, vec![batches])?))
}

fn install_derived_control_views(
    tables: &mut BTreeMap<String, Arc<dyn TableProvider>>,
) -> Result<(), ServingQueryError> {
    for projection in control_projection_specs().iter().filter(|projection| {
        projection.projection_role == ControlProjectionRole::DerivedOperational
    }) {
        let source_name = projection.source_table.ok_or_else(|| {
            ServingQueryError::Configuration(format!(
                "derived control view {} lacks a source",
                projection.view_name
            ))
        })?;
        let source = tables.get(source_name).cloned().ok_or_else(|| {
            ServingQueryError::Configuration(format!(
                "control capture lacks {source_name} for {}",
                projection.view_name
            ))
        })?;
        let plan = LogicalPlanBuilder::scan(
            format!("{source_name}_capture"),
            provider_as_source(source),
            None,
        )?
        .project(
            projection
                .columns
                .iter()
                .map(|column| col(*column))
                .collect::<Vec<_>>(),
        )?
        .build()?;
        tables.insert(
            projection.view_name.to_owned(),
            Arc::new(ViewTable::new(plan, None)) as Arc<dyn TableProvider>,
        );
    }
    Ok(())
}

fn active_snapshot_batch(lease: &SnapshotLeaseGuard) -> Result<RecordBatch, ServingQueryError> {
    let snapshot = lease.snapshot();
    let manifest = snapshot.manifest();
    let publication_id = manifest
        .raw_publication_id()
        .map_err(|error| ServingQueryError::Configuration(error.to_string()))?;
    let schema = Arc::new(Schema::new(vec![
        Field::new("snapshot_id", DataType::Binary, false),
        Field::new("workspace_id", DataType::Binary, false),
        Field::new("publication_id", DataType::Binary, false),
        Field::new("source_generation", DataType::Int64, false),
        Field::new("overlay_generation", DataType::Int64, false),
        Field::new("captured_at", DataType::Int64, false),
        Field::new("consistency", DataType::Utf8, false),
    ]));
    let overlay_generation = i64::try_from(manifest.body.overlay.overlay_generation)
        .map_err(|_| ServingQueryError::Configuration("overlay generation exceeds i64".into()))?;
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(BinaryArray::from(vec![Some(
                lease.record().snapshot_id.as_slice(),
            )])),
            Arc::new(BinaryArray::from(vec![Some(
                lease.record().workspace_id.as_slice(),
            )])),
            Arc::new(BinaryArray::from(vec![Some(publication_id.as_slice())])),
            Arc::new(Int64Array::from(vec![
                i64::try_from(manifest.body.source.source_generation).map_err(|_| {
                    ServingQueryError::Configuration("source generation exceeds i64".into())
                })?,
            ])),
            Arc::new(Int64Array::from(vec![overlay_generation])),
            Arc::new(Int64Array::from(vec![
                i64::try_from(lease.record().created_at).map_err(|_| {
                    ServingQueryError::Configuration("lease time exceeds i64".into())
                })?,
            ])),
            Arc::new(StringArray::from(vec![
                "operationally-current-not-snapshot-pinned",
            ])),
        ],
    )?)
}

fn validate_plan_allowlist(plan: &LogicalPlan) -> Result<(), ServingQueryError> {
    plan.apply(|node| {
        if let LogicalPlan::TableScan(scan) = node
            && !allowed_table_reference(&scan.table_name.to_string())
        {
            return Err(datafusion::error::DataFusionError::Plan(format!(
                "SERVING_PLAN_REJECTED:unauthorized provider {}",
                scan.table_name
            )));
        }
        if matches!(node, LogicalPlan::Extension(_)) {
            return Err(datafusion::error::DataFusionError::Plan(
                "SERVING_PLAN_REJECTED:logical extensions are not allowlisted".into(),
            ));
        }
        for expression in node.expressions() {
            expression.apply(|expression| {
                match expression {
                    Expr::ScalarVariable(..) => {
                        return Err(datafusion::error::DataFusionError::Plan(
                            "SERVING_PLAN_REJECTED:session variables are forbidden".into(),
                        ));
                    }
                    Expr::ScalarFunction(function)
                        if !ALLOWED_SCALAR_FUNCTIONS.contains(&function.name()) =>
                    {
                        return Err(datafusion::error::DataFusionError::Plan(format!(
                            "SERVING_PLAN_REJECTED:scalar function {}",
                            function.name()
                        )));
                    }
                    Expr::AggregateFunction(function)
                        if !ALLOWED_AGGREGATE_FUNCTIONS.contains(&function.func.name()) =>
                    {
                        return Err(datafusion::error::DataFusionError::Plan(format!(
                            "SERVING_PLAN_REJECTED:aggregate function {}",
                            function.func.name()
                        )));
                    }
                    Expr::WindowFunction(_) => {
                        return Err(datafusion::error::DataFusionError::Plan(
                            "SERVING_PLAN_REJECTED:window functions are not allowlisted".into(),
                        ));
                    }
                    _ => {}
                }
                Ok(TreeNodeRecursion::Continue)
            })?;
        }
        Ok(TreeNodeRecursion::Continue)
    })
    .map_err(|error| ServingQueryError::PlanRejected(error.to_string()))?;
    Ok(())
}

fn allowed_table_reference(reference: &str) -> bool {
    if reference.contains('/')
        || reference.contains('\\')
        || reference.contains(':')
        || reference.contains("..")
    {
        return false;
    }
    let parts = reference.split('.').collect::<Vec<_>>();
    parts.iter().any(|part| {
        matches!(
            part.trim_matches('"'),
            CONTROL_SCHEMA
                | BASE_SCHEMA
                | SERVING_SCHEMA
                | "cpg_python"
                | "cpg_rust"
                | "cpg_derived"
        )
    }) || (parts.len() == 1
        && (table_spec_by_name(reference).is_some()
            || reference.ends_with("_source")
            || reference.starts_with("enum_")
            || control_projection_specs().iter().any(|projection| {
                projection
                    .source_table
                    .is_some_and(|source| reference == format!("{source}_capture"))
            })))
}

fn table_spec_by_name(name: &str) -> Option<&'static TableSpec> {
    crate::schema_registry::table_specs()
        .iter()
        .find(|spec| spec.name == name)
}

fn update_length_framed(
    fingerprint: &mut crate::identity::SemanticFingerprintBuilder,
    value: &[u8],
) {
    fingerprint.update(&(value.len() as u64).to_be_bytes());
    fingerprint.update(value);
}

fn parameterized_logical_plan(plan: &LogicalPlan) -> Result<LogicalPlan, ServingQueryError> {
    let mut parameter_index = 0_u64;
    Ok(plan
        .clone()
        .transform_up_with_subqueries(|plan| {
            plan.map_expressions(|expression| {
                expression.transform_up(|expression| match expression {
                    Expr::Literal(value, metadata) => {
                        parameter_index = parameter_index.checked_add(1).ok_or_else(|| {
                            datafusion::error::DataFusionError::Plan(
                                "plan-template parameter counter overflow".to_owned(),
                            )
                        })?;
                        let mut field = Field::new(
                            format!("parameter_{parameter_index}"),
                            value.data_type(),
                            value.is_null(),
                        );
                        if let Some(metadata) = metadata {
                            field = metadata.add_to_field(field);
                        }
                        Ok(Transformed::yes(Expr::Placeholder(
                            Placeholder::new_with_field(
                                format!("$parameter_{parameter_index}"),
                                Some(Arc::new(field)),
                            ),
                        )))
                    }
                    expression => Ok(Transformed::no(expression)),
                })
            })
        })?
        .data)
}

pub(crate) fn logical_plan_template_serialization(
    logical_plan: &LogicalPlan,
) -> Result<String, ServingQueryError> {
    Ok(parameterized_logical_plan(logical_plan)?
        .display_indent()
        .to_string())
}

fn logical_plan_template_id(logical_plan: &LogicalPlan) -> Result<String, ServingQueryError> {
    let value = serde_json::json!({
        "version": "QueryPlanTemplateV1",
        "datafusion_version": datafusion::DATAFUSION_VERSION,
        "logical_plan": logical_plan_template_serialization(logical_plan)?,
    });
    let canonical = crate::contracts::jcs::canonicalize_value(&value)
        .map_err(|error| ServingQueryError::Configuration(error.to_string()))?;
    let mut fingerprint = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::QueryPlanTemplateV1,
    );
    update_length_framed(&mut fingerprint, &canonical);
    Ok(crate::integrity::frame_digest(fingerprint.finalize()))
}

fn execution_config_digest(config: &ServingRuntimeConfig) -> Result<String, ServingQueryError> {
    let value = serde_json::json!({
        "version": "ServingExecutionConfigV1",
        "memory_limit_bytes": config.memory_limit_bytes,
        "max_spill_bytes": config.max_spill_bytes,
        "batch_size": config.batch_size,
        "target_partitions": config.target_partitions,
        "max_output_rows": config.max_output_rows,
        "max_output_bytes": config.max_output_bytes,
        "max_output_batches": config.max_output_batches,
    });
    let canonical = crate::contracts::jcs::canonicalize_value(&value)
        .map_err(|error| ServingQueryError::Configuration(error.to_string()))?;
    Ok(crate::integrity::framed_digest(&canonical))
}

fn bound_query_id(
    plan_template_id: &str,
    bound_logical_plan: &str,
    snapshot_manifest_digest: &str,
    execution_config_digest: &str,
) -> String {
    let mut fingerprint = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::BoundSemanticQueryV1,
    );
    for value in [
        plan_template_id.as_bytes(),
        bound_logical_plan.as_bytes(),
        snapshot_manifest_digest.as_bytes(),
        execution_config_digest.as_bytes(),
    ] {
        update_length_framed(&mut fingerprint, value);
    }
    crate::integrity::frame_digest(fingerprint.finalize())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use arrow_array::builder::{BinaryBuilder, FixedSizeBinaryBuilder};
    use arrow_array::{
        BooleanArray, Float64Array, Int16Array, Int32Array, Int64Array, ListArray, StringArray,
        TimestampMicrosecondArray, UInt64Array, new_null_array,
    };
    use arrow_buffer::OffsetBuffer;
    use arrow_schema::TimeUnit;
    use hyper_util::rt::TokioIo;
    use rusqlite::params;
    use tempfile::{TempDir, tempdir};
    use tokio::net::UnixStream;
    use tokio::sync::Barrier;
    use tonic::transport::Endpoint;
    use tower::service_fn;

    use super::*;
    use crate::continuous::{ContinuousWorkspaceConfig, ContinuousWorkspaceEngine};
    use crate::daemon::{
        AdminCommand, DaemonConfig, ReloadableConfig, StaticConfig, administer,
        serve_with_query_backend, wait_for_discovery,
    };
    use crate::fabric::SnapshotOverlayProviderFactory as _;
    use crate::fact_ingest::{EntityRow, FactScope, ValidatedFactBatch, encode_entities};
    use crate::git_state::{GitCandidatePlanner, GitStateObservations, GixGitStateAdapter};
    use crate::identity::{IdentityDomain, encode_public_id};
    use crate::lifecycle::{
        LifecycleConfig, OverlayFlushPolicy, UpdateWaveScheduler, WatchHint, WatchHintBatch,
        WatchHintKind, prove_serving_rebuild_equivalence,
    };
    use crate::operational_store::{OperationalStore, OperationalStoreError};
    use crate::query_service::{
        PersistedQueryArtifactBundle, QueryArtifactPhase, ResultArtifactStore, VersionExplanation,
        WorkspaceQueryBackend, host_capability_profile_digest,
    };
    use crate::registries::{CpgdFeatureMask, SnapshotLeaseKind, WorkspaceRegistryLifecycle};
    use crate::rpc::generated::codefabric::cpgd::v1::cpg_query_service_client::CpgQueryServiceClient;
    use crate::rpc::generated::codefabric::cpgd::v1::query_event::Event;
    use crate::rpc::generated::codefabric::cpgd::v1::{
        CredentialProof, DeliveryPreference, HandshakeRequest, HostCapabilityProfile,
        PayloadCompression, ReadResultRequest, StartQueryRequest, StatusRequest,
        StreamQueryRequest, VersionRange, WorkspaceClaim, WorkspaceReadiness,
    };
    use crate::snapshot::{
        ServingSnapshotManifestBody, SnapshotBasePublication, SnapshotBundles,
        SnapshotContextRecord, SnapshotContexts, SnapshotIndexes, SnapshotOverlay, SnapshotSource,
    };
    use crate::snapshot_runtime::{
        ServingSnapshotCandidate, ServingSnapshotRuntime, SnapshotLeaseManager,
    };
    use crate::source_image::{SourceCapturePolicy, SourceImageStore};
    use crate::workspace_registry::{WorkspaceRecord, WorkspaceRegistry};

    const WORKSPACE: [u8; 16] = [0x11; 16];
    const CONTEXT: [u8; 16] = crate::identity::SOURCE_CONTEXT_ID;
    const OVERLAY: [u8; 32] = [0x33; 32];

    fn daemon_config(root: &std::path::Path) -> DaemonConfig {
        for path in [
            root.join("state"),
            root.join("runtime"),
            root.join("config"),
        ] {
            fs::create_dir_all(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let token = root.join("config/query.capability");
        fs::write(&token, b"test-query-capability-token").unwrap();
        fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
        DaemonConfig {
            static_config: StaticConfig {
                state_root: root.join("state"),
                runtime_root: root.join("runtime"),
                config_root: root.join("config"),
                socket_endpoint: root.join("runtime/admin.sock"),
                query_socket_endpoint: root.join("runtime/query.sock"),
                query_capability_token_file: PathBuf::from("query.capability"),
                operational_database: PathBuf::from("operational.sqlite3"),
                bundle_index: PathBuf::from(
                    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json",
                ),
                toolchain_identity: PathBuf::from("contracts/toolchain/toolchain-identity.json"),
                sandbox_policy: "required-for-untrusted".to_owned(),
                hard_limit_profile: "daemon-default-v1".to_owned(),
                supported_platform_profile: "local-workstation-v1".to_owned(),
            },
            reloadable: ReloadableConfig {
                log_level: "info".to_owned(),
                telemetry_sampling: 0.1,
                soft_query_quota: 4,
                maintenance_schedule: "daily-idle".to_owned(),
            },
        }
    }

    async fn daemon_query_client(
        socket: PathBuf,
    ) -> CpgQueryServiceClient<tonic::transport::Channel> {
        let channel = Endpoint::try_from("http://[::]:50051")
            .unwrap()
            .connect_with_connector(service_fn(move |_| {
                let socket = socket.clone();
                async move { UnixStream::connect(socket).await.map(TokioIo::new) }
            }))
            .await
            .unwrap();
        CpgQueryServiceClient::new(channel)
    }

    fn digest(byte: u8) -> String {
        crate::integrity::frame_digest([byte; 32])
    }

    fn snapshot_body(source_generation: u64) -> ServingSnapshotManifestBody {
        snapshot_body_for(WORKSPACE, source_generation, digest(1))
    }

    fn snapshot_body_for(
        workspace_id: [u8; 16],
        source_generation: u64,
        inventory_digest: String,
    ) -> ServingSnapshotManifestBody {
        ServingSnapshotManifestBody {
            manifest_version: "1.0".into(),
            workspace_id: encode_public_id(IdentityDomain::Workspace, None, workspace_id).unwrap(),
            repository_id: None,
            worktree_id: None,
            registration_revision: 1,
            source: SnapshotSource {
                source_generation,
                admitted_event_sequence: source_generation,
                reconciled_event_sequence: source_generation,
                inventory_digest,
                authorization_fingerprint: digest(2),
                inclusion_policy_fingerprint: digest(3),
                path_profile_version: "1".into(),
                source_trust_state: "CURRENT".into(),
                event_stream_health: "HEALTHY".into(),
                git_acceleration_status: "AVAILABLE".into(),
                git_state_fingerprint: Some(digest(4)),
            },
            contexts: SnapshotContexts {
                context_set_id: encode_public_id(
                    IdentityDomain::ContextSet,
                    None,
                    crate::identity::context_set_identity(workspace_id, &[CONTEXT])
                        .unwrap()
                        .id,
                )
                .unwrap(),
                default_python_context_id: None,
                default_rust_context_id: None,
                records: vec![SnapshotContextRecord {
                    analysis_context_id: encode_public_id(
                        IdentityDomain::AnalysisContext,
                        None,
                        CONTEXT,
                    )
                    .unwrap(),
                    context_manifest_digest: digest(9),
                    capability_partition_digest: digest(10),
                }],
            },
            base_publication: SnapshotBasePublication {
                publication_id: String::new(),
                tables: Vec::new(),
            },
            overlay: SnapshotOverlay {
                overlay_generation: 0,
                overlay_digest: digest(0),
                total_memory_bytes: 0,
                tables: Vec::new(),
            },
            indexes: SnapshotIndexes {
                capability_index_digest: digest(5),
                diagnostic_index_digest: digest(6),
                dependency_graph_digest: digest(7),
            },
            bundles: SnapshotBundles {
                ontology_bundle_id: "ontology:1.0".into(),
                schema_bundle_id: "schema:1.0".into(),
                provider_bundle_id: "provider:1.0".into(),
                derivation_bundle_id: "derivation:1.0".into(),
                query_language_bundle_id: "query:1.0".into(),
                model_pack_bundle_id: "model:1.0".into(),
                toolchain_bundle_id: "toolchain:1.0".into(),
            },
            limits_profile_digest: digest(8),
            source_blob_digests: Vec::new(),
        }
    }

    fn generated_batch(table_code: i16, rows: usize) -> RecordBatch {
        generated_batch_at_generation(table_code, rows, 1)
    }

    fn generated_batch_at_generation(
        table_code: i16,
        rows: usize,
        source_generation: i64,
    ) -> RecordBatch {
        let spec = table_spec(table_code).expect("fixture table is generated");
        let columns = spec
            .arrow_schema
            .fields()
            .iter()
            .map(|field| fixture_array(table_code, field, rows, source_generation))
            .collect::<Vec<_>>();
        RecordBatch::try_new(Arc::clone(&spec.arrow_schema), columns).unwrap()
    }

    fn fixture_array(
        table_code: i16,
        field: &Field,
        rows: usize,
        source_generation: i64,
    ) -> ArrayRef {
        if field.is_nullable() {
            return new_null_array(field.data_type(), rows);
        }
        match field.data_type() {
            DataType::Binary => {
                let width = if field.name().contains("digest")
                    || field.name().contains("fingerprint")
                    || field.name().contains("hash")
                {
                    32
                } else {
                    16
                };
                let mut builder = BinaryBuilder::new();
                for row in 0..rows {
                    let mut bytes = vec![u8::try_from(table_code).unwrap_or_default(); width];
                    if field.name() == "workspace_id" {
                        bytes.copy_from_slice(&WORKSPACE);
                    } else if field.name() == "analysis_context_id" {
                        bytes.copy_from_slice(&CONTEXT);
                    } else if let Some(last) = bytes.last_mut() {
                        *last = u8::try_from(row + 1).unwrap();
                    }
                    builder.append_value(bytes);
                }
                Arc::new(builder.finish())
            }
            DataType::FixedSizeBinary(16) => {
                let values = (0..rows)
                    .map(|row| {
                        let mut bytes = [u8::try_from(table_code).unwrap_or_default(); 16];
                        if field.name() == "workspace_id" {
                            bytes = WORKSPACE;
                        } else if field.name() == "analysis_context_id" {
                            bytes = CONTEXT;
                        } else {
                            bytes[15] = u8::try_from(row + 1).unwrap();
                        }
                        bytes
                    })
                    .collect::<Vec<_>>();
                crate::fabric::id16_array(values.iter().map(Some))
            }
            DataType::Int16 => Arc::new(Int16Array::from(vec![1_i16; rows])),
            DataType::Int32 => Arc::new(Int32Array::from(vec![1_i32; rows])),
            DataType::Int64 if field.name() == "source_generation" => {
                Arc::new(Int64Array::from(vec![source_generation; rows]))
            }
            DataType::Int64 => Arc::new(Int64Array::from(vec![1_i64; rows])),
            DataType::List(element) if element.data_type() == &DataType::Int64 => {
                Arc::new(ListArray::new(
                    Arc::clone(element),
                    OffsetBuffer::from_lengths(std::iter::repeat_n(1, rows)),
                    Arc::new(Int64Array::from(vec![0_i64; rows])),
                    None,
                ))
            }
            DataType::List(element) if element.data_type() == &DataType::FixedSizeBinary(16) => {
                let mut builder =
                    arrow_array::builder::ListBuilder::new(FixedSizeBinaryBuilder::new(16))
                        .with_field(Arc::clone(element));
                for _ in 0..rows {
                    builder.values().append_value(WORKSPACE).unwrap();
                    builder.append(true);
                }
                Arc::new(builder.finish())
            }
            DataType::Float64 => Arc::new(Float64Array::from(vec![1.0_f64; rows])),
            DataType::Boolean => Arc::new(BooleanArray::from(vec![true; rows])),
            DataType::Utf8 => {
                let value = match (table_code, field.name().as_str()) {
                    (11, "domain") => "language",
                    (11, "name") => "Rust",
                    (11, "version") => "1",
                    _ => "fixture",
                };
                Arc::new(StringArray::from(vec![value; rows]))
            }
            DataType::Timestamp(TimeUnit::Microsecond, _) => {
                Arc::new(TimestampMicrosecondArray::from(vec![0_i64; rows]).with_timezone_utc())
            }
            data_type => panic!("unsupported generated fixture type {data_type:?}"),
        }
    }

    fn candidate(
        publication: [u8; 16],
        source_generation: u64,
        entity_rows: usize,
    ) -> Arc<ServingSnapshotCandidate> {
        candidate_with_source_trust(publication, source_generation, entity_rows, "CURRENT")
    }

    fn candidate_with_source_trust(
        publication: [u8; 16],
        source_generation: u64,
        entity_rows: usize,
        source_trust_state: &str,
    ) -> Arc<ServingSnapshotCandidate> {
        let row_generation = i64::try_from(source_generation).unwrap();
        let batches = std::iter::once(11)
            .chain(
                serving_projection_specs()
                    .iter()
                    .map(|projection| projection.source_table_code),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|table_code| {
                let rows = if table_code == 100 { entity_rows } else { 1 };
                (
                    table_code,
                    generated_batch_at_generation(table_code, rows, row_generation),
                )
            })
            .collect();
        let catalog = Arc::new(SnapshotProviderCatalog::from_batches_for_snapshot_tests(
            publication,
            WORKSPACE,
            batches,
            OVERLAY,
            row_generation,
            vec![CONTEXT],
        ));
        let mut body = snapshot_body(source_generation);
        body.source.source_trust_state = source_trust_state.into();
        Arc::new(ServingSnapshotCandidate::build(body, catalog, &[]).unwrap())
    }

    fn candidate_from_effective_batches(
        publication: [u8; 16],
        workspace_id: [u8; 16],
        source_generation: u64,
        inventory_digest: [u8; 32],
        batches: Vec<(i16, RecordBatch)>,
    ) -> Arc<ServingSnapshotCandidate> {
        let catalog = Arc::new(SnapshotProviderCatalog::from_batches_for_snapshot_tests(
            publication,
            workspace_id,
            batches,
            [0; 32],
            i64::try_from(source_generation).unwrap(),
            vec![CONTEXT],
        ));
        Arc::new(
            ServingSnapshotCandidate::build(
                snapshot_body_for(
                    workspace_id,
                    source_generation,
                    crate::integrity::frame_digest(inventory_digest),
                ),
                catalog,
                &[],
            )
            .unwrap(),
        )
    }

    fn operational_store() -> (TempDir, OperationalStore, SourceImageStore) {
        let directory = tempdir().unwrap();
        let mut store = OperationalStore::open(&directory.path().join("state.sqlite3")).unwrap();
        store
            .write_transaction(|transaction| {
                transaction.execute(
                    "INSERT INTO workspace_registration(workspace_id,
                     workspace_registration_nonce, registration_revision,
                     administrative_key, root_path_bytes, root_path_display,
                     root_directory_file_identity, platform_code,
                     case_sensitivity_mode, authorization_revision,
                     allowed_source_disclosure_rules, repository_id, worktree_id,
                     authorization_fingerprint, context_fingerprint, status_code,
                     created_at, updated_at)
                     VALUES (?1, ?2, 1, ?3, X'2f', '/', ?4, 10, 'sensitive', 1,
                             X'', NULL, NULL, ?5, ?6, ?7, '0', '0')",
                    params![
                        WORKSPACE.as_slice(),
                        [1_u8; 16].as_slice(),
                        b"test".as_slice(),
                        [2_u8; 16].as_slice(),
                        [3_u8; 32].as_slice(),
                        [4_u8; 32].as_slice(),
                        i64::from(WorkspaceRegistryLifecycle::Bootstrapping as u16),
                    ],
                )?;
                transaction.execute(
                    "INSERT INTO worktree_state(workspace_id, worktree_id, repository_id,
                     work_dir_path_bytes, work_dir_path_display, git_dir_path_bytes,
                     git_dir_path_display, lifecycle_state_code, source_trust_state_code,
                     event_stream_health_code, git_acceleration_status_code,
                     active_snapshot_id, analysis_context_set_id, source_generation,
                     event_watermark, newest_dirty_generation, durable_generation,
                     reconcile_required, updated_at, last_diagnostic_id, inventory_digest)
                     VALUES (?1, NULL, NULL, X'2f', '/', NULL, NULL, 30, 30, 10, 10,
                             NULL, ?2, 1, 1, 0, 1, 0, '0', NULL, ?3)",
                    params![
                        WORKSPACE.as_slice(),
                        CONTEXT.as_slice(),
                        [0x55_u8; 32].as_slice(),
                    ],
                )?;
                Ok::<_, OperationalStoreError>(())
            })
            .unwrap();
        let images = SourceImageStore::open(
            &directory.path().join("source-images"),
            SourceCapturePolicy::default(),
        )
        .unwrap();
        (directory, store, images)
    }

    fn comparison_operational_store(
        workspace_id: [u8; 16],
    ) -> (TempDir, OperationalStore, SourceImageStore) {
        let directory = tempdir().unwrap();
        let mut store = OperationalStore::open(&directory.path().join("state.sqlite3")).unwrap();
        let context_set = crate::identity::context_set_identity(workspace_id, &[CONTEXT])
            .unwrap()
            .id;
        store
            .write_transaction(|transaction| {
                transaction.execute(
                    "INSERT INTO workspace_registration(workspace_id,
                     workspace_registration_nonce, registration_revision,
                     administrative_key, root_path_bytes, root_path_display,
                     root_directory_file_identity, platform_code,
                     case_sensitivity_mode, authorization_revision,
                     allowed_source_disclosure_rules, repository_id, worktree_id,
                     authorization_fingerprint, context_fingerprint, status_code,
                     created_at, updated_at)
                     VALUES (?1, ?2, 1, ?3, X'2f', '/', ?4, 10, 'sensitive', 1,
                             X'', NULL, NULL, ?5, ?6, ?7, '0', '0')",
                    params![
                        workspace_id.as_slice(),
                        [1_u8; 16].as_slice(),
                        b"comparison".as_slice(),
                        [2_u8; 16].as_slice(),
                        [3_u8; 32].as_slice(),
                        [4_u8; 32].as_slice(),
                        i64::from(WorkspaceRegistryLifecycle::Bootstrapping as u16),
                    ],
                )?;
                transaction.execute(
                    "INSERT INTO worktree_state(workspace_id, worktree_id, repository_id,
                     work_dir_path_bytes, work_dir_path_display, git_dir_path_bytes,
                     git_dir_path_display, lifecycle_state_code, source_trust_state_code,
                     event_stream_health_code, git_acceleration_status_code,
                     active_snapshot_id, analysis_context_set_id, source_generation,
                     event_watermark, newest_dirty_generation, durable_generation,
                     reconcile_required, updated_at, last_diagnostic_id, inventory_digest)
                     VALUES (?1, NULL, NULL, X'2f', '/', NULL, NULL, 30, 30, 10, 10,
                             NULL, ?2, 1, 1, 0, 1, 0, '0', NULL, ?3)",
                    params![
                        workspace_id.as_slice(),
                        context_set.as_slice(),
                        [0x55_u8; 32].as_slice(),
                    ],
                )?;
                Ok::<_, OperationalStoreError>(())
            })
            .unwrap();
        let images = SourceImageStore::open(
            &directory.path().join("source-images"),
            SourceCapturePolicy::default(),
        )
        .unwrap();
        (directory, store, images)
    }

    struct ComparisonSessionFixture {
        _directory: TempDir,
        _store: OperationalStore,
        _images: SourceImageStore,
        session: ServingQuerySession,
    }

    fn comparison_session(candidate: Arc<ServingSnapshotCandidate>) -> ComparisonSessionFixture {
        let workspace_id = candidate.manifest().raw_workspace_id().unwrap();
        let (directory, mut store, mut images) = comparison_operational_store(workspace_id);
        let runtime = ServingSnapshotRuntime::default();
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            candidate,
            directory.path(),
        );
        ComparisonSessionFixture {
            _directory: directory,
            _store: store,
            _images: images,
            session,
        }
    }

    async fn materialize_effective_batches(
        overlay: &crate::fabric::ConsolidatedOverlay,
    ) -> Vec<(i16, RecordBatch)> {
        let mut materialized = vec![(11, generated_batch(11, 1))];
        for table_code in std::iter::once(10).chain(
            serving_projection_specs()
                .iter()
                .map(|projection| projection.source_table_code),
        ) {
            let spec = table_spec(table_code).unwrap();
            let empty = RecordBatch::new_empty(Arc::clone(&spec.arrow_schema));
            let base: Arc<dyn TableProvider> = Arc::new(
                MemTable::try_new(Arc::clone(&spec.arrow_schema), vec![vec![empty]]).unwrap(),
            );
            let effective = overlay.wrap(spec, base).unwrap();
            let batches = SessionContext::new()
                .read_table(effective)
                .unwrap()
                .collect()
                .await
                .unwrap();
            let batch = if batches.is_empty() {
                RecordBatch::new_empty(Arc::clone(&spec.arrow_schema))
            } else {
                arrow_select::concat::concat_batches(&spec.arrow_schema, &batches).unwrap()
            };
            materialized.push((table_code, batch));
        }
        materialized
    }

    fn wp72_lifecycle_config() -> LifecycleConfig {
        LifecycleConfig {
            debounce_timeout: Duration::from_millis(20),
            tick_rate: Duration::from_millis(5),
            ingress_capacity: 32,
            maximum_paths_per_batch: 128,
            gather_window: Duration::from_millis(5),
            dirty_path_bulk_threshold: 8,
            await_current_timeout: Duration::from_secs(1),
            maximum_capture_bytes: 1024 * 1024,
            stable_read_retry_count: 2,
            source_blob_lease_ttl: Duration::from_secs(60),
            overlay_flush_policy: OverlayFlushPolicy {
                maximum_rows: 100_000,
                maximum_bytes: 64 * 1024 * 1024,
                maximum_touched_owners: 1_000,
                maximum_generations: 32,
            },
        }
    }

    fn wp72_engine(
        root: &std::path::Path,
        state_root: &std::path::Path,
        workspace_nonce: [u8; 16],
    ) -> (
        OperationalStore,
        ContinuousWorkspaceEngine<GixGitStateAdapter>,
        [u8; 16],
    ) {
        let mut store = OperationalStore::open(&state_root.join("operational.sqlite")).unwrap();
        let workspace_id = WorkspaceRegistry::new(&mut store)
            .add_directory_fixture(root, workspace_nonce)
            .unwrap()
            .workspace_id;
        let lifecycle = wp72_lifecycle_config();
        let scheduler = UpdateWaveScheduler::new(workspace_id, root, 0, 0, 0, lifecycle).unwrap();
        let source_images = SourceImageStore::open(
            &state_root.join("source-blobs"),
            SourceCapturePolicy {
                maximum_bytes: lifecycle.maximum_capture_bytes,
                stable_read_retries: lifecycle.stable_read_retry_count,
                lease_ttl: lifecycle.source_blob_lease_ttl,
            },
        )
        .unwrap();
        let engine = ContinuousWorkspaceEngine::new(
            scheduler,
            source_images,
            GitCandidatePlanner::without_cache(GixGitStateAdapter),
            ContinuousWorkspaceConfig {
                analysis_context_id: CONTEXT,
                registered_git_identity: None,
                git_observations: GitStateObservations {
                    inclusion_policy_fingerprint: [0x31; 32],
                    attributes_fingerprint: [0x32; 32],
                    worktree_inventory_digest: [0; 32],
                },
                prior_git_vector: None,
                overlay_memory_limit_bytes: 64 * 1024 * 1024,
                semantic_capabilities_required: false,
            },
        );
        (store, engine, workspace_id)
    }

    async fn assert_wp72_clean_rebuild(
        root: &std::path::Path,
        workspace_nonce: [u8; 16],
        incremental: &ContinuousWorkspaceEngine<GixGitStateAdapter>,
        stage: u8,
    ) {
        let clean_state = tempdir().unwrap();
        let (mut clean_store, mut clean, clean_workspace_id) =
            wp72_engine(root, clean_state.path(), workspace_nonce);
        let rebuilt = clean
            .rebuild_from_zero(&mut clean_store)
            .unwrap()
            .expect("zero-state rebuild publishes");
        let incremental_overlay = incremental.current_overlay().unwrap();
        assert_eq!(incremental.scheduler().workspace_id(), clean_workspace_id);
        assert_eq!(
            incremental.current_inventory_digest(),
            clean.current_inventory_digest()
        );
        let incremental_candidate = candidate_from_effective_batches(
            [stage; 16],
            clean_workspace_id,
            incremental.scheduler().current_source_generation(),
            incremental.current_inventory_digest(),
            materialize_effective_batches(&incremental_overlay).await,
        );
        let rebuilt_candidate = candidate_from_effective_batches(
            [stage | 0x80; 16],
            clean_workspace_id,
            rebuilt.wave.source_generation,
            clean.current_inventory_digest(),
            materialize_effective_batches(&rebuilt.overlay).await,
        );
        let incremental_session = comparison_session(incremental_candidate);
        let rebuilt_session = comparison_session(rebuilt_candidate);
        prove_serving_rebuild_equivalence(&incremental_session.session, &rebuilt_session.session)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn wp72_rejects_either_noncurrent_comparison_input() {
        let current = comparison_session(candidate_with_source_trust([0x71; 16], 1, 1, "CURRENT"));
        let noncurrent =
            comparison_session(candidate_with_source_trust([0x72; 16], 1, 1, "UNKNOWN"));

        for (left, right) in [
            (&current.session, &noncurrent.session),
            (&noncurrent.session, &current.session),
        ] {
            assert!(matches!(
                prove_serving_rebuild_equivalence(left, right).await,
                Err(crate::lifecycle::LifecycleError::ComparisonDomainMismatch(
                    _
                ))
            ));
        }
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // One ordered corpus proves true rebuild convergence after every terminal state.
    async fn wp72_behavioral_acceptance() {
        let fixture = tempdir().unwrap();
        let root = fixture.path().join("workspace");
        let state_root = fixture.path().join("incremental-state");
        fs::create_dir_all(root.join("generated")).unwrap();
        fs::create_dir_all(&state_root).unwrap();
        fs::write(root.join("a.py"), b"value = 1\n").unwrap();
        fs::write(root.join("b.rs"), b"pub fn value() -> i32 { 1 }\n").unwrap();
        fs::write(root.join("generated/bindings.py"), b"BOUND = 1\n").unwrap();
        let workspace_nonce = [0x72; 16];
        let (mut store, mut engine, workspace_id) =
            wp72_engine(&root, &state_root, workspace_nonce);

        engine
            .process_batch(
                &mut store,
                WatchHintBatch {
                    hints: Vec::new(),
                    rescan_required: true,
                },
                &BTreeMap::new(),
            )
            .unwrap()
            .expect("initial publication");
        assert_wp72_clean_rebuild(&root, workspace_nonce, &engine, 1).await;

        fs::write(root.join("a.py"), b"def broken(:\n").unwrap();
        engine
            .process_batch(
                &mut store,
                WatchHintBatch {
                    hints: vec![WatchHint {
                        path_bytes: b"a.py".to_vec(),
                        kind: WatchHintKind::CreateOrModify,
                    }],
                    rescan_required: false,
                },
                &BTreeMap::new(),
            )
            .unwrap()
            .expect("parse-break publication");
        assert_wp72_clean_rebuild(&root, workspace_nonce, &engine, 2).await;

        fs::write(root.join("a.py"), b"value = 2\n").unwrap();
        fs::rename(root.join("b.rs"), root.join("renamed.rs")).unwrap();
        fs::write(root.join("generated/bindings.py"), b"BOUND = 2\n").unwrap();
        engine
            .process_batch(
                &mut store,
                WatchHintBatch {
                    hints: vec![
                        WatchHint {
                            path_bytes: b"a.py".to_vec(),
                            kind: WatchHintKind::CreateOrModify,
                        },
                        WatchHint {
                            path_bytes: b"b.rs".to_vec(),
                            kind: WatchHintKind::RenameSource,
                        },
                        WatchHint {
                            path_bytes: b"renamed.rs".to_vec(),
                            kind: WatchHintKind::RenameTarget,
                        },
                        WatchHint {
                            path_bytes: b"generated/bindings.py".to_vec(),
                            kind: WatchHintKind::CreateOrModify,
                        },
                    ],
                    rescan_required: true,
                },
                &BTreeMap::new(),
            )
            .unwrap()
            .expect("repair/rename/burst publication");
        assert_wp72_clean_rebuild(&root, workspace_nonce, &engine, 3).await;

        let recovery = crate::lifecycle::recover_workspace(
            &store.reader_factory().open().unwrap(),
            workspace_id,
        )
        .unwrap();
        assert!(!recovery.restart_paths.is_empty());
        let recovered_inventory_digest = engine.current_inventory_digest();
        drop(engine);
        let lifecycle = wp72_lifecycle_config();
        let mut scheduler = UpdateWaveScheduler::new(
            workspace_id,
            &root,
            recovery.source_generation,
            recovery.event_watermark,
            recovery.event_watermark,
            lifecycle,
        )
        .unwrap();
        scheduler.restore_recovery(&recovery).unwrap();
        let source_images = SourceImageStore::open(
            &state_root.join("source-blobs"),
            SourceCapturePolicy {
                maximum_bytes: lifecycle.maximum_capture_bytes,
                stable_read_retries: lifecycle.stable_read_retry_count,
                lease_ttl: lifecycle.source_blob_lease_ttl,
            },
        )
        .unwrap();
        let mut engine = ContinuousWorkspaceEngine::new(
            scheduler,
            source_images,
            GitCandidatePlanner::without_cache(GixGitStateAdapter),
            ContinuousWorkspaceConfig {
                analysis_context_id: CONTEXT,
                registered_git_identity: None,
                git_observations: GitStateObservations {
                    inclusion_policy_fingerprint: [0x31; 32],
                    attributes_fingerprint: [0x32; 32],
                    worktree_inventory_digest: recovered_inventory_digest,
                },
                prior_git_vector: None,
                overlay_memory_limit_bytes: 64 * 1024 * 1024,
                semantic_capabilities_required: false,
            },
        );
        engine
            .process_batch(
                &mut store,
                WatchHintBatch {
                    hints: Vec::new(),
                    rescan_required: false,
                },
                &BTreeMap::new(),
            )
            .unwrap()
            .expect("restart replay publication");
        assert_wp72_clean_rebuild(&root, workspace_nonce, &engine, 4).await;

        fs::remove_file(root.join("a.py")).unwrap();
        engine
            .process_batch(
                &mut store,
                WatchHintBatch {
                    hints: vec![WatchHint {
                        path_bytes: b"a.py".to_vec(),
                        kind: WatchHintKind::Remove,
                    }],
                    rescan_required: false,
                },
                &BTreeMap::new(),
            )
            .unwrap()
            .expect("delete publication");
        assert_wp72_clean_rebuild(&root, workspace_nonce, &engine, 5).await;
    }

    fn workspace_record() -> WorkspaceRecord {
        WorkspaceRecord {
            workspace_id: WORKSPACE,
            workspace_registration_nonce: [0x12; 16],
            registration_revision: 1,
            administrative_key: vec![3],
            root_path_bytes: b"/workspace".to_vec(),
            root_path_display: "/workspace".into(),
            root_directory_file_identity: vec![4],
            platform_code: 2,
            case_sensitivity_mode: "sensitive".into(),
            authorization_revision: 1,
            allowed_source_disclosure_rules: Vec::new(),
            repository_id: None,
            worktree_id: None,
            authorization_fingerprint: [5; 32],
            context_fingerprint: [6; 32],
            status: WorkspaceRegistryLifecycle::Bootstrapping,
            created_at: "00000000000000001000".into(),
            updated_at: "00000000000000001000".into(),
        }
    }

    fn owner_batch(scope: FactScope) -> ValidatedFactBatch {
        let spec = table_spec(8).unwrap();
        let columns: Vec<ArrayRef> = vec![
            crate::fabric::id16_array([Some(&scope.workspace_id)]),
            crate::fabric::id16_array([Some(&scope.analysis_context_id)]),
            Arc::new(Int64Array::from(vec![scope.source_generation])),
            crate::fabric::id16_array([Some(&scope.owner_id)]),
            crate::fabric::id16_array([None]),
            Arc::new(Int16Array::from(vec![i16::from(scope.owner_id[0])])),
            Arc::new(Int16Array::from(vec![10_i16])),
            Arc::new(Int16Array::from(vec![10_i16])),
            crate::fabric::id16_array([None]),
            crate::fabric::id16_array([None]),
            Arc::new(Int64Array::from(vec![0_i64])),
            Arc::new(Int64Array::from(vec![0_i64])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>])),
            Arc::new(Int64Array::from(vec![0_i64])),
        ];
        ValidatedFactBatch::validate(
            8,
            RecordBatch::try_new(Arc::clone(&spec.arrow_schema), columns).unwrap(),
            scope,
        )
        .unwrap()
    }

    fn entity_batch(scope: FactScope, entity_id: [u8; 16]) -> ValidatedFactBatch {
        let row = EntityRow {
            scope,
            entity_id,
            language: 10,
            entity_family_code: 1,
            entity_kind_code: 10,
            raw_kind_code: None,
            file_id: None,
            start_byte: Some(0),
            end_byte: Some(0),
            name: Some("scope-probe".into()),
            qualified_name: None,
            parent_entity_id: None,
            type_id: None,
            flags: 0,
            fact_hash64: i64::from(entity_id[0]),
        };
        ValidatedFactBatch::validate(100, encode_entities(&[row]).unwrap(), scope).unwrap()
    }

    async fn seed_scope(
        fabric: &mut super::super::WorkspaceFabric,
        journal: &mut OperationalStore,
        scope: FactScope,
        marker: u8,
    ) {
        for (table_code, batch) in [
            (8_i16, owner_batch(scope)),
            (100_i16, entity_batch(scope, [marker; 16])),
        ] {
            let request = super::super::OwnerMutationRequest {
                scope: scope.batch_scope(),
                publication_id: [0x70; 16],
                operation_id: [marker ^ table_code.to_be_bytes()[1]; 16],
                table_code,
                owner_ids: vec![scope.owner_id],
                expected_predecessor: fabric.table(table_code).unwrap().version(),
            };
            fabric
                .replace_owner_rows(journal, &request, &batch)
                .await
                .unwrap();
        }
    }

    async fn published_delta_candidate(
        root: &std::path::Path,
        journal: &mut OperationalStore,
    ) -> (super::super::WorkspaceFabric, Arc<ServingSnapshotCandidate>) {
        let mut fabric =
            super::super::bootstrap_workspace(&root.join("fabric"), &workspace_record())
                .await
                .unwrap();
        seed_scope(
            &mut fabric,
            journal,
            FactScope {
                workspace_id: WORKSPACE,
                analysis_context_id: CONTEXT,
                source_generation: 1,
                owner_id: [0x61; 16],
            },
            0x71,
        )
        .await;
        seed_scope(
            &mut fabric,
            journal,
            FactScope {
                workspace_id: WORKSPACE,
                analysis_context_id: [0x45; 16],
                source_generation: 2,
                owner_id: [0x62; 16],
            },
            0x72,
        )
        .await;
        let publication_id = [0x77; 16];
        let context_ids = vec![CONTEXT];
        let request = super::super::PublicationRequest {
            operation_id: [0x78; 16],
            pins: super::super::PublicationPins {
                publication_id,
                workspace_id: WORKSPACE,
                repository_id: None,
                worktree_id: None,
                source_generation: 1,
                source_inventory_digest: [1; 32],
                analysis_context_set_id: crate::identity::context_set_identity(
                    WORKSPACE,
                    &context_ids,
                )
                .unwrap()
                .id,
                analysis_context_ids: context_ids,
                git_state_fingerprint: None,
                inclusion_policy_fingerprint: [2; 32],
                base_fact_digest: [3; 32],
                derived_fact_digest: None,
                ontology_version: "1.3".into(),
                schema_bundle_version: "1.0.0".into(),
                provider_bundle_version: "1.0.0".into(),
                derivation_bundle_version: "1.0.0".into(),
                toolchain_bundle_version: "1.0.0".into(),
            },
            expected_pointer: None,
            expected_publication_table_version: fabric.table(5).unwrap().version(),
            expected_manifest_table_version: fabric.table(6).unwrap().version(),
            expected_pointer_table_version: fabric.table(7).unwrap().version(),
            started_at_micros: 1_000,
            completed_at_micros: 1_500,
        };
        let publication = fabric.publish(journal, &request, &[]).await.unwrap();
        let providers = Arc::new(
            SnapshotProviderCatalog::build(&publication, &super::super::EmptySnapshotOverlay)
                .await
                .unwrap(),
        );
        assert_eq!(providers.metrics().validation_scan_count, 0);
        let candidate =
            Arc::new(ServingSnapshotCandidate::build(snapshot_body(1), providers, &[]).unwrap());
        (fabric, candidate)
    }

    async fn explainable_version_fixture() -> (VersionExplanation, i64, serde_json::Value) {
        let (directory, mut store, mut images) = operational_store();
        let (fabric, candidate) = published_delta_candidate(directory.path(), &mut store).await;
        let runtime = ServingSnapshotRuntime::default();
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            Arc::clone(&candidate),
            directory.path(),
        );
        let query = session
            .query("SELECT entity_id FROM entities ORDER BY entity_id")
            .await
            .unwrap();
        let execution = QueryExecutionContext {
            execution_id: query.artifact.execution_id.clone(),
            semantic_request_id: query.artifact.semantic_request_id.clone(),
            mcp_call_id: query.artifact.mcp_call_id.clone(),
        };
        let artifacts =
            ResultArtifactStore::new(directory.path().join("result-artifacts")).unwrap();
        artifacts
            .persist_query_artifact(&PersistedQueryArtifactBundle {
                artifact_schema_version: "codefabric.query-execution-artifact.v1".into(),
                execution,
                phase: QueryArtifactPhase::Succeeded,
                plan_artifacts: vec![query.artifact],
                result_artifact_id: Some("result:wp66".into()),
                public_error_code: None,
                created_at_unix_ms: 1,
                expires_at_unix_ms: 60_001,
            })
            .unwrap();
        let table = fabric.table(100).unwrap();
        let delta_version = table.version().unwrap();
        let typed_scope_rows = store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*) FROM table_mutation_operation
                     WHERE table_code=100 AND delta_version=?1
                       AND workspace_id=?2 AND source_generation=2
                       AND analysis_context_id=?3",
                    params![
                        i64::try_from(delta_version).unwrap(),
                        WORKSPACE.as_slice(),
                        [0x45_u8; 16].as_slice()
                    ],
                    |row| row.get::<_, i64>(0),
                )
            })
            .unwrap();
        let manifest_json = serde_json::to_value(candidate.manifest()).unwrap();
        let explanation = artifacts
            .explain_version(table, delta_version)
            .await
            .unwrap();
        (explanation, typed_scope_rows, manifest_json)
    }

    fn normalize_plan(plan: &str, root: &std::path::Path) -> String {
        plan.replace(&root.to_string_lossy().to_string(), "<ROOT>")
            .replace('\\', "/")
            .lines()
            .map(|line| {
                let Some((prefix, remainder)) = line.split_once("file_groups=") else {
                    return line.to_owned();
                };
                let Some((_, suffix)) = remainder.split_once(", projection=") else {
                    return line.to_owned();
                };
                format!("{prefix}file_groups=<NORMALIZED>, projection={suffix}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn activate_and_lease(
        store: &mut OperationalStore,
        images: &mut SourceImageStore,
        runtime: &ServingSnapshotRuntime,
        candidate: Arc<ServingSnapshotCandidate>,
        config_root: &std::path::Path,
    ) -> ServingQuerySession {
        let config = ServingRuntimeConfig::new(
            16 * 1024 * 1024,
            64 * 1024 * 1024,
            config_root.join("query-spill"),
            2,
        )
        .unwrap();
        activate_and_lease_with_config(store, images, runtime, candidate, config).unwrap()
    }

    fn activate_and_lease_with_config(
        store: &mut OperationalStore,
        images: &mut SourceImageStore,
        runtime: &ServingSnapshotRuntime,
        candidate: Arc<ServingSnapshotCandidate>,
        config: ServingRuntimeConfig,
    ) -> Result<ServingQuerySession, ServingQueryError> {
        runtime
            .activate(store, Arc::clone(&candidate), None, 0, 7, 100, None)
            .unwrap();
        let lease = SnapshotLeaseManager::new([0x66; 16])
            .acquire(
                store,
                images,
                candidate,
                SnapshotLeaseKind::Query,
                None,
                101,
                Duration::from_mins(1),
                None,
            )
            .unwrap();
        ServingQuerySession::from_lease(lease, &store.reader_factory(), config)
    }

    fn count(result: &ServingQueryResult) -> u64 {
        let column = result.batches[0].column(0);
        if let Some(values) = column.as_any().downcast_ref::<UInt64Array>() {
            values.value(0)
        } else {
            u64::try_from(
                column
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .expect("COUNT returns an integral Arrow array")
                    .value(0),
            )
            .unwrap()
        }
    }

    fn assert_table_reference_policy() {
        for allowed in [
            "codefabric.cpg_serving.entities",
            "cpg_control.worktree_state",
            "entity_source",
            "enum_language",
            "worktree_state_capture",
        ] {
            assert!(
                allowed_table_reference(allowed),
                "expected allowed: {allowed}"
            );
        }
        for rejected in [
            "unknown",
            "other.entity_source",
            "cpg_base.entity/suffix",
            r"cpg_base.entity\suffix",
            "cpg_base.entity:suffix",
            "cpg_base..entity",
        ] {
            assert!(
                !allowed_table_reference(rejected),
                "expected rejected: {rejected}"
            );
        }
    }

    fn assert_operational_value_types() {
        for (data_type, value) in [
            (DataType::Int64, Value::Integer(7)),
            (DataType::Float64, Value::Real(1.5)),
            (DataType::Utf8, Value::Text("value".into())),
            (DataType::Binary, Value::Blob(vec![1, 2, 3])),
        ] {
            let required = Field::new("required", data_type.clone(), false);
            assert!(values_to_array(&required, &[&value]).is_ok());
            assert!(values_to_array(&required, &[&Value::Null]).is_err());
            let optional = Field::new("optional", data_type, true);
            assert_eq!(
                values_to_array(&optional, &[&Value::Null])
                    .unwrap()
                    .null_count(),
                1
            );
        }
        assert!(
            values_to_array(
                &Field::new("strict_integer", DataType::Int64, false),
                &[&Value::Text("not-an-integer".into())],
            )
            .is_err()
        );
        assert_operational_builder_contract(
            || OperationalBuilder::Int64(Int64Builder::new()),
            DataType::Int64,
            || ValueRef::Integer(7),
        );
        assert_operational_builder_contract(
            || OperationalBuilder::Float64(Float64Builder::new()),
            DataType::Float64,
            || ValueRef::Real(1.5),
        );
        assert_operational_builder_contract(
            || OperationalBuilder::Utf8(StringBuilder::new()),
            DataType::Utf8,
            || ValueRef::Text(b"value"),
        );
        assert_operational_builder_contract(
            || OperationalBuilder::Binary(BinaryBuilder::new()),
            DataType::Binary,
            || ValueRef::Blob(&[1, 2, 3]),
        );
        assert!(!capture_batch_is_full(1, 2));
        assert!(capture_batch_is_full(2, 2));
    }

    fn assert_operational_builder_contract(
        make_builder: impl Fn() -> OperationalBuilder,
        data_type: DataType,
        value: impl Fn() -> ValueRef<'static>,
    ) {
        let required = Field::new("required", data_type.clone(), false);
        let mut valid = make_builder();
        assert!(valid.append(&required, value()).is_ok());
        assert_eq!(valid.finish().len(), 1);
        let mut invalid_null = make_builder();
        assert!(invalid_null.append(&required, ValueRef::Null).is_err());

        let optional = Field::new("optional", data_type, true);
        let mut valid_null = make_builder();
        assert!(valid_null.append(&optional, ValueRef::Null).is_ok());
        assert_eq!(valid_null.finish().null_count(), 1);
    }

    #[tokio::test]
    async fn wp38_response_kat_is_canonical_and_snapshot_pinned() {
        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let candidate = candidate([0x38; 16], 1, 1);
        let session = activate_and_lease_with_config(
            &mut store,
            &mut images,
            &runtime,
            Arc::clone(&candidate),
            ServingRuntimeConfig::new(
                32 * 1024 * 1024,
                64 * 1024 * 1024,
                directory.path().join("query-spill"),
                2,
            )
            .unwrap(),
        )
        .unwrap();
        let workspace_id = candidate.manifest().body.workspace_id.clone();
        let request = format!(
            r#"{{"specification":"composable semantic CPG fact query","version":"1.3","semantic_request_id":"response-kat","workspace_id":"{workspace_id}","freshness_policy":"current_required","queries":[{{"query_id":"entities","request":"find code entities","label":"syntax nodes","input":null,"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"properties","request":"retrieve facts about code","label":null,"input":null,"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"relations","request":"follow code relationships","label":null,"input":null,"where":null,"limit":{{"first":10,"offset":0}}}}],"response_projection":null,"cost_budget":{{"maximum_rows":30}}}}"#
        );
        let first = crate::semantic_query::execute_request(
            &session,
            crate::semantic_query::validate_request(request.as_bytes()).unwrap(),
            crate::registries::FreshnessState::Current,
        )
        .await
        .unwrap();
        let second = crate::semantic_query::execute_request(
            &session,
            crate::semantic_query::validate_request(request.as_bytes()).unwrap(),
            crate::registries::FreshnessState::Current,
        )
        .await
        .unwrap();
        assert_eq!(first.canonical_bytes, second.canonical_bytes);
        assert_eq!(first.response_digest, second.response_digest);
        assert_eq!(
            first.response.snapshot.snapshot_id,
            session.snapshot_manifest().snapshot_id
        );
        assert_eq!(first.response.successful_query_count, 3);
        assert_eq!(
            first.response.query_results[0].resolved_semantics["phrase_id"],
            "Q51_SYNTAX_NODES"
        );
    }

    #[tokio::test]
    async fn wp62_native_relational_execution() {
        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let candidate = candidate([0x62; 16], 1, 2);
        let session = activate_and_lease_with_config(
            &mut store,
            &mut images,
            &runtime,
            Arc::clone(&candidate),
            ServingRuntimeConfig::new(
                32 * 1024 * 1024,
                64 * 1024 * 1024,
                directory.path().join("wp62-spill"),
                2,
            )
            .unwrap(),
        )
        .unwrap();
        let workspace_id = candidate.manifest().body.workspace_id.clone();
        let request = format!(
            r#"{{"specification":"composable semantic CPG fact query","version":"1.3","semantic_request_id":"wp62-native","workspace_id":"{workspace_id}","freshness_policy":"current_required","queries":[{{"query_id":"entities","request":"find code entities","label":null,"input":null,"where":{{"entity_kind_codes":[1],"relation_kind_codes":[]}},"limit":{{"first":10,"offset":0}}}}],"response_projection":{{"canonical_semantic_identity":true}},"cost_budget":{{"maximum_rows":10}}}}"#
        );
        let typed = crate::semantic_query::validate_request(request.as_bytes()).unwrap();
        let bound = crate::semantic_query::bind_request(&session, &typed)
            .await
            .unwrap();
        assert_eq!(bound.snapshot_id, candidate.manifest().snapshot_id);
        let crate::semantic_query::BoundOperator::Relational { plan, .. } =
            &bound.blocks[0].operator
        else {
            panic!("find-entities must lower to a relational plan");
        };
        let plan_text = plan.display_indent().to_string();
        for native_node in ["Limit", "Sort", "Projection", "Filter", "TableScan"] {
            assert!(
                plan_text.contains(native_node),
                "missing {native_node}: {plan_text}"
            );
        }
        let native = session
            .query_plan("wp62-native", plan.clone())
            .await
            .unwrap();
        let transition = session
            .query(
                "SELECT entity_id FROM entities WHERE entity_kind_code = 1 \
                 ORDER BY entity_id LIMIT 11",
            )
            .await
            .unwrap();
        let checksums = |batches: &[RecordBatch]| {
            batches
                .iter()
                .map(|batch| crate::fabric::batch_checksum(batch).unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(checksums(&native.batches), checksums(&transition.batches));
        let response = crate::semantic_query::execute_request(
            &session,
            typed,
            crate::registries::FreshnessState::PotentiallyStale,
        )
        .await
        .unwrap();
        assert_eq!(
            response.response.freshness_state,
            crate::registries::FreshnessState::PotentiallyStale
        );
        assert_eq!(response.response.successful_query_count, 1);
        assert!(
            response
                .response
                .entities
                .keys()
                .all(|id| id.starts_with("entity:unknown:"))
        );
    }

    #[tokio::test]
    #[allow(
        clippy::too_many_lines,
        reason = "the replay acceptance test intentionally exercises the full production lifecycle"
    )]
    async fn wp64_production_replay_is_partition_and_batch_independent() {
        let (first_directory, mut first_store, mut first_images) = operational_store();
        let first_runtime = ServingSnapshotRuntime::default();
        let first_session = activate_and_lease_with_config(
            &mut first_store,
            &mut first_images,
            &first_runtime,
            candidate([0x64; 16], 1, 200),
            ServingRuntimeConfig::new(
                128 * 1024 * 1024,
                64 * 1024 * 1024,
                first_directory.path().join("wp64-spill"),
                1,
            )
            .unwrap()
            .with_batch_size(3),
        )
        .unwrap();
        let query = "SELECT entity_id FROM cpg_serving.entities \
                     WHERE entity_kind_code = 1 ORDER BY entity_id";
        let first = first_session.query(query).await.unwrap();

        let (second_directory, mut second_store, mut second_images) = operational_store();
        let second_runtime = ServingSnapshotRuntime::default();
        let second_session = activate_and_lease_with_config(
            &mut second_store,
            &mut second_images,
            &second_runtime,
            candidate([0x64; 16], 1, 200),
            ServingRuntimeConfig::new(
                128 * 1024 * 1024,
                64 * 1024 * 1024,
                second_directory.path().join("wp64-spill"),
                4,
            )
            .unwrap()
            .with_batch_size(17),
        )
        .unwrap();
        let second = second_session.query(query).await.unwrap();

        let first_rows =
            arrow_select::concat::concat_batches(&first.batches[0].schema(), first.batches.iter())
                .unwrap();
        let second_rows = arrow_select::concat::concat_batches(
            &second.batches[0].schema(),
            second.batches.iter(),
        )
        .unwrap();
        assert_eq!(first_rows, second_rows);
        assert_eq!(
            first.artifact.result_checksum,
            second.artifact.result_checksum
        );
        assert_eq!(
            first.artifact.canonical_output_schema_digest,
            second.artifact.canonical_output_schema_digest
        );
        assert_eq!(
            first.artifact.plan_template_id,
            second.artifact.plan_template_id
        );
        assert_ne!(
            first.artifact.bound_query_id,
            second.artifact.bound_query_id
        );

        let other_parameter = first_session
            .query(
                "SELECT entity_id FROM cpg_serving.entities \
                 WHERE entity_kind_code = 2 ORDER BY entity_id",
            )
            .await
            .unwrap();
        assert_eq!(
            first.artifact.plan_template_id,
            other_parameter.artifact.plan_template_id
        );
        assert_ne!(
            first.artifact.bound_query_id,
            other_parameter.artifact.bound_query_id
        );

        let workspace_id = first_session.snapshot_manifest().body.workspace_id;
        let semantic_request = |request_id: &str, kind_code: u16| {
            format!(
                r#"{{"specification":"composable semantic CPG fact query","version":"1.3","semantic_request_id":"{request_id}","workspace_id":"{workspace_id}","freshness_policy":"current_required","queries":[{{"query_id":"entities","request":"find code entities","label":null,"input":null,"where":{{"entity_kind_codes":[{kind_code}],"relation_kind_codes":[]}},"limit":{{"first":10,"offset":0}}}},{{"query_id":"paths","request":"find connecting fact paths","label":null,"input":{{"results":[{{"results_of":"entities","select":"entities"}}]}},"where":null,"limit":{{"first":10,"offset":0}}}}],"response_projection":null,"cost_budget":{{"maximum_rows":20}}}}"#
            )
        };
        let first_typed =
            crate::semantic_query::validate_request(semantic_request("request-one", 1).as_bytes())
                .unwrap();
        let same_query_new_request =
            crate::semantic_query::validate_request(semantic_request("request-two", 1).as_bytes())
                .unwrap();
        let changed_parameter = crate::semantic_query::validate_request(
            semantic_request("request-three", 2).as_bytes(),
        )
        .unwrap();
        let first_bound = crate::semantic_query::bind_request(&first_session, &first_typed)
            .await
            .unwrap();
        let same_query_bound =
            crate::semantic_query::bind_request(&first_session, &same_query_new_request)
                .await
                .unwrap();
        let changed_parameter_bound =
            crate::semantic_query::bind_request(&first_session, &changed_parameter)
                .await
                .unwrap();
        assert_ne!(
            first_typed.request_digest,
            same_query_new_request.request_digest
        );
        assert_eq!(
            first_bound.plan_template_id,
            same_query_bound.plan_template_id
        );
        assert_eq!(first_bound.bound_query_id, same_query_bound.bound_query_id);
        assert_eq!(
            first_bound.plan_template_id,
            changed_parameter_bound.plan_template_id
        );
        assert_ne!(
            first_bound.bound_query_id,
            changed_parameter_bound.bound_query_id
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[allow(clippy::too_many_lines)] // The oracle proves the entire daemon-to-Arrow result vertical in one real session.
    async fn wp63_behavioral_acceptance() {
        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let candidate = candidate([0x63; 16], 1, 3);
        let session = Arc::new(activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            Arc::clone(&candidate),
            directory.path(),
        ));
        let workspace_id = candidate.manifest().body.workspace_id.clone();
        let backend = Arc::new(WorkspaceQueryBackend::default());
        backend.install(session).await.unwrap();
        assert_eq!(backend.active_workspace_count().await, 1);

        let daemon_root = tempdir().unwrap();
        let config = daemon_config(daemon_root.path());
        let discovery = config.static_config.runtime_root.join("daemon.json");
        let query_socket = config.static_config.query_socket_endpoint.clone();
        let claim = WorkspaceClaim {
            workspace_id: workspace_id.clone(),
            repository_id: None,
            worktree_id: None,
            workspace_kind: "non-git-root".to_owned(),
            readiness: WorkspaceReadiness::Ready as i32,
            permission_claims: vec!["query".to_owned()],
        };
        let daemon = tokio::spawn(serve_with_query_backend(config, backend, vec![claim], None));
        wait_for_discovery(&discovery, Duration::from_secs(10))
            .await
            .unwrap();

        let mut client = daemon_query_client(query_socket).await;
        let mut host_capabilities = HostCapabilityProfile {
            delivery_modes: vec![
                DeliveryPreference::Inline as i32,
                DeliveryPreference::Resource as i32,
                DeliveryPreference::Auto as i32,
            ],
            compression_algorithms: vec![PayloadCompression::Identity as i32],
            supports_resource_links: true,
            supports_trace_context: true,
            maximum_frame_bytes: 1_048_576,
            profile_digest: String::new(),
        };
        host_capabilities.profile_digest =
            host_capability_profile_digest(&host_capabilities).unwrap();
        let handshake = client
            .handshake(HandshakeRequest {
                rpc_versions: Some(VersionRange {
                    minimum: "1.0".to_owned(),
                    maximum: "1.0".to_owned(),
                }),
                semantic_query_versions: Some(VersionRange {
                    minimum: "1.3".to_owned(),
                    maximum: "1.3".to_owned(),
                }),
                required_feature_bits: CpgdFeatureMask::REQUIRED.bits(),
                optional_feature_bits: CpgdFeatureMask::SUPPORTED
                    .missing_from(CpgdFeatureMask::REQUIRED)
                    .bits(),
                desired_workspace_ids: vec![workspace_id.clone()],
                host_capabilities: Some(host_capabilities.clone()),
                credential_proof: Some(CredentialProof {
                    credential_id: "wp63-credential".to_owned(),
                    capability_token: b"test-query-capability-token".to_vec(),
                }),
                agent_instance_id: "wp63-agent".to_owned(),
                ..HandshakeRequest::default()
            })
            .await
            .unwrap()
            .into_inner();
        assert_eq!(handshake.authorized_workspaces.len(), 1);
        assert_eq!(handshake.readiness.unwrap().supported_query_forms.len(), 8);

        let status = client
            .get_status(StatusRequest {
                agent_instance_id: "wp63-agent".to_owned(),
                workspace_id: workspace_id.clone(),
                include_diagnostics: false,
            })
            .await
            .unwrap()
            .into_inner();
        let public_status: serde_json::Value =
            serde_json::from_slice(&status.canonical_public_status_json).unwrap();
        assert_eq!(
            public_status["supported_request_forms"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        assert_eq!(
            public_status["capability_statuses"][0]["capability_code"],
            "CORE_SOURCE_V1"
        );

        let request = format!(
            r#"{{"specification":"composable semantic CPG fact query","version":"1.3","semantic_request_id":"wp63-eight-form","workspace_id":"{workspace_id}","freshness_policy":"best_available_snapshot","queries":[{{"query_id":"entities","request":"find code entities","label":null,"input":null,"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"properties","request":"retrieve facts about code","label":null,"input":null,"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"relations","request":"follow code relationships","label":null,"input":null,"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"paths","request":"find connecting fact paths","label":null,"input":{{"results":[{{"results_of":"entities","select":"entities"}}]}},"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"patterns","request":"match a code fact pattern","label":null,"input":{{"results":[{{"results_of":"entities","select":"entities"}}]}},"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"combined","request":"combine result sets","label":null,"input":{{"results":[{{"results_of":"properties","select":"facts"}},{{"results_of":"relations","select":"facts"}}]}},"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"summary","request":"summarize objective facts","label":null,"input":{{"results":[{{"results_of":"combined","select":"groups"}}]}},"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"context","request":"retrieve source and syntax context","label":null,"input":{{"results":[{{"results_of":"paths","select":"paths"}}]}},"where":null,"limit":{{"first":10,"offset":0}}}}],"response_projection":{{"canonical_semantic_identity":true,"coverage":true}},"cost_budget":{{"maximum_rows":80}}}}"#
        );
        let canonical = crate::contracts::jcs::canonicalize_slice(request.as_bytes()).unwrap();
        let started = client
            .start_query(StartQueryRequest {
                agent_instance_id: "wp63-agent".to_owned(),
                workspace_id: workspace_id.clone(),
                semantic_query_version: "1.3".to_owned(),
                canonical_request_json: canonical.clone(),
                request_checksum: crate::integrity::framed_digest(&canonical),
                delivery_preference: DeliveryPreference::Resource as i32,
                deadline_unix_ms: i64::MAX,
                idempotency_key: "wp63-eight-form".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                host_capability_profile_digest: host_capabilities.profile_digest,
                ..StartQueryRequest::default()
            })
            .await
            .unwrap()
            .into_inner();
        let mut events = client
            .stream_query(StreamQueryRequest {
                daemon_query_id: started.daemon_query_id,
                resume_token: started.resume_token,
                after_sequence: 0,
            })
            .await
            .unwrap()
            .into_inner();
        let mut artifact = None;
        let mut succeeded = false;
        while let Some(event) = events.message().await.unwrap() {
            match event.event {
                Some(Event::ArtifactReady(value)) => artifact = Some(value),
                Some(Event::Terminal(value)) => {
                    succeeded = value.execution_state
                        == crate::rpc::generated::codefabric::cpgd::v1::QueryExecutionState::Succeeded
                            as i32;
                }
                _ => {}
            }
        }
        assert!(succeeded);
        let artifact = artifact.expect("query result artifact");
        let mut chunks = client
            .read_result(ReadResultRequest {
                artifact_id: artifact.artifact_id,
                offset: 0,
                maximum_bytes: None,
                lease_token: artifact.lease_token,
                accepted_compression: PayloadCompression::Identity as i32,
            })
            .await
            .unwrap()
            .into_inner();
        let mut response_bytes = Vec::new();
        while let Some(chunk) = chunks.message().await.unwrap() {
            response_bytes.extend_from_slice(&chunk.payload);
            if chunk.final_chunk {
                break;
            }
        }
        let response: serde_json::Value = serde_json::from_slice(&response_bytes).unwrap();
        assert_eq!(response["successful_query_count"], 8);
        assert_eq!(response["query_results"].as_array().unwrap().len(), 8);
        assert_eq!(
            response["snapshot"]["snapshot_id"],
            candidate.manifest().snapshot_id
        );

        drop(client);
        administer(&discovery, AdminCommand::Stop).await.unwrap();
        let exit = daemon.await.unwrap().unwrap();
        assert!(!exit.drained);
        assert!(exit.shutdown_steps.contains(&"await-workers"));
    }

    #[tokio::test]
    async fn production_eight_form_semantic_query_conformance() {
        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let candidate = candidate([0x75; 16], 1, 3);
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            Arc::clone(&candidate),
            directory.path(),
        );
        let workspace_id = candidate.manifest().body.workspace_id.clone();
        let request = format!(
            r#"{{"specification":"composable semantic CPG fact query","version":"1.3","semantic_request_id":"production-eight-form","workspace_id":"{workspace_id}","freshness_policy":"best_available_snapshot","queries":[{{"query_id":"entities","request":"find code entities","label":null,"input":null,"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"properties","request":"retrieve facts about code","label":null,"input":null,"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"relations","request":"follow code relationships","label":null,"input":null,"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"paths","request":"find connecting fact paths","label":null,"input":{{"results":[{{"results_of":"entities","select":"entities"}}]}},"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"patterns","request":"match a code fact pattern","label":null,"input":{{"results":[{{"results_of":"entities","select":"entities"}}]}},"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"combined","request":"combine result sets","label":null,"input":{{"results":[{{"results_of":"properties","select":"facts"}},{{"results_of":"relations","select":"facts"}}]}},"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"summary","request":"summarize objective facts","label":null,"input":{{"results":[{{"results_of":"combined","select":"groups"}}]}},"where":null,"limit":{{"first":10,"offset":0}}}},{{"query_id":"context","request":"retrieve source and syntax context","label":null,"input":{{"results":[{{"results_of":"paths","select":"paths"}}]}},"where":null,"limit":{{"first":10,"offset":0}}}}],"response_projection":{{"canonical_semantic_identity":true,"coverage":true}},"cost_budget":{{"maximum_rows":80}}}}"#
        );
        let first = crate::semantic_query::execute_request(
            &session,
            crate::semantic_query::validate_request(request.as_bytes()).unwrap(),
            crate::registries::FreshnessState::Current,
        )
        .await
        .unwrap();
        let second = crate::semantic_query::execute_request(
            &session,
            crate::semantic_query::validate_request(request.as_bytes()).unwrap(),
            crate::registries::FreshnessState::Current,
        )
        .await
        .unwrap();
        assert_eq!(first.response.successful_query_count, 8);
        assert_eq!(first.response.query_results.len(), 8);
        assert_eq!(
            first.response.snapshot.snapshot_id,
            candidate.manifest().snapshot_id
        );
        assert_eq!(first.canonical_bytes, second.canonical_bytes);
        assert_eq!(first.response_digest, second.response_digest);
        assert_eq!(
            first
                .response
                .query_results
                .iter()
                .filter(|result| {
                    result.resolved_semantics["operator_family"] == "datafusion-relational"
                })
                .count(),
            3
        );
        assert_eq!(
            first
                .response
                .query_results
                .iter()
                .filter(|result| {
                    result.resolved_semantics["operator_family"] == "application-graph"
                })
                .count(),
            5
        );
        assert!(
            first
                .response
                .query_results
                .iter()
                .find(|result| result.request
                    == crate::semantic_query::QueryForm::RetrieveSourceContext)
                .is_some_and(|result| !result.source_context_ids.is_empty())
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wp25_behavioral_acceptance() {
        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let first = candidate([0x22; 16], 1, 1);
        let session = Arc::new(activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            Arc::clone(&first),
            directory.path(),
        ));

        let result = session
            .query("SELECT entity_id, language_name FROM entities WHERE entity_kind_code = 1")
            .await
            .unwrap();
        assert_eq!(result.artifact.output_row_count, 1);
        assert_eq!(
            result.batches[0]
                .column(1)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            "Rust"
        );
        assert!(result.artifact.logical_plan.contains("Filter"));
        assert!(result.artifact.physical_plan.contains("Projection"));
        assert_eq!(
            result.artifact.source_table_versions.len(),
            first.providers().provider_records().len()
        );
        assert!(
            result.artifact.execution_metrics["operator_output_rows"]
                > result.artifact.output_row_count as u64
        );
        assert_eq!(result.artifact.execution_metrics["spill_count"], 0);
        assert_eq!(result.artifact.execution_metrics["spilled_bytes"], 0);

        let planned = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        let mut query_task = tokio::spawn({
            let session = Arc::clone(&session);
            let planned = Arc::clone(&planned);
            let resume = Arc::clone(&resume);
            async move {
                session
                    .query_with_barrier("SELECT COUNT(*) FROM entities", &planned, &resume)
                    .await
            }
        });
        tokio::select! {
            _ = planned.wait() => {}
            result = &mut query_task => panic!("query terminated before the planning barrier: {result:?}"),
        }
        let predecessor = first.manifest().raw_snapshot_id().unwrap();
        let second = candidate([0x23; 16], 2, 2);
        runtime
            .activate(&mut store, second, Some(predecessor), 1, 8, 102, None)
            .unwrap();
        resume.wait().await;
        let pinned = query_task.await.unwrap().unwrap();
        assert_eq!(count(&pinned), 1);
        assert_eq!(session.snapshot_id(), predecessor);
    }

    #[tokio::test]
    async fn wp65_behavioral_acceptance() {
        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            candidate([0x65; 16], 1, 2),
            directory.path(),
        );
        let plan = LogicalPlanBuilder::from(session.table_plan("entities").await.unwrap())
            .project(vec![col("entity_id")])
            .unwrap()
            .sort(vec![col("entity_id").sort(true, true)])
            .unwrap()
            .build()
            .unwrap();
        let first_context = QueryExecutionContext {
            execution_id: "execution:agent-one".to_owned(),
            semantic_request_id: "semantic-agent-one".to_owned(),
            mcp_call_id: "mcp-agent-one".to_owned(),
        };
        let second_context = QueryExecutionContext {
            execution_id: "execution:agent-two".to_owned(),
            semantic_request_id: "semantic-agent-two".to_owned(),
            mcp_call_id: "mcp-agent-two".to_owned(),
        };
        let first = session
            .query_plan_in_execution("wp65-single-scan", plan.clone(), &first_context)
            .await
            .unwrap();
        let second = session
            .query_plan_in_execution("wp65-single-scan", plan, &second_context)
            .await
            .unwrap();

        assert_eq!(first.artifact.execution_id, first_context.execution_id);
        assert_eq!(second.artifact.execution_id, second_context.execution_id);
        assert_ne!(first.artifact.execution_id, second.artifact.execution_id);
        assert_ne!(
            first.artifact.semantic_request_id,
            second.artifact.semantic_request_id
        );
        assert_eq!(
            first.artifact.result_checksum,
            second.artifact.result_checksum
        );
        assert_eq!(session.runtime_evidence().observed_query_count, 2);
        assert!(!first.artifact.physical_plan.contains("AnalyzeExec"));
        assert!(
            !first
                .artifact
                .physical_plan_with_full_metrics
                .contains("AnalyzeExec")
        );
    }

    #[tokio::test]
    async fn wp65_structural_acceptance() {
        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            candidate([0x66; 16], 3, 1),
            directory.path(),
        );
        let plan = LogicalPlanBuilder::from(session.table_plan("entities").await.unwrap())
            .project(vec![col("entity_id")])
            .unwrap()
            .build()
            .unwrap();
        let result = session
            .query_plan_in_execution(
                "wp65-pins",
                plan,
                &QueryExecutionContext {
                    execution_id: "execution:wp65-pins".to_owned(),
                    semantic_request_id: "semantic:wp65-pins".to_owned(),
                    mcp_call_id: "mcp:wp65-pins".to_owned(),
                },
            )
            .await
            .unwrap();
        let artifact = &result.artifact;
        assert_eq!(
            artifact.artifact_schema_version,
            "codefabric.query-plan-artifact.v1"
        );
        for bundle_id in [
            &artifact.bundle_ids.ontology_bundle_id,
            &artifact.bundle_ids.schema_bundle_id,
            &artifact.bundle_ids.provider_bundle_id,
            &artifact.bundle_ids.derivation_bundle_id,
            &artifact.bundle_ids.query_language_bundle_id,
            &artifact.bundle_ids.model_pack_bundle_id,
            &artifact.bundle_ids.toolchain_bundle_id,
        ] {
            assert!(!bundle_id.is_empty());
        }
        assert!(!artifact.source_table_versions.is_empty());
        assert_eq!(artifact.overlay_generation, 0);
        assert!(artifact.overlay_table_versions.is_empty());
        assert!(
            artifact
                .control_schema_generation_fingerprint
                .starts_with("b3:")
        );
        assert!(
            artifact
                .physical_plan_with_full_metrics
                .contains("output_rows")
        );
        let pg_json: serde_json::Value =
            serde_json::from_str(&artifact.physical_plan_pg_json).unwrap();
        assert!(pg_json.is_object() || pg_json.is_array());
        let round_trip: QueryPlanArtifact =
            serde_json::from_slice(&serde_json::to_vec(artifact).unwrap()).unwrap();
        assert_eq!(&round_trip, artifact);
    }

    #[tokio::test]
    async fn wp66_behavioral_acceptance() {
        let (explanation, typed_scope_rows, manifest) = explainable_version_fixture().await;
        assert_eq!(
            typed_scope_rows, 1,
            "typed mutation scope join is incomplete"
        );
        assert_eq!(explanation.table_code, 100);
        assert_eq!(explanation.table_name, "entity");
        assert_eq!(explanation.executions.len(), 1);
        let plan = &explanation.executions[0].plan_artifacts[0];
        assert_eq!(
            plan.source_table_versions.get(&100),
            Some(&explanation.delta_version)
        );
        assert_eq!(plan.snapshot_id, manifest["snapshot_id"]);
        assert!(manifest["source_blob_digests"].is_array());
        for bundle in [
            &plan.bundle_ids.ontology_bundle_id,
            &plan.bundle_ids.schema_bundle_id,
            &plan.bundle_ids.provider_bundle_id,
            &plan.bundle_ids.derivation_bundle_id,
            &plan.bundle_ids.query_language_bundle_id,
            &plan.bundle_ids.model_pack_bundle_id,
            &plan.bundle_ids.toolchain_bundle_id,
        ] {
            assert!(!bundle.is_empty(), "provenance bundle identity is absent");
        }
        assert!(explanation.delta_commit_info.get("workspace_id").is_some());
        assert!(
            explanation
                .delta_commit_info
                .get("source_generation")
                .is_some()
        );
    }

    #[tokio::test]
    async fn wp66_operational_acceptance() {
        let started = std::time::Instant::now();
        let (explanation, _, _) = explainable_version_fixture().await;
        assert!(started.elapsed() < Duration::from_secs(10));
        assert_eq!(explanation.scanned_artifact_count, 1);
        assert_eq!(
            explanation.executions[0].phase,
            QueryArtifactPhase::Succeeded
        );
    }

    #[tokio::test]
    async fn wp25_structural_acceptance() {
        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let candidate = candidate([0x22; 16], 1, 1);
        let expected_provider = candidate.providers().provider(100).unwrap();
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            Arc::clone(&candidate),
            directory.path(),
        );
        let catalog = session.context.catalog(CATALOG_NAME).unwrap();
        assert_eq!(
            catalog.schema_names().into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([
                BASE_SCHEMA.into(),
                CONTROL_SCHEMA.into(),
                SERVING_SCHEMA.into(),
                "cpg_python".into(),
                "cpg_rust".into(),
                "cpg_derived".into(),
            ])
        );
        let base_provider = catalog
            .schema(BASE_SCHEMA)
            .unwrap()
            .table("entity")
            .await
            .unwrap()
            .unwrap();
        assert!(Arc::ptr_eq(&base_provider, &expected_provider));
        let serving_provider = catalog
            .schema(SERVING_SCHEMA)
            .unwrap()
            .table("entities")
            .await
            .unwrap()
            .unwrap();
        let serving_schema_provider = catalog.schema(SERVING_SCHEMA).unwrap();
        assert_eq!(
            serving_schema_provider
                .table_names()
                .into_iter()
                .collect::<BTreeSet<_>>(),
            serving_projection_specs()
                .iter()
                .map(|projection| projection.view_name.to_owned())
                .collect()
        );
        assert!(serving_schema_provider.table_exist("entities"));
        assert!(!serving_schema_provider.table_exist("missing"));
        let serving_schema = serving_provider.schema();
        let names = serving_schema
            .fields()
            .iter()
            .map(|field| field.name().as_str())
            .collect::<BTreeSet<_>>();
        assert!(!names.contains("owner_bucket"));
        assert!(!names.contains("fact_hash64"));
        assert!(names.contains("language_name"));
        for projection in serving_projection_specs() {
            let spec = table_spec(projection.source_table_code).unwrap();
            assert_eq!(
                spec.materialization_role,
                MaterializationRole::DurableEffective
            );
            assert_eq!(spec.overlay_mutation, OverlayMutationPolicy::OwnerReplace);
        }
        assert!(
            catalog
                .register_schema("mutable", Arc::new(ImmutableSchemaProvider::default()))
                .is_err()
        );
        assert!(catalog.deregister_schema(SERVING_SCHEMA, false).is_err());
        assert!(
            serving_schema_provider
                .register_table("mutable".into(), Arc::clone(&expected_provider))
                .is_err()
        );
        assert!(
            serving_schema_provider
                .deregister_table("entities")
                .is_err()
        );
        let debug = format!("{session:?}");
        assert!(debug.contains("ServingQuerySession"));
        assert!(debug.contains("evidence"));
    }

    #[tokio::test]
    async fn wp25_negative_zero_state() {
        assert!(ServingRuntimeConfig::new(0, 1, PathBuf::from("/tmp"), 1).is_err());
        assert!(ServingRuntimeConfig::new(1, 1, PathBuf::from("relative"), 1).is_err());
        let mut unspecified = table_spec(100).unwrap().clone();
        unspecified.overlay_mutation = OverlayMutationPolicy::NotApplicable;
        assert!(validate_serving_spec(&unspecified).is_err());
        let enum_only = SnapshotProviderCatalog::from_batches_for_snapshot_tests(
            [0x22; 16],
            WORKSPACE,
            vec![(11, generated_batch(11, 1))],
            OVERLAY,
            1,
            vec![CONTEXT],
        );
        assert!(build_serving_schema(&enum_only).is_err());

        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            candidate([0x22; 16], 1, 1),
            directory.path(),
        );
        for sql in [
            "CREATE TABLE protected(a INT)",
            "INSERT INTO cpg_base.entity SELECT * FROM cpg_base.entity",
            "SET datafusion.execution.batch_size = 1",
        ] {
            let plan = session
                .context
                .state()
                .create_logical_plan(sql)
                .await
                .unwrap();
            assert!(
                read_only_options().verify_plan(&plan).is_err(),
                "read-only options unexpectedly accepted: {sql}"
            );
        }
        let scalar_variable = LogicalPlanBuilder::empty(false)
            .project([Expr::ScalarVariable(
                Arc::new(Field::new("forbidden", DataType::Utf8, true)),
                vec!["forbidden".into()],
            )])
            .unwrap()
            .build()
            .unwrap();
        assert!(validate_plan_allowlist(&scalar_variable).is_err());
        let approved_scalar = session
            .query("SELECT lower(name) FROM entities")
            .await
            .unwrap();
        assert_eq!(approved_scalar.artifact.output_row_count, 1);
        for sql in [
            "CREATE TABLE nope AS SELECT 1",
            "INSERT INTO entities SELECT * FROM entities",
            "SET datafusion.execution.batch_size = 1",
            "SHOW TABLES",
            "SELECT random() FROM entities",
            "SELECT array_agg(entity_kind_code) FROM entities",
            "SELECT row_number() OVER () FROM entities",
            "SELECT * FROM read_parquet('file:///tmp/nope.parquet')",
            "SELECT owner_bucket FROM entities",
        ] {
            assert!(
                session.query(sql).await.is_err(),
                "query unexpectedly passed: {sql}"
            );
        }
        assert_table_reference_policy();
        assert_operational_value_types();
        for detail in ["Resources exhausted: probe", "allocation failed: probe"] {
            assert!(matches!(
                ServingQueryError::from(datafusion::error::DataFusionError::Execution(
                    detail.into()
                )),
                ServingQueryError::ResourceLimit(_)
            ));
        }
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // One operational scenario proves the complete resource lifecycle.
    async fn wp25_operational_acceptance() {
        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            candidate([0x22; 16], 1, 1),
            directory.path(),
        );
        let evidence = session.runtime_evidence();
        assert_eq!(evidence.memory_pool, "track_consumers");
        assert_eq!(evidence.memory_limit_bytes, 16 * 1024 * 1024);
        assert_eq!(evidence.max_spill_bytes, 64 * 1024 * 1024);
        assert_eq!(evidence.batch_size, 65_536);
        assert_eq!(evidence.target_partitions, 2);
        assert_eq!(evidence.observed_query_count, 0);
        assert_eq!(evidence.observed_pruning_metric_count, 0);
        assert_eq!(evidence.observed_pruned_row_groups, 0);
        assert_eq!(evidence.observed_repartition_operator_count, 0);
        assert_eq!(evidence.observed_repartition_output_rows, 0);
        assert!(evidence.spill_directory.is_dir());

        let control = session
            .query(
                "SELECT consistency FROM cpg_control.active_serving_snapshot WHERE source_generation = 1",
            )
            .await
            .unwrap();
        assert_eq!(
            control.batches[0]
                .column(0)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(0),
            "operationally-current-not-snapshot-pinned"
        );
        assert_eq!(
            count(
                &session
                    .query("SELECT COUNT(*) FROM cpg_control.worktree_state")
                    .await
                    .unwrap()
            ),
            1
        );
        assert_eq!(
            count(
                &session
                    .query("SELECT COUNT(*) FROM cpg_control.workspace_update_state")
                    .await
                    .unwrap()
            ),
            1
        );
        assert_eq!(
            count(
                &session
                    .query("SELECT COUNT(*) FROM cpg_control.source_trust_state")
                    .await
                    .unwrap()
            ),
            1
        );
        assert_eq!(
            control.artifact.datafusion_version,
            datafusion::DATAFUSION_VERSION
        );
        assert_eq!(control.artifact.arrow_version, arrow::ARROW_VERSION);
        assert!(control.artifact.plan_template_id.starts_with("b3:"));
        assert!(control.artifact.bound_query_id.starts_with("b3:"));
        assert_eq!(
            control.artifact.result_checksum_version,
            crate::fabric::RESULT_CHECKSUM_VERSION
        );
        assert!(control.artifact.reproducibility.deterministic);
        assert!(control.artifact.result_checksum.starts_with("b3:"));
        assert_eq!(
            control
                .artifact
                .execution_metrics
                .keys()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                &"memory_reserved_after_execution".to_owned(),
                &"operator_output_rows".to_owned(),
                &"output_batches".to_owned(),
                &"output_bytes".to_owned(),
                &"output_partitions".to_owned(),
                &"output_rows".to_owned(),
                &"pruned_row_groups".to_owned(),
                &"pruning_metric_count".to_owned(),
                &"repartition_operator_count".to_owned(),
                &"repartition_output_rows".to_owned(),
                &"spill_count".to_owned(),
                &"spilled_bytes".to_owned(),
            ])
        );
        let evidence = session.runtime_evidence();
        assert_eq!(evidence.observed_query_count, 4);
        assert!(
            evidence.observed_pruning_metric_count
                >= control.artifact.execution_metrics["pruning_metric_count"]
        );
        assert!(
            evidence.observed_repartition_operator_count
                >= control.artifact.execution_metrics["repartition_operator_count"]
        );
    }

    async fn assert_result_limit(rows: usize, bytes: usize, batches: usize, marker: u8) {
        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let config = ServingRuntimeConfig::new(
            16 * 1024 * 1024,
            64 * 1024 * 1024,
            directory.path().join("bounded-spill"),
            1,
        )
        .unwrap()
        .with_result_limits(rows, bytes, batches);
        let session = activate_and_lease_with_config(
            &mut store,
            &mut images,
            &runtime,
            candidate([marker; 16], 1, 2),
            config,
        )
        .unwrap();
        assert!(matches!(
            session
                .query("SELECT entity_id FROM cpg_serving.entities")
                .await,
            Err(ServingQueryError::ResourceLimit(_))
        ));
    }

    async fn assert_result_limits_are_inclusive() {
        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let probe = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            candidate([0x37; 16], 1, 2),
            directory.path(),
        )
        .query("SELECT entity_id FROM cpg_serving.entities")
        .await
        .unwrap();
        let rows = probe.artifact.output_row_count;
        let bytes = usize::try_from(probe.artifact.execution_metrics["output_bytes"]).unwrap();
        let batches = usize::try_from(probe.artifact.execution_metrics["output_batches"]).unwrap();

        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let config = ServingRuntimeConfig::new(
            16 * 1024 * 1024,
            64 * 1024 * 1024,
            directory.path().join("inclusive-spill"),
            1,
        )
        .unwrap()
        .with_result_limits(rows, bytes, batches);
        let session = activate_and_lease_with_config(
            &mut store,
            &mut images,
            &runtime,
            candidate([0x38; 16], 1, 2),
            config,
        )
        .unwrap();
        assert_eq!(
            session
                .query("SELECT entity_id FROM cpg_serving.entities")
                .await
                .unwrap()
                .artifact
                .output_row_count,
            rows
        );
    }

    fn retain_control_batch(
        batch: &RecordBatch,
        max_rows: usize,
        max_bytes: usize,
        max_batches: usize,
    ) -> Result<(), ServingQueryError> {
        let pool: Arc<dyn MemoryPool> = Arc::new(UnboundedMemoryPool::default());
        let reservation = MemoryConsumer::new("control-budget-test").register(&pool);
        CaptureBudget {
            rows: 0,
            bytes: 0,
            batches: 0,
            max_rows,
            max_bytes,
            max_batches,
            reservation: &reservation,
        }
        .retain(batch)
    }

    fn assert_control_limits_are_independent_and_inclusive() {
        let batch = generated_batch(11, 1);
        let bytes = batch.get_array_memory_size();
        assert!(retain_control_batch(&batch, 1, bytes, 1).is_ok());
        for result in [
            retain_control_batch(&batch, 0, usize::MAX, usize::MAX),
            retain_control_batch(&batch, usize::MAX, bytes - 1, usize::MAX),
            retain_control_batch(&batch, usize::MAX, usize::MAX, 0),
        ] {
            assert!(matches!(result, Err(ServingQueryError::ResourceLimit(_))));
        }
    }

    #[tokio::test]
    async fn wp25_resource_and_cancellation_acceptance() {
        assert_result_limit(0, usize::MAX, usize::MAX, 0x31).await;
        assert_result_limit(usize::MAX, 1, usize::MAX, 0x32).await;
        assert_result_limit(usize::MAX, usize::MAX, 0, 0x33).await;
        assert_result_limits_are_inclusive().await;
        assert_control_limits_are_independent_and_inclusive();

        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let control_limited = ServingRuntimeConfig::new(
            16 * 1024 * 1024,
            64 * 1024 * 1024,
            directory.path().join("control-spill"),
            1,
        )
        .unwrap()
        .with_control_limits(0, usize::MAX, usize::MAX);
        assert!(matches!(
            activate_and_lease_with_config(
                &mut store,
                &mut images,
                &runtime,
                candidate([0x34; 16], 1, 1),
                control_limited,
            ),
            Err(ServingQueryError::ResourceLimit(_))
        ));

        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            candidate([0x35; 16], 1, 2),
            directory.path(),
        );
        let mut stream = session
            .start_query_stream("SELECT * FROM cpg_serving.entities")
            .await
            .unwrap();
        assert!(stream.next().await.transpose().unwrap().is_some());
        drop(stream);
        assert_eq!(
            count(
                &session
                    .query("SELECT COUNT(*) FROM cpg_serving.entities")
                    .await
                    .unwrap()
            ),
            2
        );

        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let low_memory = ServingRuntimeConfig::new(
            256 * 1024,
            4 * 1024 * 1024,
            directory.path().join("low-memory-spill"),
            1,
        )
        .unwrap();
        let session = activate_and_lease_with_config(
            &mut store,
            &mut images,
            &runtime,
            candidate([0x36; 16], 1, 200),
            low_memory,
        )
        .unwrap();
        match session
            .query("SELECT entity_id FROM cpg_serving.entities ORDER BY entity_id DESC")
            .await
        {
            Ok(result) => {
                assert!(
                    result.artifact.execution_metrics["spill_count"] > 0
                        || result.artifact.execution_metrics["spilled_bytes"] > 0
                );
            }
            Err(ServingQueryError::ResourceLimit(detail)) => {
                assert!(!detail.is_empty());
            }
            Err(error) => panic!("unexpected low-memory outcome: {error}"),
        }
    }

    #[tokio::test]
    async fn wp25_delta_serving_acceptance() {
        let (directory, mut store, mut images) = operational_store();
        let (_, candidate) = published_delta_candidate(directory.path(), &mut store).await;
        let runtime = ServingSnapshotRuntime::default();
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            candidate,
            directory.path(),
        );
        let result = session
            .query(
                "SELECT workspace_id FROM cpg_base.workspace \
                 WHERE workspace_id IS NOT NULL",
            )
            .await
            .unwrap();
        assert_eq!(result.artifact.output_row_count, 1);
        assert_eq!(result.artifact.output_schema, ["workspace_id"]);
        assert_eq!(
            session
                .query("SELECT entity_id FROM cpg_base.entity")
                .await
                .unwrap()
                .artifact
                .output_row_count,
            1
        );
        assert_eq!(
            session
                .query("SELECT COUNT(*) FROM cpg_base.entity")
                .await
                .unwrap()
                .artifact
                .output_row_count,
            1
        );
        assert_eq!(
            session
                .query("SELECT entity_id FROM cpg_serving.entities")
                .await
                .unwrap()
                .artifact
                .output_row_count,
            1
        );
        let scoped = session
            .query(
                "SELECT analysis_context_id, source_generation \
                 FROM cpg_base.entity",
            )
            .await
            .unwrap();
        assert_eq!(
            scoped.batches[0]
                .column(0)
                .as_any()
                .downcast_ref::<arrow_array::FixedSizeBinaryArray>()
                .unwrap()
                .value(0),
            CONTEXT
        );
        assert_eq!(
            scoped.batches[0]
                .column(1)
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap()
                .value(0),
            1
        );
        let plans = format!(
            "UNOPTIMIZED\n{}\nOPTIMIZED\n{}\nPHYSICAL\n{}",
            result.artifact.logical_plan,
            result.artifact.optimized_logical_plan,
            result.artifact.physical_plan,
        );
        let normalized = normalize_plan(&plans, directory.path());
        assert!(normalized.contains("workspace_id IS NOT NULL"));
        assert!(normalized.contains("DeltaScanExec"));
        assert!(normalized.contains("projection=[workspace_id]"));
        assert!(normalized.contains("pruning_predicate="));
        insta::assert_snapshot!(normalized, @r###"
UNOPTIMIZED
Projection: cpg_base.workspace.workspace_id [workspace_id:FixedSizeBinary(16)]
  Filter: cpg_base.workspace.workspace_id IS NOT NULL [workspace_id:FixedSizeBinary(16), repository_id:FixedSizeBinary(16);N, worktree_id:FixedSizeBinary(16);N, workspace_kind_code:Int16, canonical_name:Utf8, root_path_bytes:Binary, root_path_display:Utf8, root_path_encoding_code:Int16, authorization_fingerprint:Binary, language_mask:Int16, registration_revision:Int64, created_at:Timestamp(µs, "UTC"), updated_at:Timestamp(µs, "UTC")]
    TableScan: cpg_base.workspace [workspace_id:FixedSizeBinary(16), repository_id:FixedSizeBinary(16);N, worktree_id:FixedSizeBinary(16);N, workspace_kind_code:Int16, canonical_name:Utf8, root_path_bytes:Binary, root_path_display:Utf8, root_path_encoding_code:Int16, authorization_fingerprint:Binary, language_mask:Int16, registration_revision:Int64, created_at:Timestamp(µs, "UTC"), updated_at:Timestamp(µs, "UTC")]
OPTIMIZED
TableScan: cpg_base.workspace projection=[workspace_id] [workspace_id:FixedSizeBinary(16)]
PHYSICAL
FilterExec: workspace_id@0 = 11111111111111111111...
  ProjectionExec: expr=[CAST(workspace_id@0 AS FixedSizeBinary(16)) as workspace_id]
    DeltaScanExec
      RepartitionExec: partitioning=RoundRobinBatch(2), input_partitions=1
        DataSourceExec: file_groups=<NORMALIZED>, projection=[workspace_id, __delta_rs_file_id__], file_type=parquet, predicate=CAST(workspace_id@0 AS FixedSizeBinary(16)) = 11111111111111111111... AND CAST(workspace_id@0 AS FixedSizeBinary(16)) = 11111111111111111111..., pruning_predicate=workspace_id_null_count@2 != row_count@3 AND CAST(workspace_id_min@0 AS FixedSizeBinary(16)) <= 11111111111111111111... AND 11111111111111111111... <= CAST(workspace_id_max@1 AS FixedSizeBinary(16)) AND workspace_id_null_count@2 != row_count@3 AND CAST(workspace_id_min@0 AS FixedSizeBinary(16)) <= 11111111111111111111... AND 11111111111111111111... <= CAST(workspace_id_max@1 AS FixedSizeBinary(16)), required_guarantees=[]
"###);
    }

    #[tokio::test]
    async fn datafusion_55_serving_equivalence() {
        let (directory, mut store, mut images) = operational_store();
        let (_, candidate) = published_delta_candidate(directory.path(), &mut store).await;
        let runtime = ServingSnapshotRuntime::default();
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            candidate,
            directory.path(),
        );
        let result = session
            .query(
                "SELECT workspace_id FROM cpg_base.workspace \
                 WHERE workspace_id IS NOT NULL",
            )
            .await
            .unwrap();
        assert_eq!(
            result.artifact.datafusion_version,
            datafusion::DATAFUSION_VERSION
        );
        assert_eq!(result.artifact.arrow_version, arrow::ARROW_VERSION);
        assert_eq!(result.artifact.output_row_count, 1);
        assert!(!result.artifact.source_table_versions.is_empty());
        assert!(result.artifact.physical_plan.contains("DeltaScanExec"));
        assert!(result.artifact.physical_plan.contains("pruning_predicate="));
        let evidence = session.runtime_evidence();
        assert!(
            result.artifact.execution_metrics["memory_reserved_after_execution"]
                < evidence.memory_limit_bytes as u64
        );
        assert_eq!(
            evidence.observed_query_count, 1,
            "only the served Delta query contributes observed runtime evidence"
        );
        assert_eq!(
            evidence.observed_pruning_metric_count,
            result.artifact.execution_metrics["pruning_metric_count"]
        );
        assert_eq!(
            evidence.observed_pruned_row_groups,
            result.artifact.execution_metrics["pruned_row_groups"]
        );
        assert_eq!(
            evidence.observed_repartition_operator_count,
            result.artifact.execution_metrics["repartition_operator_count"]
        );
        assert_eq!(
            evidence.observed_repartition_output_rows,
            result.artifact.execution_metrics["repartition_output_rows"]
        );
    }

    #[tokio::test]
    async fn wp58_operational_acceptance() {
        let (directory, mut store, mut images) = operational_store();
        let (_, candidate) = published_delta_candidate(directory.path(), &mut store).await;
        let runtime = ServingSnapshotRuntime::default();
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            candidate,
            directory.path(),
        );
        let initial = session.runtime_evidence();
        assert_eq!(initial.observed_query_count, 0);
        assert_eq!(initial.observed_pruning_metric_count, 0);
        assert_eq!(initial.observed_repartition_operator_count, 0);

        let result = session
            .query(
                "SELECT workspace_id FROM cpg_base.workspace \
                 WHERE workspace_id IS NOT NULL",
            )
            .await
            .unwrap();
        assert!(result.artifact.physical_plan.contains("DeltaScanExec"));
        assert!(
            result
                .artifact
                .physical_plan
                .contains("CAST(workspace_id@0 AS FixedSizeBinary(16))")
        );
        assert!(result.artifact.physical_plan.contains("pruning_predicate="));
        assert_eq!(
            result.batches[0]
                .schema()
                .field(0)
                .try_extension_type::<crate::schema_registry::Id16Extension>()
                .unwrap(),
            crate::schema_registry::Id16Extension::v1()
        );

        let observed = session.runtime_evidence();
        assert_eq!(observed.observed_query_count, 1);
        assert_eq!(
            observed.observed_pruning_metric_count,
            result.artifact.execution_metrics["pruning_metric_count"]
        );
        assert_eq!(
            observed.observed_pruned_row_groups,
            result.artifact.execution_metrics["pruned_row_groups"]
        );
        assert_eq!(
            observed.observed_repartition_operator_count,
            result.artifact.execution_metrics["repartition_operator_count"]
        );
        assert_eq!(
            observed.observed_repartition_output_rows,
            result.artifact.execution_metrics["repartition_output_rows"]
        );
    }

    #[tokio::test]
    async fn wp03_operational_resource_cancellation() {
        let (directory, mut store, mut images) = operational_store();
        let runtime = ServingSnapshotRuntime::default();
        let session = activate_and_lease(
            &mut store,
            &mut images,
            &runtime,
            candidate([0x57; 16], 1, 2),
            directory.path(),
        );
        let mut stream = session
            .start_query_stream("SELECT * FROM cpg_serving.entities")
            .await
            .unwrap();
        assert!(stream.next().await.transpose().unwrap().is_some());
        drop(stream);
        let follow_up = session
            .query("SELECT COUNT(*) FROM cpg_serving.entities")
            .await
            .unwrap();
        assert_eq!(count(&follow_up), 2);
        assert!(
            follow_up.artifact.execution_metrics["memory_reserved_after_execution"]
                < session.runtime_evidence().memory_limit_bytes as u64
        );
    }
}
