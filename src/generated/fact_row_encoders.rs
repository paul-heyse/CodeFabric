// @generated from codefabric.schema.contract-ir b3:3fcec223a46e71a76c8736405dc911ced2a9a989a5743c9f835844c76821b196; schema-contract-driver-v1; do not edit.

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
            binary(rows, |row| {
                row.source_fingerprint.as_ref().map(<[u8; 32]>::as_slice)
            }),
            binary(rows, |row| {
                row.semantic_fingerprint.as_ref().map(<[u8; 32]>::as_slice)
            }),
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
            binary(rows, |row| Some(row.coverage_scope_fingerprint.as_slice())),
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
            binary(rows, |row| Some(row.source_digest.as_slice())),
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
            binary(rows, |row| {
                row.raw_shape_hash.as_ref().map(<[u8; 32]>::as_slice)
            }),
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
            binary(rows, |row| Some(row.canonical_path_hash.as_slice())),
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
