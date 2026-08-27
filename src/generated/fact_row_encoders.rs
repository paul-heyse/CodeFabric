// @generated from codefabric.schema.contract-ir b3:63bd5ccd9580028acac3cf4269ca72efa927e59db44be762b05c5b4e1d449069; schema-contract-driver-v1; do not edit.

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
