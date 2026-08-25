//! Typed Arrow encoders and the bounded canonical fact-ingest boundary.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arrow_array::builder::{
    BinaryBuilder, BooleanBuilder, Float64Builder, Int16Builder, Int32Builder, Int64Builder,
    ListBuilder, StringBuilder,
};
use arrow_array::{
    Array as _, ArrayRef, BinaryArray, Int16Array, Int32Array, Int64Array, ListArray, RecordBatch,
};
use arrow_row::{RowConverter, SortField};
use arrow_schema::{ArrowError, DataType};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::schema_registry::{DurableMutationClass, TableSpec, table_spec};

const MAX_STREAMS: usize = 64;
const MAX_ROWS_PER_STREAM: usize = 65_536;

/// Stable failures at the only canonical fact-ingest boundary.
#[derive(Debug, Error)]
pub enum FactIngestError {
    #[error("SOURCE_SNAPSHOT_MISMATCH:{0}")]
    SourceSnapshotMismatch(String),
    #[error("STALE_RESULT:{0}")]
    StaleResult(String),
    #[error("FACT_BATCH_INVALID:{table}:{check}:{detail}")]
    BatchInvalid {
        table: String,
        check: &'static str,
        detail: String,
    },
    #[error("OBSERVATION_PROTOCOL_INVALID:{0}")]
    Protocol(String),
    #[error(transparent)]
    Identity(#[from] crate::identity::IdentityError),
    #[error(transparent)]
    Arrow(#[from] ArrowError),
}

/// Workspace/context/generation fields carried by every canonical fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FactScope {
    pub workspace_id: [u8; 16],
    pub analysis_context_id: [u8; 16],
    pub source_generation: i64,
    pub owner_id: [u8; 16],
}

/// One canonical replacement owner anchoring every owner-scoped fact row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerRow {
    pub scope: FactScope,
    pub parent_owner_id: Option<[u8; 16]>,
    pub owner_kind_code: i16,
    pub language: i16,
    pub file_id: Option<[u8; 16]>,
    pub semantic_entity_id: Option<[u8; 16]>,
    pub start_byte: Option<i64>,
    pub end_byte: Option<i64>,
    pub source_fingerprint: Option<[u8; 32]>,
    pub semantic_fingerprint: Option<[u8; 32]>,
    pub capability_mask: i64,
}

/// One explicit capability/completeness claim for a canonical owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityStatusRow {
    pub scope: FactScope,
    pub snapshot_id: Option<[u8; 16]>,
    pub capability_code: i16,
    pub owner_capability_state_code: i16,
    pub completeness_state_code: i16,
    pub provider_run_id: Option<[u8; 16]>,
    pub producer_code: Option<i16>,
    pub reason_code: Option<i16>,
    pub diagnostic_id: Option<[u8; 16]>,
    pub fallback_source_available: bool,
    pub coverage_scope_fingerprint: [u8; 32],
}

/// Scope shared by every owner batch in one publication selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FactBatchScope {
    pub workspace_id: [u8; 16],
    pub analysis_context_id: [u8; 16],
    pub source_generation: i64,
}

impl FactScope {
    #[must_use]
    pub const fn batch_scope(self) -> FactBatchScope {
        FactBatchScope {
            workspace_id: self.workspace_id,
            analysis_context_id: self.analysis_context_id,
            source_generation: self.source_generation,
        }
    }
}

/// One canonical `entity` row.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityRow {
    pub scope: FactScope,
    pub entity_id: [u8; 16],
    pub language: i16,
    pub entity_family_code: i16,
    pub entity_kind_code: i32,
    pub raw_kind_code: Option<i32>,
    pub file_id: Option<[u8; 16]>,
    pub start_byte: Option<i64>,
    pub end_byte: Option<i64>,
    pub name: Option<String>,
    pub qualified_name: Option<String>,
    pub parent_entity_id: Option<[u8; 16]>,
    pub type_id: Option<[u8; 16]>,
    pub flags: i64,
    pub fact_hash64: i64,
}

/// One canonical `relation` row.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationRow {
    pub scope: FactScope,
    pub fact_id: [u8; 16],
    pub language: i16,
    pub relation_family_code: i16,
    pub relation_kind_code: i32,
    pub source_id: [u8; 16],
    pub target_id: [u8; 16],
    pub ordinal: Option<i32>,
    pub role_code: Option<i16>,
    pub distance: Option<i32>,
    pub directness_code: i16,
    pub file_id: Option<[u8; 16]>,
    pub start_byte: Option<i64>,
    pub end_byte: Option<i64>,
    pub certainty_code: i16,
    pub resolution_code: i16,
    pub producer_code: i16,
    pub derivation_code: Option<i16>,
    pub flags: i64,
    pub fact_hash64: i64,
}

/// Closed property representation; its code and populated Arrow column cannot diverge.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    Entity([u8; 16]),
    Boolean(bool),
    Integer(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
    Type([u8; 16]),
}

impl PropertyValue {
    const fn code(&self) -> i16 {
        match self {
            Self::Entity(_) => 10,
            Self::Boolean(_) => 20,
            Self::Integer(_) => 30,
            Self::Float(_) => 40,
            Self::Text(_) => 50,
            Self::Bytes(_) => 60,
            Self::Type(_) => 70,
        }
    }
}

/// One canonical `property_fact` row.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyFactRow {
    pub scope: FactScope,
    pub fact_id: [u8; 16],
    pub subject_entity_id: [u8; 16],
    pub property_kind_code: i32,
    pub program_point_entity_id: Option<[u8; 16]>,
    pub value: PropertyValue,
    pub directness_code: i16,
    pub certainty_code: i16,
    pub resolution_code: i16,
    pub producer_code: i16,
    pub derivation_code: Option<i16>,
    pub file_id: Option<[u8; 16]>,
    pub start_byte: Option<i64>,
    pub end_byte: Option<i64>,
    pub fact_hash64: i64,
}

/// One immutable provenance row for an accepted observation.
#[derive(Clone, Debug, PartialEq)]
pub struct FactEvidenceRow {
    pub evidence_id: [u8; 16],
    pub scope: FactScope,
    pub fact_id: [u8; 16],
    pub fact_form_code: i16,
    pub provider_code: i16,
    pub provider_version: String,
    pub provider_run_id: [u8; 16],
    pub observation_id: [u8; 16],
    pub raw_kind_code: Option<i32>,
    pub file_id: Option<[u8; 16]>,
    pub start_byte: Option<i64>,
    pub end_byte: Option<i64>,
    pub certainty_code: i16,
    pub resolution_code: i16,
    pub conflict_disposition_code: i16,
    pub cold_payload: Option<Vec<u8>>,
}

/// One authoritative `source_file` extension row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFileRow {
    pub scope: FactScope,
    pub file_id: [u8; 16],
    pub path_bytes: Vec<u8>,
    pub path_display: String,
    pub path_encoding_code: i16,
    pub path_case_key: Option<Vec<u8>>,
    pub path_display_is_lossy: bool,
    pub language: i16,
    pub source_digest: [u8; 32],
    pub byte_len: i64,
    pub line_count: i32,
    pub encoding_name: Option<String>,
    pub newline_kind_code: i16,
    pub source_bytes: Vec<u8>,
    pub decoded_text: Option<String>,
    pub line_start_offsets: Vec<i64>,
    pub module_entity_id: Option<[u8; 16]>,
    pub is_stub: bool,
    pub flags: i64,
}

/// One provider-authenticated lexical token row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTokenRow {
    pub scope: FactScope,
    pub token_id: [u8; 16],
    pub file_id: [u8; 16],
    pub ordinal: i32,
    pub token_kind_code: i32,
    pub start_byte: i64,
    pub end_byte: i64,
    pub normalized_value: Option<String>,
    pub flags: i64,
}

/// One source comment, documentation, directive, or recovery annotation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAnnotationRow {
    pub scope: FactScope,
    pub annotation_id: [u8; 16],
    pub file_id: [u8; 16],
    pub annotation_kind_code: i32,
    pub start_byte: i64,
    pub end_byte: i64,
    pub target_entity_id: Option<[u8; 16]>,
    pub text: Option<String>,
    pub diagnostic_code: Option<i32>,
    pub flags: i64,
}

/// One canonical syntax-entity extension row.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Generated FAB §20 columns are independent facts.
pub struct SyntaxDetailRow {
    pub scope: FactScope,
    pub entity_id: [u8; 16],
    pub raw_kind_code: i32,
    pub occurrence_family_code: i16,
    pub reconciliation_step_code: i16,
    pub raw_kind_disposition_code: i16,
    pub normalized_kind_code: i32,
    pub parent_syntax_id: Option<[u8; 16]>,
    pub field_role_code: Option<i16>,
    pub ordinal: Option<i32>,
    pub named: bool,
    pub extra: bool,
    pub error: bool,
    pub missing: bool,
    pub explicitly_parenthesized: bool,
    pub provider_node_flags: i64,
}

fn binary<T>(rows: &[T], mut value: impl for<'a> FnMut(&'a T) -> Option<&'a [u8]>) -> ArrayRef {
    let mut builder = BinaryBuilder::with_capacity(rows.len(), rows.len().saturating_mul(16));
    for row in rows {
        builder.append_option(value(row));
    }
    Arc::new(builder.finish())
}

fn utf8<T>(rows: &[T], mut value: impl for<'a> FnMut(&'a T) -> Option<&'a str>) -> ArrayRef {
    let capacity = rows
        .iter()
        .filter_map(&mut value)
        .map(str::len)
        .sum::<usize>();
    let mut builder = StringBuilder::with_capacity(rows.len(), capacity);
    for row in rows {
        builder.append_option(value(row));
    }
    Arc::new(builder.finish())
}

fn i16s<T>(rows: &[T], mut value: impl FnMut(&T) -> Option<i16>) -> ArrayRef {
    let mut builder = Int16Builder::with_capacity(rows.len());
    for row in rows {
        builder.append_option(value(row));
    }
    Arc::new(builder.finish())
}

fn i32s<T>(rows: &[T], mut value: impl FnMut(&T) -> Option<i32>) -> ArrayRef {
    let mut builder = Int32Builder::with_capacity(rows.len());
    for row in rows {
        builder.append_option(value(row));
    }
    Arc::new(builder.finish())
}

fn i64s<T>(rows: &[T], mut value: impl FnMut(&T) -> Option<i64>) -> ArrayRef {
    let mut builder = Int64Builder::with_capacity(rows.len());
    for row in rows {
        builder.append_option(value(row));
    }
    Arc::new(builder.finish())
}

fn bools<T>(rows: &[T], mut value: impl FnMut(&T) -> Option<bool>) -> ArrayRef {
    let mut builder = BooleanBuilder::with_capacity(rows.len());
    for row in rows {
        builder.append_option(value(row));
    }
    Arc::new(builder.finish())
}

fn f64s<T>(rows: &[T], mut value: impl FnMut(&T) -> Option<f64>) -> ArrayRef {
    let mut builder = Float64Builder::with_capacity(rows.len());
    for row in rows {
        builder.append_option(value(row));
    }
    Arc::new(builder.finish())
}

fn i64_lists<T>(
    table_code: i16,
    column_name: &str,
    rows: &[T],
    mut value: impl for<'a> FnMut(&'a T) -> &'a [i64],
) -> ArrayRef {
    let spec = table_spec(table_code).expect("generated universal fact table");
    let column = spec
        .arrow_schema
        .field_with_name(column_name)
        .expect("generated list column");
    let DataType::List(element) = column.data_type() else {
        panic!("generated {column_name} column must be a list");
    };
    let mut builder = ListBuilder::new(Int64Builder::new()).with_field(Arc::clone(element));
    for row in rows {
        for item in value(row) {
            builder.values().append_value(*item);
        }
        builder.append(true);
    }
    Arc::new(builder.finish())
}

fn fact_batch(
    table_code: i16,
    columns: Vec<(&'static str, ArrayRef)>,
) -> Result<RecordBatch, FactIngestError> {
    let spec = table_spec(table_code).expect("generated universal fact table");
    if columns.len() != spec.arrow_schema.fields().len()
        || columns
            .iter()
            .zip(spec.arrow_schema.fields())
            .any(|((name, _), field)| *name != field.name())
    {
        return Err(invalid(
            spec,
            "encoder-columns",
            "typed encoder columns differ from generated TableSpec",
        ));
    }
    Ok(RecordBatch::try_new(
        Arc::clone(&spec.arrow_schema),
        columns.into_iter().map(|(_, column)| column).collect(),
    )?)
}

/// Encode canonical owners directly into the generated Arrow schema.
///
/// # Errors
///
/// Returns an Arrow error if the generated physical schema and encoder diverge.
pub fn encode_owners(rows: &[OwnerRow]) -> Result<RecordBatch, FactIngestError> {
    fact_batch(
        8,
        vec![
            (
                "workspace_id",
                binary(rows, |row| Some(row.scope.workspace_id.as_slice())),
            ),
            (
                "analysis_context_id",
                binary(rows, |row| Some(row.scope.analysis_context_id.as_slice())),
            ),
            (
                "source_generation",
                i64s(rows, |row| Some(row.scope.source_generation)),
            ),
            (
                "owner_id",
                binary(rows, |row| Some(row.scope.owner_id.as_slice())),
            ),
            (
                "parent_owner_id",
                binary(rows, |row| {
                    row.parent_owner_id.as_ref().map(<[u8; 16]>::as_slice)
                }),
            ),
            (
                "owner_bucket",
                i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            ),
            (
                "owner_kind_code",
                i16s(rows, |row| Some(row.owner_kind_code)),
            ),
            ("language", i16s(rows, |row| Some(row.language))),
            (
                "file_id",
                binary(rows, |row| row.file_id.as_ref().map(<[u8; 16]>::as_slice)),
            ),
            (
                "semantic_entity_id",
                binary(rows, |row| {
                    row.semantic_entity_id.as_ref().map(<[u8; 16]>::as_slice)
                }),
            ),
            ("start_byte", i64s(rows, |row| row.start_byte)),
            ("end_byte", i64s(rows, |row| row.end_byte)),
            (
                "source_fingerprint",
                binary(rows, |row| {
                    row.source_fingerprint.as_ref().map(<[u8; 32]>::as_slice)
                }),
            ),
            (
                "semantic_fingerprint",
                binary(rows, |row| {
                    row.semantic_fingerprint.as_ref().map(<[u8; 32]>::as_slice)
                }),
            ),
            (
                "capability_mask",
                i64s(rows, |row| Some(row.capability_mask)),
            ),
        ],
    )
}

/// Encode explicit owner capability status directly into the generated Arrow schema.
///
/// # Errors
///
/// Returns an Arrow error if the generated physical schema and encoder diverge.
pub fn encode_capability_statuses(
    rows: &[CapabilityStatusRow],
) -> Result<RecordBatch, FactIngestError> {
    fact_batch(
        9,
        vec![
            (
                "workspace_id",
                binary(rows, |row| Some(row.scope.workspace_id.as_slice())),
            ),
            (
                "analysis_context_id",
                binary(rows, |row| Some(row.scope.analysis_context_id.as_slice())),
            ),
            (
                "source_generation",
                i64s(rows, |row| Some(row.scope.source_generation)),
            ),
            (
                "snapshot_id",
                binary(rows, |row| {
                    row.snapshot_id.as_ref().map(<[u8; 16]>::as_slice)
                }),
            ),
            (
                "owner_id",
                binary(rows, |row| Some(row.scope.owner_id.as_slice())),
            ),
            (
                "owner_bucket",
                i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            ),
            (
                "capability_code",
                i16s(rows, |row| Some(row.capability_code)),
            ),
            (
                "owner_capability_state_code",
                i16s(rows, |row| Some(row.owner_capability_state_code)),
            ),
            (
                "completeness_state_code",
                i16s(rows, |row| Some(row.completeness_state_code)),
            ),
            (
                "provider_run_id",
                binary(rows, |row| {
                    row.provider_run_id.as_ref().map(<[u8; 16]>::as_slice)
                }),
            ),
            ("producer_code", i16s(rows, |row| row.producer_code)),
            ("reason_code", i16s(rows, |row| row.reason_code)),
            (
                "diagnostic_id",
                binary(rows, |row| {
                    row.diagnostic_id.as_ref().map(<[u8; 16]>::as_slice)
                }),
            ),
            (
                "fallback_source_available",
                bools(rows, |row| Some(row.fallback_source_available)),
            ),
            (
                "coverage_scope_fingerprint",
                binary(rows, |row| Some(row.coverage_scope_fingerprint.as_slice())),
            ),
        ],
    )
}

/// Encode typed entities directly into the generated Arrow schema.
///
/// # Errors
///
/// Returns an Arrow error if the generated physical schema and encoder diverge.
pub fn encode_entities(rows: &[EntityRow]) -> Result<RecordBatch, FactIngestError> {
    fact_batch(
        100,
        vec![
            (
                "workspace_id",
                binary(rows, |row| Some(row.scope.workspace_id.as_slice())),
            ),
            (
                "analysis_context_id",
                binary(rows, |row| Some(row.scope.analysis_context_id.as_slice())),
            ),
            (
                "source_generation",
                i64s(rows, |row| Some(row.scope.source_generation)),
            ),
            (
                "entity_id",
                binary(rows, |row| Some(row.entity_id.as_slice())),
            ),
            (
                "owner_id",
                binary(rows, |row| Some(row.scope.owner_id.as_slice())),
            ),
            (
                "owner_bucket",
                i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            ),
            ("language", i16s(rows, |row| Some(row.language))),
            (
                "entity_family_code",
                i16s(rows, |row| Some(row.entity_family_code)),
            ),
            (
                "entity_kind_code",
                i32s(rows, |row| Some(row.entity_kind_code)),
            ),
            ("raw_kind_code", i32s(rows, |row| row.raw_kind_code)),
            (
                "file_id",
                binary(rows, |row| row.file_id.as_ref().map(<[u8; 16]>::as_slice)),
            ),
            ("start_byte", i64s(rows, |row| row.start_byte)),
            ("end_byte", i64s(rows, |row| row.end_byte)),
            ("name", utf8(rows, |row| row.name.as_deref())),
            (
                "qualified_name",
                utf8(rows, |row| row.qualified_name.as_deref()),
            ),
            (
                "parent_entity_id",
                binary(rows, |row| {
                    row.parent_entity_id.as_ref().map(<[u8; 16]>::as_slice)
                }),
            ),
            (
                "type_id",
                binary(rows, |row| row.type_id.as_ref().map(<[u8; 16]>::as_slice)),
            ),
            ("flags", i64s(rows, |row| Some(row.flags))),
            ("fact_hash64", i64s(rows, |row| Some(row.fact_hash64))),
        ],
    )
}

/// Encode typed relations directly into the generated Arrow schema.
///
/// # Errors
///
/// Returns an Arrow error if the generated physical schema and encoder diverge.
pub fn encode_relations(rows: &[RelationRow]) -> Result<RecordBatch, FactIngestError> {
    fact_batch(
        110,
        vec![
            (
                "workspace_id",
                binary(rows, |row| Some(row.scope.workspace_id.as_slice())),
            ),
            (
                "analysis_context_id",
                binary(rows, |row| Some(row.scope.analysis_context_id.as_slice())),
            ),
            (
                "source_generation",
                i64s(rows, |row| Some(row.scope.source_generation)),
            ),
            ("fact_id", binary(rows, |row| Some(row.fact_id.as_slice()))),
            (
                "owner_id",
                binary(rows, |row| Some(row.scope.owner_id.as_slice())),
            ),
            (
                "owner_bucket",
                i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            ),
            ("language", i16s(rows, |row| Some(row.language))),
            (
                "relation_family_code",
                i16s(rows, |row| Some(row.relation_family_code)),
            ),
            (
                "relation_kind_code",
                i32s(rows, |row| Some(row.relation_kind_code)),
            ),
            (
                "source_id",
                binary(rows, |row| Some(row.source_id.as_slice())),
            ),
            (
                "target_id",
                binary(rows, |row| Some(row.target_id.as_slice())),
            ),
            (
                "source_bucket",
                i16s(rows, |row| Some(i16::from(row.source_id[0]))),
            ),
            (
                "target_bucket",
                i16s(rows, |row| Some(i16::from(row.target_id[0]))),
            ),
            ("ordinal", i32s(rows, |row| row.ordinal)),
            ("role_code", i16s(rows, |row| row.role_code)),
            ("distance", i32s(rows, |row| row.distance)),
            (
                "directness_code",
                i16s(rows, |row| Some(row.directness_code)),
            ),
            (
                "file_id",
                binary(rows, |row| row.file_id.as_ref().map(<[u8; 16]>::as_slice)),
            ),
            ("start_byte", i64s(rows, |row| row.start_byte)),
            ("end_byte", i64s(rows, |row| row.end_byte)),
            ("certainty_code", i16s(rows, |row| Some(row.certainty_code))),
            (
                "resolution_code",
                i16s(rows, |row| Some(row.resolution_code)),
            ),
            ("producer_code", i16s(rows, |row| Some(row.producer_code))),
            ("derivation_code", i16s(rows, |row| row.derivation_code)),
            ("flags", i64s(rows, |row| Some(row.flags))),
            ("fact_hash64", i64s(rows, |row| Some(row.fact_hash64))),
        ],
    )
}

/// Encode typed property facts directly into the generated Arrow schema.
///
/// # Errors
///
/// Returns an Arrow error if the generated physical schema and encoder diverge.
pub fn encode_properties(rows: &[PropertyFactRow]) -> Result<RecordBatch, FactIngestError> {
    let mut columns = vec![
        (
            "workspace_id",
            binary(rows, |row| Some(row.scope.workspace_id.as_slice())),
        ),
        (
            "analysis_context_id",
            binary(rows, |row| Some(row.scope.analysis_context_id.as_slice())),
        ),
        (
            "source_generation",
            i64s(rows, |row| Some(row.scope.source_generation)),
        ),
        ("fact_id", binary(rows, |row| Some(row.fact_id.as_slice()))),
        (
            "owner_id",
            binary(rows, |row| Some(row.scope.owner_id.as_slice())),
        ),
        (
            "owner_bucket",
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
        ),
        (
            "subject_entity_id",
            binary(rows, |row| Some(row.subject_entity_id.as_slice())),
        ),
        (
            "property_kind_code",
            i32s(rows, |row| Some(row.property_kind_code)),
        ),
        (
            "program_point_entity_id",
            binary(rows, |row| {
                row.program_point_entity_id
                    .as_ref()
                    .map(<[u8; 16]>::as_slice)
            }),
        ),
        ("value_kind_code", i16s(rows, |row| Some(row.value.code()))),
    ];
    columns.extend(property_value_columns(rows));
    columns.extend([
        (
            "directness_code",
            i16s(rows, |row| Some(row.directness_code)),
        ),
        ("certainty_code", i16s(rows, |row| Some(row.certainty_code))),
        (
            "resolution_code",
            i16s(rows, |row| Some(row.resolution_code)),
        ),
        ("producer_code", i16s(rows, |row| Some(row.producer_code))),
        ("derivation_code", i16s(rows, |row| row.derivation_code)),
        (
            "file_id",
            binary(rows, |row| row.file_id.as_ref().map(<[u8; 16]>::as_slice)),
        ),
        ("start_byte", i64s(rows, |row| row.start_byte)),
        ("end_byte", i64s(rows, |row| row.end_byte)),
        ("fact_hash64", i64s(rows, |row| Some(row.fact_hash64))),
    ]);
    fact_batch(120, columns)
}

fn property_value_columns(rows: &[PropertyFactRow]) -> Vec<(&'static str, ArrayRef)> {
    vec![
        (
            "value_entity_id",
            binary(rows, |row| match &row.value {
                PropertyValue::Entity(value) => Some(value.as_slice()),
                _ => None,
            }),
        ),
        (
            "value_bool",
            bools(rows, |row| match row.value {
                PropertyValue::Boolean(value) => Some(value),
                _ => None,
            }),
        ),
        (
            "value_int64",
            i64s(rows, |row| match row.value {
                PropertyValue::Integer(value) => Some(value),
                _ => None,
            }),
        ),
        (
            "value_float64",
            f64s(rows, |row| match row.value {
                PropertyValue::Float(value) => Some(value),
                _ => None,
            }),
        ),
        (
            "value_text",
            utf8(rows, |row| match &row.value {
                PropertyValue::Text(value) => Some(value.as_str()),
                _ => None,
            }),
        ),
        (
            "value_bytes",
            binary(rows, |row| match &row.value {
                PropertyValue::Bytes(value) => Some(value.as_slice()),
                _ => None,
            }),
        ),
        (
            "value_type_id",
            binary(rows, |row| match &row.value {
                PropertyValue::Type(value) => Some(value.as_slice()),
                _ => None,
            }),
        ),
    ]
}

/// Encode typed evidence directly into the generated Arrow schema.
///
/// # Errors
///
/// Returns an Arrow error if the generated physical schema and encoder diverge.
pub fn encode_evidence(rows: &[FactEvidenceRow]) -> Result<RecordBatch, FactIngestError> {
    fact_batch(
        130,
        vec![
            (
                "evidence_id",
                binary(rows, |row| Some(row.evidence_id.as_slice())),
            ),
            (
                "workspace_id",
                binary(rows, |row| Some(row.scope.workspace_id.as_slice())),
            ),
            (
                "analysis_context_id",
                binary(rows, |row| Some(row.scope.analysis_context_id.as_slice())),
            ),
            (
                "source_generation",
                i64s(rows, |row| Some(row.scope.source_generation)),
            ),
            ("fact_id", binary(rows, |row| Some(row.fact_id.as_slice()))),
            ("fact_form_code", i16s(rows, |row| Some(row.fact_form_code))),
            (
                "owner_id",
                binary(rows, |row| Some(row.scope.owner_id.as_slice())),
            ),
            (
                "owner_bucket",
                i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            ),
            ("provider_code", i16s(rows, |row| Some(row.provider_code))),
            (
                "provider_version",
                utf8(rows, |row| Some(row.provider_version.as_str())),
            ),
            (
                "provider_run_id",
                binary(rows, |row| Some(row.provider_run_id.as_slice())),
            ),
            (
                "observation_id",
                binary(rows, |row| Some(row.observation_id.as_slice())),
            ),
            ("raw_kind_code", i32s(rows, |row| row.raw_kind_code)),
            (
                "file_id",
                binary(rows, |row| row.file_id.as_ref().map(<[u8; 16]>::as_slice)),
            ),
            ("start_byte", i64s(rows, |row| row.start_byte)),
            ("end_byte", i64s(rows, |row| row.end_byte)),
            ("certainty_code", i16s(rows, |row| Some(row.certainty_code))),
            (
                "resolution_code",
                i16s(rows, |row| Some(row.resolution_code)),
            ),
            (
                "conflict_disposition_code",
                i16s(rows, |row| Some(row.conflict_disposition_code)),
            ),
            (
                "cold_payload",
                binary(rows, |row| row.cold_payload.as_deref()),
            ),
        ],
    )
}

/// Encode authoritative source images directly into the generated Arrow schema.
///
/// # Errors
///
/// Returns an Arrow error if the generated physical schema and encoder diverge.
pub fn encode_source_files(rows: &[SourceFileRow]) -> Result<RecordBatch, FactIngestError> {
    fact_batch(
        140,
        vec![
            (
                "workspace_id",
                binary(rows, |row| Some(row.scope.workspace_id.as_slice())),
            ),
            (
                "analysis_context_id",
                binary(rows, |row| Some(row.scope.analysis_context_id.as_slice())),
            ),
            (
                "source_generation",
                i64s(rows, |row| Some(row.scope.source_generation)),
            ),
            ("file_id", binary(rows, |row| Some(row.file_id.as_slice()))),
            (
                "owner_id",
                binary(rows, |row| Some(row.scope.owner_id.as_slice())),
            ),
            (
                "owner_bucket",
                i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            ),
            (
                "path_bytes",
                binary(rows, |row| Some(row.path_bytes.as_slice())),
            ),
            (
                "path_display",
                utf8(rows, |row| Some(row.path_display.as_str())),
            ),
            (
                "path_encoding_code",
                i16s(rows, |row| Some(row.path_encoding_code)),
            ),
            (
                "path_case_key",
                binary(rows, |row| row.path_case_key.as_deref()),
            ),
            (
                "path_display_is_lossy",
                bools(rows, |row| Some(row.path_display_is_lossy)),
            ),
            ("language", i16s(rows, |row| Some(row.language))),
            (
                "source_digest",
                binary(rows, |row| Some(row.source_digest.as_slice())),
            ),
            ("byte_len", i64s(rows, |row| Some(row.byte_len))),
            ("line_count", i32s(rows, |row| Some(row.line_count))),
            (
                "encoding_name",
                utf8(rows, |row| row.encoding_name.as_deref()),
            ),
            (
                "newline_kind_code",
                i16s(rows, |row| Some(row.newline_kind_code)),
            ),
            (
                "source_bytes",
                binary(rows, |row| Some(row.source_bytes.as_slice())),
            ),
            (
                "decoded_text",
                utf8(rows, |row| row.decoded_text.as_deref()),
            ),
            (
                "line_start_offsets",
                i64_lists(140, "line_start_offsets", rows, |row| {
                    row.line_start_offsets.as_slice()
                }),
            ),
            (
                "module_entity_id",
                binary(rows, |row| {
                    row.module_entity_id.as_ref().map(<[u8; 16]>::as_slice)
                }),
            ),
            ("is_stub", bools(rows, |row| Some(row.is_stub))),
            ("flags", i64s(rows, |row| Some(row.flags))),
        ],
    )
}

/// Encode source tokens directly into the generated Arrow schema.
///
/// # Errors
///
/// Returns an Arrow error if the generated physical schema and encoder diverge.
pub fn encode_source_tokens(rows: &[SourceTokenRow]) -> Result<RecordBatch, FactIngestError> {
    fact_batch(
        150,
        vec![
            (
                "workspace_id",
                binary(rows, |row| Some(row.scope.workspace_id.as_slice())),
            ),
            (
                "analysis_context_id",
                binary(rows, |row| Some(row.scope.analysis_context_id.as_slice())),
            ),
            (
                "source_generation",
                i64s(rows, |row| Some(row.scope.source_generation)),
            ),
            (
                "token_id",
                binary(rows, |row| Some(row.token_id.as_slice())),
            ),
            (
                "owner_id",
                binary(rows, |row| Some(row.scope.owner_id.as_slice())),
            ),
            (
                "owner_bucket",
                i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            ),
            ("file_id", binary(rows, |row| Some(row.file_id.as_slice()))),
            ("ordinal", i32s(rows, |row| Some(row.ordinal))),
            (
                "token_kind_code",
                i32s(rows, |row| Some(row.token_kind_code)),
            ),
            ("start_byte", i64s(rows, |row| Some(row.start_byte))),
            ("end_byte", i64s(rows, |row| Some(row.end_byte))),
            (
                "normalized_value",
                utf8(rows, |row| row.normalized_value.as_deref()),
            ),
            ("flags", i64s(rows, |row| Some(row.flags))),
        ],
    )
}

/// Encode source annotations directly into the generated Arrow schema.
///
/// # Errors
///
/// Returns an Arrow error if the generated physical schema and encoder diverge.
pub fn encode_source_annotations(
    rows: &[SourceAnnotationRow],
) -> Result<RecordBatch, FactIngestError> {
    fact_batch(
        160,
        vec![
            (
                "workspace_id",
                binary(rows, |row| Some(row.scope.workspace_id.as_slice())),
            ),
            (
                "analysis_context_id",
                binary(rows, |row| Some(row.scope.analysis_context_id.as_slice())),
            ),
            (
                "source_generation",
                i64s(rows, |row| Some(row.scope.source_generation)),
            ),
            (
                "annotation_id",
                binary(rows, |row| Some(row.annotation_id.as_slice())),
            ),
            (
                "owner_id",
                binary(rows, |row| Some(row.scope.owner_id.as_slice())),
            ),
            (
                "owner_bucket",
                i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            ),
            ("file_id", binary(rows, |row| Some(row.file_id.as_slice()))),
            (
                "annotation_kind_code",
                i32s(rows, |row| Some(row.annotation_kind_code)),
            ),
            ("start_byte", i64s(rows, |row| Some(row.start_byte))),
            ("end_byte", i64s(rows, |row| Some(row.end_byte))),
            (
                "target_entity_id",
                binary(rows, |row| {
                    row.target_entity_id.as_ref().map(<[u8; 16]>::as_slice)
                }),
            ),
            ("text", utf8(rows, |row| row.text.as_deref())),
            ("diagnostic_code", i32s(rows, |row| row.diagnostic_code)),
            ("flags", i64s(rows, |row| Some(row.flags))),
        ],
    )
}

/// Encode syntax extensions directly into the generated Arrow schema.
///
/// # Errors
///
/// Returns an Arrow error if the generated physical schema and encoder diverge.
pub fn encode_syntax_details(rows: &[SyntaxDetailRow]) -> Result<RecordBatch, FactIngestError> {
    fact_batch(
        170,
        vec![
            (
                "workspace_id",
                binary(rows, |row| Some(row.scope.workspace_id.as_slice())),
            ),
            (
                "analysis_context_id",
                binary(rows, |row| Some(row.scope.analysis_context_id.as_slice())),
            ),
            (
                "source_generation",
                i64s(rows, |row| Some(row.scope.source_generation)),
            ),
            (
                "entity_id",
                binary(rows, |row| Some(row.entity_id.as_slice())),
            ),
            (
                "owner_id",
                binary(rows, |row| Some(row.scope.owner_id.as_slice())),
            ),
            (
                "owner_bucket",
                i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            ),
            ("raw_kind_code", i32s(rows, |row| Some(row.raw_kind_code))),
            (
                "occurrence_family_code",
                i16s(rows, |row| Some(row.occurrence_family_code)),
            ),
            (
                "reconciliation_step_code",
                i16s(rows, |row| Some(row.reconciliation_step_code)),
            ),
            (
                "raw_kind_disposition_code",
                i16s(rows, |row| Some(row.raw_kind_disposition_code)),
            ),
            (
                "normalized_kind_code",
                i32s(rows, |row| Some(row.normalized_kind_code)),
            ),
            (
                "parent_syntax_id",
                binary(rows, |row| {
                    row.parent_syntax_id.as_ref().map(<[u8; 16]>::as_slice)
                }),
            ),
            ("field_role_code", i16s(rows, |row| row.field_role_code)),
            ("ordinal", i32s(rows, |row| row.ordinal)),
            ("named", bools(rows, |row| Some(row.named))),
            ("extra", bools(rows, |row| Some(row.extra))),
            ("error", bools(rows, |row| Some(row.error))),
            ("missing", bools(rows, |row| Some(row.missing))),
            (
                "explicitly_parenthesized",
                bools(rows, |row| Some(row.explicitly_parenthesized)),
            ),
            (
                "provider_node_flags",
                i64s(rows, |row| Some(row.provider_node_flags)),
            ),
        ],
    )
}

fn invalid(spec: &TableSpec, check: &'static str, detail: impl Into<String>) -> FactIngestError {
    FactIngestError::BatchInvalid {
        table: spec.name.into(),
        check,
        detail: detail.into(),
    }
}

fn column<'a>(batch: &'a RecordBatch, spec: &TableSpec, name: &str) -> &'a ArrayRef {
    let index = spec
        .arrow_schema
        .index_of(name)
        .expect("generated validator column");
    batch.column(index)
}

fn binary_column<'a>(batch: &'a RecordBatch, spec: &TableSpec, name: &str) -> &'a BinaryArray {
    column(batch, spec, name)
        .as_any()
        .downcast_ref::<BinaryArray>()
        .expect("generated binary column")
}

fn i16_column<'a>(batch: &'a RecordBatch, spec: &TableSpec, name: &str) -> &'a Int16Array {
    column(batch, spec, name)
        .as_any()
        .downcast_ref::<Int16Array>()
        .expect("generated Int16 column")
}

fn i32_column<'a>(batch: &'a RecordBatch, spec: &TableSpec, name: &str) -> &'a Int32Array {
    column(batch, spec, name)
        .as_any()
        .downcast_ref::<Int32Array>()
        .expect("generated Int32 column")
}

fn i64_column<'a>(batch: &'a RecordBatch, spec: &TableSpec, name: &str) -> &'a Int64Array {
    column(batch, spec, name)
        .as_any()
        .downcast_ref::<Int64Array>()
        .expect("generated Int64 column")
}

fn validate_primary_key(batch: &RecordBatch, spec: &TableSpec) -> Result<(), FactIngestError> {
    let columns = spec
        .primary_key
        .iter()
        .map(|name| Arc::clone(column(batch, spec, name)))
        .collect::<Vec<_>>();
    if columns.iter().any(|array| array.null_count() != 0) {
        return Err(invalid(spec, "non-null-key", "primary key contains null"));
    }
    let fields = columns
        .iter()
        .map(|array| SortField::new(array.data_type().clone()))
        .collect();
    let converter = RowConverter::new(fields)?;
    let rows = converter.convert_columns(&columns)?;
    let mut sorted = rows.iter().collect::<Vec<_>>();
    sorted.sort_unstable();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid(spec, "primary-key", "duplicate primary key"));
    }
    Ok(())
}

fn validate_id_widths(batch: &RecordBatch, spec: &TableSpec) -> Result<(), FactIngestError> {
    for field in spec.arrow_schema.fields() {
        let expected = match field.metadata().get("com.codefabric.cpg.semantic_type") {
            Some(value) if value == "id16" => Some(16),
            Some(value) if value == "hash32" => Some(32),
            _ => None,
        };
        let Some(expected) = expected else { continue };
        let array = binary_column(batch, spec, field.name());
        if array.iter().flatten().any(|value| value.len() != expected) {
            return Err(invalid(
                spec,
                "fixed-width",
                format!("{} is not {expected} bytes", field.name()),
            ));
        }
    }
    Ok(())
}

fn validate_buckets(batch: &RecordBatch, spec: &TableSpec) -> Result<(), FactIngestError> {
    for (bucket, identity) in [
        ("owner_bucket", "owner_id"),
        ("source_bucket", "source_id"),
        ("target_bucket", "target_id"),
    ] {
        if spec.arrow_schema.index_of(bucket).is_err() {
            continue;
        }
        let buckets = i16_column(batch, spec, bucket);
        let identities = binary_column(batch, spec, identity);
        for row in 0..batch.num_rows() {
            if buckets.is_null(row)
                || identities.is_null(row)
                || buckets.value(row) != i16::from(identities.value(row)[0])
            {
                return Err(invalid(spec, "bucket", format!("{bucket} drifted")));
            }
        }
    }
    Ok(())
}

fn validate_spans(batch: &RecordBatch, spec: &TableSpec) -> Result<(), FactIngestError> {
    if spec.arrow_schema.index_of("start_byte").is_err() {
        return Ok(());
    }
    let starts = i64_column(batch, spec, "start_byte");
    let ends = i64_column(batch, spec, "end_byte");
    for row in 0..batch.num_rows() {
        if starts.is_null(row) != ends.is_null(row)
            || (!starts.is_null(row)
                && (starts.value(row) < 0 || ends.value(row) < starts.value(row)))
        {
            return Err(invalid(spec, "span", "malformed source span"));
        }
    }
    Ok(())
}

fn enum_domain(name: &str) -> Option<&'static [crate::registries::RegistryEntry]> {
    crate::registries::REGISTRY_DOMAINS
        .iter()
        .find(|domain| domain.domain == name)
        .map(|domain| domain.values)
}

#[allow(clippy::too_many_lines)] // One scan validates every generated enum and registry role before publication.
fn validate_registered_codes(batch: &RecordBatch, spec: &TableSpec) -> Result<(), FactIngestError> {
    for (field, domain) in [
        ("directness_code", "DIRECTNESS"),
        ("certainty_code", "EVIDENCE_CERTAINTY"),
        ("resolution_code", "RESOLUTION_CLASS"),
    ] {
        if spec.arrow_schema.index_of(field).is_err() {
            continue;
        }
        let values = i16_column(batch, spec, field);
        let registered = enum_domain(domain).expect("generated enum domain");
        if values.iter().flatten().any(|code| {
            !registered
                .iter()
                .any(|entry| entry.code == code.cast_unsigned())
        }) {
            return Err(invalid(spec, "enum-code", format!("{field} is unknown")));
        }
    }
    for (field, domain) in [
        ("language", "LANGUAGE"),
        ("owner_kind_code", "OWNER_KIND"),
        ("owner_capability_state_code", "OWNER_CAPABILITY_STATE"),
        ("completeness_state_code", "COMPLETENESS_STATE"),
        ("provider_code", "PROVIDER_CODE"),
        ("producer_code", "PROVIDER_CODE"),
        ("path_encoding_code", "PATH_ENCODING"),
        ("newline_kind_code", "NEWLINE_KIND"),
        ("field_role_code", "SYNTAX_FIELD_ROLE"),
    ] {
        if spec.arrow_schema.index_of(field).is_err() {
            continue;
        }
        let values = i16_column(batch, spec, field);
        let registered = enum_domain(domain).expect("generated enum domain");
        if values.iter().flatten().any(|code| {
            !registered
                .iter()
                .any(|entry| entry.code == code.cast_unsigned())
        }) {
            return Err(invalid(spec, "enum-code", format!("{field} is unknown")));
        }
    }
    match spec.table_code {
        9 => {
            let capabilities = i16_column(batch, spec, "capability_code");
            if capabilities.iter().flatten().any(|code| {
                u16::try_from(code).map_or(true, |code| {
                    !crate::registries::CAPABILITY_IDS.iter().any(|id| {
                        crate::registries::capability_code(id).is_some_and(|known| known == code)
                    })
                })
            }) {
                return Err(invalid(spec, "enum-code", "capability_code is unknown"));
            }
        }
        100 => {
            let kinds = i32_column(batch, spec, "entity_kind_code");
            let families = i16_column(batch, spec, "entity_family_code");
            for row in 0..batch.num_rows() {
                let known = crate::registries::ENTITY_KIND_CODES.iter().any(|entry| {
                    entry.code == kinds.value(row) && entry.family_code == families.value(row)
                });
                if !known {
                    return Err(invalid(
                        spec,
                        "ontology-code",
                        "entity kind/family mismatch",
                    ));
                }
            }
        }
        110 => {
            let kinds = i32_column(batch, spec, "relation_kind_code");
            let families = i16_column(batch, spec, "relation_family_code");
            for row in 0..batch.num_rows() {
                let known = crate::registries::RELATION_KIND_CODES.iter().any(|entry| {
                    entry.code == kinds.value(row) && entry.family_code == families.value(row)
                });
                if !known {
                    return Err(invalid(
                        spec,
                        "ontology-code",
                        "relation kind/family mismatch",
                    ));
                }
            }
        }
        120 => {
            let kinds = i32_column(batch, spec, "property_kind_code");
            if kinds.iter().flatten().any(|code| {
                !crate::registries::PROPERTY_KIND_CODES
                    .iter()
                    .any(|entry| entry.code == code)
            }) {
                return Err(invalid(spec, "ontology-code", "property kind is unknown"));
            }
        }
        150 => validate_code32_domain(batch, spec, "token_kind_code", "TOKEN_KIND")?,
        160 => validate_code32_domain(batch, spec, "annotation_kind_code", "ANNOTATION_KIND")?,
        170 => validate_code32_domain(batch, spec, "normalized_kind_code", "SYNTAX_KIND")?,
        _ => {}
    }
    Ok(())
}

fn validate_code32_domain(
    batch: &RecordBatch,
    spec: &TableSpec,
    field: &str,
    domain: &str,
) -> Result<(), FactIngestError> {
    let values = i32_column(batch, spec, field);
    let registered = enum_domain(domain).expect("generated enum domain");
    if values.iter().flatten().any(|code| {
        u16::try_from(code).map_or(true, |code| {
            !registered.iter().any(|entry| entry.code == code)
        })
    }) {
        return Err(invalid(spec, "enum-code", format!("{field} is unknown")));
    }
    Ok(())
}

fn validate_source_file(batch: &RecordBatch, spec: &TableSpec) -> Result<(), FactIngestError> {
    if spec.table_code != 140 {
        return Ok(());
    }
    let byte_lengths = i64_column(batch, spec, "byte_len");
    let line_counts = i32_column(batch, spec, "line_count");
    let source_bytes = binary_column(batch, spec, "source_bytes");
    let source_digests = binary_column(batch, spec, "source_digest");
    let offsets = column(batch, spec, "line_start_offsets")
        .as_any()
        .downcast_ref::<ListArray>()
        .expect("generated Int64 list column");
    for row in 0..batch.num_rows() {
        let bytes = source_bytes.value(row);
        let expected_length = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
        if byte_lengths.value(row) != expected_length
            || source_digests.value(row) != crate::integrity::digest_bytes(bytes)
        {
            return Err(invalid(
                spec,
                "source-image",
                "source bytes, length, and digest differ",
            ));
        }
        let values = offsets.value(row);
        let values = values
            .as_any()
            .downcast_ref::<Int64Array>()
            .expect("generated Int64 list values");
        if !valid_line_starts(values, expected_length, line_counts.value(row)) {
            return Err(invalid(
                spec,
                "line-index",
                "line starts are not a bounded strictly increasing index",
            ));
        }
    }
    Ok(())
}

fn valid_line_starts(values: &Int64Array, expected_length: i64, line_count: i32) -> bool {
    !values.is_empty()
        && values.value(0) == 0
        && values
            .iter()
            .flatten()
            .all(|offset| offset >= 0 && offset <= expected_length)
        && values
            .iter()
            .flatten()
            .zip(values.iter().flatten().skip(1))
            .all(|(left, right)| left < right)
        && i32::try_from(values.len()).unwrap_or(i32::MAX) == line_count
}

fn validate_property_values(batch: &RecordBatch, spec: &TableSpec) -> Result<(), FactIngestError> {
    if spec.table_code != 120 {
        return Ok(());
    }
    let kinds = i16_column(batch, spec, "value_kind_code");
    let representations = [
        "value_entity_id",
        "value_bool",
        "value_int64",
        "value_float64",
        "value_text",
        "value_bytes",
        "value_type_id",
    ];
    for row in 0..batch.num_rows() {
        let populated = representations
            .iter()
            .enumerate()
            .filter(|(_, name)| !column(batch, spec, name).is_null(row))
            .map(|(index, _)| 10_i16 * i16::try_from(index + 1).expect("seven representations"))
            .collect::<Vec<_>>();
        if populated != [kinds.value(row)] {
            return Err(invalid(
                spec,
                "property-value",
                "value_kind_code does not select exactly one representation",
            ));
        }
    }
    Ok(())
}

/// Execute the complete generated-schema fact-batch validation matrix.
///
/// # Errors
///
/// Returns a stable check class for any schema, identity, fence, row-local, ontology,
/// or primary-key violation.
pub fn validate_fact_batch(
    batch: &RecordBatch,
    table_code: i16,
    expected_scope: FactScope,
) -> Result<(), FactIngestError> {
    let spec = table_spec(table_code).ok_or_else(|| {
        FactIngestError::Protocol(format!("table code {table_code} is not generated"))
    })?;
    if !matches!(
        spec.durable_mutation,
        DurableMutationClass::OwnerReplacedFact | DurableMutationClass::DerivedOwnerReplaced
    ) || [
        "workspace_id",
        "analysis_context_id",
        "source_generation",
        "owner_id",
    ]
    .iter()
    .any(|column| spec.arrow_schema.index_of(column).is_err())
    {
        return Err(invalid(
            spec,
            "table-family",
            "not a generated owner-scoped fact table",
        ));
    }
    if batch.schema() != spec.arrow_schema {
        return Err(invalid(
            spec,
            "schema",
            "schema is not the exact generated schema",
        ));
    }
    if batch.num_rows() > MAX_ROWS_PER_STREAM {
        return Err(invalid(
            spec,
            "batch-size",
            "owner batch exceeds the bounded row budget",
        ));
    }
    if batch.num_columns() != spec.arrow_schema.fields().len()
        || batch
            .columns()
            .iter()
            .any(|column| column.len() != batch.num_rows())
    {
        return Err(invalid(spec, "shape", "column or row count differs"));
    }
    validate_primary_key(batch, spec)?;
    validate_id_widths(batch, spec)?;
    validate_buckets(batch, spec)?;
    validate_spans(batch, spec)?;
    validate_registered_codes(batch, spec)?;
    validate_property_values(batch, spec)?;
    validate_source_file(batch, spec)?;
    let workspaces = binary_column(batch, spec, "workspace_id");
    let contexts = binary_column(batch, spec, "analysis_context_id");
    let generations = i64_column(batch, spec, "source_generation");
    let owners = binary_column(batch, spec, "owner_id");
    for row in 0..batch.num_rows() {
        if workspaces.value(row) != expected_scope.workspace_id
            || contexts.value(row) != expected_scope.analysis_context_id
        {
            return Err(FactIngestError::SourceSnapshotMismatch(spec.name.into()));
        }
        if generations.value(row) != expected_scope.source_generation {
            return Err(FactIngestError::StaleResult(spec.name.into()));
        }
        if owners.value(row) != expected_scope.owner_id {
            return Err(invalid(
                spec,
                "owner",
                "row owner differs from ingest owner",
            ));
        }
    }
    Ok(())
}

/// Closed terminal state for one provider observation stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamTerminal {
    Completed,
    Partial,
    Failed,
}

/// Header that must precede every observation batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationManifest {
    pub stream_id: [u8; 16],
    pub workspace_id: [u8; 16],
    pub analysis_context_id: [u8; 16],
    pub source_generation: i64,
    pub provider_code: i16,
    pub provider_version: String,
    pub provider_run_id: [u8; 16],
    pub schema_fingerprints: BTreeMap<i16, String>,
    pub declared_rows: usize,
}

/// Provider-owned provenance attached before canonical reconciliation.
#[derive(Clone, Debug, PartialEq)]
pub struct ObservationEvidence {
    pub observation_id: [u8; 16],
    pub raw_kind_code: Option<i32>,
    pub certainty_code: i16,
    pub resolution_code: i16,
    pub cold_payload: Option<Vec<u8>>,
}

/// The closed universal-fact forms admitted from provider observations.
#[derive(Clone, Debug, PartialEq)]
pub enum CanonicalFact {
    Entity(EntityRow),
    Relation(RelationRow),
    Property(PropertyFactRow),
}

impl CanonicalFact {
    const fn table_code(&self) -> i16 {
        match self {
            Self::Entity(_) => 100,
            Self::Relation(_) => 110,
            Self::Property(_) => 120,
        }
    }

    const fn fact_form_code(&self) -> i16 {
        match self {
            Self::Entity(_) => 10,
            Self::Relation(_) => 20,
            Self::Property(_) => 30,
        }
    }

    const fn scope(&self) -> FactScope {
        match self {
            Self::Entity(row) => row.scope,
            Self::Relation(row) => row.scope,
            Self::Property(row) => row.scope,
        }
    }

    const fn fact_id(&self) -> [u8; 16] {
        match self {
            Self::Entity(row) => row.entity_id,
            Self::Relation(row) => row.fact_id,
            Self::Property(row) => row.fact_id,
        }
    }

    const fn span(&self) -> (Option<[u8; 16]>, Option<i64>, Option<i64>) {
        match self {
            Self::Entity(row) => (row.file_id, row.start_byte, row.end_byte),
            Self::Relation(row) => (row.file_id, row.start_byte, row.end_byte),
            Self::Property(row) => (row.file_id, row.start_byte, row.end_byte),
        }
    }
}

/// One typed observation after the manifest and before terminal state.
#[derive(Clone, Debug, PartialEq)]
pub struct ObservedFact {
    pub fact: CanonicalFact,
    pub evidence: ObservationEvidence,
}

/// Complete bounded stream; field order makes the manifest structurally precede batches.
#[derive(Clone, Debug, PartialEq)]
pub struct ObservationStream {
    pub manifest: ObservationManifest,
    pub observations: Vec<ObservedFact>,
    pub terminal: StreamTerminal,
}

/// Wire-state messages for producers that need backpressured asynchronous delivery.
#[derive(Clone, Debug, PartialEq)]
pub enum ObservationMessage {
    Manifest(ObservationManifest),
    Batch(Vec<ObservedFact>),
    Terminal(StreamTerminal),
}

/// Construct one bounded MPSC observation channel.
#[must_use]
pub fn bounded_observation_channel(
    capacity: usize,
) -> (
    mpsc::Sender<ObservationMessage>,
    mpsc::Receiver<ObservationMessage>,
) {
    mpsc::channel(capacity.max(1))
}

/// Receive one channel as a manifest-first, terminal-complete stream.
///
/// # Errors
///
/// Rejects batches before the manifest, duplicate headers, messages after terminal,
/// missing terminal state, row-limit overflow, and a closed channel before completion.
pub async fn receive_observation_stream(
    receiver: &mut mpsc::Receiver<ObservationMessage>,
) -> Result<ObservationStream, FactIngestError> {
    let mut manifest = None;
    let mut observations = Vec::new();
    let mut terminal = None;
    while let Some(message) = receiver.recv().await {
        if terminal.is_some() {
            return Err(FactIngestError::Protocol(
                "message after terminal state".into(),
            ));
        }
        match message {
            ObservationMessage::Manifest(value) if manifest.is_none() => manifest = Some(value),
            ObservationMessage::Manifest(_) => {
                return Err(FactIngestError::Protocol("duplicate manifest".into()));
            }
            ObservationMessage::Batch(mut rows) if manifest.is_some() => {
                if observations.len().saturating_add(rows.len()) > MAX_ROWS_PER_STREAM {
                    return Err(FactIngestError::Protocol(
                        "stream row budget exceeded".into(),
                    ));
                }
                observations.append(&mut rows);
            }
            ObservationMessage::Batch(_) => {
                return Err(FactIngestError::Protocol("batch preceded manifest".into()));
            }
            ObservationMessage::Terminal(value) if manifest.is_some() => {
                terminal = Some(value);
                break;
            }
            ObservationMessage::Terminal(_) => {
                return Err(FactIngestError::Protocol(
                    "terminal preceded manifest".into(),
                ));
            }
        }
    }
    let manifest = manifest.ok_or_else(|| FactIngestError::Protocol("manifest absent".into()))?;
    let terminal = terminal.ok_or_else(|| FactIngestError::Protocol("terminal absent".into()))?;
    if receiver.try_recv().is_ok() {
        return Err(FactIngestError::Protocol(
            "message after terminal state".into(),
        ));
    }
    Ok(ObservationStream {
        manifest,
        observations,
        terminal,
    })
}

/// One deterministic reconciliation conflict retained alongside evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConflictRecord {
    pub table_code: i16,
    pub fact_id: [u8; 16],
    pub selected_provider_code: i16,
    pub rejected_provider_code: i16,
    pub selected_observation_id: [u8; 16],
    pub rejected_observation_id: [u8; 16],
}

/// Stable, bounded diagnostic emitted by canonical reconciliation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IngestDiagnostic {
    pub code: &'static str,
    pub detail: String,
    pub file_id: Option<[u8; 16]>,
    pub start_byte: Option<i64>,
    pub end_byte: Option<i64>,
}

/// Cumulative, non-timing ingest counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IngestMetrics {
    pub streams_received: u64,
    pub rows_received: u64,
    pub rows_encoded: u64,
    pub validation_failures: u64,
    pub conflicts: u64,
}

#[derive(Debug, Default)]
struct IngestCounters {
    streams_received: AtomicU64,
    rows_received: AtomicU64,
    rows_encoded: AtomicU64,
    validation_failures: AtomicU64,
    conflicts: AtomicU64,
}

impl IngestCounters {
    fn record_success(&self, metrics: IngestMetrics) {
        self.streams_received
            .fetch_add(metrics.streams_received, Ordering::Relaxed);
        self.rows_received
            .fetch_add(metrics.rows_received, Ordering::Relaxed);
        self.rows_encoded
            .fetch_add(metrics.rows_encoded, Ordering::Relaxed);
        self.conflicts
            .fetch_add(metrics.conflicts, Ordering::Relaxed);
    }

    fn snapshot(&self) -> IngestMetrics {
        IngestMetrics {
            streams_received: self.streams_received.load(Ordering::Relaxed),
            rows_received: self.rows_received.load(Ordering::Relaxed),
            rows_encoded: self.rows_encoded.load(Ordering::Relaxed),
            validation_failures: self.validation_failures.load(Ordering::Relaxed),
            conflicts: self.conflicts.load(Ordering::Relaxed),
        }
    }
}

/// Validated reconciliation result. No writer capability is exposed.
#[derive(Debug)]
pub struct CanonicalIngestOutput {
    pub batches: BTreeMap<i16, ValidatedFactBatch>,
    pub conflicts: Vec<ConflictRecord>,
    pub diagnostics: Vec<IngestDiagnostic>,
    pub metrics: IngestMetrics,
}

/// Exact-schema fact batch admitted through the sole validation boundary.
#[derive(Clone, Debug)]
pub struct ValidatedFactBatch {
    table_code: i16,
    scope: FactScope,
    batch: RecordBatch,
}

impl ValidatedFactBatch {
    /// Admit a batch only after the complete generated validation matrix.
    ///
    /// # Errors
    ///
    /// Returns the stable failing validation class.
    pub fn validate(
        table_code: i16,
        batch: RecordBatch,
        scope: FactScope,
    ) -> Result<Self, FactIngestError> {
        validate_fact_batch(&batch, table_code, scope)?;
        Ok(Self {
            table_code,
            scope,
            batch,
        })
    }

    /// Generated table code this batch exactly satisfies.
    #[must_use]
    pub const fn table_code(&self) -> i16 {
        self.table_code
    }

    /// Exact scope proved at batch admission.
    #[must_use]
    pub const fn scope(&self) -> FactScope {
        self.scope
    }

    /// Read-only access for queries, checksums, and the policy-enforcing writer.
    #[must_use]
    pub const fn batch(&self) -> &RecordBatch {
        &self.batch
    }

    /// Number of validated fact rows.
    #[must_use]
    pub fn num_rows(&self) -> usize {
        self.batch.num_rows()
    }
}

#[derive(Clone)]
struct Candidate {
    provider_code: i16,
    provider_version: String,
    provider_run_id: [u8; 16],
    observation: ObservedFact,
}

type CandidateGroups = BTreeMap<(i16, [u8; 16]), Vec<Candidate>>;

fn evidence_id(provider_run_id: [u8; 16], observation_id: [u8; 16], fact_id: [u8; 16]) -> [u8; 16] {
    crate::identity::fact_evidence_id(provider_run_id, observation_id, fact_id)
}

fn encode_selected(
    selected: Vec<CanonicalFact>,
    evidence: &[FactEvidenceRow],
) -> Result<BTreeMap<i16, RecordBatch>, FactIngestError> {
    let mut entities = Vec::new();
    let mut relations = Vec::new();
    let mut properties = Vec::new();
    for fact in selected {
        match fact {
            CanonicalFact::Entity(row) => entities.push(row),
            CanonicalFact::Relation(row) => relations.push(row),
            CanonicalFact::Property(row) => properties.push(row),
        }
    }
    Ok(BTreeMap::from([
        (100, encode_entities(&entities)?),
        (110, encode_relations(&relations)?),
        (120, encode_properties(&properties)?),
        (130, encode_evidence(evidence)?),
    ]))
}

fn collect_candidates(
    expected_scope: FactScope,
    streams: &[ObservationStream],
    provider_precedence: &BTreeMap<i16, u16>,
    metrics: &mut IngestMetrics,
) -> Result<CandidateGroups, FactIngestError> {
    let mut candidates = CandidateGroups::new();
    for stream in streams {
        metrics.streams_received += 1;
        metrics.rows_received += u64::try_from(stream.observations.len()).unwrap_or(u64::MAX);
        if stream.terminal == StreamTerminal::Failed {
            return Err(FactIngestError::Protocol(
                "failed stream is not ingestible".into(),
            ));
        }
        if stream.observations.len() != stream.manifest.declared_rows
            || stream.observations.len() > MAX_ROWS_PER_STREAM
        {
            return Err(FactIngestError::Protocol(
                "declared row count drifted".into(),
            ));
        }
        if stream.manifest.workspace_id != expected_scope.workspace_id
            || stream.manifest.analysis_context_id != expected_scope.analysis_context_id
        {
            return Err(FactIngestError::SourceSnapshotMismatch("manifest".into()));
        }
        if stream.manifest.source_generation != expected_scope.source_generation {
            return Err(FactIngestError::StaleResult("manifest".into()));
        }
        if !provider_precedence.contains_key(&stream.manifest.provider_code) {
            return Err(FactIngestError::Protocol(
                "provider precedence absent".into(),
            ));
        }
        for observed in &stream.observations {
            let code = observed.fact.table_code();
            let spec = table_spec(code)
                .ok_or_else(|| FactIngestError::Protocol(format!("unknown fact table {code}")))?;
            if stream.manifest.schema_fingerprints.get(&code) != Some(&spec.schema_digest) {
                return Err(FactIngestError::SourceSnapshotMismatch(format!(
                    "{} schema fingerprint",
                    spec.name
                )));
            }
            let scope = observed.fact.scope();
            if scope.workspace_id != expected_scope.workspace_id
                || scope.analysis_context_id != expected_scope.analysis_context_id
            {
                return Err(FactIngestError::SourceSnapshotMismatch(spec.name.into()));
            }
            if scope.source_generation != expected_scope.source_generation {
                return Err(FactIngestError::StaleResult(spec.name.into()));
            }
            candidates
                .entry((code, observed.fact.fact_id()))
                .or_default()
                .push(Candidate {
                    provider_code: stream.manifest.provider_code,
                    provider_version: stream.manifest.provider_version.clone(),
                    provider_run_id: stream.manifest.provider_run_id,
                    observation: observed.clone(),
                });
        }
    }
    Ok(candidates)
}

fn reconcile_candidates(
    candidates: CandidateGroups,
    provider_precedence: &BTreeMap<i16, u16>,
) -> (
    Vec<CanonicalFact>,
    Vec<FactEvidenceRow>,
    Vec<ConflictRecord>,
) {
    let mut selected = Vec::new();
    let mut evidence = Vec::new();
    let mut conflicts = Vec::new();
    for ((table_code, fact_id), mut group) in candidates {
        group.sort_by_key(|candidate| {
            (
                provider_precedence
                    .get(&candidate.provider_code)
                    .copied()
                    .unwrap_or(u16::MAX),
                candidate.provider_code,
                candidate.observation.evidence.observation_id,
            )
        });
        let Some(winner) = group.first().cloned() else {
            continue;
        };
        selected.push(winner.observation.fact.clone());
        for candidate in group {
            let is_conflict = candidate.observation.fact != winner.observation.fact;
            if is_conflict {
                conflicts.push(ConflictRecord {
                    table_code,
                    fact_id,
                    selected_provider_code: winner.provider_code,
                    rejected_provider_code: candidate.provider_code,
                    selected_observation_id: winner.observation.evidence.observation_id,
                    rejected_observation_id: candidate.observation.evidence.observation_id,
                });
            }
            let (file_id, start_byte, end_byte) = candidate.observation.fact.span();
            evidence.push(FactEvidenceRow {
                evidence_id: evidence_id(
                    candidate.provider_run_id,
                    candidate.observation.evidence.observation_id,
                    fact_id,
                ),
                scope: candidate.observation.fact.scope(),
                fact_id,
                fact_form_code: candidate.observation.fact.fact_form_code(),
                provider_code: candidate.provider_code,
                provider_version: candidate.provider_version,
                provider_run_id: candidate.provider_run_id,
                observation_id: candidate.observation.evidence.observation_id,
                raw_kind_code: candidate.observation.evidence.raw_kind_code,
                file_id,
                start_byte,
                end_byte,
                certainty_code: candidate.observation.evidence.certainty_code,
                resolution_code: candidate.observation.evidence.resolution_code,
                conflict_disposition_code: if is_conflict { 20 } else { 10 },
                cold_payload: candidate.observation.evidence.cold_payload,
            });
        }
    }
    (selected, evidence, conflicts)
}

/// Sole production canonicalization and cross-provider reconciliation boundary.
#[derive(Clone, Debug, Default)]
pub struct CanonicalReconciliationEngine {
    counters: Arc<IngestCounters>,
}

impl CanonicalReconciliationEngine {
    /// Return cumulative functional counters for this ingress instance.
    #[must_use]
    pub fn metrics(&self) -> IngestMetrics {
        self.counters.snapshot()
    }

    /// Project one immutable source image and its complete syntax observations through
    /// this same canonical ingress and validation boundary.
    ///
    /// # Errors
    ///
    /// Rejects stale/mismatched provider images, cross-context facts, invalid ranges,
    /// identity failures, row-budget overflow, or any generated batch-validation failure.
    #[cfg(feature = "daemon")]
    pub fn ingest_source_syntax(
        &self,
        expected_scope: FactScope,
        source: &crate::source_image::SourceImage,
        tree: &crate::tree_sitter_adapter::TreeSitterSnapshot,
        ruff: Option<&crate::ruff_adapter::RuffSnapshot>,
        runs: crate::source_syntax::SourceSyntaxProviderRuns,
    ) -> Result<CanonicalIngestOutput, FactIngestError> {
        let result = crate::source_syntax::project(expected_scope, source, tree, ruff, runs);
        match &result {
            Ok(output) => self.counters.record_success(output.metrics),
            Err(_) => {
                self.counters
                    .validation_failures
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        result
    }

    /// Reconcile N typed observation streams using an explicit provider-precedence map.
    ///
    /// # Errors
    ///
    /// Rejects limit, manifest, fingerprint, terminal, workspace/context/generation, Arrow,
    /// ontology, row-local, or primary-key violations before returning canonical batches.
    pub fn ingest(
        &self,
        expected_scope: FactScope,
        streams: &[ObservationStream],
        provider_precedence: &BTreeMap<i16, u16>,
    ) -> Result<CanonicalIngestOutput, FactIngestError> {
        let result = Self::ingest_once(expected_scope, streams, provider_precedence);
        match &result {
            Ok(output) => self.counters.record_success(output.metrics),
            Err(_) => {
                self.counters
                    .validation_failures
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        result
    }

    fn ingest_once(
        expected_scope: FactScope,
        streams: &[ObservationStream],
        provider_precedence: &BTreeMap<i16, u16>,
    ) -> Result<CanonicalIngestOutput, FactIngestError> {
        if streams.len() > MAX_STREAMS {
            return Err(FactIngestError::Protocol("stream budget exceeded".into()));
        }
        let mut metrics = IngestMetrics::default();
        let candidates =
            collect_candidates(expected_scope, streams, provider_precedence, &mut metrics)?;
        let (selected, evidence, conflicts) = reconcile_candidates(candidates, provider_precedence);
        let encoded = encode_selected(selected, &evidence)?;
        metrics.rows_encoded = encoded
            .values()
            .map(RecordBatch::num_rows)
            .map(|rows| u64::try_from(rows).unwrap_or(u64::MAX))
            .sum();
        let batches = encoded
            .into_iter()
            .map(|(table_code, batch)| {
                ValidatedFactBatch::validate(table_code, batch, expected_scope)
                    .map(|batch| (table_code, batch))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        metrics.conflicts = u64::try_from(conflicts.len()).unwrap_or(u64::MAX);
        let diagnostics = conflicts
            .iter()
            .map(|conflict| IngestDiagnostic {
                code: "FACT_RECONCILIATION_CONFLICT",
                detail: format!(
                    "table {} fact selected provider {} over provider {}",
                    conflict.table_code,
                    conflict.selected_provider_code,
                    conflict.rejected_provider_code
                ),
                file_id: None,
                start_byte: None,
                end_byte: None,
            })
            .collect();
        Ok(CanonicalIngestOutput {
            batches,
            conflicts,
            diagnostics,
            metrics,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use arrow_array::builder::BinaryBuilder;
    use arrow_array::{ArrayRef, RecordBatch};
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ConflictFixture {
        fixture_id: String,
        workspace_id: String,
        analysis_context_id: String,
        owner_id: String,
        source_generation: i64,
        fact_id: String,
        source_id: String,
        selected_target_id: String,
        rejected_target_id: String,
        providers: Vec<FixtureProvider>,
        expected: FixtureExpected,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FixtureProvider {
        provider_code: i16,
        provider_version: String,
        provider_run_id: String,
        observation_id: String,
        precedence: u16,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FixtureExpected {
        relation_rows: usize,
        evidence_rows: usize,
        conflict_rows: usize,
        selected_provider_code: i16,
        rejected_provider_code: i16,
    }

    fn hex16(value: &str) -> [u8; 16] {
        assert_eq!(value.len(), 32);
        let mut output = [0_u8; 16];
        for (index, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            let high = char::from(pair[0]).to_digit(16).expect("hex high");
            let low = char::from(pair[1]).to_digit(16).expect("hex low");
            output[index] = u8::try_from((high << 4) | low).expect("one byte");
        }
        output
    }

    fn fixture() -> ConflictFixture {
        serde_json::from_str(include_str!(
            "../contracts/fixtures/synthetic/conflicting-observations-v1.json"
        ))
        .expect("typed synthetic fixture")
    }

    fn scope() -> FactScope {
        FactScope {
            workspace_id: [1; 16],
            analysis_context_id: [2; 16],
            source_generation: 7,
            owner_id: [3; 16],
        }
    }

    fn entity(entity_id: [u8; 16]) -> EntityRow {
        EntityRow {
            scope: scope(),
            entity_id,
            language: 10,
            entity_family_code: 1,
            entity_kind_code: 10,
            raw_kind_code: None,
            file_id: None,
            start_byte: None,
            end_byte: None,
            name: None,
            qualified_name: None,
            parent_entity_id: None,
            type_id: None,
            flags: 0,
            fact_hash64: 1,
        }
    }

    fn relation(fact_id: [u8; 16], target_id: [u8; 16]) -> RelationRow {
        RelationRow {
            scope: scope(),
            fact_id,
            language: 10,
            relation_family_code: 2,
            relation_kind_code: 10,
            source_id: [5; 16],
            target_id,
            ordinal: None,
            role_code: None,
            distance: None,
            directness_code: 10,
            file_id: Some([12; 16]),
            start_byte: Some(4),
            end_byte: Some(9),
            certainty_code: 10,
            resolution_code: 10,
            producer_code: 10,
            derivation_code: None,
            flags: 0,
            fact_hash64: 2,
        }
    }

    fn property(fact_id: [u8; 16], value: PropertyValue) -> PropertyFactRow {
        PropertyFactRow {
            scope: scope(),
            fact_id,
            subject_entity_id: [5; 16],
            property_kind_code: 10,
            program_point_entity_id: None,
            value,
            directness_code: 10,
            certainty_code: 10,
            resolution_code: 10,
            producer_code: 10,
            derivation_code: None,
            file_id: None,
            start_byte: None,
            end_byte: None,
            fact_hash64: 3,
        }
    }

    fn replace_column(batch: &RecordBatch, name: &str, replacement: ArrayRef) -> RecordBatch {
        let mut columns = batch.columns().to_vec();
        let index = batch.schema().index_of(name).expect("test column");
        columns[index] = replacement;
        RecordBatch::try_new(batch.schema(), columns).expect("compatible test batch")
    }

    fn manifest(
        provider: &FixtureProvider,
        scope: FactScope,
        table_code: i16,
    ) -> ObservationManifest {
        ObservationManifest {
            stream_id: hex16(&provider.provider_run_id),
            workspace_id: scope.workspace_id,
            analysis_context_id: scope.analysis_context_id,
            source_generation: scope.source_generation,
            provider_code: provider.provider_code,
            provider_version: provider.provider_version.clone(),
            provider_run_id: hex16(&provider.provider_run_id),
            schema_fingerprints: BTreeMap::from([(
                table_code,
                table_spec(table_code)
                    .expect("fact table")
                    .schema_digest
                    .clone(),
            )]),
            declared_rows: 1,
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn wp20_behavioral_acceptance() {
        let fixture = fixture();
        assert_eq!(
            fixture.fixture_id,
            "codefabric.synthetic.conflicting-observations-v1"
        );
        let expected_scope = FactScope {
            workspace_id: hex16(&fixture.workspace_id),
            analysis_context_id: hex16(&fixture.analysis_context_id),
            source_generation: fixture.source_generation,
            owner_id: hex16(&fixture.owner_id),
        };
        let fact_id = hex16(&fixture.fact_id);
        let source_id = hex16(&fixture.source_id);
        let targets = [
            hex16(&fixture.selected_target_id),
            hex16(&fixture.rejected_target_id),
        ];
        let streams = fixture
            .providers
            .iter()
            .zip(targets)
            .map(|(provider, target_id)| {
                let mut row = relation(fact_id, target_id);
                row.scope = expected_scope;
                row.source_id = source_id;
                ObservationStream {
                    manifest: manifest(provider, expected_scope, 110),
                    observations: vec![ObservedFact {
                        fact: CanonicalFact::Relation(row),
                        evidence: ObservationEvidence {
                            observation_id: hex16(&provider.observation_id),
                            raw_kind_code: Some(10),
                            certainty_code: 10,
                            resolution_code: 10,
                            cold_payload: None,
                        },
                    }],
                    terminal: StreamTerminal::Completed,
                }
            })
            .collect::<Vec<_>>();
        let precedence = fixture
            .providers
            .iter()
            .map(|provider| (provider.provider_code, provider.precedence))
            .collect::<BTreeMap<_, _>>();
        let output = CanonicalReconciliationEngine::default()
            .ingest(expected_scope, &streams, &precedence)
            .expect("synthetic conflict ingests");
        assert_eq!(
            output.batches[&110].num_rows(),
            fixture.expected.relation_rows
        );
        assert_eq!(
            output.batches[&130].num_rows(),
            fixture.expected.evidence_rows
        );
        assert_eq!(output.conflicts.len(), fixture.expected.conflict_rows);
        assert_eq!(
            output.conflicts[0].selected_provider_code,
            fixture.expected.selected_provider_code
        );
        assert_eq!(
            output.conflicts[0].rejected_provider_code,
            fixture.expected.rejected_provider_code
        );
        assert_eq!(
            binary_column(
                output.batches[&110].batch(),
                table_spec(110).unwrap(),
                "target_id",
            )
            .value(0),
            targets[0]
        );

        let empty = encode_entities(&[]).expect("empty batch");
        validate_fact_batch(&empty, 100, expected_scope).expect("empty validates");
        let mut one = entity([21; 16]);
        one.scope = expected_scope;
        one.name = Some("x".repeat(1_048_576));
        let one_batch = encode_entities(&[one.clone()]).expect("one maximum-length row");
        validate_fact_batch(&one_batch, 100, expected_scope).expect("one row validates");
        let duplicate = encode_entities(&[one.clone(), one.clone()]).expect("duplicate encodes");
        assert!(matches!(
            validate_fact_batch(&duplicate, 100, expected_scope),
            Err(FactIngestError::BatchInvalid {
                check: "primary-key",
                ..
            })
        ));
        one.start_byte = Some(9);
        one.end_byte = Some(4);
        let malformed = encode_entities(&[one]).expect("malformed span encodes");
        assert!(matches!(
            validate_fact_batch(&malformed, 100, expected_scope),
            Err(FactIngestError::BatchInvalid { check: "span", .. })
        ));
        let mut short = BinaryBuilder::new();
        short.append_value([1_u8; 15]);
        let invalid_width = replace_column(&one_batch, "entity_id", Arc::new(short.finish()));
        assert!(matches!(
            validate_fact_batch(&invalid_width, 100, expected_scope),
            Err(FactIngestError::BatchInvalid {
                check: "fixed-width",
                ..
            })
        ));
    }

    #[test]
    fn wp20_structural_acceptance() {
        let values = vec![
            PropertyValue::Entity([11; 16]),
            PropertyValue::Boolean(true),
            PropertyValue::Integer(-1),
            PropertyValue::Float(1.25),
            PropertyValue::Text("value".into()),
            PropertyValue::Bytes(vec![0, 1, 2]),
            PropertyValue::Type([12; 16]),
        ];
        let rows = values
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                let mut fact_id = [30; 16];
                fact_id[15] = u8::try_from(index).expect("seven values");
                property(fact_id, value)
            })
            .collect::<Vec<_>>();
        let batch = encode_properties(&rows).expect("all property forms encode");
        validate_fact_batch(&batch, 120, scope()).expect("all property forms validate");
        for row in 0..batch.num_rows() {
            let populated = [
                "value_entity_id",
                "value_bool",
                "value_int64",
                "value_float64",
                "value_text",
                "value_bytes",
                "value_type_id",
            ]
            .iter()
            .filter(|name| !column(&batch, table_spec(120).unwrap(), name).is_null(row))
            .count();
            assert_eq!(populated, 1);
        }
        for table_code in [100, 110, 120, 130] {
            let spec = table_spec(table_code).unwrap();
            for name in [
                "workspace_id",
                "analysis_context_id",
                "source_generation",
                "owner_id",
            ] {
                assert!(
                    spec.arrow_schema.index_of(name).is_ok(),
                    "{table_code}:{name}"
                );
            }
        }
    }

    #[tokio::test]
    async fn wp20_negative_zero_state() {
        let mut wrong_workspace = entity([20; 16]);
        wrong_workspace.scope.workspace_id = [99; 16];
        assert!(matches!(
            validate_fact_batch(&encode_entities(&[wrong_workspace]).unwrap(), 100, scope()),
            Err(FactIngestError::SourceSnapshotMismatch(_))
        ));
        let mut wrong_context = entity([23; 16]);
        wrong_context.scope.analysis_context_id = [77; 16];
        assert!(matches!(
            validate_fact_batch(&encode_entities(&[wrong_context]).unwrap(), 100, scope()),
            Err(FactIngestError::SourceSnapshotMismatch(_))
        ));
        let mut stale = entity([21; 16]);
        stale.scope.source_generation += 1;
        assert!(matches!(
            validate_fact_batch(&encode_entities(&[stale]).unwrap(), 100, scope()),
            Err(FactIngestError::StaleResult(_))
        ));
        let mut wrong_owner = entity([22; 16]);
        wrong_owner.scope.owner_id = [88; 16];
        assert!(matches!(
            validate_fact_batch(&encode_entities(&[wrong_owner]).unwrap(), 100, scope()),
            Err(FactIngestError::BatchInvalid { check: "owner", .. })
        ));

        let (sender, mut receiver) = bounded_observation_channel(1);
        sender
            .send(ObservationMessage::Batch(Vec::new()))
            .await
            .expect("first message fits");
        assert!(
            sender
                .try_send(ObservationMessage::Terminal(StreamTerminal::Completed))
                .is_err()
        );
        drop(sender);
        assert!(matches!(
            receive_observation_stream(&mut receiver).await,
            Err(FactIngestError::Protocol(message)) if message == "batch preceded manifest"
        ));

        let provider = FixtureProvider {
            provider_code: 10,
            provider_version: "1".into(),
            provider_run_id: "08080808080808080808080808080808".into(),
            observation_id: "09090909090909090909090909090909".into(),
            precedence: 0,
        };
        let mut bad_manifest = manifest(&provider, scope(), 110);
        bad_manifest
            .schema_fingerprints
            .insert(110, "b3:wrong".into());
        let stream = ObservationStream {
            manifest: bad_manifest,
            observations: vec![ObservedFact {
                fact: CanonicalFact::Relation(relation([4; 16], [6; 16])),
                evidence: ObservationEvidence {
                    observation_id: [9; 16],
                    raw_kind_code: None,
                    certainty_code: 10,
                    resolution_code: 10,
                    cold_payload: None,
                },
            }],
            terminal: StreamTerminal::Completed,
        };
        assert!(matches!(
            CanonicalReconciliationEngine::default().ingest(
                scope(),
                &[stream],
                &BTreeMap::from([(10, 0)]),
            ),
            Err(FactIngestError::SourceSnapshotMismatch(_))
        ));

        let fabric_source = include_str!("fabric.rs");
        assert!(!fabric_source.contains("pub delta: DeltaTable"));
        assert!(!fabric_source.contains("pub provider: Arc<dyn TableProvider>"));
        let boundary_rule = include_str!("../rules/deltalake-boundary-only.yml");
        assert!(boundary_rule.contains("ignores:\n  - src/fabric.rs"));
    }

    #[test]
    fn wp20_operational_acceptance() {
        let provider = FixtureProvider {
            provider_code: 10,
            provider_version: "1".into(),
            provider_run_id: "08080808080808080808080808080808".into(),
            observation_id: "09090909090909090909090909090909".into(),
            precedence: 0,
        };
        let stream = ObservationStream {
            manifest: manifest(&provider, scope(), 100),
            observations: vec![ObservedFact {
                fact: CanonicalFact::Entity(entity([4; 16])),
                evidence: ObservationEvidence {
                    observation_id: [9; 16],
                    raw_kind_code: None,
                    certainty_code: 10,
                    resolution_code: 10,
                    cold_payload: None,
                },
            }],
            terminal: StreamTerminal::Completed,
        };
        let ingress = CanonicalReconciliationEngine::default();
        let output = ingress
            .ingest(
                scope(),
                &[stream],
                &BTreeMap::from([(provider.provider_code, provider.precedence)]),
            )
            .expect("operational ingest");
        assert_eq!(output.metrics.streams_received, 1);
        assert_eq!(output.metrics.rows_received, 1);
        assert_eq!(output.metrics.rows_encoded, 2);
        assert_eq!(output.metrics.validation_failures, 0);
        assert_eq!(output.metrics.conflicts, 0);
        let mut invalid = manifest(&provider, scope(), 100);
        invalid.schema_fingerprints.insert(100, "b3:invalid".into());
        let rejected = ObservationStream {
            manifest: invalid,
            observations: vec![ObservedFact {
                fact: CanonicalFact::Entity(entity([5; 16])),
                evidence: ObservationEvidence {
                    observation_id: [10; 16],
                    raw_kind_code: None,
                    certainty_code: 10,
                    resolution_code: 10,
                    cold_payload: None,
                },
            }],
            terminal: StreamTerminal::Completed,
        };
        assert!(
            ingress
                .ingest(
                    scope(),
                    &[rejected],
                    &BTreeMap::from([(provider.provider_code, provider.precedence)]),
                )
                .is_err()
        );
        assert_eq!(ingress.metrics().validation_failures, 1);
    }

    #[test]
    fn wp36_behavioral_acceptance() {
        wp20_behavioral_acceptance();
    }

    #[test]
    fn wp36_operational_acceptance() {
        wp20_operational_acceptance();
    }
}
