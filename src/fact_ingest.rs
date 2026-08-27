//! Typed Arrow encoders and the bounded canonical fact-ingest boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use arrow::ipc::reader::StreamDecoder;
use arrow_array::builder::{
    BinaryBuilder, BooleanBuilder, FixedSizeBinaryBuilder, Float64Builder, Int16Builder,
    Int32Builder, Int64Builder, ListBuilder, StringBuilder, TimestampMicrosecondBuilder,
};
use arrow_array::{
    Array as _, ArrayRef, BinaryArray, FixedSizeBinaryArray, Int16Array, Int32Array, Int64Array,
    ListArray, RecordBatch,
};
use arrow_buffer::Buffer;
use arrow_row::{RowConverter, SortField};
use arrow_schema::{ArrowError, DataType, SchemaRef};
use arrow_select::concat::concat_batches;
use thiserror::Error;
use tokio::sync::mpsc;

use crate::cancellation::Cancellation;
use crate::registries::ValueKind;
use crate::schema_registry::{DurableMutationClass, TableSpec, table_spec};

const MAX_STREAMS: usize = 64;
const MAX_ROWS_PER_STREAM: usize = 65_536;
const MAX_BATCHES_PER_STREAM: usize = 256;
const MAX_IPC_BYTES_PER_STREAM: usize = 16 * 1_024 * 1_024;

/// Stable failures at the only canonical fact-ingest boundary.
#[derive(Debug, Error)]
pub enum FactIngestError {
    #[error("SOURCE_SNAPSHOT_MISMATCH:{0}")]
    SourceSnapshotMismatch(String),
    #[error("SOURCE_SNAPSHOT_MISMATCH:STALE_RESULT:{0}")]
    StaleResult(String),
    #[error("PROVIDER_PROTOCOL_ERROR:FACT_BATCH_INVALID:{table}:{check}:{detail}")]
    BatchInvalid {
        table: String,
        check: &'static str,
        detail: String,
    },
    #[error("PROVIDER_PROTOCOL_ERROR:OBSERVATION_PROTOCOL_INVALID:{0}")]
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
            Self::Entity(_) => ValueKind::Entity as i16,
            Self::Boolean(_) => ValueKind::Boolean as i16,
            Self::Integer(_) => ValueKind::Integer as i16,
            Self::Float(_) => ValueKind::Float as i16,
            Self::Text(_) => ValueKind::Text as i16,
            Self::Bytes(_) => ValueKind::Bytes as i16,
            Self::Type(_) => ValueKind::Type as i16,
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

/// One persisted diagnostic emitted by the common ingest accumulator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticRow {
    pub diagnostic_id: [u8; 16],
    pub workspace_id: [u8; 16],
    pub analysis_context_id: Option<[u8; 16]>,
    pub source_generation: i64,
    pub owner_id: Option<[u8; 16]>,
    pub diagnostic_code: i32,
    pub severity_code: i16,
    pub message: String,
    pub cold_payload: Option<Vec<u8>>,
    pub created_at_micros: i64,
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
    pub source_ordinal: Option<i32>,
    pub evaluation_ordinal: Option<i32>,
    pub line: Option<i32>,
    pub column: Option<i32>,
    pub depth: Option<i32>,
    pub provider_name: Option<String>,
    pub named: bool,
    pub extra: bool,
    pub error: bool,
    pub missing: bool,
    pub explicitly_parenthesized: bool,
    pub provider_node_flags: i64,
}

/// One canonical semantic type entity derived from the application-owned type algebra.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDetailRow {
    pub scope: FactScope,
    pub type_id: [u8; 16],
    pub type_kind_code: i32,
    pub canonical_key: String,
    pub display_name: Option<String>,
    pub primitive_code: Option<i16>,
    pub nominal_entity_id: Option<[u8; 16]>,
    pub callable_entity_id: Option<[u8; 16]>,
    pub raw_shape_hash: Option<[u8; 32]>,
    pub nullable_semantics_code: Option<i16>,
    pub flags: i64,
}

/// One canonical relation extension retaining the precise role and origin of a type fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeFactDetailRow {
    pub scope: FactScope,
    pub relation_id: [u8; 16],
    pub subject_id: [u8; 16],
    pub type_id: [u8; 16],
    pub type_role_code: i16,
    pub program_point_id: Option<[u8; 16]>,
    pub origin_code: i16,
    pub certainty_code: i16,
}

/// One canonical Python scope extension row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeDetailRow {
    pub scope: FactScope,
    pub scope_id: [u8; 16],
    pub parent_scope_id: Option<[u8; 16]>,
    pub scope_kind: String,
    pub name: Option<String>,
    pub start_byte: i64,
    pub end_byte: i64,
}

/// One canonical Python binding extension row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingDetailRow {
    pub scope: FactScope,
    pub binding_id: [u8; 16],
    pub scope_id: [u8; 16],
    pub name: String,
    pub binding_kind: String,
    pub target_form: String,
    pub start_byte: i64,
    pub end_byte: i64,
}

/// One canonical Python reference extension row. Unknown resolution must carry
/// a non-null reason code and a concrete `UNKNOWN_SYMBOL` target entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceDetailRow {
    pub scope: FactScope,
    pub reference_id: [u8; 16],
    pub scope_id: [u8; 16],
    pub target_id: [u8; 16],
    pub name: String,
    pub reference_class: String,
    pub resolution: String,
    pub start_byte: i64,
    pub end_byte: i64,
    pub unknown_reason_code: Option<String>,
}

/// One canonical module import occurrence. Nullable endpoints remain null only
/// when the static provider cannot establish that distinct fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleImportDetailRow {
    pub scope: FactScope,
    pub import_id: [u8; 16],
    pub source_module_id: [u8; 16],
    pub target_module_id: Option<[u8; 16]>,
    pub imported_entity_id: Option<[u8; 16]>,
    pub local_binding_id: Option<[u8; 16]>,
    pub import_kind_code: i16,
    pub relative_level: Option<i16>,
    pub source_name: String,
    pub alias_name: Option<String>,
    pub star_import: bool,
    pub unknown_reason_code: Option<i16>,
}

/// One canonical callable semantic extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDetailRow {
    pub scope: FactScope,
    pub callable_id: [u8; 16],
    pub signature_id: Option<[u8; 16]>,
    pub return_type_id: Option<[u8; 16]>,
    pub parameter_count: i32,
    pub generic_parameter_count: i32,
    pub calling_convention_code: Option<i16>,
    pub abi_name: Option<String>,
    pub callable_flags: i64,
}

/// One canonical callable parameter extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterDetailRow {
    pub scope: FactScope,
    pub parameter_id: [u8; 16],
    pub callable_id: [u8; 16],
    pub ordinal: i32,
    pub name: Option<String>,
    pub parameter_kind_code: i16,
    pub type_id: Option<[u8; 16]>,
    pub default_syntax_id: Option<[u8; 16]>,
    pub flags: i64,
}

/// One canonical first-class call-site extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallSiteDetailRow {
    pub scope: FactScope,
    pub call_site_id: [u8; 16],
    pub caller_id: [u8; 16],
    pub syntax_id: Option<[u8; 16]>,
    pub callee_syntax_id: Option<[u8; 16]>,
    pub receiver_value_id: Option<[u8; 16]>,
    pub result_value_id: Option<[u8; 16]>,
    pub dispatch_kind_code: i16,
    pub declared_target_id: Option<[u8; 16]>,
    pub resolved_target_count: i32,
    pub call_flags: i64,
}

/// One explicit or binder-synthesized call argument extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallArgumentDetailRow {
    pub scope: FactScope,
    pub argument_id: [u8; 16],
    pub call_site_id: [u8; 16],
    pub ordinal: i32,
    pub keyword_name: Option<String>,
    pub argument_syntax_id: Option<[u8; 16]>,
    pub argument_value_id: Option<[u8; 16]>,
    pub parameter_id: Option<[u8; 16]>,
    pub binding_status_code: i16,
    pub spread_kind_code: Option<i16>,
}

/// One control-flow graph header owned by a module or callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CfgGraphRow {
    pub scope: FactScope,
    pub cfg_id: [u8; 16],
    pub callable_id: Option<[u8; 16]>,
    pub cfg_kind_code: i16,
    pub entry_node_id: [u8; 16],
    pub exit_node_id: [u8; 16],
    pub exceptional_exit_node_id: Option<[u8; 16]>,
    pub node_count: i32,
    pub edge_count: i32,
    pub flags: i64,
}

/// One control-flow node entity extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CfgNodeDetailRow {
    pub scope: FactScope,
    pub cfg_node_id: [u8; 16],
    pub cfg_id: [u8; 16],
    pub node_kind_code: i16,
    pub syntax_id: Option<[u8; 16]>,
    pub mir_statement_id: Option<[u8; 16]>,
    pub ordinal: Option<i32>,
    pub flags: i64,
}

/// One control-flow relation extension with columnar branch/exception payloads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CfgEdgeDetailRow {
    pub scope: FactScope,
    pub relation_id: [u8; 16],
    pub cfg_id: [u8; 16],
    pub condition_id: Option<[u8; 16]>,
    pub case_value_text: Option<String>,
    pub case_value_hash: Option<i64>,
    pub exception_type_id: Option<[u8; 16]>,
    pub edge_flags: i64,
}

fn binary<T>(rows: &[T], mut value: impl for<'a> FnMut(&'a T) -> Option<&'a [u8]>) -> ArrayRef {
    let mut builder = BinaryBuilder::with_capacity(rows.len(), rows.len().saturating_mul(16));
    for row in rows {
        builder.append_option(value(row));
    }
    Arc::new(builder.finish())
}

fn id16s<T>(rows: &[T], mut value: impl for<'a> FnMut(&'a T) -> Option<&'a [u8; 16]>) -> ArrayRef {
    let mut builder = FixedSizeBinaryBuilder::with_capacity(rows.len(), 16);
    for row in rows {
        if let Some(value) = value(row) {
            builder
                .append_value(value)
                .expect("typed Id16 always has the governed storage width");
        } else {
            builder.append_null();
        }
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

fn generated_fact_batch(
    table_code: i16,
    columns: Vec<ArrayRef>,
) -> Result<RecordBatch, FactIngestError> {
    let spec = table_spec(table_code).expect("generated universal fact table");
    Ok(RecordBatch::try_new(
        Arc::clone(&spec.arrow_schema),
        columns,
    )?)
}

/// Encode persisted diagnostic rows in exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if the generated diagnostic schema changes incompatibly.
pub fn encode_diagnostics(rows: &[DiagnosticRow]) -> Result<RecordBatch, FactIngestError> {
    let mut timestamps =
        TimestampMicrosecondBuilder::with_capacity(rows.len()).with_timezone("UTC");
    for row in rows {
        timestamps.append_value(row.created_at_micros);
    }
    generated_fact_batch(
        10,
        vec![
            id16s(rows, |row| Some(&row.diagnostic_id)),
            id16s(rows, |row| Some(&row.workspace_id)),
            id16s(rows, |row| row.analysis_context_id.as_ref()),
            i64s(rows, |row| Some(row.source_generation)),
            id16s(rows, |row| row.owner_id.as_ref()),
            i32s(rows, |row| Some(row.diagnostic_code)),
            i16s(rows, |row| Some(row.severity_code)),
            utf8(rows, |row| Some(row.message.as_str())),
            binary(rows, |row| row.cold_payload.as_deref()),
            Arc::new(timestamps.finish()),
        ],
    )
}

include!("generated/fact_row_encoders.rs");
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

fn id16_column<'a>(
    batch: &'a RecordBatch,
    spec: &TableSpec,
    name: &str,
) -> &'a FixedSizeBinaryArray {
    column(batch, spec, name)
        .as_any()
        .downcast_ref::<FixedSizeBinaryArray>()
        .expect("generated Id16 column")
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
        match field
            .metadata()
            .get("com.codefabric.cpg.semantic_type")
            .map(String::as_str)
        {
            Some("id16") => {
                if field
                    .try_extension_type::<crate::schema_registry::Id16Extension>()
                    .is_err()
                    || column(batch, spec, field.name()).data_type()
                        != &DataType::FixedSizeBinary(16)
                {
                    return Err(invalid(
                        spec,
                        "id16-extension",
                        format!(
                            "{} lacks the governed codefabric.id16 contract",
                            field.name()
                        ),
                    ));
                }
            }
            Some("hash32") => {
                let array = binary_column(batch, spec, field.name());
                if array.iter().flatten().any(|value| value.len() != 32) {
                    return Err(invalid(
                        spec,
                        "fixed-width",
                        format!("{} is not 32 bytes", field.name()),
                    ));
                }
            }
            _ => {}
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
        let identities = id16_column(batch, spec, identity);
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

fn semantic_code_registered(
    binding: &crate::schema_registry::SemanticTypeBindingSpec,
    code: i64,
) -> bool {
    use crate::schema_registry::SemanticAuthority;

    match binding.authority {
        SemanticAuthority::EnumRegistry => u16::try_from(code).is_ok_and(|code| {
            binding
                .domain
                .and_then(enum_domain)
                .is_some_and(|values| values.iter().any(|entry| entry.code == code))
        }),
        SemanticAuthority::TypeAlgebra => {
            binding.domain == Some("TYPE_CONSTRUCTOR") && (1..=35).contains(&code)
        }
        SemanticAuthority::OntologyEntityRegistry => match binding.domain {
            Some("ENTITY_KIND") => i32::try_from(code).is_ok_and(|code| {
                crate::registries::ENTITY_KIND_CODES
                    .iter()
                    .any(|entry| entry.code == code)
            }),
            Some("ENTITY_FAMILY") => i16::try_from(code).is_ok_and(|code| {
                crate::registries::ENTITY_KIND_CODES
                    .iter()
                    .any(|entry| entry.family_code == code)
            }),
            _ => false,
        },
        SemanticAuthority::OntologyRelationRegistry => match binding.domain {
            Some("RELATION_KIND") => i32::try_from(code).is_ok_and(|code| {
                crate::registries::RELATION_KIND_CODES
                    .iter()
                    .any(|entry| entry.code == code)
            }),
            Some("RELATION_FAMILY") => i16::try_from(code).is_ok_and(|code| {
                crate::registries::RELATION_KIND_CODES
                    .iter()
                    .any(|entry| entry.family_code == code)
            }),
            _ => false,
        },
        SemanticAuthority::OntologyPropertyRegistry => {
            binding.domain == Some("PROPERTY_KIND")
                && i32::try_from(code).is_ok_and(|code| {
                    crate::registries::PROPERTY_KIND_CODES
                        .iter()
                        .any(|entry| entry.code == code)
                })
        }
        SemanticAuthority::OntologyFactRegistry => {
            binding.domain == Some("FACT_KIND")
                && i32::try_from(code).is_ok_and(|code| {
                    crate::registries::FACT_KIND_CODES
                        .iter()
                        .any(|entry| entry.code == code)
                })
        }
        SemanticAuthority::CapabilityRegistry => {
            binding.domain == Some("CAPABILITY")
                && u16::try_from(code).is_ok_and(|code| {
                    crate::registries::CAPABILITY_IDS.iter().any(|id| {
                        crate::registries::capability_code(id).is_some_and(|known| known == code)
                    })
                })
        }
        SemanticAuthority::SchemaIr => {
            i16::try_from(code).is_ok_and(|table_code| table_spec(table_code).is_some())
        }
        SemanticAuthority::Intrinsic
        | SemanticAuthority::ProviderCatalog
        | SemanticAuthority::DiagnosticProtocol => true,
    }
}

fn validate_registered_codes(batch: &RecordBatch, spec: &TableSpec) -> Result<(), FactIngestError> {
    for field in spec.arrow_schema.fields() {
        let Some(semantic_type) = field.metadata().get("com.codefabric.cpg.semantic_type") else {
            continue;
        };
        let binding = crate::schema_registry::semantic_type_binding(semantic_type)
            .expect("schema generator resolves every semantic type");
        if matches!(
            binding.authority,
            crate::schema_registry::SemanticAuthority::Intrinsic
                | crate::schema_registry::SemanticAuthority::ProviderCatalog
                | crate::schema_registry::SemanticAuthority::DiagnosticProtocol
        ) {
            continue;
        }
        let array = column(batch, spec, field.name());
        for row in 0..batch.num_rows() {
            if array.is_null(row) {
                continue;
            }
            let code = if let Some(values) = array.as_any().downcast_ref::<Int16Array>() {
                i64::from(values.value(row))
            } else if let Some(values) = array.as_any().downcast_ref::<Int32Array>() {
                i64::from(values.value(row))
            } else {
                continue;
            };
            if !semantic_code_registered(binding, code) {
                return Err(invalid(
                    spec,
                    "semantic-code",
                    format!(
                        "{} value {code} is absent from {}",
                        field.name(),
                        semantic_type
                    ),
                ));
            }
        }
    }
    match spec.table_code {
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
        _ => {}
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
    let owner_scoped = matches!(
        spec.durable_mutation,
        DurableMutationClass::OwnerReplacedFact | DurableMutationClass::DerivedOwnerReplaced
    ) && [
        "workspace_id",
        "analysis_context_id",
        "source_generation",
        "owner_id",
    ]
    .iter()
    .all(|column| spec.arrow_schema.index_of(column).is_ok());
    let diagnostic =
        table_code == 10 && spec.durable_mutation == DurableMutationClass::PublicationAppend;
    if !owner_scoped && !diagnostic {
        return Err(invalid(
            spec,
            "table-family",
            "not a generated owner-scoped fact or diagnostic table",
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
    let workspaces = id16_column(batch, spec, "workspace_id");
    let contexts = id16_column(batch, spec, "analysis_context_id");
    let generations = i64_column(batch, spec, "source_generation");
    let owners = id16_column(batch, spec, "owner_id");
    for row in 0..batch.num_rows() {
        if workspaces.value(row) != expected_scope.workspace_id
            || (!contexts.is_null(row) && contexts.value(row) != expected_scope.analysis_context_id)
        {
            return Err(FactIngestError::SourceSnapshotMismatch(spec.name.into()));
        }
        if generations.value(row) != expected_scope.source_generation {
            return Err(FactIngestError::StaleResult(spec.name.into()));
        }
        if !owners.is_null(row) && owners.value(row) != expected_scope.owner_id {
            return Err(invalid(
                spec,
                "owner",
                "row owner differs from ingest owner",
            ));
        }
    }
    Ok(())
}

/// Return the generated durable tables admitted by the schema-driven fact port.
#[must_use]
pub fn generated_ingest_table_codes() -> Vec<i16> {
    crate::schema_registry::table_specs()
        .iter()
        .filter(|spec| {
            spec.family != "overlay-control"
                && (spec.table_code == 10
                    || matches!(
                        spec.durable_mutation,
                        DurableMutationClass::OwnerReplacedFact
                            | DurableMutationClass::DerivedOwnerReplaced
                    ))
        })
        .map(|spec| spec.table_code)
        .collect()
}

/// Closed terminal state for one provider fact stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamTerminal {
    Completed,
    Partial,
    Failed,
}

/// Header that must precede every direct or IPC-decoded provider batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderFactManifest {
    pub stream_id: [u8; 16],
    pub workspace_id: [u8; 16],
    pub analysis_context_id: [u8; 16],
    pub source_generation: i64,
    pub provider_code: i16,
    pub provider_version: String,
    pub provider_run_id: [u8; 16],
    pub emitted_at_micros: i64,
    pub schema_fingerprints: BTreeMap<i16, String>,
    pub declared_rows: usize,
}

/// One generated-schema batch emitted by a provider.
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderFactBatch {
    pub table_code: i16,
    pub batch: RecordBatch,
}

/// Complete bounded stream; field order makes the manifest structurally precede batches.
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderFactStream {
    pub manifest: ProviderFactManifest,
    pub batches: Vec<ProviderFactBatch>,
    pub terminal: StreamTerminal,
}

/// Wire-state messages for producers that need backpressured asynchronous delivery.
#[derive(Clone, Debug, PartialEq)]
pub enum ProviderFactMessage {
    Manifest(ProviderFactManifest),
    Batch(ProviderFactBatch),
    Terminal(StreamTerminal),
}

/// Construct one bounded MPSC provider-fact channel.
#[must_use]
pub fn bounded_provider_fact_channel(
    capacity: usize,
) -> (
    mpsc::Sender<ProviderFactMessage>,
    mpsc::Receiver<ProviderFactMessage>,
) {
    mpsc::channel(capacity.max(1))
}

/// Receive one channel as a manifest-first, terminal-complete stream.
///
/// # Errors
///
/// Rejects batches before the manifest, duplicate headers, messages after terminal,
/// missing terminal state, row-limit overflow, and a closed channel before completion.
pub async fn receive_provider_fact_stream(
    receiver: &mut mpsc::Receiver<ProviderFactMessage>,
    cancellation: &Cancellation,
) -> Result<ProviderFactStream, FactIngestError> {
    let mut manifest = None;
    let mut batches = Vec::new();
    let mut rows = 0_usize;
    let mut terminal = None;
    loop {
        if cancellation.is_cancelled() {
            return Err(FactIngestError::Protocol(
                "provider fact stream cancelled".into(),
            ));
        }
        let message = match tokio::time::timeout(Duration::from_millis(10), receiver.recv()).await {
            Ok(Some(message)) => message,
            Ok(None) => break,
            Err(_) => continue,
        };
        if cancellation.is_cancelled() {
            return Err(FactIngestError::Protocol(
                "provider fact stream cancelled".into(),
            ));
        }
        if terminal.is_some() {
            return Err(FactIngestError::Protocol(
                "message after terminal state".into(),
            ));
        }
        match message {
            ProviderFactMessage::Manifest(value) if manifest.is_none() => manifest = Some(value),
            ProviderFactMessage::Manifest(_) => {
                return Err(FactIngestError::Protocol("duplicate manifest".into()));
            }
            ProviderFactMessage::Batch(batch) if manifest.is_some() => {
                rows = rows.saturating_add(batch.batch.num_rows());
                if rows > MAX_ROWS_PER_STREAM || batches.len() == MAX_BATCHES_PER_STREAM {
                    return Err(FactIngestError::Protocol(
                        "stream row budget exceeded".into(),
                    ));
                }
                batches.push(batch);
            }
            ProviderFactMessage::Batch(_) => {
                return Err(FactIngestError::Protocol("batch preceded manifest".into()));
            }
            ProviderFactMessage::Terminal(value) if manifest.is_some() => {
                terminal = Some(value);
                break;
            }
            ProviderFactMessage::Terminal(_) => {
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
    Ok(ProviderFactStream {
        manifest,
        batches,
        terminal,
    })
}

/// Compression admitted by the versioned external Arrow IPC contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderIpcCompression {
    None,
}

/// Versioned, resource-bounded contract for one external Arrow IPC stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderIpcContract {
    pub version: u16,
    pub codec: &'static str,
    pub compression: ProviderIpcCompression,
    pub schema_profile: &'static str,
    pub table_code: i16,
    pub schema_digest: String,
    pub declared_rows: usize,
    pub declared_bytes: usize,
}

impl ProviderIpcContract {
    pub const VERSION: u16 = 1;
    pub const CODEC: &'static str = "arrow-ipc-stream";
    pub const SCHEMA_PROFILE: &'static str = "codefabric-generated-fact-v1";

    /// Construct the sole currently supported external fact-stream profile.
    ///
    /// # Errors
    ///
    /// Returns a protocol failure for an unknown generated fact table.
    pub fn generated(
        table_code: i16,
        declared_rows: usize,
        declared_bytes: usize,
    ) -> Result<Self, FactIngestError> {
        let spec = table_spec(table_code).ok_or_else(|| {
            FactIngestError::Protocol(format!("table code {table_code} is not generated"))
        })?;
        Ok(Self {
            version: Self::VERSION,
            codec: Self::CODEC,
            compression: ProviderIpcCompression::None,
            schema_profile: Self::SCHEMA_PROFILE,
            table_code,
            schema_digest: spec.schema_digest.clone(),
            declared_rows,
            declared_bytes,
        })
    }
}

/// Incrementally decode arbitrary byte chunks into the same batch stream used in-process.
///
/// Arrow validation remains enabled. Buffer alignment is an explicit decoder policy and is
/// deliberately not part of the wire contract.
///
/// # Errors
///
/// Rejects unsupported contract values, resource-limit drift, malformed or truncated IPC,
/// schema mismatch, and declared row/byte mismatch.
pub fn decode_provider_ipc_chunks<I, B>(
    manifest: ProviderFactManifest,
    contract: &ProviderIpcContract,
    chunks: I,
) -> Result<ProviderFactStream, FactIngestError>
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    let spec = table_spec(contract.table_code).ok_or_else(|| {
        FactIngestError::Protocol(format!(
            "table code {} is not generated",
            contract.table_code
        ))
    })?;
    if contract.version != ProviderIpcContract::VERSION
        || contract.codec != ProviderIpcContract::CODEC
        || contract.compression != ProviderIpcCompression::None
        || contract.schema_profile != ProviderIpcContract::SCHEMA_PROFILE
        || contract.schema_digest != spec.schema_digest
        || contract.declared_rows > MAX_ROWS_PER_STREAM
        || contract.declared_bytes > MAX_IPC_BYTES_PER_STREAM
    {
        return Err(FactIngestError::Protocol(
            "external IPC contract differs from the supported profile".into(),
        ));
    }
    if manifest.schema_fingerprints.get(&contract.table_code) != Some(&contract.schema_digest)
        || manifest.declared_rows != contract.declared_rows
    {
        return Err(FactIngestError::Protocol(
            "external IPC manifest differs from its stream contract".into(),
        ));
    }
    let batches = decode_validated_arrow_ipc_chunks(
        &spec.arrow_schema,
        contract.declared_rows,
        contract.declared_bytes,
        chunks,
    )?
    .into_iter()
    .map(|batch| ProviderFactBatch {
        table_code: contract.table_code,
        batch,
    })
    .collect();
    Ok(ProviderFactStream {
        manifest,
        batches,
        terminal: StreamTerminal::Completed,
    })
}

/// Decode a validated Arrow IPC stream for a provider-specific or generated schema.
///
/// This is the sole external-byte decoder. It accepts arbitrary chunk boundaries, keeps Arrow
/// validation enabled, permits valid unaligned buffers by policy, and requires a complete stream.
///
/// # Errors
///
/// Rejects resource overflow, malformed/truncated IPC, schema mismatch, and row/byte drift.
pub fn decode_validated_arrow_ipc_chunks<I, B>(
    expected_schema: &SchemaRef,
    declared_rows: usize,
    declared_bytes: usize,
    chunks: I,
) -> Result<Vec<RecordBatch>, FactIngestError>
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    decode_validated_arrow_ipc_buffers(
        expected_schema,
        declared_rows,
        declared_bytes,
        chunks
            .into_iter()
            .map(|chunk| Buffer::from(chunk.as_ref().to_vec())),
    )
}

fn decode_validated_arrow_ipc_buffers<I>(
    expected_schema: &SchemaRef,
    declared_rows: usize,
    declared_bytes: usize,
    chunks: I,
) -> Result<Vec<RecordBatch>, FactIngestError>
where
    I: IntoIterator<Item = Buffer>,
{
    if declared_rows > MAX_ROWS_PER_STREAM || declared_bytes > MAX_IPC_BYTES_PER_STREAM {
        return Err(FactIngestError::Protocol(
            "external IPC declared resource budget exceeded".into(),
        ));
    }
    let mut decoder = StreamDecoder::new().with_require_alignment(false);
    let mut batches = Vec::new();
    let mut total_bytes = 0_usize;
    let mut total_rows = 0_usize;
    for mut buffer in chunks {
        total_bytes = total_bytes.saturating_add(buffer.len());
        if total_bytes > MAX_IPC_BYTES_PER_STREAM || total_bytes > declared_bytes {
            return Err(FactIngestError::Protocol(
                "external IPC byte budget exceeded".into(),
            ));
        }
        while !buffer.is_empty() {
            if let Some(batch) = decoder.decode(&mut buffer)? {
                if batch.schema().as_ref() != expected_schema.as_ref() {
                    return Err(FactIngestError::SourceSnapshotMismatch(
                        "external IPC schema".into(),
                    ));
                }
                total_rows = total_rows.saturating_add(batch.num_rows());
                if total_rows > MAX_ROWS_PER_STREAM || batches.len() == MAX_BATCHES_PER_STREAM {
                    return Err(FactIngestError::Protocol(
                        "external IPC row or batch budget exceeded".into(),
                    ));
                }
                batches.push(batch);
            }
        }
    }
    decoder.finish()?;
    if decoder.schema().as_deref() != Some(expected_schema.as_ref())
        || total_bytes != declared_bytes
        || total_rows != declared_rows
    {
        return Err(FactIngestError::Protocol(
            "external IPC declared shape differs from decoded shape".into(),
        ));
    }
    Ok(batches)
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
struct BatchCandidate {
    table_code: i16,
    provider_code: i16,
    provider_version: String,
    provider_run_id: [u8; 16],
    stream_id: [u8; 16],
    row: RecordBatch,
    row_bytes: Vec<u8>,
    fact_id: [u8; 16],
}

type CandidateGroups = BTreeMap<(i16, Vec<u8>), Vec<BatchCandidate>>;

fn evidence_id(provider_run_id: [u8; 16], observation_id: [u8; 16], fact_id: [u8; 16]) -> [u8; 16] {
    crate::identity::fact_evidence_id(provider_run_id, observation_id, fact_id)
}

fn normalized_rows(
    batch: &RecordBatch,
    indices: impl IntoIterator<Item = usize>,
) -> Result<Vec<Vec<u8>>, FactIngestError> {
    let columns = indices
        .into_iter()
        .map(|index| Arc::clone(batch.column(index)))
        .collect::<Vec<_>>();
    let fields = columns
        .iter()
        .map(|column| SortField::new(column.data_type().clone()))
        .collect::<Vec<_>>();
    let converter = RowConverter::new(fields)?;
    let rows = converter.convert_columns(&columns)?;
    Ok(rows.iter().map(|row| row.as_ref().to_vec()).collect())
}

fn row_fact_id(batch: &RecordBatch, spec: &TableSpec, row: usize) -> [u8; 16] {
    for name in [
        "fact_id",
        "entity_id",
        "token_id",
        "annotation_id",
        "file_id",
        "owner_id",
        "evidence_id",
        "diagnostic_id",
    ] {
        if spec.arrow_schema.index_of(name).is_ok() {
            let values = id16_column(batch, spec, name);
            if !values.is_null(row) {
                let mut id = [0_u8; 16];
                id.copy_from_slice(values.value(row));
                return id;
            }
        }
    }
    crate::identity::unframed_semantic_id(
        &normalized_rows(batch, 0..batch.num_columns()).expect("generated row is normalizable")
            [row],
    )
}

fn optional_id16(batch: &RecordBatch, spec: &TableSpec, name: &str) -> Option<[u8; 16]> {
    let index = spec.arrow_schema.index_of(name).ok()?;
    let values = batch
        .column(index)
        .as_any()
        .downcast_ref::<FixedSizeBinaryArray>()?;
    if values.is_null(0) {
        return None;
    }
    let mut id = [0_u8; 16];
    id.copy_from_slice(values.value(0));
    Some(id)
}

fn optional_i64(batch: &RecordBatch, spec: &TableSpec, name: &str) -> Option<i64> {
    let index = spec.arrow_schema.index_of(name).ok()?;
    let values = batch.column(index).as_any().downcast_ref::<Int64Array>()?;
    (!values.is_null(0)).then(|| values.value(0))
}

fn optional_i32(batch: &RecordBatch, spec: &TableSpec, name: &str) -> Option<i32> {
    let index = spec.arrow_schema.index_of(name).ok()?;
    let values = batch.column(index).as_any().downcast_ref::<Int32Array>()?;
    (!values.is_null(0)).then(|| values.value(0))
}

fn optional_i16(batch: &RecordBatch, spec: &TableSpec, name: &str) -> Option<i16> {
    let index = spec.arrow_schema.index_of(name).ok()?;
    let values = batch.column(index).as_any().downcast_ref::<Int16Array>()?;
    (!values.is_null(0)).then(|| values.value(0))
}

fn fact_form_code(table_code: i16) -> Option<i16> {
    let name = match table_code {
        100 => "ENTITY_EXISTENCE",
        110 => "RELATION",
        120 => "PROPERTY",
        _ => return None,
    };
    crate::registries::fact_kind_code(name).and_then(|code| i16::try_from(code).ok())
}

fn derived_observation_id(candidate: &BatchCandidate) -> [u8; 16] {
    let mut bytes = Vec::with_capacity(16 + 2 + candidate.row_bytes.len());
    bytes.extend_from_slice(&candidate.stream_id);
    bytes.extend_from_slice(&candidate.table_code.to_be_bytes());
    bytes.extend_from_slice(&candidate.row_bytes);
    crate::identity::unframed_semantic_id(&bytes)
}

fn candidate_evidence_provider(candidate: &BatchCandidate) -> i16 {
    table_spec(candidate.table_code)
        .and_then(|spec| optional_i16(&candidate.row, spec, "producer_code"))
        .unwrap_or(candidate.provider_code)
}

fn collect_candidates(
    expected_scope: FactScope,
    streams: &[ProviderFactStream],
    provider_precedence: &BTreeMap<i16, u16>,
    metrics: &mut IngestMetrics,
) -> Result<(CandidateGroups, BTreeSet<i16>), FactIngestError> {
    let mut candidates = CandidateGroups::new();
    let mut present_tables = BTreeSet::new();
    for stream in streams {
        metrics.streams_received += 1;
        if stream.terminal == StreamTerminal::Failed {
            return Err(FactIngestError::Protocol(
                "failed stream is not ingestible".into(),
            ));
        }
        let stream_rows = stream
            .batches
            .iter()
            .map(|batch| batch.batch.num_rows())
            .sum::<usize>();
        metrics.rows_received += u64::try_from(stream_rows).unwrap_or(u64::MAX);
        if stream_rows != stream.manifest.declared_rows
            || stream_rows > MAX_ROWS_PER_STREAM
            || stream.batches.len() > MAX_BATCHES_PER_STREAM
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
        for provider_batch in &stream.batches {
            let code = provider_batch.table_code;
            present_tables.insert(code);
            let spec = table_spec(code)
                .ok_or_else(|| FactIngestError::Protocol(format!("unknown fact table {code}")))?;
            if stream.manifest.schema_fingerprints.get(&code) != Some(&spec.schema_digest) {
                return Err(FactIngestError::SourceSnapshotMismatch(format!(
                    "{} schema fingerprint",
                    spec.name
                )));
            }
            validate_fact_batch(&provider_batch.batch, code, expected_scope)?;
            let key_indices = spec.primary_key.iter().map(|name| {
                spec.arrow_schema
                    .index_of(name)
                    .expect("generated primary key")
            });
            let keys = normalized_rows(&provider_batch.batch, key_indices)?;
            let row_bytes =
                normalized_rows(&provider_batch.batch, 0..provider_batch.batch.num_columns())?;
            for row in 0..provider_batch.batch.num_rows() {
                candidates
                    .entry((code, keys[row].clone()))
                    .or_default()
                    .push(BatchCandidate {
                        table_code: code,
                        provider_code: stream.manifest.provider_code,
                        provider_version: stream.manifest.provider_version.clone(),
                        provider_run_id: stream.manifest.provider_run_id,
                        stream_id: stream.manifest.stream_id,
                        row: provider_batch.batch.slice(row, 1),
                        row_bytes: row_bytes[row].clone(),
                        fact_id: row_fact_id(&provider_batch.batch, spec, row),
                    });
            }
        }
    }
    Ok((candidates, present_tables))
}

#[allow(clippy::too_many_lines)] // One ordered pass keeps precedence, evidence, and conflict selection auditable.
fn reconcile_candidates(
    candidates: CandidateGroups,
    present_tables: &BTreeSet<i16>,
    provider_precedence: &BTreeMap<i16, u16>,
) -> Result<(BTreeMap<i16, RecordBatch>, Vec<ConflictRecord>), FactIngestError> {
    let mut selected: BTreeMap<i16, Vec<RecordBatch>> = BTreeMap::new();
    let mut evidence = Vec::new();
    let mut conflicts = Vec::new();
    let mut provided_evidence = BTreeMap::new();
    for group in candidates.values() {
        for candidate in group.iter().filter(|candidate| candidate.table_code == 130) {
            let spec = table_spec(130).expect("generated evidence table");
            let fact_id =
                optional_id16(&candidate.row, spec, "fact_id").expect("validated evidence fact id");
            let provider_code = optional_i16(&candidate.row, spec, "provider_code")
                .expect("validated evidence provider code");
            let provider_run_id = optional_id16(&candidate.row, spec, "provider_run_id")
                .expect("validated evidence provider run id");
            let observation_id = optional_id16(&candidate.row, spec, "observation_id")
                .expect("validated evidence observation id");
            provided_evidence.insert((fact_id, provider_code, provider_run_id), observation_id);
            provided_evidence
                .entry((fact_id, i16::MIN, [0; 16]))
                .or_insert(observation_id);
        }
    }
    for ((table_code, _key), mut group) in candidates {
        group.sort_by_key(|candidate| {
            (
                provider_precedence
                    .get(&candidate.provider_code)
                    .copied()
                    .unwrap_or(u16::MAX),
                candidate.provider_code,
                candidate.stream_id,
            )
        });
        let Some(winner) = group.first().cloned() else {
            continue;
        };
        selected
            .entry(table_code)
            .or_default()
            .push(winner.row.clone());
        for candidate in group {
            let is_conflict = candidate.row_bytes != winner.row_bytes;
            let winner_evidence_provider = candidate_evidence_provider(&winner);
            let candidate_evidence_provider = candidate_evidence_provider(&candidate);
            let selected_observation_id = provided_evidence
                .get(&(
                    winner.fact_id,
                    winner_evidence_provider,
                    winner.provider_run_id,
                ))
                .or_else(|| provided_evidence.get(&(winner.fact_id, i16::MIN, [0; 16])))
                .copied()
                .unwrap_or_else(|| derived_observation_id(&winner));
            let candidate_observation_id = provided_evidence
                .get(&(
                    candidate.fact_id,
                    candidate_evidence_provider,
                    candidate.provider_run_id,
                ))
                .or_else(|| provided_evidence.get(&(candidate.fact_id, i16::MIN, [0; 16])))
                .copied()
                .unwrap_or_else(|| derived_observation_id(&candidate));
            if is_conflict {
                conflicts.push(ConflictRecord {
                    table_code,
                    fact_id: winner.fact_id,
                    selected_provider_code: winner.provider_code,
                    rejected_provider_code: candidate.provider_code,
                    selected_observation_id,
                    rejected_observation_id: candidate_observation_id,
                });
            }
            if let Some(form) = fact_form_code(table_code)
                && !provided_evidence.contains_key(&(
                    candidate.fact_id,
                    candidate_evidence_provider,
                    candidate.provider_run_id,
                ))
                && !provided_evidence.contains_key(&(candidate.fact_id, i16::MIN, [0; 16]))
            {
                let spec = table_spec(table_code).expect("generated fact table");
                evidence.push(FactEvidenceRow {
                    evidence_id: evidence_id(
                        candidate.provider_run_id,
                        candidate_observation_id,
                        candidate.fact_id,
                    ),
                    scope: FactScope {
                        workspace_id: optional_id16(&candidate.row, spec, "workspace_id")
                            .expect("validated workspace"),
                        analysis_context_id: optional_id16(
                            &candidate.row,
                            spec,
                            "analysis_context_id",
                        )
                        .expect("validated context"),
                        source_generation: optional_i64(&candidate.row, spec, "source_generation")
                            .expect("validated generation"),
                        owner_id: optional_id16(&candidate.row, spec, "owner_id")
                            .expect("validated owner"),
                    },
                    fact_id: candidate.fact_id,
                    fact_form_code: form,
                    provider_code: candidate_evidence_provider,
                    provider_version: candidate.provider_version,
                    provider_run_id: candidate.provider_run_id,
                    observation_id: candidate_observation_id,
                    raw_kind_code: optional_i32(&candidate.row, spec, "raw_kind_code"),
                    file_id: optional_id16(&candidate.row, spec, "file_id"),
                    start_byte: optional_i64(&candidate.row, spec, "start_byte"),
                    end_byte: optional_i64(&candidate.row, spec, "end_byte"),
                    certainty_code: optional_i16(&candidate.row, spec, "certainty_code")
                        .unwrap_or(10),
                    resolution_code: optional_i16(&candidate.row, spec, "resolution_code")
                        .unwrap_or(10),
                    conflict_disposition_code: if is_conflict { 20 } else { 10 },
                    cold_payload: None,
                });
            }
        }
    }
    if !evidence.is_empty() {
        selected
            .entry(130)
            .or_default()
            .push(encode_evidence(&evidence)?);
    }
    for table_code in present_tables {
        selected.entry(*table_code).or_insert_with(|| {
            vec![RecordBatch::new_empty(Arc::clone(
                &table_spec(*table_code)
                    .expect("present generated table")
                    .arrow_schema,
            ))]
        });
    }
    let batches = selected
        .into_iter()
        .map(|(table_code, rows)| {
            let schema = Arc::clone(&table_spec(table_code).expect("selected table").arrow_schema);
            concat_batches(&schema, rows.iter())
                .map(|batch| (table_code, batch))
                .map_err(FactIngestError::from)
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok((batches, conflicts))
}

fn diagnostic_code(name: &str) -> i32 {
    let digest = crate::integrity::digest_bytes(name.as_bytes());
    i32::from_be_bytes([digest[0] & 0x7f, digest[1], digest[2], digest[3]])
}

fn refresh_diagnostic_batch(
    output: &mut CanonicalIngestOutput,
    scope: FactScope,
    created_at_micros: i64,
) -> Result<(), FactIngestError> {
    let provided = output.batches.remove(&10);
    if output.diagnostics.is_empty() {
        if let Some(provided) = provided {
            output.batches.insert(10, provided);
        }
        return Ok(());
    }
    let rows = output
        .diagnostics
        .iter()
        .enumerate()
        .map(|(ordinal, diagnostic)| {
            let mut identity = Vec::new();
            identity.extend_from_slice(diagnostic.code.as_bytes());
            identity.extend_from_slice(&u64::try_from(ordinal).unwrap_or(u64::MAX).to_be_bytes());
            identity.extend_from_slice(diagnostic.detail.as_bytes());
            if let Some(file_id) = diagnostic.file_id {
                identity.extend_from_slice(&file_id);
            }
            DiagnosticRow {
                diagnostic_id: crate::identity::unframed_semantic_id(&identity),
                workspace_id: scope.workspace_id,
                analysis_context_id: Some(scope.analysis_context_id),
                source_generation: scope.source_generation,
                owner_id: Some(scope.owner_id),
                diagnostic_code: diagnostic_code(diagnostic.code),
                severity_code: crate::registries::Severity::Warning as i16,
                message: diagnostic.detail.clone(),
                cold_payload: None,
                created_at_micros,
            }
        })
        .collect::<Vec<_>>();
    let accumulated = encode_diagnostics(&rows)?;
    let batch = if let Some(provided) = provided {
        concat_batches(
            &table_spec(10).expect("diagnostic table").arrow_schema,
            [provided.batch(), &accumulated],
        )?
    } else {
        accumulated
    };
    let batch = ValidatedFactBatch::validate(10, batch, scope)?;
    output.batches.insert(10, batch);
    Ok(())
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
        outputs: &[crate::source_syntax::SourceSyntaxAdapterOutput<'_>],
    ) -> Result<CanonicalIngestOutput, FactIngestError> {
        let result =
            crate::source_syntax::project(expected_scope, source, outputs).and_then(|projected| {
                let provider_code = crate::registries::ProviderCode::SourceSubstrate as i16;
                let batches = projected
                    .batches
                    .iter()
                    .map(|(table_code, batch)| ProviderFactBatch {
                        table_code: *table_code,
                        batch: batch.clone(),
                    })
                    .collect::<Vec<_>>();
                let declared_rows = batches.iter().map(|batch| batch.batch.num_rows()).sum();
                let schema_fingerprints = batches
                    .iter()
                    .map(|batch| {
                        let spec = table_spec(batch.table_code).ok_or_else(|| {
                            FactIngestError::Protocol(format!(
                                "projected table {} is not generated",
                                batch.table_code
                            ))
                        })?;
                        Ok((batch.table_code, spec.schema_digest.clone()))
                    })
                    .collect::<Result<BTreeMap<_, _>, FactIngestError>>()?;
                let stream = ProviderFactStream {
                    manifest: ProviderFactManifest {
                        stream_id: source.lease.lease_id,
                        workspace_id: expected_scope.workspace_id,
                        analysis_context_id: expected_scope.analysis_context_id,
                        source_generation: expected_scope.source_generation,
                        provider_code,
                        provider_version: "source-syntax-projection-v1".into(),
                        provider_run_id: source.lease.lease_id,
                        emitted_at_micros: 0,
                        schema_fingerprints,
                        declared_rows,
                    },
                    batches,
                    terminal: StreamTerminal::Completed,
                };
                let mut admitted = Self::ingest_once(
                    expected_scope,
                    &[stream],
                    &BTreeMap::from([(provider_code, 0)]),
                )?;
                admitted.conflicts.extend(projected.conflicts);
                admitted.diagnostics.extend(projected.diagnostics);
                admitted.metrics.streams_received = projected.metrics.streams_received;
                admitted.metrics.rows_received = projected.metrics.rows_received;
                admitted.metrics.conflicts =
                    u64::try_from(admitted.conflicts.len()).unwrap_or(u64::MAX);
                refresh_diagnostic_batch(&mut admitted, expected_scope, 0)?;
                admitted.metrics.rows_encoded = admitted
                    .batches
                    .values()
                    .map(ValidatedFactBatch::num_rows)
                    .map(|rows| u64::try_from(rows).unwrap_or(u64::MAX))
                    .sum();
                Ok(admitted)
            });
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

    /// Reconcile N schema-driven provider streams using an explicit precedence map.
    ///
    /// # Errors
    ///
    /// Rejects limit, manifest, fingerprint, terminal, workspace/context/generation, Arrow,
    /// ontology, row-local, or primary-key violations before returning canonical batches.
    pub fn ingest(
        &self,
        expected_scope: FactScope,
        streams: &[ProviderFactStream],
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
        streams: &[ProviderFactStream],
        provider_precedence: &BTreeMap<i16, u16>,
    ) -> Result<CanonicalIngestOutput, FactIngestError> {
        if streams.len() > MAX_STREAMS {
            return Err(FactIngestError::Protocol("stream budget exceeded".into()));
        }
        let mut metrics = IngestMetrics::default();
        let (candidates, present_tables) =
            collect_candidates(expected_scope, streams, provider_precedence, &mut metrics)?;
        let (encoded, conflicts) =
            reconcile_candidates(candidates, &present_tables, provider_precedence)?;
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
        let mut output = CanonicalIngestOutput {
            batches,
            conflicts,
            diagnostics,
            metrics,
        };
        let emitted_at_micros = streams
            .iter()
            .map(|stream| stream.manifest.emitted_at_micros)
            .max()
            .unwrap_or(0);
        refresh_diagnostic_batch(&mut output, expected_scope, emitted_at_micros)?;
        output.metrics.rows_encoded = output
            .batches
            .values()
            .map(ValidatedFactBatch::num_rows)
            .map(|rows| u64::try_from(rows).unwrap_or(u64::MAX))
            .sum();
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use arrow::ipc::writer::StreamWriter;
    use arrow_array::RecordBatch;
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

    fn manifest(
        provider: &FixtureProvider,
        scope: FactScope,
        table_code: i16,
    ) -> ProviderFactManifest {
        ProviderFactManifest {
            stream_id: hex16(&provider.observation_id),
            workspace_id: scope.workspace_id,
            analysis_context_id: scope.analysis_context_id,
            source_generation: scope.source_generation,
            provider_code: provider.provider_code,
            provider_version: provider.provider_version.clone(),
            provider_run_id: hex16(&provider.provider_run_id),
            emitted_at_micros: 0,
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

    fn ipc_bytes(batch: &RecordBatch) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut bytes, batch.schema().as_ref())
                .expect("IPC stream writer");
            writer.write(batch).expect("IPC record batch");
            writer.finish().expect("IPC stream terminator");
        }
        bytes
    }

    fn conflicting_relation_streams() -> (FactScope, Vec<ProviderFactStream>, BTreeMap<i16, u16>) {
        let fixture = fixture();
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
                ProviderFactStream {
                    manifest: manifest(provider, expected_scope, 110),
                    batches: vec![ProviderFactBatch {
                        table_code: 110,
                        batch: encode_relations(&[row]).expect("fixture relation encodes"),
                    }],
                    terminal: StreamTerminal::Completed,
                }
            })
            .collect::<Vec<_>>();
        let precedence = fixture
            .providers
            .iter()
            .map(|provider| (provider.provider_code, provider.precedence))
            .collect();
        (expected_scope, streams, precedence)
    }

    fn assert_same_output(left: &CanonicalIngestOutput, right: &CanonicalIngestOutput) {
        assert_eq!(
            left.batches.keys().collect::<Vec<_>>(),
            right.batches.keys().collect::<Vec<_>>()
        );
        for (table_code, batch) in &left.batches {
            assert_eq!(batch.batch(), right.batches[table_code].batch());
        }
        assert_eq!(left.conflicts, right.conflicts);
        assert_eq!(left.diagnostics, right.diagnostics);
        assert_eq!(left.metrics, right.metrics);
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
                ProviderFactStream {
                    manifest: manifest(provider, expected_scope, 110),
                    batches: vec![ProviderFactBatch {
                        table_code: 110,
                        batch: encode_relations(&[row]).expect("fixture relation encodes"),
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
            id16_column(
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
        let entity_index = one_batch.schema().index_of("entity_id").unwrap();
        let mut fields = one_batch
            .schema()
            .fields()
            .iter()
            .map(|field| field.as_ref().clone())
            .collect::<Vec<_>>();
        let mut metadata = fields[entity_index].metadata().clone();
        metadata.remove(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY);
        metadata.remove(arrow_schema::extension::EXTENSION_TYPE_METADATA_KEY);
        fields[entity_index] = fields[entity_index].clone().with_metadata(metadata);
        let missing_extension = RecordBatch::try_new(
            Arc::new(arrow_schema::Schema::new_with_metadata(
                fields,
                one_batch.schema().metadata().clone(),
            )),
            one_batch.columns().to_vec(),
        )
        .expect("storage-only unknown consumer accepts fixed-size values");
        assert!(matches!(
            validate_fact_batch(&missing_extension, 100, expected_scope),
            Err(FactIngestError::BatchInvalid {
                check: "schema",
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

    #[test]
    fn common_semantic_type_rows_encode_against_generated_tables() {
        let type_row = TypeDetailRow {
            scope: scope(),
            type_id: [0x41; 16],
            type_kind_code: crate::identity::TypeConstructor::Primitive.code(),
            canonical_key: "cbef-type-v1:fixture".to_owned(),
            display_name: Some("int".to_owned()),
            primitive_code: Some(10),
            nominal_entity_id: None,
            callable_entity_id: None,
            raw_shape_hash: Some([0x42; 32]),
            nullable_semantics_code: Some(10),
            flags: 0,
        };
        let type_batch = encode_type_details(&[type_row]).expect("type detail encodes");
        validate_fact_batch(&type_batch, 180, scope()).expect("type detail validates");

        let fact_row = TypeFactDetailRow {
            scope: scope(),
            relation_id: [0x43; 16],
            subject_id: [0x44; 16],
            type_id: [0x41; 16],
            type_role_code: 10,
            program_point_id: Some([0x45; 16]),
            origin_code: 10,
            certainty_code: 30,
        };
        let fact_batch = encode_type_fact_details(&[fact_row]).expect("type fact detail encodes");
        validate_fact_batch(&fact_batch, 190, scope()).expect("type fact detail validates");

        assert_eq!(type_batch.schema().field(6).name(), "type_kind_code");
        assert_eq!(fact_batch.schema().field(8).name(), "type_role_code");
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

        let (sender, mut receiver) = bounded_provider_fact_channel(1);
        sender
            .send(ProviderFactMessage::Batch(ProviderFactBatch {
                table_code: 100,
                batch: encode_entities(&[]).unwrap(),
            }))
            .await
            .expect("first message fits");
        assert!(
            sender
                .try_send(ProviderFactMessage::Terminal(StreamTerminal::Completed))
                .is_err()
        );
        drop(sender);
        let cancellation = Cancellation::default();
        assert!(matches!(
            receive_provider_fact_stream(&mut receiver, &cancellation).await,
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
        let stream = ProviderFactStream {
            manifest: bad_manifest,
            batches: vec![ProviderFactBatch {
                table_code: 110,
                batch: encode_relations(&[relation([4; 16], [6; 16])]).unwrap(),
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
        let stream = ProviderFactStream {
            manifest: manifest(&provider, scope(), 100),
            batches: vec![ProviderFactBatch {
                table_code: 100,
                batch: encode_entities(&[entity([4; 16])]).unwrap(),
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
        let rejected = ProviderFactStream {
            manifest: invalid,
            batches: vec![ProviderFactBatch {
                table_code: 100,
                batch: encode_entities(&[entity([5; 16])]).unwrap(),
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
    fn wp59_behavioral_acceptance() {
        let (expected_scope, direct, precedence) = conflicting_relation_streams();
        let ipc = direct
            .iter()
            .map(|stream| {
                let batch = &stream.batches[0].batch;
                let bytes = ipc_bytes(batch);
                let contract = ProviderIpcContract::generated(110, batch.num_rows(), bytes.len())
                    .expect("generated IPC contract");
                decode_provider_ipc_chunks(stream.manifest.clone(), &contract, bytes.chunks(7))
                    .expect("arbitrary IPC chunks decode")
            })
            .collect::<Vec<_>>();
        let engine = CanonicalReconciliationEngine::default();
        let direct_output = engine
            .ingest(expected_scope, &direct, &precedence)
            .expect("direct batches ingest");
        let ipc_output = CanonicalReconciliationEngine::default()
            .ingest(expected_scope, &ipc, &precedence)
            .expect("IPC batches ingest");
        assert_same_output(&direct_output, &ipc_output);
        assert_eq!(direct_output.batches[&110].num_rows(), 1);
        assert_eq!(direct_output.batches[&130].num_rows(), 2);
        assert_eq!(direct_output.batches[&10].num_rows(), 1);
    }

    #[test]
    fn wp59_structural_acceptance() {
        let codes = generated_ingest_table_codes();
        assert_eq!(
            codes,
            [
                8, 9, 10, 100, 110, 120, 130, 140, 150, 160, 170, 180, 190, 200, 210, 220, 230,
                240, 250, 260, 270, 280, 290, 300,
            ]
        );
        for table_code in codes {
            let spec = table_spec(table_code).expect("generated ingest table");
            validate_fact_batch(
                &RecordBatch::new_empty(Arc::clone(&spec.arrow_schema)),
                table_code,
                scope(),
            )
            .unwrap_or_else(|error| panic!("table {table_code} lacks an ingest path: {error}"));
        }
        let (expected_scope, mut streams, precedence) = conflicting_relation_streams();
        streams[0].manifest.schema_fingerprints.insert(
            10,
            table_spec(10)
                .expect("diagnostic table")
                .schema_digest
                .clone(),
        );
        streams[0].manifest.declared_rows += 1;
        streams[0].batches.push(ProviderFactBatch {
            table_code: 10,
            batch: encode_diagnostics(&[DiagnosticRow {
                diagnostic_id: [0x0d; 16],
                workspace_id: expected_scope.workspace_id,
                analysis_context_id: Some(expected_scope.analysis_context_id),
                source_generation: expected_scope.source_generation,
                owner_id: Some(expected_scope.owner_id),
                diagnostic_code: 59,
                severity_code: crate::registries::Severity::Info as i16,
                message: "provider diagnostic".into(),
                cold_payload: Some(vec![5, 9]),
                created_at_micros: 59,
            }])
            .expect("provider diagnostic encodes"),
        });
        let output = CanonicalReconciliationEngine::default()
            .ingest(expected_scope, &streams, &precedence)
            .expect("golden conflict ingests");
        assert_eq!(output.conflicts.len(), 1);
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.batches[&130].num_rows(), 2);
        assert_eq!(output.batches[&10].num_rows(), 2);
        let diagnostic_messages = output.batches[&10]
            .batch()
            .column_by_name("message")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow_array::StringArray>()
            .unwrap();
        assert!(
            diagnostic_messages
                .iter()
                .flatten()
                .any(|message| message == "provider diagnostic")
        );
    }

    #[test]
    fn wp59_negative_zero_state() {
        let batch = encode_entities(&[entity([0x59; 16])]).expect("entity encodes");
        let bytes = ipc_bytes(&batch);
        let schema = batch.schema();
        let one_byte_chunks = bytes.chunks(1);
        let decoded = decode_validated_arrow_ipc_chunks(&schema, 1, bytes.len(), one_byte_chunks)
            .expect("one-byte chunk splits decode");
        assert_eq!(decoded.as_slice(), std::slice::from_ref(&batch));

        let mut prefixed = Vec::with_capacity(bytes.len() + 1);
        prefixed.push(0xff);
        prefixed.extend_from_slice(&bytes);
        let unaligned = Buffer::from(prefixed).slice(1);
        let decoded = decode_validated_arrow_ipc_buffers(&schema, 1, bytes.len(), [unaligned])
            .expect("valid unaligned IPC is repaired by decoder policy");
        assert_eq!(decoded, [batch]);

        assert!(
            decode_validated_arrow_ipc_chunks(&schema, 1, bytes.len() - 1, [bytes.as_slice()],)
                .is_err()
        );
        assert!(
            decode_validated_arrow_ipc_chunks(
                &schema,
                1,
                bytes.len() - 1,
                [&bytes[..bytes.len() - 1]],
            )
            .is_err()
        );
        let relation_schema = Arc::clone(&table_spec(110).unwrap().arrow_schema);
        assert!(matches!(
            decode_validated_arrow_ipc_chunks(&relation_schema, 1, bytes.len(), [bytes.as_slice()],),
            Err(FactIngestError::SourceSnapshotMismatch(_))
        ));
        assert!(
            decode_validated_arrow_ipc_chunks(
                &schema,
                MAX_ROWS_PER_STREAM + 1,
                bytes.len(),
                [bytes.as_slice()],
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn wp59_operational_acceptance() {
        let provider = FixtureProvider {
            provider_code: 10,
            provider_version: "1".into(),
            provider_run_id: "08080808080808080808080808080808".into(),
            observation_id: "09090909090909090909090909090909".into(),
            precedence: 0,
        };
        let batch = ProviderFactBatch {
            table_code: 100,
            batch: encode_entities(&[entity([0x59; 16])]).unwrap(),
        };
        let (sender, mut receiver) = bounded_provider_fact_channel(1);
        let task = tokio::spawn(async move {
            sender
                .send(ProviderFactMessage::Manifest(manifest(
                    &provider,
                    scope(),
                    100,
                )))
                .await
                .unwrap();
            sender
                .send(ProviderFactMessage::Batch(batch))
                .await
                .unwrap();
            sender
                .send(ProviderFactMessage::Terminal(StreamTerminal::Completed))
                .await
                .unwrap();
        });
        let cancellation = Cancellation::default();
        let stream = receive_provider_fact_stream(&mut receiver, &cancellation)
            .await
            .expect("bounded provider stream completes");
        task.await.unwrap();
        let engine = CanonicalReconciliationEngine::default();
        let output = engine
            .ingest(scope(), &[stream], &BTreeMap::from([(10, 0)]))
            .expect("operational stream ingests");
        assert_eq!(output.metrics.streams_received, 1);
        assert_eq!(output.metrics.rows_received, 1);
        assert_eq!(output.metrics.rows_encoded, 2);
        assert_eq!(engine.metrics(), output.metrics);
    }

    #[test]
    fn wp58_negative_zero_state() {
        let batch = encode_entities(&[entity([0x58; 16])]).unwrap();
        let entity_index = batch.schema().index_of("entity_id").unwrap();
        let mut fields = batch
            .schema()
            .fields()
            .iter()
            .map(|field| field.as_ref().clone())
            .collect::<Vec<_>>();
        let mut metadata = fields[entity_index].metadata().clone();
        metadata.remove(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY);
        metadata.remove(arrow_schema::extension::EXTENSION_TYPE_METADATA_KEY);
        fields[entity_index] = fields[entity_index].clone().with_metadata(metadata);
        let storage_only = RecordBatch::try_new(
            Arc::new(arrow_schema::Schema::new_with_metadata(
                fields,
                batch.schema().metadata().clone(),
            )),
            batch.columns().to_vec(),
        )
        .expect("an unknown Arrow consumer can still read FixedSizeBinary(16) storage");
        assert!(matches!(
            validate_fact_batch(&storage_only, 100, scope()),
            Err(FactIngestError::BatchInvalid {
                check: "schema",
                ..
            })
        ));
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
