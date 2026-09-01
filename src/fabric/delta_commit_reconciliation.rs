//! Exact, read-only reconciliation for an uncertain controlled Delta commit.
//!
//! A controlled write has exactly one legal commit version: the exact predecessor plus one.
//! Reconciliation therefore reads that one transaction-log entry directly, reconstructs that
//! exact snapshot, and validates the application `txn` action and all command metadata through
//! the same readback path as [`super::delta_write::write_exact_delta_plan`]. It never resolves a
//! latest version, advances a shared table handle, writes, or retries.

use deltalake::logstore::LogStoreRef;
use deltalake::{DeltaTable, DeltaTableConfig};
use thiserror::Error;

use super::delta_exact::{ExactDeltaPin, read_exact_commit_entry};
use super::delta_write::{
    ApplicationTransactionMarker, CommittedDeltaWrite, ControlledDeltaWriteSpec,
    ControlledDeltaWriteUnknownStage, readback_exact_delta_commit,
};

/// Complete application-owned material required to reconcile one uncertain write.
///
/// There is intentionally no `Default`: the exact write contract and the original DataFusion
/// session identity must come from the durable command record which launched the write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UncertainDeltaCommitRequest {
    write: ControlledDeltaWriteSpec,
    recorded_session_id: String,
}

impl UncertainDeltaCommitRequest {
    /// Bind an exact write contract to the session identity recorded before execution.
    ///
    /// # Errors
    ///
    /// Rejects an empty session identity because an absent identity cannot prove commit metadata.
    pub fn try_new(
        write: ControlledDeltaWriteSpec,
        recorded_session_id: impl Into<String>,
    ) -> Result<Self, UncertainDeltaCommitInputError> {
        let recorded_session_id = recorded_session_id.into();
        if recorded_session_id.trim().is_empty() {
            return Err(UncertainDeltaCommitInputError::EmptyRecordedSessionId);
        }
        Ok(Self {
            write,
            recorded_session_id,
        })
    }

    /// Exact controlled-write contract whose outcome is uncertain.
    #[must_use]
    pub const fn write(&self) -> &ControlledDeltaWriteSpec {
        &self.write
    }

    /// Original DataFusion session identity expected in the exact commit metadata.
    #[must_use]
    pub fn recorded_session_id(&self) -> &str {
        &self.recorded_session_id
    }
}

/// Invalid reconciliation input rejected before any log-store observation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum UncertainDeltaCommitInputError {
    /// The writer always records a concrete DataFusion session identity.
    #[error("uncertain Delta commit reconciliation requires a nonempty recorded session ID")]
    EmptyRecordedSessionId,
}

/// Application transaction observed directly in one exact Delta commit entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedDeltaApplicationTransaction {
    application_id: String,
    application_version: i64,
}

impl ObservedDeltaApplicationTransaction {
    /// Observed delta-rs `txn.appId` value, including invalid or unexpected values.
    #[must_use]
    pub fn application_id(&self) -> &str {
        &self.application_id
    }

    /// Observed delta-rs `txn.version` value.
    #[must_use]
    pub const fn application_version(&self) -> i64 {
        self.application_version
    }
}

/// Direct evidence that the one legal successor commit does not exist.
///
/// This is deliberately narrower than permission to retry. It proves that the exact target log
/// object was absent while the immediately preceding exact commit remained reconstructable and
/// its application marker had not already reached the requested value. Any subsequent attempt is
/// a new application-owned transaction decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactDeltaNonCommitEvidence {
    predecessor: ExactDeltaPin,
    absent_target: ExactDeltaPin,
    marker: ApplicationTransactionMarker,
    predecessor_marker_version: Option<i64>,
}

impl ExactDeltaNonCommitEvidence {
    /// Exact predecessor reconstructed during the bounded observation.
    #[must_use]
    pub const fn predecessor(&self) -> &ExactDeltaPin {
        &self.predecessor
    }

    /// Sole legal successor whose transaction-log entry was absent.
    #[must_use]
    pub const fn absent_target(&self) -> &ExactDeltaPin {
        &self.absent_target
    }

    /// Application transaction identity which was not committed at the target.
    #[must_use]
    pub const fn marker(&self) -> &ApplicationTransactionMarker {
        &self.marker
    }

    /// Lower application version visible in the predecessor, if one existed.
    #[must_use]
    pub const fn predecessor_marker_version(&self) -> Option<i64> {
        self.predecessor_marker_version
    }
}

/// Why exact evidence could not distinguish a committed write from a safe non-commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UncertainDeltaCommitAmbiguity {
    /// No successor version can be represented for the supplied predecessor.
    TargetVersionOverflow { predecessor: ExactDeltaPin },
    /// The supplied delta-rs authority resolves to a different canonical table root.
    AuthorityRootMismatch {
        expected_root: String,
        observed_root: String,
    },
    /// The supplied authority root could not be canonicalized.
    AuthorityRootInvalid { detail: String },
    /// The canonical target pin could not be reconstructed.
    TargetPinInvalid {
        predecessor: ExactDeltaPin,
        detail: String,
    },
    /// The exact target log object could not be read reliably.
    TargetCommitReadFailed {
        target: ExactDeltaPin,
        detail: String,
    },
    /// The exact predecessor snapshot could not be reconstructed.
    PredecessorSnapshotUnavailable {
        predecessor: ExactDeltaPin,
        detail: String,
    },
    /// The predecessor marker already reached or passed this transaction identity.
    ApplicationTransactionAlreadyVisible {
        predecessor: ExactDeltaPin,
        expected: ApplicationTransactionMarker,
        observed_version: i64,
    },
    /// The application marker could not be read from the exact predecessor snapshot.
    PredecessorMarkerReadFailed {
        predecessor: ExactDeltaPin,
        detail: String,
    },
    /// Target absence could be caused by retained-away or damaged log evidence.
    PredecessorCommitEvidenceUnavailable {
        predecessor: ExactDeltaPin,
        target: ExactDeltaPin,
        detail: String,
    },
    /// The present target commit could not be reconstructed as its exact Delta snapshot.
    TargetSnapshotUnavailable {
        target: ExactDeltaPin,
        detail: String,
    },
    /// The present target log entry did not provide one valid `commitInfo` evidence record.
    TargetCommitEvidenceInvalid {
        target: ExactDeltaPin,
        detail: String,
    },
    /// The exact target contained no application transaction action.
    MissingApplicationTransaction { target: ExactDeltaPin },
    /// The exact target contained more than the controlled writer's sole transaction action.
    MultipleApplicationTransactions {
        target: ExactDeltaPin,
        observed: Vec<ObservedDeltaApplicationTransaction>,
    },
    /// The exact target's sole application transaction has a different identity.
    ApplicationTransactionConflict {
        target: ExactDeltaPin,
        expected: ApplicationTransactionMarker,
        observed: ObservedDeltaApplicationTransaction,
    },
    /// Snapshot transaction state did not corroborate the exact target action.
    TargetMarkerConflict {
        target: ExactDeltaPin,
        expected_version: i64,
        observed_version: Option<i64>,
    },
    /// Snapshot transaction state could not be read from the exact target.
    TargetMarkerReadFailed {
        target: ExactDeltaPin,
        detail: String,
    },
    /// The transaction identity matched, but command/session/fence/layout/retry metadata did not.
    CommitEvidenceConflict {
        target: ExactDeltaPin,
        stage: ControlledDeltaWriteUnknownStage,
        detail: String,
    },
}

/// Exhaustive read-only reconciliation result for one uncertain controlled write.
#[derive(Debug)]
pub enum UncertainDeltaCommitOutcome {
    /// Exact transaction and command metadata prove the expected successor committed once.
    Committed(CommittedDeltaWrite),
    /// Exact predecessor continuity plus direct successor lookup prove no target commit.
    NotCommitted(ExactDeltaNonCommitEvidence),
    /// Evidence was missing, retained away, malformed, or conflicting; no retry is authorized.
    Ambiguous(UncertainDeltaCommitAmbiguity),
}

/// Reconcile one uncertain controlled Delta write without discovering latest state or retrying.
///
/// `authority` contributes only its canonical log store. Its currently loaded version is ignored.
/// The function reads the write contract's exact predecessor and its sole legal successor.
pub async fn reconcile_uncertain_delta_commit(
    authority: &DeltaTable,
    request: &UncertainDeltaCommitRequest,
) -> UncertainDeltaCommitOutcome {
    let spec = request.write();
    let predecessor = spec.predecessor();
    let Some(target_version) = predecessor.version().checked_add(1) else {
        return UncertainDeltaCommitOutcome::Ambiguous(
            UncertainDeltaCommitAmbiguity::TargetVersionOverflow {
                predecessor: predecessor.clone(),
            },
        );
    };
    let target = match ExactDeltaPin::new(predecessor.canonical_root(), target_version) {
        Ok(target) => target,
        Err(error) => {
            return UncertainDeltaCommitOutcome::Ambiguous(
                UncertainDeltaCommitAmbiguity::TargetPinInvalid {
                    predecessor: predecessor.clone(),
                    detail: error.to_string(),
                },
            );
        }
    };
    let observed_authority = match ExactDeltaPin::new(authority.table_url(), predecessor.version())
    {
        Ok(observed) => observed,
        Err(error) => {
            return UncertainDeltaCommitOutcome::Ambiguous(
                UncertainDeltaCommitAmbiguity::AuthorityRootInvalid {
                    detail: error.to_string(),
                },
            );
        }
    };
    if observed_authority.canonical_root() != predecessor.canonical_root() {
        return UncertainDeltaCommitOutcome::Ambiguous(
            UncertainDeltaCommitAmbiguity::AuthorityRootMismatch {
                expected_root: predecessor.canonical_root().to_string(),
                observed_root: observed_authority.canonical_root().to_string(),
            },
        );
    }

    let log_store = authority.log_store();
    let target_present = match log_store.read_commit_entry(target_version).await {
        Ok(entry) => entry.is_some(),
        Err(error) => {
            return UncertainDeltaCommitOutcome::Ambiguous(
                UncertainDeltaCommitAmbiguity::TargetCommitReadFailed {
                    target,
                    detail: error.to_string(),
                },
            );
        }
    };

    let predecessor_marker =
        match inspect_predecessor_marker(log_store.clone(), spec, &target, !target_present).await {
            Ok(marker) => marker,
            Err(ambiguity) => return UncertainDeltaCommitOutcome::Ambiguous(ambiguity),
        };

    if !target_present {
        return UncertainDeltaCommitOutcome::NotCommitted(ExactDeltaNonCommitEvidence {
            predecessor: predecessor.clone(),
            absent_target: target,
            marker: spec.marker().clone(),
            predecessor_marker_version: predecessor_marker,
        });
    }

    let target_table = match load_exact_table(log_store, target_version).await {
        Ok(table) => table,
        Err(error) => {
            return UncertainDeltaCommitOutcome::Ambiguous(
                UncertainDeltaCommitAmbiguity::TargetSnapshotUnavailable {
                    target,
                    detail: error,
                },
            );
        }
    };
    let entry = match read_exact_commit_entry(&target_table).await {
        Ok(entry) => entry,
        Err(error) => {
            return UncertainDeltaCommitOutcome::Ambiguous(
                UncertainDeltaCommitAmbiguity::TargetCommitEvidenceInvalid {
                    target,
                    detail: error.to_string(),
                },
            );
        }
    };
    let observed = entry
        .application_transactions()
        .iter()
        .map(|transaction| ObservedDeltaApplicationTransaction {
            application_id: transaction.app_id.clone(),
            application_version: transaction.version,
        })
        .collect::<Vec<_>>();
    let observed_transaction = match observed.as_slice() {
        [] => {
            return UncertainDeltaCommitOutcome::Ambiguous(
                UncertainDeltaCommitAmbiguity::MissingApplicationTransaction { target },
            );
        }
        [transaction] => transaction,
        _ => {
            return UncertainDeltaCommitOutcome::Ambiguous(
                UncertainDeltaCommitAmbiguity::MultipleApplicationTransactions { target, observed },
            );
        }
    };
    if observed_transaction.application_id() != spec.marker().application_id()
        || observed_transaction.application_version() != spec.marker().application_version()
    {
        return UncertainDeltaCommitOutcome::Ambiguous(
            UncertainDeltaCommitAmbiguity::ApplicationTransactionConflict {
                target,
                expected: spec.marker().clone(),
                observed: observed_transaction.clone(),
            },
        );
    }

    let target_marker = match target_table.snapshot() {
        Ok(snapshot) => {
            snapshot
                .transaction_version(
                    target_table.log_store().as_ref(),
                    spec.marker().application_id(),
                )
                .await
        }
        Err(error) => Err(error),
    };
    let target_marker = match target_marker {
        Ok(marker) => marker,
        Err(error) => {
            return UncertainDeltaCommitOutcome::Ambiguous(
                UncertainDeltaCommitAmbiguity::TargetMarkerReadFailed {
                    target,
                    detail: error.to_string(),
                },
            );
        }
    };
    if target_marker != Some(spec.marker().application_version()) {
        return UncertainDeltaCommitOutcome::Ambiguous(
            UncertainDeltaCommitAmbiguity::TargetMarkerConflict {
                target,
                expected_version: spec.marker().application_version(),
                observed_version: target_marker,
            },
        );
    }

    match readback_exact_delta_commit(&target_table, spec, request.recorded_session_id()).await {
        Ok(committed) => UncertainDeltaCommitOutcome::Committed(committed),
        Err(error) => UncertainDeltaCommitOutcome::Ambiguous(
            UncertainDeltaCommitAmbiguity::CommitEvidenceConflict {
                target,
                stage: error.stage(),
                detail: error.detail().to_owned(),
            },
        ),
    }
}

async fn inspect_predecessor_marker(
    log_store: LogStoreRef,
    spec: &ControlledDeltaWriteSpec,
    target: &ExactDeltaPin,
    require_exact_commit_entry: bool,
) -> Result<Option<i64>, UncertainDeltaCommitAmbiguity> {
    let predecessor = match load_exact_table(log_store, spec.predecessor().version()).await {
        Ok(table) => table,
        Err(detail) => {
            return Err(
                UncertainDeltaCommitAmbiguity::PredecessorSnapshotUnavailable {
                    predecessor: spec.predecessor().clone(),
                    detail,
                },
            );
        }
    };
    if require_exact_commit_entry && let Err(error) = read_exact_commit_entry(&predecessor).await {
        return Err(
            UncertainDeltaCommitAmbiguity::PredecessorCommitEvidenceUnavailable {
                predecessor: spec.predecessor().clone(),
                target: target.clone(),
                detail: error.to_string(),
            },
        );
    }
    let marker = predecessor
        .snapshot()
        .map_err(
            |error| UncertainDeltaCommitAmbiguity::PredecessorMarkerReadFailed {
                predecessor: spec.predecessor().clone(),
                detail: error.to_string(),
            },
        )?
        .transaction_version(
            predecessor.log_store().as_ref(),
            spec.marker().application_id(),
        )
        .await
        .map_err(
            |error| UncertainDeltaCommitAmbiguity::PredecessorMarkerReadFailed {
                predecessor: spec.predecessor().clone(),
                detail: error.to_string(),
            },
        )?;
    if let Some(observed_version) = marker
        && observed_version >= spec.marker().application_version()
    {
        return Err(
            UncertainDeltaCommitAmbiguity::ApplicationTransactionAlreadyVisible {
                predecessor: spec.predecessor().clone(),
                expected: spec.marker().clone(),
                observed_version,
            },
        );
    }
    Ok(marker)
}

async fn load_exact_table(log_store: LogStoreRef, version: u64) -> Result<DeltaTable, String> {
    let mut table = DeltaTable::new(log_store, DeltaTableConfig::default());
    table
        .load_version(version)
        .await
        .map_err(|error| error.to_string())?;
    if table.version() != Some(version) {
        return Err(format!(
            "exact Delta load returned version {:?}, expected {version}",
            table.version()
        ));
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::fs::{self, File};
    use std::sync::Arc;

    use arrow_array::{Int64Array, RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::execution::SessionStateBuilder;
    use datafusion::prelude::{SessionConfig, SessionContext};
    use deltalake::DeltaTableBuilder;
    use deltalake::delta_datafusion::planner::DeltaPlanner;
    use deltalake::kernel::Transaction;
    use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
    use deltalake::kernel::transaction::CommitProperties;
    use deltalake::operations::create::CreateBuilder;
    use deltalake::protocol::SaveMode;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use serde_json::Value;
    use tempfile::TempDir;
    use url::Url;

    use super::*;
    use crate::fabric::command::{OperationId, WriterGeneration};
    use crate::fabric::delta_write::{
        ControlledDeltaHistoryProperties, ControlledDeltaWriteMode, ControlledDeltaWriteOutcome,
        SessionBoundLogicalPlan, write_exact_delta_plan,
    };

    struct Fixture {
        _temporary: TempDir,
        root: Url,
        predecessor: DeltaTable,
    }

    async fn fixture() -> Fixture {
        let temporary = TempDir::new().expect("temporary Delta reconciliation fixture root");
        let table_path = temporary.path().join("table");
        fs::create_dir_all(&table_path).expect("create Delta reconciliation fixture directory");
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
                    .with_table_name("uncertain_delta_commit_fixture")
                    .with_save_mode(SaveMode::ErrorIfExists)
                    .with_columns(kernel.fields().cloned()),
            )
            .await
            .expect("create Delta reconciliation fixture");

        let predecessor = DeltaTableBuilder::from_url(root.clone())
            .expect("construct exact fixture loader")
            .with_version(0)
            .load()
            .await
            .expect("load fixture predecessor version zero");
        Fixture {
            _temporary: temporary,
            root,
            predecessor,
        }
    }

    fn value_field() -> Field {
        Field::new("value", DataType::Int64, false).with_metadata(HashMap::from([(
            "codefabric.semantic_type".to_owned(),
            "signed-reconciliation-value".to_owned(),
        )]))
    }

    fn batch(value: i64) -> RecordBatch {
        RecordBatch::try_new(
            Arc::new(Schema::new(vec![value_field()])),
            vec![Arc::new(Int64Array::from(vec![value]))],
        )
        .expect("fixture batch")
    }

    fn write_spec(root: &Url) -> ControlledDeltaWriteSpec {
        ControlledDeltaWriteSpec::new(
            ExactDeltaPin::new(root, 0).expect("exact fixture predecessor"),
            OperationId::from_bytes([0x41; 16]),
            WriterGeneration::new(11).expect("nonzero fixture generation"),
            ApplicationTransactionMarker::new("codefabric/test/uncertain-write", 7)
                .expect("valid fixture transaction marker"),
            ControlledDeltaWriteMode::Append,
        )
        .with_commit_metadata(BTreeMap::from([(
            "codefabric.reconciliation.fixture".to_owned(),
            Value::String("exact-successor".to_owned()),
        )]))
        .expect("fixture application metadata")
    }

    fn session_and_input(value: i64) -> (String, SessionBoundLogicalPlan) {
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
        let session_id = state.session_id().to_owned();
        let input = SessionBoundLogicalPlan::try_from_dataframe(state, dataframe)
            .expect("bind fixture plan to exact session");
        (session_id, input)
    }

    async fn commit_once(
        fixture: &Fixture,
        spec: &ControlledDeltaWriteSpec,
        value: i64,
    ) -> (String, DeltaTable) {
        let (session_id, input) = session_and_input(value);
        let outcome = write_exact_delta_plan(&fixture.predecessor, spec, input).await;
        let ControlledDeltaWriteOutcome::Committed(committed) = outcome else {
            panic!("expected controlled fixture commit, got {outcome:?}");
        };
        (session_id, committed.into_table())
    }

    #[tokio::test]
    async fn committed_outcome_uses_exact_transaction_and_commit_metadata() {
        let fixture = fixture().await;
        let spec = write_spec(&fixture.root);
        let (session_id, committed_table) = commit_once(&fixture, &spec, 41).await;
        assert_eq!(committed_table.version(), Some(1));
        let request = UncertainDeltaCommitRequest::try_new(spec.clone(), session_id)
            .expect("complete reconciliation request");

        let outcome = reconcile_uncertain_delta_commit(&fixture.predecessor, &request).await;
        let UncertainDeltaCommitOutcome::Committed(committed) = outcome else {
            panic!("expected exact committed reconciliation, got {outcome:?}");
        };
        assert_eq!(committed.predecessor(), spec.predecessor());
        assert_eq!(committed.committed().version(), 1);
        assert_eq!(committed.marker_evidence().marker(), spec.marker());
        assert_eq!(committed.num_retries(), 0);
        assert_eq!(committed.commit_metadata(), spec.commit_metadata());
    }

    #[tokio::test]
    async fn absent_successor_is_not_committed_only_with_exact_predecessor_continuity() {
        let fixture = fixture().await;
        let spec = write_spec(&fixture.root);
        let request = UncertainDeltaCommitRequest::try_new(spec.clone(), "recorded-session")
            .expect("complete reconciliation request");

        let outcome = reconcile_uncertain_delta_commit(&fixture.predecessor, &request).await;
        let UncertainDeltaCommitOutcome::NotCommitted(evidence) = outcome else {
            panic!("expected exact non-commit evidence, got {outcome:?}");
        };
        assert_eq!(evidence.predecessor(), spec.predecessor());
        assert_eq!(evidence.absent_target().version(), 1);
        assert_eq!(evidence.marker(), spec.marker());
        assert_eq!(evidence.predecessor_marker_version(), None);
        assert!(
            fixture
                .predecessor
                .log_store()
                .read_commit_entry(1)
                .await
                .expect("read exact absent target")
                .is_none()
        );
    }

    #[tokio::test]
    async fn missing_predecessor_evidence_keeps_target_absence_ambiguous() {
        let fixture = fixture().await;
        let spec = write_spec(&fixture.root);
        let request = UncertainDeltaCommitRequest::try_new(spec, "recorded-session")
            .expect("complete reconciliation request");
        let predecessor_path = fixture
            .root
            .to_file_path()
            .expect("local fixture table root")
            .join("_delta_log")
            .join("00000000000000000000.json");
        fs::remove_file(predecessor_path).expect("remove exact predecessor evidence");

        let outcome = reconcile_uncertain_delta_commit(&fixture.predecessor, &request).await;
        assert!(matches!(
            outcome,
            UncertainDeltaCommitOutcome::Ambiguous(
                UncertainDeltaCommitAmbiguity::PredecessorSnapshotUnavailable { .. }
                    | UncertainDeltaCommitAmbiguity::PredecessorCommitEvidenceUnavailable { .. }
            )
        ));
    }

    #[tokio::test]
    async fn conflicting_application_transaction_is_ambiguous() {
        let fixture = fixture().await;
        let spec = write_spec(&fixture.root);
        let competing = fixture
            .predecessor
            .clone()
            .write([batch(91)])
            .with_save_mode(SaveMode::Append)
            .with_commit_properties(
                CommitProperties::default()
                    .with_max_retries(0)
                    .with_application_transaction(Transaction::new(
                        "codefabric/test/other-write",
                        3,
                    )),
            )
            .await
            .expect("commit conflicting exact successor");
        assert_eq!(competing.version(), Some(1));
        let request = UncertainDeltaCommitRequest::try_new(spec, "recorded-session")
            .expect("complete reconciliation request");

        let outcome = reconcile_uncertain_delta_commit(&fixture.predecessor, &request).await;
        let UncertainDeltaCommitOutcome::Ambiguous(
            UncertainDeltaCommitAmbiguity::ApplicationTransactionConflict { observed, .. },
        ) = outcome
        else {
            panic!("expected typed transaction ambiguity, got {outcome:?}");
        };
        assert_eq!(observed.application_id(), "codefabric/test/other-write");
        assert_eq!(observed.application_version(), 3);
    }

    #[tokio::test]
    async fn process_reopen_reconstructs_the_exact_committed_outcome() {
        let fixture = fixture().await;
        let spec = write_spec(&fixture.root);
        let (session_id, committed_table) = commit_once(&fixture, &spec, 29).await;
        drop(committed_table);
        let reopened = DeltaTableBuilder::from_url(fixture.root.clone())
            .expect("construct reopened reconciliation authority")
            .with_version(0)
            .load()
            .await
            .expect("reopen exact predecessor without selecting latest");
        assert_eq!(reopened.version(), Some(0));
        let request = UncertainDeltaCommitRequest::try_new(spec, session_id)
            .expect("complete reconciliation request");

        let outcome = reconcile_uncertain_delta_commit(&reopened, &request).await;
        let UncertainDeltaCommitOutcome::Committed(committed) = outcome else {
            panic!("expected committed result after process reopen, got {outcome:?}");
        };
        assert_eq!(committed.committed().version(), 1);
        assert_eq!(committed.num_retries(), 0);
    }

    #[tokio::test]
    async fn repeated_reconciliation_never_appends_a_duplicate() {
        let fixture = fixture().await;
        let spec = write_spec(&fixture.root);
        let (session_id, committed_table) = commit_once(&fixture, &spec, 73).await;
        drop(committed_table);
        let reopened = DeltaTableBuilder::from_url(fixture.root.clone())
            .expect("construct reopened reconciliation authority")
            .with_version(0)
            .load()
            .await
            .expect("reopen exact predecessor without selecting latest");
        let request = UncertainDeltaCommitRequest::try_new(spec, session_id)
            .expect("complete reconciliation request");

        for _ in 0..2 {
            let outcome = reconcile_uncertain_delta_commit(&reopened, &request).await;
            assert!(matches!(outcome, UncertainDeltaCommitOutcome::Committed(_)));
        }
        assert!(
            reopened
                .log_store()
                .read_commit_entry(2)
                .await
                .expect("read forbidden duplicate version")
                .is_none(),
            "read-only reconciliation must not create a second append"
        );

        let exact_one = DeltaTableBuilder::from_url(fixture.root.clone())
            .expect("construct exact committed loader")
            .with_version(1)
            .load()
            .await
            .expect("load the sole committed version");
        let adds = exact_one
            .snapshot()
            .expect("exact committed snapshot")
            .add_actions_table(true)
            .expect("flatten active add actions");
        assert_eq!(adds.num_rows(), 1);
        let relative_path = adds
            .column_by_name("path")
            .expect("active add path")
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("string active add path")
            .value(0);
        let table_path = fixture
            .root
            .to_file_path()
            .expect("local fixture table path");
        let parquet = ParquetRecordBatchReaderBuilder::try_new(
            File::open(table_path.join(relative_path)).expect("open sole active Parquet file"),
        )
        .expect("read sole active Parquet metadata");
        assert_eq!(parquet.metadata().file_metadata().num_rows(), 1);
    }
}
