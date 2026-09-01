//! Exact-version Delta CDF consumption and replay coordination.
//!
//! A request names one canonical table root, the durable checkpoint version before the range,
//! one inclusive through-version, and one consumer. The coordinator never resolves a latest
//! version. It executes delta-rs CDF only when every requested log version and the declared schema
//! policy remain available, delegates the exact Arrow batches to an idempotent downstream, and
//! advances the SQLite transport checkpoint only after the downstream returns a durable commit.

use std::fmt;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use async_trait::async_trait;
use datafusion::execution::context::SessionState;
use deltalake::{DeltaTable, DeltaTableConfig, TableProperty};
use thiserror::Error;
use url::Url;

use super::delta_cdf_checkpoint_sqlite::{
    DeltaCdfCheckpointCompareAndSwap, DeltaCdfCheckpointKey, DeltaCdfCheckpointStoreError,
    SqliteDeltaCdfCheckpointStore,
};
use super::delta_exact::{
    DeltaCdfCheckpointError, DeltaCdfDownstreamCommit, DeltaCdfDownstreamCompletionError,
    DeltaCdfDownstreamFailure, DurableDeltaCdfCheckpoint, ExactDeltaCdfExecutionError,
    ExactDeltaCdfFallbackReason, ExactDeltaCdfPreparation, ExactDeltaCdfPreparationError,
    ExactDeltaPin, ExactDeltaProviderError, ValidatedDeltaSnapshot, prepare_exact_delta_cdf,
};

/// Schema evolution accepted by one exact CDF consumer request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeltaCdfSchemaEvolutionPolicy {
    /// Every exact table version in the requested interval must have the same Arrow schema.
    Exact,
    /// Existing fields must remain byte-for-byte equal and in order; only trailing nullable
    /// fields may be added.
    AdditiveNullable,
}

/// One explicit replay request. `from_version` is the durable checkpoint before the range;
/// transport covers `[from_version + 1, through_version]` inclusively.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDeltaCdfConsumptionRequest {
    key: DeltaCdfCheckpointKey,
    from: ExactDeltaPin,
    through: ExactDeltaPin,
    starting_version: u64,
    schema_policy: DeltaCdfSchemaEvolutionPolicy,
}

impl ExactDeltaCdfConsumptionRequest {
    /// Canonicalize an exact table/consumer identity and validate an advancing version range.
    ///
    /// # Errors
    ///
    /// Rejects an invalid table root or consumer and any range whose inclusive end is not after
    /// the durable checkpoint version.
    pub fn try_new(
        table_root: &Url,
        from_version: u64,
        through_version: u64,
        consumer_id: impl Into<Arc<str>>,
        schema_policy: DeltaCdfSchemaEvolutionPolicy,
    ) -> Result<Self, ExactDeltaCdfConsumptionRequestError> {
        if through_version <= from_version {
            return Err(ExactDeltaCdfConsumptionRequestError::NonAdvancingRange {
                from_version,
                through_version,
            });
        }
        let starting_version = from_version
            .checked_add(1)
            .ok_or(ExactDeltaCdfConsumptionRequestError::VersionOverflow { from_version })?;
        let key = DeltaCdfCheckpointKey::try_new(table_root, consumer_id)?;
        let from = ExactDeltaPin::new(key.canonical_root(), from_version).map_err(|error| {
            ExactDeltaCdfConsumptionRequestError::InvalidRoot(error.to_string())
        })?;
        let through =
            ExactDeltaPin::new(key.canonical_root(), through_version).map_err(|error| {
                ExactDeltaCdfConsumptionRequestError::InvalidRoot(error.to_string())
            })?;
        Ok(Self {
            key,
            from,
            through,
            starting_version,
            schema_policy,
        })
    }

    /// Canonical source table root.
    #[must_use]
    pub const fn table_root(&self) -> &Url {
        self.key.canonical_root()
    }

    /// Exact consumer identity.
    #[must_use]
    pub fn consumer_id(&self) -> &str {
        self.key.consumer_id()
    }

    /// Durable checkpoint version required before this range may execute.
    #[must_use]
    pub const fn from_version(&self) -> u64 {
        self.from.version()
    }

    /// First CDF commit version in the explicit inclusive range.
    #[must_use]
    pub const fn starting_version(&self) -> u64 {
        self.starting_version
    }

    /// Exact inclusive end of the requested range.
    #[must_use]
    pub const fn through_version(&self) -> u64 {
        self.through.version()
    }

    /// Exact source pin at the inclusive end.
    #[must_use]
    pub const fn through_pin(&self) -> &ExactDeltaPin {
        &self.through
    }

    /// Caller-selected schema evolution contract.
    #[must_use]
    pub const fn schema_policy(&self) -> DeltaCdfSchemaEvolutionPolicy {
        self.schema_policy
    }
}

/// Invalid explicit CDF request.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ExactDeltaCdfConsumptionRequestError {
    #[error(transparent)]
    InvalidIdentity(#[from] DeltaCdfCheckpointStoreError),
    #[error("invalid canonical Delta root: {0}")]
    InvalidRoot(String),
    #[error(
        "CDF request must advance beyond its checkpoint: from {from_version}, through {through_version}"
    )]
    NonAdvancingRange {
        from_version: u64,
        through_version: u64,
    },
    #[error("CDF request starting version overflows after checkpoint {from_version}")]
    VersionOverflow { from_version: u64 },
}

/// Schema condition which requires exact-snapshot reconstruction instead of incremental CDF.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeltaCdfSchemaReconstructionReason {
    /// Column mapping is not a certified CDF surface at the pinned delta-rs revision.
    ColumnMappingUnsupported { version: u64, mode: Arc<str> },
    /// The caller required exact schema stability and an exact transition changed it.
    ExactSchemaChanged { previous_version: u64, version: u64 },
    /// A transition was not limited to trailing nullable fields.
    NonAdditiveSchemaChange { previous_version: u64, version: u64 },
    /// One exact intermediate schema could not be reconstructed for compatibility proof.
    ExactSchemaUnavailable { version: u64 },
    /// delta-rs rejected a range which crossed an otherwise accepted schema transition.
    DeltaRsRejectedEvolvedRange {
        from_version: u64,
        through_version: u64,
    },
}

/// Why the coordinator requires a governed full reconstruction at one exact source version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactDeltaCdfReconstructionReason {
    /// One required Delta transaction-log entry is not retained.
    Retention(ExactDeltaCdfFallbackReason),
    /// The incremental range cannot satisfy its explicit schema contract.
    Schema(DeltaCdfSchemaReconstructionReason),
}

/// Typed fallback instruction. It never claims the reconstruction has occurred.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDeltaCdfReconstructionRequired {
    consumer_id: Arc<str>,
    checkpoint_before: ExactDeltaPin,
    reconstruct_at: ExactDeltaPin,
    starting_version: u64,
    through_version: u64,
    reason: ExactDeltaCdfReconstructionReason,
}

impl ExactDeltaCdfReconstructionRequired {
    #[must_use]
    pub fn consumer_id(&self) -> &str {
        &self.consumer_id
    }

    #[must_use]
    pub const fn checkpoint_before(&self) -> &ExactDeltaPin {
        &self.checkpoint_before
    }

    #[must_use]
    pub const fn reconstruct_at(&self) -> &ExactDeltaPin {
        &self.reconstruct_at
    }

    #[must_use]
    pub const fn starting_version(&self) -> u64 {
        self.starting_version
    }

    #[must_use]
    pub const fn through_version(&self) -> u64 {
        self.through_version
    }

    #[must_use]
    pub const fn reason(&self) -> &ExactDeltaCdfReconstructionReason {
        &self.reason
    }
}

/// Result of one exact consume/replay attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactDeltaCdfConsumptionOutcome {
    /// The exact range was durably applied and its checkpoint CAS advanced.
    Applied {
        checkpoint: DurableDeltaCdfCheckpoint,
        batch_count: usize,
        row_count: usize,
    },
    /// The exact through-version was already the durable checkpoint; no downstream call ran.
    ReplayConfirmed {
        checkpoint: DurableDeltaCdfCheckpoint,
    },
    /// Durable progress already passed this exact request; no checkpoint was regressed.
    Superseded {
        checkpoint: DurableDeltaCdfCheckpoint,
    },
    /// Incremental transport cannot prove the requested range; reconstruct the named snapshot.
    ExactSnapshotReconstructionRequired(ExactDeltaCdfReconstructionRequired),
}

/// Idempotent durable downstream for exact CDF batches.
///
/// Implementations must key replay by the request's canonical root, consumer, and exact version
/// range. Returning `Ok` asserts the batches are durable and supplies the commit identity which
/// justifies checkpoint advancement. Returning `Err` issues no advancement token.
#[async_trait]
pub trait ExactDeltaCdfDownstream: fmt::Debug + Send + Sync {
    async fn consume_exact_cdf(
        &self,
        request: &ExactDeltaCdfConsumptionRequest,
        batches: &[RecordBatch],
    ) -> Result<DeltaCdfDownstreamCommit, DeltaCdfDownstreamFailure>;
}

/// Coordinator joining exact delta-rs CDF execution to the durable SQLite checkpoint authority.
pub struct ExactDeltaCdfReplayCoordinator {
    checkpoints: Arc<SqliteDeltaCdfCheckpointStore>,
}

impl fmt::Debug for ExactDeltaCdfReplayCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactDeltaCdfReplayCoordinator")
            .field("checkpoint_database", &self.checkpoints.database_path())
            .finish_non_exhaustive()
    }
}

impl ExactDeltaCdfReplayCoordinator {
    /// Bind an explicit durable checkpoint authority. There is no in-memory/default authority.
    #[must_use]
    pub fn new(checkpoints: Arc<SqliteDeltaCdfCheckpointStore>) -> Self {
        Self { checkpoints }
    }

    /// Consume one explicit exact range or return a typed exact-snapshot fallback.
    ///
    /// `source_through` must already be loaded at `request.through_version()` and its canonical
    /// root must equal the request root. No latest-version API is called. The request executes only
    /// when the durable checkpoint equals `from_version`; exact replay and superseded work are
    /// resolved from durable evidence without calling the downstream.
    ///
    /// # Errors
    ///
    /// Fails closed on missing/mismatched checkpoints, exact source identity failures, CDF
    /// preparation/execution failures, downstream failure, or a non-idempotent checkpoint race.
    pub async fn consume<D: ExactDeltaCdfDownstream + ?Sized>(
        &self,
        request: &ExactDeltaCdfConsumptionRequest,
        source_through: &DeltaTable,
        epoch_session: Arc<SessionState>,
        downstream: &D,
    ) -> Result<ExactDeltaCdfConsumptionOutcome, ExactDeltaCdfCoordinatorError> {
        let checkpoint = self
            .checkpoints
            .load(request.key.clone())
            .await?
            .ok_or_else(|| ExactDeltaCdfCoordinatorError::MissingCheckpoint {
                table_root: request.table_root().to_string(),
                consumer_id: request.consumer_id().to_owned(),
            })?;
        let observed_version = checkpoint.consumed_through().version();
        if observed_version == request.through_version() {
            return Ok(ExactDeltaCdfConsumptionOutcome::ReplayConfirmed { checkpoint });
        }
        if observed_version > request.through_version() {
            return Ok(ExactDeltaCdfConsumptionOutcome::Superseded { checkpoint });
        }
        if observed_version != request.from_version() {
            return Err(ExactDeltaCdfCoordinatorError::CheckpointMismatch {
                requested_from_version: request.from_version(),
                observed_version,
            });
        }

        let window = checkpoint
            .next_window(request.starting_version(), request.through_version())?
            .ok_or(ExactDeltaCdfCoordinatorError::NonAdvancingPreparedRange)?;
        ValidatedDeltaSnapshot::try_from_loaded_table(
            source_through.clone(),
            request.through_pin(),
        )?;
        source_through
            .update_datafusion_session(epoch_session.as_ref())
            .map_err(|error| {
                ExactDeltaCdfCoordinatorError::ExactIdentity(ExactDeltaProviderError::Delta(error))
            })?;
        let preparation = prepare_exact_delta_cdf(
            &checkpoint,
            source_through,
            request.through_pin(),
            window,
            epoch_session,
        )
        .await;

        let prepared = match preparation {
            Ok(ExactDeltaCdfPreparation::ExactSnapshotFallback(fallback)) => {
                return Ok(
                    ExactDeltaCdfConsumptionOutcome::ExactSnapshotReconstructionRequired(
                        reconstruction_required(
                            request,
                            &checkpoint,
                            ExactDeltaCdfReconstructionReason::Retention(fallback.reason()),
                        ),
                    ),
                );
            }
            Ok(ExactDeltaCdfPreparation::PhysicalPlan(prepared)) => prepared,
            Err(error) => {
                let inspection = inspect_schema_range(source_through, request).await?;
                if let Some(reason) = inspection.reconstruction_reason {
                    return Ok(schema_reconstruction_outcome(request, &checkpoint, reason));
                }
                if inspection.changed
                    && matches!(&error, ExactDeltaCdfPreparationError::BuildPlan(_))
                {
                    return Ok(schema_reconstruction_outcome(
                        request,
                        &checkpoint,
                        DeltaCdfSchemaReconstructionReason::DeltaRsRejectedEvolvedRange {
                            from_version: request.from_version(),
                            through_version: request.through_version(),
                        },
                    ));
                }
                return Err(error.into());
            }
        };

        let inspection = inspect_schema_range(source_through, request).await?;
        if let Some(reason) = inspection.reconstruction_reason {
            return Ok(schema_reconstruction_outcome(request, &checkpoint, reason));
        }

        let executed = prepared.execute().await?;
        let batch_count = executed.batches().len();
        let row_count = executed.row_count();
        let downstream_result = downstream
            .consume_exact_cdf(request, executed.batches())
            .await;
        let success = executed.finish_downstream(downstream_result)?;
        let replacement = checkpoint
            .advance_after_downstream_success(success)
            .map_err(ExactDeltaCdfCoordinatorError::CheckpointInvariant)?;
        match self
            .checkpoints
            .compare_and_swap(checkpoint, replacement.clone())
            .await?
        {
            DeltaCdfCheckpointCompareAndSwap::Advanced(checkpoint) => {
                Ok(ExactDeltaCdfConsumptionOutcome::Applied {
                    checkpoint,
                    batch_count,
                    row_count,
                })
            }
            DeltaCdfCheckpointCompareAndSwap::Conflict {
                observed: Some(observed),
            } if observed == replacement => Ok(ExactDeltaCdfConsumptionOutcome::ReplayConfirmed {
                checkpoint: observed,
            }),
            DeltaCdfCheckpointCompareAndSwap::Conflict { observed } => {
                Err(ExactDeltaCdfCoordinatorError::CheckpointConflict {
                    requested_through_version: request.through_version(),
                    observed_version: observed
                        .as_ref()
                        .map(|checkpoint| checkpoint.consumed_through().version()),
                })
            }
        }
    }
}

struct SchemaRangeInspection {
    changed: bool,
    reconstruction_reason: Option<DeltaCdfSchemaReconstructionReason>,
}

async fn inspect_schema_range(
    source_through: &DeltaTable,
    request: &ExactDeltaCdfConsumptionRequest,
) -> Result<SchemaRangeInspection, ExactDeltaCdfCoordinatorError> {
    let log_store = source_through.log_store();
    let mut previous: Option<(u64, SchemaRef)> = None;
    let mut changed = false;
    for version in request.from_version()..=request.through_version() {
        let table = if version == request.through_version() {
            source_through.clone()
        } else {
            let mut table = DeltaTable::new(Arc::clone(&log_store), DeltaTableConfig::default());
            if table.load_version(version).await.is_err() {
                return Ok(SchemaRangeInspection {
                    changed,
                    reconstruction_reason: Some(
                        DeltaCdfSchemaReconstructionReason::ExactSchemaUnavailable { version },
                    ),
                });
            }
            table
        };
        let pin = ExactDeltaPin::new(request.table_root(), version)
            .map_err(ExactDeltaCdfCoordinatorError::ExactIdentity)?;
        ValidatedDeltaSnapshot::try_from_loaded_table(table.clone(), &pin)?;
        let snapshot = table.snapshot().map_err(|error| {
            ExactDeltaCdfCoordinatorError::ExactIdentity(ExactDeltaProviderError::Delta(error))
        })?;
        if let Some(mode) = snapshot
            .metadata()
            .configuration()
            .get(TableProperty::ColumnMappingMode.as_ref())
            && mode != "none"
        {
            return Ok(SchemaRangeInspection {
                changed,
                reconstruction_reason: Some(
                    DeltaCdfSchemaReconstructionReason::ColumnMappingUnsupported {
                        version,
                        mode: Arc::from(mode.as_str()),
                    },
                ),
            });
        }
        let schema = snapshot.snapshot().arrow_schema();
        if let Some((previous_version, previous_schema)) = &previous
            && previous_schema.as_ref() != schema.as_ref()
        {
            changed = true;
            let incompatible = match request.schema_policy() {
                DeltaCdfSchemaEvolutionPolicy::Exact => {
                    Some(DeltaCdfSchemaReconstructionReason::ExactSchemaChanged {
                        previous_version: *previous_version,
                        version,
                    })
                }
                DeltaCdfSchemaEvolutionPolicy::AdditiveNullable
                    if !is_additive_nullable(previous_schema, &schema) =>
                {
                    Some(
                        DeltaCdfSchemaReconstructionReason::NonAdditiveSchemaChange {
                            previous_version: *previous_version,
                            version,
                        },
                    )
                }
                DeltaCdfSchemaEvolutionPolicy::AdditiveNullable => None,
            };
            if incompatible.is_some() {
                return Ok(SchemaRangeInspection {
                    changed,
                    reconstruction_reason: incompatible,
                });
            }
        }
        previous = Some((version, schema));
    }
    Ok(SchemaRangeInspection {
        changed,
        reconstruction_reason: None,
    })
}

fn is_additive_nullable(previous: &SchemaRef, current: &SchemaRef) -> bool {
    previous.metadata() == current.metadata()
        && current.fields().len() >= previous.fields().len()
        && previous
            .fields()
            .iter()
            .zip(current.fields())
            .all(|(before, after)| before == after)
        && current
            .fields()
            .iter()
            .skip(previous.fields().len())
            .all(|field| field.is_nullable())
}

fn reconstruction_required(
    request: &ExactDeltaCdfConsumptionRequest,
    checkpoint: &DurableDeltaCdfCheckpoint,
    reason: ExactDeltaCdfReconstructionReason,
) -> ExactDeltaCdfReconstructionRequired {
    ExactDeltaCdfReconstructionRequired {
        consumer_id: Arc::from(request.consumer_id()),
        checkpoint_before: checkpoint.consumed_through().clone(),
        reconstruct_at: request.through_pin().clone(),
        starting_version: request.starting_version(),
        through_version: request.through_version(),
        reason,
    }
}

fn schema_reconstruction_outcome(
    request: &ExactDeltaCdfConsumptionRequest,
    checkpoint: &DurableDeltaCdfCheckpoint,
    reason: DeltaCdfSchemaReconstructionReason,
) -> ExactDeltaCdfConsumptionOutcome {
    ExactDeltaCdfConsumptionOutcome::ExactSnapshotReconstructionRequired(reconstruction_required(
        request,
        checkpoint,
        ExactDeltaCdfReconstructionReason::Schema(reason),
    ))
}

/// Failures which cannot be represented as a safe replay or reconstruction outcome.
#[derive(Debug, Error)]
pub enum ExactDeltaCdfCoordinatorError {
    #[error(transparent)]
    CheckpointStore(#[from] DeltaCdfCheckpointStoreError),
    #[error("no durable CDF checkpoint exists for {table_root} consumer {consumer_id}")]
    MissingCheckpoint {
        table_root: String,
        consumer_id: String,
    },
    #[error(
        "CDF request expected checkpoint {requested_from_version}, observed {observed_version}"
    )]
    CheckpointMismatch {
        requested_from_version: u64,
        observed_version: u64,
    },
    #[error("an explicit CDF request unexpectedly produced no advancing window")]
    NonAdvancingPreparedRange,
    #[error(transparent)]
    ExactIdentity(#[from] ExactDeltaProviderError),
    #[error(transparent)]
    Preparation(#[from] ExactDeltaCdfPreparationError),
    #[error(transparent)]
    Execution(#[from] ExactDeltaCdfExecutionError),
    #[error(transparent)]
    DownstreamCompletion(#[from] DeltaCdfDownstreamCompletionError),
    #[error("CDF checkpoint invariant rejected successful consumption: {0}")]
    CheckpointInvariant(#[from] DeltaCdfCheckpointError),
    #[error(
        "CDF checkpoint CAS conflicted after successful range through {requested_through_version}; observed {observed_version:?}"
    )]
    CheckpointConflict {
        requested_through_version: u64,
        observed_version: Option<u64>,
    },
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use arrow_array::StringArray;
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::prelude::{SessionConfig, SessionContext};
    use deltalake::DeltaTableBuilder;
    use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
    use deltalake::operations::create::CreateBuilder;
    use deltalake::protocol::SaveMode;
    use tempfile::TempDir;

    use super::*;
    use crate::fabric::delta_cdf_checkpoint_sqlite::DeltaCdfCheckpointInsert;

    struct CdfFixture {
        temporary: TempDir,
        root: Url,
        version_zero: DeltaTable,
    }

    async fn cdf_fixture() -> CdfFixture {
        let temporary = TempDir::new().expect("temporary exact CDF coordinator root");
        fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700))
            .expect("make exact CDF coordinator root private");
        let table_path = temporary.path().join("source");
        fs::create_dir(&table_path).expect("create source table directory");
        let root = Url::from_directory_path(&table_path).expect("source table file URL");
        let arrow_schema = Schema::new(vec![Field::new("label", DataType::Utf8, true)]);
        let kernel: deltalake::kernel::StructType = (&arrow_schema)
            .try_into_kernel()
            .expect("convert source schema to Delta");
        let version_zero = CreateBuilder::new()
            .with_location(root.to_string())
            .with_table_name("exact_cdf_replay_fixture")
            .with_save_mode(SaveMode::ErrorIfExists)
            .with_columns(kernel.fields().cloned())
            .with_configuration_property(TableProperty::EnableChangeDataFeed, Some("true"))
            .await
            .expect("create CDF-enabled source table");
        assert_eq!(version_zero.version(), Some(0));
        CdfFixture {
            temporary,
            root,
            version_zero,
        }
    }

    fn label_batch(label: &str) -> RecordBatch {
        RecordBatch::try_new(
            Arc::new(Schema::new(vec![Field::new("label", DataType::Utf8, true)])),
            vec![Arc::new(StringArray::from(vec![Some(label)]))],
        )
        .expect("construct source label batch")
    }

    async fn append_label(table: &DeltaTable, label: &str) -> DeltaTable {
        table
            .clone()
            .write(vec![label_batch(label)])
            .await
            .expect("append source label")
    }

    fn session() -> Arc<SessionState> {
        let config = SessionConfig::new()
            .set_bool(
                "datafusion.execution.parquet.schema_force_view_types",
                false,
            )
            .set_bool("datafusion.execution.parquet.pushdown_filters", false);
        Arc::new(SessionContext::new_with_config(config).state())
    }

    fn database_path(fixture: &CdfFixture) -> std::path::PathBuf {
        fixture.temporary.path().join("cdf-checkpoints.sqlite3")
    }

    fn checkpoint(root: &Url, consumer: &str, version: u64) -> DurableDeltaCdfCheckpoint {
        DurableDeltaCdfCheckpoint::try_new(
            consumer,
            0,
            ExactDeltaPin::new(root, version).expect("construct source checkpoint pin"),
            DeltaCdfDownstreamCommit::External([1; 32]),
        )
        .expect("construct source checkpoint")
    }

    async fn initialize_checkpoint(
        store: &SqliteDeltaCdfCheckpointStore,
        root: &Url,
        consumer: &str,
    ) -> DurableDeltaCdfCheckpoint {
        let checkpoint = checkpoint(root, consumer, 0);
        assert_eq!(
            store.insert_if_absent(checkpoint.clone()).await.unwrap(),
            DeltaCdfCheckpointInsert::Inserted(checkpoint.clone())
        );
        checkpoint
    }

    #[derive(Debug)]
    struct RecordingDownstream {
        calls: AtomicUsize,
        labels: Mutex<Vec<String>>,
        commit: DeltaCdfDownstreamCommit,
    }

    impl RecordingDownstream {
        fn external(marker: u8) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                labels: Mutex::new(Vec::new()),
                commit: DeltaCdfDownstreamCommit::External([marker; 32]),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        fn labels(&self) -> Vec<String> {
            self.labels.lock().expect("labels mutex is healthy").clone()
        }
    }

    #[async_trait]
    impl ExactDeltaCdfDownstream for RecordingDownstream {
        async fn consume_exact_cdf(
            &self,
            _request: &ExactDeltaCdfConsumptionRequest,
            batches: &[RecordBatch],
        ) -> Result<DeltaCdfDownstreamCommit, DeltaCdfDownstreamFailure> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let mut labels = self.labels.lock().expect("labels mutex is healthy");
            for batch in batches {
                let column = batch
                    .column_by_name("label")
                    .expect("CDF batch has source label")
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("CDF source label is Utf8");
                labels.extend(column.iter().flatten().map(str::to_owned));
            }
            Ok(self.commit.clone())
        }
    }

    #[derive(Debug)]
    struct FailingDownstream;

    #[async_trait]
    impl ExactDeltaCdfDownstream for FailingDownstream {
        async fn consume_exact_cdf(
            &self,
            _request: &ExactDeltaCdfConsumptionRequest,
            _batches: &[RecordBatch],
        ) -> Result<DeltaCdfDownstreamCommit, DeltaCdfDownstreamFailure> {
            Err(DeltaCdfDownstreamFailure::new(
                "injected durable sink failure",
            ))
        }
    }

    #[tokio::test]
    async fn process_reopen_confirms_exact_replay_without_reapplying_downstream() {
        let fixture = cdf_fixture().await;
        let version_one = append_label(&fixture.version_zero, "version-one").await;
        let database = database_path(&fixture);
        let store = Arc::new(
            SqliteDeltaCdfCheckpointStore::open(&database).expect("open checkpoint authority"),
        );
        initialize_checkpoint(&store, &fixture.root, "projection").await;
        let coordinator = ExactDeltaCdfReplayCoordinator::new(Arc::clone(&store));
        let request = ExactDeltaCdfConsumptionRequest::try_new(
            &fixture.root,
            0,
            1,
            "projection",
            DeltaCdfSchemaEvolutionPolicy::Exact,
        )
        .unwrap();
        let first_downstream = RecordingDownstream::external(2);
        let outcome = coordinator
            .consume(&request, &version_one, session(), &first_downstream)
            .await
            .expect("consume first exact range");
        assert!(matches!(
            outcome,
            ExactDeltaCdfConsumptionOutcome::Applied {
                ref checkpoint,
                row_count: 1,
                ..
            } if checkpoint.consumed_through().version() == 1
        ));
        assert_eq!(first_downstream.calls(), 1);
        assert_eq!(first_downstream.labels(), ["version-one"]);
        drop(coordinator);
        drop(store);

        let exact_one = DeltaTableBuilder::from_url(fixture.root.clone())
            .unwrap()
            .with_version(1)
            .load()
            .await
            .expect("reopen exact source version one");
        let reopened_store = Arc::new(
            SqliteDeltaCdfCheckpointStore::open(&database).expect("reopen checkpoint authority"),
        );
        let replay_coordinator = ExactDeltaCdfReplayCoordinator::new(reopened_store);
        let replay_downstream = RecordingDownstream::external(2);
        let replay = replay_coordinator
            .consume(&request, &exact_one, session(), &replay_downstream)
            .await
            .expect("confirm exact replay");
        assert!(matches!(
            replay,
            ExactDeltaCdfConsumptionOutcome::ReplayConfirmed { ref checkpoint }
                if checkpoint.consumed_through().version() == 1
        ));
        assert_eq!(replay_downstream.calls(), 0);
    }

    #[tokio::test]
    async fn downstream_failure_leaves_durable_checkpoint_unchanged() {
        let fixture = cdf_fixture().await;
        let version_one = append_label(&fixture.version_zero, "version-one").await;
        let store = Arc::new(
            SqliteDeltaCdfCheckpointStore::open(&database_path(&fixture))
                .expect("open checkpoint authority"),
        );
        let initial = initialize_checkpoint(&store, &fixture.root, "projection").await;
        let coordinator = ExactDeltaCdfReplayCoordinator::new(Arc::clone(&store));
        let request = ExactDeltaCdfConsumptionRequest::try_new(
            &fixture.root,
            0,
            1,
            "projection",
            DeltaCdfSchemaEvolutionPolicy::Exact,
        )
        .unwrap();
        assert!(matches!(
            coordinator
                .consume(&request, &version_one, session(), &FailingDownstream)
                .await,
            Err(ExactDeltaCdfCoordinatorError::DownstreamCompletion(
                DeltaCdfDownstreamCompletionError::Downstream(_)
            ))
        ));
        assert_eq!(
            store
                .load(DeltaCdfCheckpointKey::try_new(&fixture.root, "projection").unwrap())
                .await
                .unwrap(),
            Some(initial)
        );
    }

    #[tokio::test]
    async fn missing_interior_log_returns_typed_exact_reconstruction_without_consumption() {
        let fixture = cdf_fixture().await;
        let version_one = append_label(&fixture.version_zero, "version-one").await;
        let version_two = append_label(&version_one, "version-two").await;
        let store = Arc::new(
            SqliteDeltaCdfCheckpointStore::open(&database_path(&fixture))
                .expect("open checkpoint authority"),
        );
        let initial = initialize_checkpoint(&store, &fixture.root, "projection").await;
        let missing = fixture
            .root
            .to_file_path()
            .unwrap()
            .join("_delta_log/00000000000000000001.json");
        fs::remove_file(missing).expect("remove retained-log fixture entry");
        let coordinator = ExactDeltaCdfReplayCoordinator::new(Arc::clone(&store));
        let request = ExactDeltaCdfConsumptionRequest::try_new(
            &fixture.root,
            0,
            2,
            "projection",
            DeltaCdfSchemaEvolutionPolicy::Exact,
        )
        .unwrap();
        let downstream = RecordingDownstream::external(3);
        let outcome = coordinator
            .consume(&request, &version_two, session(), &downstream)
            .await
            .expect("retention gap is a typed fallback");
        let ExactDeltaCdfConsumptionOutcome::ExactSnapshotReconstructionRequired(required) =
            outcome
        else {
            panic!("retention gap must require exact reconstruction: {outcome:?}");
        };
        assert_eq!(required.reconstruct_at().version(), 2);
        assert_eq!(required.starting_version(), 1);
        assert_eq!(
            required.reason(),
            &ExactDeltaCdfReconstructionReason::Retention(
                ExactDeltaCdfFallbackReason::RequiredVersionNotRetained { version: 1 }
            )
        );
        assert_eq!(downstream.calls(), 0);
        assert_eq!(
            store
                .load(DeltaCdfCheckpointKey::try_new(&fixture.root, "projection").unwrap())
                .await
                .unwrap(),
            Some(initial)
        );
    }

    #[tokio::test]
    async fn additive_schema_evolution_is_policy_bound_and_exact_policy_falls_back() {
        let fixture = cdf_fixture().await;
        let version_one = fixture
            .version_zero
            .clone()
            .add_columns()
            .with_fields([deltalake::kernel::StructField::nullable(
                "detail",
                deltalake::kernel::DataType::STRING,
            )])
            .await
            .expect("add nullable source field");
        assert_eq!(version_one.version(), Some(1));
        let store = Arc::new(
            SqliteDeltaCdfCheckpointStore::open(&database_path(&fixture))
                .expect("open checkpoint authority"),
        );
        initialize_checkpoint(&store, &fixture.root, "exact-consumer").await;
        initialize_checkpoint(&store, &fixture.root, "additive-consumer").await;
        let coordinator = ExactDeltaCdfReplayCoordinator::new(Arc::clone(&store));

        let exact_request = ExactDeltaCdfConsumptionRequest::try_new(
            &fixture.root,
            0,
            1,
            "exact-consumer",
            DeltaCdfSchemaEvolutionPolicy::Exact,
        )
        .unwrap();
        let exact_downstream = RecordingDownstream::external(4);
        let exact_outcome = coordinator
            .consume(&exact_request, &version_one, session(), &exact_downstream)
            .await
            .expect("exact schema policy returns a typed fallback");
        assert!(matches!(
            exact_outcome,
            ExactDeltaCdfConsumptionOutcome::ExactSnapshotReconstructionRequired(ref required)
                if required.reason()
                    == &ExactDeltaCdfReconstructionReason::Schema(
                        DeltaCdfSchemaReconstructionReason::ExactSchemaChanged {
                            previous_version: 0,
                            version: 1,
                        }
                    )
        ));
        assert_eq!(exact_downstream.calls(), 0);

        let additive_request = ExactDeltaCdfConsumptionRequest::try_new(
            &fixture.root,
            0,
            1,
            "additive-consumer",
            DeltaCdfSchemaEvolutionPolicy::AdditiveNullable,
        )
        .unwrap();
        let additive_downstream = RecordingDownstream::external(5);
        let additive_outcome = coordinator
            .consume(
                &additive_request,
                &version_one,
                session(),
                &additive_downstream,
            )
            .await
            .expect("additive nullable schema is consumable");
        assert!(matches!(
            additive_outcome,
            ExactDeltaCdfConsumptionOutcome::Applied { ref checkpoint, .. }
                if checkpoint.consumed_through().version() == 1
        ));
        assert_eq!(additive_downstream.calls(), 1);
    }

    #[test]
    fn additive_schema_policy_rejects_type_change_and_required_new_field() {
        let previous = Arc::new(Schema::new(vec![Field::new("label", DataType::Utf8, true)]));
        let changed_type = Arc::new(Schema::new(vec![Field::new(
            "label",
            DataType::Int64,
            true,
        )]));
        let required_addition = Arc::new(Schema::new(vec![
            Field::new("label", DataType::Utf8, true),
            Field::new("required", DataType::Utf8, false),
        ]));
        assert!(!is_additive_nullable(&previous, &changed_type));
        assert!(!is_additive_nullable(&previous, &required_addition));
    }
}
