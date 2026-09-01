//! Exact-version Delta semantic reads with native DataFusion statistics.
//!
//! Delta transaction-log statistics are useful optimizer evidence, but they are
//! not semantic authority. This adapter therefore keeps three boundaries
//! together:
//!
//! * one application-owned exact table pin validated against a loaded snapshot;
//! * one delta-rs `TableProvider`, which alone owns Delta protocol, column
//!   mapping, deletion-vector, file-skipping, and Parquet adaptation; and
//! * one DataFusion logical/physical plan, which retains residual filters when
//!   delta-rs reports an inexact pushdown.
//!
//! Missing statistics remain DataFusion `Precision::Absent`; no row count or
//! column bound is synthesized from missing Delta `add.stats` cells.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use datafusion::common::{Column, DataFusionError, Statistics};
use datafusion::datasource::provider_as_source;
use datafusion::execution::SessionState;
use datafusion::logical_expr::{Expr, LogicalPlanBuilder, TableProviderFilterPushDown};
use datafusion::physical_plan::statistics::{StatisticsArgs, StatisticsContext};
use datafusion::physical_plan::{ExecutionPlan, collect};
use deltalake::kernel::transaction::{PROTOCOL, TransactionError};
use deltalake::{DeltaTable, DeltaTableError};
use thiserror::Error;

use super::delta_exact::{
    ExactDeltaPin, ExactDeltaProviderError, ExactDeltaStatisticsInspection, ValidatedDeltaSnapshot,
    provider_read_from_validated_snapshot,
};
use super::provider::{ProviderContractError, SchemaContractStorageProvider};
use crate::schema_contract::{
    ColumnMappingMode, DeletionVectorBehavior, SchemaContract, SchemaContractError, SchemaPhase,
    SchemaRole,
};

const EXACT_READ_SOURCE: &str = "__codefabric_exact_delta";
const COLUMN_MAPPING_MODE_KEY: &str = "delta.columnMapping.mode";
const DELETION_VECTORS_KEY: &str = "delta.enableDeletionVectors";
const COLUMN_MAPPING_FEATURE: &str = "columnMapping";
const DELETION_VECTORS_FEATURE: &str = "deletionVectors";

/// One exact semantic Delta read requested by application-owned field indices.
///
/// Filter expressions are expressed against the contract's unqualified logical
/// field names. Their referenced indices are derived and checked during
/// preparation; callers cannot supply a second, potentially contradictory
/// filter-index list.
#[derive(Clone, Debug)]
pub struct ExactDeltaSemanticReadRequest {
    pin: ExactDeltaPin,
    logical_projection: Option<Arc<[usize]>>,
    filters: Arc<[Expr]>,
    limit: Option<usize>,
}

impl ExactDeltaSemanticReadRequest {
    /// Construct an explicit exact read. `None` projects every logical field.
    #[must_use]
    pub fn new(
        pin: ExactDeltaPin,
        logical_projection: Option<Vec<usize>>,
        filters: Vec<Expr>,
        limit: Option<usize>,
    ) -> Self {
        Self {
            pin,
            logical_projection: logical_projection.map(Arc::from),
            filters: Arc::from(filters),
            limit,
        }
    }

    /// Exact table root and version selected by the application.
    #[must_use]
    pub const fn pin(&self) -> &ExactDeltaPin {
        &self.pin
    }

    /// Ordered logical projection, or `None` for every logical field.
    #[must_use]
    pub fn logical_projection(&self) -> Option<&[usize]> {
        self.logical_projection.as_deref()
    }

    /// Residual-correct DataFusion filter expressions.
    #[must_use]
    pub fn filters(&self) -> &[Expr] {
        &self.filters
    }

    /// Optional semantic output limit.
    #[must_use]
    pub const fn limit(&self) -> Option<usize> {
        self.limit
    }
}

/// Exact schema-index bindings used by the Delta and DataFusion read paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDeltaReadIndexBindings {
    logical_projection: Arc<[usize]>,
    provider_projection: Arc<[usize]>,
    logical_filter_fields: Arc<[usize]>,
    provider_filter_fields: Arc<[usize]>,
    provider_statistics_fields: Arc<[usize]>,
}

impl ExactDeltaReadIndexBindings {
    #[must_use]
    pub fn logical_projection(&self) -> &[usize] {
        &self.logical_projection
    }

    #[must_use]
    pub fn provider_projection(&self) -> &[usize] {
        &self.provider_projection
    }

    #[must_use]
    pub fn logical_filter_fields(&self) -> &[usize] {
        &self.logical_filter_fields
    }

    #[must_use]
    pub fn provider_filter_fields(&self) -> &[usize] {
        &self.provider_filter_fields
    }

    #[must_use]
    pub fn provider_statistics_fields(&self) -> &[usize] {
        &self.provider_statistics_fields
    }
}

/// Protocol and physical-feature posture observed from the exact snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDeltaReadFeaturePosture {
    min_reader_version: i32,
    min_writer_version: i32,
    reader_features: Arc<[String]>,
    writer_features: Arc<[String]>,
    column_mapping_mode: ColumnMappingMode,
    deletion_vectors_declared: bool,
    deletion_vector_behavior: DeletionVectorBehavior,
}

impl ExactDeltaReadFeaturePosture {
    #[must_use]
    pub const fn min_reader_version(&self) -> i32 {
        self.min_reader_version
    }

    #[must_use]
    pub const fn min_writer_version(&self) -> i32 {
        self.min_writer_version
    }

    #[must_use]
    pub fn reader_features(&self) -> &[String] {
        &self.reader_features
    }

    #[must_use]
    pub fn writer_features(&self) -> &[String] {
        &self.writer_features
    }

    #[must_use]
    pub const fn column_mapping_mode(&self) -> ColumnMappingMode {
        self.column_mapping_mode
    }

    #[must_use]
    pub const fn deletion_vectors_declared(&self) -> bool {
        self.deletion_vectors_declared
    }

    #[must_use]
    pub const fn deletion_vector_behavior(&self) -> DeletionVectorBehavior {
        self.deletion_vector_behavior
    }
}

/// Prepared exact read whose executable plan retains every required residual.
pub struct PreparedExactDeltaSemanticRead {
    pin: ExactDeltaPin,
    epoch_session: Arc<SessionState>,
    physical_plan: Arc<dyn ExecutionPlan>,
    output_schema: SchemaRef,
    index_bindings: ExactDeltaReadIndexBindings,
    feature_posture: ExactDeltaReadFeaturePosture,
    delta_statistics: ExactDeltaStatisticsInspection,
    native_scan_statistics: Arc<Statistics>,
    semantic_plan_statistics: Arc<Statistics>,
    filter_pushdown: Arc<[TableProviderFilterPushDown]>,
}

impl fmt::Debug for PreparedExactDeltaSemanticRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedExactDeltaSemanticRead")
            .field("pin", &self.pin)
            .field("output_schema", &self.output_schema)
            .field("index_bindings", &self.index_bindings)
            .field("feature_posture", &self.feature_posture)
            .field("delta_statistics", &self.delta_statistics)
            .field("native_scan_statistics", &self.native_scan_statistics)
            .field("semantic_plan_statistics", &self.semantic_plan_statistics)
            .field("filter_pushdown", &self.filter_pushdown)
            .finish_non_exhaustive()
    }
}

impl PreparedExactDeltaSemanticRead {
    #[must_use]
    pub const fn pin(&self) -> &ExactDeltaPin {
        &self.pin
    }

    #[must_use]
    pub const fn output_schema(&self) -> &SchemaRef {
        &self.output_schema
    }

    #[must_use]
    pub const fn index_bindings(&self) -> &ExactDeltaReadIndexBindings {
        &self.index_bindings
    }

    #[must_use]
    pub const fn feature_posture(&self) -> &ExactDeltaReadFeaturePosture {
        &self.feature_posture
    }

    /// Complete exact-snapshot `add` statistics and provider-level posture.
    #[must_use]
    pub const fn delta_statistics(&self) -> &ExactDeltaStatisticsInspection {
        &self.delta_statistics
    }

    /// Statistics computed through delta-rs's native physical scan.
    ///
    /// These retain transaction-log file/column evidence where available.
    #[must_use]
    pub const fn native_scan_statistics(&self) -> &Arc<Statistics> {
        &self.native_scan_statistics
    }

    /// Statistics computed for the final residual-correct semantic plan.
    #[must_use]
    pub const fn semantic_plan_statistics(&self) -> &Arc<Statistics> {
        &self.semantic_plan_statistics
    }

    /// delta-rs pushdown classification aligned one-for-one with request filters.
    #[must_use]
    pub fn filter_pushdown(&self) -> &[TableProviderFilterPushDown] {
        &self.filter_pushdown
    }

    /// Prepared DataFusion plan exposed for inspection, not rebinding.
    #[must_use]
    pub const fn physical_plan(&self) -> &Arc<dyn ExecutionPlan> {
        &self.physical_plan
    }

    /// Execute in the same epoch session which built the exact Delta provider.
    pub async fn execute(self) -> Result<Vec<RecordBatch>, ExactDeltaSemanticReadError> {
        Ok(collect(self.physical_plan, self.epoch_session.task_ctx()).await?)
    }
}

/// Failure to prepare or execute one exact Delta semantic read.
#[derive(Debug, Error)]
pub enum ExactDeltaSemanticReadError {
    #[error(transparent)]
    ExactProvider(#[from] ExactDeltaProviderError),
    #[error(transparent)]
    SchemaContract(#[from] SchemaContractError),
    #[error(transparent)]
    ProviderContract(#[from] ProviderContractError),
    #[error(transparent)]
    DataFusion(#[from] DataFusionError),
    #[error("loaded Delta table has no exact snapshot: {0}")]
    DeltaSnapshot(#[from] DeltaTableError),
    #[error("exact Delta protocol is not readable by the pinned delta-rs provider: {source}")]
    UnsupportedProtocol {
        #[source]
        source: TransactionError,
    },
    #[error("unknown Delta column-mapping mode {observed:?}")]
    UnknownColumnMappingMode { observed: String },
    #[error(
        "Delta column-mapping posture mismatch: contract requires {expected:?}, exact snapshot declares {observed:?}"
    )]
    ColumnMappingMismatch {
        expected: ColumnMappingMode,
        observed: ColumnMappingMode,
    },
    #[error("exact Delta snapshot declares deletion vectors but the schema contract forbids them")]
    DeletionVectorsForbidden,
    #[error(
        "schema contract requires an exposed deletion-vector visibility column, but delta-rs applies deletion vectors inside its provider"
    )]
    ExposedDeletionVectorVisibilityUnsupported,
    #[error("filter column {column:?} must be unqualified and owned by the schema contract")]
    InvalidFilterColumn { column: String },
    #[error(
        "provider {purpose} index {index} is outside the exact Delta provider schema with {field_count} fields"
    )]
    ProviderIndexOutOfBounds {
        purpose: &'static str,
        index: usize,
        field_count: usize,
    },
    #[error("exact semantic read schema mismatch: expected {expected}, observed {observed}")]
    OutputSchemaMismatch { expected: String, observed: String },
}

/// Prepare a residual-correct semantic read from a table already loaded at an
/// application-selected exact version.
///
/// This function never calls `load`, `update`, `get_latest_version`, or any
/// other current-state discovery API. The loaded table must match `request.pin`.
/// The returned execution plan originates solely from delta-rs's exact provider.
pub async fn prepare_exact_delta_semantic_read(
    loaded_exact_table: DeltaTable,
    request: ExactDeltaSemanticReadRequest,
    contract: Arc<SchemaContract>,
    epoch_session: Arc<SessionState>,
) -> Result<PreparedExactDeltaSemanticRead, ExactDeltaSemanticReadError> {
    let snapshot = loaded_exact_table.snapshot()?;
    let feature_posture = validate_feature_posture(snapshot, contract.as_ref())?;
    let validated =
        ValidatedDeltaSnapshot::try_from_loaded_table(loaded_exact_table, request.pin())?;
    let exact_read =
        provider_read_from_validated_snapshot(request.pin(), validated, Arc::clone(&epoch_session))
            .await?;
    let (storage_provider, delta_statistics) = exact_read.into_parts();
    let provider = Arc::new(SchemaContractStorageProvider::try_new(
        Arc::clone(&contract),
        storage_provider,
    )?) as Arc<dyn datafusion::catalog::TableProvider>;

    contract.validate_arrow_schema(
        SchemaPhase::ProviderIngress,
        SchemaRole::Logical,
        provider.schema().as_ref(),
        contract.compatibility(),
    )?;

    let logical_projection = request.logical_projection().map_or_else(
        || (0..contract.logical_schema().fields().len()).collect::<Vec<_>>(),
        <[usize]>::to_vec,
    );
    let provider_projection = logical_projection.clone();
    validate_provider_indices(
        "projection",
        &provider_projection,
        provider.schema().fields().len(),
    )?;

    let logical_filter_fields = logical_filter_fields(&request, contract.as_ref())?;
    let provider_filter_fields = logical_filter_fields.clone();
    validate_provider_indices(
        "filter",
        &provider_filter_fields,
        provider.schema().fields().len(),
    )?;

    let provider_statistics_fields = logical_projection.clone();
    validate_provider_indices(
        "statistics",
        &provider_statistics_fields,
        provider.schema().fields().len(),
    )?;

    let filter_refs = request.filters().iter().collect::<Vec<_>>();
    let filter_pushdown: Arc<[TableProviderFilterPushDown]> =
        provider.supports_filters_pushdown(&filter_refs)?.into();
    let pushed_filters = request
        .filters()
        .iter()
        .zip(filter_pushdown.iter())
        .filter(|(_, posture)| !matches!(posture, TableProviderFilterPushDown::Unsupported))
        .map(|(filter, _)| filter.clone())
        .collect::<Vec<_>>();

    // This scan is evidence-only. It is never exposed for execution because an
    // `Inexact` pushdown may only skip files. The logical plan below retains the
    // residual predicate and is the sole executable plan.
    let native_scan = provider
        .scan(
            epoch_session.as_ref(),
            Some(&provider_projection),
            &pushed_filters,
            None,
        )
        .await?;
    let native_scan_statistics =
        StatisticsContext::new().compute(native_scan.as_ref(), &StatisticsArgs::new())?;

    let mut builder = LogicalPlanBuilder::scan(
        EXACT_READ_SOURCE,
        provider_as_source(Arc::clone(&provider)),
        None,
    )?;
    for filter in request.filters() {
        builder = builder.filter(filter.clone())?;
    }
    let projection = logical_projection
        .iter()
        .map(|index| {
            Expr::Column(Column::new_unqualified(
                contract.logical_schema().field(*index).name().clone(),
            ))
        })
        .collect::<Vec<_>>();
    builder = builder.project(projection)?;
    if let Some(limit) = request.limit() {
        builder = builder.limit(0, Some(limit))?;
    }
    let logical = builder.build()?;
    let optimized = epoch_session.optimize(&logical)?;
    let physical_plan = epoch_session.create_physical_plan(&optimized).await?;
    let expected_schema = contract.project_logical_schema(&logical_projection)?;
    if physical_plan.schema().as_ref() != expected_schema.as_ref() {
        return Err(ExactDeltaSemanticReadError::OutputSchemaMismatch {
            expected: format!("{:?}", expected_schema),
            observed: format!("{:?}", physical_plan.schema()),
        });
    }
    let semantic_plan_statistics =
        StatisticsContext::new().compute(physical_plan.as_ref(), &StatisticsArgs::new())?;

    Ok(PreparedExactDeltaSemanticRead {
        pin: request.pin,
        epoch_session,
        physical_plan,
        output_schema: expected_schema,
        index_bindings: ExactDeltaReadIndexBindings {
            logical_projection: logical_projection.into(),
            provider_projection: provider_projection.into(),
            logical_filter_fields: logical_filter_fields.into(),
            provider_filter_fields: provider_filter_fields.into(),
            provider_statistics_fields: provider_statistics_fields.into(),
        },
        feature_posture,
        delta_statistics,
        native_scan_statistics,
        semantic_plan_statistics,
        filter_pushdown,
    })
}

fn validate_feature_posture(
    snapshot: &deltalake::table::state::DeltaTableState,
    contract: &SchemaContract,
) -> Result<ExactDeltaReadFeaturePosture, ExactDeltaSemanticReadError> {
    let protocol = snapshot.protocol();
    let mut reader_features = protocol
        .reader_features()
        .unwrap_or_default()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    reader_features.sort();
    reader_features.dedup();
    let mut writer_features = protocol
        .writer_features()
        .unwrap_or_default()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    writer_features.sort();
    writer_features.dedup();

    let configuration = snapshot.metadata().configuration();
    let observed_column_mapping = match configuration
        .get(COLUMN_MAPPING_MODE_KEY)
        .map(String::as_str)
        .unwrap_or("none")
    {
        "" | "none" => ColumnMappingMode::Positional,
        "name" => ColumnMappingMode::Name,
        "id" => ColumnMappingMode::FieldId,
        observed => {
            return Err(ExactDeltaSemanticReadError::UnknownColumnMappingMode {
                observed: observed.to_owned(),
            });
        }
    };
    if observed_column_mapping != contract.column_mapping_mode() {
        return Err(ExactDeltaSemanticReadError::ColumnMappingMismatch {
            expected: contract.column_mapping_mode(),
            observed: observed_column_mapping,
        });
    }

    if let Err(source) = PROTOCOL.can_read_from(snapshot.snapshot()) {
        let delta_provider_column_mapping_exception = matches!(
            &source,
            TransactionError::UnsupportedTableFeatures(features)
                if observed_column_mapping != ColumnMappingMode::Positional
                    && features.len() == 1
                    && features[0].to_string() == COLUMN_MAPPING_FEATURE
        );
        if !delta_provider_column_mapping_exception {
            return Err(ExactDeltaSemanticReadError::UnsupportedProtocol { source });
        }
    }

    let deletion_vectors_declared = reader_features
        .iter()
        .chain(writer_features.iter())
        .any(|feature| feature == DELETION_VECTORS_FEATURE)
        || configuration
            .get(DELETION_VECTORS_KEY)
            .is_some_and(|value| value == "true");
    match contract.deletion_vector_behavior() {
        DeletionVectorBehavior::Forbidden if deletion_vectors_declared => {
            return Err(ExactDeltaSemanticReadError::DeletionVectorsForbidden);
        }
        DeletionVectorBehavior::ExposedVisibilityColumn => {
            return Err(ExactDeltaSemanticReadError::ExposedDeletionVectorVisibilityUnsupported);
        }
        DeletionVectorBehavior::Forbidden | DeletionVectorBehavior::AppliedByProvider => {}
    }

    Ok(ExactDeltaReadFeaturePosture {
        min_reader_version: protocol.min_reader_version(),
        min_writer_version: protocol.min_writer_version(),
        reader_features: reader_features.into(),
        writer_features: writer_features.into(),
        column_mapping_mode: observed_column_mapping,
        deletion_vectors_declared,
        deletion_vector_behavior: contract.deletion_vector_behavior(),
    })
}

fn logical_filter_fields(
    request: &ExactDeltaSemanticReadRequest,
    contract: &SchemaContract,
) -> Result<Vec<usize>, ExactDeltaSemanticReadError> {
    let mut logical_indices = BTreeSet::new();
    for filter in request.filters() {
        for column in filter.column_refs() {
            if column.relation.is_some() {
                return Err(ExactDeltaSemanticReadError::InvalidFilterColumn {
                    column: column.to_string(),
                });
            }
            let index = contract
                .logical_schema()
                .index_of(&column.name)
                .map_err(|_| ExactDeltaSemanticReadError::InvalidFilterColumn {
                    column: column.to_string(),
                })?;
            logical_indices.insert(index);
        }
    }
    Ok(logical_indices.into_iter().collect())
}

fn validate_provider_indices(
    purpose: &'static str,
    indices: &[usize],
    field_count: usize,
) -> Result<(), ExactDeltaSemanticReadError> {
    if let Some(index) = indices.iter().copied().find(|index| *index >= field_count) {
        return Err(ExactDeltaSemanticReadError::ProviderIndexOutOfBounds {
            purpose,
            index,
            field_count,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use arrow_array::{Array, Int64Array, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::common::stats::Precision;
    use datafusion::common::{ScalarValue, TableReference};
    use datafusion::prelude::{SessionConfig, SessionContext, col, lit};
    use deltalake::DeltaTableBuilder;
    use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
    use deltalake::operations::create::CreateBuilder;
    use deltalake::protocol::SaveMode;
    use serde_json::Value;
    use tempfile::TempDir;
    use url::Url;

    use super::*;
    use crate::schema_contract::FieldIndexMapping;

    struct Fixture {
        _temporary: TempDir,
        root: Url,
        schema: SchemaRef,
        version_zero: DeltaTable,
    }

    async fn fixture() -> Fixture {
        let temporary = TempDir::new().expect("temporary exact semantic-read root");
        let table_path = temporary.path().join("table");
        fs::create_dir_all(&table_path).expect("create exact semantic-read table directory");
        let root = Url::from_directory_path(&table_path).expect("fixture file URL");
        let schema = Arc::new(Schema::new(vec![
            Field::new("value", DataType::Int64, true),
            Field::new("label", DataType::Utf8, true),
        ]));
        let kernel: deltalake::kernel::StructType = schema
            .as_ref()
            .try_into_kernel()
            .expect("Arrow fixture schema converts to Delta");
        let version_zero = CreateBuilder::new()
            .with_location(root.to_string())
            .with_table_name("exact_delta_semantic_read_fixture")
            .with_save_mode(SaveMode::ErrorIfExists)
            .with_columns(kernel.fields().cloned())
            .await
            .expect("create exact semantic-read Delta table");
        assert_eq!(version_zero.version(), Some(0));
        Fixture {
            _temporary: temporary,
            root,
            schema,
            version_zero,
        }
    }

    fn batch(values: Vec<Option<i64>>, labels: Vec<Option<&str>>) -> RecordBatch {
        RecordBatch::try_new(
            Arc::new(Schema::new(vec![
                Field::new("value", DataType::Int64, true),
                Field::new("label", DataType::Utf8, true),
            ])),
            vec![
                Arc::new(Int64Array::from(values)),
                Arc::new(StringArray::from(labels)),
            ],
        )
        .expect("semantic-read fixture batch")
    }

    async fn append(table: &DeltaTable, batch: RecordBatch) -> DeltaTable {
        table
            .clone()
            .write(vec![batch])
            .await
            .expect("append exact semantic-read fixture")
    }

    fn epoch_session() -> Arc<SessionState> {
        let config = SessionConfig::new()
            .set_bool(
                "datafusion.execution.parquet.schema_force_view_types",
                false,
            )
            .set_bool("datafusion.execution.parquet.pushdown_filters", true);
        Arc::new(SessionContext::new_with_config(config).state())
    }

    fn contract(schema: &SchemaRef) -> Arc<SchemaContract> {
        Arc::new(
            SchemaContract::try_new(
                "delta.semantic-read.fixture.v1",
                TableReference::bare(EXACT_READ_SOURCE),
                Arc::clone(schema),
                Arc::clone(schema),
                (0..schema.fields().len())
                    .map(|index| FieldIndexMapping::direct(index, index))
                    .collect(),
            )
            .expect("exact semantic-read schema contract"),
        )
    }

    fn scalar(batch: &RecordBatch, name: &str, row: usize) -> ScalarValue {
        let array = batch
            .column_by_name(name)
            .unwrap_or_else(|| panic!("missing add-action statistic {name}"));
        ScalarValue::try_from_array(array, row).expect("read add-action statistic scalar")
    }

    fn rewrite_log_action(root: &Url, version: u64, mut rewrite: impl FnMut(&mut Value)) {
        let path = root
            .to_file_path()
            .expect("fixture root is a local path")
            .join("_delta_log")
            .join(format!("{version:020}.json"));
        let input = fs::read_to_string(&path).expect("read Delta JSON commit");
        let output = input
            .lines()
            .map(|line| {
                let mut action: Value = serde_json::from_str(line).expect("decode Delta action");
                rewrite(&mut action);
                serde_json::to_string(&action).expect("encode Delta action")
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, format!("{output}\n")).expect("rewrite Delta JSON commit");
    }

    async fn reopen(root: &Url, version: u64) -> DeltaTable {
        DeltaTableBuilder::from_url(root.clone())
            .expect("construct exact Delta table builder")
            .with_version(version)
            .load()
            .await
            .expect("reopen exact Delta version")
    }

    #[tokio::test]
    async fn exact_read_preserves_known_delta_row_null_min_and_max_statistics() {
        let fixture = fixture().await;
        let version_one = append(
            &fixture.version_zero,
            batch(
                vec![Some(1), None, Some(9)],
                vec![Some("one"), Some("missing"), Some("nine")],
            ),
        )
        .await;
        let pin = ExactDeltaPin::new(&fixture.root, 1).expect("exact version-one pin");
        let prepared = prepare_exact_delta_semantic_read(
            version_one,
            ExactDeltaSemanticReadRequest::new(
                pin,
                Some(vec![0]),
                vec![col("value").is_not_null()],
                None,
            ),
            contract(&fixture.schema),
            epoch_session(),
        )
        .await
        .expect("prepare exact semantic read with statistics");

        let add = prepared.delta_statistics().add_actions();
        assert_eq!(add.num_rows(), 1);
        assert_eq!(scalar(add, "num_records", 0), ScalarValue::Int64(Some(3)));
        assert_eq!(
            scalar(add, "null_count.value", 0),
            ScalarValue::Int64(Some(1))
        );
        assert_eq!(scalar(add, "min.value", 0), ScalarValue::Int64(Some(1)));
        assert_eq!(scalar(add, "max.value", 0), ScalarValue::Int64(Some(9)));

        let native = prepared.native_scan_statistics();
        assert_eq!(native.num_rows.get_value(), Some(&3));
        let value = native
            .column_statistics
            .first()
            .expect("projected value statistics");
        assert_eq!(
            value.min_value.get_value(),
            Some(&ScalarValue::Int64(Some(1)))
        );
        assert_eq!(
            value.max_value.get_value(),
            Some(&ScalarValue::Int64(Some(9)))
        );
        assert_eq!(value.null_count.get_value(), Some(&1));
        assert_eq!(
            prepared.feature_posture().column_mapping_mode(),
            ColumnMappingMode::Positional
        );
        assert!(!prepared.feature_posture().deletion_vectors_declared());

        let batches = prepared.execute().await.expect("execute semantic read");
        let values = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .expect("projected Int64 values")
                    .iter()
            })
            .collect::<Vec<_>>();
        assert_eq!(values, vec![Some(1), Some(9)]);
    }

    #[tokio::test]
    async fn projection_and_inexact_filter_keep_residual_semantics_and_native_statistics() {
        let fixture = fixture().await;
        let version_one = append(
            &fixture.version_zero,
            batch(vec![Some(1), Some(2)], vec![Some("one"), Some("two")]),
        )
        .await;
        let version_two = append(
            &version_one,
            batch(
                vec![Some(100), Some(200)],
                vec![Some("hundred"), Some("two-hundred")],
            ),
        )
        .await;
        let pin = ExactDeltaPin::new(&fixture.root, 2).expect("exact version-two pin");
        let prepared = prepare_exact_delta_semantic_read(
            version_two,
            ExactDeltaSemanticReadRequest::new(
                pin,
                Some(vec![0]),
                vec![col("value").gt(lit(50_i64))],
                None,
            ),
            contract(&fixture.schema),
            epoch_session(),
        )
        .await
        .expect("prepare projected exact semantic read");

        assert_eq!(
            prepared.filter_pushdown(),
            &[TableProviderFilterPushDown::Inexact]
        );
        assert_eq!(prepared.index_bindings().logical_projection(), &[0]);
        assert_eq!(prepared.index_bindings().provider_projection(), &[0]);
        assert_eq!(prepared.index_bindings().logical_filter_fields(), &[0]);
        let native = prepared.native_scan_statistics();
        assert_eq!(native.num_rows.get_value(), Some(&2));
        assert_eq!(
            native.column_statistics[0].min_value.get_value(),
            Some(&ScalarValue::Int64(Some(100)))
        );
        assert_eq!(
            native.column_statistics[0].max_value.get_value(),
            Some(&ScalarValue::Int64(Some(200)))
        );
        let semantic = prepared.semantic_plan_statistics();
        assert_eq!(semantic.column_statistics.len(), 1);
        assert_eq!(
            semantic.column_statistics[0].min_value.get_value(),
            Some(&ScalarValue::Int64(Some(100)))
        );
        assert_eq!(
            semantic.column_statistics[0].max_value.get_value(),
            Some(&ScalarValue::Int64(Some(200)))
        );
        assert!(semantic.num_rows.get_value().is_some_and(|rows| *rows > 0));

        let batches = prepared.execute().await.expect("execute projected read");
        assert!(batches.iter().all(|batch| batch.num_columns() == 1));
        let values = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .expect("projected Int64 values")
                    .iter()
            })
            .collect::<Vec<_>>();
        assert_eq!(values, vec![Some(100), Some(200)]);
    }

    #[tokio::test]
    async fn missing_file_statistics_remain_absent_and_never_become_zero_cardinality() {
        let fixture = fixture().await;
        let _version_one = append(
            &fixture.version_zero,
            batch(vec![Some(7), Some(8)], vec![Some("seven"), Some("eight")]),
        )
        .await;
        rewrite_log_action(&fixture.root, 1, |action| {
            if let Some(add) = action.get_mut("add").and_then(Value::as_object_mut) {
                add.remove("stats");
            }
        });
        let reopened = reopen(&fixture.root, 1).await;
        let pin = ExactDeltaPin::new(&fixture.root, 1).expect("exact version-one pin");
        let prepared = prepare_exact_delta_semantic_read(
            reopened,
            ExactDeltaSemanticReadRequest::new(pin, Some(vec![0]), Vec::new(), None),
            contract(&fixture.schema),
            epoch_session(),
        )
        .await
        .expect("prepare exact read without Delta statistics");

        assert!(matches!(
            prepared
                .delta_statistics()
                .field("num_records")
                .expect("row-count availability")
                .availability(),
            super::super::delta_exact::ExactDeltaStatisticAvailability::UnknownForFiles {
                file_count: 1,
                unknown_file_count: 1
            }
        ));
        let native = prepared.native_scan_statistics();
        assert_eq!(native.num_rows, Precision::Absent);
        assert_eq!(native.column_statistics[0].null_count, Precision::Absent);
        assert_eq!(native.column_statistics[0].min_value, Precision::Absent);
        assert_eq!(native.column_statistics[0].max_value, Precision::Absent);
        assert!(!matches!(
            native.num_rows,
            Precision::Exact(0) | Precision::Inexact(0)
        ));
        let semantic = prepared.semantic_plan_statistics();
        assert_eq!(semantic.num_rows, Precision::Absent);
        assert_eq!(semantic.column_statistics[0].min_value, Precision::Absent);
        assert_eq!(semantic.column_statistics[0].max_value, Precision::Absent);

        let batches = prepared.execute().await.expect("execute stats-free read");
        assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 2);
    }

    #[tokio::test]
    async fn column_mapping_metadata_must_match_the_application_schema_contract() {
        let fixture = fixture().await;
        rewrite_log_action(&fixture.root, 0, |action| {
            if let Some(configuration) = action
                .get_mut("metaData")
                .and_then(|metadata| metadata.get_mut("configuration"))
                .and_then(Value::as_object_mut)
            {
                configuration.insert(
                    COLUMN_MAPPING_MODE_KEY.to_owned(),
                    Value::String("name".to_owned()),
                );
            }
        });
        let reopened = reopen(&fixture.root, 0).await;
        let pin = ExactDeltaPin::new(&fixture.root, 0).expect("exact version-zero pin");
        let error = prepare_exact_delta_semantic_read(
            reopened,
            ExactDeltaSemanticReadRequest::new(pin, None, Vec::new(), None),
            contract(&fixture.schema),
            epoch_session(),
        )
        .await
        .expect_err("column-mapping mismatch must reject before scanning");
        assert!(matches!(
            error,
            ExactDeltaSemanticReadError::ColumnMappingMismatch {
                expected: ColumnMappingMode::Positional,
                observed: ColumnMappingMode::Name
            }
        ));
    }

    #[tokio::test]
    async fn unknown_reader_feature_is_rejected_while_reopening_the_exact_delta_version() {
        let fixture = fixture().await;
        rewrite_log_action(&fixture.root, 0, |action| {
            if let Some(protocol) = action.get_mut("protocol").and_then(Value::as_object_mut) {
                protocol.insert("minReaderVersion".to_owned(), Value::from(3));
                protocol.insert("minWriterVersion".to_owned(), Value::from(7));
                protocol.insert(
                    "readerFeatures".to_owned(),
                    Value::Array(vec![Value::String(
                        "codefabricUnknownReaderFeature".to_owned(),
                    )]),
                );
                protocol.insert(
                    "writerFeatures".to_owned(),
                    Value::Array(vec![Value::String(
                        "codefabricUnknownReaderFeature".to_owned(),
                    )]),
                );
            }
        });
        let error = DeltaTableBuilder::from_url(fixture.root.clone())
            .expect("construct exact Delta table builder")
            .with_version(0)
            .load()
            .await
            .expect_err("unknown reader feature must reject exact-version reconstruction");
        assert!(error.to_string().contains("codefabricUnknownReaderFeature"));
    }

    #[tokio::test]
    async fn declared_deletion_vectors_are_rejected_when_the_contract_forbids_them() {
        let fixture = fixture().await;
        rewrite_log_action(&fixture.root, 0, |action| {
            if let Some(configuration) = action
                .get_mut("metaData")
                .and_then(|metadata| metadata.get_mut("configuration"))
                .and_then(Value::as_object_mut)
            {
                configuration.insert(
                    DELETION_VECTORS_KEY.to_owned(),
                    Value::String("true".to_owned()),
                );
            }
        });
        let reopened = reopen(&fixture.root, 0).await;
        let pin = ExactDeltaPin::new(&fixture.root, 0).expect("exact version-zero pin");
        let error = prepare_exact_delta_semantic_read(
            reopened,
            ExactDeltaSemanticReadRequest::new(pin, None, Vec::new(), None),
            contract(&fixture.schema),
            epoch_session(),
        )
        .await
        .expect_err("forbidden deletion-vector declaration must reject exact read");
        assert!(matches!(
            error,
            ExactDeltaSemanticReadError::DeletionVectorsForbidden
        ));
    }

    #[tokio::test]
    async fn semantic_read_stays_on_an_older_exact_version_after_a_newer_commit_exists() {
        let fixture = fixture().await;
        let version_one = append(
            &fixture.version_zero,
            batch(vec![Some(11)], vec![Some("version-one")]),
        )
        .await;
        let _version_two = append(
            &version_one,
            batch(vec![Some(22)], vec![Some("version-two")]),
        )
        .await;
        let reopened_version_one = reopen(&fixture.root, 1).await;
        let pin = ExactDeltaPin::new(&fixture.root, 1).expect("older exact version pin");
        let prepared = prepare_exact_delta_semantic_read(
            reopened_version_one,
            ExactDeltaSemanticReadRequest::new(pin, Some(vec![0]), Vec::new(), None),
            contract(&fixture.schema),
            epoch_session(),
        )
        .await
        .expect("prepare older exact semantic read");

        assert_eq!(prepared.pin().version(), 1);
        let batches = prepared.execute().await.expect("execute older exact read");
        let values = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .expect("projected Int64 values")
                    .iter()
            })
            .collect::<Vec<_>>();
        assert_eq!(values, vec![Some(11)]);
    }
}
