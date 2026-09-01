//! Exact-version Delta provider reconstruction for one immutable fabric epoch.
//!
//! This module deliberately exposes only two provider recipes:
//!
//! 1. an application-validated loaded snapshot plus the epoch session, without a
//!    table-version selector; and
//! 2. a log store plus an exact table-version selector and the epoch session,
//!    without a supplied snapshot.
//!
//! Keeping the recipes separate matters because delta-rs gives a supplied
//! snapshot precedence over `with_table_version`. Combining both inputs would
//! make the version selector inert and would weaken the epoch pin.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use arrow_array::{Array, RecordBatch, StringArray, UInt64Array};
use arrow_schema::DataType;
use datafusion::catalog::TableProvider;
use datafusion::common::Statistics;
use datafusion::execution::SessionState;
use datafusion::physical_plan::{ExecutionPlan, collect};
use deltalake::delta_datafusion::TableProviderBuilder;
use deltalake::delta_datafusion::cdf::{CHANGE_TYPE_COL, COMMIT_VERSION_COL};
use deltalake::kernel::{Action, CommitInfo, EagerSnapshot, Transaction};
use deltalake::logstore::{LogStoreRef, get_actions};
use deltalake::table::normalize_table_url;
use deltalake::{DeltaTable, DeltaTableError};
use thiserror::Error;
use url::Url;

/// Application-owned identity of one exact Delta table state.
///
/// The root is canonicalized when the pin is constructed. Credentials, query
/// parameters, and fragments are not part of table identity and are removed.
/// Existing local roots are also filesystem-canonicalized so a symlink spelling
/// cannot manufacture a second identity for the same table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDeltaPin {
    canonical_root: Url,
    version: u64,
}

impl ExactDeltaPin {
    /// Construct an exact pin from a table root and Delta log version.
    ///
    /// # Errors
    ///
    /// Returns [`ExactDeltaProviderError::InvalidTableRoot`] when the root
    /// cannot be represented as a canonical hierarchical table URL.
    pub fn new(root: &Url, version: u64) -> Result<Self, ExactDeltaProviderError> {
        Ok(Self {
            canonical_root: canonical_delta_root(root)?,
            version,
        })
    }

    /// Canonical table-root identity carried by this pin.
    #[must_use]
    pub fn canonical_root(&self) -> &Url {
        &self.canonical_root
    }

    /// Exact Delta transaction-log version carried by this pin.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    fn validate_observation(
        &self,
        observed: &ObservedDeltaIdentity,
    ) -> Result<(), ExactDeltaProviderError> {
        if observed.canonical_root != self.canonical_root || observed.version != self.version {
            return Err(ExactDeltaProviderError::IdentityMismatch {
                expected_root: self.canonical_root.to_string(),
                expected_version: self.version,
                observed_root: observed.canonical_root.to_string(),
                observed_version: observed.version,
            });
        }
        Ok(())
    }
}

/// Root and version observed from loaded Delta state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedDeltaIdentity {
    canonical_root: Url,
    version: u64,
}

impl ObservedDeltaIdentity {
    /// Canonical root observed from the loaded state's owning log store.
    #[must_use]
    pub fn canonical_root(&self) -> &Url {
        &self.canonical_root
    }

    /// Version observed from the loaded snapshot itself.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }
}

/// A loaded eager snapshot whose observed root and version matched one exact pin.
///
/// Fields stay private so callers cannot pair an eager snapshot with separately
/// asserted identity. The owning `DeltaTable` is retained only to register its
/// root object store into the exact epoch session before the snapshot-only
/// provider is built.
#[derive(Clone)]
pub struct ValidatedDeltaSnapshot {
    table: DeltaTable,
    eager_snapshot: Arc<EagerSnapshot>,
    observed: ObservedDeltaIdentity,
}

impl fmt::Debug for ValidatedDeltaSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedDeltaSnapshot")
            .field("observed", &self.observed)
            .finish_non_exhaustive()
    }
}

impl ValidatedDeltaSnapshot {
    /// Validate a loaded table against `pin` and capture its eager snapshot.
    ///
    /// This constructor never loads or advances a table. The caller must have
    /// loaded the exact state already; an uninitialized table is rejected.
    ///
    /// # Errors
    ///
    /// Rejects an uninitialized table, an invalid root, or a root/version that
    /// differs from `pin`.
    pub fn try_from_loaded_table(
        table: DeltaTable,
        pin: &ExactDeltaPin,
    ) -> Result<Self, ExactDeltaProviderError> {
        let state = table
            .snapshot()
            .map_err(|_| ExactDeltaProviderError::MissingLoadedSnapshot)?;
        let observed = ObservedDeltaIdentity {
            canonical_root: canonical_delta_root(table.table_url())?,
            version: state.version(),
        };
        pin.validate_observation(&observed)?;

        Ok(Self {
            eager_snapshot: Arc::new(state.snapshot().clone()),
            table,
            observed,
        })
    }

    /// Identity actually observed while validating the loaded snapshot.
    #[must_use]
    pub const fn observed_identity(&self) -> &ObservedDeltaIdentity {
        &self.observed
    }

    fn revalidate(&self, pin: &ExactDeltaPin) -> Result<(), ExactDeltaProviderError> {
        let state = self
            .table
            .snapshot()
            .map_err(|_| ExactDeltaProviderError::MissingLoadedSnapshot)?;
        let current = ObservedDeltaIdentity {
            canonical_root: canonical_delta_root(self.table.table_url())?,
            version: state.version(),
        };
        pin.validate_observation(&self.observed)?;
        pin.validate_observation(&current)?;
        if current != self.observed || state.snapshot().version() != self.observed.version {
            return Err(ExactDeltaProviderError::ValidatedSnapshotChanged);
        }
        Ok(())
    }
}

/// Failure to prove that a Delta provider represents an exact epoch pin.
#[derive(Debug, Error)]
pub enum ExactDeltaProviderError {
    /// A root cannot be canonicalized without ambiguity.
    #[error("invalid Delta table-root identity: {0}")]
    InvalidTableRoot(String),
    /// A supposedly loaded table has no snapshot and therefore no observed version.
    #[error("loaded Delta table has no snapshot/version identity")]
    MissingLoadedSnapshot,
    /// A validated wrapper's private table state no longer agrees with its observation.
    #[error("validated Delta snapshot changed after validation")]
    ValidatedSnapshotChanged,
    /// Observed table state differs from the epoch pin.
    #[error(
        "exact Delta identity mismatch: expected {expected_root}@{expected_version}, observed {observed_root}@{observed_version}"
    )]
    IdentityMismatch {
        expected_root: String,
        expected_version: u64,
        observed_root: String,
        observed_version: u64,
    },
    /// Query-serving snapshots must retain their active-file set.
    #[error("exact Delta provider snapshot was loaded without active files")]
    StatisticsFilesNotLoaded,
    /// Query-serving snapshots must parse Delta file statistics for pruning and inspection.
    #[error("exact Delta provider snapshot was loaded with file statistics disabled")]
    StatisticsParsingDisabled,
    /// Exact snapshot reconstruction or session object-store setup failed.
    #[error(transparent)]
    Delta(#[from] DeltaTableError),
    /// The pinned delta-rs provider builder rejected the exact recipe.
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
}

/// Whether every active file supplied a named Delta statistic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactDeltaStatisticAvailability {
    /// Every active file supplied a non-null value. Zero files is an exact empty set.
    KnownForAllFiles { file_count: usize },
    /// At least one active file omitted the statistic; no zero is synthesized.
    UnknownForFiles {
        file_count: usize,
        unknown_file_count: usize,
    },
}

/// One flattened Delta add-action statistic and its completeness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDeltaStatisticField {
    name: String,
    availability: ExactDeltaStatisticAvailability,
}

impl ExactDeltaStatisticField {
    /// Flattened `add`-action field name (`size_bytes`, `min.<column>`, and so on).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Completeness across the exact snapshot's active files.
    #[must_use]
    pub const fn availability(&self) -> ExactDeltaStatisticAvailability {
        self.availability
    }
}

/// Full statistics inspection paired with one exact delta-rs provider.
///
/// The flattened add-action batch retains path, file size, row count, null
/// count, min, max, and partition values exactly as Delta exposed them. Null or
/// absent cells are also summarized as [`ExactDeltaStatisticAvailability::UnknownForFiles`];
/// they are never converted into zero rows or zero values. DataFusion's
/// provider-level statistics are normalized to [`Statistics::new_unknown`]
/// when delta-rs does not report them.
#[derive(Clone, Debug)]
pub struct ExactDeltaStatisticsInspection {
    add_actions: RecordBatch,
    fields: Vec<ExactDeltaStatisticField>,
    optimizer_statistics: Statistics,
    optimizer_statistics_reported: bool,
}

impl ExactDeltaStatisticsInspection {
    /// Complete flattened active-file add actions for the pinned version.
    #[must_use]
    pub const fn add_actions(&self) -> &RecordBatch {
        &self.add_actions
    }

    /// Explicit completeness for every inspected file/column statistic.
    #[must_use]
    pub fn fields(&self) -> &[ExactDeltaStatisticField] {
        &self.fields
    }

    /// DataFusion optimizer statistics, with every value absent when the
    /// provider did not report statistics.
    #[must_use]
    pub const fn optimizer_statistics(&self) -> &Statistics {
        &self.optimizer_statistics
    }

    /// Whether the Delta provider itself reported optimizer statistics.
    #[must_use]
    pub const fn optimizer_statistics_reported(&self) -> bool {
        self.optimizer_statistics_reported
    }

    /// Look up one flattened statistic by its stable field name.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&ExactDeltaStatisticField> {
        self.fields.iter().find(|field| field.name == name)
    }
}

/// Exact delta-rs `TableProvider` plus its non-lossy statistics inspection.
pub struct ExactDeltaProviderRead {
    provider: Arc<dyn TableProvider>,
    statistics: ExactDeltaStatisticsInspection,
}

impl fmt::Debug for ExactDeltaProviderRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactDeltaProviderRead")
            .field("provider_schema", &self.provider.schema())
            .field("statistics", &self.statistics)
            .finish_non_exhaustive()
    }
}

impl ExactDeltaProviderRead {
    /// Exact delta-rs provider; no raw Parquet/listing provider is exposed.
    #[must_use]
    pub fn provider(&self) -> Arc<dyn TableProvider> {
        Arc::clone(&self.provider)
    }

    /// Full active-file and optimizer statistics inspection.
    #[must_use]
    pub const fn statistics(&self) -> &ExactDeltaStatisticsInspection {
        &self.statistics
    }

    /// Consume the exact read while preserving both registration capability
    /// and the complete statistics inspection as separate owned values.
    #[must_use]
    pub fn into_parts(self) -> (Arc<dyn TableProvider>, ExactDeltaStatisticsInspection) {
        (self.provider, self.statistics)
    }

    /// Consume the evidence product when only provider registration remains.
    #[must_use]
    pub fn into_provider(self) -> Arc<dyn TableProvider> {
        self.provider
    }
}

/// Failure to read the exact commit entry for the version actually loaded in
/// a Delta table handle.
#[derive(Debug, Error)]
pub(crate) enum ExactDeltaCommitInfoError {
    #[error("Delta table has no loaded version")]
    MissingLoadedVersion,
    #[error("failed to read exact Delta commit {version}: {source}")]
    Read {
        version: u64,
        #[source]
        source: DeltaTableError,
    },
    #[error("exact Delta commit {version} is no longer retained")]
    MissingCommit { version: u64 },
    #[error("failed to decode exact Delta commit {version}: {source}")]
    Decode {
        version: u64,
        #[source]
        source: DeltaTableError,
    },
    #[error("exact Delta commit {version} contains no commitInfo action")]
    MissingCommitInfo { version: u64 },
    #[error("exact Delta commit {version} contains more than one commitInfo action")]
    DuplicateCommitInfo { version: u64 },
}

/// Actions required to prove one exact Delta commit.
///
/// `commitInfo` and application transaction actions are kept together so a
/// caller cannot accidentally combine metadata from one version with a `txn`
/// marker observed only through a later aggregate snapshot.
#[derive(Debug)]
pub(crate) struct ExactDeltaCommitEntry {
    version: u64,
    commit_info: CommitInfo,
    application_transactions: Vec<Transaction>,
}

impl ExactDeltaCommitEntry {
    /// Exact transaction-log version whose JSON entry was decoded.
    pub(crate) const fn version(&self) -> u64 {
        self.version
    }

    /// Sole `commitInfo` action decoded from this exact version.
    pub(crate) const fn commit_info(&self) -> &CommitInfo {
        &self.commit_info
    }

    /// Every application `txn` action decoded from this exact version.
    pub(crate) fn application_transactions(&self) -> &[Transaction] {
        &self.application_transactions
    }

    fn into_commit_info(self) -> CommitInfo {
        self.commit_info
    }
}

/// Read proof actions from the exact version loaded in `table`.
///
/// `DeltaTable::history(Some(1))` is intentionally not used here: history is
/// a log-store query whose newest entry may be newer than an old snapshot
/// loaded with `with_version`. Exact-version proof reads the selected JSON log
/// entry directly and fails closed if it has been retained away.
pub(crate) async fn read_exact_commit_entry(
    table: &DeltaTable,
) -> Result<ExactDeltaCommitEntry, ExactDeltaCommitInfoError> {
    let version = table
        .version()
        .ok_or(ExactDeltaCommitInfoError::MissingLoadedVersion)?;
    let bytes = table
        .log_store()
        .read_commit_entry(version)
        .await
        .map_err(|source| ExactDeltaCommitInfoError::Read { version, source })?
        .ok_or(ExactDeltaCommitInfoError::MissingCommit { version })?;
    let actions = get_actions(version, &bytes)
        .map_err(|source| ExactDeltaCommitInfoError::Decode { version, source })?;
    let mut commit_info = None;
    let mut application_transactions = Vec::new();
    for action in actions {
        match action {
            Action::CommitInfo(info) => {
                if commit_info.replace(info).is_some() {
                    return Err(ExactDeltaCommitInfoError::DuplicateCommitInfo { version });
                }
            }
            Action::Txn(transaction) => application_transactions.push(transaction),
            _ => {}
        }
    }
    let commit_info =
        commit_info.ok_or(ExactDeltaCommitInfoError::MissingCommitInfo { version })?;
    Ok(ExactDeltaCommitEntry {
        version,
        commit_info,
        application_transactions,
    })
}

/// Read only commit metadata from the exact version loaded in `table`.
///
/// Callers which also prove an application transaction must use
/// [`read_exact_commit_entry`] so both actions come from the same log entry.
pub(crate) async fn read_exact_commit_info(
    table: &DeltaTable,
) -> Result<CommitInfo, ExactDeltaCommitInfoError> {
    Ok(read_exact_commit_entry(table).await?.into_commit_info())
}

/// Build from a previously loaded and validated eager snapshot.
///
/// This is the snapshot-authoritative recipe. It intentionally calls
/// `with_eager_snapshot` and `with_session`, but never `with_log_store` or
/// `with_table_version`. The retained table is used only to register the
/// snapshot's object store into the supplied epoch session.
///
/// # Errors
///
/// Rejects changed/mismatched identity, object-store registration failure, or
/// provider construction failure.
pub async fn provider_from_validated_snapshot(
    pin: &ExactDeltaPin,
    snapshot: ValidatedDeltaSnapshot,
    epoch_session: Arc<SessionState>,
) -> Result<Arc<dyn TableProvider>, ExactDeltaProviderError> {
    Ok(
        provider_read_from_validated_snapshot(pin, snapshot, epoch_session)
            .await?
            .into_provider(),
    )
}

/// Build the snapshot-authoritative exact provider and inspect all retained
/// Delta file/column statistics before returning it.
///
/// # Errors
///
/// In addition to provider construction failures, rejects snapshots loaded
/// with `require_files = false` or `skip_stats = true`.
pub async fn provider_read_from_validated_snapshot(
    pin: &ExactDeltaPin,
    snapshot: ValidatedDeltaSnapshot,
    epoch_session: Arc<SessionState>,
) -> Result<ExactDeltaProviderRead, ExactDeltaProviderError> {
    snapshot.revalidate(pin)?;
    let state = snapshot.table.snapshot()?;
    validate_statistics_load_config(state.load_config())?;
    let add_actions = state.add_actions_table(true)?;
    let arrow_schema = state.snapshot().arrow_schema();
    snapshot
        .table
        .update_datafusion_session(epoch_session.as_ref())?;

    let provider = TableProviderBuilder::default()
        .with_eager_snapshot(snapshot.eager_snapshot)
        .with_session(epoch_session)
        .await?;
    Ok(provider_read(provider, add_actions, arrow_schema))
}

/// Build from a log store and an exact transaction-log version.
///
/// This is the log-store-authoritative recipe. It first reconstructs the exact
/// kernel snapshot solely to observe and validate the requested root/version,
/// then constructs the provider with `with_log_store`, `with_table_version`, and
/// `with_session`. No snapshot is supplied to the provider builder, and no
/// latest-version discovery is performed.
///
/// # Errors
///
/// Rejects root/version mismatch, an unavailable exact version, or provider
/// construction failure.
pub async fn provider_from_exact_log_store(
    pin: &ExactDeltaPin,
    log_store: LogStoreRef,
    epoch_session: Arc<SessionState>,
) -> Result<Arc<dyn TableProvider>, ExactDeltaProviderError> {
    Ok(
        provider_read_from_exact_log_store(pin, log_store, epoch_session)
            .await?
            .into_provider(),
    )
}

/// Build the log-store-authoritative exact provider and inspect all retained
/// Delta file/column statistics before returning it.
///
/// # Errors
///
/// Rejects an unavailable/mismatched version or any snapshot configuration
/// which does not retain active files and parsed statistics.
pub async fn provider_read_from_exact_log_store(
    pin: &ExactDeltaPin,
    log_store: LogStoreRef,
    epoch_session: Arc<SessionState>,
) -> Result<ExactDeltaProviderRead, ExactDeltaProviderError> {
    let observed_root = canonical_delta_root(log_store.config().location())?;
    if observed_root != pin.canonical_root {
        return Err(ExactDeltaProviderError::IdentityMismatch {
            expected_root: pin.canonical_root.to_string(),
            expected_version: pin.version,
            observed_root: observed_root.to_string(),
            observed_version: pin.version,
        });
    }

    // `load_version(pin.version)` is load-bearing: `load()` means latest.
    let mut observed_table = DeltaTable::new(
        Arc::clone(&log_store),
        deltalake::DeltaTableConfig::default(),
    );
    observed_table.load_version(pin.version).await?;
    let observed_snapshot = observed_table.snapshot()?;
    let observed = ObservedDeltaIdentity {
        canonical_root: observed_root,
        version: observed_snapshot.version(),
    };
    pin.validate_observation(&observed)?;
    validate_statistics_load_config(observed_snapshot.load_config())?;
    let add_actions = observed_snapshot.add_actions_table(true)?;
    let arrow_schema = observed_snapshot.snapshot().arrow_schema();

    let provider = TableProviderBuilder::default()
        .with_log_store(log_store)
        .with_table_version(Some(pin.version))
        .with_session(epoch_session)
        .await?;
    Ok(provider_read(provider, add_actions, arrow_schema))
}

fn validate_statistics_load_config(
    config: &deltalake::DeltaTableConfig,
) -> Result<(), ExactDeltaProviderError> {
    if !config.require_files {
        return Err(ExactDeltaProviderError::StatisticsFilesNotLoaded);
    }
    if config.skip_stats {
        return Err(ExactDeltaProviderError::StatisticsParsingDisabled);
    }
    Ok(())
}

fn provider_read(
    provider: Arc<dyn TableProvider>,
    add_actions: RecordBatch,
    table_schema: Arc<arrow_schema::Schema>,
) -> ExactDeltaProviderRead {
    let fields = inspected_statistic_names(table_schema.as_ref())
        .into_iter()
        .map(|name| {
            let unknown_file_count = add_actions
                .schema()
                .index_of(&name)
                .map_or(add_actions.num_rows(), |index| {
                    add_actions.column(index).null_count()
                });
            let availability = if unknown_file_count == 0 {
                ExactDeltaStatisticAvailability::KnownForAllFiles {
                    file_count: add_actions.num_rows(),
                }
            } else {
                ExactDeltaStatisticAvailability::UnknownForFiles {
                    file_count: add_actions.num_rows(),
                    unknown_file_count,
                }
            };
            ExactDeltaStatisticField { name, availability }
        })
        .collect();
    let reported = provider.statistics();
    let optimizer_statistics_reported = reported.is_some();
    let optimizer_statistics =
        reported.unwrap_or_else(|| Statistics::new_unknown(provider.schema().as_ref()));
    ExactDeltaProviderRead {
        provider,
        statistics: ExactDeltaStatisticsInspection {
            add_actions,
            fields,
            optimizer_statistics,
            optimizer_statistics_reported,
        },
    }
}

fn inspected_statistic_names(schema: &arrow_schema::Schema) -> Vec<String> {
    let mut names = vec![
        "path".to_owned(),
        "size_bytes".to_owned(),
        "num_records".to_owned(),
    ];
    for field in schema.fields() {
        for prefix in ["null_count", "min", "max", "partition"] {
            names.push(format!("{prefix}.{}", field.name()));
        }
    }
    names
}

/// Result of preparing one explicit, inclusive Delta change-data-feed range.
///
/// A retained CDF transport range produces a physical plan. Any missing log
/// entry produces an exact-snapshot reconstruction requirement instead of a
/// shortened or out-of-range-tolerant scan.
pub enum ExactDeltaCdfPreparation {
    /// Every required log entry was retained and an exact physical plan exists.
    PhysicalPlan(PreparedExactDeltaCdfRead),
    /// CDF transport continuity is unavailable; reconstruct the named exact snapshot.
    ExactSnapshotFallback(ExactDeltaSnapshotFallbackRequired),
}

impl fmt::Debug for ExactDeltaCdfPreparation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PhysicalPlan(plan) => formatter.debug_tuple("PhysicalPlan").field(plan).finish(),
            Self::ExactSnapshotFallback(fallback) => formatter
                .debug_tuple("ExactSnapshotFallback")
                .field(fallback)
                .finish(),
        }
    }
}

/// Why exact-snapshot reconstruction replaced incremental CDF transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactDeltaCdfFallbackReason {
    /// The requested inclusive end was not retained.
    EndingVersionNotRetained { version: u64 },
    /// A required entry before the inclusive end was not retained.
    RequiredVersionNotRetained { version: u64 },
}

/// Typed instruction to reconstruct a full exact snapshot after a CDF gap.
///
/// CDF is only transport. This value does not claim semantic completeness; it
/// names the exact source state at which the caller must perform its governed
/// full reconstruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDeltaSnapshotFallbackRequired {
    consumer_id: Arc<str>,
    checkpoint_before: ExactDeltaPin,
    reconstruct_at: ExactDeltaPin,
    requested_window: DeltaCdfReadWindow,
    reason: ExactDeltaCdfFallbackReason,
}

impl ExactDeltaSnapshotFallbackRequired {
    /// Consumer whose incremental transport encountered the gap.
    #[must_use]
    pub fn consumer_id(&self) -> &str {
        &self.consumer_id
    }

    /// Durable source checkpoint from which the failed range began.
    #[must_use]
    pub const fn checkpoint_before(&self) -> &ExactDeltaPin {
        &self.checkpoint_before
    }

    /// Full snapshot which must be reconstructed exactly.
    #[must_use]
    pub const fn reconstruct_at(&self) -> &ExactDeltaPin {
        &self.reconstruct_at
    }

    /// Explicit inclusive range which could not be transported by CDF.
    #[must_use]
    pub const fn requested_window(&self) -> DeltaCdfReadWindow {
        self.requested_window
    }

    /// Exact retained-log failure which required reconstruction.
    #[must_use]
    pub const fn reason(&self) -> ExactDeltaCdfFallbackReason {
        self.reason
    }
}

/// A prepared Delta CDF physical plan bound to one durable checkpoint.
///
/// The plan and its epoch session are kept together. Execution therefore
/// cannot substitute another session or silently change the explicit inclusive
/// version range.
pub struct PreparedExactDeltaCdfRead {
    consumer_id: Arc<str>,
    checkpoint_before: ExactDeltaPin,
    source_through: ExactDeltaPin,
    window: DeltaCdfReadWindow,
    physical_plan: Arc<dyn ExecutionPlan>,
    epoch_session: Arc<SessionState>,
    commit_version_index: usize,
    change_type_index: usize,
}

impl fmt::Debug for PreparedExactDeltaCdfRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedExactDeltaCdfRead")
            .field("consumer_id", &self.consumer_id)
            .field("checkpoint_before", &self.checkpoint_before)
            .field("source_through", &self.source_through)
            .field("window", &self.window)
            .finish_non_exhaustive()
    }
}

impl PreparedExactDeltaCdfRead {
    /// Consumer bound to this plan.
    #[must_use]
    pub fn consumer_id(&self) -> &str {
        &self.consumer_id
    }

    /// Exact source state containing the inclusive end.
    #[must_use]
    pub const fn source_through(&self) -> &ExactDeltaPin {
        &self.source_through
    }

    /// Explicit inclusive version range supplied to delta-rs.
    #[must_use]
    pub const fn window(&self) -> DeltaCdfReadWindow {
        self.window
    }

    /// Prepared DataFusion physical plan, exposed for inspection only.
    ///
    /// Successful execution through [`Self::execute`] is required before the
    /// opaque checkpoint-advancement token can be produced.
    #[must_use]
    pub const fn physical_plan(&self) -> &Arc<dyn ExecutionPlan> {
        &self.physical_plan
    }

    /// Execute the physical plan in the same epoch session used to prepare it.
    ///
    /// Zero rows are a successful exact execution. Every emitted row must carry
    /// a non-null `_commit_version` inside the inclusive range and a supported,
    /// non-null `_change_type`.
    pub async fn execute(self) -> Result<ExecutedExactDeltaCdfRead, ExactDeltaCdfExecutionError> {
        let batches = collect(
            Arc::clone(&self.physical_plan),
            self.epoch_session.task_ctx(),
        )
        .await?;
        validate_cdf_batches(
            &batches,
            self.commit_version_index,
            self.change_type_index,
            self.window,
        )?;
        Ok(ExecutedExactDeltaCdfRead {
            consumer_id: self.consumer_id,
            checkpoint_before: self.checkpoint_before,
            source_through: self.source_through,
            window: self.window,
            batches,
        })
    }
}

/// Successfully executed exact CDF batches awaiting a downstream durable commit.
pub struct ExecutedExactDeltaCdfRead {
    consumer_id: Arc<str>,
    checkpoint_before: ExactDeltaPin,
    source_through: ExactDeltaPin,
    window: DeltaCdfReadWindow,
    batches: Vec<RecordBatch>,
}

impl fmt::Debug for ExecutedExactDeltaCdfRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExecutedExactDeltaCdfRead")
            .field("consumer_id", &self.consumer_id)
            .field("checkpoint_before", &self.checkpoint_before)
            .field("source_through", &self.source_through)
            .field("window", &self.window)
            .field("batch_count", &self.batches.len())
            .field("row_count", &self.row_count())
            .finish()
    }
}

impl ExecutedExactDeltaCdfRead {
    /// Exact Arrow batches to apply to the downstream consumer.
    #[must_use]
    pub fn batches(&self) -> &[RecordBatch] {
        &self.batches
    }

    /// Total number of transported rows. Zero is a successful interval.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.batches.iter().map(RecordBatch::num_rows).sum()
    }

    /// Finish the range after attempting to apply these batches durably.
    ///
    /// This consumes both the execution result and the explicit downstream
    /// outcome. Only `Ok` with a nonzero durable commit identity returns the
    /// opaque token accepted by checkpoint advancement. `Err` returns no token.
    pub fn finish_downstream(
        self,
        downstream_result: Result<DeltaCdfDownstreamCommit, DeltaCdfDownstreamFailure>,
    ) -> Result<DeltaCdfDownstreamSuccess, DeltaCdfDownstreamCompletionError> {
        let downstream_commit = downstream_result?;
        if downstream_commit.has_zero_identity() {
            return Err(DeltaCdfCheckpointError::ZeroDownstreamCommitIdentity.into());
        }
        Ok(DeltaCdfDownstreamSuccess {
            consumer_id: self.consumer_id,
            checkpoint_before: self.checkpoint_before,
            source_through: self.source_through,
            window: self.window,
            downstream_commit,
        })
    }
}

/// Explicit failure returned by the durable downstream application step.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("downstream did not durably apply the CDF range: {detail}")]
pub struct DeltaCdfDownstreamFailure {
    detail: Arc<str>,
}

impl DeltaCdfDownstreamFailure {
    /// Describe a downstream failure without manufacturing a success token.
    #[must_use]
    pub fn new(detail: impl Into<Arc<str>>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    /// Application-owned downstream failure detail.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// Failure to turn executed CDF batches into a checkpoint-advancement token.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DeltaCdfDownstreamCompletionError {
    /// The downstream side effect failed and no token was issued.
    #[error(transparent)]
    Downstream(#[from] DeltaCdfDownstreamFailure),
    /// The claimed durable commit identity was invalid.
    #[error(transparent)]
    InvalidCommit(#[from] DeltaCdfCheckpointError),
}

/// Opaque proof that one executed CDF range was durably applied downstream.
///
/// Fields are private, there is no public constructor, and the token is not
/// cloneable. It can only be produced by consuming a successful execution and
/// is itself consumed by checkpoint advancement.
#[must_use = "a downstream-success token must advance its matching durable checkpoint"]
pub struct DeltaCdfDownstreamSuccess {
    consumer_id: Arc<str>,
    checkpoint_before: ExactDeltaPin,
    source_through: ExactDeltaPin,
    window: DeltaCdfReadWindow,
    downstream_commit: DeltaCdfDownstreamCommit,
}

impl fmt::Debug for DeltaCdfDownstreamSuccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeltaCdfDownstreamSuccess")
            .field("consumer_id", &self.consumer_id)
            .field("checkpoint_before", &self.checkpoint_before)
            .field("source_through", &self.source_through)
            .field("window", &self.window)
            .finish_non_exhaustive()
    }
}

/// Prepare a production CDF scan for one explicit inclusive version range.
///
/// The loaded table must be the canonical source root at the exact inclusive
/// end. The range must start immediately after `checkpoint`. Before delta-rs is
/// invoked, the end entry is read and decoded first, then every preceding entry
/// in the requested range is read and decoded. A missing entry returns a typed
/// full-snapshot fallback. The scan always supplies both version bounds and
/// never enables delta-rs out-of-range tolerance.
pub async fn prepare_exact_delta_cdf(
    checkpoint: &DurableDeltaCdfCheckpoint,
    table: &DeltaTable,
    source_through: &ExactDeltaPin,
    window: DeltaCdfReadWindow,
    epoch_session: Arc<SessionState>,
) -> Result<ExactDeltaCdfPreparation, ExactDeltaCdfPreparationError> {
    let expected_starting_version = checkpoint
        .consumed_through()
        .version()
        .checked_add(1)
        .ok_or(ExactDeltaCdfPreparationError::VersionOverflow)?;
    if window.ending_version < window.starting_version {
        return Err(ExactDeltaCdfPreparationError::InvalidWindow {
            starting_version: window.starting_version,
            ending_version: window.ending_version,
        });
    }
    if window.starting_version != expected_starting_version {
        return Err(ExactDeltaCdfPreparationError::NonContiguousWindow {
            expected_starting_version,
            actual_starting_version: window.starting_version,
        });
    }
    if source_through.version() != window.ending_version {
        return Err(ExactDeltaCdfPreparationError::InclusiveEndMismatch {
            source_version: source_through.version(),
            ending_version: window.ending_version,
        });
    }
    if checkpoint.consumed_through().canonical_root() != source_through.canonical_root() {
        return Err(ExactDeltaCdfPreparationError::CheckpointRootMismatch {
            checkpoint_root: checkpoint.consumed_through().canonical_root().to_string(),
            source_root: source_through.canonical_root().to_string(),
        });
    }

    let loaded_version = table
        .version()
        .ok_or(ExactDeltaCdfPreparationError::MissingLoadedVersion)?;
    let loaded_root = canonical_delta_root(table.table_url())
        .map_err(|error| ExactDeltaCdfPreparationError::InvalidSourceRoot(error.to_string()))?;
    if &loaded_root != source_through.canonical_root() || loaded_version != source_through.version()
    {
        return Err(ExactDeltaCdfPreparationError::LoadedSourceMismatch {
            expected_root: source_through.canonical_root().to_string(),
            expected_version: source_through.version(),
            loaded_root: loaded_root.to_string(),
            loaded_version,
        });
    }

    if let Some(reason) = first_missing_cdf_log_entry(table, window).await? {
        return Ok(ExactDeltaCdfPreparation::ExactSnapshotFallback(
            exact_snapshot_fallback(checkpoint, source_through, window, reason),
        ));
    }

    let plan_result = table
        .clone()
        .scan_cdf()
        .with_starting_version(window.starting_version)
        .with_ending_version(window.ending_version)
        .build(epoch_session.as_ref(), None)
        .await;
    let physical_plan = match plan_result {
        Ok(plan) => plan,
        Err(source) => {
            // Retention may race preparation. Re-probe end-first so a newly
            // missing entry still becomes an explicit reconstruction request.
            if let Some(reason) = first_missing_cdf_log_entry(table, window).await? {
                return Ok(ExactDeltaCdfPreparation::ExactSnapshotFallback(
                    exact_snapshot_fallback(checkpoint, source_through, window, reason),
                ));
            }
            return Err(ExactDeltaCdfPreparationError::BuildPlan(source));
        }
    };
    let (commit_version_index, change_type_index) =
        validate_cdf_output_schema(physical_plan.schema().as_ref())?;

    Ok(ExactDeltaCdfPreparation::PhysicalPlan(
        PreparedExactDeltaCdfRead {
            consumer_id: Arc::clone(&checkpoint.consumer_id),
            checkpoint_before: checkpoint.consumed_through.clone(),
            source_through: source_through.clone(),
            window,
            physical_plan,
            epoch_session,
            commit_version_index,
            change_type_index,
        },
    ))
}

async fn first_missing_cdf_log_entry(
    table: &DeltaTable,
    window: DeltaCdfReadWindow,
) -> Result<Option<ExactDeltaCdfFallbackReason>, ExactDeltaCdfPreparationError> {
    if !read_and_decode_cdf_log_entry(table, window.ending_version).await? {
        return Ok(Some(
            ExactDeltaCdfFallbackReason::EndingVersionNotRetained {
                version: window.ending_version,
            },
        ));
    }
    for version in window.starting_version..window.ending_version {
        if !read_and_decode_cdf_log_entry(table, version).await? {
            return Ok(Some(
                ExactDeltaCdfFallbackReason::RequiredVersionNotRetained { version },
            ));
        }
    }
    Ok(None)
}

async fn read_and_decode_cdf_log_entry(
    table: &DeltaTable,
    version: u64,
) -> Result<bool, ExactDeltaCdfPreparationError> {
    let Some(bytes) = table
        .log_store()
        .read_commit_entry(version)
        .await
        .map_err(|source| ExactDeltaCdfPreparationError::ReadLogEntry { version, source })?
    else {
        return Ok(false);
    };
    get_actions(version, &bytes)
        .map_err(|source| ExactDeltaCdfPreparationError::DecodeLogEntry { version, source })?;
    Ok(true)
}

fn exact_snapshot_fallback(
    checkpoint: &DurableDeltaCdfCheckpoint,
    source_through: &ExactDeltaPin,
    window: DeltaCdfReadWindow,
    reason: ExactDeltaCdfFallbackReason,
) -> ExactDeltaSnapshotFallbackRequired {
    ExactDeltaSnapshotFallbackRequired {
        consumer_id: Arc::clone(&checkpoint.consumer_id),
        checkpoint_before: checkpoint.consumed_through.clone(),
        reconstruct_at: source_through.clone(),
        requested_window: window,
        reason,
    }
}

fn validate_cdf_output_schema(
    schema: &arrow_schema::Schema,
) -> Result<(usize, usize), ExactDeltaCdfPreparationError> {
    let commit_version_index = schema.index_of(COMMIT_VERSION_COL).map_err(|_| {
        ExactDeltaCdfPreparationError::MissingOutputColumn {
            column: COMMIT_VERSION_COL,
        }
    })?;
    let commit_version_type = schema.field(commit_version_index).data_type();
    if commit_version_type != &DataType::UInt64 {
        return Err(ExactDeltaCdfPreparationError::WrongOutputColumnType {
            column: COMMIT_VERSION_COL,
            expected: DataType::UInt64,
            actual: commit_version_type.clone(),
        });
    }
    let change_type_index = schema.index_of(CHANGE_TYPE_COL).map_err(|_| {
        ExactDeltaCdfPreparationError::MissingOutputColumn {
            column: CHANGE_TYPE_COL,
        }
    })?;
    let change_type = schema.field(change_type_index).data_type();
    if change_type != &DataType::Utf8 {
        return Err(ExactDeltaCdfPreparationError::WrongOutputColumnType {
            column: CHANGE_TYPE_COL,
            expected: DataType::Utf8,
            actual: change_type.clone(),
        });
    }
    Ok((commit_version_index, change_type_index))
}

fn validate_cdf_batches(
    batches: &[RecordBatch],
    commit_version_index: usize,
    change_type_index: usize,
    window: DeltaCdfReadWindow,
) -> Result<(), ExactDeltaCdfExecutionError> {
    for (batch_index, batch) in batches.iter().enumerate() {
        let commit_versions = batch
            .column(commit_version_index)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or(ExactDeltaCdfExecutionError::WrongBatchColumnType {
                batch_index,
                column: COMMIT_VERSION_COL,
            })?;
        let change_types = batch
            .column(change_type_index)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(ExactDeltaCdfExecutionError::WrongBatchColumnType {
                batch_index,
                column: CHANGE_TYPE_COL,
            })?;
        for row_index in 0..batch.num_rows() {
            if commit_versions.is_null(row_index) {
                return Err(ExactDeltaCdfExecutionError::NullCommitVersion {
                    batch_index,
                    row_index,
                });
            }
            let version = commit_versions.value(row_index);
            if version < window.starting_version || version > window.ending_version {
                return Err(ExactDeltaCdfExecutionError::CommitVersionOutOfRange {
                    batch_index,
                    row_index,
                    version,
                    starting_version: window.starting_version,
                    ending_version: window.ending_version,
                });
            }
            if change_types.is_null(row_index) {
                return Err(ExactDeltaCdfExecutionError::NullChangeType {
                    batch_index,
                    row_index,
                });
            }
            let change_type = change_types.value(row_index);
            if !matches!(
                change_type,
                "insert" | "delete" | "update_preimage" | "update_postimage"
            ) {
                return Err(ExactDeltaCdfExecutionError::UnsupportedChangeType {
                    batch_index,
                    row_index,
                    change_type: change_type.to_owned(),
                });
            }
        }
    }
    Ok(())
}

/// Failure while validating or preparing an explicit exact CDF range.
#[derive(Debug, Error)]
pub enum ExactDeltaCdfPreparationError {
    #[error("CDF version range [{starting_version}, {ending_version}] is not ordered")]
    InvalidWindow {
        starting_version: u64,
        ending_version: u64,
    },
    #[error(
        "CDF range is not contiguous with its checkpoint: expected start {expected_starting_version}, got {actual_starting_version}"
    )]
    NonContiguousWindow {
        expected_starting_version: u64,
        actual_starting_version: u64,
    },
    #[error(
        "CDF inclusive end {ending_version} does not equal exact source version {source_version}"
    )]
    InclusiveEndMismatch {
        source_version: u64,
        ending_version: u64,
    },
    #[error("CDF checkpoint root {checkpoint_root} differs from source root {source_root}")]
    CheckpointRootMismatch {
        checkpoint_root: String,
        source_root: String,
    },
    #[error("CDF source table has no loaded version")]
    MissingLoadedVersion,
    #[error("invalid CDF source table root: {0}")]
    InvalidSourceRoot(String),
    #[error(
        "loaded CDF source mismatch: expected {expected_root}@{expected_version}, loaded {loaded_root}@{loaded_version}"
    )]
    LoadedSourceMismatch {
        expected_root: String,
        expected_version: u64,
        loaded_root: String,
        loaded_version: u64,
    },
    #[error("CDF checkpoint version overflow")]
    VersionOverflow,
    #[error("failed to read required CDF log entry {version}: {source}")]
    ReadLogEntry {
        version: u64,
        #[source]
        source: DeltaTableError,
    },
    #[error("failed to decode required CDF log entry {version}: {source}")]
    DecodeLogEntry {
        version: u64,
        #[source]
        source: DeltaTableError,
    },
    #[error("delta-rs rejected the exact CDF physical plan: {0}")]
    BuildPlan(DeltaTableError),
    #[error("delta-rs CDF output omitted required column {column}")]
    MissingOutputColumn { column: &'static str },
    #[error("delta-rs CDF output column {column} has type {actual:?}, expected {expected:?}")]
    WrongOutputColumnType {
        column: &'static str,
        expected: DataType,
        actual: DataType,
    },
}

/// Failure while executing or validating a prepared exact CDF plan.
#[derive(Debug, Error)]
pub enum ExactDeltaCdfExecutionError {
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
    #[error("CDF batch {batch_index} column {column} has an unexpected Arrow type")]
    WrongBatchColumnType {
        batch_index: usize,
        column: &'static str,
    },
    #[error("CDF batch {batch_index} row {row_index} has a null commit version")]
    NullCommitVersion {
        batch_index: usize,
        row_index: usize,
    },
    #[error(
        "CDF batch {batch_index} row {row_index} has commit version {version} outside inclusive range [{starting_version}, {ending_version}]"
    )]
    CommitVersionOutOfRange {
        batch_index: usize,
        row_index: usize,
        version: u64,
        starting_version: u64,
        ending_version: u64,
    },
    #[error("CDF batch {batch_index} row {row_index} has a null change type")]
    NullChangeType {
        batch_index: usize,
        row_index: usize,
    },
    #[error("CDF batch {batch_index} row {row_index} has unsupported change type {change_type:?}")]
    UnsupportedChangeType {
        batch_index: usize,
        row_index: usize,
        change_type: String,
    },
}

/// Durable acknowledgement for the side effect completed before a CDF
/// consumer checkpoint may advance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeltaCdfDownstreamCommit {
    /// Exact downstream Delta commit containing the consumed changes.
    Delta(ExactDeltaPin),
    /// Application-owned durable acknowledgement for a non-Delta sink.
    External([u8; 32]),
}

impl DeltaCdfDownstreamCommit {
    fn has_zero_identity(&self) -> bool {
        matches!(self, Self::External(identity) if identity.iter().all(|byte| *byte == 0))
    }
}

/// One exact inclusive CDF range selected from a durable checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeltaCdfReadWindow {
    starting_version: u64,
    ending_version: u64,
}

impl DeltaCdfReadWindow {
    #[must_use]
    pub const fn starting_version(self) -> u64 {
        self.starting_version
    }

    #[must_use]
    pub const fn ending_version(self) -> u64 {
        self.ending_version
    }
}

/// Application-owned, durable CDF consumer checkpoint.
///
/// `_commit_version`, not timestamp, is the ordering identity. The checkpoint
/// names the exact source root/version already consumed and the durable sink
/// commit which justified advancement. It never discovers latest state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableDeltaCdfCheckpoint {
    consumer_id: Arc<str>,
    cdf_activation_version: u64,
    consumed_through: ExactDeltaPin,
    downstream_commit: DeltaCdfDownstreamCommit,
}

impl DurableDeltaCdfCheckpoint {
    /// Construct a checkpoint only after its downstream effect is durable.
    ///
    /// # Errors
    ///
    /// Rejects an empty consumer identity, a zero external acknowledgement, or
    /// a consumed version before CDF activation.
    pub fn try_new(
        consumer_id: impl Into<Arc<str>>,
        cdf_activation_version: u64,
        consumed_through: ExactDeltaPin,
        downstream_commit: DeltaCdfDownstreamCommit,
    ) -> Result<Self, DeltaCdfCheckpointError> {
        let consumer_id = consumer_id.into();
        if consumer_id.trim().is_empty() {
            return Err(DeltaCdfCheckpointError::EmptyConsumerIdentity);
        }
        if downstream_commit.has_zero_identity() {
            return Err(DeltaCdfCheckpointError::ZeroDownstreamCommitIdentity);
        }
        if consumed_through.version() < cdf_activation_version {
            return Err(DeltaCdfCheckpointError::BeforeCdfActivation {
                activation_version: cdf_activation_version,
                consumed_version: consumed_through.version(),
            });
        }
        Ok(Self {
            consumer_id,
            cdf_activation_version,
            consumed_through,
            downstream_commit,
        })
    }

    #[must_use]
    pub fn consumer_id(&self) -> &str {
        &self.consumer_id
    }

    #[must_use]
    pub const fn cdf_activation_version(&self) -> u64 {
        self.cdf_activation_version
    }

    #[must_use]
    pub const fn consumed_through(&self) -> &ExactDeltaPin {
        &self.consumed_through
    }

    #[must_use]
    pub const fn downstream_commit(&self) -> &DeltaCdfDownstreamCommit {
        &self.downstream_commit
    }

    /// Select the next exact inclusive CDF window after checking the retained
    /// source range. `earliest_available_version` must come from the source's
    /// retention-aware history observation; the method does not infer it.
    pub fn next_window(
        &self,
        earliest_available_version: u64,
        latest_available_version: u64,
    ) -> Result<Option<DeltaCdfReadWindow>, DeltaCdfCheckpointError> {
        if latest_available_version < self.consumed_through.version() {
            return Err(DeltaCdfCheckpointError::SourceRegressed {
                checkpoint_version: self.consumed_through.version(),
                latest_available_version,
            });
        }
        let starting_version = self
            .consumed_through
            .version()
            .checked_add(1)
            .ok_or(DeltaCdfCheckpointError::VersionOverflow)?;
        if starting_version > latest_available_version {
            return Ok(None);
        }
        if earliest_available_version > starting_version {
            return Err(DeltaCdfCheckpointError::RetentionGap {
                required_starting_version: starting_version,
                earliest_available_version,
            });
        }
        Ok(Some(DeltaCdfReadWindow {
            starting_version,
            ending_version: latest_available_version,
        }))
    }

    /// Advance only by consuming an opaque downstream-success token.
    ///
    /// The token is bound to this consumer, this exact prior checkpoint, and a
    /// contiguous explicit range. It can only be produced after successful CDF
    /// execution and a nonzero durable downstream acknowledgement.
    pub fn advance_after_downstream_success(
        &self,
        success: DeltaCdfDownstreamSuccess,
    ) -> Result<Self, DeltaCdfCheckpointError> {
        if success.consumer_id != self.consumer_id
            || success.checkpoint_before != self.consumed_through
        {
            return Err(DeltaCdfCheckpointError::DownstreamSuccessMismatch);
        }
        let expected_start = self
            .consumed_through
            .version()
            .checked_add(1)
            .ok_or(DeltaCdfCheckpointError::VersionOverflow)?;
        if success.window.starting_version != expected_start
            || success.window.ending_version < success.window.starting_version
        {
            return Err(DeltaCdfCheckpointError::NonContiguousAdvance {
                expected_starting_version: expected_start,
                actual_starting_version: success.window.starting_version,
                ending_version: success.window.ending_version,
            });
        }
        if success.source_through.canonical_root() != self.consumed_through.canonical_root()
            || success.source_through.version() != success.window.ending_version
        {
            return Err(DeltaCdfCheckpointError::DownstreamSuccessMismatch);
        }
        Self::try_new(
            Arc::clone(&self.consumer_id),
            self.cdf_activation_version,
            success.source_through,
            success.downstream_commit,
        )
    }
}

/// Fail-closed CDF checkpoint/range validation errors.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DeltaCdfCheckpointError {
    #[error("CDF consumer identity must be nonempty")]
    EmptyConsumerIdentity,
    #[error("CDF downstream commit identity must be nonzero")]
    ZeroDownstreamCommitIdentity,
    #[error(
        "CDF checkpoint version {consumed_version} precedes activation version {activation_version}"
    )]
    BeforeCdfActivation {
        activation_version: u64,
        consumed_version: u64,
    },
    #[error(
        "CDF source latest version {latest_available_version} regressed behind checkpoint {checkpoint_version}"
    )]
    SourceRegressed {
        checkpoint_version: u64,
        latest_available_version: u64,
    },
    #[error(
        "CDF retention gap: required version {required_starting_version}, earliest available is {earliest_available_version}"
    )]
    RetentionGap {
        required_starting_version: u64,
        earliest_available_version: u64,
    },
    #[error(
        "CDF checkpoint advance is not contiguous: expected start {expected_starting_version}, actual [{actual_starting_version}, {ending_version}]"
    )]
    NonContiguousAdvance {
        expected_starting_version: u64,
        actual_starting_version: u64,
        ending_version: u64,
    },
    #[error("CDF checkpoint version overflow")]
    VersionOverflow,
    #[error("CDF downstream-success token does not match this exact consumer checkpoint")]
    DownstreamSuccessMismatch,
    #[error("invalid CDF source table root: {0}")]
    InvalidSourceRoot(String),
}

/// Durable authority family which can keep an epoch resource reachable.
///
/// The variants are the fixed retention semantics. Concrete authority identities, protected
/// resources, and expiry are runtime relation rows supplied by the command/activation layer.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DeltaRetentionAuthorityKind {
    ActiveEpoch,
    InFlightPublication,
    QueryLease,
    ResultLease,
    RollbackLease,
    ExpectationLease,
    ProgramReleaseLease,
    CdfConsumerCheckpoint,
    AuditHold,
}

impl DeltaRetentionAuthorityKind {
    /// Complete fixed set of retention-source relations which must be observed, even when one
    /// source currently contributes zero active claims.
    pub const ALL: [Self; 9] = [
        Self::ActiveEpoch,
        Self::InFlightPublication,
        Self::QueryLease,
        Self::ResultLease,
        Self::RollbackLease,
        Self::ExpectationLease,
        Self::ProgramReleaseLease,
        Self::CdfConsumerCheckpoint,
        Self::AuditHold,
    ];

    const fn requires_expiry(self) -> bool {
        matches!(
            self,
            Self::QueryLease
                | Self::ResultLease
                | Self::RollbackLease
                | Self::ExpectationLease
                | Self::ProgramReleaseLease
        )
    }
}

/// One resource whose deletion can invalidate a retained fabric observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeltaRetainedResource {
    DeltaVersion(ExactDeltaPin),
    ImmutableSegment([u8; 32]),
    ProgramRelease([u8; 32]),
    Expectation([u8; 32]),
    QueryResult([u8; 32]),
    RollbackPoint([u8; 32]),
}

impl DeltaRetainedResource {
    fn stable_key(&self) -> String {
        match self {
            Self::DeltaVersion(pin) => {
                format!("delta:{}@{:020}", pin.canonical_root(), pin.version())
            }
            Self::ImmutableSegment(id) => format!("segment:{}", lower_hex(id)),
            Self::ProgramRelease(id) => format!("program-release:{}", lower_hex(id)),
            Self::Expectation(id) => format!("expectation:{}", lower_hex(id)),
            Self::QueryResult(id) => format!("result:{}", lower_hex(id)),
            Self::RollbackPoint(id) => format!("rollback:{}", lower_hex(id)),
        }
    }

    fn has_zero_identity(&self) -> bool {
        match self {
            Self::DeltaVersion(_) => false,
            Self::ImmutableSegment(id)
            | Self::ProgramRelease(id)
            | Self::Expectation(id)
            | Self::QueryResult(id)
            | Self::RollbackPoint(id) => id.iter().all(|byte| *byte == 0),
        }
    }
}

/// One row replayed from activation or lease relations into the retention closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaRetentionClaim {
    authority_kind: DeltaRetentionAuthorityKind,
    authority_id: [u8; 32],
    protected: DeltaRetainedResource,
    expires_at: Option<u64>,
}

impl DeltaRetentionClaim {
    /// Construct one runtime retention row.
    ///
    /// Active epochs, in-flight publications, CDF checkpoints, and audit holds
    /// are non-expiring until their owning relation removes or supersedes them.
    /// Time-bounded lease authorities must carry an expiry; an expired row
    /// remains valid input and is omitted when the closure is evaluated at
    /// `now`.
    ///
    /// # Errors
    ///
    /// Rejects zero identities and authority/expiry shape drift.
    pub fn try_new(
        authority_kind: DeltaRetentionAuthorityKind,
        authority_id: [u8; 32],
        protected: DeltaRetainedResource,
        expires_at: Option<u64>,
    ) -> Result<Self, DeltaRetentionClosureError> {
        if authority_id.iter().all(|byte| *byte == 0) {
            return Err(DeltaRetentionClosureError::ZeroAuthorityIdentity);
        }
        if protected.has_zero_identity() {
            return Err(DeltaRetentionClosureError::ZeroResourceIdentity);
        }
        if authority_kind.requires_expiry() != expires_at.is_some() {
            return Err(DeltaRetentionClosureError::InvalidExpiryShape {
                authority_kind,
                expires_at,
            });
        }
        Ok(Self {
            authority_kind,
            authority_id,
            protected,
            expires_at,
        })
    }

    #[must_use]
    pub const fn authority_kind(&self) -> DeltaRetentionAuthorityKind {
        self.authority_kind
    }

    #[must_use]
    pub const fn authority_id(&self) -> &[u8; 32] {
        &self.authority_id
    }

    #[must_use]
    pub const fn protected_resource(&self) -> &DeltaRetainedResource {
        &self.protected
    }

    #[must_use]
    pub const fn expires_at(&self) -> Option<u64> {
        self.expires_at
    }

    fn is_active_at(&self, now: u64) -> bool {
        self.expires_at.is_none_or(|expires_at| expires_at > now)
    }

    fn stable_key(&self) -> String {
        format!(
            "{:?}:{}:{}",
            self.authority_kind,
            lower_hex(&self.authority_id),
            self.protected.stable_key()
        )
    }
}

/// Observed closure cardinalities, including expired rows excluded from protection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeltaRetentionClosureObservation {
    pub authority_sources_observed: usize,
    pub supplied_claims: usize,
    pub active_claims: usize,
    pub expired_claims: usize,
    pub protected_resources: usize,
    pub protected_delta_versions: usize,
}

/// Exact active retention union used to configure vacuum and reject resource deletion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaRetentionClosure {
    evaluated_at: u64,
    active_claims: Vec<DeltaRetentionClaim>,
    protected_resources: Vec<DeltaRetainedResource>,
    observation: DeltaRetentionClosureObservation,
}

impl DeltaRetentionClosure {
    /// Evaluate activation/lease rows at one exact clock observation.
    ///
    /// `observed_authorities` distinguishes an observed-empty source relation from a missing
    /// source. All fixed authority families must be present before absence can be interpreted.
    ///
    /// # Errors
    ///
    /// Rejects duplicate authority-resource rows instead of silently choosing one expiry.
    pub fn try_new(
        evaluated_at: u64,
        observed_authorities: impl IntoIterator<Item = DeltaRetentionAuthorityKind>,
        claims: impl IntoIterator<Item = DeltaRetentionClaim>,
    ) -> Result<Self, DeltaRetentionClosureError> {
        let observed_authorities = observed_authorities.into_iter().collect::<BTreeSet<_>>();
        let missing = DeltaRetentionAuthorityKind::ALL
            .into_iter()
            .filter(|kind| !observed_authorities.contains(kind))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(DeltaRetentionClosureError::IncompleteAuthorityCoverage { missing });
        }
        let claims = claims.into_iter().collect::<Vec<_>>();
        let mut keys = BTreeSet::new();
        for claim in &claims {
            if !keys.insert(claim.stable_key()) {
                return Err(DeltaRetentionClosureError::DuplicateClaim {
                    authority_kind: claim.authority_kind,
                    authority_id: claim.authority_id,
                    resource: Box::new(claim.protected.clone()),
                });
            }
        }

        let mut active_claims = claims
            .iter()
            .filter(|claim| claim.is_active_at(evaluated_at))
            .cloned()
            .collect::<Vec<_>>();
        active_claims.sort_by_key(DeltaRetentionClaim::stable_key);
        let mut protected_resources = active_claims
            .iter()
            .map(|claim| claim.protected.clone())
            .collect::<Vec<_>>();
        protected_resources.sort_by_key(DeltaRetainedResource::stable_key);
        protected_resources.dedup();

        let observation = DeltaRetentionClosureObservation {
            authority_sources_observed: observed_authorities.len(),
            supplied_claims: claims.len(),
            active_claims: active_claims.len(),
            expired_claims: claims.len() - active_claims.len(),
            protected_resources: protected_resources.len(),
            protected_delta_versions: protected_resources
                .iter()
                .filter(|resource| matches!(resource, DeltaRetainedResource::DeltaVersion(_)))
                .count(),
        };
        Ok(Self {
            evaluated_at,
            active_claims,
            protected_resources,
            observation,
        })
    }

    #[must_use]
    pub const fn evaluated_at(&self) -> u64 {
        self.evaluated_at
    }

    #[must_use]
    pub fn active_claims(&self) -> &[DeltaRetentionClaim] {
        &self.active_claims
    }

    #[must_use]
    pub fn protected_resources(&self) -> &[DeltaRetainedResource] {
        &self.protected_resources
    }

    #[must_use]
    pub const fn observation(&self) -> DeltaRetentionClosureObservation {
        self.observation
    }

    /// Exact sorted `with_keep_versions` input for one canonical table root.
    ///
    /// # Errors
    ///
    /// Rejects a root that cannot be canonicalized under the same rules as epoch pins.
    pub fn keep_versions_for(
        &self,
        table_root: &Url,
    ) -> Result<Vec<u64>, DeltaRetentionClosureError> {
        let canonical_root = canonical_delta_root(table_root)?;
        Ok(self
            .protected_resources
            .iter()
            .filter_map(|resource| match resource {
                DeltaRetainedResource::DeltaVersion(pin)
                    if pin.canonical_root() == &canonical_root =>
                {
                    Some(pin.version())
                }
                _ => None,
            })
            .collect())
    }

    /// Validate the application-owned contract around a delta-rs vacuum dry run.
    ///
    /// This does not authorize destructive execution. It proves only that the preflight was a
    /// dry run and received exactly the active protected versions derived for this table.
    ///
    /// # Errors
    ///
    /// Rejects a destructive first pass or any missing, extra, duplicate, or unsorted version.
    pub fn validate_vacuum_dry_run_contract(
        &self,
        table_root: &Url,
        dry_run: bool,
        configured_keep_versions: &[u64],
    ) -> Result<(), DeltaRetentionClosureError> {
        if !dry_run {
            return Err(DeltaRetentionClosureError::DestructiveFirstPass);
        }
        let expected = self.keep_versions_for(table_root)?;
        if configured_keep_versions != expected {
            return Err(DeltaRetentionClosureError::KeepVersionsMismatch {
                expected,
                actual: configured_keep_versions.to_vec(),
            });
        }
        Ok(())
    }

    /// Reject a proposed deletion set which intersects any active retained resource.
    ///
    /// # Errors
    ///
    /// Returns the first protected resource in deterministic input order.
    pub fn validate_resource_deletions(
        &self,
        candidates: &[DeltaRetainedResource],
    ) -> Result<(), DeltaRetentionClosureError> {
        for candidate in candidates {
            if self.protected_resources.contains(candidate) {
                return Err(DeltaRetentionClosureError::ProtectedResourceDeletion(
                    Box::new(candidate.clone()),
                ));
            }
        }
        Ok(())
    }
}

/// Fail-closed retention-closure and vacuum-preflight errors.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DeltaRetentionClosureError {
    #[error("retention closure did not observe authority sources {missing:?}")]
    IncompleteAuthorityCoverage {
        missing: Vec<DeltaRetentionAuthorityKind>,
    },
    #[error("retention authority identity must be nonzero")]
    ZeroAuthorityIdentity,
    #[error("retained resource identity must be nonzero")]
    ZeroResourceIdentity,
    #[error("retention authority {authority_kind:?} has invalid expiry shape {expires_at:?}")]
    InvalidExpiryShape {
        authority_kind: DeltaRetentionAuthorityKind,
        expires_at: Option<u64>,
    },
    #[error(
        "duplicate retention claim for {authority_kind:?} authority {authority_id:?} and {resource:?}"
    )]
    DuplicateClaim {
        authority_kind: DeltaRetentionAuthorityKind,
        authority_id: [u8; 32],
        resource: Box<DeltaRetainedResource>,
    },
    #[error("vacuum must begin with a dry run")]
    DestructiveFirstPass,
    #[error("vacuum keep_versions mismatch: expected {expected:?}, actual {actual:?}")]
    KeepVersionsMismatch {
        expected: Vec<u64>,
        actual: Vec<u64>,
    },
    #[error("retention candidate would delete protected resource {0:?}")]
    ProtectedResourceDeletion(Box<DeltaRetainedResource>),
    #[error("invalid retained Delta table root: {0}")]
    InvalidTableRoot(String),
}

impl From<ExactDeltaProviderError> for DeltaRetentionClosureError {
    fn from(error: ExactDeltaProviderError) -> Self {
        match error {
            ExactDeltaProviderError::InvalidTableRoot(detail) => Self::InvalidTableRoot(detail),
            other => Self::InvalidTableRoot(other.to_string()),
        }
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn canonical_delta_root(root: &Url) -> Result<Url, ExactDeltaProviderError> {
    if root.cannot_be_a_base() {
        return Err(ExactDeltaProviderError::InvalidTableRoot(
            "URL cannot be a hierarchical table root".to_owned(),
        ));
    }

    let mut root = normalize_table_url(root);
    if root.password().is_some() {
        root.set_password(None).map_err(|()| {
            ExactDeltaProviderError::InvalidTableRoot("URL password cannot be removed".to_owned())
        })?;
    }
    if !root.username().is_empty() {
        root.set_username("").map_err(|()| {
            ExactDeltaProviderError::InvalidTableRoot("URL username cannot be removed".to_owned())
        })?;
    }
    root.set_query(None);
    root.set_fragment(None);

    if root.scheme() == "file" {
        let path = root.to_file_path().map_err(|()| {
            ExactDeltaProviderError::InvalidTableRoot(
                "file URL cannot be converted to a filesystem path".to_owned(),
            )
        })?;
        let canonical = std::fs::canonicalize(path).map_err(|error| {
            ExactDeltaProviderError::InvalidTableRoot(format!(
                "local table root cannot be canonicalized: {error}"
            ))
        })?;
        root = Url::from_directory_path(canonical).map_err(|()| {
            ExactDeltaProviderError::InvalidTableRoot(
                "canonical local path cannot be converted to a file URL".to_owned(),
            )
        })?;
    }

    Ok(normalize_table_url(&root))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use arrow_array::{RecordBatch, StringArray, UInt64Array};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::prelude::{SessionConfig, SessionContext};
    use deltalake::DeltaTableBuilder;
    use deltalake::TableProperty;
    use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
    use deltalake::kernel::transaction::{PROTOCOL, TransactionError};
    use deltalake::operations::create::CreateBuilder;
    use deltalake::protocol::SaveMode;
    use serde_json::{Map, Value, json};
    use tempfile::TempDir;

    use super::*;

    const WP33_EXPECTATIONS: &str =
        include_str!("../../contracts/acceptance/relational-fabric-v3/expectations.jsonl");
    const WP33_FIXTURES: &str =
        include_str!("../../contracts/acceptance/relational-fabric-v3/negative-fixtures.jsonl");

    struct Fixture {
        _temporary: TempDir,
        root: Url,
        table: DeltaTable,
        pin: ExactDeltaPin,
    }

    struct CdfFixture {
        _temporary: TempDir,
        root: Url,
        version_zero: DeltaTable,
    }

    async fn fixture() -> Fixture {
        let temporary = TempDir::new().expect("temporary Delta fixture root");
        let table_path = temporary.path().join("table");
        fs::create_dir_all(&table_path).expect("create Delta fixture directory");
        let root = Url::from_directory_path(&table_path).expect("fixture file URL");
        let arrow_schema = Schema::new(vec![Field::new("label", DataType::Utf8, true)]);
        let kernel: deltalake::kernel::StructType = (&arrow_schema)
            .try_into_kernel()
            .expect("Arrow fixture schema converts to Delta");

        CreateBuilder::new()
            .with_location(root.to_string())
            .with_table_name("exact_delta_provider_fixture")
            .with_save_mode(SaveMode::ErrorIfExists)
            .with_columns(kernel.fields().cloned())
            .await
            .expect("create exact Delta fixture");

        let table = DeltaTableBuilder::from_url(root.clone())
            .expect("construct exact-version table builder")
            .with_version(0)
            .load()
            .await
            .expect("load fixture version zero");
        let pin = ExactDeltaPin::new(&root, 0).expect("fixture exact pin");
        Fixture {
            _temporary: temporary,
            root,
            table,
            pin,
        }
    }

    async fn cdf_fixture() -> CdfFixture {
        let temporary = TempDir::new().expect("temporary CDF fixture root");
        let table_path = temporary.path().join("table");
        fs::create_dir_all(&table_path).expect("create CDF fixture directory");
        let root = Url::from_directory_path(&table_path).expect("CDF fixture file URL");
        let arrow_schema = Schema::new(vec![Field::new("label", DataType::Utf8, true)]);
        let kernel: deltalake::kernel::StructType = (&arrow_schema)
            .try_into_kernel()
            .expect("Arrow CDF fixture schema converts to Delta");

        let version_zero = CreateBuilder::new()
            .with_location(root.to_string())
            .with_table_name("exact_delta_cdf_fixture")
            .with_save_mode(SaveMode::ErrorIfExists)
            .with_columns(kernel.fields().cloned())
            .with_configuration_property(TableProperty::EnableChangeDataFeed, Some("true"))
            .await
            .expect("create CDF-enabled Delta fixture");
        assert_eq!(version_zero.version(), Some(0));
        CdfFixture {
            _temporary: temporary,
            root,
            version_zero,
        }
    }

    fn label_batch(label: &str) -> RecordBatch {
        RecordBatch::try_new(
            Arc::new(Schema::new(vec![Field::new("label", DataType::Utf8, true)])),
            vec![Arc::new(StringArray::from(vec![Some(label)]))],
        )
        .expect("CDF label batch")
    }

    async fn append_label(table: &DeltaTable, label: &str) -> DeltaTable {
        table
            .clone()
            .write(vec![label_batch(label)])
            .await
            .expect("append CDF fixture label")
    }

    fn cdf_checkpoint(root: &Url, consumed_version: u64) -> DurableDeltaCdfCheckpoint {
        DurableDeltaCdfCheckpoint::try_new(
            "derived-current-view",
            0,
            ExactDeltaPin::new(root, consumed_version).expect("CDF checkpoint pin"),
            DeltaCdfDownstreamCommit::External([1; 32]),
        )
        .expect("CDF checkpoint")
    }

    fn cdf_values(executed: &ExecutedExactDeltaCdfRead) -> (Vec<String>, Vec<u64>) {
        let mut labels = Vec::new();
        let mut versions = Vec::new();
        for batch in executed.batches() {
            let label_array = batch
                .column_by_name("label")
                .expect("CDF label column")
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("CDF label string array");
            let version_array = batch
                .column_by_name(COMMIT_VERSION_COL)
                .expect("CDF commit-version column")
                .as_any()
                .downcast_ref::<UInt64Array>()
                .expect("CDF commit-version array");
            for row in 0..batch.num_rows() {
                labels.push(label_array.value(row).to_owned());
                versions.push(version_array.value(row));
            }
        }
        (labels, versions)
    }

    fn remove_commit_entry(root: &Url, version: u64) {
        let table_path = root
            .to_file_path()
            .expect("CDF fixture Delta root must be a local path");
        fs::remove_file(
            table_path
                .join("_delta_log")
                .join(format!("{version:020}.json")),
        )
        .expect("remove CDF fixture commit entry");
    }

    fn epoch_session() -> Arc<SessionState> {
        let config = SessionConfig::new()
            .set_bool(
                "datafusion.execution.parquet.schema_force_view_types",
                false,
            )
            .set_bool("datafusion.execution.parquet.pushdown_filters", false);
        Arc::new(SessionContext::new_with_config(config).state())
    }

    fn wp33_row(document: &str, key: &str, value: &str) -> Value {
        document
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("valid WP33 JSONL row"))
            .find(|row| row[key] == value)
            .unwrap_or_else(|| panic!("missing WP33 row {key}={value}"))
    }

    fn wp33_object<'a>(value: &'a Value, context: &str) -> &'a Map<String, Value> {
        value
            .as_object()
            .unwrap_or_else(|| panic!("{context} must be an object"))
    }

    fn wp33_u64(value: &Value, context: &str) -> u64 {
        value
            .as_u64()
            .unwrap_or_else(|| panic!("{context} must be an unsigned integer"))
    }

    struct Claim012Fixture {
        _temporary: TempDir,
        root: Url,
        latest: DeltaTable,
    }

    fn claim_012_batch(rows: &Value) -> RecordBatch {
        let rows = rows
            .as_array()
            .expect("Claim 012 version input_rows must be an array");
        let mut entity_ids = Vec::with_capacity(rows.len());
        let mut values = Vec::with_capacity(rows.len());
        for row in rows {
            let fields = row
                .as_array()
                .expect("Claim 012 input row must be an array");
            assert_eq!(fields.len(), 2, "Claim 012 input row arity");
            entity_ids.push(
                fields[0]
                    .as_str()
                    .expect("Claim 012 entity_id must be a string"),
            );
            values.push(
                fields[1]
                    .as_str()
                    .expect("Claim 012 value must be a string"),
            );
        }
        RecordBatch::try_new(
            Arc::new(Schema::new(vec![
                Field::new("entity_id", DataType::Utf8, false),
                Field::new("value", DataType::Utf8, false),
            ])),
            vec![
                Arc::new(StringArray::from(entity_ids)),
                Arc::new(StringArray::from(values)),
            ],
        )
        .expect("Claim 012 exact Arrow input batch")
    }

    async fn claim_012_fixture(claim: &Value) -> Claim012Fixture {
        let history = &claim["complete_input_universe"]["inputs"]["delta_table_history"];
        let temporary = TempDir::new().expect("temporary Claim 012 Delta root");
        let table_path = temporary.path().join("fact-entity");
        fs::create_dir_all(&table_path).expect("create Claim 012 Delta root");
        let root = Url::from_directory_path(&table_path).expect("Claim 012 file URL");
        let schema = claim_012_batch(&json!([])).schema();
        let kernel: deltalake::kernel::StructType = schema
            .as_ref()
            .try_into_kernel()
            .expect("Claim 012 Arrow schema converts to Delta");
        let version_zero = CreateBuilder::new()
            .with_location(root.to_string())
            .with_table_name(history["table"].as_str().expect("Claim 012 table identity"))
            .with_save_mode(SaveMode::ErrorIfExists)
            .with_columns(kernel.fields().cloned())
            .with_configuration_property(TableProperty::EnableChangeDataFeed, Some("true"))
            .await
            .expect("create Claim 012 CDF table");
        assert_eq!(version_zero.version(), Some(0));

        let mut latest = version_zero;
        for version in history["versions"]
            .as_array()
            .expect("Claim 012 versions")
            .iter()
            .skip(1)
        {
            latest = latest
                .write(vec![claim_012_batch(&version["input_rows"])])
                .await
                .expect("append Claim 012 Delta version");
            assert_eq!(
                latest.version(),
                Some(wp33_u64(&version["version"], "Claim 012 version"))
            );
        }
        Claim012Fixture {
            _temporary: temporary,
            root,
            latest,
        }
    }

    async fn claim_012_observation(fixture: &Claim012Fixture, selected_version: u64) -> Value {
        let exact = DeltaTableBuilder::from_url(fixture.root.clone())
            .expect("construct Claim 012 exact builder")
            .with_version(selected_version)
            .load()
            .await
            .expect("load Claim 012 exact version");
        let exact_session = epoch_session();
        let provider = provider_from_exact_log_store(
            &ExactDeltaPin::new(&fixture.root, selected_version).expect("Claim 012 exact pin"),
            exact.log_store(),
            Arc::clone(&exact_session),
        )
        .await
        .expect("construct Claim 012 exact Delta provider");
        let context = SessionContext::new_with_state(exact_session.as_ref().clone());
        context
            .register_table("claim_012_exact", provider)
            .expect("register Claim 012 exact provider");
        let batches = context
            .table("claim_012_exact")
            .await
            .expect("resolve Claim 012 exact provider")
            .collect()
            .await
            .expect("execute Claim 012 exact provider");
        let mut snapshot_rows = Vec::new();
        for batch in &batches {
            let ids = batch
                .column_by_name("entity_id")
                .expect("Claim 012 entity_id column")
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("Claim 012 entity_id strings");
            let values = batch
                .column_by_name("value")
                .expect("Claim 012 value column")
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("Claim 012 value strings");
            for row in 0..batch.num_rows() {
                snapshot_rows.push(json!([ids.value(row), values.value(row)]));
            }
        }
        snapshot_rows.sort_by(|left, right| {
            left[0]
                .as_str()
                .expect("Claim 012 entity identity")
                .cmp(right[0].as_str().expect("Claim 012 entity identity"))
        });

        let mut refreshed = exact.clone();
        refreshed
            .update_state()
            .await
            .expect("refresh Claim 012 table to latest state");
        let latest_version = refreshed.version().expect("Claim 012 latest version");
        let snapshot = refreshed.snapshot().expect("Claim 012 latest snapshot");
        let protocol = snapshot.protocol();
        let mut reader_features = protocol
            .reader_features()
            .unwrap_or_default()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        reader_features.sort();
        let mut writer_features = protocol
            .writer_features()
            .unwrap_or_default()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        writer_features.sort();
        let cdf_property = snapshot
            .metadata()
            .configuration()
            .get("delta.enableChangeDataFeed")
            .cloned()
            .expect("Claim 012 CDF property");

        let checkpoint = DurableDeltaCdfCheckpoint::try_new(
            "wp38-claim-012",
            0,
            ExactDeltaPin::new(&fixture.root, 2).expect("Claim 012 CDF checkpoint"),
            DeltaCdfDownstreamCommit::External([0x12; 32]),
        )
        .expect("Claim 012 durable CDF checkpoint");
        let window = checkpoint
            .next_window(0, latest_version)
            .expect("select Claim 012 exact CDF window")
            .expect("Claim 012 CDF version remains");
        let cdf = prepare_exact_delta_cdf(
            &checkpoint,
            &fixture.latest,
            &ExactDeltaPin::new(&fixture.root, latest_version)
                .expect("Claim 012 CDF source-through pin"),
            window,
            epoch_session(),
        )
        .await
        .expect("prepare Claim 012 exact CDF");
        let ExactDeltaCdfPreparation::PhysicalPlan(cdf) = cdf else {
            panic!("Claim 012 retained CDF range must execute")
        };
        let executed = cdf.execute().await.expect("execute Claim 012 exact CDF");
        let mut cdf_rows = Vec::new();
        for batch in executed.batches() {
            let ids = batch
                .column_by_name("entity_id")
                .expect("Claim 012 CDF entity_id")
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("Claim 012 CDF entity_id strings");
            let values = batch
                .column_by_name("value")
                .expect("Claim 012 CDF value")
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("Claim 012 CDF value strings");
            let changes = batch
                .column_by_name(CHANGE_TYPE_COL)
                .expect("Claim 012 CDF change type")
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("Claim 012 CDF change strings");
            let versions = batch
                .column_by_name(COMMIT_VERSION_COL)
                .expect("Claim 012 CDF commit version")
                .as_any()
                .downcast_ref::<UInt64Array>()
                .expect("Claim 012 CDF commit versions");
            for row in 0..batch.num_rows() {
                cdf_rows.push(json!([
                    ids.value(row),
                    values.value(row),
                    changes.value(row),
                    versions.value(row)
                ]));
            }
        }
        cdf_rows.sort_by_key(|row| {
            (
                row[3].as_u64().expect("Claim 012 CDF version"),
                row[2]
                    .as_str()
                    .expect("Claim 012 CDF change type")
                    .to_owned(),
                row[0].as_str().expect("Claim 012 CDF entity").to_owned(),
            )
        });

        json!({
            "selected_version": selected_version,
            "latest_version": latest_version,
            "protocol": {
                "min_reader_version": protocol.min_reader_version(),
                "min_writer_version": protocol.min_writer_version(),
                "reader_features": reader_features,
                "writer_features": writer_features,
                "table_properties": {"delta.enableChangeDataFeed": cdf_property},
            },
            "snapshot_rows": snapshot_rows,
            "cdf_window": {
                "starting_version": window.starting_version(),
                "ending_version": window.ending_version(),
                "inclusive": true,
            },
            "cdf_columns": ["entity_id", "value", "_change_type", "_commit_version"],
            "cdf_rows": cdf_rows,
        })
    }

    fn assert_session_config_preserved(provider: &Arc<dyn TableProvider>) {
        assert_eq!(
            provider
                .schema()
                .field_with_name("label")
                .unwrap()
                .data_type(),
            &DataType::Utf8,
            "schema_force_view_types=false must come from the epoch SessionState"
        );
    }

    async fn collect_labels(
        provider: Arc<dyn TableProvider>,
        session: Arc<SessionState>,
    ) -> Vec<String> {
        let context = SessionContext::new_with_state(session.as_ref().clone());
        context
            .register_table("exact_delta", provider)
            .expect("register exact Delta provider");
        let batches = context
            .table("exact_delta")
            .await
            .expect("resolve exact Delta provider")
            .collect()
            .await
            .expect("execute exact Delta provider");
        batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("label column")
                    .iter()
                    .map(|value| value.expect("non-null fixture label").to_owned())
            })
            .collect()
    }

    #[tokio::test]
    async fn snapshot_recipe_reconstructs_the_exact_pin_without_a_version_selector() {
        let fixture = fixture().await;
        let validated =
            ValidatedDeltaSnapshot::try_from_loaded_table(fixture.table.clone(), &fixture.pin)
                .expect("validate loaded fixture snapshot");
        assert_eq!(validated.observed_identity().version(), 0);
        assert_eq!(
            validated.observed_identity().canonical_root(),
            fixture.pin.canonical_root()
        );

        let provider = provider_from_validated_snapshot(&fixture.pin, validated, epoch_session())
            .await
            .expect("build snapshot-authoritative provider");
        assert_session_config_preserved(&provider);
    }

    #[tokio::test]
    async fn log_store_recipe_reconstructs_the_exact_pin_without_a_supplied_snapshot() {
        let fixture = fixture().await;
        let provider =
            provider_from_exact_log_store(&fixture.pin, fixture.table.log_store(), epoch_session())
                .await
                .expect("build exact log-store provider");
        assert_session_config_preserved(&provider);
    }

    #[tokio::test]
    async fn exact_provider_read_retains_full_stats_and_marks_missing_values_unknown() {
        let fixture = fixture().await;
        let version_one = append_label(&fixture.table, "statistics-row").await;
        let pin = ExactDeltaPin::new(&fixture.root, 1).expect("version-one pin");
        let read =
            provider_read_from_exact_log_store(&pin, version_one.log_store(), epoch_session())
                .await
                .expect("exact provider and statistics evidence");

        assert_eq!(read.statistics().add_actions().num_rows(), 1);
        assert_eq!(
            read.statistics()
                .field("size_bytes")
                .expect("file-size inspection")
                .availability(),
            ExactDeltaStatisticAvailability::KnownForAllFiles { file_count: 1 }
        );
        assert_eq!(
            read.statistics()
                .field("partition.label")
                .expect("partition-stat inspection")
                .availability(),
            ExactDeltaStatisticAvailability::UnknownForFiles {
                file_count: 1,
                unknown_file_count: 1,
            }
        );
        assert!(!read.statistics().optimizer_statistics_reported());
        assert!(matches!(
            read.statistics().optimizer_statistics().num_rows,
            datafusion::common::stats::Precision::Absent
        ));
        assert_eq!(
            read.statistics()
                .optimizer_statistics()
                .column_statistics
                .len(),
            read.provider().schema().fields().len()
        );
    }

    #[tokio::test]
    async fn exact_provider_rejects_a_snapshot_loaded_with_statistics_disabled() {
        let fixture = fixture().await;
        let skipped = DeltaTableBuilder::from_url(fixture.root.clone())
            .expect("construct skip-stats table builder")
            .with_skip_stats(true)
            .with_version(0)
            .load()
            .await
            .expect("load exact fixture without statistics");
        let validated = ValidatedDeltaSnapshot::try_from_loaded_table(skipped, &fixture.pin)
            .expect("validate exact skip-stats identity");
        let error = provider_read_from_validated_snapshot(&fixture.pin, validated, epoch_session())
            .await
            .expect_err("query-serving provider must reject skip_stats=true");
        assert!(matches!(
            error,
            ExactDeltaProviderError::StatisticsParsingDisabled
        ));
    }

    #[tokio::test]
    async fn snapshot_recipe_rejects_a_different_exact_version() {
        let fixture = fixture().await;
        let validated =
            ValidatedDeltaSnapshot::try_from_loaded_table(fixture.table.clone(), &fixture.pin)
                .expect("validate loaded fixture snapshot");
        let wrong_pin = ExactDeltaPin::new(&fixture.root, 1).expect("wrong exact pin");

        let error = provider_from_validated_snapshot(&wrong_pin, validated, epoch_session())
            .await
            .expect_err("different version must be rejected");
        assert!(matches!(
            error,
            ExactDeltaProviderError::IdentityMismatch {
                expected_version: 1,
                observed_version: 0,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn log_store_recipe_rejects_a_different_root_before_provider_construction() {
        let fixture = fixture().await;
        let other = TempDir::new().expect("second table root");
        let wrong_root = Url::from_directory_path(other.path()).expect("second root URL");
        let wrong_pin = ExactDeltaPin::new(&wrong_root, 0).expect("wrong-root pin");

        let error =
            provider_from_exact_log_store(&wrong_pin, fixture.table.log_store(), epoch_session())
                .await
                .expect_err("different root must be rejected");
        assert!(matches!(
            error,
            ExactDeltaProviderError::IdentityMismatch {
                expected_version: 0,
                observed_version: 0,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn log_store_recipe_rejects_an_unavailable_exact_version() {
        let fixture = fixture().await;
        let unavailable = ExactDeltaPin::new(&fixture.root, 1).expect("unavailable-version pin");

        provider_from_exact_log_store(&unavailable, fixture.table.log_store(), epoch_session())
            .await
            .expect_err("missing version must never fall back to latest");
    }

    #[tokio::test]
    async fn exact_log_store_recipe_remains_at_the_pin_after_the_table_head_advances() {
        let fixture = fixture().await;
        let batch = RecordBatch::try_new(
            Arc::new(Schema::new(vec![Field::new("label", DataType::Utf8, true)])),
            vec![Arc::new(StringArray::from(vec![Some("new-head-row")]))],
        )
        .expect("new-head fixture batch");
        let advanced = fixture
            .table
            .clone()
            .write(vec![batch])
            .await
            .expect("advance fixture table head");
        assert_eq!(advanced.version(), Some(1));

        let pinned_session = epoch_session();
        let pinned = provider_from_exact_log_store(
            &fixture.pin,
            advanced.log_store(),
            Arc::clone(&pinned_session),
        )
        .await
        .expect("reconstruct version zero after head advancement");
        assert!(collect_labels(pinned, pinned_session).await.is_empty());

        let latest_pin = ExactDeltaPin::new(&fixture.root, 1).expect("exact version-one pin");
        let latest_session = epoch_session();
        let latest = provider_from_exact_log_store(
            &latest_pin,
            advanced.log_store(),
            Arc::clone(&latest_session),
        )
        .await
        .expect("reconstruct exact version one");
        assert_eq!(
            collect_labels(latest, latest_session).await,
            ["new-head-row"]
        );
    }

    #[tokio::test]
    async fn exact_cdf_range_has_an_inclusive_end_and_excludes_a_newer_head() {
        let fixture = cdf_fixture().await;
        let version_one = append_label(&fixture.version_zero, "version-one").await;
        let version_two = append_label(&version_one, "version-two").await;
        assert_eq!(version_two.version(), Some(2));

        let checkpoint = cdf_checkpoint(&fixture.root, 0);
        let window = checkpoint
            .next_window(0, 1)
            .expect("valid explicit CDF range")
            .expect("version one requires transport");
        let source_through = ExactDeltaPin::new(&fixture.root, 1).expect("source-through pin");
        let preparation = prepare_exact_delta_cdf(
            &checkpoint,
            &version_one,
            &source_through,
            window,
            epoch_session(),
        )
        .await
        .expect("prepare exact CDF plan");
        let ExactDeltaCdfPreparation::PhysicalPlan(prepared) = preparation else {
            panic!("retained inclusive range must produce a physical plan: {preparation:?}");
        };
        assert_eq!(prepared.window().starting_version(), 1);
        assert_eq!(prepared.window().ending_version(), 1);
        let executed = prepared.execute().await.expect("execute exact CDF range");
        let (labels, versions) = cdf_values(&executed);
        assert_eq!(labels, ["version-one"]);
        assert_eq!(versions, [1]);
    }

    #[tokio::test]
    async fn exact_cdf_missing_inclusive_end_requires_exact_snapshot_fallback() {
        let fixture = cdf_fixture().await;
        let version_one = append_label(&fixture.version_zero, "version-one").await;
        let version_two = append_label(&version_one, "version-two").await;
        let checkpoint = cdf_checkpoint(&fixture.root, 0);
        let window = checkpoint
            .next_window(0, 2)
            .expect("valid explicit CDF range")
            .expect("versions require transport");
        let source_through = ExactDeltaPin::new(&fixture.root, 2).expect("source-through pin");
        remove_commit_entry(&fixture.root, 2);

        let preparation = prepare_exact_delta_cdf(
            &checkpoint,
            &version_two,
            &source_through,
            window,
            epoch_session(),
        )
        .await
        .expect("missing end is a typed fallback, not a scan error");
        let ExactDeltaCdfPreparation::ExactSnapshotFallback(fallback) = preparation else {
            panic!("missing inclusive end must not produce a CDF plan: {preparation:?}");
        };
        assert_eq!(fallback.reconstruct_at(), &source_through);
        assert_eq!(fallback.requested_window(), window);
        assert_eq!(
            fallback.reason(),
            ExactDeltaCdfFallbackReason::EndingVersionNotRetained { version: 2 }
        );
        assert_eq!(checkpoint.consumed_through().version(), 0);
    }

    #[tokio::test]
    async fn exact_cdf_interior_gap_requires_exact_snapshot_fallback() {
        let fixture = cdf_fixture().await;
        let version_one = append_label(&fixture.version_zero, "version-one").await;
        let version_two = append_label(&version_one, "version-two").await;
        let checkpoint = cdf_checkpoint(&fixture.root, 0);
        let window = checkpoint
            .next_window(0, 2)
            .expect("valid explicit CDF range")
            .expect("versions require transport");
        let source_through = ExactDeltaPin::new(&fixture.root, 2).expect("source-through pin");
        remove_commit_entry(&fixture.root, 1);

        let preparation = prepare_exact_delta_cdf(
            &checkpoint,
            &version_two,
            &source_through,
            window,
            epoch_session(),
        )
        .await
        .expect("interior gap is a typed fallback, not a shortened scan");
        let ExactDeltaCdfPreparation::ExactSnapshotFallback(fallback) = preparation else {
            panic!("interior gap must not produce a CDF plan: {preparation:?}");
        };
        assert_eq!(fallback.reconstruct_at(), &source_through);
        assert_eq!(
            fallback.reason(),
            ExactDeltaCdfFallbackReason::RequiredVersionNotRetained { version: 1 }
        );
        assert_eq!(checkpoint.consumed_through().version(), 0);
    }

    #[tokio::test]
    async fn zero_row_cdf_range_advances_only_after_downstream_success() {
        let fixture = cdf_fixture().await;
        let mut properties = HashMap::new();
        properties.insert(
            TableProperty::AppendOnly.as_ref().to_owned(),
            "true".to_owned(),
        );
        let metadata_only = fixture
            .version_zero
            .clone()
            .set_tbl_properties()
            .with_properties(properties)
            .await
            .expect("create metadata-only CDF interval");
        assert_eq!(metadata_only.version(), Some(1));

        let checkpoint = cdf_checkpoint(&fixture.root, 0);
        let window = checkpoint
            .next_window(0, 1)
            .expect("valid explicit CDF range")
            .expect("metadata version requires transport");
        let source_through = ExactDeltaPin::new(&fixture.root, 1).expect("source-through pin");
        let preparation = prepare_exact_delta_cdf(
            &checkpoint,
            &metadata_only,
            &source_through,
            window,
            epoch_session(),
        )
        .await
        .expect("prepare zero-row CDF plan");
        let ExactDeltaCdfPreparation::PhysicalPlan(prepared) = preparation else {
            panic!("retained metadata range must produce a physical plan: {preparation:?}");
        };
        let executed = prepared
            .execute()
            .await
            .expect("execute zero-row CDF range");
        assert_eq!(executed.row_count(), 0);
        assert_eq!(checkpoint.consumed_through().version(), 0);

        let success = executed
            .finish_downstream(Ok(DeltaCdfDownstreamCommit::External([2; 32])))
            .expect("durable zero-row acknowledgement issues a token");
        let advanced = checkpoint
            .advance_after_downstream_success(success)
            .expect("zero-row successful range advances checkpoint");
        assert_eq!(advanced.consumed_through().version(), 1);
    }

    #[tokio::test]
    async fn downstream_failure_issues_no_token_and_leaves_checkpoint_unchanged() {
        let fixture = cdf_fixture().await;
        let version_one = append_label(&fixture.version_zero, "version-one").await;
        let checkpoint = cdf_checkpoint(&fixture.root, 0);
        let window = checkpoint
            .next_window(0, 1)
            .expect("valid explicit CDF range")
            .expect("version one requires transport");
        let source_through = ExactDeltaPin::new(&fixture.root, 1).expect("source-through pin");
        let preparation = prepare_exact_delta_cdf(
            &checkpoint,
            &version_one,
            &source_through,
            window,
            epoch_session(),
        )
        .await
        .expect("prepare exact CDF plan");
        let ExactDeltaCdfPreparation::PhysicalPlan(prepared) = preparation else {
            panic!("retained range must produce a physical plan: {preparation:?}");
        };
        let executed = prepared.execute().await.expect("execute exact CDF range");
        let failure = DeltaCdfDownstreamFailure::new("injected downstream failure");
        let downstream_result = executed.finish_downstream(Err(failure.clone()));
        assert!(matches!(
            downstream_result,
            Err(DeltaCdfDownstreamCompletionError::Downstream(observed)) if observed == failure
        ));
        assert_eq!(checkpoint.consumed_through().version(), 0);
        assert_eq!(
            checkpoint.downstream_commit(),
            &DeltaCdfDownstreamCommit::External([1; 32])
        );
    }

    #[tokio::test]
    async fn reconstructed_checkpoint_restarts_at_the_next_exact_version() {
        let fixture = cdf_fixture().await;
        let version_one = append_label(&fixture.version_zero, "version-one").await;
        let checkpoint = cdf_checkpoint(&fixture.root, 0);
        let first_window = checkpoint
            .next_window(0, 1)
            .expect("valid first CDF range")
            .expect("version one requires transport");
        let version_one_pin = ExactDeltaPin::new(&fixture.root, 1).expect("version-one pin");
        let first_preparation = prepare_exact_delta_cdf(
            &checkpoint,
            &version_one,
            &version_one_pin,
            first_window,
            epoch_session(),
        )
        .await
        .expect("prepare first CDF range");
        let ExactDeltaCdfPreparation::PhysicalPlan(first_prepared) = first_preparation else {
            panic!("retained first range must produce a plan: {first_preparation:?}");
        };
        let first_success = first_prepared
            .execute()
            .await
            .expect("execute first CDF range")
            .finish_downstream(Ok(DeltaCdfDownstreamCommit::External([3; 32])))
            .expect("durably apply first CDF range");
        let advanced = checkpoint
            .advance_after_downstream_success(first_success)
            .expect("advance first CDF range");

        let restarted = DurableDeltaCdfCheckpoint::try_new(
            advanced.consumer_id().to_owned(),
            advanced.cdf_activation_version(),
            advanced.consumed_through().clone(),
            advanced.downstream_commit().clone(),
        )
        .expect("reconstruct durable checkpoint after process loss");
        let version_two = append_label(&version_one, "version-two").await;
        let second_window = restarted
            .next_window(0, 2)
            .expect("valid post-restart CDF range")
            .expect("version two requires transport");
        assert_eq!(second_window.starting_version(), 2);
        assert_eq!(second_window.ending_version(), 2);

        let version_two_pin = ExactDeltaPin::new(&fixture.root, 2).expect("version-two pin");
        let second_preparation = prepare_exact_delta_cdf(
            &restarted,
            &version_two,
            &version_two_pin,
            second_window,
            epoch_session(),
        )
        .await
        .expect("prepare post-restart CDF range");
        let ExactDeltaCdfPreparation::PhysicalPlan(second_prepared) = second_preparation else {
            panic!("retained post-restart range must produce a plan: {second_preparation:?}");
        };
        let second_executed = second_prepared
            .execute()
            .await
            .expect("execute post-restart CDF range");
        let (labels, versions) = cdf_values(&second_executed);
        assert_eq!(labels, ["version-two"]);
        assert_eq!(versions, [2]);
    }

    #[test]
    fn durable_cdf_checkpoint_selects_contiguous_versions_and_fails_on_retention_gaps() {
        let root = TempDir::new().expect("CDF source root");
        let root_url = Url::from_directory_path(root.path()).expect("CDF source URL");
        let checkpoint = DurableDeltaCdfCheckpoint::try_new(
            "derived-current-view",
            2,
            ExactDeltaPin::new(&root_url, 4).expect("source checkpoint pin"),
            DeltaCdfDownstreamCommit::External([7; 32]),
        )
        .expect("durable checkpoint");

        let window = checkpoint
            .next_window(2, 7)
            .expect("retained source range")
            .expect("new CDF range");
        assert_eq!(window.starting_version(), 5);
        assert_eq!(window.ending_version(), 7);
        let downstream_commit = DeltaCdfDownstreamCommit::Delta(
            ExactDeltaPin::new(&root_url, 9).expect("downstream commit pin"),
        );
        let advanced = checkpoint
            .advance_after_downstream_success(DeltaCdfDownstreamSuccess {
                consumer_id: Arc::clone(&checkpoint.consumer_id),
                checkpoint_before: checkpoint.consumed_through.clone(),
                source_through: ExactDeltaPin::new(&root_url, 7).expect("source through pin"),
                window,
                downstream_commit,
            })
            .expect("advance after downstream commit");
        assert_eq!(advanced.consumed_through().version(), 7);
        assert_eq!(advanced.next_window(2, 7), Ok(None));

        assert_eq!(
            checkpoint.next_window(6, 8),
            Err(DeltaCdfCheckpointError::RetentionGap {
                required_starting_version: 5,
                earliest_available_version: 6,
            })
        );
        assert_eq!(
            checkpoint.advance_after_downstream_success(DeltaCdfDownstreamSuccess {
                consumer_id: Arc::clone(&checkpoint.consumer_id),
                checkpoint_before: checkpoint.consumed_through.clone(),
                source_through: ExactDeltaPin::new(&root_url, 7).expect("source through pin"),
                window: DeltaCdfReadWindow {
                    starting_version: 6,
                    ending_version: 7,
                },
                downstream_commit: DeltaCdfDownstreamCommit::External([8; 32]),
            }),
            Err(DeltaCdfCheckpointError::NonContiguousAdvance {
                expected_starting_version: 5,
                actual_starting_version: 6,
                ending_version: 7,
            })
        );
    }

    #[test]
    fn malformed_cdf_checkpoints_fail_closed() {
        let root = TempDir::new().expect("CDF source root");
        let root_url = Url::from_directory_path(root.path()).expect("CDF source URL");
        let pin = ExactDeltaPin::new(&root_url, 3).expect("source checkpoint pin");
        assert_eq!(
            DurableDeltaCdfCheckpoint::try_new(
                " ",
                2,
                pin.clone(),
                DeltaCdfDownstreamCommit::External([1; 32]),
            ),
            Err(DeltaCdfCheckpointError::EmptyConsumerIdentity)
        );
        assert_eq!(
            DurableDeltaCdfCheckpoint::try_new(
                "consumer",
                2,
                pin.clone(),
                DeltaCdfDownstreamCommit::External([0; 32]),
            ),
            Err(DeltaCdfCheckpointError::ZeroDownstreamCommitIdentity)
        );
        assert_eq!(
            DurableDeltaCdfCheckpoint::try_new(
                "consumer",
                4,
                pin,
                DeltaCdfDownstreamCommit::External([1; 32]),
            ),
            Err(DeltaCdfCheckpointError::BeforeCdfActivation {
                activation_version: 4,
                consumed_version: 3,
            })
        );
    }

    fn retention_claim(
        kind: DeltaRetentionAuthorityKind,
        authority_byte: u8,
        protected: DeltaRetainedResource,
        expires_at: Option<u64>,
    ) -> DeltaRetentionClaim {
        DeltaRetentionClaim::try_new(kind, [authority_byte; 32], protected, expires_at)
            .expect("valid retention claim")
    }

    #[tokio::test]
    async fn wp38_claim_012_positive_executes_frozen_exact_delta_and_cdf_semantics() {
        let claim = wp33_row(WP33_EXPECTATIONS, "claim_id", "RFV3-CLAIM-012");
        let fixture = claim_012_fixture(&claim).await;
        let selected_version = wp33_u64(
            &claim["complete_input_universe"]["inputs"]["selected_version_vector"]["table_versions"]
                ["fact.entity"],
            "Claim 012 selected version",
        );

        let observed = claim_012_observation(&fixture, selected_version).await;
        assert_eq!(observed, claim["decoded_expectation"]["rows"][0][1]);
    }

    #[tokio::test]
    async fn wp38_claim_012_causal_exact_version_changes_the_decoded_snapshot() {
        let claim = wp33_row(WP33_EXPECTATIONS, "claim_id", "RFV3-CLAIM-012");
        let causal = wp33_row(WP33_FIXTURES, "fixture_id", "RFV3-FIX-012-C");
        assert_eq!(causal["kind"], "causal");
        assert_eq!(causal["expected_terminal"], "changed");
        let mutation = wp33_object(&causal["mutation"], "Claim 012 causal mutation");
        assert_eq!(mutation["input_role"], "selected_version_vector");
        assert_eq!(mutation["json_pointer"], "/table_versions/fact.entity");
        let before = wp33_u64(&mutation["before"], "Claim 012 causal before version");
        let after = wp33_u64(&mutation["after"], "Claim 012 causal after version");
        let fixture = claim_012_fixture(&claim).await;

        let baseline = claim_012_observation(&fixture, before).await;
        let observed = claim_012_observation(&fixture, after).await;
        assert_ne!(observed, baseline, "exact-version mutation must be causal");
        assert_eq!(observed, causal["expected_decoded"]);
    }

    #[tokio::test]
    async fn wp38_claim_012_negative_rejects_frozen_unsupported_writer_feature() {
        let claim = wp33_row(WP33_EXPECTATIONS, "claim_id", "RFV3-CLAIM-012");
        let negative = wp33_row(WP33_FIXTURES, "fixture_id", "RFV3-FIX-012-N");
        assert_eq!(negative["kind"], "negative");
        assert_eq!(negative["expected_terminal"], "reject");
        let fixture = claim_012_fixture(&claim).await;
        let mutation = wp33_object(&negative["mutation"], "Claim 012 negative mutation");
        assert_eq!(mutation["input_role"], "delta_table_history");
        assert_eq!(mutation["json_pointer"], "/versions/3/protocol");
        let protocol = wp33_object(&mutation["after"], "Claim 012 mutated protocol");
        let commit_path = fixture
            .root
            .to_file_path()
            .expect("Claim 012 local Delta root")
            .join("_delta_log/00000000000000000003.json");
        let mut commit = fs::read_to_string(&commit_path).expect("read Claim 012 commit 3");
        let mut physical_protocol = Map::from_iter([
            (
                "minReaderVersion".to_owned(),
                protocol["min_reader_version"].clone(),
            ),
            (
                "minWriterVersion".to_owned(),
                protocol["min_writer_version"].clone(),
            ),
        ]);
        if protocol["min_reader_version"] == 3 {
            physical_protocol.insert(
                "readerFeatures".to_owned(),
                protocol["reader_features"].clone(),
            );
        }
        if protocol["min_writer_version"] == 7 {
            physical_protocol.insert(
                "writerFeatures".to_owned(),
                protocol["writer_features"].clone(),
            );
        }
        commit.push_str(
            &serde_json::to_string(&json!({"protocol": physical_protocol}))
                .expect("encode Claim 012 protocol action"),
        );
        commit.push('\n');
        fs::write(&commit_path, commit).expect("write Claim 012 protocol mutation");

        let exact = DeltaTableBuilder::from_url(fixture.root.clone())
            .expect("construct Claim 012 negative exact builder")
            .with_version(3)
            .load()
            .await
            .expect("load Claim 012 writer-feature snapshot");
        let error = PROTOCOL
            .can_write_to(
                exact
                    .snapshot()
                    .expect("Claim 012 negative exact snapshot")
                    .snapshot(),
            )
            .expect_err("rowTracking must be rejected by the pinned delta-rs writer");
        let TransactionError::UnsupportedTableFeatures(features) = error else {
            panic!("unexpected Claim 012 writer compatibility error: {error:?}")
        };
        let feature = features
            .iter()
            .map(ToString::to_string)
            .find(|feature| feature == "rowTracking")
            .expect("Claim 012 rowTracking rejection");
        assert_eq!(
            json!({
                "error": "DELTA_WRITER_FEATURE_UNSUPPORTED",
                "feature": feature,
                "table_version": exact.version().expect("Claim 012 negative version"),
            }),
            negative["expected_decoded"]
        );
    }

    #[test]
    fn retention_closure_unions_every_active_authority_and_excludes_expired_leases() {
        let root = TempDir::new().expect("retention table root");
        let root_url = Url::from_directory_path(root.path()).expect("retention root URL");
        let v1 = ExactDeltaPin::new(&root_url, 1).expect("version one pin");
        let v3 = ExactDeltaPin::new(&root_url, 3).expect("version three pin");
        let claims = vec![
            retention_claim(
                DeltaRetentionAuthorityKind::ActiveEpoch,
                1,
                DeltaRetainedResource::DeltaVersion(v3),
                None,
            ),
            retention_claim(
                DeltaRetentionAuthorityKind::QueryLease,
                2,
                DeltaRetainedResource::DeltaVersion(v1),
                Some(101),
            ),
            retention_claim(
                DeltaRetentionAuthorityKind::ResultLease,
                3,
                DeltaRetainedResource::QueryResult([31; 32]),
                Some(100),
            ),
            retention_claim(
                DeltaRetentionAuthorityKind::RollbackLease,
                4,
                DeltaRetainedResource::RollbackPoint([41; 32]),
                Some(120),
            ),
            retention_claim(
                DeltaRetentionAuthorityKind::ExpectationLease,
                5,
                DeltaRetainedResource::Expectation([51; 32]),
                Some(120),
            ),
            retention_claim(
                DeltaRetentionAuthorityKind::ProgramReleaseLease,
                6,
                DeltaRetainedResource::ProgramRelease([61; 32]),
                Some(120),
            ),
            retention_claim(
                DeltaRetentionAuthorityKind::QueryLease,
                7,
                DeltaRetainedResource::ImmutableSegment([71; 32]),
                Some(120),
            ),
        ];
        let closure = DeltaRetentionClosure::try_new(100, DeltaRetentionAuthorityKind::ALL, claims)
            .expect("retention closure");

        assert_eq!(closure.keep_versions_for(&root_url).unwrap(), [1, 3]);
        assert_eq!(
            closure.observation(),
            DeltaRetentionClosureObservation {
                authority_sources_observed: 9,
                supplied_claims: 7,
                active_claims: 6,
                expired_claims: 1,
                protected_resources: 6,
                protected_delta_versions: 2,
            }
        );
        assert!(
            closure
                .protected_resources()
                .iter()
                .any(|resource| { resource == &DeltaRetainedResource::ImmutableSegment([71; 32]) })
        );
        assert!(
            !closure
                .protected_resources()
                .iter()
                .any(|resource| { resource == &DeltaRetainedResource::QueryResult([31; 32]) })
        );
    }

    #[test]
    fn vacuum_preflight_requires_dry_run_and_the_exact_derived_keep_versions() {
        let root = TempDir::new().expect("vacuum table root");
        let root_url = Url::from_directory_path(root.path()).expect("vacuum root URL");
        let closure = DeltaRetentionClosure::try_new(
            10,
            DeltaRetentionAuthorityKind::ALL,
            [
                retention_claim(
                    DeltaRetentionAuthorityKind::ActiveEpoch,
                    1,
                    DeltaRetainedResource::DeltaVersion(
                        ExactDeltaPin::new(&root_url, 2).expect("version two pin"),
                    ),
                    None,
                ),
                retention_claim(
                    DeltaRetentionAuthorityKind::QueryLease,
                    2,
                    DeltaRetainedResource::DeltaVersion(
                        ExactDeltaPin::new(&root_url, 5).expect("version five pin"),
                    ),
                    Some(20),
                ),
            ],
        )
        .expect("vacuum retention closure");

        closure
            .validate_vacuum_dry_run_contract(&root_url, true, &[2, 5])
            .expect("exact dry-run contract");
        assert_eq!(
            closure.validate_vacuum_dry_run_contract(&root_url, false, &[2, 5]),
            Err(DeltaRetentionClosureError::DestructiveFirstPass)
        );
        assert_eq!(
            closure.validate_vacuum_dry_run_contract(&root_url, true, &[2]),
            Err(DeltaRetentionClosureError::KeepVersionsMismatch {
                expected: vec![2, 5],
                actual: vec![2],
            })
        );
        assert_eq!(
            closure.validate_vacuum_dry_run_contract(&root_url, true, &[5, 2]),
            Err(DeltaRetentionClosureError::KeepVersionsMismatch {
                expected: vec![2, 5],
                actual: vec![5, 2],
            })
        );
    }

    #[test]
    fn retained_resource_deletion_and_malformed_claims_fail_closed() {
        let protected_segment = DeltaRetainedResource::ImmutableSegment([9; 32]);
        let closure = DeltaRetentionClosure::try_new(
            10,
            DeltaRetentionAuthorityKind::ALL,
            [retention_claim(
                DeltaRetentionAuthorityKind::QueryLease,
                1,
                protected_segment.clone(),
                Some(20),
            )],
        )
        .expect("segment retention closure");
        assert_eq!(
            closure.validate_resource_deletions(std::slice::from_ref(&protected_segment)),
            Err(DeltaRetentionClosureError::ProtectedResourceDeletion(
                Box::new(protected_segment)
            ))
        );
        closure
            .validate_resource_deletions(&[DeltaRetainedResource::ImmutableSegment([8; 32])])
            .expect("unretained segment can proceed to the next maintenance gate");

        assert_eq!(
            DeltaRetentionClaim::try_new(
                DeltaRetentionAuthorityKind::QueryLease,
                [1; 32],
                DeltaRetainedResource::QueryResult([2; 32]),
                None,
            ),
            Err(DeltaRetentionClosureError::InvalidExpiryShape {
                authority_kind: DeltaRetentionAuthorityKind::QueryLease,
                expires_at: None,
            })
        );
        assert_eq!(
            DeltaRetentionClaim::try_new(
                DeltaRetentionAuthorityKind::ActiveEpoch,
                [1; 32],
                DeltaRetainedResource::QueryResult([2; 32]),
                Some(20),
            ),
            Err(DeltaRetentionClosureError::InvalidExpiryShape {
                authority_kind: DeltaRetentionAuthorityKind::ActiveEpoch,
                expires_at: Some(20),
            })
        );
        assert_eq!(
            DeltaRetentionClosure::try_new(
                10,
                [DeltaRetentionAuthorityKind::ActiveEpoch],
                std::iter::empty(),
            ),
            Err(DeltaRetentionClosureError::IncompleteAuthorityCoverage {
                missing: vec![
                    DeltaRetentionAuthorityKind::InFlightPublication,
                    DeltaRetentionAuthorityKind::QueryLease,
                    DeltaRetentionAuthorityKind::ResultLease,
                    DeltaRetentionAuthorityKind::RollbackLease,
                    DeltaRetentionAuthorityKind::ExpectationLease,
                    DeltaRetentionAuthorityKind::ProgramReleaseLease,
                    DeltaRetentionAuthorityKind::CdfConsumerCheckpoint,
                    DeltaRetentionAuthorityKind::AuditHold,
                ],
            })
        );
    }

    #[test]
    fn publication_cdf_and_audit_holds_are_non_expiring_retention_authorities() {
        let root = TempDir::new().expect("retention table root");
        let root_url = Url::from_directory_path(root.path()).expect("retention root URL");
        let pin = ExactDeltaPin::new(&root_url, 5).expect("retained source pin");
        let claims = [
            retention_claim(
                DeltaRetentionAuthorityKind::InFlightPublication,
                1,
                DeltaRetainedResource::DeltaVersion(pin.clone()),
                None,
            ),
            retention_claim(
                DeltaRetentionAuthorityKind::CdfConsumerCheckpoint,
                2,
                DeltaRetainedResource::DeltaVersion(pin),
                None,
            ),
            retention_claim(
                DeltaRetentionAuthorityKind::AuditHold,
                3,
                DeltaRetainedResource::ImmutableSegment([4; 32]),
                None,
            ),
        ];
        let closure =
            DeltaRetentionClosure::try_new(u64::MAX, DeltaRetentionAuthorityKind::ALL, claims)
                .expect("non-expiring retention holds");
        assert_eq!(closure.keep_versions_for(&root_url).unwrap(), [5]);
        assert_eq!(closure.observation().active_claims, 3);
        assert_eq!(closure.observation().expired_claims, 0);

        assert_eq!(
            DeltaRetentionClaim::try_new(
                DeltaRetentionAuthorityKind::CdfConsumerCheckpoint,
                [2; 32],
                DeltaRetainedResource::QueryResult([3; 32]),
                Some(10),
            ),
            Err(DeltaRetentionClosureError::InvalidExpiryShape {
                authority_kind: DeltaRetentionAuthorityKind::CdfConsumerCheckpoint,
                expires_at: Some(10),
            })
        );
    }
}
