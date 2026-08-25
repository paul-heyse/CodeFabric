//! Durable multi-table publication and current-pointer activation.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::{
    Array as _, ArrayRef, BinaryArray, BooleanArray, Int16Array, Int32Array, Int64Array,
    RecordBatch, StringArray, TimestampMicrosecondArray,
};
use arrow_row::{RowConverter, SortField};
use arrow_select::concat::concat_batches;
use datafusion::common::ScalarValue;
use datafusion::datasource::MemTable;
use datafusion::logical_expr::{Expr, JoinType, col, lit};
use datafusion::prelude::SessionContext;
use deltalake::protocol::SaveMode;
use thiserror::Error;

use super::mutation::{
    DurableWriteKind, append_phase, application_id, commit_properties, enforce_write_kind, hex,
    primary_key_checksum, reconcile_prepared, reload_table, storage_batch,
};
use super::{
    DeltaAccessProfile, FabricError, LocalProviderFactory, WorkspaceFabric, exact_provider,
};
use crate::fabric::{
    MutationJournal, MutationPhase, MutationPhaseSpec, OwnerMutationRequest, batch_checksum,
};
use crate::fact_ingest::ValidatedFactBatch;
use crate::identity::context_set_identity;
use crate::registries::{
    DURABLE_PUBLICATION_STATE_TRANSITIONS, DURABLE_PUBLICATION_STATE_VALUES,
    DurablePublicationState, generated_transition, registry_state_name,
};
use crate::schema_registry::{
    PublicationPinRole, TableScopeSpec, TableSpec, foreign_key_contracts, table_scope_spec,
    table_spec, table_specs,
};

const PUBLICATION_REFERENTIAL_INTEGRITY: &str = "PUBLICATION_REFERENTIAL_INTEGRITY";
const CANDIDATE_REFERENCE_COVERAGE: &str = "COMPLETE_CANDIDATE_EFFECTIVE_SNAPSHOT";

/// Closed row selection bound to one publication and every derived serving snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationScope {
    pub workspace_id: [u8; 16],
    pub source_generation: i64,
    pub analysis_context_set_id: [u8; 16],
    pub analysis_context_ids: Vec<[u8; 16]>,
}

/// Immutable identity and environment pins for one durable publication attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationPins {
    pub publication_id: [u8; 16],
    pub workspace_id: [u8; 16],
    pub repository_id: Option<[u8; 16]>,
    pub worktree_id: Option<[u8; 16]>,
    pub source_generation: i64,
    pub source_inventory_digest: [u8; 32],
    pub analysis_context_set_id: [u8; 16],
    pub analysis_context_ids: Vec<[u8; 16]>,
    pub git_state_fingerprint: Option<[u8; 32]>,
    pub inclusion_policy_fingerprint: [u8; 32],
    pub base_fact_digest: [u8; 32],
    pub derived_fact_digest: Option<[u8; 32]>,
    pub ontology_version: String,
    pub schema_bundle_version: String,
    pub provider_bundle_version: String,
    pub derivation_bundle_version: String,
    pub toolchain_bundle_version: String,
}

/// Expected pointer and timestamps for one idempotent publication operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationRequest {
    pub operation_id: [u8; 16],
    pub pins: PublicationPins,
    pub expected_pointer: Option<CurrentPublicationRecord>,
    pub expected_publication_table_version: Option<u64>,
    pub expected_manifest_table_version: Option<u64>,
    pub expected_pointer_table_version: Option<u64>,
    pub started_at_micros: i64,
    pub completed_at_micros: i64,
}

/// One owner-scoped batch committed before manifest sealing.
pub struct OwnerPublicationWrite {
    pub request: OwnerMutationRequest,
    pub batch: ValidatedFactBatch,
}

/// Exact immutable data-table entry in a publication manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationTableRecord {
    pub publication_id: [u8; 16],
    pub workspace_id: [u8; 16],
    pub table_code: i16,
    pub table_uri: String,
    pub delta_version: u64,
    pub schema_fingerprint: [u8; 32],
    pub row_count: i64,
    pub owner_count: i64,
    pub table_checksum: [u8; 32],
    pub primary_key_digest: [u8; 32],
    pub required: bool,
    pub validated: bool,
}

/// One durable base pointer for the workspace-local Delta namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentPublicationRecord {
    pub workspace_id: [u8; 16],
    pub publication_id: [u8; 16],
    pub pointer_generation: i64,
    pub updated_at_micros: i64,
}

/// Coherent result returned only after pointer read-back succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationOutcome {
    pub publication_id: [u8; 16],
    pub scope: PublicationScope,
    pub pointer: CurrentPublicationRecord,
    pub tables: BTreeMap<i16, PublicationTableRecord>,
}

/// Registered cross-table failure over the complete candidate publication relation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error(
    "{error_code}:source={source_table}.{source_column}:target={target_table}.{target_column}:key={key}:owner_scope={owner_scope}:coverage={coverage}"
)]
pub struct PublicationReferenceViolation {
    pub error_code: &'static str,
    pub source_table: &'static str,
    pub source_column: &'static str,
    pub target_table: &'static str,
    pub target_column: &'static str,
    pub key: String,
    pub owner_scope: String,
    pub coverage: &'static str,
}

impl PublicationPins {
    fn scope(&self) -> Result<PublicationScope, FabricError> {
        if self.analysis_context_ids.is_empty() {
            return Err(FabricError::PublicationIntegrity(
                "publication context membership is empty".into(),
            ));
        }
        let mut normalized = self.analysis_context_ids.clone();
        normalized.sort_unstable();
        normalized.dedup();
        if normalized != self.analysis_context_ids {
            return Err(FabricError::PublicationIntegrity(
                "publication context membership is not sorted and unique".into(),
            ));
        }
        let derived = context_set_identity(self.workspace_id, &normalized)
            .map_err(|error| FabricError::PublicationIntegrity(error.to_string()))?;
        if derived.id != self.analysis_context_set_id {
            return Err(FabricError::PublicationIntegrity(
                "publication context-set identity does not match its members".into(),
            ));
        }
        Ok(PublicationScope {
            workspace_id: self.workspace_id,
            source_generation: self.source_generation,
            analysis_context_set_id: self.analysis_context_set_id,
            analysis_context_ids: normalized,
        })
    }
}

/// Registered deterministic crash seams in the durable publication protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationFaultPoint {
    AfterStaging,
    AfterOwnerWrites,
    AfterManifestWrite,
    BeforePointerCommit,
    AfterPointerCommit,
}

impl PublicationFaultPoint {
    /// Closed fault registry used by restart/recovery tests.
    pub const ALL: [Self; 5] = [
        Self::AfterStaging,
        Self::AfterOwnerWrites,
        Self::AfterManifestWrite,
        Self::BeforePointerCommit,
        Self::AfterPointerCommit,
    ];
}

const fn state_code(state: DurablePublicationState) -> i16 {
    (state as u16).cast_signed()
}

fn derived_operation_id(base: [u8; 16], table_code: i16, label: &str) -> [u8; 16] {
    let mut hasher = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::PublicationOperation,
    );
    hasher.update(&base);
    hasher.update(&table_code.to_be_bytes());
    hasher.update(label.as_bytes());
    hasher.finalize_id16()
}

fn digest_payload(label: &str, payload: &[u8]) -> [u8; 32] {
    let mut hasher = crate::integrity::IntegrityHasher::for_domain(
        crate::integrity::IntegrityDomain::PublicationPhase,
    );
    hasher.update(label.as_bytes());
    hasher.update(payload);
    hasher.finalize()
}

fn advanced_version(base: Option<u64>, commits: u64) -> Result<Option<u64>, FabricError> {
    base.map(|version| {
        version.checked_add(commits).ok_or_else(|| {
            FabricError::PublicationIntegrity("Delta version progression exhausted".into())
        })
    })
    .transpose()
}

fn schema_digest_bytes(spec: &TableSpec) -> Result<[u8; 32], FabricError> {
    let payload = spec
        .schema_digest
        .strip_prefix("b3:")
        .filter(|payload| payload.len() == 64)
        .ok_or_else(|| FabricError::TableInvariant {
            table: spec.name.into(),
            detail: "generated schema digest framing is invalid".into(),
        })?;
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&payload[index * 2..index * 2 + 2], 16).map_err(|_| {
            FabricError::TableInvariant {
                table: spec.name.into(),
                detail: "generated schema digest hex is invalid".into(),
            }
        })?;
    }
    Ok(digest)
}

fn publication_batch(
    request: &PublicationRequest,
    state: DurablePublicationState,
    required_table_count: i32,
    published_table_count: i32,
    diagnostic_count: i64,
) -> Result<RecordBatch, FabricError> {
    let spec = table_spec(5).expect("generated publication table");
    let pins = &request.pins;
    let columns: Vec<ArrayRef> = vec![
        Arc::new(BinaryArray::from(vec![Some(
            pins.publication_id.as_slice(),
        )])),
        Arc::new(BinaryArray::from(vec![Some(pins.workspace_id.as_slice())])),
        Arc::new(BinaryArray::from(vec![
            pins.repository_id.as_ref().map(<[u8; 16]>::as_slice),
        ])),
        Arc::new(BinaryArray::from(vec![
            pins.worktree_id.as_ref().map(<[u8; 16]>::as_slice),
        ])),
        Arc::new(Int16Array::from(vec![state_code(state)])),
        Arc::new(Int64Array::from(vec![pins.source_generation])),
        Arc::new(BinaryArray::from(vec![Some(
            pins.source_inventory_digest.as_slice(),
        )])),
        Arc::new(BinaryArray::from(vec![Some(
            pins.analysis_context_set_id.as_slice(),
        )])),
        Arc::new(BinaryArray::from(vec![
            pins.git_state_fingerprint
                .as_ref()
                .map(<[u8; 32]>::as_slice),
        ])),
        Arc::new(BinaryArray::from(vec![Some(
            pins.inclusion_policy_fingerprint.as_slice(),
        )])),
        Arc::new(BinaryArray::from(vec![Some(
            pins.base_fact_digest.as_slice(),
        )])),
        Arc::new(BinaryArray::from(vec![
            pins.derived_fact_digest.as_ref().map(<[u8; 32]>::as_slice),
        ])),
        Arc::new(StringArray::from(vec![pins.ontology_version.as_str()])),
        Arc::new(StringArray::from(vec![pins.schema_bundle_version.as_str()])),
        Arc::new(StringArray::from(vec![
            pins.provider_bundle_version.as_str(),
        ])),
        Arc::new(StringArray::from(vec![
            pins.derivation_bundle_version.as_str(),
        ])),
        Arc::new(StringArray::from(vec![
            pins.toolchain_bundle_version.as_str(),
        ])),
        Arc::new(
            TimestampMicrosecondArray::from(vec![request.started_at_micros]).with_timezone("UTC"),
        ),
        Arc::new(TimestampMicrosecondArray::from(vec![None::<i64>]).with_timezone("UTC")),
        Arc::new(Int32Array::from(vec![required_table_count])),
        Arc::new(Int32Array::from(vec![published_table_count])),
        Arc::new(Int64Array::from(vec![diagnostic_count])),
    ];
    Ok(RecordBatch::try_new(
        Arc::clone(&spec.arrow_schema),
        columns,
    )?)
}

fn publication_table_batch(records: &[PublicationTableRecord]) -> Result<RecordBatch, FabricError> {
    let spec = table_spec(6).expect("generated publication_table");
    let publication_ids = records
        .iter()
        .map(|record| Some(record.publication_id.as_slice()))
        .collect::<Vec<_>>();
    let workspace_ids = records
        .iter()
        .map(|record| Some(record.workspace_id.as_slice()))
        .collect::<Vec<_>>();
    let uris = records
        .iter()
        .map(|record| record.table_uri.as_str())
        .collect::<Vec<_>>();
    let schema_digests = records
        .iter()
        .map(|record| Some(record.schema_fingerprint.as_slice()))
        .collect::<Vec<_>>();
    let checksums = records
        .iter()
        .map(|record| Some(record.table_checksum.as_slice()))
        .collect::<Vec<_>>();
    let primary_key_digests = records
        .iter()
        .map(|record| Some(record.primary_key_digest.as_slice()))
        .collect::<Vec<_>>();
    let columns: Vec<ArrayRef> = vec![
        Arc::new(BinaryArray::from(publication_ids)),
        Arc::new(BinaryArray::from(workspace_ids)),
        Arc::new(Int16Array::from(
            records
                .iter()
                .map(|record| record.table_code)
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(uris)),
        Arc::new(Int64Array::from(
            records
                .iter()
                .map(|record| i64::try_from(record.delta_version))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| {
                    FabricError::PublicationIntegrity("Delta version exceeds i64".into())
                })?,
        )),
        Arc::new(BinaryArray::from(schema_digests)),
        Arc::new(Int64Array::from(
            records
                .iter()
                .map(|record| record.row_count)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Int64Array::from(
            records
                .iter()
                .map(|record| record.owner_count)
                .collect::<Vec<_>>(),
        )),
        Arc::new(BinaryArray::from(checksums)),
        Arc::new(BinaryArray::from(primary_key_digests)),
        Arc::new(BooleanArray::from(
            records
                .iter()
                .map(|record| record.required)
                .collect::<Vec<_>>(),
        )),
        Arc::new(BooleanArray::from(
            records
                .iter()
                .map(|record| record.validated)
                .collect::<Vec<_>>(),
        )),
    ];
    Ok(RecordBatch::try_new(
        Arc::clone(&spec.arrow_schema),
        columns,
    )?)
}

fn current_pointer_batch(record: &CurrentPublicationRecord) -> Result<RecordBatch, FabricError> {
    let spec = table_spec(7).expect("generated current_publication table");
    let columns: Vec<ArrayRef> = vec![
        Arc::new(BinaryArray::from(vec![Some(
            record.workspace_id.as_slice(),
        )])),
        Arc::new(BinaryArray::from(vec![Some(
            record.publication_id.as_slice(),
        )])),
        Arc::new(Int64Array::from(vec![record.pointer_generation])),
        Arc::new(
            TimestampMicrosecondArray::from(vec![record.updated_at_micros]).with_timezone("UTC"),
        ),
    ];
    Ok(RecordBatch::try_new(
        Arc::clone(&spec.arrow_schema),
        columns,
    )?)
}

fn phase(
    request: &PublicationRequest,
    table_code: i16,
    mutation_phase: MutationPhase,
    label: &str,
    input_checksum: [u8; 32],
    expected_output_checksum: [u8; 32],
    expected_predecessor: Option<u64>,
) -> Result<MutationPhaseSpec, FabricError> {
    Ok(MutationPhaseSpec {
        operation_id: derived_operation_id(request.operation_id, table_code, label),
        publication_id: request.pins.publication_id,
        table_code,
        phase: mutation_phase,
        application_id: application_id(request.pins.workspace_id, table_code, mutation_phase)?,
        owner_set_fingerprint: [0; 32],
        input_checksum,
        expected_output_checksum,
        expected_predecessor,
    })
}

async fn collect_table(
    table: &super::FabricTable,
    spec: &TableSpec,
    filter: Option<Expr>,
) -> Result<RecordBatch, FabricError> {
    let frame = SessionContext::new().read_table(Arc::clone(&table.provider))?;
    let frame = if let Some(filter) = filter {
        frame.filter(filter)?
    } else {
        frame
    };
    let batches = frame.collect().await?;
    Ok(concat_batches(&spec.arrow_schema, &batches)?)
}

pub(super) fn scope_filter(spec: &TableScopeSpec, scope: &PublicationScope) -> Option<Expr> {
    let mut predicates = Vec::new();
    if let Some(column) = spec.workspace_column {
        predicates
            .push(col(column).eq(lit(ScalarValue::Binary(Some(scope.workspace_id.to_vec())))));
    }
    if let Some(column) = spec.source_generation_column {
        predicates.push(col(column).eq(lit(scope.source_generation)));
    }
    if let Some(column) = spec.analysis_context_set_column {
        predicates.push(col(column).eq(lit(ScalarValue::Binary(Some(
            scope.analysis_context_set_id.to_vec(),
        )))));
    }
    if let Some(column) = spec.analysis_context_column {
        let contexts = scope
            .analysis_context_ids
            .iter()
            .fold(None, |combined, context| {
                let predicate = col(column).eq(lit(ScalarValue::Binary(Some(context.to_vec()))));
                Some(combined.map_or(predicate.clone(), |prior: Expr| prior.or(predicate)))
            });
        if let Some(contexts) = contexts {
            predicates.push(contexts);
        }
    }
    predicates.into_iter().reduce(Expr::and)
}

fn distinct_binary(batch: &RecordBatch, column: &str) -> Result<i64, FabricError> {
    let index = batch.schema().index_of(column)?;
    let values = batch
        .column(index)
        .as_any()
        .downcast_ref::<BinaryArray>()
        .ok_or_else(|| FabricError::PublicationIntegrity(format!("{column} is not Binary")))?;
    let count = values
        .iter()
        .flatten()
        .map(<[u8]>::to_vec)
        .collect::<BTreeSet<_>>()
        .len();
    i64::try_from(count)
        .map_err(|_| FabricError::PublicationIntegrity("owner count exceeds i64".into()))
}

async fn manifest_records(
    fabric: &WorkspaceFabric,
    request: &PublicationRequest,
) -> Result<
    (
        BTreeMap<i16, PublicationTableRecord>,
        BTreeMap<i16, RecordBatch>,
    ),
    FabricError,
> {
    let scope = request.pins.scope()?;
    let mut records = BTreeMap::new();
    let mut batches = BTreeMap::new();
    for spec in table_specs()
        .iter()
        .filter(|spec| spec.publication_pin_role == PublicationPinRole::PinnedData)
    {
        let table = fabric
            .table(spec.table_code)
            .ok_or_else(|| FabricError::TableInvariant {
                table: spec.name.into(),
                detail: "pinned publication table is absent".into(),
            })?;
        let delta_version = table.version().ok_or_else(|| {
            FabricError::PublicationIntegrity(format!("{} has no Delta version", spec.name))
        })?;
        let batch = collect_table(
            table,
            spec,
            table_scope_spec(spec.table_code).and_then(|selectors| scope_filter(selectors, &scope)),
        )
        .await?;
        let row_count = i64::try_from(batch.num_rows())
            .map_err(|_| FabricError::PublicationIntegrity("row count exceeds i64".into()))?;
        let owner_count = if spec.arrow_schema.index_of("owner_id").is_ok() {
            distinct_binary(&batch, "owner_id")?
        } else {
            0
        };
        let table_uri = LocalProviderFactory::file_url(&table.path)?.to_string();
        let record = PublicationTableRecord {
            publication_id: request.pins.publication_id,
            workspace_id: request.pins.workspace_id,
            table_code: spec.table_code,
            table_uri,
            delta_version,
            schema_fingerprint: schema_digest_bytes(spec)?,
            row_count,
            owner_count,
            table_checksum: batch_checksum(&batch)?,
            primary_key_digest: primary_key_checksum(&batch, spec)?,
            required: spec.required_for_publication,
            validated: false,
        };
        records.insert(spec.table_code, record);
        batches.insert(spec.table_code, batch);
    }
    Ok((records, batches))
}

fn validate_primary_keys(
    records: &BTreeMap<i16, PublicationTableRecord>,
    batches: &BTreeMap<i16, RecordBatch>,
) -> Result<(), FabricError> {
    for (&table_code, record) in records {
        let spec = table_spec(table_code).expect("generated manifest table");
        let batch = &batches[&table_code];
        let columns = spec
            .primary_key
            .iter()
            .map(|name| Ok(Arc::clone(batch.column(batch.schema().index_of(name)?))))
            .collect::<Result<Vec<_>, arrow_schema::ArrowError>>()?;
        let fields = columns
            .iter()
            .map(|column| SortField::new(column.data_type().clone()))
            .collect();
        let converter = RowConverter::new(fields)?;
        let rows = converter.convert_columns(&columns)?;
        let unique = rows
            .iter()
            .map(|row| row.data().to_vec())
            .collect::<BTreeSet<_>>();
        let observed_count = i64::try_from(batch.num_rows()).map_err(|_| {
            FabricError::PublicationIntegrity("observed row count exceeds i64".into())
        })?;
        if unique.len() != batch.num_rows() || record.row_count != observed_count {
            return Err(FabricError::PublicationIntegrity(format!(
                "{} primary keys or row count are invalid",
                spec.name
            )));
        }
    }
    Ok(())
}

fn validate_identifiers_and_spans(batches: &BTreeMap<i16, RecordBatch>) -> Result<(), FabricError> {
    for (&table_code, batch) in batches {
        let spec = table_spec(table_code).expect("generated manifest table");
        for (index, field) in spec.arrow_schema.fields().iter().enumerate() {
            if field
                .metadata()
                .get("com.codefabric.cpg.id_width")
                .map(String::as_str)
                == Some("16")
            {
                let values = batch
                    .column(index)
                    .as_any()
                    .downcast_ref::<BinaryArray>()
                    .expect("generated id16 is Binary");
                if values.iter().flatten().any(|value| value.len() != 16) {
                    return Err(FabricError::PublicationIntegrity(format!(
                        "{} contains a non-16-byte {}",
                        spec.name,
                        field.name()
                    )));
                }
            }
        }
        if let (Ok(start_index), Ok(end_index)) = (
            batch.schema().index_of("start_byte"),
            batch.schema().index_of("end_byte"),
        ) {
            let starts = batch
                .column(start_index)
                .as_any()
                .downcast_ref::<Int64Array>()
                .expect("generated start_byte is Int64");
            let ends = batch
                .column(end_index)
                .as_any()
                .downcast_ref::<Int64Array>()
                .expect("generated end_byte is Int64");
            for row in 0..batch.num_rows() {
                if starts.is_null(row) != ends.is_null(row)
                    || (!starts.is_null(row)
                        && (starts.value(row) < 0 || ends.value(row) < starts.value(row)))
                {
                    return Err(FabricError::PublicationIntegrity(format!(
                        "{} contains an invalid source span",
                        spec.name
                    )));
                }
            }
        }
    }
    Ok(())
}

fn prospective_pointer(
    request: &PublicationRequest,
) -> Result<CurrentPublicationRecord, FabricError> {
    Ok(CurrentPublicationRecord {
        workspace_id: request.pins.workspace_id,
        publication_id: request.pins.publication_id,
        pointer_generation: request
            .expected_pointer
            .as_ref()
            .map_or(Ok(1), |pointer| {
                pointer.pointer_generation.checked_add(1).ok_or(())
            })
            .map_err(|()| {
                FabricError::CurrentPointerConflict("pointer generation exhausted".into())
            })?,
        updated_at_micros: request.completed_at_micros,
    })
}

fn candidate_effective_batches(
    request: &PublicationRequest,
    records: &BTreeMap<i16, PublicationTableRecord>,
    batches: &BTreeMap<i16, RecordBatch>,
) -> Result<BTreeMap<i16, RecordBatch>, FabricError> {
    let mut candidate = batches.clone();
    let table_count = i32::try_from(records.len())
        .map_err(|_| FabricError::PublicationIntegrity("table census exceeds i32".into()))?;
    candidate.insert(
        5,
        publication_batch(
            request,
            DurablePublicationState::Validating,
            table_count,
            table_count,
            0,
        )?,
    );
    candidate.insert(
        6,
        publication_table_batch(&records.values().cloned().collect::<Vec<_>>())?,
    );
    candidate.insert(7, current_pointer_batch(&prospective_pointer(request)?)?);
    Ok(candidate)
}

fn diagnostic_key(batch: &RecordBatch, column: &str) -> Result<String, FabricError> {
    let array = batch.column(batch.schema().index_of(column)?);
    if let Some(binary) = array.as_any().downcast_ref::<BinaryArray>() {
        return Ok(hex(binary.value(0)));
    }
    Ok(ScalarValue::try_from_array(array, 0)?.to_string())
}

fn owner_scope(batch: &RecordBatch, scope: &PublicationScope) -> Result<String, FabricError> {
    if let Ok(index) = batch.schema().index_of("owner_id") {
        let owners = batch
            .column(index)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| {
                FabricError::PublicationIntegrity("generated owner_id is not Binary".into())
            })?;
        return Ok(format!("owner:{}", hex(owners.value(0))));
    }
    Ok(format!(
        "workspace:{};source_generation:{};analysis_context_set:{};analysis_context_count:{}",
        hex(&scope.workspace_id),
        scope.source_generation,
        hex(&scope.analysis_context_set_id),
        scope.analysis_context_ids.len()
    ))
}

async fn validate_references(
    request: &PublicationRequest,
    batches: &BTreeMap<i16, RecordBatch>,
) -> Result<usize, FabricError> {
    let scope = request.pins.scope()?;
    let mut validated = 0;
    for contract in foreign_key_contracts() {
        let source_spec = table_spec(contract.source_table_code).expect("generated FK source");
        let target_spec = table_spec(contract.target_table_code).expect("generated FK target");
        let source = batches.get(&contract.source_table_code).ok_or_else(|| {
            FabricError::PublicationIntegrity(format!(
                "candidate relation lacks FK source {}",
                source_spec.name
            ))
        })?;
        let target = batches.get(&contract.target_table_code).ok_or_else(|| {
            FabricError::PublicationIntegrity(format!(
                "candidate relation lacks FK target {}",
                target_spec.name
            ))
        })?;
        if source.column(contract.source_column_index).data_type()
            != target.column(contract.target_column_index).data_type()
        {
            return Err(FabricError::PublicationIntegrity(format!(
                "generated FK physical types differ for {}.{} and {}.{}",
                source_spec.name, contract.source_column, target_spec.name, contract.target_column
            )));
        }

        let context = SessionContext::new();
        let source_provider = Arc::new(MemTable::try_new(
            source.schema(),
            vec![vec![source.clone()]],
        )?);
        let target_provider = Arc::new(MemTable::try_new(
            target.schema(),
            vec![vec![target.clone()]],
        )?);
        let mut source_projection = vec![contract.source_column];
        if source.schema().index_of("owner_id").is_ok() && contract.source_column != "owner_id" {
            source_projection.push("owner_id");
        }
        let source_frame = context
            .read_table(source_provider)?
            .filter(col(contract.source_column).is_not_null())?
            .select_columns(&source_projection)?;
        let target_frame = context
            .read_table(target_provider)?
            .select_columns(&[contract.target_column])?;
        let missing = source_frame
            .join(
                target_frame,
                JoinType::LeftAnti,
                &[contract.source_column],
                &[contract.target_column],
                None,
            )?
            .limit(0, Some(1))?
            .collect()
            .await?;
        let Some(missing) = missing.first() else {
            validated += 1;
            continue;
        };
        return Err(PublicationReferenceViolation {
            error_code: PUBLICATION_REFERENTIAL_INTEGRITY,
            source_table: source_spec.name,
            source_column: contract.source_column,
            target_table: target_spec.name,
            target_column: contract.target_column,
            key: diagnostic_key(missing, contract.source_column)?,
            owner_scope: owner_scope(missing, &scope)?,
            coverage: CANDIDATE_REFERENCE_COVERAGE,
        }
        .into());
    }
    Ok(validated)
}

async fn validate_candidate(
    request: &PublicationRequest,
    records: &BTreeMap<i16, PublicationTableRecord>,
    batches: &BTreeMap<i16, RecordBatch>,
) -> Result<(), FabricError> {
    let expected = table_specs()
        .iter()
        .filter(|spec| spec.publication_pin_role == PublicationPinRole::PinnedData)
        .map(|spec| spec.table_code)
        .collect::<BTreeSet<_>>();
    if records.keys().copied().collect::<BTreeSet<_>>() != expected
        || batches.len() != expected.len()
    {
        return Err(FabricError::PublicationIntegrity(
            "publication manifest does not cover the generated pinned-data census".into(),
        ));
    }
    validate_primary_keys(records, batches)?;
    validate_identifiers_and_spans(batches)?;
    validate_references(
        request,
        &candidate_effective_batches(request, records, batches)?,
    )
    .await
    .map(|_| ())
}

struct PublicationTransition<'a> {
    prior: DurablePublicationState,
    event: &'a str,
    guard: &'a str,
    next: DurablePublicationState,
    expected_predecessor: Option<u64>,
}

async fn transition_publication<J: MutationJournal>(
    fabric: &mut WorkspaceFabric,
    journal: &mut J,
    request: &PublicationRequest,
    requested: PublicationTransition<'_>,
) -> Result<Option<u64>, FabricError> {
    let prior_name = registry_state_name(DURABLE_PUBLICATION_STATE_VALUES, requested.prior as u16)
        .expect("generated durable state");
    let next_name = registry_state_name(DURABLE_PUBLICATION_STATE_VALUES, requested.next as u16)
        .expect("generated durable state");
    let transition = generated_transition(
        DURABLE_PUBLICATION_STATE_TRANSITIONS,
        prior_name,
        requested.event,
        requested.guard,
    )
    .map_err(|error| FabricError::PublicationIntegrity(error.error_code.into()))?;
    if transition.to != next_name {
        return Err(FabricError::PublicationIntegrity(
            "generated durable-publication transition target drifted".into(),
        ));
    }
    let table = fabric.tables.get_mut(&5).expect("publication table exists");
    reload_table(table, DeltaAccessProfile::OptimizeDml).await?;
    let prepared = journal
        .prepare(&phase(
            request,
            5,
            MutationPhase::PublicationTransition,
            transition.idempotency_key,
            digest_payload("publication-state", &(requested.prior as u16).to_be_bytes()),
            digest_payload("publication-state", &(requested.next as u16).to_be_bytes()),
            requested.expected_predecessor,
        )?)
        .map_err(FabricError::MutationJournal)?;
    if let Some(version) = reconcile_prepared(table, journal, &prepared).await? {
        return Ok(Some(version));
    }
    if table.delta.version() != prepared.spec.expected_predecessor {
        return Err(FabricError::CurrentPointerConflict(
            "publication state predecessor changed".into(),
        ));
    }
    let predicate = col("publication_id")
        .eq(lit(ScalarValue::Binary(Some(
            request.pins.publication_id.to_vec(),
        ))))
        .and(col("durable_state_code").eq(lit(state_code(requested.prior))));
    let mut update = table
        .delta
        .clone()
        .update()
        .with_predicate(predicate)
        .with_update("durable_state_code", lit(state_code(requested.next)));
    if requested.next == DurablePublicationState::Complete {
        update = update
            .with_update(
                "completed_at",
                lit(ScalarValue::TimestampMicrosecond(
                    Some(request.completed_at_micros),
                    Some(Arc::from("UTC")),
                )),
            )
            .with_update(
                "published_table_count",
                lit(i32::try_from(
                    table_specs()
                        .iter()
                        .filter(|spec| spec.publication_pin_role == PublicationPinRole::PinnedData)
                        .count(),
                )
                .map_err(|_| {
                    FabricError::PublicationIntegrity("table census exceeds i32".into())
                })?),
            );
    } else if requested.next == DurablePublicationState::Failed {
        update = update.with_update("diagnostic_count", lit(1_i64));
    }
    let (delta, metrics) = update
        .with_commit_properties(commit_properties(&prepared))
        .await?;
    if metrics.num_updated_rows != 1 {
        return Err(FabricError::PublicationIntegrity(format!(
            "publication transition updated {} rows",
            metrics.num_updated_rows
        )));
    }
    table.delta = delta;
    table.provider = exact_provider(
        &table.delta,
        table_spec(5).unwrap(),
        DeltaAccessProfile::QueryServing,
    )
    .await?;
    let version = table.delta.version().ok_or_else(|| {
        FabricError::PublicationIntegrity("publication transition returned no version".into())
    })?;
    journal
        .mark_committed(&prepared, version)
        .map_err(FabricError::MutationJournal)?;
    Ok(Some(version))
}

async fn mark_manifest_validated<J: MutationJournal>(
    fabric: &mut WorkspaceFabric,
    journal: &mut J,
    request: &PublicationRequest,
    records: &BTreeMap<i16, PublicationTableRecord>,
) -> Result<(), FabricError> {
    let table = fabric.tables.get_mut(&6).expect("publication_table exists");
    reload_table(table, DeltaAccessProfile::OptimizeDml).await?;
    let mut payload = Vec::with_capacity(records.len() * 34);
    for record in records.values() {
        payload.extend_from_slice(&record.table_code.to_be_bytes());
        payload.extend_from_slice(&record.table_checksum);
    }
    let expected_predecessor = advanced_version(request.expected_manifest_table_version, 1)?;
    let prepared = journal
        .prepare(&phase(
            request,
            6,
            MutationPhase::PublicationTransition,
            "manifest-validated",
            digest_payload("manifest-unvalidated", &payload),
            digest_payload("manifest-validated", &payload),
            expected_predecessor,
        )?)
        .map_err(FabricError::MutationJournal)?;
    if reconcile_prepared(table, journal, &prepared)
        .await?
        .is_some()
    {
        return Ok(());
    }
    let predicate = col("publication_id")
        .eq(lit(ScalarValue::Binary(Some(
            request.pins.publication_id.to_vec(),
        ))))
        .and(col("validated").eq(lit(false)));
    let (delta, metrics) = table
        .delta
        .clone()
        .update()
        .with_predicate(predicate)
        .with_update("validated", lit(true))
        .with_commit_properties(commit_properties(&prepared))
        .await?;
    if metrics.num_updated_rows != records.len() {
        return Err(FabricError::PublicationIntegrity(format!(
            "manifest validation updated {} of {} rows",
            metrics.num_updated_rows,
            records.len()
        )));
    }
    table.delta = delta;
    table.provider = exact_provider(
        &table.delta,
        table_spec(6).unwrap(),
        DeltaAccessProfile::QueryServing,
    )
    .await?;
    let version = table.delta.version().ok_or_else(|| {
        FabricError::PublicationIntegrity("manifest validation returned no version".into())
    })?;
    journal
        .mark_committed(&prepared, version)
        .map_err(FabricError::MutationJournal)
}

async fn read_current_pointer(
    table: &super::FabricTable,
) -> Result<Option<CurrentPublicationRecord>, FabricError> {
    let batch = collect_table(table, table_spec(7).unwrap(), None).await?;
    if batch.num_rows() == 0 {
        return Ok(None);
    }
    if batch.num_rows() != 1 {
        return Err(FabricError::CurrentPointerConflict(
            "current pointer table is not singleton".into(),
        ));
    }
    let binary = |name: &str| -> [u8; 16] {
        let values = batch
            .column(batch.schema().index_of(name).unwrap())
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        <[u8; 16]>::try_from(values.value(0)).unwrap()
    };
    let generation = batch
        .column(batch.schema().index_of("pointer_generation")?)
        .as_any()
        .downcast_ref::<Int64Array>()
        .expect("generated pointer generation")
        .value(0);
    let updated = batch
        .column(batch.schema().index_of("updated_at")?)
        .as_any()
        .downcast_ref::<TimestampMicrosecondArray>()
        .expect("generated pointer timestamp")
        .value(0);
    Ok(Some(CurrentPublicationRecord {
        workspace_id: binary("workspace_id"),
        publication_id: binary("publication_id"),
        pointer_generation: generation,
        updated_at_micros: updated,
    }))
}

async fn commit_pointer<J: MutationJournal>(
    fabric: &mut WorkspaceFabric,
    journal: &mut J,
    request: &PublicationRequest,
) -> Result<CurrentPublicationRecord, FabricError> {
    let spec = table_spec(7).unwrap();
    enforce_write_kind(spec, DurableWriteKind::CurrentPointerSwap)?;
    let table = fabric.tables.get_mut(&7).expect("current pointer exists");
    reload_table(table, DeltaAccessProfile::OptimizeDml).await?;
    let next = prospective_pointer(request)?;
    let batch = current_pointer_batch(&next)?;
    let checksum = batch_checksum(&batch)?;
    let prepared = journal
        .prepare(&phase(
            request,
            7,
            MutationPhase::SingletonUpsert,
            "current-pointer",
            request
                .expected_pointer
                .as_ref()
                .map_or([0; 32], |pointer| {
                    digest_payload(
                        "pointer-predecessor",
                        &pointer.pointer_generation.to_be_bytes(),
                    )
                }),
            checksum,
            request.expected_pointer_table_version,
        )?)
        .map_err(FabricError::CurrentPointerConflict)?;
    if reconcile_prepared(table, journal, &prepared)
        .await?
        .is_none()
    {
        if table.delta.version() != request.expected_pointer_table_version {
            return Err(FabricError::CurrentPointerConflict(
                "pointer Delta predecessor changed".into(),
            ));
        }
        if read_current_pointer(table).await? != request.expected_pointer {
            return Err(FabricError::CurrentPointerConflict(
                "pointer publication or generation predecessor changed".into(),
            ));
        }
        if table.delta.version() != prepared.spec.expected_predecessor {
            return Err(FabricError::CurrentPointerConflict(
                "pointer predecessor changed before commit".into(),
            ));
        }
        let delta = table
            .delta
            .clone()
            .write([storage_batch(&batch)?])
            .with_save_mode(SaveMode::Overwrite)
            .with_commit_properties(commit_properties(&prepared))
            .await
            .map_err(|error| FabricError::CurrentPointerConflict(error.to_string()))?;
        table.delta = delta;
        table.provider =
            exact_provider(&table.delta, spec, DeltaAccessProfile::QueryServing).await?;
        let version = table.delta.version().ok_or_else(|| {
            FabricError::CurrentPointerConflict("pointer commit returned no version".into())
        })?;
        journal
            .mark_committed(&prepared, version)
            .map_err(FabricError::MutationJournal)?;
    }
    reload_table(table, DeltaAccessProfile::OptimizeDml).await?;
    let verified = read_current_pointer(table).await?;
    if verified.as_ref() != Some(&next) {
        return Err(FabricError::CurrentPointerConflict(
            "pointer post-commit read-back differs".into(),
        ));
    }
    Ok(next)
}

fn inject_fault(
    requested: Option<PublicationFaultPoint>,
    point: PublicationFaultPoint,
) -> Result<(), FabricError> {
    if requested == Some(point) {
        return Err(FabricError::PublicationFault(point));
    }
    Ok(())
}

async fn stage_publication<J: MutationJournal>(
    fabric: &mut WorkspaceFabric,
    journal: &mut J,
    request: &PublicationRequest,
    fault: Option<PublicationFaultPoint>,
) -> Result<(), FabricError> {
    let pin_count = table_specs()
        .iter()
        .filter(|spec| spec.publication_pin_role == PublicationPinRole::PinnedData)
        .count();
    let pin_count_i32 = i32::try_from(pin_count)
        .map_err(|_| FabricError::PublicationIntegrity("table census exceeds i32".into()))?;
    let staging = publication_batch(
        request,
        DurablePublicationState::Staging,
        pin_count_i32,
        0,
        0,
    )?;
    let table = fabric.tables.get_mut(&5).expect("publication table exists");
    reload_table(table, DeltaAccessProfile::OptimizeDml).await?;
    enforce_write_kind(table_spec(5).unwrap(), DurableWriteKind::PublicationAppend)?;
    let checksum = batch_checksum(&staging)?;
    let phase = phase(
        request,
        5,
        MutationPhase::PublicationAppend,
        "staging-row",
        checksum,
        checksum,
        request.expected_publication_table_version,
    )?;
    append_phase(table, journal, phase, &staging).await?;
    inject_fault(fault, PublicationFaultPoint::AfterStaging)
}

async fn apply_owner_publication_writes<J: MutationJournal>(
    fabric: &mut WorkspaceFabric,
    journal: &mut J,
    request: &PublicationRequest,
    writes: &[OwnerPublicationWrite],
    fault: Option<PublicationFaultPoint>,
) -> Result<(), FabricError> {
    for write in writes {
        let publication_scope = request.pins.scope()?;
        if write.request.publication_id != request.pins.publication_id
            || write.request.scope.workspace_id != publication_scope.workspace_id
            || write.request.scope.source_generation != publication_scope.source_generation
            || !publication_scope
                .analysis_context_ids
                .contains(&write.request.scope.analysis_context_id)
            || write.batch.scope().batch_scope() != write.request.scope
        {
            return Err(FabricError::PublicationIntegrity(
                "owner write is outside publication identity".into(),
            ));
        }
        fabric
            .replace_owner_rows(journal, &write.request, &write.batch)
            .await?;
    }
    inject_fault(fault, PublicationFaultPoint::AfterOwnerWrites)
}

async fn write_publication_manifest<J: MutationJournal>(
    fabric: &mut WorkspaceFabric,
    journal: &mut J,
    request: &PublicationRequest,
    fault: Option<PublicationFaultPoint>,
) -> Result<
    (
        BTreeMap<i16, PublicationTableRecord>,
        BTreeMap<i16, RecordBatch>,
    ),
    FabricError,
> {
    transition_publication(
        fabric,
        journal,
        request,
        PublicationTransition {
            prior: DurablePublicationState::Staging,
            event: "outputs-staged",
            guard: "manifest-complete",
            next: DurablePublicationState::Validating,
            expected_predecessor: advanced_version(request.expected_publication_table_version, 1)?,
        },
    )
    .await?;
    let (records, batches) = manifest_records(fabric, request).await?;
    let manifest = publication_table_batch(&records.values().cloned().collect::<Vec<_>>())?;
    let checksum = batch_checksum(&manifest)?;
    let table = fabric.tables.get_mut(&6).expect("publication_table exists");
    reload_table(table, DeltaAccessProfile::OptimizeDml).await?;
    enforce_write_kind(table_spec(6).unwrap(), DurableWriteKind::PublicationAppend)?;
    let phase = phase(
        request,
        6,
        MutationPhase::PublicationAppend,
        "table-manifest",
        checksum,
        checksum,
        request.expected_manifest_table_version,
    )?;
    append_phase(table, journal, phase, &manifest).await?;
    inject_fault(fault, PublicationFaultPoint::AfterManifestWrite)?;
    Ok((records, batches))
}

async fn validate_and_mark_publication<J: MutationJournal>(
    fabric: &mut WorkspaceFabric,
    journal: &mut J,
    request: &PublicationRequest,
    records: &mut BTreeMap<i16, PublicationTableRecord>,
    batches: &BTreeMap<i16, RecordBatch>,
) -> Result<(), FabricError> {
    if let Err(error) = validate_candidate(request, records, batches).await {
        transition_publication(
            fabric,
            journal,
            request,
            PublicationTransition {
                prior: DurablePublicationState::Validating,
                event: "validation-failed",
                guard: "terminal-validation-error",
                next: DurablePublicationState::Failed,
                expected_predecessor: advanced_version(
                    request.expected_publication_table_version,
                    2,
                )?,
            },
        )
        .await?;
        return Err(error);
    }
    mark_manifest_validated(fabric, journal, request, records).await?;
    for record in records.values_mut() {
        record.validated = true;
    }
    transition_publication(
        fabric,
        journal,
        request,
        PublicationTransition {
            prior: DurablePublicationState::Validating,
            event: "validation-passed",
            guard: "constraints-green",
            next: DurablePublicationState::Validated,
            expected_predecessor: advanced_version(request.expected_publication_table_version, 2)?,
        },
    )
    .await?;
    Ok(())
}

async fn complete_publication<J: MutationJournal>(
    fabric: &mut WorkspaceFabric,
    journal: &mut J,
    request: &PublicationRequest,
    fault: Option<PublicationFaultPoint>,
) -> Result<CurrentPublicationRecord, FabricError> {
    transition_publication(
        fabric,
        journal,
        request,
        PublicationTransition {
            prior: DurablePublicationState::Validated,
            event: "pointer-lease-held",
            guard: "predecessor-matches",
            next: DurablePublicationState::Committing,
            expected_predecessor: advanced_version(request.expected_publication_table_version, 3)?,
        },
    )
    .await?;
    transition_publication(
        fabric,
        journal,
        request,
        PublicationTransition {
            prior: DurablePublicationState::Committing,
            event: "commit-complete",
            guard: "durable-commit-visible",
            next: DurablePublicationState::Complete,
            expected_predecessor: advanced_version(request.expected_publication_table_version, 4)?,
        },
    )
    .await?;
    inject_fault(fault, PublicationFaultPoint::BeforePointerCommit)?;
    let pointer = commit_pointer(fabric, journal, request).await?;
    inject_fault(fault, PublicationFaultPoint::AfterPointerCommit)?;
    Ok(pointer)
}

impl WorkspaceFabric {
    /// Return the current durable base pointer without exposing mutable Delta handles.
    ///
    /// # Errors
    ///
    /// Rejects a non-singleton or physically invalid pointer table.
    pub async fn current_publication(
        &self,
    ) -> Result<Option<CurrentPublicationRecord>, FabricError> {
        let table = self.table(7).ok_or_else(|| FabricError::TableInvariant {
            table: "current_publication".into(),
            detail: "generated pointer table is absent".into(),
        })?;
        read_current_pointer(table).await
    }

    /// Execute one idempotent durable publication and pointer activation.
    ///
    /// # Errors
    ///
    /// Returns typed mutation, integrity, lifecycle, pointer-CAS, or injected-fault
    /// failures. Intermediate table versions never change the durable pointer.
    pub async fn publish<J: MutationJournal>(
        &mut self,
        journal: &mut J,
        request: &PublicationRequest,
        writes: &[OwnerPublicationWrite],
    ) -> Result<PublicationOutcome, FabricError> {
        self.publish_with_fault(journal, request, writes, None)
            .await
    }

    /// Mark a staged publication superseded without changing the current pointer.
    ///
    /// # Errors
    ///
    /// Rejects a missing/non-staging publication, lifecycle drift, or journal conflict.
    pub async fn abandon_publication<J: MutationJournal>(
        &mut self,
        journal: &mut J,
        request: &PublicationRequest,
    ) -> Result<(), FabricError> {
        transition_publication(
            self,
            journal,
            request,
            PublicationTransition {
                prior: DurablePublicationState::Staging,
                event: "abandoned",
                guard: "superseded",
                next: DurablePublicationState::Abandoned,
                expected_predecessor: advanced_version(
                    request.expected_publication_table_version,
                    1,
                )?,
            },
        )
        .await?;
        Ok(())
    }

    async fn publish_with_fault<J: MutationJournal>(
        &mut self,
        journal: &mut J,
        request: &PublicationRequest,
        writes: &[OwnerPublicationWrite],
        fault: Option<PublicationFaultPoint>,
    ) -> Result<PublicationOutcome, FabricError> {
        let scope = request.pins.scope()?;
        stage_publication(self, journal, request, fault).await?;
        apply_owner_publication_writes(self, journal, request, writes, fault).await?;
        let (mut records, batches) =
            write_publication_manifest(self, journal, request, fault).await?;
        validate_and_mark_publication(self, journal, request, &mut records, &batches).await?;
        let pointer = complete_publication(self, journal, request, fault).await?;
        Ok(PublicationOutcome {
            publication_id: request.pins.publication_id,
            scope,
            pointer,
            tables: records,
        })
    }
}

#[cfg(all(test, feature = "daemon"))]
mod tests {
    use std::path::Path;

    use arrow_array::{ArrayRef, BinaryArray, Int16Array, Int64Array, RecordBatch};

    use super::*;
    use crate::fabric::{EmptySnapshotOverlay, SnapshotProviderCatalog};
    use crate::fact_ingest::{
        EntityRow, FactScope, RelationRow, ValidatedFactBatch, encode_entities, encode_relations,
    };
    use crate::operational_store::OperationalStore;
    use crate::registries::{PUBLIC_ERROR_IDS, WorkspaceRegistryLifecycle};
    use crate::workspace_registry::WorkspaceRecord;

    fn workspace_record() -> WorkspaceRecord {
        WorkspaceRecord {
            workspace_id: [1; 16],
            workspace_registration_nonce: [2; 16],
            registration_revision: 1,
            administrative_key: vec![3],
            root_path_bytes: b"/workspace".to_vec(),
            root_path_display: "/workspace".into(),
            root_directory_file_identity: vec![4],
            platform_code: 2,
            case_sensitivity_mode: "sensitive".into(),
            authorization_revision: 1,
            allowed_source_disclosure_rules: Vec::new(),
            repository_id: None,
            worktree_id: None,
            authorization_fingerprint: [5; 32],
            context_fingerprint: [6; 32],
            status: WorkspaceRegistryLifecycle::Bootstrapping,
            created_at: "00000000000000001000".into(),
            updated_at: "00000000000000001000".into(),
        }
    }

    const fn scope() -> FactScope {
        FactScope {
            workspace_id: [1; 16],
            analysis_context_id: crate::identity::SOURCE_CONTEXT_ID,
            source_generation: 7,
            owner_id: [3; 16],
        }
    }

    fn owner_batch() -> ValidatedFactBatch {
        let spec = table_spec(8).unwrap();
        let columns: Vec<ArrayRef> = vec![
            Arc::new(BinaryArray::from(vec![Some([1; 16].as_slice())])),
            Arc::new(BinaryArray::from(vec![Some(
                crate::identity::SOURCE_CONTEXT_ID.as_slice(),
            )])),
            Arc::new(Int64Array::from(vec![7_i64])),
            Arc::new(BinaryArray::from(vec![Some([3; 16].as_slice())])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>])),
            Arc::new(Int16Array::from(vec![3_i16])),
            Arc::new(Int16Array::from(vec![10_i16])),
            Arc::new(Int16Array::from(vec![10_i16])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>])),
            Arc::new(Int64Array::from(vec![0_i64])),
            Arc::new(Int64Array::from(vec![0_i64])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>])),
            Arc::new(BinaryArray::from(vec![None::<&[u8]>])),
            Arc::new(Int64Array::from(vec![0_i64])),
        ];
        let batch = RecordBatch::try_new(Arc::clone(&spec.arrow_schema), columns).unwrap();
        ValidatedFactBatch::validate(8, batch, scope()).unwrap()
    }

    fn entity_batch(entity_id: [u8; 16]) -> ValidatedFactBatch {
        let row = EntityRow {
            scope: scope(),
            entity_id,
            language: 10,
            entity_family_code: 1,
            entity_kind_code: 10,
            raw_kind_code: None,
            file_id: None,
            start_byte: Some(0),
            end_byte: Some(0),
            name: Some("entity".into()),
            qualified_name: None,
            parent_entity_id: None,
            type_id: None,
            flags: 0,
            fact_hash64: i64::from(entity_id[0]),
        };
        ValidatedFactBatch::validate(100, encode_entities(&[row]).unwrap(), scope()).unwrap()
    }

    fn empty_entity_batch() -> ValidatedFactBatch {
        ValidatedFactBatch::validate(100, encode_entities(&[]).unwrap(), scope()).unwrap()
    }

    fn relation_batch(source_id: [u8; 16], target_id: [u8; 16]) -> ValidatedFactBatch {
        let row = RelationRow {
            scope: scope(),
            fact_id: [8; 16],
            language: 10,
            relation_family_code: 2,
            relation_kind_code: 10,
            source_id,
            target_id,
            ordinal: None,
            role_code: None,
            distance: None,
            directness_code: 10,
            file_id: None,
            start_byte: Some(0),
            end_byte: Some(0),
            certainty_code: 10,
            resolution_code: 10,
            producer_code: 10,
            derivation_code: None,
            flags: 0,
            fact_hash64: 8,
        };
        ValidatedFactBatch::validate(110, encode_relations(&[row]).unwrap(), scope()).unwrap()
    }

    fn dangling_relation_batch() -> ValidatedFactBatch {
        relation_batch([44; 16], [45; 16])
    }

    fn operation_id(publication: u8, table_code: i16) -> [u8; 16] {
        [publication ^ table_code.to_be_bytes()[1]; 16]
    }

    fn owner_write(
        fabric: &WorkspaceFabric,
        publication: u8,
        table_code: i16,
        batch: ValidatedFactBatch,
    ) -> OwnerPublicationWrite {
        OwnerPublicationWrite {
            request: OwnerMutationRequest {
                scope: scope().batch_scope(),
                publication_id: [publication; 16],
                operation_id: operation_id(publication, table_code),
                table_code,
                owner_ids: vec![[3; 16]],
                expected_predecessor: fabric.table(table_code).unwrap().version(),
            },
            batch,
        }
    }

    fn valid_writes(fabric: &WorkspaceFabric, publication: u8) -> Vec<OwnerPublicationWrite> {
        vec![
            owner_write(fabric, publication, 8, owner_batch()),
            owner_write(fabric, publication, 100, entity_batch([4; 16])),
        ]
    }

    async fn request(fabric: &WorkspaceFabric, publication: u8) -> PublicationRequest {
        let analysis_context_ids = vec![crate::identity::SOURCE_CONTEXT_ID];
        let analysis_context_set_id = context_set_identity([1; 16], &analysis_context_ids)
            .unwrap()
            .id;
        PublicationRequest {
            operation_id: [publication.wrapping_add(100); 16],
            pins: PublicationPins {
                publication_id: [publication; 16],
                workspace_id: [1; 16],
                repository_id: None,
                worktree_id: None,
                source_generation: 7,
                source_inventory_digest: [10; 32],
                analysis_context_set_id,
                analysis_context_ids,
                git_state_fingerprint: None,
                inclusion_policy_fingerprint: [12; 32],
                base_fact_digest: [13; 32],
                derived_fact_digest: None,
                ontology_version: "1.3".into(),
                schema_bundle_version: "1.0.0".into(),
                provider_bundle_version: "1.0.0".into(),
                derivation_bundle_version: "1.0.0".into(),
                toolchain_bundle_version: "1.0.0".into(),
            },
            expected_pointer: fabric.current_publication().await.unwrap(),
            expected_publication_table_version: fabric.table(5).unwrap().version(),
            expected_manifest_table_version: fabric.table(6).unwrap().version(),
            expected_pointer_table_version: fabric.table(7).unwrap().version(),
            started_at_micros: i64::from(publication) * 1_000,
            completed_at_micros: i64::from(publication) * 1_000 + 500,
        }
    }

    async fn state_and_diagnostics(
        fabric: &WorkspaceFabric,
        publication_id: [u8; 16],
    ) -> (i16, i64) {
        let batch = collect_table(fabric.table(5).unwrap(), table_spec(5).unwrap(), None)
            .await
            .unwrap();
        let ids = batch
            .column(batch.schema().index_of("publication_id").unwrap())
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        let states = batch
            .column(batch.schema().index_of("durable_state_code").unwrap())
            .as_any()
            .downcast_ref::<Int16Array>()
            .unwrap();
        let diagnostics = batch
            .column(batch.schema().index_of("diagnostic_count").unwrap())
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap();
        let row = (0..batch.num_rows())
            .find(|&row| ids.value(row) == publication_id)
            .unwrap();
        (states.value(row), diagnostics.value(row))
    }

    async fn completion_metadata(
        fabric: &WorkspaceFabric,
        publication_id: [u8; 16],
    ) -> (i32, Option<i64>) {
        let batch = collect_table(fabric.table(5).unwrap(), table_spec(5).unwrap(), None)
            .await
            .unwrap();
        let ids = batch
            .column(batch.schema().index_of("publication_id").unwrap())
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        let published = batch
            .column(batch.schema().index_of("published_table_count").unwrap())
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        let completed = batch
            .column(batch.schema().index_of("completed_at").unwrap())
            .as_any()
            .downcast_ref::<TimestampMicrosecondArray>()
            .unwrap();
        let row = (0..batch.num_rows())
            .find(|&row| ids.value(row) == publication_id)
            .unwrap();
        (
            published.value(row),
            (!completed.is_null(row)).then(|| completed.value(row)),
        )
    }

    async fn fixture(root: &Path) -> (WorkspaceFabric, OperationalStore) {
        let fabric = super::super::bootstrap_workspace(root, &workspace_record())
            .await
            .unwrap();
        let journal = OperationalStore::open(&root.join("operations.sqlite3")).unwrap();
        (fabric, journal)
    }

    async fn published_rows(outcome: &PublicationOutcome, table_code: i16) -> usize {
        let catalog = SnapshotProviderCatalog::build(outcome, &EmptySnapshotOverlay)
            .await
            .unwrap();
        SessionContext::new()
            .read_table(catalog.provider(table_code).unwrap())
            .unwrap()
            .collect()
            .await
            .unwrap()
            .iter()
            .map(RecordBatch::num_rows)
            .sum()
    }

    #[tokio::test]
    async fn wp22_behavioral_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let (mut fabric, mut journal) = fixture(root.path()).await;
        let request = request(&fabric, 20).await;
        let writes = valid_writes(&fabric, 20);
        let outcome = fabric
            .publish(&mut journal, &request, &writes)
            .await
            .unwrap();
        assert_eq!(outcome.pointer.pointer_generation, 1);
        assert_eq!(
            outcome.tables.len(),
            table_specs()
                .iter()
                .filter(|spec| spec.publication_pin_role == PublicationPinRole::PinnedData)
                .count()
        );
        assert!(outcome.tables.values().all(|record| record.validated));
        assert_eq!(
            state_and_diagnostics(&fabric, [20; 16]).await,
            (state_code(DurablePublicationState::Complete), 0)
        );
        assert_eq!(
            completion_metadata(&fabric, [20; 16]).await,
            (
                i32::try_from(outcome.tables.len()).unwrap(),
                Some(request.completed_at_micros),
            )
        );
        let (records, batches) = manifest_records(&fabric, &request).await.unwrap();
        assert!(
            validate_candidate(&request, &records, &batches)
                .await
                .is_ok()
        );
        let mut missing_record = records.clone();
        missing_record.pop_first();
        assert!(
            validate_candidate(&request, &missing_record, &batches)
                .await
                .is_err()
        );
        let mut missing_batch = batches;
        missing_batch.pop_first();
        assert!(
            validate_candidate(&request, &records, &missing_batch)
                .await
                .is_err()
        );
        let mut other_scope = request.clone();
        other_scope.pins.source_generation = 8;
        other_scope.pins.analysis_context_ids = vec![[9; 16]];
        other_scope.pins.analysis_context_set_id = context_set_identity(
            other_scope.pins.workspace_id,
            &other_scope.pins.analysis_context_ids,
        )
        .unwrap()
        .id;
        let (other_records, _) = manifest_records(&fabric, &other_scope).await.unwrap();
        assert_eq!(other_records[&100].row_count, 0);
        let duplicate = fabric
            .publish(&mut journal, &request, &writes)
            .await
            .unwrap();
        assert_eq!(duplicate, outcome);
        drop(journal);
        drop(fabric);
        let reopened = super::super::bootstrap_workspace(root.path(), &workspace_record())
            .await
            .unwrap();
        assert_eq!(
            reopened.current_publication().await.unwrap(),
            Some(outcome.pointer)
        );
    }

    #[test]
    fn wp22_structural_acceptance() {
        let counts = table_specs().iter().fold([0_usize; 4], |mut counts, spec| {
            counts[match spec.publication_pin_role {
                PublicationPinRole::PinnedData => 0,
                PublicationPinRole::ManifestControl => 1,
                PublicationPinRole::PointerControl => 2,
                PublicationPinRole::NotPublished => 3,
            }] += 1;
            counts
        });
        assert_eq!(&counts[1..], [2, 1, 2]);
        assert_eq!(counts[0], table_specs().len() - 5);
        assert_eq!(PublicationFaultPoint::ALL.len(), 5);
        assert!(
            DURABLE_PUBLICATION_STATE_TRANSITIONS
                .iter()
                .all(|transition| {
                    !transition.actions.contains(&"write-pointer")
                        && !transition.actions.contains(&"release-lease")
                })
        );
        assert_eq!(
            MutationPhase::PublicationAppend.as_str(),
            "publication-append"
        );
        assert_eq!(MutationPhase::SingletonUpsert.as_str(), "singleton-upsert");
    }

    #[tokio::test]
    async fn wp22_negative_zero_state() {
        let invalid_root = tempfile::tempdir().unwrap();
        let (mut invalid_fabric, mut invalid_journal) = fixture(invalid_root.path()).await;
        let invalid_request = request(&invalid_fabric, 30).await;
        let invalid_writes = vec![
            owner_write(&invalid_fabric, 30, 8, owner_batch()),
            owner_write(&invalid_fabric, 30, 110, dangling_relation_batch()),
        ];
        assert!(matches!(
            invalid_fabric
                .publish(&mut invalid_journal, &invalid_request, &invalid_writes)
                .await,
            Err(FabricError::PublicationReference(_))
        ));
        assert_eq!(invalid_fabric.current_publication().await.unwrap(), None);
        assert_eq!(
            state_and_diagnostics(&invalid_fabric, [30; 16]).await,
            (state_code(DurablePublicationState::Failed), 1)
        );

        let scope_root = tempfile::tempdir().unwrap();
        let (mut scope_fabric, mut scope_journal) = fixture(scope_root.path()).await;
        let mut scope_request = request(&scope_fabric, 33).await;
        scope_request.pins.analysis_context_set_id = [0xff; 16];
        assert!(matches!(
            scope_fabric
                .publish(&mut scope_journal, &scope_request, &[])
                .await,
            Err(FabricError::PublicationIntegrity(_))
        ));

        let scope_request = request(&scope_fabric, 34).await;
        let mut scope_writes = valid_writes(&scope_fabric, 34);
        scope_writes[0].request.scope.source_generation = 99;
        assert!(matches!(
            scope_fabric
                .publish(&mut scope_journal, &scope_request, &scope_writes)
                .await,
            Err(FabricError::PublicationIntegrity(_))
        ));

        let intermediate_root = tempfile::tempdir().unwrap();
        let (mut intermediate, mut journal) = fixture(intermediate_root.path()).await;
        let intermediate_request = request(&intermediate, 31).await;
        let intermediate_writes = valid_writes(&intermediate, 31);
        assert!(matches!(
            intermediate
                .publish_with_fault(
                    &mut journal,
                    &intermediate_request,
                    &intermediate_writes,
                    Some(PublicationFaultPoint::AfterOwnerWrites),
                )
                .await,
            Err(FabricError::PublicationFault(
                PublicationFaultPoint::AfterOwnerWrites
            ))
        ));
        assert_eq!(intermediate.current_publication().await.unwrap(), None);

        intermediate
            .abandon_publication(&mut journal, &intermediate_request)
            .await
            .unwrap();
        assert_eq!(
            state_and_diagnostics(&intermediate, [31; 16]).await,
            (state_code(DurablePublicationState::Abandoned), 0)
        );

        let race_root = tempfile::tempdir().unwrap();
        let (mut race, mut race_journal) = fixture(race_root.path()).await;
        let race_request = request(&race, 32).await;
        let race_writes = valid_writes(&race, 32);
        let first = race
            .publish(&mut race_journal, &race_request, &race_writes)
            .await
            .unwrap();
        let mut stale = race_request.clone();
        stale.operation_id = [99; 16];
        assert!(matches!(
            commit_pointer(&mut race, &mut race_journal, &stale).await,
            Err(FabricError::CurrentPointerConflict(_))
        ));
        assert_eq!(
            race.current_publication().await.unwrap(),
            Some(first.pointer)
        );
        let source = include_str!("publication.rs");
        assert!(!source.contains(&["blind", "retry"].join("_")));
    }

    #[tokio::test]
    async fn wp22_operational_acceptance() {
        for (index, fault) in PublicationFaultPoint::ALL.into_iter().enumerate() {
            let root = tempfile::tempdir().unwrap();
            let (mut fabric, mut journal) = fixture(root.path()).await;
            let publication = u8::try_from(40 + index).unwrap();
            let request = request(&fabric, publication).await;
            let writes = valid_writes(&fabric, publication);
            assert!(matches!(
                fabric
                    .publish_with_fault(&mut journal, &request, &writes, Some(fault))
                    .await,
                Err(FabricError::PublicationFault(observed)) if observed == fault
            ));
            drop(fabric);
            let mut fabric = super::super::bootstrap_workspace(root.path(), &workspace_record())
                .await
                .unwrap();
            let recovered = fabric
                .publish(&mut journal, &request, &writes)
                .await
                .unwrap();
            assert_eq!(
                fabric.current_publication().await.unwrap(),
                Some(recovered.pointer.clone())
            );
            let reader = journal.reader_factory().open().unwrap();
            let (operations, committed): (i64, i64) = reader
                .with_connection(|connection| {
                    connection.query_row(
                        "SELECT COUNT(*), SUM(state_code=20)
                           FROM table_mutation_operation WHERE publication_id=?1",
                        [request.pins.publication_id.as_slice()],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                })
                .unwrap();
            assert!(operations >= 10);
            assert_eq!(operations, committed);
        }
    }

    #[tokio::test]
    async fn wp74_behavioral_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let (mut fabric, mut journal) = fixture(root.path()).await;

        let base_request = request(&fabric, 74).await;
        let base_writes = valid_writes(&fabric, 74);
        let base = fabric
            .publish(&mut journal, &base_request, &base_writes)
            .await
            .unwrap();
        assert_eq!(published_rows(&base, 100).await, 1);

        let unchanged_request = request(&fabric, 75).await;
        let unchanged_writes = vec![owner_write(
            &fabric,
            75,
            110,
            relation_batch([4; 16], [4; 16]),
        )];
        let unchanged = fabric
            .publish(&mut journal, &unchanged_request, &unchanged_writes)
            .await
            .unwrap();
        assert_eq!(published_rows(&unchanged, 110).await, 1);

        let co_arriving_request = request(&fabric, 76).await;
        let co_arriving_writes = vec![
            owner_write(&fabric, 76, 100, entity_batch([5; 16])),
            owner_write(&fabric, 76, 110, relation_batch([5; 16], [5; 16])),
        ];
        let co_arriving = fabric
            .publish(&mut journal, &co_arriving_request, &co_arriving_writes)
            .await
            .unwrap();
        assert_eq!(published_rows(&co_arriving, 100).await, 1);
        assert_eq!(published_rows(&co_arriving, 110).await, 1);

        let replacement_request = request(&fabric, 77).await;
        let replacement_writes = vec![
            owner_write(&fabric, 77, 100, entity_batch([6; 16])),
            owner_write(&fabric, 77, 110, relation_batch([6; 16], [6; 16])),
        ];
        let replacement = fabric
            .publish(&mut journal, &replacement_request, &replacement_writes)
            .await
            .unwrap();
        assert_eq!(replacement.pointer.pointer_generation, 4);
        assert_eq!(published_rows(&replacement, 100).await, 1);
        assert_eq!(published_rows(&replacement, 110).await, 1);
    }

    #[tokio::test]
    async fn wp74_structural_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let (fabric, _journal) = fixture(root.path()).await;
        let request = request(&fabric, 78).await;
        let (records, batches) = manifest_records(&fabric, &request).await.unwrap();
        let candidate = candidate_effective_batches(&request, &records, &batches).unwrap();
        assert_eq!(
            validate_references(&request, &candidate).await.unwrap(),
            foreign_key_contracts().len()
        );
        assert_eq!(foreign_key_contracts().len(), 14);
        assert!(foreign_key_contracts().iter().all(|contract| {
            candidate.contains_key(&contract.source_table_code)
                && candidate.contains_key(&contract.target_table_code)
        }));
        assert!(PUBLIC_ERROR_IDS.contains(&PUBLICATION_REFERENTIAL_INTEGRITY));
    }

    #[tokio::test]
    async fn wp74_negative_zero_state() {
        let root = tempfile::tempdir().unwrap();
        let (mut fabric, mut journal) = fixture(root.path()).await;
        let base_request = request(&fabric, 79).await;
        let base_writes = valid_writes(&fabric, 79);
        let base = fabric
            .publish(&mut journal, &base_request, &base_writes)
            .await
            .unwrap();

        let tombstone_request = request(&fabric, 80).await;
        let tombstone_writes = vec![
            owner_write(&fabric, 80, 100, empty_entity_batch()),
            owner_write(&fabric, 80, 110, relation_batch([4; 16], [4; 16])),
        ];
        let violation = fabric
            .publish(&mut journal, &tombstone_request, &tombstone_writes)
            .await
            .unwrap_err();
        let FabricError::PublicationReference(violation) = violation else {
            panic!("expected registered referential-integrity failure");
        };
        assert_eq!(violation.error_code, PUBLICATION_REFERENTIAL_INTEGRITY);
        assert_eq!(violation.source_table, "relation");
        assert_eq!(violation.target_table, "entity");
        assert_eq!(violation.owner_scope, format!("owner:{}", hex(&[3; 16])));
        assert_eq!(violation.coverage, CANDIDATE_REFERENCE_COVERAGE);
        assert_eq!(
            fabric.current_publication().await.unwrap(),
            Some(base.pointer.clone())
        );
        assert_eq!(published_rows(&base, 100).await, 1);
        assert_eq!(
            state_and_diagnostics(&fabric, [80; 16]).await,
            (state_code(DurablePublicationState::Failed), 1)
        );

        let missing_root = tempfile::tempdir().unwrap();
        let (mut missing, mut missing_journal) = fixture(missing_root.path()).await;
        let missing_request = request(&missing, 81).await;
        let missing_writes = vec![
            owner_write(&missing, 81, 8, owner_batch()),
            owner_write(&missing, 81, 110, dangling_relation_batch()),
        ];
        assert!(matches!(
            missing
                .publish(&mut missing_journal, &missing_request, &missing_writes)
                .await,
            Err(FabricError::PublicationReference(_))
        ));
        assert_eq!(missing.current_publication().await.unwrap(), None);
    }

    #[tokio::test]
    async fn wp74_operational_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let (mut fabric, mut journal) = fixture(root.path()).await;
        let base_request = request(&fabric, 82).await;
        let base_writes = valid_writes(&fabric, 82);
        let base = fabric
            .publish(&mut journal, &base_request, &base_writes)
            .await
            .unwrap();

        let failed_request = request(&fabric, 83).await;
        let failed_writes = vec![
            owner_write(&fabric, 83, 100, empty_entity_batch()),
            owner_write(&fabric, 83, 110, relation_batch([4; 16], [4; 16])),
        ];
        assert!(matches!(
            fabric
                .publish(&mut journal, &failed_request, &failed_writes)
                .await,
            Err(FabricError::PublicationReference(_))
        ));
        assert_eq!(
            fabric.current_publication().await.unwrap(),
            Some(base.pointer.clone())
        );
        assert_eq!(published_rows(&base, 100).await, 1);

        let recovery_request = request(&fabric, 84).await;
        let recovery_writes = vec![
            owner_write(&fabric, 84, 100, entity_batch([9; 16])),
            owner_write(&fabric, 84, 110, relation_batch([9; 16], [9; 16])),
        ];
        let recovered = fabric
            .publish(&mut journal, &recovery_request, &recovery_writes)
            .await
            .unwrap();
        assert_eq!(recovered.pointer.pointer_generation, 2);
        assert_eq!(published_rows(&recovered, 100).await, 1);
        assert_eq!(published_rows(&recovered, 110).await, 1);
        assert_eq!(
            state_and_diagnostics(&fabric, [83; 16]).await,
            (state_code(DurablePublicationState::Failed), 1)
        );
    }

    #[test]
    fn wp05_behavioral_mutation_recovery() {
        wp22_behavioral_acceptance();
        wp22_negative_zero_state();
        wp22_operational_acceptance();
    }
}
