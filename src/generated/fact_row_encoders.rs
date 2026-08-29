// @generated from codefabric.schema.contract-ir b3:3464705d29f1868f4c508094141b806b7e096c20dc9fb3d76646be5a5cf4bdd3; schema-contract-driver-v1; do not edit.

/// Generated encoder input for the canonical `owner` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `capability_status` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `diagnostic` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `entity` relation.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `relation` relation.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `property_fact` relation.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `fact_evidence` relation.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct FactEvidenceRow {
    pub scope: FactScope,
    pub evidence_id: [u8; 16],
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

/// Generated encoder input for the canonical `source_file` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `source_token` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `source_annotation` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `syntax_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `type_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `type_fact_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `scope_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ScopeDetailRow {
    pub scope: FactScope,
    pub scope_id: [u8; 16],
    pub parent_scope_id: Option<[u8; 16]>,
    pub scope_kind: String,
    pub name: Option<String>,
    pub start_byte: i64,
    pub end_byte: i64,
}

/// Generated encoder input for the canonical `binding_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `reference_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `module_import_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `callable_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `parameter_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `call_site_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `call_argument_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `cfg_graph` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `cfg_node_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `cfg_edge_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
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

/// Generated encoder input for the canonical `value_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ValueDetailRow {
    pub scope: FactScope,
    pub value_id: [u8; 16],
    pub value_kind_code: i16,
    pub type_id: Option<[u8; 16]>,
    pub producer_operation_id: Option<[u8; 16]>,
    pub constant_value_id: Option<[u8; 16]>,
    pub syntax_id: Option<[u8; 16]>,
    pub flags: i64,
    pub precision_profile_id: String,
    pub derivation_bundle_id: String,
}

/// Generated encoder input for the canonical `operation_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct OperationDetailRow {
    pub scope: FactScope,
    pub operation_id: [u8; 16],
    pub cfg_node_id: Option<[u8; 16]>,
    pub operation_kind_code: i32,
    pub result_value_id: Option<[u8; 16]>,
    pub type_id: Option<[u8; 16]>,
    pub syntax_id: Option<[u8; 16]>,
    pub raw_kind_code: Option<i32>,
    pub flags: i64,
    pub precision_profile_id: String,
    pub derivation_bundle_id: String,
}

/// Generated encoder input for the canonical `dataflow_event_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct DataflowEventDetailRow {
    pub scope: FactScope,
    pub event_id: [u8; 16],
    pub cfg_node_id: Option<[u8; 16]>,
    pub event_kind_code: i16,
    pub binding_id: Option<[u8; 16]>,
    pub value_id: Option<[u8; 16]>,
    pub location_id: Option<[u8; 16]>,
    pub syntax_id: Option<[u8; 16]>,
    pub ordinal: Option<i32>,
    pub flags: i64,
    pub precision_profile_id: String,
    pub derivation_bundle_id: String,
}

/// Generated encoder input for the canonical `memory_location_detail` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct MemoryLocationDetailRow {
    pub scope: FactScope,
    pub location_id: [u8; 16],
    pub location_kind_code: i16,
    pub base_entity_id: Option<[u8; 16]>,
    pub base_local_id: Option<[u8; 16]>,
    pub type_id: Option<[u8; 16]>,
    pub parent_location_id: Option<[u8; 16]>,
    pub projection_depth: i16,
    pub canonical_path_hash: [u8; 32],
    pub display_path: Option<String>,
    pub flags: i64,
    pub precision_profile_id: String,
    pub derivation_bundle_id: String,
}

/// Generated encoder input for the canonical `access_path_component` relation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct AccessPathComponentRow {
    pub scope: FactScope,
    pub component_id: [u8; 16],
    pub location_id: [u8; 16],
    pub ordinal: i16,
    pub projection_kind_code: i16,
    pub field_entity_id: Option<[u8; 16]>,
    pub index_value_id: Option<[u8; 16]>,
    pub variant_entity_id: Option<[u8; 16]>,
    pub constant_index: Option<i64>,
    pub subslice_from: Option<i64>,
    pub subslice_to: Option<i64>,
    pub flags: i64,
    pub precision_profile_id: String,
    pub derivation_bundle_id: String,
}

/// Encode `owner` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_owners(rows: &[OwnerRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        8,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            id16s(rows, |row| row.parent_owner_id.as_ref()),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            i16s(rows, |row| Some(row.owner_kind_code)),
            i16s(rows, |row| Some(row.language)),
            id16s(rows, |row| row.file_id.as_ref()),
            id16s(rows, |row| row.semantic_entity_id.as_ref()),
            i64s(rows, |row| row.start_byte),
            i64s(rows, |row| row.end_byte),
            hash32s(rows, |row| row.source_fingerprint.as_ref()),
            hash32s(rows, |row| row.semantic_fingerprint.as_ref()),
            i64s(rows, |row| Some(row.capability_mask)),
        ],
    )
}

/// Encode `capability_status` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_capability_statuses(
    rows: &[CapabilityStatusRow],
) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        9,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| row.snapshot_id.as_ref()),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            i16s(rows, |row| Some(row.capability_code)),
            i16s(rows, |row| Some(row.owner_capability_state_code)),
            i16s(rows, |row| Some(row.completeness_state_code)),
            id16s(rows, |row| row.provider_run_id.as_ref()),
            i16s(rows, |row| row.producer_code),
            i16s(rows, |row| row.reason_code),
            id16s(rows, |row| row.diagnostic_id.as_ref()),
            bools(rows, |row| Some(row.fallback_source_available)),
            hash32s(rows, |row| Some(&row.coverage_scope_fingerprint)),
        ],
    )
}

/// Encode `entity` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_entities(rows: &[EntityRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        100,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.entity_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            i16s(rows, |row| Some(row.language)),
            i16s(rows, |row| Some(row.entity_family_code)),
            i32s(rows, |row| Some(row.entity_kind_code)),
            i32s(rows, |row| row.raw_kind_code),
            id16s(rows, |row| row.file_id.as_ref()),
            i64s(rows, |row| row.start_byte),
            i64s(rows, |row| row.end_byte),
            utf8(rows, |row| row.name.as_deref()),
            utf8(rows, |row| row.qualified_name.as_deref()),
            id16s(rows, |row| row.parent_entity_id.as_ref()),
            id16s(rows, |row| row.type_id.as_ref()),
            i64s(rows, |row| Some(row.flags)),
            i64s(rows, |row| Some(row.fact_hash64)),
        ],
    )
}

/// Encode `relation` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_relations(rows: &[RelationRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        110,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.fact_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            i16s(rows, |row| Some(row.language)),
            i16s(rows, |row| Some(row.relation_family_code)),
            i32s(rows, |row| Some(row.relation_kind_code)),
            id16s(rows, |row| Some(&row.source_id)),
            id16s(rows, |row| Some(&row.target_id)),
            i16s(rows, |row| Some(i16::from(row.source_id[0]))),
            i16s(rows, |row| Some(i16::from(row.target_id[0]))),
            i32s(rows, |row| row.ordinal),
            i16s(rows, |row| row.role_code),
            i32s(rows, |row| row.distance),
            i16s(rows, |row| Some(row.directness_code)),
            id16s(rows, |row| row.file_id.as_ref()),
            i64s(rows, |row| row.start_byte),
            i64s(rows, |row| row.end_byte),
            i16s(rows, |row| Some(row.certainty_code)),
            i16s(rows, |row| Some(row.resolution_code)),
            i16s(rows, |row| Some(row.producer_code)),
            i16s(rows, |row| row.derivation_code),
            i64s(rows, |row| Some(row.flags)),
            i64s(rows, |row| Some(row.fact_hash64)),
        ],
    )
}

/// Encode `property_fact` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_properties(rows: &[PropertyFactRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        120,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.fact_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.subject_entity_id)),
            i32s(rows, |row| Some(row.property_kind_code)),
            id16s(rows, |row| row.program_point_entity_id.as_ref()),
            i16s(rows, |row| Some(row.value.code())),
            id16s(rows, |row| match &row.value {
                PropertyValue::Entity(value) => Some(value),
                _ => None,
            }),
            bools(rows, |row| match row.value {
                PropertyValue::Boolean(value) => Some(value),
                _ => None,
            }),
            i64s(rows, |row| match row.value {
                PropertyValue::Integer(value) => Some(value),
                _ => None,
            }),
            f64s(rows, |row| match row.value {
                PropertyValue::Float(value) => Some(value),
                _ => None,
            }),
            utf8(rows, |row| match &row.value {
                PropertyValue::Text(value) => Some(value.as_str()),
                _ => None,
            }),
            binary(rows, |row| match &row.value {
                PropertyValue::Bytes(value) => Some(value.as_slice()),
                _ => None,
            }),
            id16s(rows, |row| match &row.value {
                PropertyValue::Type(value) => Some(value),
                _ => None,
            }),
            i16s(rows, |row| Some(row.directness_code)),
            i16s(rows, |row| Some(row.certainty_code)),
            i16s(rows, |row| Some(row.resolution_code)),
            i16s(rows, |row| Some(row.producer_code)),
            i16s(rows, |row| row.derivation_code),
            id16s(rows, |row| row.file_id.as_ref()),
            i64s(rows, |row| row.start_byte),
            i64s(rows, |row| row.end_byte),
            i64s(rows, |row| Some(row.fact_hash64)),
        ],
    )
}

/// Encode `fact_evidence` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_evidence(rows: &[FactEvidenceRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        130,
        vec![
            id16s(rows, |row| Some(&row.evidence_id)),
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.fact_id)),
            i16s(rows, |row| Some(row.fact_form_code)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            i16s(rows, |row| Some(row.provider_code)),
            utf8(rows, |row| Some(row.provider_version.as_str())),
            id16s(rows, |row| Some(&row.provider_run_id)),
            id16s(rows, |row| Some(&row.observation_id)),
            i32s(rows, |row| row.raw_kind_code),
            id16s(rows, |row| row.file_id.as_ref()),
            i64s(rows, |row| row.start_byte),
            i64s(rows, |row| row.end_byte),
            i16s(rows, |row| Some(row.certainty_code)),
            i16s(rows, |row| Some(row.resolution_code)),
            i16s(rows, |row| Some(row.conflict_disposition_code)),
            binary(rows, |row| row.cold_payload.as_deref()),
        ],
    )
}

/// Encode `source_file` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_source_files(rows: &[SourceFileRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        140,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.file_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            binary(rows, |row| Some(row.path_bytes.as_slice())),
            utf8(rows, |row| Some(row.path_display.as_str())),
            i16s(rows, |row| Some(row.path_encoding_code)),
            binary(rows, |row| row.path_case_key.as_deref()),
            bools(rows, |row| Some(row.path_display_is_lossy)),
            i16s(rows, |row| Some(row.language)),
            hash32s(rows, |row| Some(&row.source_digest)),
            i64s(rows, |row| Some(row.byte_len)),
            i32s(rows, |row| Some(row.line_count)),
            utf8(rows, |row| row.encoding_name.as_deref()),
            i16s(rows, |row| Some(row.newline_kind_code)),
            binary(rows, |row| Some(row.source_bytes.as_slice())),
            utf8(rows, |row| row.decoded_text.as_deref()),
            i64_lists(140, "line_start_offsets", rows, |row| {
                row.line_start_offsets.as_slice()
            }),
            id16s(rows, |row| row.module_entity_id.as_ref()),
            bools(rows, |row| Some(row.is_stub)),
            i64s(rows, |row| Some(row.flags)),
        ],
    )
}

/// Encode `source_token` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_source_tokens(rows: &[SourceTokenRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        150,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.token_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.file_id)),
            i32s(rows, |row| Some(row.ordinal)),
            i32s(rows, |row| Some(row.token_kind_code)),
            i64s(rows, |row| Some(row.start_byte)),
            i64s(rows, |row| Some(row.end_byte)),
            utf8(rows, |row| row.normalized_value.as_deref()),
            i64s(rows, |row| Some(row.flags)),
        ],
    )
}

/// Encode `source_annotation` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_source_annotations(
    rows: &[SourceAnnotationRow],
) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        160,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.annotation_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.file_id)),
            i32s(rows, |row| Some(row.annotation_kind_code)),
            i64s(rows, |row| Some(row.start_byte)),
            i64s(rows, |row| Some(row.end_byte)),
            id16s(rows, |row| row.target_entity_id.as_ref()),
            utf8(rows, |row| row.text.as_deref()),
            i32s(rows, |row| row.diagnostic_code),
            i64s(rows, |row| Some(row.flags)),
        ],
    )
}

/// Encode `syntax_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_syntax_details(rows: &[SyntaxDetailRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        170,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.entity_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            i32s(rows, |row| Some(row.raw_kind_code)),
            i16s(rows, |row| Some(row.occurrence_family_code)),
            i16s(rows, |row| Some(row.reconciliation_step_code)),
            i16s(rows, |row| Some(row.raw_kind_disposition_code)),
            i32s(rows, |row| Some(row.normalized_kind_code)),
            id16s(rows, |row| row.parent_syntax_id.as_ref()),
            i16s(rows, |row| row.field_role_code),
            i32s(rows, |row| row.ordinal),
            i32s(rows, |row| row.source_ordinal),
            i32s(rows, |row| row.evaluation_ordinal),
            i32s(rows, |row| row.line),
            i32s(rows, |row| row.column),
            i32s(rows, |row| row.depth),
            utf8(rows, |row| row.provider_name.as_deref()),
            bools(rows, |row| Some(row.named)),
            bools(rows, |row| Some(row.extra)),
            bools(rows, |row| Some(row.error)),
            bools(rows, |row| Some(row.missing)),
            bools(rows, |row| Some(row.explicitly_parenthesized)),
            i64s(rows, |row| Some(row.provider_node_flags)),
        ],
    )
}

/// Encode `type_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_type_details(rows: &[TypeDetailRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        180,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.type_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            i32s(rows, |row| Some(row.type_kind_code)),
            utf8(rows, |row| Some(row.canonical_key.as_str())),
            utf8(rows, |row| row.display_name.as_deref()),
            i16s(rows, |row| row.primitive_code),
            id16s(rows, |row| row.nominal_entity_id.as_ref()),
            id16s(rows, |row| row.callable_entity_id.as_ref()),
            hash32s(rows, |row| row.raw_shape_hash.as_ref()),
            i16s(rows, |row| row.nullable_semantics_code),
            i64s(rows, |row| Some(row.flags)),
        ],
    )
}

/// Encode `type_fact_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_type_fact_details(
    rows: &[TypeFactDetailRow],
) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        190,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.relation_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.subject_id)),
            id16s(rows, |row| Some(&row.type_id)),
            i16s(rows, |row| Some(row.type_role_code)),
            id16s(rows, |row| row.program_point_id.as_ref()),
            i16s(rows, |row| Some(row.origin_code)),
            i16s(rows, |row| Some(row.certainty_code)),
        ],
    )
}

/// Encode `scope_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_scope_details(rows: &[ScopeDetailRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        200,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.scope_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| row.parent_scope_id.as_ref()),
            utf8(rows, |row| Some(row.scope_kind.as_str())),
            utf8(rows, |row| row.name.as_deref()),
            i64s(rows, |row| Some(row.start_byte)),
            i64s(rows, |row| Some(row.end_byte)),
        ],
    )
}

/// Encode `binding_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_binding_details(rows: &[BindingDetailRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        210,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.binding_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.scope_id)),
            utf8(rows, |row| Some(row.name.as_str())),
            utf8(rows, |row| Some(row.binding_kind.as_str())),
            utf8(rows, |row| Some(row.target_form.as_str())),
            i64s(rows, |row| Some(row.start_byte)),
            i64s(rows, |row| Some(row.end_byte)),
        ],
    )
}

/// Encode `reference_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_reference_details(
    rows: &[ReferenceDetailRow],
) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        220,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.reference_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.scope_id)),
            id16s(rows, |row| Some(&row.target_id)),
            utf8(rows, |row| Some(row.name.as_str())),
            utf8(rows, |row| Some(row.reference_class.as_str())),
            utf8(rows, |row| Some(row.resolution.as_str())),
            i64s(rows, |row| Some(row.start_byte)),
            i64s(rows, |row| Some(row.end_byte)),
            utf8(rows, |row| row.unknown_reason_code.as_deref()),
        ],
    )
}

/// Encode `module_import_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_module_import_details(
    rows: &[ModuleImportDetailRow],
) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        230,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.import_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.source_module_id)),
            id16s(rows, |row| row.target_module_id.as_ref()),
            id16s(rows, |row| row.imported_entity_id.as_ref()),
            id16s(rows, |row| row.local_binding_id.as_ref()),
            i16s(rows, |row| Some(row.import_kind_code)),
            i16s(rows, |row| row.relative_level),
            utf8(rows, |row| Some(row.source_name.as_str())),
            utf8(rows, |row| row.alias_name.as_deref()),
            bools(rows, |row| Some(row.star_import)),
            i16s(rows, |row| row.unknown_reason_code),
        ],
    )
}

/// Encode `callable_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_callable_details(rows: &[CallableDetailRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        240,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.callable_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| row.signature_id.as_ref()),
            id16s(rows, |row| row.return_type_id.as_ref()),
            i32s(rows, |row| Some(row.parameter_count)),
            i32s(rows, |row| Some(row.generic_parameter_count)),
            i16s(rows, |row| row.calling_convention_code),
            utf8(rows, |row| row.abi_name.as_deref()),
            i64s(rows, |row| Some(row.callable_flags)),
        ],
    )
}

/// Encode `parameter_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_parameter_details(
    rows: &[ParameterDetailRow],
) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        250,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.parameter_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.callable_id)),
            i32s(rows, |row| Some(row.ordinal)),
            utf8(rows, |row| row.name.as_deref()),
            i16s(rows, |row| Some(row.parameter_kind_code)),
            id16s(rows, |row| row.type_id.as_ref()),
            id16s(rows, |row| row.default_syntax_id.as_ref()),
            i64s(rows, |row| Some(row.flags)),
        ],
    )
}

/// Encode `call_site_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_call_site_details(
    rows: &[CallSiteDetailRow],
) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        260,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.call_site_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.caller_id)),
            id16s(rows, |row| row.syntax_id.as_ref()),
            id16s(rows, |row| row.callee_syntax_id.as_ref()),
            id16s(rows, |row| row.receiver_value_id.as_ref()),
            id16s(rows, |row| row.result_value_id.as_ref()),
            i16s(rows, |row| Some(row.dispatch_kind_code)),
            id16s(rows, |row| row.declared_target_id.as_ref()),
            i32s(rows, |row| Some(row.resolved_target_count)),
            i64s(rows, |row| Some(row.call_flags)),
        ],
    )
}

/// Encode `call_argument_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_call_argument_details(
    rows: &[CallArgumentDetailRow],
) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        270,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.argument_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.call_site_id)),
            i32s(rows, |row| Some(row.ordinal)),
            utf8(rows, |row| row.keyword_name.as_deref()),
            id16s(rows, |row| row.argument_syntax_id.as_ref()),
            id16s(rows, |row| row.argument_value_id.as_ref()),
            id16s(rows, |row| row.parameter_id.as_ref()),
            i16s(rows, |row| Some(row.binding_status_code)),
            i16s(rows, |row| row.spread_kind_code),
        ],
    )
}

/// Encode `cfg_graph` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_cfg_graphs(rows: &[CfgGraphRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        280,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.cfg_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| row.callable_id.as_ref()),
            i16s(rows, |row| Some(row.cfg_kind_code)),
            id16s(rows, |row| Some(&row.entry_node_id)),
            id16s(rows, |row| Some(&row.exit_node_id)),
            id16s(rows, |row| row.exceptional_exit_node_id.as_ref()),
            i32s(rows, |row| Some(row.node_count)),
            i32s(rows, |row| Some(row.edge_count)),
            i64s(rows, |row| Some(row.flags)),
        ],
    )
}

/// Encode `cfg_node_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_cfg_node_details(rows: &[CfgNodeDetailRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        290,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.cfg_node_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.cfg_id)),
            i16s(rows, |row| Some(row.node_kind_code)),
            id16s(rows, |row| row.syntax_id.as_ref()),
            id16s(rows, |row| row.mir_statement_id.as_ref()),
            i32s(rows, |row| row.ordinal),
            i64s(rows, |row| Some(row.flags)),
        ],
    )
}

/// Encode `cfg_edge_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_cfg_edge_details(rows: &[CfgEdgeDetailRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        300,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.relation_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.cfg_id)),
            id16s(rows, |row| row.condition_id.as_ref()),
            utf8(rows, |row| row.case_value_text.as_deref()),
            i64s(rows, |row| row.case_value_hash),
            id16s(rows, |row| row.exception_type_id.as_ref()),
            i64s(rows, |row| Some(row.edge_flags)),
        ],
    )
}

/// Encode `value_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_value_details(rows: &[ValueDetailRow]) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        310,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.value_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            i16s(rows, |row| Some(row.value_kind_code)),
            id16s(rows, |row| row.type_id.as_ref()),
            id16s(rows, |row| row.producer_operation_id.as_ref()),
            id16s(rows, |row| row.constant_value_id.as_ref()),
            id16s(rows, |row| row.syntax_id.as_ref()),
            i64s(rows, |row| Some(row.flags)),
            utf8(rows, |row| Some(row.precision_profile_id.as_str())),
            utf8(rows, |row| Some(row.derivation_bundle_id.as_str())),
        ],
    )
}

/// Encode `operation_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_operation_details(
    rows: &[OperationDetailRow],
) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        320,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.operation_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| row.cfg_node_id.as_ref()),
            i32s(rows, |row| Some(row.operation_kind_code)),
            id16s(rows, |row| row.result_value_id.as_ref()),
            id16s(rows, |row| row.type_id.as_ref()),
            id16s(rows, |row| row.syntax_id.as_ref()),
            i32s(rows, |row| row.raw_kind_code),
            i64s(rows, |row| Some(row.flags)),
            utf8(rows, |row| Some(row.precision_profile_id.as_str())),
            utf8(rows, |row| Some(row.derivation_bundle_id.as_str())),
        ],
    )
}

/// Encode `dataflow_event_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_dataflow_event_details(
    rows: &[DataflowEventDetailRow],
) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        330,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.event_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| row.cfg_node_id.as_ref()),
            i16s(rows, |row| Some(row.event_kind_code)),
            id16s(rows, |row| row.binding_id.as_ref()),
            id16s(rows, |row| row.value_id.as_ref()),
            id16s(rows, |row| row.location_id.as_ref()),
            id16s(rows, |row| row.syntax_id.as_ref()),
            i32s(rows, |row| row.ordinal),
            i64s(rows, |row| Some(row.flags)),
            utf8(rows, |row| Some(row.precision_profile_id.as_str())),
            utf8(rows, |row| Some(row.derivation_bundle_id.as_str())),
        ],
    )
}

/// Encode `memory_location_detail` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_memory_location_details(
    rows: &[MemoryLocationDetailRow],
) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        340,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.location_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            i16s(rows, |row| Some(row.location_kind_code)),
            id16s(rows, |row| row.base_entity_id.as_ref()),
            id16s(rows, |row| row.base_local_id.as_ref()),
            id16s(rows, |row| row.type_id.as_ref()),
            id16s(rows, |row| row.parent_location_id.as_ref()),
            i16s(rows, |row| Some(row.projection_depth)),
            hash32s(rows, |row| Some(&row.canonical_path_hash)),
            utf8(rows, |row| row.display_path.as_deref()),
            i64s(rows, |row| Some(row.flags)),
            utf8(rows, |row| Some(row.precision_profile_id.as_str())),
            utf8(rows, |row| Some(row.derivation_bundle_id.as_str())),
        ],
    )
}

/// Encode `access_path_component` rows in the exact generated schema order.
///
/// # Errors
///
/// Returns an Arrow error if a typed accessor and its generated physical field diverge.
pub fn encode_access_path_components(
    rows: &[AccessPathComponentRow],
) -> Result<RecordBatch, FactIngestError> {
    generated_fact_batch(
        350,
        vec![
            id16s(rows, |row| Some(&row.scope.workspace_id)),
            id16s(rows, |row| Some(&row.scope.analysis_context_id)),
            i64s(rows, |row| Some(row.scope.source_generation)),
            id16s(rows, |row| Some(&row.component_id)),
            id16s(rows, |row| Some(&row.scope.owner_id)),
            i16s(rows, |row| Some(i16::from(row.scope.owner_id[0]))),
            id16s(rows, |row| Some(&row.location_id)),
            i16s(rows, |row| Some(row.ordinal)),
            i16s(rows, |row| Some(row.projection_kind_code)),
            id16s(rows, |row| row.field_entity_id.as_ref()),
            id16s(rows, |row| row.index_value_id.as_ref()),
            id16s(rows, |row| row.variant_entity_id.as_ref()),
            i64s(rows, |row| row.constant_index),
            i64s(rows, |row| row.subslice_from),
            i64s(rows, |row| row.subslice_to),
            i64s(rows, |row| Some(row.flags)),
            utf8(rows, |row| Some(row.precision_profile_id.as_str())),
            utf8(rows, |row| Some(row.derivation_bundle_id.as_str())),
        ],
    )
}
