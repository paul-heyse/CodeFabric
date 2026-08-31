//! Typed bridge from controlled Delta writes into the durable command reducer.
//!
//! Delta marker visibility and backend conflicts are not equivalent to a complete command
//! result. This bridge therefore confirms only a fully proved
//! [`ControlledDeltaWriteOutcome::Committed`] value. Every other outcome enters the reducer's
//! existing operation-marker/control-history reconciliation state without inventing a semantic
//! identity or guessing whether the command committed.

use super::command::{
    CommandContractError, CommandEvent, CommandEventKind, CommandReducer, CommandResult,
    DiagnosticRef, ExecutionOwner, ReconciliationClassification, Reduction, ReductionContext,
    RetryClassification, TransactionRef, UnknownCommit, UnknownCommitReason,
};
use super::command::{CommandRecord, OperationId, WriterGeneration};
use super::delta_exact::ExactDeltaPin;
use super::delta_write::{
    ApplicationMarkerEvidence, CommitReadVersionEvidence, ControlledDeltaWriteConflict,
    ControlledDeltaWriteOutcome, ControlledDeltaWriteReconciliation,
    ControlledDeltaWriteUnknownStage,
};
use thiserror::Error;

/// Bridge-specific binding failures plus the authoritative reducer error surface.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CommandDeltaBridgeError {
    /// A valid Delta receipt belongs to another command operation.
    #[error(
        "committed Delta receipt operation mismatch: expected {expected:?}, observed {observed:?}"
    )]
    ReceiptOperationMismatch {
        expected: OperationId,
        observed: OperationId,
    },
    /// A valid Delta receipt was produced under another writer generation.
    #[error(
        "committed Delta receipt writer-generation mismatch: expected {expected:?}, observed {observed:?}"
    )]
    ReceiptWriterGenerationMismatch {
        expected: WriterGeneration,
        observed: WriterGeneration,
    },
    /// The selected event violated the command reducer's existing contract.
    #[error(transparent)]
    Command(#[from] CommandContractError),
}

/// Stable projection of a Delta conflict retained by the bridge receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeltaWriteBridgeConflict {
    /// The loaded handle did not represent the command's exact root/version predecessor.
    PredecessorMismatch {
        expected: ExactDeltaPin,
        observed: Option<ExactDeltaPin>,
    },
    /// A later application transaction already occupies the marker stream.
    ApplicationTransactionAdvanced {
        application_id: String,
        requested_version: i64,
        observed_version: i64,
        observed_in: ExactDeltaPin,
    },
    /// The target Delta version was claimed or the predecessor advanced.
    CommitCollision {
        predecessor: ExactDeltaPin,
        target_version: u64,
        delta_error: String,
    },
}

/// Lossless semantic classification needed after reducing one Delta write outcome.
///
/// The committed projection omits only the live `DeltaTable` handle; all durable identities and
/// readback evidence remain present. Error strings are diagnostic observations, never reducer
/// dispatch keys.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeltaWriteBridgeClassification {
    /// Delta returned exactly predecessor + 1 and all commit/marker metadata read back.
    Committed {
        predecessor: ExactDeltaPin,
        committed: ExactDeltaPin,
        operation_id: OperationId,
        writer_generation: WriterGeneration,
        marker: ApplicationMarkerEvidence,
        session_id: String,
        read_version_evidence: CommitReadVersionEvidence,
        num_retries: u64,
    },
    /// The marker is visible, but the pinned API does not prove its introducing Delta version or
    /// the complete command result.
    MarkerAlreadyCommitted(ApplicationMarkerEvidence),
    /// Optimistic concurrency or transaction-stream conflict.
    Conflict(DeltaWriteBridgeConflict),
    /// The controlled boundary could not prove the commit outcome.
    Unknown {
        stage: ControlledDeltaWriteUnknownStage,
        predecessor: ExactDeltaPin,
        detail: String,
        delta_error: Option<String>,
    },
}

impl DeltaWriteBridgeClassification {
    /// Reducer event selected from this classification.
    #[must_use]
    pub const fn event_kind(&self) -> CommandEventKind {
        match self {
            Self::Committed { .. } => CommandEventKind::ConfirmCommit,
            Self::MarkerAlreadyCommitted(_) | Self::Conflict(_) | Self::Unknown { .. } => {
                CommandEventKind::ReportUnknownCommit
            }
        }
    }

    /// Whether marker/control-history reconciliation remains mandatory.
    #[must_use]
    pub const fn reconciliation_classification(&self) -> ReconciliationClassification {
        match self {
            Self::Committed { .. } => ReconciliationClassification::NotRequired,
            Self::MarkerAlreadyCommitted(_) | Self::Conflict(_) | Self::Unknown { .. } => {
                ReconciliationClassification::OperationMarkerAndControlHistory
            }
        }
    }

    /// Retry posture after this observation.
    #[must_use]
    pub const fn retry_classification(&self) -> RetryClassification {
        match self {
            Self::Committed { .. } => RetryClassification::Never,
            Self::MarkerAlreadyCommitted(_) | Self::Conflict(_) | Self::Unknown { .. } => {
                RetryClassification::ReconcileBeforeDecision
            }
        }
    }
}

/// Reducer result plus the exact Delta classification which selected its event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandDeltaBridgeReceipt {
    classification: DeltaWriteBridgeClassification,
    reduction: Reduction,
}

impl CommandDeltaBridgeReceipt {
    /// Exact Delta outcome projection used to select the reducer event.
    #[must_use]
    pub const fn classification(&self) -> &DeltaWriteBridgeClassification {
        &self.classification
    }

    /// Event selected by the bridge.
    #[must_use]
    pub const fn event_kind(&self) -> CommandEventKind {
        self.classification.event_kind()
    }

    /// Command reducer output; this remains the sole authority for successor state.
    #[must_use]
    pub const fn reduction(&self) -> Reduction {
        self.reduction
    }

    /// Whether the command may be retried after this observation.
    #[must_use]
    pub const fn retry_classification(&self) -> RetryClassification {
        self.classification.retry_classification()
    }

    /// Durable evidence family required before any future attempt.
    #[must_use]
    pub const fn reconciliation_classification(&self) -> ReconciliationClassification {
        self.classification.reconciliation_classification()
    }
}

/// Reduce one controlled Delta-write result without manufacturing domain identities.
///
/// The caller supplies the already-durable transaction reference, diagnostic reference,
/// execution owner, typed command result, and authoritative reduction context. `result` is used
/// only for a fully committed outcome. Marker evidence, conflicts, and unknowns all emit
/// `ReportUnknownCommit(ReadbackUnavailable)` and require the normal durable reconciliation
/// query before a commit confirmation or retry decision is legal.
///
/// # Errors
///
/// Rejects a committed receipt whose operation does not match the command or whose writer
/// generation does not match the execution owner (including a newer recovery generation), then
/// returns the underlying command-contract error for an illegal state, owner, fence, transaction,
/// context, or result-kind transition.
pub fn reduce_controlled_delta_write(
    record: &CommandRecord,
    outcome: &ControlledDeltaWriteOutcome,
    transaction: TransactionRef,
    diagnostic: DiagnosticRef,
    owner: ExecutionOwner,
    result: CommandResult,
    context: ReductionContext,
) -> Result<CommandDeltaBridgeReceipt, CommandDeltaBridgeError> {
    validate_committed_binding(record, outcome, owner)?;
    let classification = classify(outcome);
    let event = match classification.event_kind() {
        CommandEventKind::ConfirmCommit => CommandEvent::ConfirmCommit {
            owner,
            transaction,
            result,
        },
        CommandEventKind::ReportUnknownCommit => CommandEvent::ReportUnknownCommit {
            owner,
            transaction,
            unknown: UnknownCommit {
                reason: UnknownCommitReason::ReadbackUnavailable,
                diagnostic,
            },
        },
        _ => unreachable!("Delta outcome classification selects only commit terminal events"),
    };
    let reduction = CommandReducer::reduce(record, event, context)?;
    Ok(CommandDeltaBridgeReceipt {
        classification,
        reduction,
    })
}

fn validate_committed_binding(
    record: &CommandRecord,
    outcome: &ControlledDeltaWriteOutcome,
    owner: ExecutionOwner,
) -> Result<(), CommandDeltaBridgeError> {
    let ControlledDeltaWriteOutcome::Committed(committed) = outcome else {
        return Ok(());
    };
    let expected_operation = record.command().identity.operation_id;
    if committed.operation_id() != expected_operation {
        return Err(CommandDeltaBridgeError::ReceiptOperationMismatch {
            expected: expected_operation,
            observed: committed.operation_id(),
        });
    }
    let expected_generation = owner.fence.generation;
    if committed.writer_generation() != expected_generation {
        return Err(CommandDeltaBridgeError::ReceiptWriterGenerationMismatch {
            expected: expected_generation,
            observed: committed.writer_generation(),
        });
    }
    Ok(())
}

fn classify(outcome: &ControlledDeltaWriteOutcome) -> DeltaWriteBridgeClassification {
    match outcome {
        ControlledDeltaWriteOutcome::Committed(committed) => {
            DeltaWriteBridgeClassification::Committed {
                predecessor: committed.predecessor().clone(),
                committed: committed.committed().clone(),
                operation_id: committed.operation_id(),
                writer_generation: committed.writer_generation(),
                marker: committed.marker_evidence().clone(),
                session_id: committed.session_id().to_owned(),
                read_version_evidence: committed.read_version_evidence(),
                num_retries: committed.num_retries(),
            }
        }
        ControlledDeltaWriteOutcome::MarkerAlreadyCommitted(evidence) => {
            DeltaWriteBridgeClassification::MarkerAlreadyCommitted(evidence.clone())
        }
        ControlledDeltaWriteOutcome::Reconcile(ControlledDeltaWriteReconciliation::Conflict(
            conflict,
        )) => DeltaWriteBridgeClassification::Conflict(classify_conflict(conflict)),
        ControlledDeltaWriteOutcome::Reconcile(ControlledDeltaWriteReconciliation::Unknown(
            unknown,
        )) => DeltaWriteBridgeClassification::Unknown {
            stage: unknown.stage(),
            predecessor: unknown.predecessor().clone(),
            detail: unknown.detail().to_owned(),
            delta_error: unknown.source().map(ToString::to_string),
        },
    }
}

fn classify_conflict(conflict: &ControlledDeltaWriteConflict) -> DeltaWriteBridgeConflict {
    match conflict {
        ControlledDeltaWriteConflict::PredecessorMismatch { expected, observed } => {
            DeltaWriteBridgeConflict::PredecessorMismatch {
                expected: expected.clone(),
                observed: observed.clone(),
            }
        }
        ControlledDeltaWriteConflict::ApplicationTransactionAdvanced {
            application_id,
            requested_version,
            observed_version,
            observed_in,
        } => DeltaWriteBridgeConflict::ApplicationTransactionAdvanced {
            application_id: application_id.clone(),
            requested_version: *requested_version,
            observed_version: *observed_version,
            observed_in: observed_in.clone(),
        },
        ControlledDeltaWriteConflict::CommitCollision {
            predecessor,
            target_version,
            source,
        } => DeltaWriteBridgeConflict::CommitCollision {
            predecessor: predecessor.clone(),
            target_version: *target_version,
            delta_error: source.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use arrow_array::{Int64Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::execution::{SessionState, SessionStateBuilder};
    use datafusion::prelude::{SessionConfig, SessionContext};
    use deltalake::DeltaTableBuilder;
    use deltalake::delta_datafusion::planner::DeltaPlanner;
    use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
    use deltalake::operations::create::CreateBuilder;
    use deltalake::protocol::SaveMode;
    use tempfile::TempDir;
    use url::Url;

    use super::*;
    use crate::fabric::command::{
        ActorId, AdmissionContext, AdmissionOutcome, AuthorizationDecision, AuthorizationRef,
        CommandIdentity, CommandKind, CommandOwnership, CommandPins, CommitConfirmation,
        CompilerReleaseRef, DurableCommandState, EpochId, ExpectedHead, FabricCommand,
        FabricCommandPayload, IdempotencyKey, LeaseId, ModelHeadRef, OperationSelectionRef,
        PrincipalId, ProofReceiptRef, ProviderSetRef, ReconciliationEvidenceRef,
        ReconciliationObservation, RelationSetRef, ResourceEnvelopeRef, SourceGeneration,
        WorkspaceId, WriterFence,
    };
    use crate::fabric::delta_write::{
        ApplicationTransactionMarker, ControlledDeltaWriteMode, ControlledDeltaWriteSpec,
        SessionBoundLogicalPlan, write_exact_delta_plan,
    };

    struct Fixture {
        _temporary: TempDir,
        root: Url,
        table: deltalake::DeltaTable,
    }

    async fn fixture() -> Fixture {
        let temporary = TempDir::new().expect("temporary command/Delta bridge fixture");
        let table_path = temporary.path().join("table");
        fs::create_dir_all(&table_path).expect("create bridge fixture directory");
        let root = Url::from_directory_path(&table_path).expect("fixture table URL");
        let schema = Schema::new(vec![Field::new("value", DataType::Int64, false)]);
        let kernel: deltalake::kernel::StructType = (&schema)
            .try_into_kernel()
            .expect("convert bridge fixture schema");
        CreateBuilder::new()
            .with_location(root.to_string())
            .with_table_name("command_delta_bridge_fixture")
            .with_save_mode(SaveMode::ErrorIfExists)
            .with_columns(kernel.fields().cloned())
            .await
            .expect("create bridge fixture table");
        let table = DeltaTableBuilder::from_url(root.clone())
            .expect("construct exact fixture loader")
            .with_version(0)
            .load()
            .await
            .expect("load fixture version zero");
        Fixture {
            _temporary: temporary,
            root,
            table,
        }
    }

    fn bytes16(value: u8) -> [u8; 16] {
        [value; 16]
    }

    fn bytes32(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn fence() -> WriterFence {
        WriterFence {
            lease_id: LeaseId::from_bytes(bytes16(1)),
            generation: WriterGeneration::new(7).expect("nonzero test writer generation"),
        }
    }

    fn owner() -> ExecutionOwner {
        ExecutionOwner {
            actor_id: ActorId::from_bytes(bytes16(2)),
            fence: fence(),
        }
    }

    fn recovery_owner() -> ExecutionOwner {
        ExecutionOwner {
            actor_id: ActorId::from_bytes(bytes16(18)),
            fence: WriterFence {
                lease_id: LeaseId::from_bytes(bytes16(19)),
                generation: WriterGeneration::new(fence().generation.get() + 1)
                    .expect("recovery generation is nonzero"),
            },
        }
    }

    fn command() -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(bytes16(3)),
                idempotency_key: IdempotencyKey::from_bytes(bytes32(4)),
            },
            ownership: CommandOwnership {
                workspace_id: WorkspaceId::from_bytes(bytes16(5)),
                principal_id: PrincipalId::from_bytes(bytes16(6)),
                authorization: AuthorizationRef::from_bytes(bytes32(7)),
            },
            expected_head: ExpectedHead::Epoch(EpochId::from_bytes(bytes16(8))),
            writer_fence: fence(),
            pins: CommandPins {
                compiler_release: CompilerReleaseRef::from_bytes(bytes32(9)),
                model_head: ModelHeadRef::from_bytes(bytes32(10)),
                source_generation: SourceGeneration::new(11),
                provider_set: ProviderSetRef::from_bytes(bytes32(12)),
            },
            resources: ResourceEnvelopeRef::from_bytes(bytes32(13)),
            payload: FabricCommandPayload::CompactRelations {
                relations: RelationSetRef::from_bytes(bytes32(14)),
                equivalence_proof: ProofReceiptRef::from_bytes(bytes32(15)),
            },
        }
    }

    fn result() -> CommandResult {
        CommandResult::RelationsCompacted {
            relations: RelationSetRef::from_bytes(bytes32(14)),
            resulting_head: ExpectedHead::Epoch(EpochId::from_bytes(bytes16(16))),
            selection: OperationSelectionRef::from_bytes(bytes32(17)),
        }
    }

    fn context() -> ReductionContext {
        ReductionContext {
            current_head: command().expected_head,
            active_fence: fence(),
        }
    }

    fn recovery_context() -> ReductionContext {
        ReductionContext {
            current_head: command().expected_head,
            active_fence: recovery_owner().fence,
        }
    }

    fn prepared(transaction: TransactionRef) -> CommandRecord {
        let command = command();
        assert_eq!(command.kind(), CommandKind::CompactRelations);
        let admitted = CommandReducer::admit(
            None,
            &command,
            AdmissionContext {
                workspace_id: command.ownership.workspace_id,
                current_head: command.expected_head,
                active_fence: command.writer_fence,
                authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
            },
        )
        .expect("admit bridge command");
        let AdmissionOutcome::New(admitted) = admitted else {
            panic!("fresh bridge command must be newly admitted");
        };
        let started =
            CommandReducer::reduce(&admitted, CommandEvent::Start { owner: owner() }, context())
                .expect("start bridge command")
                .record;
        CommandReducer::reduce(
            &started,
            CommandEvent::PrepareCommit {
                owner: owner(),
                transaction,
            },
            context(),
        )
        .expect("prepare bridge transaction")
        .record
    }

    fn recovery_prepared(transaction: TransactionRef) -> CommandRecord {
        let original_transaction = TransactionRef::from_bytes(bytes32(40));
        let original_owner = owner();
        let prepared = prepared(original_transaction);
        let awaiting = CommandReducer::reduce(
            &prepared,
            CommandEvent::ReportUnknownCommit {
                owner: original_owner,
                transaction: original_transaction,
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ProcessInterrupted,
                    diagnostic: DiagnosticRef::from_bytes(bytes32(41)),
                },
            },
            context(),
        )
        .expect("original attempt becomes unknown")
        .record;
        let retry_ready = CommandReducer::reduce(
            &awaiting,
            CommandEvent::ObserveReconciliation {
                owner: recovery_owner(),
                transaction: original_transaction,
                observation: ReconciliationObservation::NotCommitted {
                    evidence: ReconciliationEvidenceRef::from_bytes(bytes32(42)),
                },
            },
            recovery_context(),
        )
        .expect("recovery proves original transaction absent")
        .record;
        let executing = CommandReducer::reduce(
            &retry_ready,
            CommandEvent::Start {
                owner: recovery_owner(),
            },
            recovery_context(),
        )
        .expect("new generation owns retry")
        .record;
        CommandReducer::reduce(
            &executing,
            CommandEvent::PrepareCommit {
                owner: recovery_owner(),
                transaction,
            },
            recovery_context(),
        )
        .expect("new generation prepares retry transaction")
        .record
    }

    fn batch(value: i64) -> RecordBatch {
        RecordBatch::try_new(
            Arc::new(Schema::new(vec![Field::new(
                "value",
                DataType::Int64,
                false,
            )])),
            vec![Arc::new(Int64Array::from(vec![value]))],
        )
        .expect("bridge fixture batch")
    }

    fn delta_session_and_input(
        value: i64,
        delta_planner: bool,
    ) -> (Arc<SessionState>, SessionBoundLogicalPlan) {
        let mut builder = SessionStateBuilder::new()
            .with_default_features()
            .with_config(SessionConfig::new());
        if delta_planner {
            builder = builder.with_query_planner(DeltaPlanner::new());
        }
        let context = SessionContext::new_with_state(builder.build());
        let dataframe = context
            .read_batch(batch(value))
            .expect("construct bridge input DataFrame");
        let state = Arc::new(context.state());
        let input = SessionBoundLogicalPlan::try_from_dataframe(Arc::clone(&state), dataframe)
            .expect("bind bridge input to exact session");
        (state, input)
    }

    fn delta_spec_with_binding(
        root: &Url,
        version: u64,
        marker_version: i64,
        operation_id: OperationId,
        writer_generation: WriterGeneration,
    ) -> ControlledDeltaWriteSpec {
        ControlledDeltaWriteSpec::new(
            ExactDeltaPin::new(root, version).expect("exact bridge table pin"),
            operation_id,
            writer_generation,
            ApplicationTransactionMarker::new("codefabric/test/command-delta", marker_version)
                .expect("valid bridge marker"),
            ControlledDeltaWriteMode::Append,
        )
    }

    fn delta_spec(root: &Url, version: u64, marker_version: i64) -> ControlledDeltaWriteSpec {
        delta_spec_with_binding(
            root,
            version,
            marker_version,
            command().identity.operation_id,
            fence().generation,
        )
    }

    fn bridge(
        record: &CommandRecord,
        outcome: &ControlledDeltaWriteOutcome,
        transaction: TransactionRef,
        diagnostic: DiagnosticRef,
    ) -> CommandDeltaBridgeReceipt {
        reduce_controlled_delta_write(
            record,
            outcome,
            transaction,
            diagnostic,
            owner(),
            result(),
            context(),
        )
        .expect("reduce controlled Delta outcome")
    }

    #[tokio::test]
    async fn committed_delta_outcome_confirms_the_prepared_command_directly() {
        let fixture = fixture().await;
        let (_, input) = delta_session_and_input(1, true);
        let outcome =
            write_exact_delta_plan(&fixture.table, &delta_spec(&fixture.root, 0, 1), input).await;
        let transaction = TransactionRef::from_bytes(bytes32(20));
        let receipt = bridge(
            &prepared(transaction),
            &outcome,
            transaction,
            DiagnosticRef::from_bytes(bytes32(21)),
        );

        assert_eq!(receipt.event_kind(), CommandEventKind::ConfirmCommit);
        assert_eq!(
            receipt.reconciliation_classification(),
            ReconciliationClassification::NotRequired
        );
        assert!(matches!(
            receipt.classification(),
            DeltaWriteBridgeClassification::Committed {
                read_version_evidence: CommitReadVersionEvidence::NotExposedByCommitHistory,
                num_retries: 0,
                ..
            }
        ));
        assert!(matches!(
            receipt.reduction().record.state(),
            DurableCommandState::Succeeded {
                confirmation: CommitConfirmation::Direct,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn recovered_retry_binds_delta_to_the_new_execution_generation() {
        let fixture = fixture().await;
        let (_, input) = delta_session_and_input(3, true);
        let outcome = write_exact_delta_plan(
            &fixture.table,
            &delta_spec_with_binding(
                &fixture.root,
                0,
                32,
                command().identity.operation_id,
                recovery_owner().fence.generation,
            ),
            input,
        )
        .await;
        let transaction = TransactionRef::from_bytes(bytes32(43));
        let receipt = reduce_controlled_delta_write(
            &recovery_prepared(transaction),
            &outcome,
            transaction,
            DiagnosticRef::from_bytes(bytes32(44)),
            recovery_owner(),
            result(),
            recovery_context(),
        )
        .expect("recovered retry validates against its execution generation");

        assert!(matches!(
            receipt.classification(),
            DeltaWriteBridgeClassification::Committed {
                writer_generation,
                ..
            } if *writer_generation == recovery_owner().fence.generation
        ));
        assert!(receipt.reduction().record.state().is_terminal());
    }

    #[tokio::test]
    async fn committed_receipt_must_match_command_operation_and_writer_generation() {
        let operation_fixture = fixture().await;
        let foreign_operation = OperationId::from_bytes(bytes16(90));
        let (_, operation_input) = delta_session_and_input(1, true);
        let operation_outcome = write_exact_delta_plan(
            &operation_fixture.table,
            &delta_spec_with_binding(
                &operation_fixture.root,
                0,
                30,
                foreign_operation,
                fence().generation,
            ),
            operation_input,
        )
        .await;
        let transaction = TransactionRef::from_bytes(bytes32(31));
        let record = prepared(transaction);
        assert_eq!(
            reduce_controlled_delta_write(
                &record,
                &operation_outcome,
                transaction,
                DiagnosticRef::from_bytes(bytes32(32)),
                owner(),
                result(),
                context(),
            ),
            Err(CommandDeltaBridgeError::ReceiptOperationMismatch {
                expected: command().identity.operation_id,
                observed: foreign_operation,
            })
        );

        let generation_fixture = fixture().await;
        let foreign_generation =
            WriterGeneration::new(fence().generation.get() + 1).expect("foreign generation");
        let (_, generation_input) = delta_session_and_input(2, true);
        let generation_outcome = write_exact_delta_plan(
            &generation_fixture.table,
            &delta_spec_with_binding(
                &generation_fixture.root,
                0,
                31,
                command().identity.operation_id,
                foreign_generation,
            ),
            generation_input,
        )
        .await;
        assert_eq!(
            reduce_controlled_delta_write(
                &record,
                &generation_outcome,
                transaction,
                DiagnosticRef::from_bytes(bytes32(33)),
                owner(),
                result(),
                context(),
            ),
            Err(CommandDeltaBridgeError::ReceiptWriterGenerationMismatch {
                expected: fence().generation,
                observed: foreign_generation,
            })
        );
        assert!(matches!(
            record.state(),
            DurableCommandState::CommitPrepared { .. }
        ));
    }

    #[tokio::test]
    async fn marker_already_committed_enters_reconciliation_instead_of_guessing_result() {
        let fixture = fixture().await;
        let (_, first_input) = delta_session_and_input(1, true);
        let first = write_exact_delta_plan(
            &fixture.table,
            &delta_spec(&fixture.root, 0, 2),
            first_input,
        )
        .await;
        let ControlledDeltaWriteOutcome::Committed(first) = first else {
            panic!("expected marker fixture commit, got {first:?}");
        };
        let table = first.into_table();
        let (_, repeated_input) = delta_session_and_input(2, true);
        let repeated =
            write_exact_delta_plan(&table, &delta_spec(&fixture.root, 1, 2), repeated_input).await;
        let transaction = TransactionRef::from_bytes(bytes32(22));
        let diagnostic = DiagnosticRef::from_bytes(bytes32(23));
        let receipt = bridge(&prepared(transaction), &repeated, transaction, diagnostic);

        assert!(matches!(
            receipt.classification(),
            DeltaWriteBridgeClassification::MarkerAlreadyCommitted(_)
        ));
        assert_eq!(
            receipt.retry_classification(),
            RetryClassification::ReconcileBeforeDecision
        );
        assert!(matches!(
            receipt.reduction().record.state(),
            DurableCommandState::AwaitingReconciliation {
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ReadbackUnavailable,
                    diagnostic: observed,
                },
                ..
            } if observed == diagnostic
        ));
    }

    #[tokio::test]
    async fn conflict_and_unknown_outcomes_both_require_durable_reconciliation() {
        let fixture = fixture().await;
        let (_, conflict_input) = delta_session_and_input(1, true);
        let conflict = write_exact_delta_plan(
            &fixture.table,
            &delta_spec(&fixture.root, 1, 3),
            conflict_input,
        )
        .await;
        let conflict_transaction = TransactionRef::from_bytes(bytes32(24));
        let conflict_receipt = bridge(
            &prepared(conflict_transaction),
            &conflict,
            conflict_transaction,
            DiagnosticRef::from_bytes(bytes32(25)),
        );
        assert!(matches!(
            conflict_receipt.classification(),
            DeltaWriteBridgeClassification::Conflict(
                DeltaWriteBridgeConflict::PredecessorMismatch { .. }
            )
        ));
        assert_eq!(
            conflict_receipt.event_kind(),
            CommandEventKind::ReportUnknownCommit
        );

        let (_, unknown_input) = delta_session_and_input(2, false);
        let unknown = write_exact_delta_plan(
            &fixture.table,
            &delta_spec(&fixture.root, 0, 4),
            unknown_input,
        )
        .await;
        let unknown_transaction = TransactionRef::from_bytes(bytes32(26));
        let unknown_receipt = bridge(
            &prepared(unknown_transaction),
            &unknown,
            unknown_transaction,
            DiagnosticRef::from_bytes(bytes32(27)),
        );
        assert!(matches!(
            unknown_receipt.classification(),
            DeltaWriteBridgeClassification::Unknown {
                stage: ControlledDeltaWriteUnknownStage::ExecuteWrite,
                ..
            }
        ));
        assert_eq!(
            unknown_receipt.reconciliation_classification(),
            ReconciliationClassification::OperationMarkerAndControlHistory
        );
        assert!(matches!(
            unknown_receipt.reduction().record.state(),
            DurableCommandState::AwaitingReconciliation { .. }
        ));
    }
}
