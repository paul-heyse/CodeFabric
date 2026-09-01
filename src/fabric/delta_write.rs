//! Command-owned, zero-retry Delta writes from session-bound DataFusion plans.
//!
//! This module is intentionally narrower than delta-rs' full mutation surface. It accepts
//! only [`WriteBuilder`](deltalake::operations::write::WriteBuilder) plan writes against a
//! loaded exact predecessor, always injects the concrete DataFusion session which owns the
//! plan, and always selects `SessionFallbackPolicy::RequireSessionState` and
//! `CommitProperties::with_max_retries(0)`. Conflicts and ambiguous failures are observations
//! for the `FabricCommand` reducer; this boundary never reloads latest state or retries.

use std::collections::BTreeMap;
use std::fmt;
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Arc;

use arrow_array::RecordBatch;
use datafusion::dataframe::DataFrame;
use datafusion::execution::SessionState;
use datafusion::logical_expr::LogicalPlan;
use deltalake::TableProperty;
use deltalake::delta_datafusion::SessionFallbackPolicy;
use deltalake::kernel::Transaction;
use deltalake::kernel::transaction::{CommitProperties, TransactionError};
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;
use deltalake::{DeltaTable, DeltaTableError};
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use serde_json::Value;
use thiserror::Error;

use super::command::{OperationId, TransactionRef, WriterGeneration};
use super::delta_exact::{ExactDeltaPin, read_exact_commit_entry};

const META_OPERATION_ID: &str = "codefabric.operation_id";
const META_WRITER_GENERATION: &str = "codefabric.writer_generation";
const META_EXPECTED_ROOT: &str = "codefabric.expected_predecessor_root";
const META_EXPECTED_VERSION: &str = "codefabric.expected_predecessor_version";
const META_SESSION_ID: &str = "codefabric.session_id";
const META_APPLICATION_ID: &str = "codefabric.application_id";
const META_APPLICATION_VERSION: &str = "codefabric.application_version";
const META_WRITE_PRIMITIVE: &str = "codefabric.write_primitive";
const META_TARGET_FILE_SIZE_BYTES: &str = "codefabric.target_file_size_bytes";
const META_WRITE_BATCH_ROWS: &str = "codefabric.write_batch_rows";
const META_MAX_ROW_GROUP_ROWS: &str = "codefabric.max_row_group_rows";
const META_MAX_ROW_GROUP_BYTES: &str = "codefabric.max_row_group_bytes";
const META_PARQUET_COMPRESSION: &str = "codefabric.parquet_compression";
const CODEFABRIC_METADATA_PREFIX: &str = "codefabric.";
const OPERATION_METRICS: &str = "operationMetrics";
const NUM_RETRIES: &str = "num_retries";

const DEFAULT_TARGET_FILE_SIZE_BYTES: u64 = 128 * 1024 * 1024;
const DEFAULT_WRITE_BATCH_ROWS: usize = 65_536;
const DEFAULT_MAX_ROW_GROUP_ROWS: usize = 65_536;
const DEFAULT_MAX_ROW_GROUP_BYTES: u64 = 64 * 1024 * 1024;

/// Required creation-time properties for a durable append-only Delta history.
///
/// Applying this contract to [`CreateBuilder`] enables CDF at version zero,
/// selects explicit data-skipping columns, and disables deletion vectors. The
/// caller may add retention/checkpoint properties appropriate to the relation,
/// but cannot defer these correctness-relevant properties to a later commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlledDeltaHistoryProperties {
    statistics_columns: String,
}

impl ControlledDeltaHistoryProperties {
    /// Construct the mandatory creation contract.
    ///
    /// # Errors
    ///
    /// Rejects an empty comma-separated statistics-column selection.
    pub fn try_new(
        statistics_columns: impl Into<String>,
    ) -> Result<Self, ControlledDeltaWriteInputError> {
        let statistics_columns = statistics_columns.into();
        if statistics_columns
            .split(',')
            .all(|column| column.trim().is_empty())
        {
            return Err(ControlledDeltaWriteInputError::EmptyStatisticsColumns);
        }
        Ok(Self { statistics_columns })
    }

    /// Exact comma-separated statistics columns set at table creation.
    #[must_use]
    pub fn statistics_columns(&self) -> &str {
        &self.statistics_columns
    }

    /// Apply mandatory properties without replacing relation-specific
    /// retention/checkpoint configuration already attached to the builder.
    #[must_use]
    pub fn apply_to(self, builder: CreateBuilder) -> CreateBuilder {
        builder
            .with_configuration_property(TableProperty::AppendOnly, Some("true"))
            .with_configuration_property(TableProperty::EnableChangeDataFeed, Some("true"))
            .with_configuration_property(
                TableProperty::DataSkippingStatsColumns,
                Some(self.statistics_columns),
            )
            .with_configuration_property(TableProperty::EnableDeletionVectors, Some("false"))
    }
}

/// Explicit physical policy for one controlled Delta append/replacement.
///
/// These settings affect layout and cost only, never logical identity. The
/// values are recorded in the exact commit entry and supplied directly to
/// delta-rs/Parquet; no library default participates in a controlled write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlledDeltaWriteLayout {
    target_file_size_bytes: NonZeroU64,
    write_batch_rows: NonZeroUsize,
    max_row_group_rows: NonZeroUsize,
    max_row_group_bytes: NonZeroUsize,
}

impl Default for ControlledDeltaWriteLayout {
    fn default() -> Self {
        Self {
            target_file_size_bytes: NonZeroU64::new(DEFAULT_TARGET_FILE_SIZE_BYTES)
                .expect("controlled target file size is nonzero"),
            write_batch_rows: NonZeroUsize::new(DEFAULT_WRITE_BATCH_ROWS)
                .expect("controlled write batch is nonzero"),
            max_row_group_rows: NonZeroUsize::new(DEFAULT_MAX_ROW_GROUP_ROWS)
                .expect("controlled row group count is nonzero"),
            max_row_group_bytes: NonZeroUsize::new(
                usize::try_from(DEFAULT_MAX_ROW_GROUP_BYTES)
                    .expect("controlled row group byte size fits usize"),
            )
            .expect("controlled row group byte size is nonzero"),
        }
    }
}

impl ControlledDeltaWriteLayout {
    /// Construct a fully explicit layout policy. Non-zero types exclude the
    /// invalid zero limits rejected by Parquet and delta-rs.
    #[must_use]
    pub const fn new(
        target_file_size_bytes: NonZeroU64,
        write_batch_rows: NonZeroUsize,
        max_row_group_rows: NonZeroUsize,
        max_row_group_bytes: NonZeroUsize,
    ) -> Self {
        Self {
            target_file_size_bytes,
            write_batch_rows,
            max_row_group_rows,
            max_row_group_bytes,
        }
    }

    #[must_use]
    pub const fn target_file_size_bytes(self) -> NonZeroU64 {
        self.target_file_size_bytes
    }

    #[must_use]
    pub const fn write_batch_rows(self) -> NonZeroUsize {
        self.write_batch_rows
    }

    #[must_use]
    pub const fn max_row_group_rows(self) -> NonZeroUsize {
        self.max_row_group_rows
    }

    #[must_use]
    pub const fn max_row_group_bytes(self) -> NonZeroUsize {
        self.max_row_group_bytes
    }

    /// Stable compression identity supplied to every Parquet column.
    #[must_use]
    pub const fn parquet_compression(self) -> &'static str {
        "zstd"
    }

    fn writer_properties(self) -> WriterProperties {
        WriterProperties::builder()
            .set_compression(Compression::ZSTD(Default::default()))
            .set_write_batch_size(self.write_batch_rows.get())
            .set_max_row_group_row_count(Some(self.max_row_group_rows.get()))
            .set_max_row_group_bytes(Some(self.max_row_group_bytes.get()))
            .build()
    }
}

/// The two high-level plan-write primitives admitted at this boundary.
///
/// Predicate rewrites, row-level DML, and retrying maintenance builders are deliberately absent.
/// Compaction is a full replacement produced by a proved DataFusion plan and is therefore
/// represented by [`Self::ReplaceAll`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlledDeltaWriteMode {
    /// Add the plan's rows while retaining the exact predecessor's active rows.
    Append,
    /// Replace every active row with the plan's rows.
    ReplaceAll,
}

impl ControlledDeltaWriteMode {
    const fn save_mode(self) -> SaveMode {
        match self {
            Self::Append => SaveMode::Append,
            Self::ReplaceAll => SaveMode::Overwrite,
        }
    }

    /// Stable commit-metadata spelling for the selected primitive.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Append => "write-builder-append",
            Self::ReplaceAll => "write-builder-replace-all",
        }
    }
}

/// Application transaction action used to recognize an already-committed command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationTransactionMarker {
    application_id: String,
    application_version: i64,
}

impl ApplicationTransactionMarker {
    /// Derive the one canonical Delta transaction marker for an application
    /// transaction identity.
    ///
    /// Each application-owned transaction has its own marker stream at
    /// version zero. Repeating the same transaction therefore resolves to the
    /// same Delta `txn` action on every target table, while a different
    /// transaction cannot alias it through a caller-selected string/version.
    #[must_use]
    pub fn from_transaction_ref(transaction: TransactionRef) -> Self {
        Self {
            application_id: format!("codefabric/transaction/{}", hex(transaction.as_bytes())),
            application_version: 0,
        }
    }

    /// Construct a nonempty, nonnegative application marker.
    ///
    /// # Errors
    ///
    /// Rejects an empty application ID or a negative application version. The caller owns the
    /// stronger rule that versions are monotonic within one application-ID stream.
    pub fn new(
        application_id: impl Into<String>,
        application_version: i64,
    ) -> Result<Self, ControlledDeltaWriteInputError> {
        let application_id = application_id.into();
        if application_id.trim().is_empty() {
            return Err(ControlledDeltaWriteInputError::EmptyApplicationId);
        }
        if application_version < 0 {
            return Err(ControlledDeltaWriteInputError::NegativeApplicationVersion(
                application_version,
            ));
        }
        Ok(Self {
            application_id,
            application_version,
        })
    }

    /// Stable application-ID namespace written into Delta's `txn` action.
    #[must_use]
    pub fn application_id(&self) -> &str {
        &self.application_id
    }

    /// Monotonic application version written into Delta's `txn` action.
    #[must_use]
    pub const fn application_version(&self) -> i64 {
        self.application_version
    }
}

/// Complete command-owned transaction contract for one plan write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlledDeltaWriteSpec {
    predecessor: ExactDeltaPin,
    operation_id: OperationId,
    writer_generation: WriterGeneration,
    marker: ApplicationTransactionMarker,
    mode: ControlledDeltaWriteMode,
    layout: ControlledDeltaWriteLayout,
    commit_metadata: BTreeMap<String, Value>,
}

impl ControlledDeltaWriteSpec {
    /// Bind one operation and writer generation to one exact table predecessor.
    #[must_use]
    pub fn new(
        predecessor: ExactDeltaPin,
        operation_id: OperationId,
        writer_generation: WriterGeneration,
        marker: ApplicationTransactionMarker,
        mode: ControlledDeltaWriteMode,
    ) -> Self {
        Self {
            predecessor,
            operation_id,
            writer_generation,
            marker,
            mode,
            layout: ControlledDeltaWriteLayout::default(),
            commit_metadata: BTreeMap::new(),
        }
    }

    /// Select an explicit measured physical layout without changing the
    /// transaction's logical identity.
    #[must_use]
    pub fn with_layout(mut self, layout: ControlledDeltaWriteLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Add application-owned metadata which is written and read back as part
    /// of the exact commit proof.
    ///
    /// The generic seam is intentionally restricted to the `codefabric.`
    /// namespace and cannot replace the transaction boundary's reserved keys.
    /// This lets callers persist compact materialization/provenance evidence
    /// while keeping the operation, fence, predecessor, session, marker, and
    /// primitive fields authoritative here.
    pub fn with_commit_metadata(
        mut self,
        metadata: BTreeMap<String, Value>,
    ) -> Result<Self, ControlledDeltaWriteInputError> {
        for (key, value) in metadata {
            if !key.starts_with(CODEFABRIC_METADATA_PREFIX) {
                return Err(ControlledDeltaWriteInputError::InvalidCommitMetadataKey(
                    key,
                ));
            }
            if reserved_commit_metadata_key(&key) {
                return Err(ControlledDeltaWriteInputError::ReservedCommitMetadataKey(
                    key,
                ));
            }
            if value.is_null() {
                return Err(ControlledDeltaWriteInputError::NullCommitMetadataValue(key));
            }
            if self.commit_metadata.insert(key.clone(), value).is_some() {
                return Err(ControlledDeltaWriteInputError::DuplicateCommitMetadataKey(
                    key,
                ));
            }
        }
        Ok(self)
    }

    /// Exact root/version against which the write was authored.
    #[must_use]
    pub const fn predecessor(&self) -> &ExactDeltaPin {
        &self.predecessor
    }

    /// Command operation identity recorded in commit metadata.
    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    /// Monotonic single-writer fence recorded in commit metadata.
    #[must_use]
    pub const fn writer_generation(&self) -> WriterGeneration {
        self.writer_generation
    }

    /// Application transaction marker used for durable reconciliation.
    #[must_use]
    pub const fn marker(&self) -> &ApplicationTransactionMarker {
        &self.marker
    }

    /// Selected high-level write primitive.
    #[must_use]
    pub const fn mode(&self) -> ControlledDeltaWriteMode {
        self.mode
    }

    /// Explicit target-file, row-group, batch, and compression policy.
    #[must_use]
    pub const fn layout(&self) -> ControlledDeltaWriteLayout {
        self.layout
    }

    /// Application-owned metadata included in the durable commit/readback
    /// contract, excluding the reserved transaction fields.
    #[must_use]
    pub const fn commit_metadata(&self) -> &BTreeMap<String, Value> {
        &self.commit_metadata
    }
}

/// A logical plan proven to travel with the concrete session snapshot that produced it.
///
/// DataFusion's `DataFrame::into_parts` is the public API which preserves the plan together
/// with its `SessionState`. The constructor additionally compares that state with the exact
/// epoch state supplied by the caller. Private fields prevent later plan/session substitution.
pub struct SessionBoundLogicalPlan {
    session: Arc<SessionState>,
    plan: LogicalPlan,
    session_id: String,
}

impl fmt::Debug for SessionBoundLogicalPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionBoundLogicalPlan")
            .field("session_id", &self.session_id)
            .field("plan_schema", self.plan.schema())
            .finish_non_exhaustive()
    }
}

impl SessionBoundLogicalPlan {
    /// Consume a DataFrame and prove it carries the supplied epoch session identity.
    ///
    /// Session ID, runtime environment, and catalog-list identity are all checked. The latter
    /// two pointer checks distinguish independently constructed states that happen to reuse a
    /// textual session ID.
    ///
    /// # Errors
    ///
    /// Returns [`ControlledDeltaWriteInputError::PlanSessionMismatch`] when the DataFrame did
    /// not originate from the supplied epoch state.
    pub fn try_from_dataframe(
        epoch_session: Arc<SessionState>,
        dataframe: DataFrame,
    ) -> Result<Self, ControlledDeltaWriteInputError> {
        let (plan_session, plan) = dataframe.into_parts();
        let same_session_id = epoch_session.session_id() == plan_session.session_id();
        let same_runtime = Arc::ptr_eq(epoch_session.runtime_env(), plan_session.runtime_env());
        let same_catalog = Arc::ptr_eq(epoch_session.catalog_list(), plan_session.catalog_list());
        if !(same_session_id && same_runtime && same_catalog) {
            return Err(ControlledDeltaWriteInputError::PlanSessionMismatch {
                expected_session_id: epoch_session.session_id().to_owned(),
                observed_session_id: plan_session.session_id().to_owned(),
                same_runtime,
                same_catalog,
            });
        }

        Ok(Self {
            session_id: epoch_session.session_id().to_owned(),
            session: epoch_session,
            plan,
        })
    }

    /// Exact DataFusion session ID carried into commit readback evidence.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

/// Invalid application-owned input rejected before any Delta write is constructed.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum ControlledDeltaWriteInputError {
    /// Durable history creation must explicitly select statistics columns.
    #[error("controlled Delta history statistics columns must be nonempty")]
    EmptyStatisticsColumns,
    /// Delta transaction application IDs must be nonempty.
    #[error("Delta application transaction ID must be nonempty")]
    EmptyApplicationId,
    /// CodeFabric uses nonnegative monotonic application versions.
    #[error("Delta application transaction version must be nonnegative, got {0}")]
    NegativeApplicationVersion(i64),
    /// The plan and the required epoch session are not the same DataFusion state.
    #[error(
        "logical plan session mismatch: expected {expected_session_id}, observed {observed_session_id}, same_runtime={same_runtime}, same_catalog={same_catalog}"
    )]
    PlanSessionMismatch {
        expected_session_id: String,
        observed_session_id: String,
        same_runtime: bool,
        same_catalog: bool,
    },
    /// Caller metadata must live in the application-owned namespace.
    #[error("controlled Delta commit metadata key is outside codefabric namespace: {0}")]
    InvalidCommitMetadataKey(String),
    /// Transaction-boundary keys cannot be supplied or replaced by callers.
    #[error("controlled Delta commit metadata key is reserved: {0}")]
    ReservedCommitMetadataKey(String),
    /// One metadata key must have exactly one asserted value.
    #[error("controlled Delta commit metadata key was supplied more than once: {0}")]
    DuplicateCommitMetadataKey(String),
    /// Null is not accepted as durable proof evidence.
    #[error("controlled Delta commit metadata value must be non-null: {0}")]
    NullCommitMetadataValue(String),
}

/// How the introducing Delta commit version for an application marker was proved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkerCommitVersionEvidence {
    /// The marker was read directly from the named commit entry's `txn` action.
    ExactCommitEntry(u64),
    /// `transaction_version` proves marker visibility at an exact snapshot but does not expose
    /// the Delta commit version which first introduced that marker. This is used only when the
    /// marker was already present before a new write attempt.
    NotExposedByPinnedSnapshotApi,
}

/// Exact-snapshot evidence that one application marker is durable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationMarkerEvidence {
    marker: ApplicationTransactionMarker,
    observed_in: ExactDeltaPin,
    commit_version_evidence: MarkerCommitVersionEvidence,
}

impl ApplicationMarkerEvidence {
    /// Marker value observed through Delta's transaction-log snapshot API.
    #[must_use]
    pub const fn marker(&self) -> &ApplicationTransactionMarker {
        &self.marker
    }

    /// Exact loaded snapshot in which the marker was visible.
    #[must_use]
    pub const fn observed_in(&self) -> &ExactDeltaPin {
        &self.observed_in
    }

    /// Truthful limit of the pinned marker lookup API.
    #[must_use]
    pub const fn commit_version_evidence(&self) -> MarkerCommitVersionEvidence {
        self.commit_version_evidence
    }
}

/// What the selected commit's `commitInfo` action exposed for the write's read version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitReadVersionEvidence {
    /// `commitInfo.readVersion` exposed the exact expected predecessor version.
    Exact(u64),
    /// `commitInfo` omitted `readVersion`; exact application metadata and the returned
    /// predecessor+1 snapshot remain the binding evidence.
    NotExposedByCommitHistory,
}

/// Successful controlled commit plus its exact returned table handle.
#[derive(Debug)]
pub struct CommittedDeltaWrite {
    table: DeltaTable,
    predecessor: ExactDeltaPin,
    committed: ExactDeltaPin,
    operation_id: OperationId,
    writer_generation: WriterGeneration,
    layout: ControlledDeltaWriteLayout,
    marker_evidence: ApplicationMarkerEvidence,
    session_id: String,
    read_version_evidence: CommitReadVersionEvidence,
    num_retries: u64,
    commit_metadata: BTreeMap<String, Value>,
}

impl CommittedDeltaWrite {
    /// Exact loaded predecessor used by the write builder.
    #[must_use]
    pub const fn predecessor(&self) -> &ExactDeltaPin {
        &self.predecessor
    }

    /// Exact root/version returned by the successful Delta commit.
    #[must_use]
    pub const fn committed(&self) -> &ExactDeltaPin {
        &self.committed
    }

    /// Operation identity read back from the exact committed log entry.
    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    /// Writer generation read back from the exact committed log entry.
    #[must_use]
    pub const fn writer_generation(&self) -> WriterGeneration {
        self.writer_generation
    }

    /// Physical policy read back from the exact committed log entry.
    #[must_use]
    pub const fn layout(&self) -> ControlledDeltaWriteLayout {
        self.layout
    }

    /// Application marker read back from the exact committed snapshot.
    #[must_use]
    pub const fn marker_evidence(&self) -> &ApplicationMarkerEvidence {
        &self.marker_evidence
    }

    /// Concrete session ID recorded and read back from the commit.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Evidence exposed by exact `commitInfo` for the predecessor read version.
    #[must_use]
    pub const fn read_version_evidence(&self) -> CommitReadVersionEvidence {
        self.read_version_evidence
    }

    /// Delta write retry count read from operation metrics. This boundary accepts only zero.
    #[must_use]
    pub const fn num_retries(&self) -> u64 {
        self.num_retries
    }

    /// Application-owned metadata whose exact values were read back from the
    /// committed log entry.
    #[must_use]
    pub const fn commit_metadata(&self) -> &BTreeMap<String, Value> {
        &self.commit_metadata
    }

    /// Consume the evidence wrapper and retain the exact newly committed table handle.
    #[must_use]
    pub fn into_table(self) -> DeltaTable {
        self.table
    }
}

/// Conflict which cannot be internally rebased or retried by this write boundary.
#[derive(Debug)]
pub enum ControlledDeltaWriteConflict {
    /// The supplied table handle does not represent the command's exact predecessor.
    PredecessorMismatch {
        expected: ExactDeltaPin,
        observed: Option<ExactDeltaPin>,
    },
    /// The marker stream already advanced beyond this command's version.
    ApplicationTransactionAdvanced {
        application_id: String,
        requested_version: i64,
        observed_version: i64,
        observed_in: ExactDeltaPin,
    },
    /// The target version was claimed or the predecessor advanced before the zero-retry commit.
    CommitCollision {
        predecessor: ExactDeltaPin,
        target_version: u64,
        source: DeltaTableError,
    },
}

/// Stage at which the outcome became unprovable without command-owned reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlledDeltaWriteUnknownStage {
    ObservePredecessor,
    ReadMarkerBeforeWrite,
    ExecuteWrite,
    ObserveCommittedIdentity,
    ReadMarkerAfterWrite,
    ReadCommitHistory,
    ValidateCommitReadback,
}

/// Ambiguous or unproved write outcome returned to `FabricCommand` reconciliation.
#[derive(Debug)]
pub struct ControlledDeltaWriteUnknown {
    stage: ControlledDeltaWriteUnknownStage,
    predecessor: ExactDeltaPin,
    detail: String,
    source: Option<DeltaTableError>,
}

impl ControlledDeltaWriteUnknown {
    /// Stage whose durable evidence must be reconstructed by the command actor.
    #[must_use]
    pub const fn stage(&self) -> ControlledDeltaWriteUnknownStage {
        self.stage
    }

    /// Exact predecessor from the command's transaction contract.
    #[must_use]
    pub const fn predecessor(&self) -> &ExactDeltaPin {
        &self.predecessor
    }

    /// Bounded diagnostic explaining which proof was unavailable.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// Preserve the typed delta-rs error when one exists.
    #[must_use]
    pub const fn source(&self) -> Option<&DeltaTableError> {
        self.source.as_ref()
    }
}

/// Durable outcome which requires the command actor to reconcile before any new attempt.
#[derive(Debug)]
pub enum ControlledDeltaWriteReconciliation {
    Conflict(ControlledDeltaWriteConflict),
    Unknown(ControlledDeltaWriteUnknown),
}

/// Exhaustive result of one and only one controlled Delta write attempt.
#[derive(Debug)]
pub enum ControlledDeltaWriteOutcome {
    /// One new exact Delta version committed and all local readback proofs matched.
    Committed(CommittedDeltaWrite),
    /// The same application marker was already visible in the supplied exact predecessor.
    MarkerAlreadyCommitted(ApplicationMarkerEvidence),
    /// No internal retry is legal; the command actor must reconcile durable state.
    Reconcile(ControlledDeltaWriteReconciliation),
}

/// Execute one zero-retry `WriteBuilder` attempt from a session-bound logical plan.
///
/// The function never reloads table state, discovers the backing-store head, invokes DML or
/// maintenance builders, or calls itself recursively. Pre-write marker inspection is performed
/// against the already-loaded exact predecessor; after success, both `commitInfo` and `txn` are
/// read from the returned version's exact JSON log entry.
pub async fn write_exact_delta_plan(
    table: &DeltaTable,
    spec: &ControlledDeltaWriteSpec,
    input: SessionBoundLogicalPlan,
) -> ControlledDeltaWriteOutcome {
    let observed_predecessor = match observed_pin(table) {
        Ok(pin) => pin,
        Err(detail) => {
            return unknown(
                spec,
                ControlledDeltaWriteUnknownStage::ObservePredecessor,
                detail,
                None,
            );
        }
    };
    if &observed_predecessor != spec.predecessor() {
        return ControlledDeltaWriteOutcome::Reconcile(
            ControlledDeltaWriteReconciliation::Conflict(
                ControlledDeltaWriteConflict::PredecessorMismatch {
                    expected: spec.predecessor.clone(),
                    observed: Some(observed_predecessor),
                },
            ),
        );
    }

    match read_marker(table, spec.marker()).await {
        Ok(Some(observed)) if observed == spec.marker.application_version => {
            return ControlledDeltaWriteOutcome::MarkerAlreadyCommitted(marker_evidence(
                spec.marker.clone(),
                observed_predecessor,
                MarkerCommitVersionEvidence::NotExposedByPinnedSnapshotApi,
            ));
        }
        Ok(Some(observed)) if observed > spec.marker.application_version => {
            return ControlledDeltaWriteOutcome::Reconcile(
                ControlledDeltaWriteReconciliation::Conflict(
                    ControlledDeltaWriteConflict::ApplicationTransactionAdvanced {
                        application_id: spec.marker.application_id.clone(),
                        requested_version: spec.marker.application_version,
                        observed_version: observed,
                        observed_in: observed_predecessor,
                    },
                ),
            );
        }
        Ok(_) => {}
        Err(source) => {
            return unknown(
                spec,
                ControlledDeltaWriteUnknownStage::ReadMarkerBeforeWrite,
                "failed to read the application transaction marker from the exact predecessor",
                Some(source),
            );
        }
    }

    let SessionBoundLogicalPlan {
        session,
        plan,
        session_id,
    } = input;
    let commit_properties = controlled_commit_properties(spec, &session_id);
    let layout = spec.layout();
    let write = table
        .clone()
        .write(std::iter::empty::<RecordBatch>())
        .with_input_plan(plan)
        .with_session_state(session)
        .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
        .with_save_mode(spec.mode.save_mode())
        .with_target_file_size(Some(layout.target_file_size_bytes()))
        .with_write_batch_size(layout.write_batch_rows().get())
        .with_writer_properties(layout.writer_properties())
        .with_commit_properties(commit_properties);

    let committed_table = match write.await {
        Ok(table) => table,
        Err(source) => return classify_write_failure(spec, source),
    };

    let committed_pin = match observed_pin(&committed_table) {
        Ok(pin) => pin,
        Err(detail) => {
            return unknown(
                spec,
                ControlledDeltaWriteUnknownStage::ObserveCommittedIdentity,
                detail,
                None,
            );
        }
    };
    let expected_version = match spec.predecessor.version().checked_add(1) {
        Some(version) => version,
        None => {
            return unknown(
                spec,
                ControlledDeltaWriteUnknownStage::ObserveCommittedIdentity,
                "predecessor version overflowed while deriving the only legal target version",
                None,
            );
        }
    };
    if committed_pin.canonical_root() != spec.predecessor.canonical_root()
        || committed_pin.version() != expected_version
    {
        return unknown(
            spec,
            ControlledDeltaWriteUnknownStage::ObserveCommittedIdentity,
            format!(
                "write returned {}, expected {}@{}",
                pin_label(&committed_pin),
                spec.predecessor.canonical_root(),
                expected_version
            ),
            None,
        );
    }

    let readback = match readback_commit(&committed_table, spec, &session_id).await {
        Ok(readback) => readback,
        Err(unknown) => {
            return ControlledDeltaWriteOutcome::Reconcile(
                ControlledDeltaWriteReconciliation::Unknown(unknown),
            );
        }
    };

    ControlledDeltaWriteOutcome::Committed(CommittedDeltaWrite {
        table: committed_table,
        predecessor: spec.predecessor.clone(),
        committed: committed_pin.clone(),
        operation_id: spec.operation_id,
        writer_generation: spec.writer_generation,
        layout: spec.layout,
        marker_evidence: marker_evidence(
            spec.marker.clone(),
            committed_pin,
            MarkerCommitVersionEvidence::ExactCommitEntry(readback.transaction_commit_version),
        ),
        session_id,
        read_version_evidence: readback.read_version_evidence,
        num_retries: readback.num_retries,
        commit_metadata: spec.commit_metadata.clone(),
    })
}

/// Reconstruct the same exact commit evidence after restart from a table
/// loaded at the command's committed version.
///
/// The caller supplies the original session identity recorded in the durable
/// control row. This function validates the loaded root/version, transaction
/// marker, application commit metadata, predecessor read version, and zero
/// retry count. It never writes, refreshes, or resolves latest state.
pub(crate) async fn readback_exact_delta_commit(
    committed_table: &DeltaTable,
    spec: &ControlledDeltaWriteSpec,
    recorded_session_id: &str,
) -> Result<CommittedDeltaWrite, ControlledDeltaWriteUnknown> {
    let committed_pin = observed_pin(committed_table).map_err(|detail| {
        unknown_value(
            spec,
            ControlledDeltaWriteUnknownStage::ObserveCommittedIdentity,
            detail,
            None,
        )
    })?;
    let expected_version = spec.predecessor.version().checked_add(1).ok_or_else(|| {
        unknown_value(
            spec,
            ControlledDeltaWriteUnknownStage::ObserveCommittedIdentity,
            "predecessor version overflowed while reconstructing committed identity",
            None,
        )
    })?;
    if committed_pin.canonical_root() != spec.predecessor.canonical_root()
        || committed_pin.version() != expected_version
    {
        return Err(unknown_value(
            spec,
            ControlledDeltaWriteUnknownStage::ObserveCommittedIdentity,
            format!(
                "loaded restart snapshot {}, expected {}@{}",
                pin_label(&committed_pin),
                spec.predecessor.canonical_root(),
                expected_version
            ),
            None,
        ));
    }
    let readback = readback_commit(committed_table, spec, recorded_session_id).await?;
    Ok(CommittedDeltaWrite {
        table: committed_table.clone(),
        predecessor: spec.predecessor.clone(),
        committed: committed_pin.clone(),
        operation_id: spec.operation_id,
        writer_generation: spec.writer_generation,
        layout: spec.layout,
        marker_evidence: marker_evidence(
            spec.marker.clone(),
            committed_pin,
            MarkerCommitVersionEvidence::ExactCommitEntry(readback.transaction_commit_version),
        ),
        session_id: recorded_session_id.to_owned(),
        read_version_evidence: readback.read_version_evidence,
        num_retries: readback.num_retries,
        commit_metadata: spec.commit_metadata.clone(),
    })
}

fn controlled_commit_properties(
    spec: &ControlledDeltaWriteSpec,
    session_id: &str,
) -> CommitProperties {
    let mut metadata = BTreeMap::from([
        (
            META_OPERATION_ID.to_owned(),
            Value::String(hex(spec.operation_id.as_bytes())),
        ),
        (
            META_WRITER_GENERATION.to_owned(),
            Value::from(spec.writer_generation.get()),
        ),
        (
            META_EXPECTED_ROOT.to_owned(),
            Value::String(spec.predecessor.canonical_root().to_string()),
        ),
        (
            META_EXPECTED_VERSION.to_owned(),
            Value::from(spec.predecessor.version()),
        ),
        (
            META_SESSION_ID.to_owned(),
            Value::String(session_id.to_owned()),
        ),
        (
            META_APPLICATION_ID.to_owned(),
            Value::String(spec.marker.application_id.clone()),
        ),
        (
            META_APPLICATION_VERSION.to_owned(),
            Value::from(spec.marker.application_version),
        ),
        (
            META_WRITE_PRIMITIVE.to_owned(),
            Value::String(spec.mode.as_str().to_owned()),
        ),
        (
            META_TARGET_FILE_SIZE_BYTES.to_owned(),
            Value::from(spec.layout.target_file_size_bytes().get()),
        ),
        (
            META_WRITE_BATCH_ROWS.to_owned(),
            Value::from(spec.layout.write_batch_rows().get() as u64),
        ),
        (
            META_MAX_ROW_GROUP_ROWS.to_owned(),
            Value::from(spec.layout.max_row_group_rows().get() as u64),
        ),
        (
            META_MAX_ROW_GROUP_BYTES.to_owned(),
            Value::from(spec.layout.max_row_group_bytes().get() as u64),
        ),
        (
            META_PARQUET_COMPRESSION.to_owned(),
            Value::String(spec.layout.parquet_compression().to_owned()),
        ),
    ]);
    metadata.extend(spec.commit_metadata.clone());

    CommitProperties::default()
        .with_max_retries(0)
        .with_metadata(metadata)
        .with_application_transaction(Transaction::new(
            &spec.marker.application_id,
            spec.marker.application_version,
        ))
}

async fn read_marker(
    table: &DeltaTable,
    marker: &ApplicationTransactionMarker,
) -> Result<Option<i64>, DeltaTableError> {
    table
        .snapshot()?
        .transaction_version(table.log_store().as_ref(), &marker.application_id)
        .await
}

async fn readback_commit(
    table: &DeltaTable,
    spec: &ControlledDeltaWriteSpec,
    session_id: &str,
) -> Result<CommitReadback, ControlledDeltaWriteUnknown> {
    let entry = read_exact_commit_entry(table).await.map_err(|source| {
        unknown_value(
            spec,
            ControlledDeltaWriteUnknownStage::ReadCommitHistory,
            format!("exact committed log entry could not be loaded: {source}"),
            None,
        )
    })?;
    let transaction = match entry.application_transactions() {
        [transaction] => transaction,
        [] => {
            return Err(unknown_value(
                spec,
                ControlledDeltaWriteUnknownStage::ValidateCommitReadback,
                format!(
                    "exact commit {} contains no application transaction action",
                    entry.version()
                ),
                None,
            ));
        }
        transactions => {
            return Err(unknown_value(
                spec,
                ControlledDeltaWriteUnknownStage::ValidateCommitReadback,
                format!(
                    "exact commit {} contains {} application transaction actions, expected one",
                    entry.version(),
                    transactions.len()
                ),
                None,
            ));
        }
    };
    if transaction.app_id != spec.marker.application_id
        || transaction.version != spec.marker.application_version
    {
        return Err(unknown_value(
            spec,
            ControlledDeltaWriteUnknownStage::ValidateCommitReadback,
            format!(
                "exact commit {} transaction action was {:?}@{}, expected {:?}@{}",
                entry.version(),
                transaction.app_id,
                transaction.version,
                spec.marker.application_id,
                spec.marker.application_version
            ),
            None,
        ));
    }
    let commit = entry.commit_info();

    let expectations = [
        (
            META_OPERATION_ID,
            Value::String(hex(spec.operation_id.as_bytes())),
        ),
        (
            META_WRITER_GENERATION,
            Value::from(spec.writer_generation.get()),
        ),
        (
            META_EXPECTED_ROOT,
            Value::String(spec.predecessor.canonical_root().to_string()),
        ),
        (
            META_EXPECTED_VERSION,
            Value::from(spec.predecessor.version()),
        ),
        (META_SESSION_ID, Value::String(session_id.to_owned())),
        (
            META_APPLICATION_ID,
            Value::String(spec.marker.application_id.clone()),
        ),
        (
            META_APPLICATION_VERSION,
            Value::from(spec.marker.application_version),
        ),
        (
            META_WRITE_PRIMITIVE,
            Value::String(spec.mode.as_str().to_owned()),
        ),
        (
            META_TARGET_FILE_SIZE_BYTES,
            Value::from(spec.layout.target_file_size_bytes().get()),
        ),
        (
            META_WRITE_BATCH_ROWS,
            Value::from(spec.layout.write_batch_rows().get() as u64),
        ),
        (
            META_MAX_ROW_GROUP_ROWS,
            Value::from(spec.layout.max_row_group_rows().get() as u64),
        ),
        (
            META_MAX_ROW_GROUP_BYTES,
            Value::from(spec.layout.max_row_group_bytes().get() as u64),
        ),
        (
            META_PARQUET_COMPRESSION,
            Value::String(spec.layout.parquet_compression().to_owned()),
        ),
    ];
    for (key, expected) in expectations {
        if commit.info.get(key) != Some(&expected) {
            return Err(unknown_value(
                spec,
                ControlledDeltaWriteUnknownStage::ValidateCommitReadback,
                format!(
                    "commit metadata {key:?} was {:?}, expected {expected}",
                    commit.info.get(key)
                ),
                None,
            ));
        }
    }
    for (key, expected) in spec.commit_metadata() {
        if commit.info.get(key) != Some(expected) {
            return Err(unknown_value(
                spec,
                ControlledDeltaWriteUnknownStage::ValidateCommitReadback,
                format!(
                    "application commit metadata {key:?} was {:?}, expected {expected}",
                    commit.info.get(key)
                ),
                None,
            ));
        }
    }
    let read_version_evidence = match commit.read_version {
        Some(version) if version == spec.predecessor.version() => {
            CommitReadVersionEvidence::Exact(version)
        }
        None => CommitReadVersionEvidence::NotExposedByCommitHistory,
        Some(version) => {
            return Err(unknown_value(
                spec,
                ControlledDeltaWriteUnknownStage::ValidateCommitReadback,
                format!(
                    "commit readVersion was {version}, expected {}",
                    spec.predecessor.version()
                ),
                None,
            ));
        }
    };

    let num_retries = commit
        .info
        .get(OPERATION_METRICS)
        .and_then(Value::as_object)
        .and_then(|metrics| metrics.get(NUM_RETRIES))
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            unknown_value(
                spec,
                ControlledDeltaWriteUnknownStage::ValidateCommitReadback,
                "commit operationMetrics.num_retries was absent or not an unsigned integer",
                None,
            )
        })?;
    if num_retries != 0 {
        return Err(unknown_value(
            spec,
            ControlledDeltaWriteUnknownStage::ValidateCommitReadback,
            format!("controlled write reported {num_retries} internal retries, expected zero"),
            None,
        ));
    }
    Ok(CommitReadback {
        read_version_evidence,
        num_retries,
        transaction_commit_version: entry.version(),
    })
}

struct CommitReadback {
    read_version_evidence: CommitReadVersionEvidence,
    num_retries: u64,
    transaction_commit_version: u64,
}

fn reserved_commit_metadata_key(key: &str) -> bool {
    matches!(
        key,
        META_OPERATION_ID
            | META_WRITER_GENERATION
            | META_EXPECTED_ROOT
            | META_EXPECTED_VERSION
            | META_SESSION_ID
            | META_APPLICATION_ID
            | META_APPLICATION_VERSION
            | META_WRITE_PRIMITIVE
            | META_TARGET_FILE_SIZE_BYTES
            | META_WRITE_BATCH_ROWS
            | META_MAX_ROW_GROUP_ROWS
            | META_MAX_ROW_GROUP_BYTES
            | META_PARQUET_COMPRESSION
    )
}

fn observed_pin(table: &DeltaTable) -> Result<ExactDeltaPin, String> {
    let version = table
        .version()
        .ok_or_else(|| "Delta table handle has no loaded predecessor version".to_owned())?;
    ExactDeltaPin::new(table.table_url(), version).map_err(|error| error.to_string())
}

fn marker_evidence(
    marker: ApplicationTransactionMarker,
    observed_in: ExactDeltaPin,
    commit_version_evidence: MarkerCommitVersionEvidence,
) -> ApplicationMarkerEvidence {
    ApplicationMarkerEvidence {
        marker,
        observed_in,
        commit_version_evidence,
    }
}

fn classify_write_failure(
    spec: &ControlledDeltaWriteSpec,
    source: DeltaTableError,
) -> ControlledDeltaWriteOutcome {
    let is_collision = matches!(
        &source,
        DeltaTableError::VersionAlreadyExists(_)
            | DeltaTableError::VersionMismatch(_, _)
            | DeltaTableError::Transaction {
                source: TransactionError::CommitConflict(_)
                    | TransactionError::MaxCommitAttempts(0)
                    | TransactionError::VersionAlreadyExists(_),
            }
    );
    if is_collision {
        let target_version = spec.predecessor.version().saturating_add(1);
        ControlledDeltaWriteOutcome::Reconcile(ControlledDeltaWriteReconciliation::Conflict(
            ControlledDeltaWriteConflict::CommitCollision {
                predecessor: spec.predecessor.clone(),
                target_version,
                source,
            },
        ))
    } else {
        unknown(
            spec,
            ControlledDeltaWriteUnknownStage::ExecuteWrite,
            "write execution failed and this boundary cannot prove whether a commit became durable",
            Some(source),
        )
    }
}

fn unknown(
    spec: &ControlledDeltaWriteSpec,
    stage: ControlledDeltaWriteUnknownStage,
    detail: impl Into<String>,
    source: Option<DeltaTableError>,
) -> ControlledDeltaWriteOutcome {
    ControlledDeltaWriteOutcome::Reconcile(ControlledDeltaWriteReconciliation::Unknown(
        unknown_value(spec, stage, detail, source),
    ))
}

fn unknown_value(
    spec: &ControlledDeltaWriteSpec,
    stage: ControlledDeltaWriteUnknownStage,
    detail: impl Into<String>,
    source: Option<DeltaTableError>,
) -> ControlledDeltaWriteUnknown {
    ControlledDeltaWriteUnknown {
        stage,
        predecessor: spec.predecessor.clone(),
        detail: detail.into(),
        source,
    }
}

fn pin_label(pin: &ExactDeltaPin) -> String {
    format!("{}@{}", pin.canonical_root(), pin.version())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::fs::File;

    use arrow_array::{Int64Array, RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::execution::SessionStateBuilder;
    use datafusion::prelude::{SessionConfig, SessionContext};
    use deltalake::DeltaTableBuilder;
    use deltalake::delta_datafusion::planner::DeltaPlanner;
    use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
    use deltalake::operations::create::CreateBuilder;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use tempfile::TempDir;
    use url::Url;

    use super::*;

    struct Fixture {
        _temporary: TempDir,
        root: Url,
        table: DeltaTable,
    }

    async fn fixture() -> Fixture {
        let temporary = TempDir::new().expect("temporary Delta write fixture root");
        let table_path = temporary.path().join("table");
        fs::create_dir_all(&table_path).expect("create Delta write fixture directory");
        let root = Url::from_directory_path(&table_path).expect("fixture file URL");
        let schema = Schema::new(vec![value_field()]);
        let kernel: deltalake::kernel::StructType = (&schema)
            .try_into_kernel()
            .expect("Arrow fixture schema converts to Delta");

        ControlledDeltaHistoryProperties::try_new("value")
            .expect("history creation properties")
            .apply_to(
                CreateBuilder::new()
                    .with_location(root.to_string())
                    .with_table_name("controlled_delta_write_fixture")
                    .with_save_mode(SaveMode::ErrorIfExists)
                    .with_columns(kernel.fields().cloned()),
            )
            .await
            .expect("create controlled Delta write fixture");

        let table = DeltaTableBuilder::from_url(root.clone())
            .expect("construct exact fixture loader")
            .with_version(0)
            .load()
            .await
            .expect("load fixture predecessor version zero");
        Fixture {
            _temporary: temporary,
            root,
            table,
        }
    }

    fn value_field() -> Field {
        Field::new("value", DataType::Int64, false).with_metadata(HashMap::from([(
            "codefabric.semantic_type".to_owned(),
            "signed-test-value".to_owned(),
        )]))
    }

    fn batch(value: i64) -> RecordBatch {
        RecordBatch::try_new(
            Arc::new(Schema::new(vec![value_field()])),
            vec![Arc::new(Int64Array::from(vec![value]))],
        )
        .expect("fixture batch")
    }

    fn spec(root: &Url, version: u64, marker_version: i64) -> ControlledDeltaWriteSpec {
        ControlledDeltaWriteSpec::new(
            ExactDeltaPin::new(root, version).expect("exact fixture pin"),
            OperationId::from_bytes([0x11; 16]),
            WriterGeneration::new(7).expect("nonzero writer generation"),
            ApplicationTransactionMarker::new("codefabric/test/controlled-write", marker_version)
                .expect("valid application marker"),
            ControlledDeltaWriteMode::Append,
        )
    }

    fn session_and_input(value: i64) -> (Arc<SessionState>, SessionBoundLogicalPlan) {
        let session = SessionStateBuilder::new()
            .with_default_features()
            .with_config(
                SessionConfig::new()
                    .set_bool("datafusion.execution.parquet.pushdown_filters", false),
            )
            .with_query_planner(DeltaPlanner::new())
            .build();
        let context = SessionContext::new_with_state(session);
        let dataframe = context
            .read_batch(batch(value))
            .expect("construct fixture DataFrame");
        let state = Arc::new(context.state());
        let input = SessionBoundLogicalPlan::try_from_dataframe(Arc::clone(&state), dataframe)
            .expect("bind fixture plan to exact session");
        (state, input)
    }

    async fn exact_version_exists(root: &Url, version: u64) -> bool {
        DeltaTableBuilder::from_url(root.clone())
            .expect("construct exact-version assertion loader")
            .with_version(version)
            .load()
            .await
            .is_ok()
    }

    #[test]
    fn transaction_refs_have_one_canonical_delta_marker() {
        let transaction = TransactionRef::from_bytes([0xab; 32]);
        let first = ApplicationTransactionMarker::from_transaction_ref(transaction);
        let second = ApplicationTransactionMarker::from_transaction_ref(transaction);
        let different = ApplicationTransactionMarker::from_transaction_ref(
            TransactionRef::from_bytes([0xac; 32]),
        );

        assert_eq!(first, second);
        assert_ne!(first, different);
        assert_eq!(first.application_version(), 0);
        assert_eq!(
            first.application_id(),
            "codefabric/transaction/abababababababababababababababababababababababababababababababab"
        );
    }

    #[tokio::test]
    async fn history_creation_contract_sets_cdf_stats_and_feature_properties_at_version_zero() {
        let fixture = fixture().await;
        let configuration = fixture
            .table
            .snapshot()
            .expect("created history snapshot")
            .metadata()
            .configuration();
        assert_eq!(
            configuration
                .get("delta.enableChangeDataFeed")
                .map(String::as_str),
            Some("true")
        );
        assert_eq!(
            configuration
                .get("delta.dataSkippingStatsColumns")
                .map(String::as_str),
            Some("value")
        );
        assert_eq!(
            configuration
                .get("delta.enableDeletionVectors")
                .map(String::as_str),
            Some("false")
        );
        assert_eq!(
            configuration.get("delta.appendOnly").map(String::as_str),
            Some("true")
        );
    }

    #[tokio::test]
    async fn commit_preserves_session_marker_provenance_and_zero_retry() {
        let fixture = fixture().await;
        let expected_metadata = BTreeMap::from([
            (
                "codefabric.materialization.row_count".to_owned(),
                Value::from(1_u64),
            ),
            (
                "codefabric.materialization.empty".to_owned(),
                Value::Bool(false),
            ),
        ]);
        let transaction = spec(&fixture.root, 0, 4)
            .with_commit_metadata(expected_metadata.clone())
            .expect("application metadata contract");
        let (session, input) = session_and_input(41);
        let expected_session_id = session.session_id().to_owned();

        let outcome = write_exact_delta_plan(&fixture.table, &transaction, input).await;
        let ControlledDeltaWriteOutcome::Committed(committed) = outcome else {
            panic!("expected committed controlled write, got {outcome:?}");
        };
        assert_eq!(committed.predecessor().version(), 0);
        assert_eq!(committed.committed().version(), 1);
        assert_eq!(
            committed.committed().canonical_root(),
            transaction.predecessor().canonical_root()
        );
        assert_eq!(committed.session_id(), expected_session_id);
        assert_eq!(
            committed.read_version_evidence(),
            CommitReadVersionEvidence::NotExposedByCommitHistory
        );
        assert_eq!(committed.num_retries(), 0);
        assert_eq!(committed.operation_id(), transaction.operation_id());
        assert_eq!(
            committed.writer_generation(),
            transaction.writer_generation()
        );
        assert_eq!(committed.marker_evidence().marker(), transaction.marker());
        assert_eq!(
            committed.marker_evidence().commit_version_evidence(),
            MarkerCommitVersionEvidence::ExactCommitEntry(1)
        );
        assert_eq!(committed.commit_metadata(), &expected_metadata);
        assert_eq!(committed.layout(), ControlledDeltaWriteLayout::default());
        assert_eq!(
            committed.marker_evidence().observed_in(),
            committed.committed()
        );

        let add_actions = committed
            .table
            .snapshot()
            .expect("committed exact snapshot")
            .add_actions_table(true)
            .expect("flatten committed add actions");
        let relative_path = add_actions
            .column_by_name("path")
            .expect("add path")
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("string add path")
            .value(0);
        let table_path = committed
            .table
            .table_url()
            .to_file_path()
            .expect("local fixture table path");
        let parquet = ParquetRecordBatchReaderBuilder::try_new(
            File::open(table_path.join(relative_path)).expect("open committed Parquet file"),
        )
        .expect("read committed Parquet metadata");
        assert_eq!(
            parquet
                .schema()
                .field_with_name("value")
                .expect("written value field")
                .metadata()
                .get("codefabric.semantic_type")
                .map(String::as_str),
            Some("signed-test-value")
        );
        assert!(matches!(
            parquet.metadata().row_group(0).column(0).compression(),
            Compression::ZSTD(_)
        ));
    }

    #[tokio::test]
    async fn exact_predecessor_mismatch_fails_before_write() {
        let fixture = fixture().await;
        let transaction = spec(&fixture.root, 1, 4);
        let (_, input) = session_and_input(41);

        let outcome = write_exact_delta_plan(&fixture.table, &transaction, input).await;
        assert!(matches!(
            outcome,
            ControlledDeltaWriteOutcome::Reconcile(ControlledDeltaWriteReconciliation::Conflict(
                ControlledDeltaWriteConflict::PredecessorMismatch { .. }
            ))
        ));
        assert_eq!(fixture.table.version(), Some(0));
        assert!(!exact_version_exists(&fixture.root, 1).await);
    }

    #[tokio::test]
    async fn exact_marker_readback_returns_idempotent_evidence_without_guessing_commit_version() {
        let fixture = fixture().await;
        let first_spec = spec(&fixture.root, 0, 9);
        let (_, first_input) = session_and_input(1);
        let first = write_exact_delta_plan(&fixture.table, &first_spec, first_input).await;
        let ControlledDeltaWriteOutcome::Committed(first) = first else {
            panic!("expected initial commit, got {first:?}");
        };
        let committed_table = first.into_table();

        let repeated_spec = spec(&fixture.root, 1, 9);
        let (_, repeated_input) = session_and_input(2);
        let repeated =
            write_exact_delta_plan(&committed_table, &repeated_spec, repeated_input).await;
        let ControlledDeltaWriteOutcome::MarkerAlreadyCommitted(evidence) = repeated else {
            panic!("expected marker replay evidence, got {repeated:?}");
        };
        assert_eq!(evidence.marker(), repeated_spec.marker());
        assert_eq!(evidence.observed_in().version(), 1);
        assert_eq!(
            evidence.commit_version_evidence(),
            MarkerCommitVersionEvidence::NotExposedByPinnedSnapshotApi
        );
        assert!(!exact_version_exists(&fixture.root, 2).await);
    }

    #[tokio::test]
    async fn restart_readback_uses_the_loaded_exact_commit_after_the_head_advances() {
        let fixture = fixture().await;
        let transaction = spec(&fixture.root, 0, 12)
            .with_commit_metadata(BTreeMap::from([(
                "codefabric.materialization.row_count".to_owned(),
                Value::from(1_u64),
            )]))
            .expect("application commit metadata");
        let (session, input) = session_and_input(17);
        let recorded_session_id = session.session_id().to_owned();
        let committed = write_exact_delta_plan(&fixture.table, &transaction, input).await;
        let ControlledDeltaWriteOutcome::Committed(committed) = committed else {
            panic!("expected controlled version one, got {committed:?}");
        };
        let exact_version_one = committed.into_table();

        let latest = exact_version_one
            .clone()
            .write([batch(18)])
            .with_commit_properties(CommitProperties::default().with_max_retries(0))
            .await
            .expect("advance backing log to version two");
        assert_eq!(latest.version(), Some(2));
        assert_eq!(exact_version_one.version(), Some(1));

        let readback =
            readback_exact_delta_commit(&exact_version_one, &transaction, &recorded_session_id)
                .await
                .expect("restart proof must read version one's commitInfo and txn directly");
        assert_eq!(readback.committed().version(), 1);
        assert_eq!(
            readback.marker_evidence().commit_version_evidence(),
            MarkerCommitVersionEvidence::ExactCommitEntry(1)
        );
        assert_eq!(
            readback
                .commit_metadata()
                .get("codefabric.materialization.row_count"),
            Some(&Value::from(1_u64))
        );
    }

    #[tokio::test]
    async fn restart_readback_rejects_txn_drift_in_the_exact_commit_entry() {
        let fixture = fixture().await;
        let transaction = spec(&fixture.root, 0, 13);
        let (session, input) = session_and_input(21);
        let recorded_session_id = session.session_id().to_owned();
        let committed = write_exact_delta_plan(&fixture.table, &transaction, input).await;
        let ControlledDeltaWriteOutcome::Committed(committed) = committed else {
            panic!("expected controlled version one, got {committed:?}");
        };
        let exact_version_one = committed.into_table();

        assert_eq!(
            read_marker(&exact_version_one, transaction.marker())
                .await
                .expect("read cached exact-snapshot marker before corruption"),
            Some(transaction.marker().application_version())
        );
        rewrite_exact_transaction_app_id(&fixture.root, 1, "codefabric/test/corrupted-marker");

        let error =
            readback_exact_delta_commit(&exact_version_one, &transaction, &recorded_session_id)
                .await
                .expect_err("exact commit txn drift must fail despite cached snapshot evidence");
        assert_eq!(
            error.stage(),
            ControlledDeltaWriteUnknownStage::ValidateCommitReadback
        );
        assert!(error.detail().contains("transaction action was"));
    }

    #[tokio::test]
    async fn concurrent_advance_is_a_typed_conflict_and_never_retries() {
        let fixture = fixture().await;
        let competing_table = fixture
            .table
            .clone()
            .write([batch(7)])
            .with_commit_properties(CommitProperties::default().with_max_retries(0))
            .await
            .expect("commit competing version one");
        assert_eq!(competing_table.version(), Some(1));

        let transaction = spec(&fixture.root, 0, 3);
        let (_, input) = session_and_input(8);
        let outcome = write_exact_delta_plan(&fixture.table, &transaction, input).await;
        assert!(matches!(
            outcome,
            ControlledDeltaWriteOutcome::Reconcile(ControlledDeltaWriteReconciliation::Conflict(
                ControlledDeltaWriteConflict::CommitCollision {
                    target_version: 1,
                    ..
                }
            ))
        ));
        assert!(exact_version_exists(&fixture.root, 1).await);
        assert!(!exact_version_exists(&fixture.root, 2).await);
    }

    fn rewrite_exact_transaction_app_id(root: &Url, version: u64, application_id: &str) {
        let table_path = root
            .to_file_path()
            .expect("fixture Delta root must be a local path");
        let commit_path = table_path
            .join("_delta_log")
            .join(format!("{version:020}.json"));
        let input = fs::read_to_string(&commit_path).expect("read exact fixture commit");
        let mut found = false;
        let mut actions = Vec::new();
        for line in input.lines() {
            let mut action: Value = serde_json::from_str(line).expect("parse fixture action");
            if let Some(transaction) = action.get_mut("txn").and_then(Value::as_object_mut) {
                transaction.insert("appId".to_owned(), Value::String(application_id.to_owned()));
                found = true;
            }
            actions.push(serde_json::to_string(&action).expect("serialize fixture action"));
        }
        assert!(
            found,
            "fixture commit must contain an application txn action"
        );
        fs::write(commit_path, format!("{}\n", actions.join("\n")))
            .expect("rewrite exact fixture transaction action");
    }

    #[test]
    fn independently_constructed_session_cannot_be_substituted_for_plan_session() {
        let plan_context = SessionContext::new();
        let dataframe = plan_context
            .read_batch(batch(1))
            .expect("construct plan DataFrame");
        let other_context = SessionContext::new();
        let error =
            SessionBoundLogicalPlan::try_from_dataframe(Arc::new(other_context.state()), dataframe)
                .expect_err("independent session must be rejected");
        assert!(matches!(
            error,
            ControlledDeltaWriteInputError::PlanSessionMismatch { .. }
        ));
    }

    #[test]
    fn marker_contract_rejects_empty_and_negative_values() {
        assert_eq!(
            ApplicationTransactionMarker::new(" ", 0),
            Err(ControlledDeltaWriteInputError::EmptyApplicationId)
        );
        assert_eq!(
            ApplicationTransactionMarker::new("codefabric/test", -1),
            Err(ControlledDeltaWriteInputError::NegativeApplicationVersion(
                -1
            ))
        );

        assert_eq!(
            spec(&Url::parse("memory:///metadata").unwrap(), 0, 0).with_commit_metadata(
                BTreeMap::from([("foreign.metadata".to_owned(), Value::Bool(true),)])
            ),
            Err(ControlledDeltaWriteInputError::InvalidCommitMetadataKey(
                "foreign.metadata".to_owned()
            ))
        );
        assert_eq!(
            spec(&Url::parse("memory:///metadata").unwrap(), 0, 0).with_commit_metadata(
                BTreeMap::from([(
                    META_OPERATION_ID.to_owned(),
                    Value::String("shadow".to_owned()),
                )])
            ),
            Err(ControlledDeltaWriteInputError::ReservedCommitMetadataKey(
                META_OPERATION_ID.to_owned()
            ))
        );
    }
}
