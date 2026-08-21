//! Generated-schema Delta fabric for the local workstation deployment profile.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::builder::{BinaryBuilder, ListBuilder};
use arrow_array::{
    Array as _, ArrayRef, BinaryArray, BooleanArray, Int16Array, Int32Array, Int64Array,
    RecordBatch, StringArray, TimestampMicrosecondArray,
};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use datafusion::catalog::TableProvider;
use datafusion::prelude::{SessionConfig, SessionContext, col};
use deltalake::DeltaTable;
use deltalake::kernel::engine::arrow_conversion::{TryIntoArrow as _, TryIntoKernel as _};
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;
use thiserror::Error;
use url::Url;

use crate::identity::{IdentityDomain, SOURCE_CONTEXT_ID, context_set_identity, encode_public_id};
use crate::schema_registry::{TableSpec, table_spec, table_specs};
use crate::workspace_registry::WorkspaceRecord;

mod mutation;
mod overlay;
mod publication;
#[cfg(feature = "daemon")]
mod serving;
mod snapshot_catalog;
pub use mutation::{
    MutationFaultPoint, MutationJournal, MutationPhase, MutationPhaseSpec, MutationResult,
    OwnerMutationRequest, PreparedMutation, batch_checksum,
};
pub use overlay::{
    ConsolidatedOverlay, OverlayConsolidationRequest, OverlayMutation, OverlayTable,
};
#[cfg(feature = "daemon")]
pub use overlay::{OverlayRebaseFaultPoint, OverlayRebaseOutcome, OverlayRebaseRequest};
pub use publication::{
    CurrentPublicationRecord, OwnerPublicationWrite, PublicationFaultPoint, PublicationOutcome,
    PublicationPins, PublicationRequest, PublicationScope, PublicationTableRecord,
};
#[cfg(feature = "daemon")]
pub use serving::{
    QueryPlanArtifact, ServingQueryError, ServingQueryResult, ServingQuerySession,
    ServingRuntimeConfig, ServingRuntimeEvidence,
};
pub use snapshot_catalog::{
    DeltaAccessProfile, DeltaHandleFactory, DeltaMaterializationPosture, EmptySnapshotOverlay,
    ProfiledDeltaHandle, SnapshotConstructionMetrics, SnapshotConstructionStage,
    SnapshotOverlayProviderFactory, SnapshotProviderCatalog, SnapshotProviderRecord,
};

const SCHEMA_DIGEST_KEY: &str = "com.codefabric.cpg.schema_digest";
const TYPE_WIDENING_KEY: &str = "delta.enableTypeWidening";
const TARGET_FILE_SIZE_BYTES: &str = "134217728";
const CHECKPOINT_INTERVAL: &str = "10";
const LOG_RETENTION: &str = "interval 30 days";
const DELETED_FILE_RETENTION: &str = "interval 7 days";

/// Stable failures at the generated-schema/Delta boundary.
#[derive(Debug, Error)]
pub enum FabricError {
    #[error("SCHEMA_DIGEST_MISMATCH:{table}")]
    SchemaDigestMismatch { table: String },
    #[error("FABRIC_TABLE_INVARIANT:{table}:{detail}")]
    TableInvariant { table: String, detail: String },
    #[error("LOCAL_STORAGE_PROFILE_REJECTED:{0}")]
    LocalProfile(String),
    #[error("MUTATION_CONFLICT:{0}")]
    MutationConflict(String),
    #[error("MUTATION_JOURNAL:{0}")]
    MutationJournal(String),
    #[error("MUTATION_FAULT:{0:?}")]
    MutationFault(MutationFaultPoint),
    #[error("PUBLICATION_INTEGRITY:{0}")]
    PublicationIntegrity(String),
    #[error("CURRENT_POINTER_CONFLICT:{0}")]
    CurrentPointerConflict(String),
    #[error("PUBLICATION_FAULT:{0:?}")]
    PublicationFault(PublicationFaultPoint),
    #[error("SNAPSHOT_PROVIDER_INTEGRITY:{0}")]
    SnapshotProviderIntegrity(String),
    #[error("SNAPSHOT_CATALOG_FROZEN:{0}")]
    SnapshotCatalogFrozen(String),
    #[error("OVERLAY_POLICY_VIOLATION:{0}")]
    OverlayPolicyViolation(String),
    #[error("OVERLAY_GENERATION_CONFLICT:{0}")]
    OverlayGenerationConflict(String),
    #[error("OVERLAY_MEMORY_RESERVATION:{0}")]
    OverlayMemoryReservation(String),
    #[error("OVERLAY_REBASE_RESTART_REQUIRED:{0}")]
    OverlayRebaseRestartRequired(String),
    #[error("fabric I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Delta(#[from] deltalake::DeltaTableError),
    #[error(transparent)]
    Arrow(#[from] ArrowError),
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
    #[error(transparent)]
    Identity(#[from] crate::identity::IdentityError),
}

/// Closed provider request validated before any object-store construction.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LocalProviderRequest {
    pub location: String,
    pub endpoint: Option<String>,
    pub credential: Option<String>,
    pub storage_options: BTreeMap<String, String>,
}

/// Only the local filesystem provider is constructible in Waves 0-3.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalProviderFactory;

impl LocalProviderFactory {
    /// Validate a request without constructing or registering a provider.
    ///
    /// # Errors
    ///
    /// Rejects non-file schemes and every cloud configuration seam.
    pub fn validate(request: &LocalProviderRequest) -> Result<PathBuf, FabricError> {
        if request.endpoint.is_some()
            || request.credential.is_some()
            || !request.storage_options.is_empty()
        {
            return Err(FabricError::LocalProfile(
                "credentials, endpoints, and storage options are forbidden".into(),
            ));
        }
        if let Ok(url) = Url::parse(&request.location) {
            if url.scheme() != "file"
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(FabricError::LocalProfile(format!(
                    "scheme or URL decorations are not local-only: {}",
                    url.scheme()
                )));
            }
            return url.to_file_path().map_err(|()| {
                FabricError::LocalProfile("file URL is not a local filesystem path".into())
            });
        }
        let path = PathBuf::from(&request.location);
        if !path.is_absolute() {
            return Err(FabricError::LocalProfile(
                "local table path must be absolute".into(),
            ));
        }
        Ok(path)
    }

    fn file_url(path: &Path) -> Result<Url, FabricError> {
        let request = LocalProviderRequest {
            location: path.to_string_lossy().into_owned(),
            ..LocalProviderRequest::default()
        };
        let path = Self::validate(&request)?;
        Url::from_directory_path(path).map_err(|()| {
            FabricError::LocalProfile("local table path cannot be represented as a file URL".into())
        })
    }
}

/// One workspace's deterministic three-directory Delta namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceNamespace {
    pub root: PathBuf,
    pub control: PathBuf,
    pub facts: PathBuf,
    pub derived: PathBuf,
}

impl WorkspaceNamespace {
    /// Derive the namespace from the internal workspace identity.
    ///
    /// # Errors
    ///
    /// Returns an identity encoding failure.
    pub fn new(state_root: &Path, workspace_id: [u8; 16]) -> Result<Self, FabricError> {
        let encoded = encode_public_id(IdentityDomain::Workspace, None, workspace_id)?;
        let root = state_root.join("cpg").join(encoded);
        Ok(Self {
            control: root.join("control"),
            facts: root.join("facts"),
            derived: root.join("derived"),
            root,
        })
    }

    fn table_path(&self, spec: &TableSpec) -> Result<PathBuf, FabricError> {
        let parent = match spec.family {
            "control" | "bundle" => &self.control,
            "universal-fact" => &self.facts,
            "overlay-control" => &self.derived,
            family => {
                return Err(FabricError::TableInvariant {
                    table: spec.name.into(),
                    detail: format!("unknown generated table family {family}"),
                });
            }
        };
        Ok(parent.join(spec.name))
    }
}

/// One opened table bound to its immutable generated contract.
pub struct FabricTable {
    pub table_code: i16,
    pub path: PathBuf,
    pub(super) delta: DeltaTable,
    pub(super) provider: Arc<dyn TableProvider>,
}

impl FabricTable {
    /// Current Delta transaction version for diagnostics and publication fencing.
    #[must_use]
    pub fn version(&self) -> Option<u64> {
        self.delta.version()
    }

    /// Physical-query-compatible schema exposed by the validated Delta provider.
    ///
    /// Governed table metadata remains authoritative in the generated `TableSpec` and
    /// mirrored Delta table properties. DataFusion requires this logical schema to equal
    /// the physical scan schema, whose top-level metadata is intentionally empty.
    #[must_use]
    pub fn schema(&self) -> SchemaRef {
        self.provider.schema()
    }

    /// Clone the read-only query provider without exposing a Delta writer.
    #[must_use]
    pub fn provider(&self) -> Arc<dyn TableProvider> {
        Arc::clone(&self.provider)
    }
}

/// Complete local Delta namespace for one registered workspace.
pub struct WorkspaceFabric {
    pub namespace: WorkspaceNamespace,
    pub(super) tables: BTreeMap<i16, FabricTable>,
}

/// Registry-owned projection of one Git common-directory record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonRepositoryRecord {
    pub repository_id: [u8; 16],
    pub common_dir_path_bytes: Vec<u8>,
    pub common_dir_path_display: String,
    pub object_format_code: i16,
    pub trust_policy_fingerprint: [u8; 32],
    pub updated_at: String,
}

impl WorkspaceFabric {
    #[must_use]
    pub fn table(&self, table_code: i16) -> Option<&FabricTable> {
        self.tables.get(&table_code)
    }

    #[must_use]
    pub fn tables(&self) -> impl ExactSizeIterator<Item = &FabricTable> {
        self.tables.values()
    }

    async fn replace(&mut self, table_code: i16, batch: RecordBatch) -> Result<(), FabricError> {
        let spec = table_spec(table_code).ok_or_else(|| FabricError::TableInvariant {
            table: table_code.to_string(),
            detail: "generated table is absent from the schema registry".into(),
        })?;
        mutation::enforce_write_kind(spec, mutation::DurableWriteKind::BootstrapReplace)?;
        let entry =
            self.tables
                .get_mut(&table_code)
                .ok_or_else(|| FabricError::TableInvariant {
                    table: table_code.to_string(),
                    detail: "generated table is absent from workspace fabric".into(),
                })?;
        // Delta persists field metadata in its StructType but not Arrow's top-level
        // schema metadata. Governed table metadata remains stored as table properties,
        // so present Delta with its native physical schema.
        let fields = batch.schema().fields().clone();
        let (_, columns, _) = batch.into_parts();
        let storage_batch = RecordBatch::try_new(Arc::new(Schema::new(fields)), columns)?;
        let table = entry
            .delta
            .clone()
            .write([storage_batch])
            .with_save_mode(SaveMode::Overwrite)
            .await?;
        entry.delta = table;
        entry.provider =
            exact_provider(&entry.delta, spec, DeltaAccessProfile::QueryServing).await?;
        Ok(())
    }

    async fn current_workspace_revision(&self) -> Result<Option<u64>, FabricError> {
        let provider = Arc::clone(&self.table(1).expect("generated workspace table").provider);
        let context = SessionContext::new();
        let batches = context
            .read_table(provider)?
            .select(vec![col("registration_revision")])?
            .limit(0, Some(1))?
            .collect()
            .await?;
        let Some(batch) = batches.first() else {
            return Ok(None);
        };
        if batch.num_rows() == 0 {
            return Ok(None);
        }
        let revisions = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or_else(|| FabricError::TableInvariant {
                table: "workspace".into(),
                detail: "registration_revision provider type drift".into(),
            })?;
        let revision = revisions.value(0);
        u64::try_from(revision)
            .map(Some)
            .map_err(|_| FabricError::TableInvariant {
                table: "workspace".into(),
                detail: "registration_revision is negative".into(),
            })
    }

    async fn seed_control_rows(
        &mut self,
        record: &WorkspaceRecord,
        repository: Option<&CommonRepositoryRecord>,
    ) -> Result<(), FabricError> {
        if self.current_workspace_revision().await? != Some(record.registration_revision) {
            self.replace(1, workspace_batch(record)?).await?;
        }
        if self.table_is_empty(3)? {
            self.replace(3, source_context_batch(record)?).await?;
        }
        if self.table_is_empty(4)? {
            self.replace(4, source_context_set_batch(record)?).await?;
        }
        if self.table_is_empty(11)? {
            self.replace(11, enum_catalog_batch()?).await?;
        }
        if let Some(repository) = repository
            && self.table_is_empty(2)?
        {
            self.replace(2, common_repository_batch(repository)?)
                .await?;
        }
        Ok(())
    }

    fn table_is_empty(&self, table_code: i16) -> Result<bool, FabricError> {
        let table = self
            .table(table_code)
            .ok_or_else(|| FabricError::TableInvariant {
                table: table_code.to_string(),
                detail: "generated table is absent".into(),
            })?;
        Ok(table.delta.snapshot()?.log_data().num_files() == 0)
    }
}

/// Create or reopen every generated Wave-3 table and seed registry-owned control rows.
///
/// # Errors
///
/// Returns a path, identity, generated-schema, Delta, Arrow, or DataFusion failure.
pub async fn bootstrap_workspace(
    state_root: &Path,
    record: &WorkspaceRecord,
) -> Result<WorkspaceFabric, FabricError> {
    bootstrap_workspace_with_repository(state_root, record, None).await
}

/// Create or reopen a workspace fabric and project an available Git repository row.
///
/// # Errors
///
/// Returns a path, identity, generated-schema, Delta, Arrow, or DataFusion failure.
pub async fn bootstrap_workspace_with_repository(
    state_root: &Path,
    record: &WorkspaceRecord,
    repository: Option<&CommonRepositoryRecord>,
) -> Result<WorkspaceFabric, FabricError> {
    let namespace = WorkspaceNamespace::new(state_root, record.workspace_id)?;
    for path in [&namespace.control, &namespace.facts, &namespace.derived] {
        std::fs::create_dir_all(path).map_err(|source| FabricError::Io {
            path: path.clone(),
            source,
        })?;
    }
    let mut tables = BTreeMap::new();
    for spec in table_specs() {
        let path = namespace.table_path(spec)?;
        std::fs::create_dir_all(&path).map_err(|source| FabricError::Io {
            path: path.clone(),
            source,
        })?;
        let mut table = create_or_open(&path, spec, DeltaAccessProfile::OptimizeDml).await?;
        table = install_constraints(table, spec).await?;
        validate_open_table(&table, spec)?;
        let provider = exact_provider(&table, spec, DeltaAccessProfile::QueryServing).await?;
        tables.insert(
            spec.table_code,
            FabricTable {
                table_code: spec.table_code,
                path,
                delta: table,
                provider,
            },
        );
    }
    let mut fabric = WorkspaceFabric { namespace, tables };
    fabric.seed_control_rows(record, repository).await?;
    Ok(fabric)
}

async fn create_or_open(
    path: &Path,
    spec: &TableSpec,
    profile: DeltaAccessProfile,
) -> Result<DeltaTable, FabricError> {
    if profile != DeltaAccessProfile::OptimizeDml || profile.skip_stats() {
        return Err(FabricError::SnapshotProviderIntegrity(
            "table creation requires the OPTIMIZE_DML full-statistics profile".into(),
        ));
    }
    let url = LocalProviderFactory::file_url(path)?;
    let kernel: deltalake::kernel::StructType = spec.arrow_schema.as_ref().try_into_kernel()?;
    let mut configuration = HashMap::from([
        (
            SCHEMA_DIGEST_KEY.to_owned(),
            Some(spec.schema_digest.clone()),
        ),
        (
            "delta.enableChangeDataFeed".to_owned(),
            Some("false".to_owned()),
        ),
        (TYPE_WIDENING_KEY.to_owned(), Some("false".to_owned())),
        (
            "delta.enableDeletionVectors".to_owned(),
            Some("false".to_owned()),
        ),
        (
            "delta.checkpointInterval".to_owned(),
            Some(CHECKPOINT_INTERVAL.to_owned()),
        ),
        (
            "delta.logRetentionDuration".to_owned(),
            Some(LOG_RETENTION.to_owned()),
        ),
        (
            "delta.deletedFileRetentionDuration".to_owned(),
            Some(DELETED_FILE_RETENTION.to_owned()),
        ),
        (
            "delta.targetFileSize".to_owned(),
            Some(TARGET_FILE_SIZE_BYTES.to_owned()),
        ),
    ]);
    configuration.extend(
        spec.arrow_schema
            .metadata()
            .iter()
            .map(|(key, value)| (key.clone(), Some(value.clone()))),
    );
    Ok(CreateBuilder::new()
        .with_location(url.to_string())
        .with_table_name(spec.name)
        .with_comment(format!("CodeFabric {}: {}", spec.name, spec.grain))
        .with_save_mode(SaveMode::Ignore)
        .with_columns(kernel.fields().cloned())
        .with_partition_columns(spec.partition_columns.iter().copied())
        .with_configuration(configuration)
        .with_raise_if_key_not_exists(false)
        .await?)
}

fn constraints_for(spec: &TableSpec) -> BTreeMap<String, String> {
    let names = spec
        .arrow_schema
        .fields()
        .iter()
        .map(|field| field.name().as_str())
        .collect::<Vec<_>>();
    let mut constraints = BTreeMap::new();
    if names.contains(&"start_byte") && names.contains(&"end_byte") {
        constraints.insert(
            "source_span_ordered".into(),
            "start_byte >= 0 AND end_byte >= start_byte".into(),
        );
    }
    for name in names.iter().filter(|name| name.ends_with("_bucket")) {
        constraints.insert(format!("{name}_range"), format!("{name} BETWEEN 0 AND 255"));
    }
    for name in names.iter().filter(|name| name.ends_with("_count")) {
        constraints.insert(format!("{name}_nonnegative"), format!("{name} >= 0"));
    }
    constraints
}

async fn install_constraints(
    table: DeltaTable,
    spec: &TableSpec,
) -> Result<DeltaTable, FabricError> {
    let metadata = table.snapshot()?.metadata();
    let missing = constraints_for(spec)
        .into_iter()
        .filter(|(name, _)| {
            !metadata
                .configuration()
                .contains_key(&format!("delta.constraints.{name}"))
        })
        .collect::<HashMap<_, _>>();
    if missing.is_empty() {
        Ok(table)
    } else {
        Ok(table.add_constraint().with_constraints(missing).await?)
    }
}

fn validate_open_table(table: &DeltaTable, spec: &TableSpec) -> Result<(), FabricError> {
    let state = table.snapshot()?;
    let metadata = state.metadata();
    let configuration = metadata.configuration();
    validate_contract_identity(
        spec.name,
        configuration.get(SCHEMA_DIGEST_KEY).map(String::as_str),
        &spec.schema_digest,
        state.table_config().column_mapping_mode.is_some(),
    )?;
    if configuration
        .get("delta.enableChangeDataFeed")
        .map(String::as_str)
        != Some("false")
        || configuration.get(TYPE_WIDENING_KEY).map(String::as_str) != Some("false")
    {
        return Err(FabricError::TableInvariant {
            table: spec.name.into(),
            detail: "CDF and type widening must remain disabled".into(),
        });
    }
    if metadata.partition_columns()
        != spec
            .partition_columns
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    {
        return Err(FabricError::TableInvariant {
            table: spec.name.into(),
            detail: "partition columns differ from generated TableSpec".into(),
        });
    }
    for (key, value) in spec.arrow_schema.metadata() {
        if configuration.get(key) != Some(value) {
            return Err(FabricError::TableInvariant {
                table: spec.name.into(),
                detail: format!("generated schema metadata key {key} drifted"),
            });
        }
    }
    let opened: Schema = state.schema().as_ref().try_into_arrow()?;
    if opened.fields() != spec.arrow_schema.fields() {
        return Err(FabricError::TableInvariant {
            table: spec.name.into(),
            detail: "Delta StructType round trip changed Arrow fields".into(),
        });
    }
    Ok(())
}

fn validate_contract_identity(
    table: &str,
    actual_digest: Option<&str>,
    expected_digest: &str,
    column_mapping_enabled: bool,
) -> Result<(), FabricError> {
    if actual_digest != Some(expected_digest) {
        return Err(FabricError::SchemaDigestMismatch {
            table: table.into(),
        });
    }
    if column_mapping_enabled {
        return Err(FabricError::TableInvariant {
            table: table.into(),
            detail: "column mapping must remain none".into(),
        });
    }
    Ok(())
}

pub(super) async fn exact_provider(
    table: &DeltaTable,
    spec: &TableSpec,
    profile: DeltaAccessProfile,
) -> Result<Arc<dyn TableProvider>, FabricError> {
    if profile != DeltaAccessProfile::QueryServing || profile.skip_stats() {
        return Err(FabricError::SnapshotProviderIntegrity(
            "DataFusion providers require the QUERY_SERVING statistics profile".into(),
        ));
    }
    // Delta/DataFusion defaults to Arrow view types for Parquet scans. The governed
    // schema deliberately uses ordinary Utf8/Binary, so bind the provider to the
    // library's session option instead of maintaining a conversion layer.
    let config = SessionConfig::new().set_bool(
        "datafusion.execution.parquet.schema_force_view_types",
        false,
    );
    let session = Arc::new(SessionContext::new_with_config(config).state());
    let inner = table.table_provider().with_session(session).await?;
    let provider_schema = inner.schema();
    if provider_schema.fields() != spec.arrow_schema.fields() {
        return Err(FabricError::TableInvariant {
            table: spec.name.into(),
            detail: "DataFusion provider physical schema differs from generated TableSpec".into(),
        });
    }
    Ok(inner)
}

fn millis_to_micros(value: &str, field: &str) -> Result<i64, FabricError> {
    let millis = value
        .parse::<i64>()
        .map_err(|_| FabricError::TableInvariant {
            table: "workspace".into(),
            detail: format!("{field} is not the operational millisecond timestamp"),
        })?;
    millis
        .checked_mul(1_000)
        .ok_or_else(|| FabricError::TableInvariant {
            table: "workspace".into(),
            detail: format!("{field} timestamp overflow"),
        })
}

fn workspace_batch(record: &WorkspaceRecord) -> Result<RecordBatch, FabricError> {
    let spec = table_spec(1).expect("generated workspace table");
    let workspace_kind = if record.repository_id.is_some() {
        2_i16
    } else {
        1_i16
    };
    let revision =
        i64::try_from(record.registration_revision).map_err(|_| FabricError::TableInvariant {
            table: "workspace".into(),
            detail: "registration revision exceeds Int64".into(),
        })?;
    let created_at = millis_to_micros(&record.created_at, "created_at")?;
    let updated_at = millis_to_micros(&record.updated_at, "updated_at")?;
    let repository = record.repository_id.as_ref().map(<[u8; 16]>::as_slice);
    let worktree = record.worktree_id.as_ref().map(<[u8; 16]>::as_slice);
    let columns: Vec<ArrayRef> = vec![
        Arc::new(BinaryArray::from(vec![Some(
            record.workspace_id.as_slice(),
        )])),
        Arc::new(BinaryArray::from(vec![repository])),
        Arc::new(BinaryArray::from(vec![worktree])),
        Arc::new(Int16Array::from(vec![workspace_kind])),
        Arc::new(StringArray::from(vec![record.root_path_display.as_str()])),
        Arc::new(BinaryArray::from(vec![Some(
            record.root_path_bytes.as_slice(),
        )])),
        Arc::new(StringArray::from(vec![record.root_path_display.as_str()])),
        Arc::new(Int16Array::from(vec![1_i16])),
        Arc::new(BinaryArray::from(vec![Some(
            record.authorization_fingerprint.as_slice(),
        )])),
        Arc::new(Int16Array::from(vec![0_i16])),
        Arc::new(Int64Array::from(vec![revision])),
        Arc::new(TimestampMicrosecondArray::from(vec![created_at]).with_timezone("UTC")),
        Arc::new(TimestampMicrosecondArray::from(vec![updated_at]).with_timezone("UTC")),
    ];
    Ok(RecordBatch::try_new(
        Arc::clone(&spec.arrow_schema),
        columns,
    )?)
}

fn common_repository_batch(record: &CommonRepositoryRecord) -> Result<RecordBatch, FabricError> {
    let spec = table_spec(2).expect("generated common_repository table");
    let updated_at = millis_to_micros(&record.updated_at, "updated_at")?;
    let columns: Vec<ArrayRef> = vec![
        Arc::new(BinaryArray::from(vec![Some(
            record.repository_id.as_slice(),
        )])),
        Arc::new(BinaryArray::from(vec![Some(
            record.common_dir_path_bytes.as_slice(),
        )])),
        Arc::new(StringArray::from(vec![
            record.common_dir_path_display.as_str(),
        ])),
        Arc::new(Int16Array::from(vec![record.object_format_code])),
        Arc::new(BinaryArray::from(vec![Some(
            record.trust_policy_fingerprint.as_slice(),
        )])),
        Arc::new(TimestampMicrosecondArray::from(vec![updated_at]).with_timezone("UTC")),
    ];
    Ok(RecordBatch::try_new(
        Arc::clone(&spec.arrow_schema),
        columns,
    )?)
}

fn source_context_batch(record: &WorkspaceRecord) -> Result<RecordBatch, FabricError> {
    let spec = table_spec(3).expect("generated analysis_context table");
    let columns: Vec<ArrayRef> = vec![
        Arc::new(BinaryArray::from(vec![Some(
            record.workspace_id.as_slice(),
        )])),
        Arc::new(BinaryArray::from(vec![Some(SOURCE_CONTEXT_ID.as_slice())])),
        Arc::new(Int16Array::from(vec![1_i16])),
        Arc::new(BinaryArray::from(vec![Some(
            record.context_fingerprint.as_slice(),
        )])),
        Arc::new(StringArray::from(vec!["1.0"])),
        Arc::new(StringArray::from(vec!["source"])),
        Arc::new(StringArray::from(vec![None::<&str>])),
        Arc::new(BooleanArray::from(vec![true])),
    ];
    Ok(RecordBatch::try_new(
        Arc::clone(&spec.arrow_schema),
        columns,
    )?)
}

fn source_context_set_batch(record: &WorkspaceRecord) -> Result<RecordBatch, FabricError> {
    let spec = table_spec(4).expect("generated analysis_context_set table");
    let identity = context_set_identity(record.workspace_id, &[SOURCE_CONTEXT_ID])?;
    let mut contexts = ListBuilder::new(BinaryBuilder::new()).with_field(Arc::new(Field::new(
        "element",
        DataType::Binary,
        false,
    )));
    contexts.values().append_value(SOURCE_CONTEXT_ID);
    contexts.append(true);
    let created_at = millis_to_micros(&record.created_at, "created_at")?;
    let columns: Vec<ArrayRef> = vec![
        Arc::new(BinaryArray::from(vec![Some(identity.id.as_slice())])),
        Arc::new(BinaryArray::from(vec![Some(
            record.workspace_id.as_slice(),
        )])),
        Arc::new(contexts.finish()),
        Arc::new(BinaryArray::from(vec![Some(
            identity.full_digest.as_slice(),
        )])),
        Arc::new(TimestampMicrosecondArray::from(vec![created_at]).with_timezone("UTC")),
    ];
    Ok(RecordBatch::try_new(
        Arc::clone(&spec.arrow_schema),
        columns,
    )?)
}

fn digest_bytes(value: &str) -> Result<[u8; 32], FabricError> {
    let payload = value
        .strip_prefix("b3:")
        .filter(|payload| payload.len() == 64)
        .ok_or_else(|| FabricError::TableInvariant {
            table: "enum_catalog".into(),
            detail: "generated registry digest framing is invalid".into(),
        })?;
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&payload[index * 2..index * 2 + 2], 16).map_err(|_| {
            FabricError::TableInvariant {
                table: "enum_catalog".into(),
                detail: "generated registry digest hex is invalid".into(),
            }
        })?;
    }
    Ok(digest)
}

fn enum_catalog_batch() -> Result<RecordBatch, FabricError> {
    let spec = table_spec(11).expect("generated enum_catalog table");
    let row_count = crate::registries::REGISTRY_DOMAINS
        .iter()
        .map(|domain| domain.values.len())
        .sum::<usize>();
    let mut domains = Vec::with_capacity(row_count);
    let mut codes = Vec::with_capacity(row_count);
    let mut names = Vec::with_capacity(row_count);
    let mut versions = Vec::with_capacity(row_count);
    let mut digests = Vec::with_capacity(row_count);
    for domain in crate::registries::REGISTRY_DOMAINS {
        let digest = digest_bytes(domain.canonical_digest)?;
        for entry in domain.values {
            domains.push(domain.domain);
            codes.push(i32::from(entry.code));
            names.push(entry.name);
            versions.push(domain.version);
            digests.push(digest);
        }
    }
    let digest_refs = digests
        .iter()
        .map(|digest| Some(digest.as_slice()))
        .collect::<Vec<_>>();
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(domains)),
        Arc::new(Int32Array::from(codes)),
        Arc::new(StringArray::from(names)),
        Arc::new(StringArray::from(versions)),
        Arc::new(BinaryArray::from(digest_refs)),
    ];
    Ok(RecordBatch::try_new(
        Arc::clone(&spec.arrow_schema),
        columns,
    )?)
}

/// Validate that one application-owned Arrow schema has an exact Delta Kernel mapping.
pub(crate) fn validate_delta_schema(schema: &SchemaRef) -> Result<(), ArrowError> {
    let _: deltalake::kernel::StructType = schema.as_ref().try_into_kernel()?;
    Ok(())
}

/// Compute the semantic identity of the exact Delta `StructType` derived from Arrow.
///
/// # Errors
///
/// Returns an Arrow conversion, serialization, or canonicalization failure.
pub(crate) fn delta_schema_digest(schema: &SchemaRef) -> Result<String, FabricError> {
    let kernel: deltalake::kernel::StructType = schema.as_ref().try_into_kernel()?;
    let value = serde_json::to_value(kernel).map_err(|error| FabricError::TableInvariant {
        table: "generated-schema".into(),
        detail: format!("Delta StructType serialization failed: {error}"),
    })?;
    let canonical = crate::contracts::jcs::canonicalize_value(&value).map_err(|error| {
        FabricError::TableInvariant {
            table: "generated-schema".into(),
            detail: format!("Delta StructType canonicalization failed: {error}"),
        }
    })?;
    Ok(crate::contracts::jcs::checksum(&canonical))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use arrow_array::Int32Array;
    use datafusion::datasource::{MemTable, ViewTable, provider_as_source};
    use datafusion::logical_expr::{JoinType, LogicalPlanBuilder};

    use super::*;
    use crate::registries::WorkspaceRegistryLifecycle;

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("codefabric-{label}-{}-{nonce}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn record(revision: u64) -> WorkspaceRecord {
        WorkspaceRecord {
            workspace_id: [1; 16],
            workspace_registration_nonce: [2; 16],
            registration_revision: revision,
            administrative_key: vec![3],
            root_path_bytes: b"/workspace".to_vec(),
            root_path_display: "/workspace".into(),
            root_directory_file_identity: vec![4],
            platform_code: 2,
            case_sensitivity_mode: "sensitive".into(),
            authorization_revision: revision,
            allowed_source_disclosure_rules: Vec::new(),
            repository_id: None,
            worktree_id: None,
            authorization_fingerprint: [5; 32],
            context_fingerprint: [6; 32],
            status: WorkspaceRegistryLifecycle::Bootstrapping,
            created_at: "00000000000000001000".into(),
            updated_at: format!("{revision:020}"),
        }
    }

    #[tokio::test]
    async fn wp19_behavioral_acceptance() {
        let root = TestRoot::new("wp19-behavioral");
        let first = bootstrap_workspace(&root.0, &record(1)).await.unwrap();
        assert_eq!(first.tables().len(), table_specs().len());
        assert!(first.namespace.control.is_dir());
        assert!(first.namespace.facts.is_dir());
        assert!(first.namespace.derived.is_dir());
        assert!(first.tables().all(|table| {
            table.provider.schema().fields()
                == table_spec(table.table_code).unwrap().arrow_schema.fields()
        }));

        let reopened = bootstrap_workspace(&root.0, &record(2)).await.unwrap();
        assert_eq!(
            reopened.current_workspace_revision().await.unwrap(),
            Some(2)
        );
    }

    #[tokio::test]
    async fn wp19_structural_acceptance() {
        let root = TestRoot::new("wp19-structural");
        let fabric = bootstrap_workspace(&root.0, &record(1)).await.unwrap();
        for table in fabric.tables() {
            let spec = table_spec(table.table_code).unwrap();
            let state = table.delta.snapshot().unwrap();
            let configuration = state.metadata().configuration();
            assert_eq!(
                configuration.get(SCHEMA_DIGEST_KEY),
                Some(&spec.schema_digest)
            );
            assert!(state.table_config().column_mapping_mode.is_none());
            assert_eq!(
                state.metadata().partition_columns(),
                &spec
                    .partition_columns
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            );
            for (key, value) in spec.arrow_schema.metadata() {
                assert_eq!(configuration.get(key), Some(value));
            }
            for name in constraints_for(spec).keys() {
                assert!(configuration.contains_key(&format!("delta.constraints.{name}")));
            }
        }
    }

    #[test]
    fn wp19_negative_zero_state() {
        assert!(matches!(
            validate_contract_identity("workspace", Some("b3:wrong"), "b3:expected", false),
            Err(FabricError::SchemaDigestMismatch { .. })
        ));
        assert!(matches!(
            validate_contract_identity("workspace", Some("b3:expected"), "b3:expected", true),
            Err(FabricError::TableInvariant { .. })
        ));
        for location in [
            "s3://bucket/table",
            "az://container/table",
            "gs://bucket/table",
        ] {
            assert!(
                LocalProviderFactory::validate(&LocalProviderRequest {
                    location: location.into(),
                    ..LocalProviderRequest::default()
                })
                .is_err()
            );
        }
        assert!(
            LocalProviderFactory::validate(&LocalProviderRequest {
                location: "/tmp/codefabric".into(),
                endpoint: Some("https://example.invalid".into()),
                ..LocalProviderRequest::default()
            })
            .is_err()
        );
        assert!(
            LocalProviderFactory::validate(&LocalProviderRequest {
                location: "/tmp/codefabric".into(),
                credential: Some("secret".into()),
                ..LocalProviderRequest::default()
            })
            .is_err()
        );
        assert!(
            LocalProviderFactory::validate(&LocalProviderRequest {
                location: "/tmp/codefabric".into(),
                storage_options: BTreeMap::from([("region".into(), "test".into())]),
                ..LocalProviderRequest::default()
            })
            .is_err()
        );
    }

    #[tokio::test]
    async fn wp19_operational_acceptance() {
        let root = TestRoot::new("wp19-operational");
        let first = bootstrap_workspace(&root.0, &record(1)).await.unwrap();
        let versions = first
            .tables()
            .map(|table| (table.table_code, table.delta.version()))
            .collect::<BTreeMap<_, _>>();
        let second = bootstrap_workspace(&root.0, &record(1)).await.unwrap();
        assert!(second.tables().all(|table| {
            versions.get(&table.table_code).copied() == Some(table.delta.version())
        }));
    }

    #[tokio::test]
    async fn wp19_programmatic_view_and_anti_join_probe() {
        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, false)]));
        let base = Arc::new(
            MemTable::try_new(
                Arc::clone(&schema),
                vec![vec![
                    RecordBatch::try_new(
                        Arc::clone(&schema),
                        vec![Arc::new(Int32Array::from(vec![1, 2, 3]))],
                    )
                    .unwrap(),
                ]],
            )
            .unwrap(),
        );
        let tombstone = Arc::new(
            MemTable::try_new(
                Arc::clone(&schema),
                vec![vec![
                    RecordBatch::try_new(
                        Arc::clone(&schema),
                        vec![Arc::new(Int32Array::from(vec![2]))],
                    )
                    .unwrap(),
                ]],
            )
            .unwrap(),
        );
        let right = LogicalPlanBuilder::scan("tombstone", provider_as_source(tombstone), None)
            .unwrap()
            .build()
            .unwrap();
        let effective = LogicalPlanBuilder::scan("base", provider_as_source(base), None)
            .unwrap()
            .join(right, JoinType::LeftAnti, (vec!["id"], vec!["id"]), None)
            .unwrap()
            .build()
            .unwrap();
        let view = Arc::new(ViewTable::new(effective, None));
        let context = SessionContext::new();
        let rows = context.read_table(view).unwrap().collect().await.unwrap();
        assert_eq!(rows.iter().map(RecordBatch::num_rows).sum::<usize>(), 2);
    }

    #[test]
    fn wp09_delta_conversion_stays_inside_the_fabric_boundary() {
        for table in table_specs() {
            validate_delta_schema(&table.arrow_schema).unwrap();
        }
    }
}
