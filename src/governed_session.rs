//! Sealed DataFusion ingress for candidate gates and serving execution.

use std::fmt::Write as _;
use std::num::NonZeroUsize;
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};
use std::path::PathBuf;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::ArrowError;
use datafusion::catalog::TableProvider;
use datafusion::datasource::MemTable;
use datafusion::error::DataFusionError;
use datafusion::execution::SessionStateBuilder;
use datafusion::execution::memory_pool::{FairSpillPool, TrackConsumersPool};
use datafusion::execution::runtime_env::{RuntimeEnv, RuntimeEnvBuilder};
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
    policy: Arc<crate::domain_conformance::DomainOperationPolicy>,
    runtime_profile: GovernedRuntimeProfile,
    spill_directory: PrivateSpillDirectory,
}

/// Private process-owned spill directory with bounded orphan reconciliation.
#[derive(Debug)]
pub(crate) struct PrivateSpillDirectory {
    path: PathBuf,
}

impl PrivateSpillDirectory {
    pub(crate) fn create(parent: &std::path::Path, family: &str) -> Result<Self, std::io::Error> {
        std::fs::create_dir_all(parent)?;
        reconcile_orphaned_spill_directories(parent, family)?;
        let nonce = crate::identity::random_registration_nonce()
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let suffix = nonce.iter().fold(
            String::with_capacity(nonce.len() * 2),
            |mut output, byte| {
                write!(output, "{byte:02x}").expect("writing to a String cannot fail");
                output
            },
        );
        let path = parent.join(format!("{family}-{}-{suffix}", std::process::id()));
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700).create(&path)?;
        Ok(Self { path })
    }

    #[must_use]
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for PrivateSpillDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn reconcile_orphaned_spill_directories(
    parent: &std::path::Path,
    family: &str,
) -> Result<usize, std::io::Error> {
    let prefix = format!("{family}-");
    let process_id = std::process::id();
    let owner_uid = rustix::process::getuid().as_raw();
    let mut removed = 0;
    for entry in std::fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(remainder) = name.strip_prefix(&prefix) else {
            continue;
        };
        let Some((pid, nonce)) = remainder.split_once('-') else {
            continue;
        };
        let Ok(pid) = pid.parse::<u32>() else {
            continue;
        };
        if pid == process_id
            || nonce.len() != 32
            || !nonce.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            continue;
        }
        let metadata = std::fs::symlink_metadata(entry.path())?;
        if !metadata.file_type().is_dir()
            || metadata.uid() != owner_uid
            || metadata.mode() & 0o777 != 0o700
        {
            continue;
        }
        let Ok(raw_pid) = i32::try_from(pid) else {
            continue;
        };
        let Some(pid) = rustix::process::Pid::from_raw(raw_pid) else {
            continue;
        };
        if matches!(
            rustix::process::test_kill_process(pid),
            Err(rustix::io::Errno::SRCH)
        ) {
            std::fs::remove_dir_all(entry.path())?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Versioned bounded runtime profile shared by candidate program execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernedRuntimeProfile {
    pub profile_version: &'static str,
    pub memory_limit_bytes: usize,
    pub max_spill_bytes: u64,
    pub batch_size: usize,
    pub target_partitions: usize,
    pub max_execution_millis: u64,
    pub tracked_consumer_count: NonZeroUsize,
}

impl Default for GovernedRuntimeProfile {
    fn default() -> Self {
        Self {
            profile_version: "governed-runtime.v1",
            memory_limit_bytes: 32 * 1_024 * 1_024,
            max_spill_bytes: 64 * 1_024 * 1_024,
            batch_size: 65_536,
            target_partitions: 2,
            max_execution_millis: 30_000,
            tracked_consumer_count: NonZeroUsize::new(16).expect("non-zero constant"),
        }
    }
}

impl GovernedRuntimeProfile {
    #[must_use]
    pub fn identity(&self) -> String {
        framed([
            self.profile_version.as_bytes(),
            &self.memory_limit_bytes.to_be_bytes(),
            &self.max_spill_bytes.to_be_bytes(),
            &self.batch_size.to_be_bytes(),
            &self.target_partitions.to_be_bytes(),
            &self.max_execution_millis.to_be_bytes(),
            &self.tracked_consumer_count.get().to_be_bytes(),
            b"drop-task-stream-on-cancel.v1",
            b"remove-private-spill-directory-on-drop.v1",
        ])
    }

    fn validate(&self) -> Result<(), GovernedSessionError> {
        if self.profile_version.is_empty()
            || self.memory_limit_bytes == 0
            || self.max_spill_bytes == 0
            || self.batch_size == 0
            || self.target_partitions == 0
            || self.max_execution_millis == 0
        {
            return Err(GovernedSessionError::Ingress(
                "governed runtime profile contains a zero/empty limit".into(),
            ));
        }
        Ok(())
    }
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
    #[error(transparent)]
    Program(#[from] crate::ontology_program::OntologyProgramError),
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

#[cfg(test)]
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
    #[cfg(test)]
    pub(crate) fn new(config: SessionConfig) -> Result<Self, GovernedSessionError> {
        let package = crate::ontology_program::build_ontology_program_package(
            &crate::ontology_program::OntologyPackagingProfile::default(),
        )?;
        Self::new_for_package(config, &package)
    }

    /// Construct a session from the exact ontology package that selects its analyzer policy.
    pub(crate) fn new_for_package(
        config: SessionConfig,
        package: &crate::ontology_program::OntologyProgramPackage,
    ) -> Result<Self, GovernedSessionError> {
        Self::new_with_runtime(config, package, GovernedRuntimeProfile::default())
    }

    /// Construct a session with an explicit versioned memory, spill, batch, partition, and
    /// deadline profile.
    ///
    /// # Errors
    ///
    /// Rejects an invalid runtime profile or empty policy identity, or returns a DataFusion/
    /// Arrow construction error when the governed runtime cannot be built.
    pub(crate) fn new_with_runtime(
        config: SessionConfig,
        package: &crate::ontology_program::OntologyProgramPackage,
        runtime_profile: GovernedRuntimeProfile,
    ) -> Result<Self, GovernedSessionError> {
        runtime_profile.validate()?;
        crate::ontology_program::validate_ontology_program_package(package)?;
        let policy =
            Arc::new(crate::domain_conformance::DomainOperationPolicy::from_package(package)?);
        let policy_identity = policy.identity().to_owned();
        let config = config
            .with_batch_size(runtime_profile.batch_size)
            .with_target_partitions(runtime_profile.target_partitions);
        let config_identity = framed([
            b"governed-datafusion-config.v1".as_slice(),
            format!("{config:?}").as_bytes(),
            runtime_profile.identity().as_bytes(),
        ]);
        let spill_directory =
            PrivateSpillDirectory::create(&std::env::temp_dir(), "codefabric-governed").map_err(
                |error| {
                    GovernedSessionError::Ingress(format!(
                        "cannot create governed spill directory: {error}"
                    ))
                },
            )?;
        let runtime = RuntimeEnvBuilder::new()
            .with_memory_pool(Arc::new(TrackConsumersPool::new(
                FairSpillPool::new(runtime_profile.memory_limit_bytes),
                runtime_profile.tracked_consumer_count,
            )))
            .with_temp_file_path(spill_directory.path())
            .with_max_temp_directory_size(runtime_profile.max_spill_bytes)
            .build()
            .map_err(GovernedSessionError::DataFusion)?;
        let extension_types = MemoryExtensionTypeRegistry::new_with_types(
            crate::schema_registry::extension_type_registrations(),
        )?;
        let state = SessionStateBuilder::new()
            .with_default_features()
            .with_config(config)
            .with_runtime_env(Arc::new(runtime))
            .with_extension_type_registry(Arc::new(extension_types))
            .with_analyzer_rule(Arc::new(
                crate::domain_conformance::DomainConformanceRule::new(Arc::clone(&policy)),
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
            policy,
            runtime_profile,
            spill_directory,
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

    #[must_use]
    pub fn result_policy_identity(&self) -> String {
        self.policy.result_policy_identity()
    }

    #[must_use]
    pub fn runtime_profile(&self) -> &GovernedRuntimeProfile {
        &self.runtime_profile
    }

    #[must_use]
    pub fn spill_directory(&self) -> &std::path::Path {
        self.spill_directory.path()
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
    pub(crate) fn seal_plan(
        &self,
        plan: LogicalPlan,
    ) -> Result<GovernedPlan, GovernedSessionError> {
        let state = self.context.state();
        let analyzed = state.analyzer().execute_and_check(
            plan,
            state.config_options().as_ref(),
            |_plan, _rule| {},
        )?;
        // The application analyzer is idempotent and must remain last after built-in resolution.
        let twice = crate::domain_conformance::analyze_governed_plan(
            analyzed.clone(),
            Arc::clone(&self.policy),
        )?;
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
    #[cfg(test)]
    pub(crate) async fn seal_sql(&self, sql: &str) -> Result<GovernedPlan, GovernedSessionError> {
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
        self.execute_gate_with_cancellation(
            governed,
            execution_id,
            candidate_id,
            action_id,
            limits,
            &crate::cancellation::Cancellation::default(),
        )
        .await
    }

    /// Execute one sealed gate with cooperative cancellation. Cancellation or timeout drops the
    /// sole DataFusion stream future, which is the pinned engine's task-cancellation boundary.
    ///
    /// # Errors
    ///
    /// Rejects a plan from another governed session or policy and reports cancellation,
    /// deadline, planning, execution, resource, spill, and checksum failures.
    pub async fn execute_gate_with_cancellation(
        &self,
        governed: &GovernedPlan,
        execution_id: &str,
        candidate_id: &str,
        action_id: &str,
        limits: &GateResourceEnvelope,
        cancellation: &crate::cancellation::Cancellation,
    ) -> Result<OntologyGateOutcome, GovernedSessionError> {
        if governed.session_identity != self.session_identity
            || governed.policy_identity != self.policy_identity
        {
            return Err(GovernedSessionError::Ingress(
                "governed plan belongs to another session or policy".into(),
            ));
        }
        let deadline = limits
            .max_execution_millis
            .min(self.runtime_profile.max_execution_millis);
        if cancellation.is_cancelled() {
            return Err(crate::ontology_gate::OntologyGateError::Resource(
                crate::ontology_gate::GateResourceFailure::Cancelled,
            )
            .into());
        }
        if deadline == 0 {
            return Err(crate::ontology_gate::OntologyGateError::Resource(
                crate::ontology_gate::GateResourceFailure::Deadline { limit_millis: 0 },
            )
            .into());
        }
        let execution = execute_ontology_gate_once(
            &self.context,
            &governed.plan,
            execution_id,
            candidate_id,
            action_id,
            limits,
        );
        tokio::pin!(execution);
        let cancel = async {
            loop {
                if cancellation.is_cancelled() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        };
        tokio::pin!(cancel);
        tokio::select! {
            biased;
            () = &mut cancel => Err(crate::ontology_gate::OntologyGateError::Resource(
                crate::ontology_gate::GateResourceFailure::Cancelled,
            ).into()),
            () = tokio::time::sleep(std::time::Duration::from_millis(deadline)) => Err(crate::ontology_gate::OntologyGateError::Resource(
                crate::ontology_gate::GateResourceFailure::Deadline {
                    limit_millis: deadline,
                },
            )
            .into()),
            outcome = &mut execution => match outcome {
                Err(crate::ontology_gate::OntologyGateError::DataFusion(
                    DataFusionError::ResourcesExhausted(_),
                )) => Err(crate::ontology_gate::OntologyGateError::Resource(
                    crate::ontology_gate::GateResourceFailure::Memory {
                        limit_bytes: self.runtime_profile.memory_limit_bytes,
                    },
                ).into()),
                outcome => Ok(outcome?),
            },
        }
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
    use datafusion::execution::memory_pool::MemoryConsumer;
    use datafusion::logical_expr::{LogicalPlanBuilder, col};
    use datafusion::prelude::SessionConfig;
    use parquet::arrow::ArrowWriter;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    use super::{GovernedRuntimeProfile, GovernedSession, GovernedSessionError};
    use crate::domain_conformance::{
        DATAFUSION_EXPR_VARIANT_CENSUS, DATAFUSION_LOGICAL_PLAN_VARIANT_CENSUS,
    };
    use crate::ontology_gate::GateResourceEnvelope;
    use crate::schema_registry::{DomainTypedLiteral, table_spec};

    fn session() -> GovernedSession {
        GovernedSession::new(SessionConfig::new()).expect("governed session")
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
    fn ontology_governed_spill_orphan_reconciliation() {
        use std::os::unix::fs::PermissionsExt as _;

        let parent = tempfile::tempdir().expect("private spill parent");
        let nonce = "00000000000000000000000000000000";
        let orphan = parent
            .path()
            .join(format!("codefabric-candidate-2147483647-{nonce}"));
        std::fs::create_dir(&orphan).expect("orphan spill directory");
        std::fs::set_permissions(&orphan, std::fs::Permissions::from_mode(0o700))
            .expect("private orphan permissions");
        std::fs::write(orphan.join("spill"), b"orphaned bytes").expect("orphan spill bytes");

        let foreign_mode = parent
            .path()
            .join(format!("codefabric-candidate-2147483646-{nonce}"));
        std::fs::create_dir(&foreign_mode).expect("non-private spill directory");
        std::fs::set_permissions(&foreign_mode, std::fs::Permissions::from_mode(0o755))
            .expect("non-private permissions");

        assert_eq!(
            super::reconcile_orphaned_spill_directories(parent.path(), "codefabric-candidate")
                .expect("bounded orphan reconciliation"),
            1
        );
        assert!(!orphan.exists());
        assert!(
            foreign_mode.exists(),
            "unsafe ownership/mode must fail closed"
        );

        let live = super::PrivateSpillDirectory::create(parent.path(), "codefabric-candidate")
            .expect("new private spill directory");
        assert_eq!(
            std::fs::symlink_metadata(live.path())
                .expect("live spill metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
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

    #[tokio::test]
    async fn ontology_governed_runtime_deadline_cancellation_memory_cleanup() {
        let profile = GovernedRuntimeProfile {
            profile_version: "governed-runtime.test.v1",
            memory_limit_bytes: 1_024,
            max_spill_bytes: 4_096,
            batch_size: 64,
            target_partitions: 1,
            max_execution_millis: 1_000,
            tracked_consumer_count: std::num::NonZeroUsize::new(4).expect("non-zero"),
        };
        let package = crate::ontology_program::build_ontology_program_package(
            &crate::ontology_program::OntologyPackagingProfile::default(),
        )
        .expect("generated ontology package");
        let session =
            GovernedSession::new_with_runtime(SessionConfig::new(), &package, profile.clone())
                .expect("bounded governed session");
        assert_eq!(session.runtime_profile(), &profile);

        let task_context = session.context.task_ctx();
        let pool = task_context.memory_pool();
        let reservation = MemoryConsumer::new("ontology-runtime-limit-probe").register(pool);
        let memory_error = reservation
            .try_grow(profile.memory_limit_bytes + 1)
            .expect_err("unspillable reservation exceeds governed memory");
        assert!(memory_error.to_string().contains("Resources exhausted"));

        let governed = session
            .seal_plan(workspace_plan())
            .expect("sealed test plan");
        let cancellation = crate::cancellation::Cancellation::with_check_interval(1);
        cancellation.cancel();
        let cancelled = session
            .execute_gate_with_cancellation(
                &governed,
                "cancelled-execution",
                "candidate-runtime",
                "runtime-cancel",
                &GateResourceEnvelope::default(),
                &cancellation,
            )
            .await
            .expect_err("pre-cancelled execution");
        assert!(matches!(
            cancelled,
            GovernedSessionError::Gate(crate::ontology_gate::OntologyGateError::Resource(
                crate::ontology_gate::GateResourceFailure::Cancelled
            ))
        ));

        let deadline = session
            .execute_gate(
                &governed,
                "deadline-execution",
                "candidate-runtime",
                "runtime-deadline",
                &GateResourceEnvelope {
                    max_execution_millis: 0,
                    ..GateResourceEnvelope::default()
                },
            )
            .await
            .expect_err("zero deadline");
        assert!(matches!(
            deadline,
            GovernedSessionError::Gate(crate::ontology_gate::OntologyGateError::Resource(
                crate::ontology_gate::GateResourceFailure::Deadline { limit_millis: 0 }
            ))
        ));

        let spill_directory = session.spill_directory().to_path_buf();
        std::fs::write(spill_directory.join("orphaned-spill-probe"), b"spill")
            .expect("write spill cleanup probe");
        drop(session);
        assert!(
            !spill_directory.exists(),
            "session drop must remove spill authority"
        );
    }
}
