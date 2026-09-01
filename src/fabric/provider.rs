//! Immutable, schema-bound delegation for honest DataFusion table providers.
//!
//! [`SchemaContractTableProvider`] is intentionally a transparent adapter: it
//! preserves a native provider's scan plan and metadata claims, while adding
//! the application-owned schema checks and structured observations owned by the
//! fabric. It does not synthesize pushdown, constraints, statistics, ordering,
//! partitioning, defaults, or table definitions.

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use async_trait::async_trait;
use datafusion::catalog::{ScanArgs, ScanResult, Session, TableProvider};
use datafusion::common::tree_node::{Transformed, TreeNode};
use datafusion::common::{Constraints, DataFusionError, ScalarValue, Statistics};
use datafusion::logical_expr::statistics::StatisticsRequest;
use datafusion::logical_expr::{Expr, LogicalPlan, TableProviderFilterPushDown, TableType};
use datafusion::physical_expr::expressions::{cast, col as physical_col};
use datafusion::physical_plan::ExecutionPlan;
use datafusion::physical_plan::projection::{ProjectionExec, ProjectionExpr};
use datafusion::prelude::SessionContext;

use crate::schema_contract::{
    SchemaCompatibility, SchemaContract, SchemaContractError, SchemaPhase, SchemaRole,
};

/// Exact-provider read request admitted by the internal DataFusion adapter.
///
/// The closed request shape prevents fabric callers from acquiring a raw context, optimizer,
/// planner, or physical plan merely to inspect an exact-version provider.
pub(crate) struct ProviderReadRequest {
    pub provider: Arc<dyn TableProvider>,
    pub filter: Option<Expr>,
    pub projection: Option<Vec<Expr>>,
    pub limit: Option<usize>,
}

/// Collect one exact-provider read without exposing DataFusion session or action APIs.
pub(crate) async fn collect_provider(
    request: ProviderReadRequest,
) -> Result<Vec<RecordBatch>, DataFusionError> {
    let context = SessionContext::new();
    let mut frame = context.read_table(request.provider)?;
    if let Some(filter) = request.filter {
        frame = frame.filter(filter)?;
    }
    if let Some(projection) = request.projection {
        frame = frame.select(projection)?;
    }
    if let Some(limit) = request.limit {
        frame = frame.limit(0, Some(limit))?;
    }
    frame.collect().await
}

/// Lossless planning arguments observed at the schema-bound provider boundary.
///
/// The observation is deterministic and contains no inferred capability or
/// result claim. A caller that persists it is responsible for governing any
/// literals carried by the cloned filter expressions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderScanObservation {
    source_schema_identity: String,
    projection: Option<Vec<usize>>,
    filters: Option<Vec<Expr>>,
    limit: Option<usize>,
    statistics_requests: Vec<StatisticsRequest>,
}

impl ProviderScanObservation {
    fn from_args(contract: &SchemaContract, args: &ScanArgs<'_>) -> Self {
        Self {
            source_schema_identity: contract.source_schema_identity().to_owned(),
            projection: args.projection().map(<[usize]>::to_vec),
            filters: args.filters().map(<[Expr]>::to_vec),
            limit: args.limit(),
            statistics_requests: args.statistics_requests().to_vec(),
        }
    }

    #[must_use]
    pub fn source_schema_identity(&self) -> &str {
        &self.source_schema_identity
    }

    #[must_use]
    pub fn projection(&self) -> Option<&[usize]> {
        self.projection.as_deref()
    }

    #[must_use]
    pub fn filters(&self) -> Option<&[Expr]> {
        self.filters.as_deref()
    }

    #[must_use]
    pub const fn limit(&self) -> Option<usize> {
        self.limit
    }

    #[must_use]
    pub fn statistics_requests(&self) -> &[StatisticsRequest] {
        &self.statistics_requests
    }
}

/// Destination for derived provider-boundary scan observations.
///
/// The default adapter uses a disabled sink so normal planning pays no cloning
/// cost. An epoch-owned system-relation observer can opt in explicitly.
pub trait ProviderScanObservationSink: fmt::Debug + Send + Sync {
    /// Whether the adapter should snapshot structured arguments for this sink.
    fn is_enabled(&self) -> bool {
        true
    }

    /// Record one received structured scan without changing scan semantics.
    fn record(&self, observation: ProviderScanObservation);
}

#[derive(Debug, Default)]
struct DisabledProviderScanObservationSink;

impl ProviderScanObservationSink for DisabledProviderScanObservationSink {
    fn is_enabled(&self) -> bool {
        false
    }

    fn record(&self, _observation: ProviderScanObservation) {}
}

/// Typed failures introduced by the schema-bound adapter.
#[derive(Debug, thiserror::Error)]
pub enum ProviderContractError {
    #[error("the wrapped provider violated its schema contract: {source}")]
    SchemaContract { source: Box<SchemaContractError> },
    #[error(
        "the native scan plan schema drifted from the contract projection; expected {expected:?}, actual {actual:?}"
    )]
    PlanSchemaDrift {
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("the native provider storage schema differs from the contract: {detail}")]
    StorageSchemaDrift { detail: String },
    #[error(
        "storage restoration supports only direct, name-preserving fields and equal types or fixed-size-binary-to-binary casts; field {field_name:?} maps {logical_type:?} to {storage_type:?}"
    )]
    UnsupportedStorageRestoration {
        field_name: String,
        logical_type: arrow_schema::DataType,
        storage_type: arrow_schema::DataType,
    },
}

impl From<SchemaContractError> for ProviderContractError {
    fn from(source: SchemaContractError) -> Self {
        Self::SchemaContract {
            source: Box::new(source),
        }
    }
}

/// Native storage provider restored to an exact logical [`SchemaContract`].
///
/// Delta reconstructs physical Arrow schemas from its log and Parquet files.
/// This adapter preserves the native scan/statistics provider while restoring
/// application-owned field metadata and fixed-width identity meaning at the
/// logical boundary. It accepts only direct, name-preserving mapped fields so an
/// optimizer-visible scan cannot silently reorder or synthesize a field. Storage-only envelope
/// fields may remain unmapped; they never enter the provider's logical schema.
pub struct SchemaContractStorageProvider {
    contract: Arc<SchemaContract>,
    inner: Arc<dyn TableProvider>,
}

impl fmt::Debug for SchemaContractStorageProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SchemaContractStorageProvider")
            .field(
                "source_schema_identity",
                &self.contract.source_schema_identity(),
            )
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl SchemaContractStorageProvider {
    /// Bind one exact native storage provider and validate the complete direct
    /// logical/storage mapping before it can enter a candidate catalog.
    pub fn try_new(
        contract: Arc<SchemaContract>,
        inner: Arc<dyn TableProvider>,
    ) -> Result<Self, ProviderContractError> {
        validate_native_storage_schema(contract.storage_schema(), &inner.schema())?;
        for (logical_index, logical) in contract.logical_schema().fields().iter().enumerate() {
            let projection = contract.map_projection(&[logical_index])?[0];
            let filter = contract.map_filter_indices(&[logical_index])?[0];
            let statistics = contract.map_statistics_indices(&[logical_index])?[0];
            let storage = contract.storage_schema().field(projection);
            let direct = filter == projection && statistics == projection;
            let supported_type = logical.data_type() == storage.data_type()
                || matches!(
                    (logical.data_type(), storage.data_type()),
                    (
                        arrow_schema::DataType::FixedSizeBinary(_),
                        arrow_schema::DataType::Binary
                    )
                );
            if !direct || logical.name() != storage.name() || !supported_type {
                return Err(ProviderContractError::UnsupportedStorageRestoration {
                    field_name: logical.name().to_owned(),
                    logical_type: logical.data_type().clone(),
                    storage_type: storage.data_type().clone(),
                });
            }
        }
        Ok(Self { contract, inner })
    }

    fn projected_logical_schema(
        &self,
        projection: Option<&[usize]>,
    ) -> Result<SchemaRef, DataFusionError> {
        projection.map_or_else(
            || Ok(Arc::clone(self.contract.logical_schema())),
            |projection| {
                self.contract
                    .project_logical_schema(projection)
                    .map_err(|error| DataFusionError::External(Box::new(error)))
            },
        )
    }

    fn storage_filter(filter: &Expr) -> datafusion::common::Result<Expr> {
        filter
            .clone()
            .transform_down(|expression| match expression {
                Expr::Literal(ScalarValue::FixedSizeBinary(_, value), metadata) => Ok(
                    Transformed::yes(Expr::Literal(ScalarValue::Binary(value), metadata)),
                ),
                expression => Ok(Transformed::no(expression)),
            })
            .map(|value| value.data)
    }

    fn reattach_logical_schema(
        &self,
        plan: Arc<dyn ExecutionPlan>,
        projection: Option<&[usize]>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        let target = self.projected_logical_schema(projection)?;
        let input = plan.schema();
        if input.fields().len() != target.fields().len() {
            return Err(DataFusionError::Plan(format!(
                "storage plan field count {} differs from logical field count {}",
                input.fields().len(),
                target.fields().len()
            )));
        }
        // DataFusion's physical `ProjectionPushdown` deliberately removes an
        // identity projection. A projection whose only effect is Arrow
        // metadata would therefore be removed and then rejected as a schema
        // metadata mismatch. Keep the native plan when values, names, and
        // ordering already match; `TableProvider::schema` remains the logical
        // metadata boundary, and result/resource boundaries reattach that
        // application-owned metadata. A real storage-to-logical type cast is
        // represented by the projection below and cannot be optimized away.
        let needs_value_projection =
            input
                .fields()
                .iter()
                .zip(target.fields())
                .any(|(storage, logical)| {
                    storage.name() != logical.name() || storage.data_type() != logical.data_type()
                });
        if !needs_value_projection {
            return Ok(plan);
        }
        let expressions = target
            .fields()
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let expression = physical_col(input.field(index).name(), &input)?;
                let expression = if input.field(index).data_type() == field.data_type() {
                    expression
                } else {
                    cast(expression, &input, field.data_type().clone())?
                };
                Ok(ProjectionExpr {
                    expr: expression,
                    alias: field.name().to_owned(),
                })
            })
            .collect::<datafusion::common::Result<Vec<_>>>()?;
        Ok(Arc::new(ProjectionExec::try_new_with_schema_metadata(
            expressions,
            plan,
            &target,
        )?))
    }
}

#[async_trait]
impl TableProvider for SchemaContractStorageProvider {
    fn schema(&self) -> SchemaRef {
        Arc::clone(self.contract.logical_schema())
    }

    fn constraints(&self) -> Option<&Constraints> {
        Some(self.contract.constraints().as_ref())
    }

    fn table_type(&self) -> TableType {
        self.inner.table_type()
    }

    fn get_table_definition(&self) -> Option<&str> {
        self.inner.get_table_definition()
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        let storage_filters = filters
            .iter()
            .map(Self::storage_filter)
            .collect::<datafusion::common::Result<Vec<_>>>()?;
        let logical_projection = projection.cloned().unwrap_or_else(|| {
            (0..self.contract.logical_schema().fields().len()).collect::<Vec<_>>()
        });
        let storage_projection = self
            .contract
            .map_projection(&logical_projection)
            .map_err(|error| DataFusionError::External(Box::new(error)))?;
        let plan = self
            .inner
            .scan(state, Some(&storage_projection), &storage_filters, limit)
            .await?;
        self.reattach_logical_schema(plan, projection.map(Vec::as_slice))
    }

    async fn scan_with_args<'a>(
        &self,
        state: &dyn Session,
        args: ScanArgs<'a>,
    ) -> datafusion::common::Result<ScanResult> {
        let storage_filters = args
            .filters()
            .map(|filters| {
                filters
                    .iter()
                    .map(Self::storage_filter)
                    .collect::<datafusion::common::Result<Vec<_>>>()
            })
            .transpose()?;
        let logical_projection = args.projection().map_or_else(
            || (0..self.contract.logical_schema().fields().len()).collect::<Vec<_>>(),
            <[usize]>::to_vec,
        );
        let storage_projection = self
            .contract
            .map_projection(&logical_projection)
            .map_err(|error| DataFusionError::External(Box::new(error)))?;
        let storage_args = ScanArgs::default()
            .with_projection(Some(storage_projection.as_slice()))
            .with_filters(storage_filters.as_deref())
            .with_limit(args.limit())
            .with_statistics_requests(args.statistics_requests());
        let result = self.inner.scan_with_args(state, storage_args).await?;
        Ok(self
            .reattach_logical_schema(result.into_inner(), args.projection())?
            .into())
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> datafusion::common::Result<Vec<TableProviderFilterPushDown>> {
        let storage_filters = filters
            .iter()
            .map(|filter| Self::storage_filter(filter))
            .collect::<datafusion::common::Result<Vec<_>>>()?;
        self.inner
            .supports_filters_pushdown(&storage_filters.iter().collect::<Vec<_>>())
    }

    fn statistics(&self) -> Option<Statistics> {
        let statistics = self.inner.statistics()?;
        let logical_indices =
            (0..self.contract.logical_schema().fields().len()).collect::<Vec<_>>();
        let storage_indices = self
            .contract
            .map_statistics_indices(&logical_indices)
            .expect("validated schema-contract statistics mappings remain total");
        let Some(column_statistics) = storage_indices
            .iter()
            .map(|index| statistics.column_statistics.get(*index).cloned())
            .collect::<Option<Vec<_>>>()
        else {
            return Some(Statistics::new_unknown(self.contract.logical_schema()));
        };
        Some(Statistics {
            num_rows: statistics.num_rows,
            total_byte_size: statistics.total_byte_size,
            column_statistics,
        })
    }
}

fn validate_native_storage_schema(
    expected: &SchemaRef,
    actual: &SchemaRef,
) -> Result<(), ProviderContractError> {
    if expected.fields().len() != actual.fields().len() {
        return Err(ProviderContractError::StorageSchemaDrift {
            detail: format!(
                "field count {} != {}",
                actual.fields().len(),
                expected.fields().len()
            ),
        });
    }
    for (ordinal, (expected, actual)) in expected.fields().iter().zip(actual.fields()).enumerate() {
        if expected.name() != actual.name()
            || expected.data_type() != actual.data_type()
            || expected.is_nullable() != actual.is_nullable()
            || (!actual.metadata().is_empty() && actual.metadata() != expected.metadata())
        {
            return Err(ProviderContractError::StorageSchemaDrift {
                detail: format!(
                    "field {ordinal} differs: expected={expected:?}, actual={actual:?}"
                ),
            });
        }
    }
    if !actual.metadata().is_empty() && actual.metadata() != expected.metadata() {
        return Err(ProviderContractError::StorageSchemaDrift {
            detail: "schema metadata differs from the exact storage contract".to_owned(),
        });
    }
    Ok(())
}

/// An immutable `SchemaContract`-bound adapter around one native provider.
///
/// The adapter deliberately does not delegate mutation methods. Its scan path
/// returns the exact native `ExecutionPlan` `Arc`, preserving all native plan
/// properties, including ordering and partitioning, without restating them.
pub struct SchemaContractTableProvider {
    contract: Arc<SchemaContract>,
    inner: Arc<dyn TableProvider>,
    observations: Arc<dyn ProviderScanObservationSink>,
}

impl fmt::Debug for SchemaContractTableProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SchemaContractTableProvider")
            .field(
                "source_schema_identity",
                &self.contract.source_schema_identity(),
            )
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl SchemaContractTableProvider {
    /// Bind an exact native provider to an application-owned schema contract.
    ///
    /// # Errors
    ///
    /// Returns a typed schema-contract error when the provider's declared
    /// schema is not exactly the contract's logical schema.
    pub fn try_new(
        contract: Arc<SchemaContract>,
        inner: Arc<dyn TableProvider>,
    ) -> Result<Self, ProviderContractError> {
        Self::try_new_with_observations(
            contract,
            inner,
            Arc::new(DisabledProviderScanObservationSink),
        )
    }

    /// Bind a provider and an epoch-owned observation sink.
    ///
    /// # Errors
    ///
    /// Returns a typed schema-contract error when the provider's declared
    /// schema is not exactly the contract's logical schema.
    pub fn try_new_with_observations(
        contract: Arc<SchemaContract>,
        inner: Arc<dyn TableProvider>,
        observations: Arc<dyn ProviderScanObservationSink>,
    ) -> Result<Self, ProviderContractError> {
        let provider = Self {
            contract,
            inner,
            observations,
        };
        provider.validate_provider_schema()?;
        Ok(provider)
    }

    #[must_use]
    pub fn source_schema_identity(&self) -> &str {
        self.contract.source_schema_identity()
    }

    async fn plan_scan(
        &self,
        state: &dyn Session,
        args: ScanArgs<'_>,
    ) -> datafusion::common::Result<ScanResult> {
        if self.observations.is_enabled() {
            self.observations
                .record(ProviderScanObservation::from_args(&self.contract, &args));
        }

        self.validate_provider_schema()
            .map_err(provider_datafusion_error)?;
        let expected_schema = self
            .projected_contract_schema(args.projection())
            .map_err(provider_datafusion_error)?;

        // Move the complete structured value into the native provider. This is
        // the load-bearing step that preserves caller-supplied statistics
        // requests instead of using DataFusion's lossy default down-conversion.
        let result = self.inner.scan_with_args(state, args).await?;
        let actual_schema = result.plan().schema();
        if actual_schema.as_ref() != expected_schema.as_ref() {
            return Err(provider_datafusion_error(
                ProviderContractError::PlanSchemaDrift {
                    expected: expected_schema,
                    actual: actual_schema,
                },
            ));
        }

        // Returning the same plan Arc preserves native ordering, partitioning,
        // equivalence properties, statistics duties, and execution behavior.
        Ok(result)
    }

    fn validate_provider_schema(&self) -> Result<(), ProviderContractError> {
        let declared_schema = self.inner.schema();
        self.contract.validate_arrow_schema(
            SchemaPhase::ProviderIngress,
            SchemaRole::Logical,
            declared_schema.as_ref(),
            SchemaCompatibility::Exact,
        )?;
        Ok(())
    }

    fn projected_contract_schema(
        &self,
        projection: Option<&[usize]>,
    ) -> Result<SchemaRef, ProviderContractError> {
        projection.map_or_else(
            || Ok(Arc::clone(self.contract.logical_schema())),
            |projection| {
                self.contract
                    .project_logical_schema(projection)
                    .map_err(ProviderContractError::from)
            },
        )
    }
}

#[async_trait]
impl TableProvider for SchemaContractTableProvider {
    fn schema(&self) -> SchemaRef {
        Arc::clone(self.contract.logical_schema())
    }

    fn constraints(&self) -> Option<&Constraints> {
        self.inner.constraints()
    }

    fn table_type(&self) -> TableType {
        self.inner.table_type()
    }

    fn get_table_definition(&self) -> Option<&str> {
        self.inner.get_table_definition()
    }

    fn get_logical_plan(&'_ self) -> Option<Cow<'_, LogicalPlan>> {
        self.inner.get_logical_plan()
    }

    fn get_column_default(&self, column: &str) -> Option<&Expr> {
        self.inner.get_column_default(column)
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        let args = ScanArgs::default()
            .with_projection(projection.map(Vec::as_slice))
            .with_filters(Some(filters))
            .with_limit(limit);
        Ok(self.plan_scan(state, args).await?.into_inner())
    }

    async fn scan_with_args<'a>(
        &self,
        state: &dyn Session,
        args: ScanArgs<'a>,
    ) -> datafusion::common::Result<ScanResult> {
        self.plan_scan(state, args).await
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> datafusion::common::Result<Vec<TableProviderFilterPushDown>> {
        self.validate_provider_schema()
            .map_err(provider_datafusion_error)?;
        self.inner.supports_filters_pushdown(filters)
    }

    fn statistics(&self) -> Option<Statistics> {
        self.inner.statistics()
    }
}

fn provider_datafusion_error(error: ProviderContractError) -> DataFusionError {
    DataFusionError::External(Box::new(error))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, RwLock};

    use arrow_array::{BinaryArray, FixedSizeBinaryArray, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::common::stats::Precision;
    use datafusion::common::{Column, Constraint};
    use datafusion::datasource::MemTable;
    use datafusion::physical_plan::empty::EmptyExec;
    use datafusion::prelude::{SessionContext, col, lit};

    use super::*;
    use crate::schema_contract::{FieldIndexMapping, SchemaDifferenceKind};

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct CapturedScan {
        projection: Option<Vec<usize>>,
        filters: Option<Vec<Expr>>,
        limit: Option<usize>,
        statistics_requests: Vec<StatisticsRequest>,
    }

    #[derive(Debug)]
    struct StructuredScanSpy {
        schema: RwLock<SchemaRef>,
        plan_schema_override: Mutex<Option<SchemaRef>>,
        constraints: Constraints,
        statistics: Statistics,
        pushdown: Vec<TableProviderFilterPushDown>,
        legacy_scan_calls: AtomicUsize,
        captured: Mutex<Vec<CapturedScan>>,
        last_plan: Mutex<Option<Arc<dyn ExecutionPlan>>>,
    }

    impl StructuredScanSpy {
        fn set_schema(&self, schema: SchemaRef) {
            *self.schema.write().expect("schema lock") = schema;
        }

        fn set_plan_schema_override(&self, schema: SchemaRef) {
            *self.plan_schema_override.lock().expect("plan schema lock") = Some(schema);
        }

        fn captured(&self) -> Vec<CapturedScan> {
            self.captured.lock().expect("capture lock").clone()
        }

        fn last_plan(&self) -> Arc<dyn ExecutionPlan> {
            Arc::clone(
                self.last_plan
                    .lock()
                    .expect("plan lock")
                    .as_ref()
                    .expect("scan plan"),
            )
        }
    }

    #[async_trait]
    impl TableProvider for StructuredScanSpy {
        fn schema(&self) -> SchemaRef {
            Arc::clone(&self.schema.read().expect("schema lock"))
        }

        fn constraints(&self) -> Option<&Constraints> {
            Some(&self.constraints)
        }

        fn table_type(&self) -> TableType {
            TableType::Base
        }

        async fn scan(
            &self,
            _state: &dyn Session,
            _projection: Option<&Vec<usize>>,
            _filters: &[Expr],
            _limit: Option<usize>,
        ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
            self.legacy_scan_calls.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(EmptyExec::new(self.schema())))
        }

        async fn scan_with_args<'a>(
            &self,
            _state: &dyn Session,
            args: ScanArgs<'a>,
        ) -> datafusion::common::Result<ScanResult> {
            self.captured
                .lock()
                .expect("capture lock")
                .push(CapturedScan {
                    projection: args.projection().map(<[usize]>::to_vec),
                    filters: args.filters().map(<[Expr]>::to_vec),
                    limit: args.limit(),
                    statistics_requests: args.statistics_requests().to_vec(),
                });

            let schema_override = self
                .plan_schema_override
                .lock()
                .expect("plan schema lock")
                .clone();
            let plan_schema = match schema_override {
                Some(schema) => schema,
                None => args.projection().map_or_else(
                    || Ok(self.schema()),
                    |projection| {
                        self.schema()
                            .project(projection)
                            .map(Arc::new)
                            .map_err(DataFusionError::from)
                    },
                )?,
            };
            let plan: Arc<dyn ExecutionPlan> = Arc::new(EmptyExec::new(plan_schema));
            *self.last_plan.lock().expect("plan lock") = Some(Arc::clone(&plan));
            Ok(plan.into())
        }

        fn supports_filters_pushdown(
            &self,
            filters: &[&Expr],
        ) -> datafusion::common::Result<Vec<TableProviderFilterPushDown>> {
            if filters.len() != self.pushdown.len() {
                return Err(DataFusionError::Internal(format!(
                    "test spy expected {} filters, received {}",
                    self.pushdown.len(),
                    filters.len()
                )));
            }
            Ok(self.pushdown.clone())
        }

        fn statistics(&self) -> Option<Statistics> {
            Some(self.statistics.clone())
        }
    }

    #[derive(Debug, Default)]
    struct RecordingObservations {
        observations: Mutex<Vec<ProviderScanObservation>>,
    }

    impl RecordingObservations {
        fn snapshot(&self) -> Vec<ProviderScanObservation> {
            self.observations.lock().expect("observation lock").clone()
        }
    }

    impl ProviderScanObservationSink for RecordingObservations {
        fn record(&self, observation: ProviderScanObservation) {
            self.observations
                .lock()
                .expect("observation lock")
                .push(observation);
        }
    }

    fn logical_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Arc::new(Field::new("id", DataType::UInt64, false)),
            Arc::new(Field::new("label", DataType::Utf8, true)),
        ]))
    }

    fn test_fixture() -> (
        Arc<StructuredScanSpy>,
        SchemaContractTableProvider,
        Arc<RecordingObservations>,
    ) {
        let schema = logical_schema();
        let contract = Arc::new(
            SchemaContract::try_new(
                "application.test_relation.v1",
                datafusion::common::TableReference::bare("test_relation"),
                Arc::clone(&schema),
                Arc::clone(&schema),
                vec![
                    FieldIndexMapping::direct(0, 0),
                    FieldIndexMapping::direct(1, 1),
                ],
            )
            .expect("schema contract"),
        );
        let mut statistics = Statistics::new_unknown(schema.as_ref());
        statistics.num_rows = Precision::Exact(3);
        let spy = Arc::new(StructuredScanSpy {
            schema: RwLock::new(schema),
            plan_schema_override: Mutex::new(None),
            constraints: Constraints::new_unverified(vec![Constraint::Unique(vec![0])]),
            statistics,
            pushdown: vec![TableProviderFilterPushDown::Inexact],
            legacy_scan_calls: AtomicUsize::new(0),
            captured: Mutex::new(Vec::new()),
            last_plan: Mutex::new(None),
        });
        let observations = Arc::new(RecordingObservations::default());
        let provider = SchemaContractTableProvider::try_new_with_observations(
            contract,
            Arc::clone(&spy) as Arc<dyn TableProvider>,
            Arc::clone(&observations) as Arc<dyn ProviderScanObservationSink>,
        )
        .expect("schema-bound provider");
        (spy, provider, observations)
    }

    #[tokio::test]
    async fn storage_provider_restores_fixed_identity_type_and_logical_schema() {
        let logical = Arc::new(Schema::new(vec![Field::new(
            "fabric_epoch_id",
            DataType::FixedSizeBinary(16),
            false,
        )]));
        let storage = Arc::new(Schema::new(vec![Field::new(
            "fabric_epoch_id",
            DataType::Binary,
            false,
        )]));
        let contract = Arc::new(
            SchemaContract::try_new(
                "system.programmatic-observation-history.v1",
                datafusion::common::TableReference::bare("observation_history"),
                Arc::clone(&logical),
                Arc::clone(&storage),
                vec![FieldIndexMapping::direct(0, 0)],
            )
            .expect("logical/storage observation contract"),
        );
        let value = [0x5a_u8; 16];
        let batch = RecordBatch::try_new(
            Arc::clone(&storage),
            vec![Arc::new(BinaryArray::from_vec(vec![value.as_slice()]))],
        )
        .expect("storage batch");
        let inner = Arc::new(
            MemTable::try_new(storage, vec![vec![batch]]).expect("storage MemTable fixture"),
        ) as Arc<dyn TableProvider>;
        let provider = Arc::new(
            SchemaContractStorageProvider::try_new(Arc::clone(&contract), inner)
                .expect("storage-restoring provider"),
        ) as Arc<dyn TableProvider>;
        assert_eq!(provider.schema(), logical);

        let context = SessionContext::new();
        context
            .register_table("observation_history", provider)
            .expect("register restored provider");
        let batches = context
            .table("observation_history")
            .await
            .expect("resolve restored provider")
            .collect()
            .await
            .expect("collect restored identity");
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].schema(), logical);
        let restored = batches[0]
            .column(0)
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .expect("logical fixed-size identity");
        assert_eq!(restored.value(0), value);
    }

    #[tokio::test]
    async fn forwards_complete_structured_scan_and_preserves_native_plan() {
        let (spy, provider, observations) = test_fixture();
        let projection = [1_usize];
        let filters = [col("id").gt(lit(1_u64))];
        let statistics_requests = [
            StatisticsRequest::RowCount,
            StatisticsRequest::Min(Arc::new(Column::from_name("id"))),
            StatisticsRequest::TotalByteSize,
        ];
        let state = SessionContext::new().state();
        let result = provider
            .scan_with_args(
                &state,
                ScanArgs::default()
                    .with_projection(Some(&projection))
                    .with_filters(Some(&filters))
                    .with_limit(Some(2))
                    .with_statistics_requests(&statistics_requests),
            )
            .await
            .expect("structured scan");

        let expected = CapturedScan {
            projection: Some(projection.to_vec()),
            filters: Some(filters.to_vec()),
            limit: Some(2),
            statistics_requests: statistics_requests.to_vec(),
        };
        assert_eq!(spy.captured().as_slice(), std::slice::from_ref(&expected));
        assert_eq!(spy.legacy_scan_calls.load(Ordering::SeqCst), 0);
        assert!(Arc::ptr_eq(result.plan(), &spy.last_plan()));
        assert_eq!(result.plan().schema().field(0).name(), "label");

        let observed = observations.snapshot();
        assert_eq!(observed.len(), 1);
        assert_eq!(
            observed[0].source_schema_identity(),
            "application.test_relation.v1"
        );
        assert_eq!(observed[0].projection(), Some(projection.as_slice()));
        assert_eq!(observed[0].filters(), Some(filters.as_slice()));
        assert_eq!(observed[0].limit(), Some(2));
        assert_eq!(observed[0].statistics_requests(), statistics_requests);
    }

    #[tokio::test]
    async fn legacy_scan_also_uses_structured_path_and_delegates_truthful_metadata() {
        let (spy, provider, observations) = test_fixture();
        let projection = vec![0_usize];
        let filters = [col("id").gt(lit(0_u64))];
        let state = SessionContext::new().state();

        assert_eq!(
            provider
                .supports_filters_pushdown(&[&filters[0]])
                .expect("pushdown posture"),
            [TableProviderFilterPushDown::Inexact]
        );
        assert_eq!(provider.constraints(), Some(&spy.constraints));
        assert_eq!(provider.statistics(), Some(spy.statistics.clone()));

        let plan = provider
            .scan(&state, Some(&projection), &filters, Some(1))
            .await
            .expect("legacy scan");
        assert!(Arc::ptr_eq(&plan, &spy.last_plan()));
        assert_eq!(spy.legacy_scan_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            spy.captured(),
            [CapturedScan {
                projection: Some(projection),
                filters: Some(filters.to_vec()),
                limit: Some(1),
                statistics_requests: Vec::new(),
            }]
        );
        assert_eq!(observations.snapshot().len(), 1);
    }

    #[tokio::test]
    async fn rejects_provider_and_native_plan_schema_drift() {
        let (spy, provider, observations) = test_fixture();
        let state = SessionContext::new().state();
        let drifted = Arc::new(Schema::new(vec![
            Arc::new(Field::new("id", DataType::Int64, false)),
            Arc::new(Field::new("label", DataType::Utf8, true)),
        ]));
        spy.set_schema(Arc::clone(&drifted));

        let error = provider
            .scan_with_args(&state, ScanArgs::default().with_limit(Some(1)))
            .await
            .expect_err("provider schema drift must fail before delegation");
        let provider_error = external_provider_error(&error);
        assert!(matches!(
            provider_error,
            ProviderContractError::SchemaContract { source }
                if matches!(
                    source.as_ref(),
                    SchemaContractError::IncompatibleSchema { difference, .. }
                        if matches!(difference.kind(), SchemaDifferenceKind::DataType { .. })
                )
        ));
        assert!(spy.captured().is_empty());
        assert_eq!(observations.snapshot().len(), 1);

        spy.set_schema(logical_schema());
        spy.set_plan_schema_override(drifted);
        let error = provider
            .scan_with_args(&state, ScanArgs::default())
            .await
            .expect_err("native plan schema drift must fail after delegation");
        assert!(matches!(
            external_provider_error(&error),
            ProviderContractError::PlanSchemaDrift { .. }
        ));
        assert_eq!(spy.captured().len(), 1);
    }

    fn external_provider_error(error: &DataFusionError) -> &ProviderContractError {
        match error {
            DataFusionError::External(source) => source
                .downcast_ref::<ProviderContractError>()
                .expect("provider contract error"),
            other => panic!("unexpected DataFusion error: {other}"),
        }
    }
}
