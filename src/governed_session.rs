//! Sealed DataFusion ingress for candidate gates and serving execution.

use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::ArrowError;
use datafusion::catalog::TableProvider;
use datafusion::datasource::MemTable;
use datafusion::error::DataFusionError;
use datafusion::execution::SessionStateBuilder;
use datafusion::execution::runtime_env::RuntimeEnv;
use datafusion::logical_expr::registry::MemoryExtensionTypeRegistry;
use datafusion::logical_expr::{Expr, LogicalPlan};
use datafusion::physical_plan::{SendableRecordBatchStream, execute_stream};
use datafusion::prelude::{DataFrame, SessionConfig, SessionContext};
use thiserror::Error;

use crate::ontology_gate::{
    GateResourceEnvelope, OntologyGateError, OntologyGateOutcome, execute_ontology_gate_once,
};

/// Plan capability with private construction and no public raw-plan accessor.
#[derive(Clone, Debug)]
pub struct GovernedPlan {
    plan: LogicalPlan,
    session_identity: String,
    policy_identity: String,
    plan_identity: String,
}

impl GovernedPlan {
    #[must_use]
    pub fn plan_identity(&self) -> &str {
        &self.plan_identity
    }

    #[must_use]
    pub fn policy_identity(&self) -> &str {
        &self.policy_identity
    }

    #[must_use]
    pub fn session_identity(&self) -> &str {
        &self.session_identity
    }
}

/// Application-owned session; callers cannot reach its optimizer, planner, or physical context.
pub struct GovernedSession {
    context: SessionContext,
    session_identity: String,
    config_identity: String,
    policy_identity: String,
}

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

/// Build the exact Delta-provider session state behind the internal adapter boundary.
pub(crate) fn provider_session_state(
    config: SessionConfig,
) -> Arc<datafusion::execution::SessionState> {
    Arc::new(SessionContext::new_with_config(config).state())
}

/// Execute one exact-provider scan under a caller-supplied bounded runtime.
///
/// The caller receives only the result stream; logical and physical planning remain sealed.
pub(crate) async fn stream_provider(
    config: SessionConfig,
    runtime: Arc<RuntimeEnv>,
    provider: Arc<dyn TableProvider>,
) -> Result<SendableRecordBatchStream, DataFusionError> {
    let context = SessionContext::new_with_config_rt(config, runtime);
    let plan = context
        .state()
        .create_physical_plan(&context.read_table(provider)?.into_optimized_plan()?)
        .await?;
    execute_stream(plan, context.task_ctx())
}

/// Failures at the sole governed ingress.
#[derive(Debug, Error)]
pub enum GovernedSessionError {
    #[error(transparent)]
    Arrow(#[from] ArrowError),
    #[error(transparent)]
    DataFusion(#[from] DataFusionError),
    #[error(transparent)]
    Gate(#[from] OntologyGateError),
    #[error("GOVERNED_PLAN_INGRESS_REJECTED:{0}")]
    Ingress(String),
}

fn framed(parts: impl IntoIterator<Item = impl AsRef<[u8]>>) -> String {
    let mut bytes = Vec::new();
    for part in parts {
        let part = part.as_ref();
        bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
        bytes.extend_from_slice(part);
    }
    crate::integrity::framed_digest(&bytes)
}

fn reject_sql_surface(sql: &str) -> Result<(), GovernedSessionError> {
    let normalized = sql.trim_start().to_ascii_uppercase();
    let forbidden = [
        "PREPARE ",
        "CREATE ",
        "DROP ",
        "ALTER ",
        "INSERT ",
        "UPDATE ",
        "DELETE ",
        "COPY ",
        "ANALYZE ",
        "EXPLAIN ANALYZE ",
        "SET ",
        "START TRANSACTION",
        "COMMIT",
        "ROLLBACK",
    ];
    if let Some(prefix) = forbidden
        .into_iter()
        .find(|prefix| normalized.starts_with(prefix))
    {
        return Err(GovernedSessionError::Ingress(format!(
            "{prefix} is outside sealed query ingress"
        )));
    }
    Ok(())
}

impl GovernedSession {
    /// Construct a bounded session with DataFusion defaults followed by the application analyzer.
    ///
    /// # Errors
    ///
    /// Returns an Arrow error if the generated extension registry is internally inconsistent.
    pub fn new(
        config: SessionConfig,
        policy_identity: impl Into<String>,
    ) -> Result<Self, GovernedSessionError> {
        let policy_identity = policy_identity.into();
        if policy_identity.trim().is_empty() {
            return Err(GovernedSessionError::Ingress(
                "policy identity is empty".into(),
            ));
        }
        let config_identity = framed([
            b"governed-datafusion-config.v1".as_slice(),
            format!("{config:?}").as_bytes(),
        ]);
        let extension_types = MemoryExtensionTypeRegistry::new_with_types(
            crate::schema_registry::extension_type_registrations(),
        )?;
        let state = SessionStateBuilder::new()
            .with_default_features()
            .with_config(config)
            .with_extension_type_registry(Arc::new(extension_types))
            .with_analyzer_rule(Arc::new(
                crate::domain_conformance::DomainConformanceRule::new(),
            ))
            .build();
        let context = SessionContext::new_with_state(state);
        let session_identity = framed([
            b"governed-datafusion-session.v1".as_slice(),
            policy_identity.as_bytes(),
            config_identity.as_bytes(),
            crate::schema_registry::schema_contract_digest().as_bytes(),
        ]);
        Ok(Self {
            context,
            session_identity,
            config_identity,
            policy_identity,
        })
    }

    #[must_use]
    pub fn session_identity(&self) -> &str {
        &self.session_identity
    }

    #[must_use]
    pub fn config_identity(&self) -> &str {
        &self.config_identity
    }

    #[must_use]
    pub fn policy_identity(&self) -> &str {
        &self.policy_identity
    }

    /// Construct an in-memory relational input inside this sealed session.
    pub(crate) fn frame(
        &self,
        batch: &arrow_array::RecordBatch,
    ) -> Result<DataFrame, GovernedSessionError> {
        let schema = batch.schema();
        Ok(self.context.read_table(Arc::new(MemTable::try_new(
            Arc::clone(&schema),
            vec![vec![batch.clone()]],
        )?))?)
    }

    /// Seal, execute once, and report whether a bounded violation frame has any rows.
    pub(crate) async fn rejects_any(
        &self,
        frame: DataFrame,
        action_id: &str,
    ) -> Result<bool, GovernedSessionError> {
        let outcome = self
            .execute_frame(frame.limit(0, Some(1))?, action_id)
            .await?;
        Ok(outcome.receipt.gate_checksum.row_count != 0)
    }

    /// Seal and execute one relational validation frame through the once-only gate action.
    pub(crate) async fn execute_frame(
        &self,
        frame: DataFrame,
        action_id: &str,
    ) -> Result<OntologyGateOutcome, GovernedSessionError> {
        let governed = self.seal_plan(frame.into_unoptimized_plan())?;
        self.execute_gate(
            &governed,
            &format!("validation:{action_id}"),
            "candidate:publication",
            action_id,
            &GateResourceEnvelope::default(),
        )
        .await
    }

    /// Seal an already-built plan only after default and application analyzers accept it.
    ///
    /// # Errors
    ///
    /// Rejects statements, custom nodes, unresolved forms, or ID-domain violations.
    pub fn seal_plan(&self, plan: LogicalPlan) -> Result<GovernedPlan, GovernedSessionError> {
        let state = self.context.state();
        let analyzed = state.analyzer().execute_and_check(
            plan,
            state.config_options().as_ref(),
            |_plan, _rule| {},
        )?;
        // The application analyzer is idempotent and must remain last after built-in resolution.
        let twice = crate::domain_conformance::analyze_governed_plan(analyzed.clone())?;
        if analyzed != twice {
            return Err(GovernedSessionError::Ingress(
                "application analyzer is not idempotent".into(),
            ));
        }
        let plan_identity = framed([
            b"governed-logical-plan.v1".as_slice(),
            self.session_identity.as_bytes(),
            self.policy_identity.as_bytes(),
            analyzed.display_indent_schema().to_string().as_bytes(),
        ]);
        Ok(GovernedPlan {
            plan: analyzed,
            session_identity: self.session_identity.clone(),
            policy_identity: self.policy_identity.clone(),
            plan_identity,
        })
    }

    /// Parse SQL through DataFusion's default analyzer and then seal the resulting query plan.
    ///
    /// # Errors
    ///
    /// Rejects every non-query command before planning and all invalid plans after analysis.
    pub async fn seal_sql(&self, sql: &str) -> Result<GovernedPlan, GovernedSessionError> {
        reject_sql_surface(sql)?;
        let plan = self.context.state().create_logical_plan(sql).await?;
        self.seal_plan(plan)
    }

    /// Execute one sealed gate plan without exposing raw optimizer/planner/physical APIs.
    ///
    /// # Errors
    ///
    /// Rejects plans sealed by another governed session or policy, planning failures,
    /// execution failures, resource-limit breaches, and result-checksum failures.
    pub async fn execute_gate(
        &self,
        governed: &GovernedPlan,
        execution_id: &str,
        candidate_id: &str,
        action_id: &str,
        limits: &GateResourceEnvelope,
    ) -> Result<OntologyGateOutcome, GovernedSessionError> {
        if governed.session_identity != self.session_identity
            || governed.policy_identity != self.policy_identity
        {
            return Err(GovernedSessionError::Ingress(
                "governed plan belongs to another session or policy".into(),
            ));
        }
        Ok(execute_ontology_gate_once(
            &self.context,
            &governed.plan,
            execution_id,
            candidate_id,
            action_id,
            limits,
        )
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::io::{Seek as _, SeekFrom};
    use std::sync::Arc;

    use arrow_array::{ArrayRef, FixedSizeBinaryArray, RecordBatch};
    use arrow_ipc::reader::StreamReader;
    use arrow_ipc::writer::StreamWriter;
    use datafusion::datasource::{MemTable, provider_as_source};
    use datafusion::logical_expr::{LogicalPlanBuilder, col};
    use datafusion::prelude::SessionConfig;
    use parquet::arrow::ArrowWriter;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    use super::GovernedSession;
    use crate::domain_conformance::{
        DATAFUSION_EXPR_VARIANT_CENSUS, DATAFUSION_LOGICAL_PLAN_VARIANT_CENSUS,
    };
    use crate::schema_registry::{DomainTypedLiteral, table_spec};

    fn session() -> GovernedSession {
        GovernedSession::new(SessionConfig::new(), "policy.test.v1").expect("governed session")
    }

    fn workspace_plan() -> datafusion::logical_expr::LogicalPlan {
        let schema = Arc::clone(&table_spec(1).expect("workspace table").arrow_schema);
        let provider = Arc::new(
            MemTable::try_new(
                Arc::clone(&schema),
                vec![vec![RecordBatch::new_empty(schema)]],
            )
            .expect("workspace provider"),
        );
        LogicalPlanBuilder::scan("workspace_probe", provider_as_source(provider), None)
            .expect("scan")
            .build()
            .expect("plan")
    }

    #[test]
    fn ontology_domain_state_effect_truth_table() {
        let session = session();
        let valid = LogicalPlanBuilder::from(workspace_plan())
            .filter(
                col("workspace_id").eq(DomainTypedLiteral::new("workspace", [1; 16])
                    .expect("workspace literal")
                    .into_expr()),
            )
            .expect("same-domain filter")
            .build()
            .expect("valid plan");
        assert!(session.seal_plan(valid).is_ok());
        let invalid = LogicalPlanBuilder::from(workspace_plan())
            .filter(
                col("workspace_id").eq(DomainTypedLiteral::new("repository", [1; 16])
                    .expect("repository literal")
                    .into_expr()),
            )
            .expect("cross-domain filter")
            .build()
            .expect("invalid plan");
        assert!(session.seal_plan(invalid).is_err());
    }

    #[test]
    fn ontology_analyzer_pinned_variant_census() {
        assert_eq!(DATAFUSION_EXPR_VARIANT_CENSUS.len(), 37);
        assert_eq!(DATAFUSION_LOGICAL_PLAN_VARIANT_CENSUS.len(), 25);
        assert!(DATAFUSION_EXPR_VARIANT_CENSUS.contains(&"Wildcard"));
        assert!(DATAFUSION_LOGICAL_PLAN_VARIANT_CENSUS.contains(&"Analyze"));
    }

    #[tokio::test]
    async fn ontology_analyzer_bypass_matrix() {
        let session = session();
        for sql in [
            "PREPARE q AS SELECT 1",
            "CREATE TABLE t(x INT)",
            "INSERT INTO t VALUES (1)",
            "COPY (SELECT 1) TO 'x'",
            "EXPLAIN ANALYZE SELECT 1",
            "ANALYZE t",
        ] {
            assert!(session.seal_sql(sql).await.is_err(), "{sql}");
        }
        assert!(session.seal_sql("SELECT 1 AS value").await.is_ok());
    }

    #[test]
    fn ontology_arrow_extension_boundary_matrix() {
        let source_schema = Arc::clone(&table_spec(1).expect("workspace table").arrow_schema);
        let workspace_index = source_schema
            .index_of("workspace_id")
            .expect("workspace ID field");
        let schema = Arc::new(
            source_schema
                .project(&[workspace_index])
                .expect("workspace ID projection"),
        );
        let workspace = FixedSizeBinaryArray::try_from_sparse_iter_with_size(
            std::iter::once(Some([1_u8; 16].as_slice())),
            16,
        )
        .expect("workspace array");
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(workspace.clone()) as ArrayRef],
        )
        .expect("typed batch");
        let mut bytes = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut bytes, schema.as_ref()).expect("writer");
            writer.write(&batch).expect("write");
            writer.finish().expect("finish");
        }
        let replay = StreamReader::try_new(Cursor::new(bytes), None)
            .expect("reader")
            .collect::<Result<Vec<_>, _>>()
            .expect("replay");
        assert_eq!(replay[0].schema(), schema);
        for field in replay[0].schema().fields() {
            crate::schema_registry::validate_logical_extension_field(field)
                .expect("extension boundary");
        }

        let mut parquet_file = tempfile::tempfile().expect("temporary Parquet file");
        {
            let mut writer = ArrowWriter::try_new(
                parquet_file
                    .try_clone()
                    .expect("clone temporary Parquet file"),
                Arc::clone(&schema),
                None,
            )
            .expect("Parquet writer");
            writer.write(&batch).expect("write Parquet batch");
            writer.close().expect("close Parquet writer");
        }
        parquet_file
            .seek(SeekFrom::Start(0))
            .expect("rewind Parquet file");
        let parquet_batch = ParquetRecordBatchReaderBuilder::try_new(parquet_file)
            .expect("Parquet reader")
            .build()
            .expect("Parquet batch reader")
            .next()
            .expect("one Parquet batch")
            .expect("read Parquet batch");
        assert_eq!(parquet_batch.schema().field(0), schema.field(0));
        crate::schema_registry::validate_logical_extension_field(parquet_batch.schema().field(0))
            .expect("Parquet extension boundary");
    }
}
