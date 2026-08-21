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
use datafusion::logical_expr::{col, lit};
use datafusion::prelude::SessionContext;
use deltalake::protocol::SaveMode;

use super::mutation::{
    DurableWriteKind, append_phase, application_id, commit_properties, enforce_write_kind,
    reconcile_prepared, reload_table, storage_batch,
};
use super::{
    DeltaAccessProfile, FabricError, LocalProviderFactory, WorkspaceFabric, exact_provider,
};
use crate::fabric::{
    MutationJournal, MutationPhase, MutationPhaseSpec, OwnerMutationRequest, batch_checksum,
};
use crate::fact_ingest::ValidatedFactBatch;
use crate::registries::{
    DURABLE_PUBLICATION_STATE_TRANSITIONS, DURABLE_PUBLICATION_STATE_VALUES,
    DurablePublicationState, generated_transition, registry_state_name,
};
use crate::schema_registry::{PublicationPinRole, TableSpec, table_spec, table_specs};

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
    pub pointer: CurrentPublicationRecord,
    pub tables: BTreeMap<i16, PublicationTableRecord>,
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
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric-publication-operation-v1\0");
    hasher.update(&base);
    hasher.update(&table_code.to_be_bytes());
    hasher.update(label.as_bytes());
    let mut id = [0_u8; 16];
    id.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    id
}

fn digest_payload(label: &str, payload: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric-publication-phase-v1\0");
    hasher.update(label.as_bytes());
    hasher.update(payload);
    *hasher.finalize().as_bytes()
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
) -> Result<RecordBatch, FabricError> {
    let batches = SessionContext::new()
        .read_table(Arc::clone(&table.provider))?
        .collect()
        .await?;
    Ok(concat_batches(&spec.arrow_schema, &batches)?)
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
        let batch = collect_table(table, spec).await?;
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

fn binary_set(batch: &RecordBatch, name: &str) -> BTreeSet<Vec<u8>> {
    let index = batch
        .schema()
        .index_of(name)
        .expect("generated binary column");
    batch
        .column(index)
        .as_any()
        .downcast_ref::<BinaryArray>()
        .expect("generated binary physical type")
        .iter()
        .flatten()
        .map(<[u8]>::to_vec)
        .collect()
}

fn validate_references(batches: &BTreeMap<i16, RecordBatch>) -> Result<(), FabricError> {
    let owners = binary_set(&batches[&8], "owner_id");
    let entities = binary_set(&batches[&100], "entity_id");
    for (&table_code, batch) in batches {
        if batch.schema().index_of("owner_id").is_ok()
            && binary_set(batch, "owner_id")
                .iter()
                .any(|owner| !owners.contains(owner))
        {
            return Err(FabricError::PublicationIntegrity(format!(
                "{} contains an unknown owner",
                table_spec(table_code).expect("generated table").name
            )));
        }
    }
    let relations = &batches[&110];
    for endpoint in ["source_id", "target_id"] {
        if binary_set(relations, endpoint)
            .iter()
            .any(|entity| !entities.contains(entity))
        {
            return Err(FabricError::PublicationIntegrity(format!(
                "relation {endpoint} is absent from entity"
            )));
        }
    }
    Ok(())
}

fn validate_candidate(
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
    validate_references(batches)
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
    let batch = collect_table(table, table_spec(7).unwrap()).await?;
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
    let next = CurrentPublicationRecord {
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
    };
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
        if write.request.publication_id != request.pins.publication_id
            || write.request.workspace_id != request.pins.workspace_id
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
    if let Err(error) = validate_candidate(records, batches) {
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
        stage_publication(self, journal, request, fault).await?;
        apply_owner_publication_writes(self, journal, request, writes, fault).await?;
        let (mut records, batches) =
            write_publication_manifest(self, journal, request, fault).await?;
        validate_and_mark_publication(self, journal, request, &mut records, &batches).await?;
        let pointer = complete_publication(self, journal, request, fault).await?;
        Ok(PublicationOutcome {
            publication_id: request.pins.publication_id,
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
    use crate::fact_ingest::{
        EntityRow, FactScope, RelationRow, ValidatedFactBatch, encode_entities, encode_relations,
    };
    use crate::operational_store::OperationalStore;
    use crate::registries::WorkspaceRegistryLifecycle;
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
            analysis_context_id: [2; 16],
            source_generation: 7,
            owner_id: [3; 16],
        }
    }

    fn owner_batch() -> ValidatedFactBatch {
        let spec = table_spec(8).unwrap();
        let columns: Vec<ArrayRef> = vec![
            Arc::new(BinaryArray::from(vec![Some([1; 16].as_slice())])),
            Arc::new(BinaryArray::from(vec![Some([2; 16].as_slice())])),
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

    fn dangling_relation_batch() -> ValidatedFactBatch {
        let row = RelationRow {
            scope: scope(),
            fact_id: [8; 16],
            language: 10,
            relation_family_code: 2,
            relation_kind_code: 10,
            source_id: [44; 16],
            target_id: [45; 16],
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
                workspace_id: [1; 16],
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
        PublicationRequest {
            operation_id: [publication.wrapping_add(100); 16],
            pins: PublicationPins {
                publication_id: [publication; 16],
                workspace_id: [1; 16],
                repository_id: None,
                worktree_id: None,
                source_generation: 7,
                source_inventory_digest: [10; 32],
                analysis_context_set_id: [11; 16],
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
        let batch = collect_table(fabric.table(5).unwrap(), table_spec(5).unwrap())
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
        let batch = collect_table(fabric.table(5).unwrap(), table_spec(5).unwrap())
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
        assert!(validate_candidate(&records, &batches).is_ok());
        let mut missing_record = records.clone();
        missing_record.pop_first();
        assert!(validate_candidate(&missing_record, &batches).is_err());
        let mut missing_batch = batches;
        missing_batch.pop_first();
        assert!(validate_candidate(&records, &missing_batch).is_err());
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
        assert_eq!(counts, [12, 2, 1, 2]);
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
            Err(FabricError::PublicationIntegrity(_))
        ));
        assert_eq!(invalid_fabric.current_publication().await.unwrap(), None);
        assert_eq!(
            state_and_diagnostics(&invalid_fabric, [30; 16]).await,
            (state_code(DurablePublicationState::Failed), 1)
        );

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
}
