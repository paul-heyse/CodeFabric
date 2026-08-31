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

use datafusion::catalog::TableProvider;
use datafusion::execution::SessionState;
use deltalake::delta_datafusion::TableProviderBuilder;
use deltalake::kernel::{Action, CommitInfo, EagerSnapshot, Snapshot};
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
    /// Exact snapshot reconstruction or session object-store setup failed.
    #[error(transparent)]
    Delta(#[from] DeltaTableError),
    /// The pinned delta-rs provider builder rejected the exact recipe.
    #[error(transparent)]
    DataFusion(#[from] datafusion::error::DataFusionError),
}

/// Failure to read the `commitInfo` action for the version actually loaded in
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
}

/// Read the commit metadata for the exact version loaded in `table`.
///
/// `DeltaTable::history(Some(1))` is intentionally not used here: history is
/// a log-store query whose newest entry may be newer than an old snapshot
/// loaded with `with_version`. Exact-version proof reads the selected JSON log
/// entry directly and fails closed if it has been retained away.
pub(crate) async fn read_exact_commit_info(
    table: &DeltaTable,
) -> Result<CommitInfo, ExactDeltaCommitInfoError> {
    let version = table
        .version()
        .ok_or(ExactDeltaCommitInfoError::MissingLoadedVersion)?;
    let bytes = table
        .log_store()
        .read_commit_entry(version)
        .await
        .map_err(|source| ExactDeltaCommitInfoError::Read { version, source })?
        .ok_or(ExactDeltaCommitInfoError::MissingCommit { version })?;
    get_actions(version, &bytes)
        .map_err(|source| ExactDeltaCommitInfoError::Decode { version, source })?
        .into_iter()
        .find_map(|action| match action {
            Action::CommitInfo(info) => Some(info),
            _ => None,
        })
        .ok_or(ExactDeltaCommitInfoError::MissingCommitInfo { version })
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
    snapshot.revalidate(pin)?;
    snapshot
        .table
        .update_datafusion_session(epoch_session.as_ref())?;

    let provider = TableProviderBuilder::default()
        .with_eager_snapshot(snapshot.eager_snapshot)
        .with_session(epoch_session)
        .await?;
    Ok(provider)
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
    let observed_root = canonical_delta_root(log_store.config().location())?;
    if observed_root != pin.canonical_root {
        return Err(ExactDeltaProviderError::IdentityMismatch {
            expected_root: pin.canonical_root.to_string(),
            expected_version: pin.version,
            observed_root: observed_root.to_string(),
            observed_version: pin.version,
        });
    }

    // `Some(pin.version)` is load-bearing: `None` means latest in this API.
    let observed_snapshot = Snapshot::try_new(
        log_store.as_ref(),
        deltalake::DeltaTableConfig::default(),
        Some(pin.version),
    )
    .await?;
    let observed = ObservedDeltaIdentity {
        canonical_root: observed_root,
        version: observed_snapshot.version(),
    };
    pin.validate_observation(&observed)?;

    let provider = TableProviderBuilder::default()
        .with_log_store(log_store)
        .with_table_version(Some(pin.version))
        .with_session(epoch_session)
        .await?;
    Ok(provider)
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

    /// Advance after the selected window's downstream commit is durable.
    ///
    /// The next checkpoint must be contiguous and use the same canonical
    /// source root; gaps and caller-selected alternate ranges fail closed.
    pub fn advance(
        &self,
        window: DeltaCdfReadWindow,
        downstream_commit: DeltaCdfDownstreamCommit,
    ) -> Result<Self, DeltaCdfCheckpointError> {
        let expected_start = self
            .consumed_through
            .version()
            .checked_add(1)
            .ok_or(DeltaCdfCheckpointError::VersionOverflow)?;
        if window.starting_version != expected_start
            || window.ending_version < window.starting_version
        {
            return Err(DeltaCdfCheckpointError::NonContiguousAdvance {
                expected_starting_version: expected_start,
                actual_starting_version: window.starting_version,
                ending_version: window.ending_version,
            });
        }
        let consumed_through = ExactDeltaPin::new(
            self.consumed_through.canonical_root(),
            window.ending_version,
        )
        .map_err(|error| DeltaCdfCheckpointError::InvalidSourceRoot(error.to_string()))?;
        Self::try_new(
            Arc::clone(&self.consumer_id),
            self.cdf_activation_version,
            consumed_through,
            downstream_commit,
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
    CompilerReleaseLease,
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
        Self::CompilerReleaseLease,
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
                | Self::CompilerReleaseLease
        )
    }
}

/// One resource whose deletion can invalidate a retained fabric observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeltaRetainedResource {
    DeltaVersion(ExactDeltaPin),
    ImmutableSegment([u8; 32]),
    CompilerRelease([u8; 32]),
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
            Self::CompilerRelease(id) => format!("compiler:{}", lower_hex(id)),
            Self::Expectation(id) => format!("expectation:{}", lower_hex(id)),
            Self::QueryResult(id) => format!("result:{}", lower_hex(id)),
            Self::RollbackPoint(id) => format!("rollback:{}", lower_hex(id)),
        }
    }

    fn has_zero_identity(&self) -> bool {
        match self {
            Self::DeltaVersion(_) => false,
            Self::ImmutableSegment(id)
            | Self::CompilerRelease(id)
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
    use std::fs;

    use arrow_array::{RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::prelude::{SessionConfig, SessionContext};
    use deltalake::DeltaTableBuilder;
    use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
    use deltalake::operations::create::CreateBuilder;
    use deltalake::protocol::SaveMode;
    use tempfile::TempDir;

    use super::*;

    struct Fixture {
        _temporary: TempDir,
        root: Url,
        table: DeltaTable,
        pin: ExactDeltaPin,
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

    fn epoch_session() -> Arc<SessionState> {
        let config = SessionConfig::new()
            .set_bool(
                "datafusion.execution.parquet.schema_force_view_types",
                false,
            )
            .set_bool("datafusion.execution.parquet.pushdown_filters", false);
        Arc::new(SessionContext::new_with_config(config).state())
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
        let advanced = checkpoint
            .advance(
                window,
                DeltaCdfDownstreamCommit::Delta(
                    ExactDeltaPin::new(&root_url, 9).expect("downstream commit pin"),
                ),
            )
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
            checkpoint.advance(
                DeltaCdfReadWindow {
                    starting_version: 6,
                    ending_version: 7,
                },
                DeltaCdfDownstreamCommit::External([8; 32]),
            ),
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
                DeltaRetentionAuthorityKind::CompilerReleaseLease,
                6,
                DeltaRetainedResource::CompilerRelease([61; 32]),
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
                    DeltaRetentionAuthorityKind::CompilerReleaseLease,
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
