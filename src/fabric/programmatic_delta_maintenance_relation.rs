//! Exact Delta request and operation-history relation for guarded maintenance commands.
//!
//! One append-only Delta table carries immutable request rows and committed readback rows. The
//! selected request row names the exact history predecessor; its command can therefore attempt
//! only `predecessor + 1`, and restart reconciliation loads only that derived exact version. The
//! adapter never refreshes a table, resolves latest state, or inspects raw files.

use std::sync::{Arc, Mutex};

use arrow_array::{Array, ArrayRef, BinaryArray, Int64Array, RecordBatch, StringArray};
use arrow_cast::cast;
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use arrow_select::concat::concat_batches;
use async_trait::async_trait;
use datafusion::execution::SessionState;
use datafusion::execution::context::SessionContext;
use deltalake::table::config::TablePropertiesExt as _;
use deltalake::{DeltaTable, DeltaTableBuilder};
use thiserror::Error;

use super::administration_command_effect::{
    AdministrationCommitObservation, AdministrationCommitReceipt, AdministrationMarkerObservation,
    AdministrationMarkerReceipt, AdministrationReconciliationRequest,
};
use super::command::{
    AdministrationAction, AdministrationRequestRef, ExpectedHead, OperationId,
    OperationSelectionRef, ReconciliationEvidenceRef, TransactionRef, WorkspaceId, WriterFence,
    WriterGeneration,
};
use super::command_actor::CommandPortError;
use super::delta_exact::{
    DeltaRetainedResource, ExactDeltaPin, ValidatedDeltaSnapshot,
    provider_read_from_validated_snapshot,
};
use super::delta_guarded_maintenance::{
    DeltaMaintenanceOutcome, GuardedDeltaMaintenanceIntent, GuardedDeltaMaintenanceRequest,
};
use super::delta_write::{
    ApplicationTransactionMarker, ControlledDeltaHistoryProperties, ControlledDeltaWriteMode,
    ControlledDeltaWriteOutcome, ControlledDeltaWriteReconciliation, ControlledDeltaWriteSpec,
    SessionBoundLogicalPlan, write_exact_delta_plan,
};
use super::programmatic_delta_maintenance_command::{
    ProgrammaticDeltaMaintenanceAdministrationPorts, ProgrammaticDeltaMaintenanceCommandCommit,
    ProgrammaticDeltaMaintenanceCommandDiagnostics, ProgrammaticDeltaMaintenanceCommandSelection,
    ProgrammaticDeltaMaintenanceHistoryPort, ProgrammaticDeltaMaintenanceRequestPort,
    proposed_deletion_set_identity,
};

const ROW_REQUEST: &str = "request";
const ROW_PROPOSED_DELETION: &str = "proposed_deletion";
const ROW_COMMIT: &str = "commit";
const INTENT_INSPECT: &str = "inspect_retention";
const INTENT_VALIDATE: &str = "validate_retention";
const INTENT_VACUUM_DRY_RUN: &str = "vacuum_dry_run";
const OUTCOME_INSPECTED: &str = "retention_inspected";
const OUTCOME_VALIDATED: &str = "retention_validated";
const OUTCOME_VACUUM_DRY_RUN: &str = "vacuum_dry_run";

const RESOURCE_DELTA_VERSION: &str = "delta_version";
const RESOURCE_IMMUTABLE_SEGMENT: &str = "immutable_segment";
const RESOURCE_PROGRAM_RELEASE: &str = "program_release";
const RESOURCE_EXPECTATION: &str = "expectation";
const RESOURCE_QUERY_RESULT: &str = "query_result";
const RESOURCE_ROLLBACK_POINT: &str = "rollback_point";

/// Exact loaded state retained by the relation adapter.
#[derive(Clone)]
struct RelationState {
    pin: ExactDeltaPin,
    table: DeltaTable,
}

/// Concrete Delta-backed request resolver and durable operation-history port.
pub struct DeltaProgrammaticMaintenanceCommandRelation {
    session: Arc<SessionState>,
    state: Mutex<RelationState>,
}

impl std::fmt::Debug for DeltaProgrammaticMaintenanceCommandRelation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pin = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pin
            .clone();
        formatter
            .debug_struct("DeltaProgrammaticMaintenanceCommandRelation")
            .field("pin", &pin)
            .field("session_id", &self.session.session_id())
            .finish_non_exhaustive()
    }
}

impl DeltaProgrammaticMaintenanceCommandRelation {
    /// Bind one exact, already-loaded append-only operation relation to the epoch session.
    pub fn try_from_loaded_table(
        session: Arc<SessionState>,
        pin: ExactDeltaPin,
        table: DeltaTable,
    ) -> Result<Self, DeltaProgrammaticMaintenanceRelationError> {
        ValidatedDeltaSnapshot::try_from_loaded_table(table.clone(), &pin)
            .map_err(|error| DeltaProgrammaticMaintenanceRelationError::Exact(error.to_string()))?;
        let snapshot = table
            .snapshot()
            .map_err(|error| DeltaProgrammaticMaintenanceRelationError::Delta(error.to_string()))?;
        let actual_schema = snapshot.snapshot().arrow_schema();
        let expected_schema = relation_schema();
        if actual_schema.as_ref() != expected_schema.as_ref() {
            return Err(DeltaProgrammaticMaintenanceRelationError::SchemaMismatch {
                expected: expected_schema,
                actual: actual_schema,
            });
        }
        let table_config = snapshot.table_config();
        if !table_config.append_only() {
            return Err(DeltaProgrammaticMaintenanceRelationError::AppendOnlyRequired);
        }
        if !table_config.enable_change_data_feed() {
            return Err(DeltaProgrammaticMaintenanceRelationError::ChangeDataFeedRequired);
        }
        Ok(Self {
            session,
            state: Mutex::new(RelationState { pin, table }),
        })
    }

    /// Exact Delta storage schema for the append-only maintenance request/history relation.
    #[must_use]
    pub fn schema() -> SchemaRef {
        relation_schema()
    }

    /// Mandatory creation-time Delta properties for this durable control history.
    #[must_use]
    pub fn creation_properties() -> ControlledDeltaHistoryProperties {
        ControlledDeltaHistoryProperties::try_new(
            "row_kind,workspace_id,operation_id,request,proposed_deletion_set_digest,proposed_deletion_ordinal,transaction",
        )
        .expect("maintenance history statistics selection is nonempty")
    }

    /// Select this one concrete Delta relation as both request and operation-history authority.
    #[must_use]
    pub fn administration_ports(
        self: &Arc<Self>,
        diagnostics: ProgrammaticDeltaMaintenanceCommandDiagnostics,
    ) -> ProgrammaticDeltaMaintenanceAdministrationPorts {
        let requests: Arc<dyn ProgrammaticDeltaMaintenanceRequestPort> = self.clone();
        let history: Arc<dyn ProgrammaticDeltaMaintenanceHistoryPort> = self.clone();
        ProgrammaticDeltaMaintenanceAdministrationPorts::new(requests, history, diagnostics)
    }

    /// Encode one immutable request parent and any ordered child rows for a controlled append.
    ///
    /// The selection's history predecessor must equal the Delta version created by the request
    /// append. This makes the atomic request row set itself the only legal predecessor of its
    /// result row. `ValidateRetention` stores every proposed resource reversibly; its parent count
    /// and digest bind the exact child relation without an opaque JSON/list payload.
    pub fn encode_request(
        selection: &ProgrammaticDeltaMaintenanceCommandSelection,
    ) -> Result<RecordBatch, DeltaProgrammaticMaintenanceRelationError> {
        let encoded_intent = encode_intent(selection.maintenance().intent())?;
        let mut batches = vec![encode_row(&EncodedRow {
            row_kind: ROW_REQUEST,
            workspace_id: selection.workspace_id(),
            operation_id: selection.maintenance().operation_id(),
            request: selection.request(),
            action: selection.action(),
            relation_id: selection.relation_id(),
            target: selection.maintenance().target(),
            activation_head: selection.maintenance().expected_activation_head(),
            writer_fence: selection.maintenance().writer_fence(),
            intent: encoded_intent.code,
            retention_seconds: encoded_intent.retention_seconds,
            proposed_deletion_count: encoded_intent.proposed_deletion_count,
            proposed_deletion_set_digest: encoded_intent.proposed_deletion_set_digest,
            proposed_deletion_ordinal: None,
            proposed_deletion: None,
            operation_selection: selection.operation_selection(),
            history_predecessor: selection.history_predecessor(),
            transaction: None,
            expected_head: None,
            outcome: None,
            evidence_revision: None,
            vacuum_candidate_digest: None,
        })?];
        if let GuardedDeltaMaintenanceIntent::ValidateRetention { proposed_deletions } =
            selection.maintenance().intent()
        {
            for (ordinal, proposed_deletion) in proposed_deletions.iter().enumerate() {
                batches.push(encode_row(&EncodedRow {
                    row_kind: ROW_PROPOSED_DELETION,
                    workspace_id: selection.workspace_id(),
                    operation_id: selection.maintenance().operation_id(),
                    request: selection.request(),
                    action: selection.action(),
                    relation_id: selection.relation_id(),
                    target: selection.maintenance().target(),
                    activation_head: selection.maintenance().expected_activation_head(),
                    writer_fence: selection.maintenance().writer_fence(),
                    intent: encoded_intent.code,
                    retention_seconds: None,
                    proposed_deletion_count: encoded_intent.proposed_deletion_count,
                    proposed_deletion_set_digest: encoded_intent.proposed_deletion_set_digest,
                    proposed_deletion_ordinal: Some(u64::try_from(ordinal).map_err(|_| {
                        DeltaProgrammaticMaintenanceRelationError::CardinalityOverflow(
                            "proposed_deletion_ordinal",
                        )
                    })?),
                    proposed_deletion: Some(proposed_deletion),
                    operation_selection: selection.operation_selection(),
                    history_predecessor: selection.history_predecessor(),
                    transaction: None,
                    expected_head: None,
                    outcome: None,
                    evidence_revision: None,
                    vacuum_candidate_digest: None,
                })?);
            }
        }
        concat_batches(&relation_schema(), batches.iter())
            .map_err(|error| DeltaProgrammaticMaintenanceRelationError::Arrow(error.to_string()))
    }

    fn snapshot(&self) -> RelationState {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    async fn rows_at(
        &self,
        state: &RelationState,
    ) -> Result<Vec<DecodedRow>, DeltaProgrammaticMaintenanceRelationError> {
        let snapshot =
            ValidatedDeltaSnapshot::try_from_loaded_table(state.table.clone(), &state.pin)
                .map_err(|error| {
                    DeltaProgrammaticMaintenanceRelationError::Exact(error.to_string())
                })?;
        let read =
            provider_read_from_validated_snapshot(&state.pin, snapshot, Arc::clone(&self.session))
                .await
                .map_err(|error| {
                    DeltaProgrammaticMaintenanceRelationError::Exact(error.to_string())
                })?;
        let (provider, _) = read.into_parts();
        let context = SessionContext::new_with_state(self.session.as_ref().clone());
        let batches = context
            .read_table(provider)
            .map_err(|error| {
                DeltaProgrammaticMaintenanceRelationError::DataFusion(error.to_string())
            })?
            .collect()
            .await
            .map_err(|error| {
                DeltaProgrammaticMaintenanceRelationError::DataFusion(error.to_string())
            })?;
        let normalized = batches
            .iter()
            .map(normalize_provider_batch)
            .collect::<Result<Vec<_>, _>>()?;
        decode_batches(&normalized)
    }

    async fn resolve_at(
        &self,
        state: &RelationState,
        action: AdministrationAction,
        request: AdministrationRequestRef,
        require_current_predecessor: bool,
    ) -> Result<
        Option<ProgrammaticDeltaMaintenanceCommandSelection>,
        DeltaProgrammaticMaintenanceRelationError,
    > {
        let rows = self.rows_at(state).await?;
        let mut matching = rows
            .iter()
            .filter(|row| {
                row.row_kind == ROW_REQUEST && row.action == action && row.request == request
            })
            .cloned()
            .collect::<Vec<_>>();
        if matching.len() > 1 {
            return Err(DeltaProgrammaticMaintenanceRelationError::DuplicateRequest);
        }
        let proposed_deletion_rows = rows
            .into_iter()
            .filter(|row| row.row_kind == ROW_PROPOSED_DELETION && row.request == request)
            .collect::<Vec<_>>();
        let Some(row) = matching.pop() else {
            if !proposed_deletion_rows.is_empty() {
                return Err(
                    DeltaProgrammaticMaintenanceRelationError::ProposedDeletionParentBindingMismatch,
                );
            }
            return Ok(None);
        };
        let selection = row.into_selection(proposed_deletion_rows)?;
        if require_current_predecessor && selection.history_predecessor() != &state.pin {
            return Ok(None);
        }
        Ok(Some(selection))
    }

    async fn commit_row(
        &self,
        commit: &ProgrammaticDeltaMaintenanceCommandCommit,
        predecessor: &RelationState,
    ) -> Result<ControlledDeltaWriteOutcome, DeltaProgrammaticMaintenanceRelationError> {
        let command = commit.attempt().command();
        let selection = commit.selection();
        let (outcome, evidence_revision, vacuum_candidate_digest) =
            match (selection.maintenance().intent(), commit.outcome()) {
                (
                    GuardedDeltaMaintenanceIntent::InspectRetention,
                    DeltaMaintenanceOutcome::RetentionInspected {
                        evidence_revision, ..
                    },
                ) => (OUTCOME_INSPECTED, Some(evidence_revision.get()), None),
                (
                    GuardedDeltaMaintenanceIntent::ValidateRetention { .. },
                    DeltaMaintenanceOutcome::RetentionValidated { evidence_revision },
                ) => (OUTCOME_VALIDATED, Some(evidence_revision.get()), None),
                (
                    GuardedDeltaMaintenanceIntent::VacuumDryRun { .. },
                    DeltaMaintenanceOutcome::VacuumDryRun(receipt),
                ) => (
                    OUTCOME_VACUUM_DRY_RUN,
                    Some(receipt.evidence_revision().get()),
                    Some(*receipt.candidate_digest()),
                ),
                _ => return Err(DeltaProgrammaticMaintenanceRelationError::UnsupportedOutcome),
            };
        let encoded_intent = encode_intent(selection.maintenance().intent())?;
        let batch = encode_row(&EncodedRow {
            row_kind: ROW_COMMIT,
            workspace_id: selection.workspace_id(),
            operation_id: command.identity.operation_id,
            request: selection.request(),
            action: selection.action(),
            relation_id: selection.relation_id(),
            target: selection.maintenance().target(),
            activation_head: selection.maintenance().expected_activation_head(),
            writer_fence: selection.maintenance().writer_fence(),
            intent: encoded_intent.code,
            retention_seconds: encoded_intent.retention_seconds,
            proposed_deletion_count: encoded_intent.proposed_deletion_count,
            proposed_deletion_set_digest: encoded_intent.proposed_deletion_set_digest,
            proposed_deletion_ordinal: None,
            proposed_deletion: None,
            operation_selection: selection.operation_selection(),
            history_predecessor: selection.history_predecessor(),
            transaction: Some(commit.transaction()),
            expected_head: Some(command.expected_head),
            outcome: Some(outcome),
            evidence_revision,
            vacuum_candidate_digest,
        })?;
        let context = SessionContext::new_with_state(self.session.as_ref().clone());
        let dataframe = context.read_batch(batch).map_err(|error| {
            DeltaProgrammaticMaintenanceRelationError::DataFusion(error.to_string())
        })?;
        let plan =
            SessionBoundLogicalPlan::try_from_dataframe(Arc::clone(&self.session), dataframe)
                .map_err(|error| {
                    DeltaProgrammaticMaintenanceRelationError::DataFusion(error.to_string())
                })?;
        let spec = ControlledDeltaWriteSpec::new(
            predecessor.pin.clone(),
            command.identity.operation_id,
            commit.attempt().execution_owner().fence.generation,
            ApplicationTransactionMarker::from_transaction_ref(commit.transaction()),
            ControlledDeltaWriteMode::Append,
        );
        Ok(write_exact_delta_plan(&predecessor.table, &spec, plan).await)
    }

    async fn load_exact_successor(
        &self,
        predecessor: &ExactDeltaPin,
    ) -> Result<RelationState, DeltaProgrammaticMaintenanceRelationError> {
        let version = predecessor
            .version()
            .checked_add(1)
            .ok_or(DeltaProgrammaticMaintenanceRelationError::VersionOverflow)?;
        let pin = ExactDeltaPin::new(predecessor.canonical_root(), version)
            .map_err(|error| DeltaProgrammaticMaintenanceRelationError::Exact(error.to_string()))?;
        let table = DeltaTableBuilder::from_url(pin.canonical_root().clone())
            .map_err(|error| DeltaProgrammaticMaintenanceRelationError::Delta(error.to_string()))?
            .with_version(pin.version())
            .load()
            .await
            .map_err(|error| DeltaProgrammaticMaintenanceRelationError::Delta(error.to_string()))?;
        ValidatedDeltaSnapshot::try_from_loaded_table(table.clone(), &pin)
            .map_err(|error| DeltaProgrammaticMaintenanceRelationError::Exact(error.to_string()))?;
        Ok(RelationState { pin, table })
    }
}

#[async_trait]
impl ProgrammaticDeltaMaintenanceRequestPort for DeltaProgrammaticMaintenanceCommandRelation {
    async fn resolve(
        &self,
        action: AdministrationAction,
        request: AdministrationRequestRef,
    ) -> Result<Option<ProgrammaticDeltaMaintenanceCommandSelection>, CommandPortError> {
        let state = self.snapshot();
        self.resolve_at(&state, action, request, true)
            .await
            .map_err(|_| CommandPortError::ContextUnavailable)
    }
}

#[async_trait]
impl ProgrammaticDeltaMaintenanceHistoryPort for DeltaProgrammaticMaintenanceCommandRelation {
    async fn commit_readback(
        &self,
        commit: ProgrammaticDeltaMaintenanceCommandCommit,
    ) -> Result<AdministrationCommitObservation, CommandPortError> {
        let predecessor = self.snapshot();
        if commit.selection().history_predecessor() != &predecessor.pin {
            return Ok(AdministrationCommitObservation::Conflict {
                diagnostic: diagnostic(&commit, b"history-predecessor-mismatch"),
            });
        }
        let outcome = self
            .commit_row(&commit, &predecessor)
            .await
            .map_err(|_| CommandPortError::EffectUnavailable)?;
        match outcome {
            ControlledDeltaWriteOutcome::Committed(committed) => {
                let committed_state = RelationState {
                    pin: committed.committed().clone(),
                    table: committed.into_table(),
                };
                let receipt = find_commit_receipt(
                    self.rows_at(&committed_state)
                        .await
                        .map_err(|_| CommandPortError::EffectUnavailable)?,
                    commit.selection(),
                    commit.attempt().command().identity.operation_id,
                    commit.transaction(),
                )
                .map_err(|_| CommandPortError::CorruptRecord)?
                .ok_or(CommandPortError::CorruptRecord)?;
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if state.pin != predecessor.pin {
                    return Err(CommandPortError::CorruptRecord);
                }
                *state = committed_state;
                Ok(AdministrationCommitObservation::Committed(receipt))
            }
            ControlledDeltaWriteOutcome::MarkerAlreadyCommitted(_) => {
                Ok(AdministrationCommitObservation::MarkerAlreadyCommitted {
                    marker: AdministrationMarkerReceipt {
                        workspace_id: commit.selection().workspace_id(),
                        operation_id: commit.attempt().command().identity.operation_id,
                        transaction: commit.transaction(),
                        writer_generation: commit.attempt().execution_owner().fence.generation,
                    },
                    diagnostic: diagnostic(&commit, b"marker-already-visible"),
                })
            }
            ControlledDeltaWriteOutcome::Reconcile(
                ControlledDeltaWriteReconciliation::Conflict(_),
            ) => Ok(AdministrationCommitObservation::Conflict {
                diagnostic: diagnostic(&commit, b"zero-retry-conflict"),
            }),
            ControlledDeltaWriteOutcome::Reconcile(
                ControlledDeltaWriteReconciliation::Unknown(_),
            ) => Ok(AdministrationCommitObservation::Unknown {
                reason: super::command::UnknownCommitReason::ReadbackUnavailable,
                diagnostic: diagnostic(&commit, b"commit-readback-unknown"),
            }),
        }
    }

    async fn read_exact(
        &self,
        request: AdministrationReconciliationRequest,
    ) -> Result<AdministrationMarkerObservation, CommandPortError> {
        let current = self.snapshot();
        let attempt = request.attempt();
        let selection = self
            .resolve_at(&current, attempt.action(), attempt.request(), false)
            .await
            .map_err(|_| CommandPortError::ContextUnavailable)?
            .ok_or(CommandPortError::CorruptRecord)?;
        let successor = match self
            .load_exact_successor(selection.history_predecessor())
            .await
        {
            Ok(successor) => successor,
            Err(_) => {
                return Ok(AdministrationMarkerObservation::Indeterminate {
                    evidence: reconciliation_evidence(request, b"successor-unavailable"),
                });
            }
        };
        let receipt = find_commit_receipt(
            self.rows_at(&successor)
                .await
                .map_err(|_| CommandPortError::ContextUnavailable)?,
            &selection,
            attempt.command().identity.operation_id,
            request.transaction(),
        )
        .map_err(|_| CommandPortError::CorruptRecord)?;
        Ok(receipt.map_or_else(
            || AdministrationMarkerObservation::ProvedNotCommitted {
                evidence: reconciliation_evidence(request, b"exact-successor-without-row"),
            },
            |receipt| AdministrationMarkerObservation::Committed {
                receipt,
                evidence: reconciliation_evidence(request, b"exact-successor-readback"),
            },
        ))
    }
}

/// Concrete relation failures never collapse into observed-empty authority.
#[derive(Debug, Error)]
pub enum DeltaProgrammaticMaintenanceRelationError {
    #[error("maintenance request/history Delta schema differs from the exact relation contract")]
    SchemaMismatch {
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("maintenance request/history Delta relation must be append-only")]
    AppendOnlyRequired,
    #[error("maintenance request/history Delta relation must enable CDF at creation")]
    ChangeDataFeedRequired,
    #[error("maintenance request relation contains duplicate request identity")]
    DuplicateRequest,
    #[error("maintenance history contains duplicate commit identity")]
    DuplicateCommit,
    #[error(
        "maintenance relation supports only inspect-retention, validate-retention, and vacuum-dry-run intents"
    )]
    UnsupportedIntent,
    #[error("maintenance relation received an unsupported committed outcome")]
    UnsupportedOutcome,
    #[error("maintenance history version overflowed")]
    VersionOverflow,
    #[error("maintenance child-relation cardinality overflowed: {0}")]
    CardinalityOverflow(&'static str),
    #[error(
        "maintenance proposed-deletion child count differs: parent declared {expected}, observed {actual}"
    )]
    ProposedDeletionCountMismatch { expected: u64, actual: usize },
    #[error(
        "maintenance proposed-deletion child ordinal differs: expected {expected}, observed {actual}"
    )]
    ProposedDeletionOrdinalMismatch { expected: u64, actual: u64 },
    #[error("maintenance proposed-deletion child relation does not bind its parent request")]
    ProposedDeletionParentBindingMismatch,
    #[error("maintenance proposed-deletion child relation digest differs from its parent")]
    ProposedDeletionDigestMismatch,
    #[error("maintenance commit row does not bind the resolved request: {0}")]
    CommitBindingMismatch(&'static str),
    #[error("maintenance relation unsigned integer cannot be stored as Delta INT: {0}")]
    UnsignedIntegerOutOfRange(&'static str),
    #[error("maintenance relation Delta INT is negative: {0}")]
    NegativeInteger(&'static str),
    #[error("exact Delta maintenance relation failed: {0}")]
    Exact(String),
    #[error("Delta maintenance relation failed: {0}")]
    Delta(String),
    #[error("DataFusion maintenance relation failed: {0}")]
    DataFusion(String),
    #[error("Arrow maintenance relation failed: {0}")]
    Arrow(String),
    #[error("maintenance relation row is malformed: {0}")]
    Malformed(&'static str),
}

struct EncodedRow<'a> {
    row_kind: &'a str,
    workspace_id: WorkspaceId,
    operation_id: OperationId,
    request: AdministrationRequestRef,
    action: AdministrationAction,
    relation_id: &'a str,
    target: &'a ExactDeltaPin,
    activation_head: &'a [u8; 32],
    writer_fence: WriterFence,
    intent: &'a str,
    retention_seconds: Option<u64>,
    proposed_deletion_count: Option<u64>,
    proposed_deletion_set_digest: Option<[u8; 32]>,
    proposed_deletion_ordinal: Option<u64>,
    proposed_deletion: Option<&'a DeltaRetainedResource>,
    operation_selection: OperationSelectionRef,
    history_predecessor: &'a ExactDeltaPin,
    transaction: Option<TransactionRef>,
    expected_head: Option<ExpectedHead>,
    outcome: Option<&'a str>,
    evidence_revision: Option<u64>,
    vacuum_candidate_digest: Option<[u8; 32]>,
}

#[derive(Clone)]
struct DecodedRow {
    row_kind: String,
    workspace_id: WorkspaceId,
    operation_id: OperationId,
    request: AdministrationRequestRef,
    action: AdministrationAction,
    relation_id: String,
    target: ExactDeltaPin,
    activation_head: [u8; 32],
    writer_fence: WriterFence,
    intent: String,
    retention_seconds: Option<u64>,
    proposed_deletion_count: Option<u64>,
    proposed_deletion_set_digest: Option<[u8; 32]>,
    proposed_deletion_ordinal: Option<u64>,
    proposed_deletion_resource_kind: Option<String>,
    proposed_deletion_delta_root: Option<String>,
    proposed_deletion_delta_version: Option<u64>,
    proposed_deletion_resource_id: Option<[u8; 32]>,
    operation_selection: OperationSelectionRef,
    history_predecessor: ExactDeltaPin,
    transaction: Option<TransactionRef>,
    expected_head: Option<ExpectedHead>,
    outcome: Option<String>,
    evidence_revision: Option<u64>,
    vacuum_candidate_digest: Option<[u8; 32]>,
}

impl DecodedRow {
    fn into_selection(
        self,
        mut proposed_deletion_rows: Vec<Self>,
    ) -> Result<
        ProgrammaticDeltaMaintenanceCommandSelection,
        DeltaProgrammaticMaintenanceRelationError,
    > {
        if self.row_kind != ROW_REQUEST
            || self.transaction.is_some()
            || self.expected_head.is_some()
            || self.outcome.is_some()
            || self.evidence_revision.is_some()
            || self.vacuum_candidate_digest.is_some()
            || self.proposed_deletion_ordinal.is_some()
            || self.proposed_deletion_resource_kind.is_some()
            || self.proposed_deletion_delta_root.is_some()
            || self.proposed_deletion_delta_version.is_some()
            || self.proposed_deletion_resource_id.is_some()
        {
            return Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
                "request_row_shape",
            ));
        }
        let intent = match self.intent.as_str() {
            INTENT_INSPECT => {
                require_no_proposed_deletion_parent(&self, &proposed_deletion_rows)?;
                if self.retention_seconds.is_some() {
                    return Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
                        "retention_seconds",
                    ));
                }
                GuardedDeltaMaintenanceIntent::InspectRetention
            }
            INTENT_VALIDATE => {
                if self.retention_seconds.is_some() {
                    return Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
                        "retention_seconds",
                    ));
                }
                let expected_count = self.proposed_deletion_count.ok_or(
                    DeltaProgrammaticMaintenanceRelationError::Malformed("proposed_deletion_count"),
                )?;
                let expected_digest = self.proposed_deletion_set_digest.ok_or(
                    DeltaProgrammaticMaintenanceRelationError::Malformed(
                        "proposed_deletion_set_digest",
                    ),
                )?;
                let actual_count = proposed_deletion_rows.len();
                if usize::try_from(expected_count).ok() != Some(actual_count) {
                    return Err(
                        DeltaProgrammaticMaintenanceRelationError::ProposedDeletionCountMismatch {
                            expected: expected_count,
                            actual: actual_count,
                        },
                    );
                }
                proposed_deletion_rows.sort_by_key(|row| row.proposed_deletion_ordinal);
                let mut proposed_deletions = Vec::with_capacity(actual_count);
                for (expected_ordinal, row) in proposed_deletion_rows.iter().enumerate() {
                    if !row.binds_proposed_deletion_parent(&self) {
                        return Err(DeltaProgrammaticMaintenanceRelationError::
                            ProposedDeletionParentBindingMismatch);
                    }
                    let actual_ordinal = row.proposed_deletion_ordinal.ok_or(
                        DeltaProgrammaticMaintenanceRelationError::Malformed(
                            "proposed_deletion_ordinal",
                        ),
                    )?;
                    let expected_ordinal = u64::try_from(expected_ordinal).map_err(|_| {
                        DeltaProgrammaticMaintenanceRelationError::CardinalityOverflow(
                            "proposed_deletion_ordinal",
                        )
                    })?;
                    if actual_ordinal != expected_ordinal {
                        return Err(DeltaProgrammaticMaintenanceRelationError::
                            ProposedDeletionOrdinalMismatch {
                                expected: expected_ordinal,
                                actual: actual_ordinal,
                            });
                    }
                    proposed_deletions.push(row.decode_proposed_deletion()?);
                }
                if proposed_deletion_set_identity(&proposed_deletions) != expected_digest {
                    return Err(
                        DeltaProgrammaticMaintenanceRelationError::ProposedDeletionDigestMismatch,
                    );
                }
                GuardedDeltaMaintenanceIntent::ValidateRetention {
                    proposed_deletions: proposed_deletions.into(),
                }
            }
            INTENT_VACUUM_DRY_RUN => GuardedDeltaMaintenanceIntent::VacuumDryRun {
                expected_retention_seconds: {
                    require_no_proposed_deletion_parent(&self, &proposed_deletion_rows)?;
                    self.retention_seconds.ok_or(
                        DeltaProgrammaticMaintenanceRelationError::Malformed("retention_seconds"),
                    )?
                },
            },
            _ => return Err(DeltaProgrammaticMaintenanceRelationError::UnsupportedIntent),
        };
        let maintenance = GuardedDeltaMaintenanceRequest::try_new(
            self.target,
            self.activation_head,
            self.writer_fence,
            self.operation_id,
            intent,
        )
        .map_err(|_| DeltaProgrammaticMaintenanceRelationError::Malformed("maintenance_request"))?;
        ProgrammaticDeltaMaintenanceCommandSelection::try_new(
            self.action,
            self.request,
            self.workspace_id,
            self.relation_id,
            maintenance,
            self.history_predecessor,
            self.operation_selection,
        )
        .map_err(|_| DeltaProgrammaticMaintenanceRelationError::Malformed("selection"))
    }

    fn binds_proposed_deletion_parent(&self, parent: &Self) -> bool {
        self.row_kind == ROW_PROPOSED_DELETION
            && self.workspace_id == parent.workspace_id
            && self.operation_id == parent.operation_id
            && self.request == parent.request
            && self.action == parent.action
            && self.relation_id == parent.relation_id
            && self.target == parent.target
            && self.activation_head == parent.activation_head
            && self.writer_fence == parent.writer_fence
            && self.intent == parent.intent
            && self.retention_seconds.is_none()
            && self.proposed_deletion_count == parent.proposed_deletion_count
            && self.proposed_deletion_set_digest == parent.proposed_deletion_set_digest
            && self.operation_selection == parent.operation_selection
            && self.history_predecessor == parent.history_predecessor
            && self.transaction.is_none()
            && self.expected_head.is_none()
            && self.outcome.is_none()
            && self.evidence_revision.is_none()
            && self.vacuum_candidate_digest.is_none()
    }

    fn decode_proposed_deletion(
        &self,
    ) -> Result<DeltaRetainedResource, DeltaProgrammaticMaintenanceRelationError> {
        let kind = self.proposed_deletion_resource_kind.as_deref().ok_or(
            DeltaProgrammaticMaintenanceRelationError::Malformed("proposed_deletion_resource_kind"),
        )?;
        match kind {
            RESOURCE_DELTA_VERSION => {
                if self.proposed_deletion_resource_id.is_some() {
                    return Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
                        "proposed_deletion_resource_shape",
                    ));
                }
                let root = self.proposed_deletion_delta_root.as_deref().ok_or(
                    DeltaProgrammaticMaintenanceRelationError::Malformed(
                        "proposed_deletion_delta_root",
                    ),
                )?;
                let version = self.proposed_deletion_delta_version.ok_or(
                    DeltaProgrammaticMaintenanceRelationError::Malformed(
                        "proposed_deletion_delta_version",
                    ),
                )?;
                let root = url::Url::parse(root).map_err(|_| {
                    DeltaProgrammaticMaintenanceRelationError::Malformed(
                        "proposed_deletion_delta_root",
                    )
                })?;
                ExactDeltaPin::new(&root, version)
                    .map(DeltaRetainedResource::DeltaVersion)
                    .map_err(|_| {
                        DeltaProgrammaticMaintenanceRelationError::Malformed(
                            "proposed_deletion_delta_root",
                        )
                    })
            }
            RESOURCE_IMMUTABLE_SEGMENT
            | RESOURCE_PROGRAM_RELEASE
            | RESOURCE_EXPECTATION
            | RESOURCE_QUERY_RESULT
            | RESOURCE_ROLLBACK_POINT => {
                if self.proposed_deletion_delta_root.is_some()
                    || self.proposed_deletion_delta_version.is_some()
                {
                    return Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
                        "proposed_deletion_resource_shape",
                    ));
                }
                let identity = self.proposed_deletion_resource_id.ok_or(
                    DeltaProgrammaticMaintenanceRelationError::Malformed(
                        "proposed_deletion_resource_id",
                    ),
                )?;
                if identity.iter().all(|byte| *byte == 0) {
                    return Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
                        "proposed_deletion_resource_id",
                    ));
                }
                Ok(match kind {
                    RESOURCE_IMMUTABLE_SEGMENT => DeltaRetainedResource::ImmutableSegment(identity),
                    RESOURCE_PROGRAM_RELEASE => DeltaRetainedResource::ProgramRelease(identity),
                    RESOURCE_EXPECTATION => DeltaRetainedResource::Expectation(identity),
                    RESOURCE_QUERY_RESULT => DeltaRetainedResource::QueryResult(identity),
                    RESOURCE_ROLLBACK_POINT => DeltaRetainedResource::RollbackPoint(identity),
                    _ => unreachable!("closed proposed-deletion resource kind"),
                })
            }
            _ => Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
                "proposed_deletion_resource_kind",
            )),
        }
    }
}

fn require_no_proposed_deletion_parent(
    parent: &DecodedRow,
    children: &[DecodedRow],
) -> Result<(), DeltaProgrammaticMaintenanceRelationError> {
    if parent.proposed_deletion_count.is_some()
        || parent.proposed_deletion_set_digest.is_some()
        || !children.is_empty()
    {
        return Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
            "unexpected_proposed_deletion_relation",
        ));
    }
    Ok(())
}

fn relation_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("row_kind", DataType::Utf8, false),
        Field::new("workspace_id", DataType::Binary, false),
        Field::new("operation_id", DataType::Binary, false),
        Field::new("request", DataType::Binary, false),
        Field::new("action", DataType::Utf8, false),
        Field::new("relation_id", DataType::Utf8, false),
        Field::new("target_root", DataType::Utf8, false),
        Field::new("target_version", DataType::Int64, false),
        Field::new("activation_head", DataType::Binary, false),
        Field::new("writer_lease", DataType::Binary, false),
        Field::new("writer_generation", DataType::Int64, false),
        Field::new("intent", DataType::Utf8, false),
        Field::new("retention_seconds", DataType::Int64, true),
        Field::new("proposed_deletion_count", DataType::Int64, true),
        Field::new("proposed_deletion_set_digest", DataType::Binary, true),
        Field::new("proposed_deletion_ordinal", DataType::Int64, true),
        Field::new("proposed_deletion_resource_kind", DataType::Utf8, true),
        Field::new("proposed_deletion_delta_root", DataType::Utf8, true),
        Field::new("proposed_deletion_delta_version", DataType::Int64, true),
        Field::new("proposed_deletion_resource_id", DataType::Binary, true),
        Field::new("operation_selection", DataType::Binary, false),
        Field::new("history_predecessor_root", DataType::Utf8, false),
        Field::new("history_predecessor_version", DataType::Int64, false),
        Field::new("transaction", DataType::Binary, true),
        Field::new("expected_head_kind", DataType::Utf8, true),
        Field::new("expected_epoch", DataType::Binary, true),
        Field::new("outcome", DataType::Utf8, true),
        Field::new("evidence_revision", DataType::Int64, true),
        Field::new("vacuum_candidate_digest", DataType::Binary, true),
    ]))
}

fn relation_provider_schema() -> SchemaRef {
    Arc::new(Schema::new(
        relation_schema()
            .fields()
            .iter()
            .map(|field| {
                let data_type = match field.data_type() {
                    DataType::Utf8 => DataType::Utf8View,
                    DataType::Binary => DataType::BinaryView,
                    data_type => data_type.clone(),
                };
                Field::new(field.name(), data_type, field.is_nullable())
            })
            .collect::<Vec<_>>(),
    ))
}

fn normalize_provider_batch(
    batch: &RecordBatch,
) -> Result<RecordBatch, DeltaProgrammaticMaintenanceRelationError> {
    let expected_provider = relation_provider_schema();
    if batch.schema_ref().as_ref() != expected_provider.as_ref() {
        return Err(DeltaProgrammaticMaintenanceRelationError::SchemaMismatch {
            expected: expected_provider,
            actual: batch.schema(),
        });
    }
    let storage = relation_schema();
    let columns = batch
        .columns()
        .iter()
        .zip(storage.fields())
        .map(|(column, field)| {
            cast(column, field.data_type()).map_err(|error| {
                DeltaProgrammaticMaintenanceRelationError::Arrow(error.to_string())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    RecordBatch::try_new(storage, columns)
        .map_err(|error| DeltaProgrammaticMaintenanceRelationError::Arrow(error.to_string()))
}

fn encode_row(
    row: &EncodedRow<'_>,
) -> Result<RecordBatch, DeltaProgrammaticMaintenanceRelationError> {
    let (head_kind, epoch) = match row.expected_head {
        Some(ExpectedHead::Empty) => (Some("empty"), None),
        Some(ExpectedHead::Epoch(epoch)) => (Some("epoch"), Some(*epoch.as_bytes())),
        None => (None, None),
    };
    let (resource_kind, delta_root, delta_version, resource_id) =
        encode_proposed_deletion_resource(row.proposed_deletion);
    RecordBatch::try_new(
        relation_schema(),
        vec![
            Arc::new(StringArray::from(vec![row.row_kind])) as ArrayRef,
            fixed_array::<16>(Some(*row.workspace_id.as_bytes()))?,
            fixed_array::<16>(Some(*row.operation_id.as_bytes()))?,
            fixed_array::<32>(Some(*row.request.as_bytes()))?,
            Arc::new(StringArray::from(vec![action_code(row.action)])),
            Arc::new(StringArray::from(vec![row.relation_id])),
            Arc::new(StringArray::from(vec![
                row.target.canonical_root().as_str(),
            ])),
            Arc::new(Int64Array::from(vec![delta_int(
                row.target.version(),
                "target_version",
            )?])),
            fixed_array::<32>(Some(*row.activation_head))?,
            fixed_array::<16>(Some(*row.writer_fence.lease_id.as_bytes()))?,
            Arc::new(Int64Array::from(vec![delta_int(
                row.writer_fence.generation.get(),
                "writer_generation",
            )?])),
            Arc::new(StringArray::from(vec![row.intent])),
            Arc::new(Int64Array::from(vec![
                row.retention_seconds
                    .map(|value| delta_int(value, "retention_seconds"))
                    .transpose()?,
            ])),
            Arc::new(Int64Array::from(vec![
                row.proposed_deletion_count
                    .map(|value| delta_int(value, "proposed_deletion_count"))
                    .transpose()?,
            ])),
            fixed_array::<32>(row.proposed_deletion_set_digest)?,
            Arc::new(Int64Array::from(vec![
                row.proposed_deletion_ordinal
                    .map(|value| delta_int(value, "proposed_deletion_ordinal"))
                    .transpose()?,
            ])),
            Arc::new(StringArray::from(vec![resource_kind])),
            Arc::new(StringArray::from(vec![delta_root])),
            Arc::new(Int64Array::from(vec![
                delta_version
                    .map(|value| delta_int(value, "proposed_deletion_delta_version"))
                    .transpose()?,
            ])),
            fixed_array::<32>(resource_id)?,
            fixed_array::<32>(Some(*row.operation_selection.as_bytes()))?,
            Arc::new(StringArray::from(vec![
                row.history_predecessor.canonical_root().as_str(),
            ])),
            Arc::new(Int64Array::from(vec![delta_int(
                row.history_predecessor.version(),
                "history_predecessor_version",
            )?])),
            fixed_array::<32>(row.transaction.map(|value| *value.as_bytes()))?,
            Arc::new(StringArray::from(vec![head_kind])),
            fixed_array::<16>(epoch)?,
            Arc::new(StringArray::from(vec![row.outcome])),
            Arc::new(Int64Array::from(vec![
                row.evidence_revision
                    .map(|value| delta_int(value, "evidence_revision"))
                    .transpose()?,
            ])),
            fixed_array::<32>(row.vacuum_candidate_digest)?,
        ],
    )
    .map_err(|error| DeltaProgrammaticMaintenanceRelationError::Arrow(error.to_string()))
}

fn encode_proposed_deletion_resource(
    resource: Option<&DeltaRetainedResource>,
) -> (
    Option<&'static str>,
    Option<&str>,
    Option<u64>,
    Option<[u8; 32]>,
) {
    match resource {
        None => (None, None, None, None),
        Some(DeltaRetainedResource::DeltaVersion(pin)) => (
            Some(RESOURCE_DELTA_VERSION),
            Some(pin.canonical_root().as_str()),
            Some(pin.version()),
            None,
        ),
        Some(DeltaRetainedResource::ImmutableSegment(identity)) => (
            Some(RESOURCE_IMMUTABLE_SEGMENT),
            None,
            None,
            Some(*identity),
        ),
        Some(DeltaRetainedResource::ProgramRelease(identity)) => {
            (Some(RESOURCE_PROGRAM_RELEASE), None, None, Some(*identity))
        }
        Some(DeltaRetainedResource::Expectation(identity)) => {
            (Some(RESOURCE_EXPECTATION), None, None, Some(*identity))
        }
        Some(DeltaRetainedResource::QueryResult(identity)) => {
            (Some(RESOURCE_QUERY_RESULT), None, None, Some(*identity))
        }
        Some(DeltaRetainedResource::RollbackPoint(identity)) => {
            (Some(RESOURCE_ROLLBACK_POINT), None, None, Some(*identity))
        }
    }
}

fn fixed_array<const N: usize>(
    value: Option<[u8; N]>,
) -> Result<ArrayRef, DeltaProgrammaticMaintenanceRelationError> {
    Ok(Arc::new(BinaryArray::from(vec![
        value.as_ref().map(<[u8; N]>::as_slice),
    ])))
}

fn delta_int(
    value: u64,
    name: &'static str,
) -> Result<i64, DeltaProgrammaticMaintenanceRelationError> {
    i64::try_from(value)
        .map_err(|_| DeltaProgrammaticMaintenanceRelationError::UnsignedIntegerOutOfRange(name))
}

fn decode_batches(
    batches: &[RecordBatch],
) -> Result<Vec<DecodedRow>, DeltaProgrammaticMaintenanceRelationError> {
    let mut rows = Vec::new();
    for batch in batches {
        if batch.schema_ref().as_ref() != relation_schema().as_ref() {
            return Err(DeltaProgrammaticMaintenanceRelationError::SchemaMismatch {
                expected: relation_schema(),
                actual: batch.schema(),
            });
        }
        for row in 0..batch.num_rows() {
            let expected_head = match optional_string(batch, "expected_head_kind", row)? {
                None => None,
                Some("empty") => Some(ExpectedHead::Empty),
                Some("epoch") => Some(ExpectedHead::Epoch(super::command::EpochId::from_bytes(
                    required_fixed::<16>(batch, "expected_epoch", row)?,
                ))),
                Some(_) => {
                    return Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
                        "expected_head_kind",
                    ));
                }
            };
            rows.push(DecodedRow {
                row_kind: required_string(batch, "row_kind", row)?.to_owned(),
                workspace_id: WorkspaceId::from_bytes(required_fixed::<16>(
                    batch,
                    "workspace_id",
                    row,
                )?),
                operation_id: OperationId::from_bytes(required_fixed::<16>(
                    batch,
                    "operation_id",
                    row,
                )?),
                request: AdministrationRequestRef::from_bytes(required_fixed::<32>(
                    batch, "request", row,
                )?),
                action: decode_action(required_string(batch, "action", row)?)?,
                relation_id: required_string(batch, "relation_id", row)?.to_owned(),
                target: ExactDeltaPin::new(
                    &url::Url::parse(required_string(batch, "target_root", row)?).map_err(
                        |_| DeltaProgrammaticMaintenanceRelationError::Malformed("target_root"),
                    )?,
                    required_u64(batch, "target_version", row)?,
                )
                .map_err(|_| DeltaProgrammaticMaintenanceRelationError::Malformed("target"))?,
                activation_head: required_fixed::<32>(batch, "activation_head", row)?,
                writer_fence: WriterFence {
                    lease_id: super::command::LeaseId::from_bytes(required_fixed::<16>(
                        batch,
                        "writer_lease",
                        row,
                    )?),
                    generation: WriterGeneration::new(required_u64(
                        batch,
                        "writer_generation",
                        row,
                    )?)
                    .ok_or(
                        DeltaProgrammaticMaintenanceRelationError::Malformed("writer_generation"),
                    )?,
                },
                intent: required_string(batch, "intent", row)?.to_owned(),
                retention_seconds: optional_u64(batch, "retention_seconds", row)?,
                proposed_deletion_count: optional_u64(batch, "proposed_deletion_count", row)?,
                proposed_deletion_set_digest: optional_fixed::<32>(
                    batch,
                    "proposed_deletion_set_digest",
                    row,
                )?,
                proposed_deletion_ordinal: optional_u64(batch, "proposed_deletion_ordinal", row)?,
                proposed_deletion_resource_kind: optional_string(
                    batch,
                    "proposed_deletion_resource_kind",
                    row,
                )?
                .map(str::to_owned),
                proposed_deletion_delta_root: optional_string(
                    batch,
                    "proposed_deletion_delta_root",
                    row,
                )?
                .map(str::to_owned),
                proposed_deletion_delta_version: optional_u64(
                    batch,
                    "proposed_deletion_delta_version",
                    row,
                )?,
                proposed_deletion_resource_id: optional_fixed::<32>(
                    batch,
                    "proposed_deletion_resource_id",
                    row,
                )?,
                operation_selection: OperationSelectionRef::from_bytes(required_fixed::<32>(
                    batch,
                    "operation_selection",
                    row,
                )?),
                history_predecessor: ExactDeltaPin::new(
                    &url::Url::parse(required_string(batch, "history_predecessor_root", row)?)
                        .map_err(|_| {
                            DeltaProgrammaticMaintenanceRelationError::Malformed(
                                "history_predecessor_root",
                            )
                        })?,
                    required_u64(batch, "history_predecessor_version", row)?,
                )
                .map_err(|_| {
                    DeltaProgrammaticMaintenanceRelationError::Malformed("history_predecessor")
                })?,
                transaction: optional_fixed::<32>(batch, "transaction", row)?
                    .map(TransactionRef::from_bytes),
                expected_head,
                outcome: optional_string(batch, "outcome", row)?.map(str::to_owned),
                evidence_revision: optional_u64(batch, "evidence_revision", row)?,
                vacuum_candidate_digest: optional_fixed::<32>(
                    batch,
                    "vacuum_candidate_digest",
                    row,
                )?,
            });
        }
    }
    Ok(rows)
}

fn find_commit_receipt(
    rows: Vec<DecodedRow>,
    selection: &ProgrammaticDeltaMaintenanceCommandSelection,
    operation_id: OperationId,
    transaction: TransactionRef,
) -> Result<Option<AdministrationCommitReceipt>, DeltaProgrammaticMaintenanceRelationError> {
    let mut matching = rows.into_iter().filter(|row| {
        row.row_kind == ROW_COMMIT
            && row.operation_id == operation_id
            && row.transaction == Some(transaction)
    });
    let Some(row) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err(DeltaProgrammaticMaintenanceRelationError::DuplicateCommit);
    }
    validate_commit_binding(&row, selection)?;
    Ok(Some(AdministrationCommitReceipt {
        workspace_id: row.workspace_id,
        operation_id: row.operation_id,
        transaction,
        writer_generation: row.writer_fence.generation,
        action: row.action,
        request: row.request,
        resulting_head: row.expected_head.ok_or(
            DeltaProgrammaticMaintenanceRelationError::Malformed("expected_head"),
        )?,
        operation_selection: row.operation_selection,
    }))
}

fn validate_commit_binding(
    row: &DecodedRow,
    selection: &ProgrammaticDeltaMaintenanceCommandSelection,
) -> Result<(), DeltaProgrammaticMaintenanceRelationError> {
    let encoded_intent = encode_intent(selection.maintenance().intent())?;
    let exact_binding = row.workspace_id == selection.workspace_id()
        && row.operation_id == selection.maintenance().operation_id()
        && row.request == selection.request()
        && row.action == selection.action()
        && row.relation_id == selection.relation_id()
        && &row.target == selection.maintenance().target()
        && &row.activation_head == selection.maintenance().expected_activation_head()
        && row.writer_fence == selection.maintenance().writer_fence()
        && row.intent == encoded_intent.code
        && row.retention_seconds == encoded_intent.retention_seconds
        && row.proposed_deletion_count == encoded_intent.proposed_deletion_count
        && row.proposed_deletion_set_digest == encoded_intent.proposed_deletion_set_digest
        && row.operation_selection == selection.operation_selection()
        && &row.history_predecessor == selection.history_predecessor();
    if !exact_binding {
        return Err(
            DeltaProgrammaticMaintenanceRelationError::CommitBindingMismatch("request_authority"),
        );
    }
    if row.proposed_deletion_ordinal.is_some()
        || row.proposed_deletion_resource_kind.is_some()
        || row.proposed_deletion_delta_root.is_some()
        || row.proposed_deletion_delta_version.is_some()
        || row.proposed_deletion_resource_id.is_some()
    {
        return Err(
            DeltaProgrammaticMaintenanceRelationError::CommitBindingMismatch(
                "proposed_deletion_child_shape",
            ),
        );
    }
    if row.evidence_revision.is_none_or(|revision| revision == 0) {
        return Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
            "evidence_revision",
        ));
    }
    let (expected_outcome, expects_vacuum_digest) = match selection.maintenance().intent() {
        GuardedDeltaMaintenanceIntent::InspectRetention => (OUTCOME_INSPECTED, false),
        GuardedDeltaMaintenanceIntent::ValidateRetention { .. } => (OUTCOME_VALIDATED, false),
        GuardedDeltaMaintenanceIntent::VacuumDryRun { .. } => (OUTCOME_VACUUM_DRY_RUN, true),
        _ => return Err(DeltaProgrammaticMaintenanceRelationError::UnsupportedIntent),
    };
    if row.outcome.as_deref() != Some(expected_outcome) {
        return Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
            "outcome",
        ));
    }
    if row.vacuum_candidate_digest.is_some() != expects_vacuum_digest {
        return Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
            "vacuum_candidate_digest",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct EncodedIntent {
    code: &'static str,
    retention_seconds: Option<u64>,
    proposed_deletion_count: Option<u64>,
    proposed_deletion_set_digest: Option<[u8; 32]>,
}

fn encode_intent(
    intent: &GuardedDeltaMaintenanceIntent,
) -> Result<EncodedIntent, DeltaProgrammaticMaintenanceRelationError> {
    match intent {
        GuardedDeltaMaintenanceIntent::InspectRetention => Ok(EncodedIntent {
            code: INTENT_INSPECT,
            retention_seconds: None,
            proposed_deletion_count: None,
            proposed_deletion_set_digest: None,
        }),
        GuardedDeltaMaintenanceIntent::ValidateRetention { proposed_deletions } => {
            Ok(EncodedIntent {
                code: INTENT_VALIDATE,
                retention_seconds: None,
                proposed_deletion_count: Some(u64::try_from(proposed_deletions.len()).map_err(
                    |_| {
                        DeltaProgrammaticMaintenanceRelationError::CardinalityOverflow(
                            "proposed_deletion_count",
                        )
                    },
                )?),
                proposed_deletion_set_digest: Some(proposed_deletion_set_identity(
                    proposed_deletions,
                )),
            })
        }
        GuardedDeltaMaintenanceIntent::VacuumDryRun {
            expected_retention_seconds,
        } => Ok(EncodedIntent {
            code: INTENT_VACUUM_DRY_RUN,
            retention_seconds: Some(*expected_retention_seconds),
            proposed_deletion_count: None,
            proposed_deletion_set_digest: None,
        }),
        _ => Err(DeltaProgrammaticMaintenanceRelationError::UnsupportedIntent),
    }
}

const fn action_code(action: AdministrationAction) -> &'static str {
    match action {
        AdministrationAction::InspectDeltaRetention => "inspect_delta_retention",
        AdministrationAction::PlanDeltaVacuum => "plan_delta_vacuum",
        AdministrationAction::ValidateDeltaRetention => "validate_delta_retention",
        AdministrationAction::CreateDeltaCheckpoint => "create_delta_checkpoint",
        AdministrationAction::CompactDelta => "compact_delta",
        AdministrationAction::ExecuteDeltaVacuum => "execute_delta_vacuum",
        AdministrationAction::RebuildCandidate => "rebuild_candidate",
        AdministrationAction::RepairTemporalCache => "repair_temporal_cache",
        AdministrationAction::ReconcileOperation => "reconcile_operation",
    }
}

fn decode_action(
    value: &str,
) -> Result<AdministrationAction, DeltaProgrammaticMaintenanceRelationError> {
    match value {
        "inspect_delta_retention" => Ok(AdministrationAction::InspectDeltaRetention),
        "plan_delta_vacuum" => Ok(AdministrationAction::PlanDeltaVacuum),
        "validate_delta_retention" => Ok(AdministrationAction::ValidateDeltaRetention),
        "create_delta_checkpoint" => Ok(AdministrationAction::CreateDeltaCheckpoint),
        "compact_delta" => Ok(AdministrationAction::CompactDelta),
        "execute_delta_vacuum" => Ok(AdministrationAction::ExecuteDeltaVacuum),
        "rebuild_candidate" => Ok(AdministrationAction::RebuildCandidate),
        "repair_temporal_cache" => Ok(AdministrationAction::RepairTemporalCache),
        "reconcile_operation" => Ok(AdministrationAction::ReconcileOperation),
        _ => Err(DeltaProgrammaticMaintenanceRelationError::Malformed(
            "action",
        )),
    }
}

fn diagnostic(
    commit: &ProgrammaticDeltaMaintenanceCommandCommit,
    stage: &[u8],
) -> super::command::DiagnosticRef {
    let mut digest = blake3::Hasher::new();
    digest.update(b"codefabric.delta-maintenance-history-diagnostic.v1");
    digest.update(commit.attempt().command().identity.operation_id.as_bytes());
    digest.update(commit.transaction().as_bytes());
    digest.update(stage);
    super::command::DiagnosticRef::from_bytes(*digest.finalize().as_bytes())
}

fn reconciliation_evidence(
    request: AdministrationReconciliationRequest,
    stage: &[u8],
) -> ReconciliationEvidenceRef {
    let mut digest = blake3::Hasher::new();
    digest.update(b"codefabric.delta-maintenance-history-reconciliation.v1");
    digest.update(request.attempt().command().identity.operation_id.as_bytes());
    digest.update(request.transaction().as_bytes());
    digest.update(request.active_recovery_owner().fence.lease_id.as_bytes());
    digest.update(
        &request
            .active_recovery_owner()
            .fence
            .generation
            .get()
            .to_be_bytes(),
    );
    digest.update(stage);
    ReconciliationEvidenceRef::from_bytes(*digest.finalize().as_bytes())
}

fn required_string<'a>(
    batch: &'a RecordBatch,
    name: &'static str,
    row: usize,
) -> Result<&'a str, DeltaProgrammaticMaintenanceRelationError> {
    optional_string(batch, name, row)?
        .ok_or(DeltaProgrammaticMaintenanceRelationError::Malformed(name))
}

fn optional_string<'a>(
    batch: &'a RecordBatch,
    name: &'static str,
    row: usize,
) -> Result<Option<&'a str>, DeltaProgrammaticMaintenanceRelationError> {
    let array = batch
        .column_by_name(name)
        .and_then(|array| array.as_any().downcast_ref::<StringArray>())
        .ok_or(DeltaProgrammaticMaintenanceRelationError::Malformed(name))?;
    Ok((!array.is_null(row)).then(|| array.value(row)))
}

fn required_u64(
    batch: &RecordBatch,
    name: &'static str,
    row: usize,
) -> Result<u64, DeltaProgrammaticMaintenanceRelationError> {
    optional_u64(batch, name, row)?
        .ok_or(DeltaProgrammaticMaintenanceRelationError::Malformed(name))
}

fn optional_u64(
    batch: &RecordBatch,
    name: &'static str,
    row: usize,
) -> Result<Option<u64>, DeltaProgrammaticMaintenanceRelationError> {
    let array = batch
        .column_by_name(name)
        .and_then(|array| array.as_any().downcast_ref::<Int64Array>())
        .ok_or(DeltaProgrammaticMaintenanceRelationError::Malformed(name))?;
    if array.is_null(row) {
        return Ok(None);
    }
    u64::try_from(array.value(row))
        .map(Some)
        .map_err(|_| DeltaProgrammaticMaintenanceRelationError::NegativeInteger(name))
}

fn required_fixed<const N: usize>(
    batch: &RecordBatch,
    name: &'static str,
    row: usize,
) -> Result<[u8; N], DeltaProgrammaticMaintenanceRelationError> {
    optional_fixed(batch, name, row)?
        .ok_or(DeltaProgrammaticMaintenanceRelationError::Malformed(name))
}

fn optional_fixed<const N: usize>(
    batch: &RecordBatch,
    name: &'static str,
    row: usize,
) -> Result<Option<[u8; N]>, DeltaProgrammaticMaintenanceRelationError> {
    let array = batch
        .column_by_name(name)
        .and_then(|array| array.as_any().downcast_ref::<BinaryArray>())
        .ok_or(DeltaProgrammaticMaintenanceRelationError::Malformed(name))?;
    if array.is_null(row) {
        return Ok(None);
    }
    array
        .value(row)
        .try_into()
        .map(Some)
        .map_err(|_| DeltaProgrammaticMaintenanceRelationError::Malformed(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::command::{LeaseId, OperationId};
    use url::Url;

    #[test]
    fn validation_request_round_trip_preserves_reversible_ordered_child_relation() {
        let selection = validation_selection(proposed_deletions());
        let batch = DeltaProgrammaticMaintenanceCommandRelation::encode_request(&selection)
            .expect("encode validation request and child relation");
        assert_eq!(batch.num_rows(), 7, "one parent and six resource rows");

        let (parent, children) = decode_request(batch);
        let reconstructed = parent
            .into_selection(children)
            .expect("decode exact child relation");
        assert_eq!(reconstructed, selection);
    }

    #[test]
    fn validation_request_rejects_noncontiguous_child_ordinals_and_digest_drift() {
        let selection = validation_selection(proposed_deletions());
        let batch = DeltaProgrammaticMaintenanceCommandRelation::encode_request(&selection)
            .expect("encode validation request");
        let (parent, mut children) = decode_request(batch.clone());
        children[1].proposed_deletion_ordinal = Some(0);
        assert!(matches!(
            parent.into_selection(children),
            Err(
                DeltaProgrammaticMaintenanceRelationError::ProposedDeletionOrdinalMismatch {
                    expected: 1,
                    actual: 0
                }
            )
        ));

        let (mut parent, mut children) = decode_request(batch);
        parent.proposed_deletion_set_digest = Some([0xa7; 32]);
        for child in &mut children {
            child.proposed_deletion_set_digest = Some([0xa7; 32]);
        }
        assert!(matches!(
            parent.into_selection(children),
            Err(DeltaProgrammaticMaintenanceRelationError::ProposedDeletionDigestMismatch)
        ));
    }

    #[test]
    fn validation_child_rows_reject_parent_authority_substitution() {
        let selection = validation_selection(proposed_deletions());
        let batch = DeltaProgrammaticMaintenanceCommandRelation::encode_request(&selection)
            .expect("encode validation request");
        let (parent, children) = decode_request(batch);
        let child = &children[0];

        let mut workspace = child.clone();
        workspace.workspace_id = WorkspaceId::from_bytes([0x91; 16]);
        let mut operation = child.clone();
        operation.operation_id = OperationId::from_bytes([0x92; 16]);
        let mut target = child.clone();
        target.target =
            ExactDeltaPin::new(&Url::parse("memory:///semantic/substitute").unwrap(), 7).unwrap();
        let mut fence = child.clone();
        fence.writer_fence.generation = WriterGeneration::new(6).unwrap();
        let mut predecessor = child.clone();
        predecessor.history_predecessor = ExactDeltaPin::new(
            &Url::parse("memory:///programmatic-maintenance-history").unwrap(),
            8,
        )
        .unwrap();

        for substituted in [workspace, operation, target, fence, predecessor] {
            assert!(!substituted.binds_proposed_deletion_parent(&parent));
        }

        let mut substituted_children = children;
        substituted_children[0].writer_fence.generation = WriterGeneration::new(6).unwrap();
        assert!(matches!(
            parent.into_selection(substituted_children),
            Err(DeltaProgrammaticMaintenanceRelationError::ProposedDeletionParentBindingMismatch)
        ));
    }

    #[test]
    fn validation_commit_readback_rejects_changed_child_set_binding() {
        let selection = validation_selection(proposed_deletions());
        let encoded_intent = encode_intent(selection.maintenance().intent()).unwrap();
        let batch = encode_row(&EncodedRow {
            row_kind: ROW_COMMIT,
            workspace_id: selection.workspace_id(),
            operation_id: selection.maintenance().operation_id(),
            request: selection.request(),
            action: selection.action(),
            relation_id: selection.relation_id(),
            target: selection.maintenance().target(),
            activation_head: selection.maintenance().expected_activation_head(),
            writer_fence: selection.maintenance().writer_fence(),
            intent: encoded_intent.code,
            retention_seconds: encoded_intent.retention_seconds,
            proposed_deletion_count: encoded_intent.proposed_deletion_count,
            proposed_deletion_set_digest: encoded_intent.proposed_deletion_set_digest,
            proposed_deletion_ordinal: None,
            proposed_deletion: None,
            operation_selection: selection.operation_selection(),
            history_predecessor: selection.history_predecessor(),
            transaction: Some(TransactionRef::from_bytes([0x71; 32])),
            expected_head: Some(ExpectedHead::Empty),
            outcome: Some(OUTCOME_VALIDATED),
            evidence_revision: Some(1),
            vacuum_candidate_digest: None,
        })
        .expect("encode validation commit");
        let mut rows = decode_batches(&[batch]).expect("decode validation commit");
        let mut row = rows.pop().unwrap();
        validate_commit_binding(&row, &selection).expect("exact commit binding");

        row.proposed_deletion_set_digest = Some([0xb8; 32]);
        assert!(matches!(
            validate_commit_binding(&row, &selection),
            Err(
                DeltaProgrammaticMaintenanceRelationError::CommitBindingMismatch(
                    "request_authority"
                )
            )
        ));
    }

    fn decode_request(batch: RecordBatch) -> (DecodedRow, Vec<DecodedRow>) {
        let mut rows = decode_batches(&[batch]).expect("decode request rows");
        let parent_index = rows
            .iter()
            .position(|row| row.row_kind == ROW_REQUEST)
            .expect("request parent row");
        let parent = rows.remove(parent_index);
        (parent, rows)
    }

    fn validation_selection(
        proposed_deletions: Arc<[DeltaRetainedResource]>,
    ) -> ProgrammaticDeltaMaintenanceCommandSelection {
        let history = ExactDeltaPin::new(
            &Url::parse("memory:///programmatic-maintenance-history").unwrap(),
            9,
        )
        .unwrap();
        ProgrammaticDeltaMaintenanceCommandSelection::try_new(
            AdministrationAction::ValidateDeltaRetention,
            AdministrationRequestRef::from_bytes([0x31; 32]),
            WorkspaceId::from_bytes([0x32; 16]),
            "system.programmatic_relation_observation",
            GuardedDeltaMaintenanceRequest::try_new(
                ExactDeltaPin::new(&Url::parse("memory:///semantic/table").unwrap(), 7).unwrap(),
                [0x33; 32],
                WriterFence {
                    lease_id: LeaseId::from_bytes([0x34; 16]),
                    generation: WriterGeneration::new(5).unwrap(),
                },
                OperationId::from_bytes([0x35; 16]),
                GuardedDeltaMaintenanceIntent::ValidateRetention { proposed_deletions },
            )
            .unwrap(),
            history,
            OperationSelectionRef::from_bytes([0x36; 32]),
        )
        .unwrap()
    }

    fn proposed_deletions() -> Arc<[DeltaRetainedResource]> {
        vec![
            DeltaRetainedResource::DeltaVersion(
                ExactDeltaPin::new(&Url::parse("memory:///semantic/retained").unwrap(), 3).unwrap(),
            ),
            DeltaRetainedResource::ImmutableSegment([0x41; 32]),
            DeltaRetainedResource::ProgramRelease([0x42; 32]),
            DeltaRetainedResource::Expectation([0x43; 32]),
            DeltaRetainedResource::QueryResult([0x44; 32]),
            DeltaRetainedResource::RollbackPoint([0x45; 32]),
        ]
        .into()
    }
}
