//! Exact, append-only Delta histories for executable proof relations.
//!
//! The evaluator's Arrow batches are transient products. This module writes every proof output to
//! its own workspace-owned Delta table, records the complete materialization identity in the exact
//! zero-retry commit, and reconstructs [`ProofRelations`] only through delta-rs providers opened at
//! the published versions. No directory listing or raw Parquet path participates in reopening.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::sync::Arc;

use arrow_array::{
    Array as _, ArrayRef, BinaryArray, FixedSizeBinaryArray, Int8Array, Int64Array, RecordBatch,
    UInt32Array, UInt64Array,
};
use arrow_cast::cast;
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use arrow_select::concat::concat_batches;
use arrow_select::take::take;
use datafusion::prelude::SessionContext;
use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;
use deltalake::{DeltaTable, DeltaTableBuilder};
use serde_json::Value;
use thiserror::Error;
use url::Url;

use super::{
    OracleId, OracleImplementationRef, ProofCandidatePins, ProofError, ProofOracleObservation,
    ProofRelationKind, ProofRelationOutput, ProofRelations, ProofRunId, ProofTerminalStatus,
    capability_schema, coverage_schema, expectation_schema, fault_schema, issue_schema,
    oracle_schema, proof_run_schema, provenance_schema, violation_schema,
};
use crate::fabric::command::{EpochId, OperationId, TransactionRef, WorkspaceId, WriterGeneration};
use crate::fabric::delta_exact::{
    ExactDeltaPin, ValidatedDeltaSnapshot, provider_read_from_validated_snapshot,
    read_exact_commit_entry,
};
use crate::fabric::delta_write::{
    ApplicationTransactionMarker, ControlledDeltaHistoryProperties, ControlledDeltaWriteMode,
    ControlledDeltaWriteOutcome, ControlledDeltaWriteSpec, SessionBoundLogicalPlan,
    write_exact_delta_plan,
};

const HISTORY_DIRECTORY: &str = "proof-relations";
const PROOF_SET_FIELD: &str = "_codefabric_proof_set_id";
const ROW_ORDINAL_FIELD: &str = "_codefabric_row_ordinal";
const RELATION_METADATA_KEY: &str = "codefabric.proof_history.relation_id";
const META_WORKSPACE_ID: &str = "codefabric.proof_history.workspace_id";
const META_EPOCH_ID: &str = "codefabric.proof_history.epoch_id";
const META_PROOF_SET_ID: &str = "codefabric.proof_history.proof_set_id";
const META_ROW_COUNT: &str = "codefabric.proof_history.row_count";
const META_SCHEMA_DIGEST: &str = "codefabric.proof_history.schema_digest";
const META_BATCH_DIGEST: &str = "codefabric.proof_history.batch_digest";
const OPERATION_METRICS: &str = "operationMetrics";
const NUM_RETRIES: &str = "num_retries";

impl ProofRelationKind {
    /// Stable typed durable relation identity. This is storage identity, not a catalog name.
    #[must_use]
    pub const fn durable_relation_id(self) -> &'static str {
        match self {
            Self::ProofRun => "proof.proof_run",
            Self::OracleResult => "proof.oracle_result",
            Self::CapabilityResult => "proof.capability_result",
            Self::Expectation => "proof.expectation",
            Self::CoverageResult => "proof.coverage_result",
            Self::FaultResult => "proof.fault_result",
            Self::ViolationResult => "proof.violation_result",
            Self::ProvenanceEdge => "proof.provenance_edge",
            Self::Issue => "proof.issue",
        }
    }

    const fn history_slug(self) -> &'static str {
        match self {
            Self::ProofRun => "proof_run",
            Self::OracleResult => "oracle_result",
            Self::CapabilityResult => "capability_result",
            Self::Expectation => "expectation",
            Self::CoverageResult => "coverage_result",
            Self::FaultResult => "fault_result",
            Self::ViolationResult => "violation_result",
            Self::ProvenanceEdge => "provenance_edge",
            Self::Issue => "issue",
        }
    }
}

/// Canonical workspace root from which every proof-history root is deterministically derived.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofDeltaWorkspaceRoot {
    workspace_id: WorkspaceId,
    canonical_root: Url,
}

impl ProofDeltaWorkspaceRoot {
    /// Canonicalize an existing workspace-owned directory/object-store prefix.
    pub fn try_new(
        workspace_id: WorkspaceId,
        workspace_root: Url,
    ) -> Result<Self, ProofRelationsDeltaError> {
        let canonical_root = ExactDeltaPin::new(&workspace_root, 0)
            .map_err(|source| ProofRelationsDeltaError::Root(source.to_string()))?
            .canonical_root()
            .clone();
        Ok(Self {
            workspace_id,
            canonical_root,
        })
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn canonical_root(&self) -> &Url {
        &self.canonical_root
    }

    fn relation_root(&self, kind: ProofRelationKind) -> Result<Url, ProofRelationsDeltaError> {
        self.canonical_root
            .join(&format!(
                "{HISTORY_DIRECTORY}/{}/{}/",
                lower_hex(self.workspace_id.as_bytes()),
                kind.history_slug()
            ))
            .map_err(|source| ProofRelationsDeltaError::Root(source.to_string()))
    }

    fn validate_pin(
        &self,
        kind: ProofRelationKind,
        pin: &ExactDeltaPin,
    ) -> Result<(), ProofRelationsDeltaError> {
        let expected = ExactDeltaPin::new(&self.relation_root(kind)?, pin.version())
            .map_err(|source| ProofRelationsDeltaError::Root(source.to_string()))?;
        if &expected != pin {
            return Err(ProofRelationsDeltaError::RootOwnership {
                relation: kind,
                expected: expected.canonical_root().to_string(),
                observed: pin.canonical_root().to_string(),
            });
        }
        Ok(())
    }
}

struct ProofDeltaHistoryTarget {
    predecessor: ExactDeltaPin,
    table: DeltaTable,
}

/// Complete version-zero target set for all nine proof relations.
pub struct ProofDeltaHistoryTargets {
    workspace: ProofDeltaWorkspaceRoot,
    targets: BTreeMap<ProofRelationKind, ProofDeltaHistoryTarget>,
}

impl std::fmt::Debug for ProofDeltaHistoryTargets {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProofDeltaHistoryTargets")
            .field("workspace", &self.workspace)
            .field(
                "pins",
                &self
                    .targets
                    .iter()
                    .map(|(kind, target)| (*kind, &target.predecessor))
                    .collect::<BTreeMap<_, _>>(),
            )
            .finish()
    }
}

/// One controlled publication attempt shared by all nine relation histories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofDeltaWriteIdentity {
    pub operation_id: OperationId,
    pub writer_generation: WriterGeneration,
    pub proof_set_id: TransactionRef,
}

/// Durable activation input naming exactly one persisted version of every proof relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofDeltaHistoryPublication {
    workspace: ProofDeltaWorkspaceRoot,
    candidate_pins: ProofCandidatePins,
    proof_set_id: TransactionRef,
    versions: BTreeMap<ProofRelationKind, ExactDeltaPin>,
}

impl ProofDeltaHistoryPublication {
    /// Validate a complete nine-relation exact-version vector.
    pub fn try_new(
        workspace: ProofDeltaWorkspaceRoot,
        candidate_pins: ProofCandidatePins,
        proof_set_id: TransactionRef,
        versions: BTreeMap<ProofRelationKind, ExactDeltaPin>,
    ) -> Result<Self, ProofRelationsDeltaError> {
        for kind in ProofRelationKind::ALL {
            let pin = versions
                .get(&kind)
                .ok_or(ProofRelationsDeltaError::MissingRelation(kind))?;
            workspace.validate_pin(kind, pin)?;
        }
        if versions.len() != ProofRelationKind::ALL.len() {
            return Err(ProofRelationsDeltaError::UnexpectedRelationCount {
                observed: versions.len(),
            });
        }
        Ok(Self {
            workspace,
            candidate_pins,
            proof_set_id,
            versions,
        })
    }

    #[must_use]
    pub const fn workspace(&self) -> &ProofDeltaWorkspaceRoot {
        &self.workspace
    }

    #[must_use]
    pub const fn candidate_pins(&self) -> ProofCandidatePins {
        self.candidate_pins
    }

    #[must_use]
    pub const fn proof_set_id(&self) -> TransactionRef {
        self.proof_set_id
    }

    #[must_use]
    pub fn exact_pin(&self, kind: ProofRelationKind) -> &ExactDeltaPin {
        self.versions
            .get(&kind)
            .expect("publication constructor proves the complete relation census")
    }

    #[must_use]
    pub const fn versions(&self) -> &BTreeMap<ProofRelationKind, ExactDeltaPin> {
        &self.versions
    }
}

/// Failure to provision, publish, or exactly reopen durable proof relations.
#[derive(Debug, Error)]
pub enum ProofRelationsDeltaError {
    #[error("invalid proof-history root: {0}")]
    Root(String),
    #[error(
        "proof history {relation:?} is outside its workspace root: expected {expected}, observed {observed}"
    )]
    RootOwnership {
        relation: ProofRelationKind,
        expected: String,
        observed: String,
    },
    #[error("proof-history relation {0:?} is missing")]
    MissingRelation(ProofRelationKind),
    #[error("proof-history vector contains {observed} relations instead of nine")]
    UnexpectedRelationCount { observed: usize },
    #[error("Delta operation for proof relation {relation:?} failed: {detail}")]
    Delta {
        relation: ProofRelationKind,
        detail: String,
    },
    #[error("Arrow operation for proof relation {relation:?} failed: {detail}")]
    Arrow {
        relation: ProofRelationKind,
        detail: String,
    },
    #[error("DataFusion operation for proof relation {relation:?} failed: {detail}")]
    DataFusion {
        relation: ProofRelationKind,
        detail: String,
    },
    #[error("controlled write for proof relation {relation:?} did not commit: {outcome:?}")]
    WriteOutcome {
        relation: ProofRelationKind,
        outcome: Box<ControlledDeltaWriteOutcome>,
    },
    #[error("exact proof relation {relation:?} commit metadata is invalid: {detail}")]
    CommitEvidence {
        relation: ProofRelationKind,
        detail: String,
    },
    #[error("exact proof relation {relation:?} rows are invalid: {detail}")]
    RowIntegrity {
        relation: ProofRelationKind,
        detail: String,
    },
    #[error("reopened proof relation set is internally inconsistent: {0}")]
    RelationSet(String),
    #[error(transparent)]
    Proof(#[from] ProofError),
}

/// Create all nine append-only/CDF-enabled histories at exact version zero.
pub async fn provision_proof_relation_histories(
    workspace: ProofDeltaWorkspaceRoot,
) -> Result<ProofDeltaHistoryTargets, ProofRelationsDeltaError> {
    let mut targets = BTreeMap::new();
    for kind in ProofRelationKind::ALL {
        let root = workspace.relation_root(kind)?;
        if root.scheme() == "file" {
            let path = root.to_file_path().map_err(|()| {
                ProofRelationsDeltaError::Root(format!("{root} is not a local directory URL"))
            })?;
            fs::create_dir_all(&path).map_err(|source| ProofRelationsDeltaError::Delta {
                relation: kind,
                detail: source.to_string(),
            })?;
        }
        let schema = history_schema(kind);
        let kernel: deltalake::kernel::StructType =
            schema.as_ref().try_into_kernel().map_err(|source| {
                ProofRelationsDeltaError::Delta {
                    relation: kind,
                    detail: source.to_string(),
                }
            })?;
        ControlledDeltaHistoryProperties::try_new(format!(
            "{PROOF_SET_FIELD},{ROW_ORDINAL_FIELD},epoch_id"
        ))
        .map_err(|source| ProofRelationsDeltaError::Delta {
            relation: kind,
            detail: source.to_string(),
        })?
        .apply_to(
            CreateBuilder::new()
                .with_location(root.to_string())
                .with_table_name(kind.history_slug())
                .with_comment(format!(
                    "CodeFabric append-only history for {}",
                    kind.durable_relation_id()
                ))
                .with_save_mode(SaveMode::ErrorIfExists)
                .with_columns(kernel.fields().cloned()),
        )
        .await
        .map_err(|source| ProofRelationsDeltaError::Delta {
            relation: kind,
            detail: source.to_string(),
        })?;
        let table = DeltaTableBuilder::from_url(root.clone())
            .map_err(|source| ProofRelationsDeltaError::Delta {
                relation: kind,
                detail: source.to_string(),
            })?
            .with_skip_stats(false)
            .with_version(0)
            .load()
            .await
            .map_err(|source| ProofRelationsDeltaError::Delta {
                relation: kind,
                detail: source.to_string(),
            })?;
        let predecessor = ExactDeltaPin::new(&root, 0)
            .map_err(|source| ProofRelationsDeltaError::Root(source.to_string()))?;
        workspace.validate_pin(kind, &predecessor)?;
        targets.insert(kind, ProofDeltaHistoryTarget { predecessor, table });
    }
    Ok(ProofDeltaHistoryTargets { workspace, targets })
}

/// Append every proof output once with the same transaction identity and return exact versions.
pub async fn persist_proof_relations(
    session: Arc<datafusion::execution::SessionState>,
    mut targets: ProofDeltaHistoryTargets,
    identity: ProofDeltaWriteIdentity,
    relations: &ProofRelations,
) -> Result<ProofDeltaHistoryPublication, ProofRelationsDeltaError> {
    let context = SessionContext::new_with_state(session.as_ref().clone());
    let mut versions = BTreeMap::new();
    for kind in ProofRelationKind::ALL {
        let target = targets
            .targets
            .remove(&kind)
            .ok_or(ProofRelationsDeltaError::MissingRelation(kind))?;
        let logical = relations.relation(kind).batch();
        let batch_digest = batch_digest(kind, logical)?;
        let schema_digest = schema_digest(kind)?;
        let history_batch = build_history_batch(kind, logical, identity.proof_set_id)?;
        let dataframe = context.read_batch(history_batch).map_err(|source| {
            ProofRelationsDeltaError::DataFusion {
                relation: kind,
                detail: source.to_string(),
            }
        })?;
        let plan = SessionBoundLogicalPlan::try_from_dataframe(Arc::clone(&session), dataframe)
            .map_err(|source| ProofRelationsDeltaError::DataFusion {
                relation: kind,
                detail: source.to_string(),
            })?;
        let metadata = commit_metadata(
            &targets.workspace,
            kind,
            relations.candidate_pins().epoch,
            identity.proof_set_id,
            logical.num_rows(),
            schema_digest,
            batch_digest,
        );
        let spec = ControlledDeltaWriteSpec::new(
            target.predecessor,
            identity.operation_id,
            identity.writer_generation,
            ApplicationTransactionMarker::from_transaction_ref(identity.proof_set_id),
            ControlledDeltaWriteMode::Append,
        )
        .with_commit_metadata(metadata)
        .map_err(|source| ProofRelationsDeltaError::CommitEvidence {
            relation: kind,
            detail: source.to_string(),
        })?;
        match write_exact_delta_plan(&target.table, &spec, plan).await {
            ControlledDeltaWriteOutcome::Committed(committed) => {
                versions.insert(kind, committed.committed().clone());
            }
            outcome => {
                return Err(ProofRelationsDeltaError::WriteOutcome {
                    relation: kind,
                    outcome: Box::new(outcome),
                });
            }
        }
    }
    ProofDeltaHistoryPublication::try_new(
        targets.workspace,
        relations.candidate_pins(),
        identity.proof_set_id,
        versions,
    )
}

/// Reconstruct all proof relations from their exact published Delta versions.
pub async fn reopen_proof_relations(
    session: Arc<datafusion::execution::SessionState>,
    publication: &ProofDeltaHistoryPublication,
) -> Result<ProofRelations, ProofRelationsDeltaError> {
    let mut outputs = BTreeMap::new();
    for kind in ProofRelationKind::ALL {
        let pin = publication.exact_pin(kind);
        publication.workspace.validate_pin(kind, pin)?;
        let table = DeltaTableBuilder::from_url(pin.canonical_root().clone())
            .map_err(|source| ProofRelationsDeltaError::Delta {
                relation: kind,
                detail: source.to_string(),
            })?
            .with_skip_stats(false)
            .with_version(pin.version())
            .load()
            .await
            .map_err(|source| ProofRelationsDeltaError::Delta {
                relation: kind,
                detail: source.to_string(),
            })?;
        let evidence = read_commit_evidence(kind, publication, &table).await?;
        let snapshot =
            ValidatedDeltaSnapshot::try_from_loaded_table(table, pin).map_err(|source| {
                ProofRelationsDeltaError::Delta {
                    relation: kind,
                    detail: source.to_string(),
                }
            })?;
        let read = provider_read_from_validated_snapshot(pin, snapshot, Arc::clone(&session))
            .await
            .map_err(|source| ProofRelationsDeltaError::Delta {
                relation: kind,
                detail: source.to_string(),
            })?;
        let provider = read.into_provider();
        let expected_history_schema = history_schema(kind);
        validate_provider_schema(kind, provider.schema().as_ref(), &expected_history_schema)?;
        let provider_schema = provider.schema();
        let context = SessionContext::new_with_state(session.as_ref().clone());
        let batches = context
            .read_table(provider)
            .map_err(|source| ProofRelationsDeltaError::DataFusion {
                relation: kind,
                detail: source.to_string(),
            })?
            .collect()
            .await
            .map_err(|source| ProofRelationsDeltaError::DataFusion {
                relation: kind,
                detail: source.to_string(),
            })?;
        let logical = select_proof_set(kind, publication.proof_set_id, provider_schema, &batches)?;
        if logical.num_rows() != evidence.row_count {
            return Err(ProofRelationsDeltaError::RowIntegrity {
                relation: kind,
                detail: format!(
                    "selected {} rows, exact commit declares {}",
                    logical.num_rows(),
                    evidence.row_count
                ),
            });
        }
        if batch_digest(kind, &logical)? != evidence.batch_digest {
            return Err(ProofRelationsDeltaError::RowIntegrity {
                relation: kind,
                detail: "selected rows do not match the exact commit digest".to_owned(),
            });
        }
        validate_epoch_column(kind, &logical, publication.candidate_pins.epoch)?;
        outputs.insert(
            kind,
            ProofRelationOutput::try_new(expected_schema(kind), logical)?,
        );
    }
    reconstruct_relations(publication.candidate_pins, outputs)
}

#[derive(Clone, Copy)]
struct CommitEvidence {
    row_count: usize,
    batch_digest: [u8; 32],
}

async fn read_commit_evidence(
    kind: ProofRelationKind,
    publication: &ProofDeltaHistoryPublication,
    table: &DeltaTable,
) -> Result<CommitEvidence, ProofRelationsDeltaError> {
    let entry = read_exact_commit_entry(table).await.map_err(|source| {
        ProofRelationsDeltaError::CommitEvidence {
            relation: kind,
            detail: source.to_string(),
        }
    })?;
    if entry.version() != publication.exact_pin(kind).version() {
        return commit_error(kind, "commit entry version differs from the published pin");
    }
    let marker = ApplicationTransactionMarker::from_transaction_ref(publication.proof_set_id);
    match entry.application_transactions() {
        [transaction]
            if transaction.app_id == marker.application_id()
                && transaction.version == marker.application_version() => {}
        _ => return commit_error(kind, "exact commit has the wrong application transaction"),
    }
    let info = &entry.commit_info().info;
    expect_string(
        info,
        RELATION_METADATA_KEY,
        kind.durable_relation_id(),
        kind,
    )?;
    expect_string(
        info,
        META_WORKSPACE_ID,
        &lower_hex(publication.workspace.workspace_id.as_bytes()),
        kind,
    )?;
    expect_string(
        info,
        META_EPOCH_ID,
        &lower_hex(publication.candidate_pins.epoch.as_bytes()),
        kind,
    )?;
    expect_string(
        info,
        META_PROOF_SET_ID,
        &lower_hex(publication.proof_set_id.as_bytes()),
        kind,
    )?;
    expect_string(
        info,
        META_SCHEMA_DIGEST,
        &lower_hex(&schema_digest(kind)?),
        kind,
    )?;
    let row_count_u64 = info
        .get(META_ROW_COUNT)
        .and_then(Value::as_u64)
        .ok_or_else(|| ProofRelationsDeltaError::CommitEvidence {
            relation: kind,
            detail: format!("{META_ROW_COUNT} is missing or not an unsigned integer"),
        })?;
    let row_count = usize::try_from(row_count_u64).map_err(|source| {
        ProofRelationsDeltaError::CommitEvidence {
            relation: kind,
            detail: source.to_string(),
        }
    })?;
    let digest = info
        .get(META_BATCH_DIGEST)
        .and_then(Value::as_str)
        .ok_or_else(|| ProofRelationsDeltaError::CommitEvidence {
            relation: kind,
            detail: format!("{META_BATCH_DIGEST} is missing or not a string"),
        })?;
    let batch_digest = parse_lower_hex::<32>(digest).map_err(|detail| {
        ProofRelationsDeltaError::CommitEvidence {
            relation: kind,
            detail,
        }
    })?;
    let num_retries = info
        .get(OPERATION_METRICS)
        .and_then(Value::as_object)
        .and_then(|metrics| metrics.get(NUM_RETRIES))
        .and_then(Value::as_u64);
    if num_retries != Some(0) {
        return commit_error(kind, "exact commit does not prove zero internal retries");
    }
    Ok(CommitEvidence {
        row_count,
        batch_digest,
    })
}

fn expected_schema(kind: ProofRelationKind) -> SchemaRef {
    match kind {
        ProofRelationKind::ProofRun => proof_run_schema(),
        ProofRelationKind::OracleResult => oracle_schema(),
        ProofRelationKind::CapabilityResult => capability_schema(),
        ProofRelationKind::Expectation => expectation_schema(),
        ProofRelationKind::CoverageResult => coverage_schema(),
        ProofRelationKind::FaultResult => fault_schema(),
        ProofRelationKind::ViolationResult => violation_schema(),
        ProofRelationKind::ProvenanceEdge => provenance_schema(),
        ProofRelationKind::Issue => issue_schema(),
    }
}

fn storage_type(data_type: &DataType) -> DataType {
    match data_type {
        DataType::FixedSizeBinary(_) => DataType::Binary,
        DataType::UInt64 => DataType::Decimal128(20, 0),
        other => other.clone(),
    }
}

fn payload_storage_schema(kind: ProofRelationKind) -> SchemaRef {
    let logical = expected_schema(kind);
    Arc::new(Schema::new(
        logical
            .fields()
            .iter()
            .map(|field| {
                Arc::new(
                    Field::new(
                        field.name(),
                        storage_type(field.data_type()),
                        field.is_nullable(),
                    )
                    .with_metadata(field.metadata().clone()),
                )
            })
            .collect::<Vec<_>>(),
    ))
}

fn history_schema(kind: ProofRelationKind) -> SchemaRef {
    let mut fields = Vec::with_capacity(expected_schema(kind).fields().len() + 2);
    fields.push(Arc::new(Field::new(
        PROOF_SET_FIELD,
        DataType::Binary,
        false,
    )));
    fields.push(Arc::new(Field::new(
        ROW_ORDINAL_FIELD,
        DataType::Int64,
        false,
    )));
    fields.extend(payload_storage_schema(kind).fields().iter().cloned());
    Arc::new(Schema::new(fields))
}

fn build_history_batch(
    kind: ProofRelationKind,
    logical: &RecordBatch,
    proof_set_id: TransactionRef,
) -> Result<RecordBatch, ProofRelationsDeltaError> {
    if logical.schema_ref().as_ref() != expected_schema(kind).as_ref() {
        return row_error(
            kind,
            "logical schema differs from the proof relation contract",
        );
    }
    let row_count = logical.num_rows();
    let storage = payload_storage_schema(kind);
    let mut columns = Vec::with_capacity(logical.num_columns() + 2);
    columns.push(Arc::new(BinaryArray::from_iter_values(std::iter::repeat_n(
        proof_set_id.as_bytes().as_slice(),
        row_count,
    ))) as ArrayRef);
    columns.push(Arc::new(Int64Array::from_iter_values((0..row_count).map(
        |ordinal| i64::try_from(ordinal).expect("one Arrow batch fits signed row ordinals"),
    ))) as ArrayRef);
    for (column, field) in logical.columns().iter().zip(storage.fields()) {
        columns.push(cast(column, field.data_type()).map_err(|source| {
            ProofRelationsDeltaError::Arrow {
                relation: kind,
                detail: source.to_string(),
            }
        })?);
    }
    RecordBatch::try_new(history_schema(kind), columns).map_err(|source| {
        ProofRelationsDeltaError::Arrow {
            relation: kind,
            detail: source.to_string(),
        }
    })
}

fn select_proof_set(
    kind: ProofRelationKind,
    proof_set_id: TransactionRef,
    provider_schema: SchemaRef,
    batches: &[RecordBatch],
) -> Result<RecordBatch, ProofRelationsDeltaError> {
    let schema = history_schema(kind);
    let combined = if batches.is_empty() {
        RecordBatch::new_empty(Arc::clone(&schema))
    } else {
        let provider_batch =
            concat_batches(&provider_schema, batches.iter()).map_err(|source| {
                ProofRelationsDeltaError::Arrow {
                    relation: kind,
                    detail: source.to_string(),
                }
            })?;
        let columns = provider_batch
            .columns()
            .iter()
            .zip(schema.fields())
            .map(|(column, field)| {
                cast(column, field.data_type()).map_err(|source| ProofRelationsDeltaError::Arrow {
                    relation: kind,
                    detail: source.to_string(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        RecordBatch::try_new(Arc::clone(&schema), columns).map_err(|source| {
            ProofRelationsDeltaError::Arrow {
                relation: kind,
                detail: source.to_string(),
            }
        })?
    };
    let set_ids = combined
        .column(0)
        .as_any()
        .downcast_ref::<BinaryArray>()
        .ok_or_else(|| ProofRelationsDeltaError::RowIntegrity {
            relation: kind,
            detail: "proof-set storage column is not Binary".to_owned(),
        })?;
    let ordinals = combined
        .column(1)
        .as_any()
        .downcast_ref::<Int64Array>()
        .ok_or_else(|| ProofRelationsDeltaError::RowIntegrity {
            relation: kind,
            detail: "row-ordinal storage column is not Int64".to_owned(),
        })?;
    let mut selected = (0..combined.num_rows())
        .filter(|&row| !set_ids.is_null(row) && set_ids.value(row) == proof_set_id.as_bytes())
        .map(|row| {
            if ordinals.is_null(row) || ordinals.value(row) < 0 {
                return row_error(kind, "selected row has an invalid ordinal");
            }
            Ok((ordinals.value(row), row))
        })
        .collect::<Result<Vec<_>, _>>()?;
    selected.sort_unstable_by_key(|(ordinal, _)| *ordinal);
    for (expected, (observed, _)) in selected.iter().enumerate() {
        if i64::try_from(expected).ok() != Some(*observed) {
            return row_error(kind, "selected row ordinals are not exactly contiguous");
        }
    }
    let indices = UInt32Array::from(
        selected
            .iter()
            .map(|(_, row)| {
                u32::try_from(*row).map_err(|source| ProofRelationsDeltaError::RowIntegrity {
                    relation: kind,
                    detail: source.to_string(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    let payload_schema = payload_storage_schema(kind);
    let storage_columns = combined
        .columns()
        .iter()
        .skip(2)
        .map(|column| {
            take(column.as_ref(), &indices, None).map_err(|source| {
                ProofRelationsDeltaError::Arrow {
                    relation: kind,
                    detail: source.to_string(),
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let storage =
        RecordBatch::try_new(Arc::clone(&payload_schema), storage_columns).map_err(|source| {
            ProofRelationsDeltaError::Arrow {
                relation: kind,
                detail: source.to_string(),
            }
        })?;
    let logical_schema = expected_schema(kind);
    let logical_columns = storage
        .columns()
        .iter()
        .zip(logical_schema.fields())
        .map(|(column, field)| {
            cast(column, field.data_type()).map_err(|source| ProofRelationsDeltaError::Arrow {
                relation: kind,
                detail: source.to_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    RecordBatch::try_new(logical_schema, logical_columns).map_err(|source| {
        ProofRelationsDeltaError::Arrow {
            relation: kind,
            detail: source.to_string(),
        }
    })
}

fn validate_provider_schema(
    kind: ProofRelationKind,
    observed: &Schema,
    expected: &Schema,
) -> Result<(), ProofRelationsDeltaError> {
    if observed.fields().len() != expected.fields().len() {
        return row_error(kind, "exact provider field count differs from history");
    }
    for (observed, expected) in observed.fields().iter().zip(expected.fields()) {
        let compatible_type = observed.data_type() == expected.data_type()
            || matches!(
                (observed.data_type(), expected.data_type()),
                (DataType::BinaryView, DataType::Binary)
            );
        if observed.name() != expected.name()
            || observed.is_nullable() != expected.is_nullable()
            || !compatible_type
        {
            return row_error(
                kind,
                format!(
                    "exact provider field {:?} differs from expected {:?}",
                    observed, expected
                ),
            );
        }
    }
    Ok(())
}

fn reconstruct_relations(
    candidate_pins: ProofCandidatePins,
    mut outputs: BTreeMap<ProofRelationKind, ProofRelationOutput>,
) -> Result<ProofRelations, ProofRelationsDeltaError> {
    let proof_run = outputs.remove(&ProofRelationKind::ProofRun).ok_or(
        ProofRelationsDeltaError::MissingRelation(ProofRelationKind::ProofRun),
    )?;
    if proof_run.batch.num_rows() != 1 {
        return row_error(
            ProofRelationKind::ProofRun,
            "proof_run must contain exactly one selected row",
        );
    }
    validate_candidate_pins(&proof_run.batch, candidate_pins)?;
    let terminal = terminal_at(&proof_run.batch, 14, 0, ProofRelationKind::ProofRun)?;
    let oracle_results = take_output(&mut outputs, ProofRelationKind::OracleResult)?;
    let oracle_observations = decode_oracle_observations(&oracle_results.batch)?;
    validate_summary_counts(&proof_run.batch, &oracle_results.batch, &outputs)?;
    Ok(ProofRelations {
        terminal,
        candidate_pins,
        oracle_observations: oracle_observations.into(),
        proof_run,
        oracle_results,
        capability_results: take_output(&mut outputs, ProofRelationKind::CapabilityResult)?,
        expectations: take_output(&mut outputs, ProofRelationKind::Expectation)?,
        coverage_results: take_output(&mut outputs, ProofRelationKind::CoverageResult)?,
        fault_results: take_output(&mut outputs, ProofRelationKind::FaultResult)?,
        violation_results: take_output(&mut outputs, ProofRelationKind::ViolationResult)?,
        provenance_edges: take_output(&mut outputs, ProofRelationKind::ProvenanceEdge)?,
        issues: take_output(&mut outputs, ProofRelationKind::Issue)?,
    })
}

fn take_output(
    outputs: &mut BTreeMap<ProofRelationKind, ProofRelationOutput>,
    kind: ProofRelationKind,
) -> Result<ProofRelationOutput, ProofRelationsDeltaError> {
    outputs
        .remove(&kind)
        .ok_or(ProofRelationsDeltaError::MissingRelation(kind))
}

fn validate_candidate_pins(
    batch: &RecordBatch,
    pins: ProofCandidatePins,
) -> Result<(), ProofRelationsDeltaError> {
    let kind = ProofRelationKind::ProofRun;
    let expected = [
        (0, pins.epoch.as_bytes().as_slice()),
        (1, pins.input_release.as_bytes().as_slice()),
        (2, pins.program_release.as_bytes().as_slice()),
        (3, pins.application_release.as_bytes().as_slice()),
        (4, pins.source_authority.as_bytes().as_slice()),
        (6, pins.source_images.as_bytes().as_slice()),
        (7, pins.provider_release.as_bytes().as_slice()),
        (8, pins.provider_set.as_bytes().as_slice()),
        (9, pins.table_versions.as_bytes().as_slice()),
        (10, pins.overlay_segments.as_bytes().as_slice()),
        (11, pins.policy_set.as_bytes().as_slice()),
        (12, pins.resource_envelope.as_bytes().as_slice()),
    ];
    for (column, expected) in expected {
        let array = batch
            .column(column)
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .ok_or_else(|| ProofRelationsDeltaError::RowIntegrity {
                relation: kind,
                detail: format!("candidate pin column {column} is not fixed binary"),
            })?;
        if array.is_null(0) || array.value(0) != expected {
            return row_error(
                kind,
                format!("candidate pin column {column} does not match"),
            );
        }
    }
    let source_generation = batch
        .column(5)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| ProofRelationsDeltaError::RowIntegrity {
            relation: kind,
            detail: "source_generation is not UInt64".to_owned(),
        })?;
    if source_generation.is_null(0) || source_generation.value(0) != pins.source_generation.get() {
        return row_error(kind, "source_generation does not match the publication");
    }
    Ok(())
}

fn decode_oracle_observations(
    batch: &RecordBatch,
) -> Result<Vec<ProofOracleObservation>, ProofRelationsDeltaError> {
    let kind = ProofRelationKind::OracleResult;
    let oracle_ids = fixed_array(batch, 1, kind)?;
    let implementations = fixed_array(batch, 2, kind)?;
    let run_ids = fixed_array(batch, 4, kind)?;
    let mut observations = Vec::with_capacity(batch.num_rows());
    for row in 0..batch.num_rows() {
        let oracle_id =
            OracleId::new(copy_bytes::<16>(oracle_ids.value(row), kind)?).ok_or_else(|| {
                ProofRelationsDeltaError::RowIntegrity {
                    relation: kind,
                    detail: "oracle_id is the zero sentinel".to_owned(),
                }
            })?;
        let implementation =
            OracleImplementationRef::new(copy_bytes::<32>(implementations.value(row), kind)?)
                .ok_or_else(|| ProofRelationsDeltaError::RowIntegrity {
                    relation: kind,
                    detail: "oracle implementation is the zero sentinel".to_owned(),
                })?;
        let run_id = if run_ids.is_null(row) {
            None
        } else {
            Some(
                ProofRunId::new(copy_bytes::<16>(run_ids.value(row), kind)?).ok_or_else(|| {
                    ProofRelationsDeltaError::RowIntegrity {
                        relation: kind,
                        detail: "run_id is the zero sentinel".to_owned(),
                    }
                })?,
            )
        };
        observations.push(ProofOracleObservation {
            epoch: EpochId::from_bytes(copy_bytes::<16>(
                fixed_array(batch, 0, kind)?.value(row),
                kind,
            )?),
            oracle_id,
            implementation,
            run_id,
            status: terminal_at(batch, 5, row, kind)?,
        });
    }
    Ok(observations)
}

fn validate_summary_counts(
    proof_run: &RecordBatch,
    oracle_results: &RecordBatch,
    outputs: &BTreeMap<ProofRelationKind, ProofRelationOutput>,
) -> Result<(), ProofRelationsDeltaError> {
    let kind = ProofRelationKind::ProofRun;
    let count_at = |index: usize| -> Result<u64, ProofRelationsDeltaError> {
        let values = proof_run
            .column(index)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or_else(|| ProofRelationsDeltaError::RowIntegrity {
                relation: kind,
                detail: format!("summary column {index} is not UInt64"),
            })?;
        Ok(values.value(0))
    };
    let oracle_count = u64::try_from(oracle_results.num_rows())
        .map_err(|source| ProofRelationsDeltaError::RelationSet(source.to_string()))?;
    let statuses = oracle_results
        .column(5)
        .as_any()
        .downcast_ref::<Int8Array>()
        .ok_or_else(|| ProofRelationsDeltaError::RowIntegrity {
            relation: ProofRelationKind::OracleResult,
            detail: "status is not Int8".to_owned(),
        })?;
    let pass = statuses
        .values()
        .iter()
        .filter(|&&value| value == 1)
        .count() as u64;
    let fail = statuses
        .values()
        .iter()
        .filter(|&&value| value == 2)
        .count() as u64;
    let unknown = statuses
        .values()
        .iter()
        .filter(|&&value| value == 3)
        .count() as u64;
    let expected = [
        (15, oracle_count),
        (16, pass),
        (17, fail),
        (18, unknown),
        (19, output_rows(outputs, ProofRelationKind::Expectation)?),
        (20, output_rows(outputs, ProofRelationKind::FaultResult)?),
        (
            25,
            output_rows(outputs, ProofRelationKind::CapabilityResult)?,
        ),
    ];
    for (column, expected) in expected {
        if count_at(column)? != expected {
            return row_error(kind, format!("summary column {column} disagrees with rows"));
        }
    }
    Ok(())
}

fn output_rows(
    outputs: &BTreeMap<ProofRelationKind, ProofRelationOutput>,
    kind: ProofRelationKind,
) -> Result<u64, ProofRelationsDeltaError> {
    let rows = outputs
        .get(&kind)
        .ok_or(ProofRelationsDeltaError::MissingRelation(kind))?
        .batch
        .num_rows();
    u64::try_from(rows).map_err(|source| ProofRelationsDeltaError::RelationSet(source.to_string()))
}

fn validate_epoch_column(
    kind: ProofRelationKind,
    batch: &RecordBatch,
    epoch: EpochId,
) -> Result<(), ProofRelationsDeltaError> {
    let epochs = fixed_array(batch, 0, kind)?;
    for row in 0..batch.num_rows() {
        if epochs.is_null(row) || epochs.value(row) != epoch.as_bytes() {
            return row_error(kind, "row names a different fabric epoch");
        }
    }
    Ok(())
}

fn fixed_array(
    batch: &RecordBatch,
    column: usize,
    kind: ProofRelationKind,
) -> Result<&FixedSizeBinaryArray, ProofRelationsDeltaError> {
    batch
        .column(column)
        .as_any()
        .downcast_ref::<FixedSizeBinaryArray>()
        .ok_or_else(|| ProofRelationsDeltaError::RowIntegrity {
            relation: kind,
            detail: format!("column {column} is not fixed binary"),
        })
}

fn terminal_at(
    batch: &RecordBatch,
    column: usize,
    row: usize,
    kind: ProofRelationKind,
) -> Result<ProofTerminalStatus, ProofRelationsDeltaError> {
    let statuses = batch
        .column(column)
        .as_any()
        .downcast_ref::<Int8Array>()
        .ok_or_else(|| ProofRelationsDeltaError::RowIntegrity {
            relation: kind,
            detail: format!("terminal column {column} is not Int8"),
        })?;
    if statuses.is_null(row) {
        return row_error(kind, format!("terminal column {column} is null"));
    }
    match statuses.value(row) {
        1 => Ok(ProofTerminalStatus::Pass),
        2 => Ok(ProofTerminalStatus::Fail),
        3 => Ok(ProofTerminalStatus::Unknown),
        observed => row_error(kind, format!("unknown terminal status code {observed}")),
    }
}

fn copy_bytes<const N: usize>(
    bytes: &[u8],
    kind: ProofRelationKind,
) -> Result<[u8; N], ProofRelationsDeltaError> {
    bytes
        .try_into()
        .map_err(|_| ProofRelationsDeltaError::RowIntegrity {
            relation: kind,
            detail: format!("fixed binary value does not have width {N}"),
        })
}

fn commit_metadata(
    workspace: &ProofDeltaWorkspaceRoot,
    kind: ProofRelationKind,
    epoch: EpochId,
    proof_set_id: TransactionRef,
    row_count: usize,
    schema_digest: [u8; 32],
    batch_digest: [u8; 32],
) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            RELATION_METADATA_KEY.to_owned(),
            Value::String(kind.durable_relation_id().to_owned()),
        ),
        (
            META_WORKSPACE_ID.to_owned(),
            Value::String(lower_hex(workspace.workspace_id.as_bytes())),
        ),
        (
            META_EPOCH_ID.to_owned(),
            Value::String(lower_hex(epoch.as_bytes())),
        ),
        (
            META_PROOF_SET_ID.to_owned(),
            Value::String(lower_hex(proof_set_id.as_bytes())),
        ),
        (
            META_ROW_COUNT.to_owned(),
            Value::from(u64::try_from(row_count).expect("one Arrow batch fits u64")),
        ),
        (
            META_SCHEMA_DIGEST.to_owned(),
            Value::String(lower_hex(&schema_digest)),
        ),
        (
            META_BATCH_DIGEST.to_owned(),
            Value::String(lower_hex(&batch_digest)),
        ),
    ])
}

fn schema_digest(kind: ProofRelationKind) -> Result<[u8; 32], ProofRelationsDeltaError> {
    batch_digest(kind, &RecordBatch::new_empty(expected_schema(kind)))
}

fn batch_digest(
    kind: ProofRelationKind,
    batch: &RecordBatch,
) -> Result<[u8; 32], ProofRelationsDeltaError> {
    let mut bytes = Vec::new();
    {
        let mut writer =
            StreamWriter::try_new(&mut bytes, batch.schema_ref()).map_err(|source| {
                ProofRelationsDeltaError::Arrow {
                    relation: kind,
                    detail: source.to_string(),
                }
            })?;
        writer
            .write(batch)
            .and_then(|()| writer.finish())
            .map_err(|source| ProofRelationsDeltaError::Arrow {
                relation: kind,
                detail: source.to_string(),
            })?;
    }
    Ok(*blake3::hash(&bytes).as_bytes())
}

fn expect_string(
    info: &HashMap<String, Value>,
    key: &'static str,
    expected: &str,
    kind: ProofRelationKind,
) -> Result<(), ProofRelationsDeltaError> {
    if info.get(key).and_then(Value::as_str) != Some(expected) {
        return commit_error(kind, format!("{key} does not match {expected:?}"));
    }
    Ok(())
}

fn commit_error<T>(
    kind: ProofRelationKind,
    detail: impl Into<String>,
) -> Result<T, ProofRelationsDeltaError> {
    Err(ProofRelationsDeltaError::CommitEvidence {
        relation: kind,
        detail: detail.into(),
    })
}

fn row_error<T>(
    kind: ProofRelationKind,
    detail: impl Into<String>,
) -> Result<T, ProofRelationsDeltaError> {
    Err(ProofRelationsDeltaError::RowIntegrity {
        relation: kind,
        detail: detail.into(),
    })
}

fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn parse_lower_hex<const N: usize>(encoded: &str) -> Result<[u8; N], String> {
    if encoded.len() != N * 2 {
        return Err(format!("expected {} lowercase hexadecimal bytes", N * 2));
    }
    let mut output = [0_u8; N];
    for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0]).ok_or_else(|| "invalid lowercase hexadecimal".to_owned())?;
        let low = hex_nibble(pair[1]).ok_or_else(|| "invalid lowercase hexadecimal".to_owned())?;
        output[index] = (high << 4) | low;
    }
    Ok(output)
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(feature = "daemon")]
mod activation_port {
    use async_trait::async_trait;

    use super::*;
    use crate::fabric::activation::FabricEpochPins;
    use crate::fabric::activation_transaction::CandidateProofRequest;
    use crate::fabric::command::{DiagnosticRef, ProofReceiptRef};
    use crate::fabric::programmatic_activation_command_ports::{
        ActivationCandidateProofEvidence, ActivationCandidateProofObservation,
        ActivationCandidateProofRelationsPort,
    };

    /// Production activation proof authority backed only by exact persisted Delta versions.
    pub struct DeltaActivationCandidateProofRelations {
        publication: ProofDeltaHistoryPublication,
        session: Arc<datafusion::execution::SessionState>,
        proof_receipt: Option<ProofReceiptRef>,
        diagnostic: Option<DiagnosticRef>,
        integrity_diagnostic: DiagnosticRef,
    }

    impl std::fmt::Debug for DeltaActivationCandidateProofRelations {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("DeltaActivationCandidateProofRelations")
                .field("publication", &self.publication)
                .field("session_id", &self.session.session_id())
                .finish_non_exhaustive()
        }
    }

    impl DeltaActivationCandidateProofRelations {
        #[must_use]
        pub fn new(
            publication: ProofDeltaHistoryPublication,
            session: Arc<datafusion::execution::SessionState>,
            proof_receipt: Option<ProofReceiptRef>,
            diagnostic: Option<DiagnosticRef>,
            integrity_diagnostic: DiagnosticRef,
        ) -> Self {
            Self {
                publication,
                session,
                proof_receipt,
                diagnostic,
                integrity_diagnostic,
            }
        }

        fn pins_match(proof: ProofCandidatePins, activation: FabricEpochPins) -> bool {
            proof.epoch == activation.epoch
                && proof.input_release == activation.input_release
                && proof.program_release == activation.program_release
                && proof.application_release == activation.application_release
                && proof.source_authority == activation.source_authority
                && proof.source_generation == activation.source_generation
                && proof.provider_release == activation.provider_release
                && proof.provider_set == activation.provider_set
                && proof.table_versions == activation.table_versions
                && proof.overlay_segments == activation.overlay_segments
                && proof.policy_set == activation.policy_set
                && proof.resource_envelope == activation.resource_envelope
        }

        fn unavailable(
            &self,
            request: CandidateProofRequest,
        ) -> ActivationCandidateProofObservation {
            ActivationCandidateProofObservation::Unavailable {
                request,
                diagnostic: self.integrity_diagnostic,
            }
        }
    }

    #[async_trait]
    impl ActivationCandidateProofRelationsPort for DeltaActivationCandidateProofRelations {
        async fn observe_candidate(
            &self,
            request: CandidateProofRequest,
        ) -> ActivationCandidateProofObservation {
            if request.workspace_id != self.publication.workspace.workspace_id
                || !Self::pins_match(self.publication.candidate_pins, request.pins)
            {
                return self.unavailable(request);
            }
            let relations =
                match reopen_proof_relations(Arc::clone(&self.session), &self.publication).await {
                    Ok(relations) => Arc::new(relations),
                    Err(_) => return self.unavailable(request),
                };
            match ActivationCandidateProofEvidence::try_new(
                request,
                relations,
                self.proof_receipt,
                self.diagnostic,
            ) {
                Ok(evidence) => ActivationCandidateProofObservation::Evaluated(evidence),
                Err(_) => self.unavailable(request),
            }
        }
    }
}

#[cfg(feature = "daemon")]
pub use activation_port::DeltaActivationCandidateProofRelations;

#[cfg(test)]
mod tests {
    use datafusion::execution::SessionStateBuilder;
    use datafusion::prelude::SessionConfig;
    use deltalake::delta_datafusion::planner::DeltaPlanner;
    use tempfile::TempDir;

    use super::*;
    use crate::fabric::activation::{OverlaySegmentSetRef, PolicySetRef, TableVersionSetRef};
    use crate::fabric::command::{
        ApplicationReleaseRef, InputReleaseRef, ProgramReleaseRef, ProviderReleaseRef,
        ProviderSetRef, ResourceEnvelopeRef, SourceAuthorityRef, SourceGeneration,
        SourceImageSetRef,
    };
    use crate::fabric::proof::{
        CandidateProofInput, IndependentProofInput, ProofOwnerId, evaluate_candidate_proof,
    };

    fn session() -> Arc<datafusion::execution::SessionState> {
        Arc::new(
            SessionStateBuilder::new()
                .with_default_features()
                .with_config(
                    SessionConfig::new()
                        .set_bool("datafusion.execution.parquet.pushdown_filters", false),
                )
                .with_query_planner(DeltaPlanner::new())
                .build(),
        )
    }

    fn candidate_pins(seed: u8) -> ProofCandidatePins {
        ProofCandidatePins {
            epoch: EpochId::from_bytes([seed; 16]),
            input_release: InputReleaseRef::from_bytes([seed.wrapping_add(1); 32]),
            program_release: ProgramReleaseRef::from_bytes([seed.wrapping_add(2); 32]),
            application_release: ApplicationReleaseRef::from_bytes([seed.wrapping_add(3); 32]),
            source_authority: SourceAuthorityRef::from_bytes([seed.wrapping_add(4); 32]),
            source_generation: SourceGeneration::new(u64::from(seed)),
            source_images: SourceImageSetRef::from_bytes([seed.wrapping_add(5); 32]),
            provider_release: ProviderReleaseRef::from_bytes([seed.wrapping_add(6); 32]),
            provider_set: ProviderSetRef::from_bytes([seed.wrapping_add(7); 32]),
            table_versions: TableVersionSetRef::from_bytes([seed.wrapping_add(8); 32]),
            overlay_segments: OverlaySegmentSetRef::from_bytes([seed.wrapping_add(9); 32]),
            policy_set: PolicySetRef::from_bytes([seed.wrapping_add(10); 32]),
            resource_envelope: ResourceEnvelopeRef::from_bytes([seed.wrapping_add(11); 32]),
        }
    }

    fn relations(seed: u8) -> ProofRelations {
        evaluate_candidate_proof(
            &CandidateProofInput {
                producer_owner: ProofOwnerId::new([seed.wrapping_add(12); 32]).unwrap(),
                candidate_pins: candidate_pins(seed),
                oracle_requests: &[],
                capability_requests: &[],
                capability_requirements: &[],
                oracle_executions: &[],
                violations: &[],
                fault_executions: &[],
                provenance_edges: &[],
            },
            &IndependentProofInput {
                expectations: &[],
                required_faults: &[],
            },
        )
        .expect("minimal unknown proof is a valid complete relation set")
    }

    struct Fixture {
        _temporary: TempDir,
        session: Arc<datafusion::execution::SessionState>,
        relations: ProofRelations,
        publication: ProofDeltaHistoryPublication,
    }

    async fn fixture() -> Fixture {
        let temporary = TempDir::new().expect("proof-history temporary root");
        let workspace_path = temporary.path().join("workspace");
        fs::create_dir_all(&workspace_path).expect("create canonical workspace root");
        let workspace_url =
            Url::from_directory_path(&workspace_path).expect("workspace root file URL");
        let workspace =
            ProofDeltaWorkspaceRoot::try_new(WorkspaceId::from_bytes([0x21; 16]), workspace_url)
                .expect("canonical workspace proof root");
        let targets = provision_proof_relation_histories(workspace)
            .await
            .expect("provision all nine proof histories");
        let session = session();
        let relations = relations(0x31);
        let publication = persist_proof_relations(
            Arc::clone(&session),
            targets,
            ProofDeltaWriteIdentity {
                operation_id: OperationId::from_bytes([0x41; 16]),
                writer_generation: WriterGeneration::new(1).unwrap(),
                proof_set_id: TransactionRef::from_bytes([0x51; 32]),
            },
            &relations,
        )
        .await
        .expect("persist every proof relation");
        Fixture {
            _temporary: temporary,
            session,
            relations,
            publication,
        }
    }

    async fn append_corrupt_duplicate(fixture: &Fixture) -> ProofDeltaHistoryPublication {
        let kind = ProofRelationKind::ProofRun;
        let predecessor = fixture.publication.exact_pin(kind).clone();
        let table = DeltaTableBuilder::from_url(predecessor.canonical_root().clone())
            .unwrap()
            .with_skip_stats(false)
            .with_version(predecessor.version())
            .load()
            .await
            .expect("load exact proof-run predecessor");
        let logical = fixture.relations.relation(kind).batch();
        let history = build_history_batch(kind, logical, fixture.publication.proof_set_id)
            .expect("build deliberately duplicated proof row");
        let context = SessionContext::new_with_state(fixture.session.as_ref().clone());
        let dataframe = context.read_batch(history).unwrap();
        let plan =
            SessionBoundLogicalPlan::try_from_dataframe(Arc::clone(&fixture.session), dataframe)
                .unwrap();
        let spec = ControlledDeltaWriteSpec::new(
            predecessor,
            OperationId::from_bytes([0x61; 16]),
            WriterGeneration::new(2).unwrap(),
            ApplicationTransactionMarker::from_transaction_ref(TransactionRef::from_bytes(
                [0x71; 32],
            )),
            ControlledDeltaWriteMode::Append,
        )
        .with_commit_metadata(commit_metadata(
            &fixture.publication.workspace,
            kind,
            fixture.publication.candidate_pins.epoch,
            fixture.publication.proof_set_id,
            logical.num_rows(),
            schema_digest(kind).unwrap(),
            batch_digest(kind, logical).unwrap(),
        ))
        .unwrap();
        let corrupt_pin = match write_exact_delta_plan(&table, &spec, plan).await {
            ControlledDeltaWriteOutcome::Committed(committed) => committed.committed().clone(),
            outcome => panic!("corrupt fixture append did not commit: {outcome:?}"),
        };
        let mut versions = fixture.publication.versions.clone();
        versions.insert(kind, corrupt_pin);
        ProofDeltaHistoryPublication::try_new(
            fixture.publication.workspace.clone(),
            fixture.publication.candidate_pins,
            fixture.publication.proof_set_id,
            versions,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn exact_process_reopen_restores_all_nine_relations() {
        let fixture = fixture().await;
        let reopened = reopen_proof_relations(session(), &fixture.publication)
            .await
            .expect("fresh DataFusion process state reopens exact Delta proof versions");
        assert_eq!(reopened.terminal(), fixture.relations.terminal());
        assert_eq!(
            reopened.candidate_pins(),
            fixture.relations.candidate_pins()
        );
        for kind in ProofRelationKind::ALL {
            assert_eq!(
                reopened.relation(kind).schema(),
                fixture.relations.relation(kind).schema()
            );
            assert_eq!(
                reopened.relation(kind).batch(),
                fixture.relations.relation(kind).batch()
            );
        }
    }

    #[tokio::test]
    async fn missing_and_wrong_version_vectors_fail_closed() {
        let fixture = fixture().await;
        let mut missing = fixture.publication.versions.clone();
        missing.remove(&ProofRelationKind::Issue);
        assert!(matches!(
            ProofDeltaHistoryPublication::try_new(
                fixture.publication.workspace.clone(),
                fixture.publication.candidate_pins,
                fixture.publication.proof_set_id,
                missing,
            ),
            Err(ProofRelationsDeltaError::MissingRelation(
                ProofRelationKind::Issue
            ))
        ));

        let mut wrong = fixture.publication.versions.clone();
        let proof_run = fixture.publication.exact_pin(ProofRelationKind::ProofRun);
        wrong.insert(
            ProofRelationKind::ProofRun,
            ExactDeltaPin::new(proof_run.canonical_root(), proof_run.version() + 100).unwrap(),
        );
        let wrong = ProofDeltaHistoryPublication::try_new(
            fixture.publication.workspace.clone(),
            fixture.publication.candidate_pins,
            fixture.publication.proof_set_id,
            wrong,
        )
        .unwrap();
        assert!(reopen_proof_relations(session(), &wrong).await.is_err());
    }

    #[tokio::test]
    async fn corrupt_duplicate_rows_fail_closed_after_exact_reopen() {
        let fixture = fixture().await;
        let corrupt = append_corrupt_duplicate(&fixture).await;
        let error = reopen_proof_relations(session(), &corrupt)
            .await
            .expect_err("duplicate rows for one proof set must not reconstruct authority");
        assert!(
            matches!(
                error,
                ProofRelationsDeltaError::RowIntegrity {
                    relation: ProofRelationKind::ProofRun,
                    ..
                } | ProofRelationsDeltaError::CommitEvidence {
                    relation: ProofRelationKind::ProofRun,
                    ..
                }
            ),
            "unexpected fail-closed error: {error:?}"
        );
    }
}
