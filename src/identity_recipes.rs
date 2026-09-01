//! Released application-owned identity and hash-purpose primitives.

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipeValue {
    Bytes(Vec<u8>),
    Utf8(String),
    Unsigned(Vec<u8>),
    Signed(Vec<u8>),
    Boolean(bool),
    Id([u8; 16]),
    Digest([u8; 32]),
    OrderedList(Vec<RecipeValue>),
    Set(Vec<RecipeValue>),
    Map(Vec<(RecipeValue, RecipeValue)>),
    TaggedUnion(u16, Box<RecipeValue>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipeNormalization {
    None,
    AsciiLower,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeField {
    pub tag: u16,
    pub normalization: RecipeNormalization,
    pub value: RecipeValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeRecord {
    pub domain_code: u16,
    pub fields: Vec<RecipeField>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecipeError;

fn expect(value: &RecipeValue, expected: u8, width: usize) -> Result<(), RecipeError> {
    let actual = match value {
        RecipeValue::Bytes(_) => 1,
        RecipeValue::Utf8(_) => 2,
        RecipeValue::Unsigned(_) => 4,
        RecipeValue::Signed(_) => 5,
        RecipeValue::Boolean(_) => 6,
        RecipeValue::Id(_) => 7,
        RecipeValue::Digest(_) => 8,
        RecipeValue::OrderedList(_) => 9,
        RecipeValue::Set(_) => 10,
        RecipeValue::Map(_) => 11,
        RecipeValue::TaggedUnion(_, _) => 12,
    };
    if actual != expected {
        return Err(RecipeError);
    }
    if width != 0 {
        match value {
            RecipeValue::Unsigned(bytes) if bytes.len() == width => {}
            _ => return Err(RecipeError),
        }
    }
    Ok(())
}

fn expect_members(values: &[RecipeValue], expected: u8) -> Result<(), RecipeError> {
    values
        .iter()
        .try_for_each(|value| expect(value, expected, 0))
}

fn expect_utf8_set(value: &RecipeValue) -> Result<(), RecipeError> {
    expect(value, 10, 0)?;
    let RecipeValue::Set(values) = value else {
        return Err(RecipeError);
    };
    expect_members(values, 2)
}

fn expect_id_set(value: &RecipeValue) -> Result<(), RecipeError> {
    expect(value, 10, 0)?;
    let RecipeValue::Set(values) = value else {
        return Err(RecipeError);
    };
    expect_members(values, 7)
}

fn expect_utf8_to_utf8_set_map(value: &RecipeValue) -> Result<(), RecipeError> {
    expect(value, 11, 0)?;
    let RecipeValue::Map(entries) = value else {
        return Err(RecipeError);
    };
    entries.iter().try_for_each(|(key, value)| {
        expect(key, 2, 0)?;
        expect_utf8_set(value)
    })
}

pub struct OwnerFields {
    pub workspace_id: RecipeValue,
    pub analysis_context_id: RecipeValue,
    pub owner_kind: RecipeValue,
    pub semantic_key: RecipeValue,
}

pub fn owner(fields: OwnerFields) -> Result<RecipeRecord, RecipeError> {
    expect(&fields.workspace_id, 7, 0)?;
    expect(&fields.analysis_context_id, 7, 0)?;
    expect(&fields.owner_kind, 2, 0)?;
    expect(&fields.semantic_key, 1, 0)?;
    Ok(RecipeRecord {
        domain_code: 7,
        fields: vec![
            RecipeField {
                tag: 1,
                normalization: RecipeNormalization::None,
                value: fields.workspace_id,
            },
            RecipeField {
                tag: 2,
                normalization: RecipeNormalization::None,
                value: fields.analysis_context_id,
            },
            RecipeField {
                tag: 3,
                normalization: RecipeNormalization::AsciiLower,
                value: fields.owner_kind,
            },
            RecipeField {
                tag: 4,
                normalization: RecipeNormalization::None,
                value: fields.semantic_key,
            },
        ],
    })
}

pub struct EntityFields {
    pub workspace_id: RecipeValue,
    pub analysis_context_id: RecipeValue,
    pub kind_code: RecipeValue,
    pub owner_id: RecipeValue,
    pub semantic_key: RecipeValue,
}

pub fn entity(fields: EntityFields) -> Result<RecipeRecord, RecipeError> {
    expect(&fields.workspace_id, 7, 0)?;
    expect(&fields.analysis_context_id, 7, 0)?;
    expect(&fields.kind_code, 4, 2)?;
    expect(&fields.owner_id, 7, 0)?;
    expect(&fields.semantic_key, 1, 0)?;
    Ok(RecipeRecord {
        domain_code: 8,
        fields: vec![
            RecipeField {
                tag: 1,
                normalization: RecipeNormalization::None,
                value: fields.workspace_id,
            },
            RecipeField {
                tag: 2,
                normalization: RecipeNormalization::None,
                value: fields.analysis_context_id,
            },
            RecipeField {
                tag: 3,
                normalization: RecipeNormalization::None,
                value: fields.kind_code,
            },
            RecipeField {
                tag: 4,
                normalization: RecipeNormalization::None,
                value: fields.owner_id,
            },
            RecipeField {
                tag: 5,
                normalization: RecipeNormalization::None,
                value: fields.semantic_key,
            },
        ],
    })
}

pub struct RelationFactFields {
    pub workspace_id: RecipeValue,
    pub analysis_context_id: RecipeValue,
    pub relation_kind_code: RecipeValue,
    pub subject_entity_id: RecipeValue,
    pub object_entity_id: RecipeValue,
    pub role: RecipeValue,
}

pub fn relation_fact(fields: RelationFactFields) -> Result<RecipeRecord, RecipeError> {
    expect(&fields.workspace_id, 7, 0)?;
    expect(&fields.analysis_context_id, 7, 0)?;
    expect(&fields.relation_kind_code, 4, 2)?;
    expect(&fields.subject_entity_id, 7, 0)?;
    expect(&fields.object_entity_id, 7, 0)?;
    expect(&fields.role, 12, 0)?;
    Ok(RecipeRecord {
        domain_code: 9,
        fields: vec![
            RecipeField {
                tag: 1,
                normalization: RecipeNormalization::None,
                value: fields.workspace_id,
            },
            RecipeField {
                tag: 2,
                normalization: RecipeNormalization::None,
                value: fields.analysis_context_id,
            },
            RecipeField {
                tag: 3,
                normalization: RecipeNormalization::None,
                value: fields.relation_kind_code,
            },
            RecipeField {
                tag: 4,
                normalization: RecipeNormalization::None,
                value: fields.subject_entity_id,
            },
            RecipeField {
                tag: 5,
                normalization: RecipeNormalization::None,
                value: fields.object_entity_id,
            },
            RecipeField {
                tag: 6,
                normalization: RecipeNormalization::None,
                value: fields.role,
            },
        ],
    })
}

pub struct PropertyFactFields {
    pub workspace_id: RecipeValue,
    pub analysis_context_id: RecipeValue,
    pub property_kind_code: RecipeValue,
    pub subject_entity_id: RecipeValue,
    pub canonical_value: RecipeValue,
}

pub fn property_fact(fields: PropertyFactFields) -> Result<RecipeRecord, RecipeError> {
    expect(&fields.workspace_id, 7, 0)?;
    expect(&fields.analysis_context_id, 7, 0)?;
    expect(&fields.property_kind_code, 4, 2)?;
    expect(&fields.subject_entity_id, 7, 0)?;
    expect(&fields.canonical_value, 12, 0)?;
    Ok(RecipeRecord {
        domain_code: 10,
        fields: vec![
            RecipeField {
                tag: 1,
                normalization: RecipeNormalization::None,
                value: fields.workspace_id,
            },
            RecipeField {
                tag: 2,
                normalization: RecipeNormalization::None,
                value: fields.analysis_context_id,
            },
            RecipeField {
                tag: 3,
                normalization: RecipeNormalization::None,
                value: fields.property_kind_code,
            },
            RecipeField {
                tag: 4,
                normalization: RecipeNormalization::None,
                value: fields.subject_entity_id,
            },
            RecipeField {
                tag: 5,
                normalization: RecipeNormalization::None,
                value: fields.canonical_value,
            },
        ],
    })
}

pub struct PathResultFields {
    pub workspace_id: RecipeValue,
    pub analysis_context_id: RecipeValue,
    pub fabric_epoch_id: RecipeValue,
    pub policy_identity: RecipeValue,
    pub ordered_entity_ids: RecipeValue,
    pub ordered_fact_ids: RecipeValue,
}

pub fn path_result(fields: PathResultFields) -> Result<RecipeRecord, RecipeError> {
    expect(&fields.workspace_id, 7, 0)?;
    expect(&fields.analysis_context_id, 7, 0)?;
    expect(&fields.fabric_epoch_id, 7, 0)?;
    expect(&fields.policy_identity, 2, 0)?;
    expect(&fields.ordered_entity_ids, 9, 0)?;
    expect(&fields.ordered_fact_ids, 9, 0)?;
    let RecipeValue::OrderedList(entity_ids) = &fields.ordered_entity_ids else {
        return Err(RecipeError);
    };
    let RecipeValue::OrderedList(fact_ids) = &fields.ordered_fact_ids else {
        return Err(RecipeError);
    };
    expect_members(entity_ids, 7)?;
    expect_members(fact_ids, 7)?;
    Ok(RecipeRecord {
        domain_code: 18,
        fields: vec![
            field(1, RecipeNormalization::None, fields.workspace_id),
            field(2, RecipeNormalization::None, fields.analysis_context_id),
            field(3, RecipeNormalization::None, fields.fabric_epoch_id),
            field(4, RecipeNormalization::None, fields.policy_identity),
            field(5, RecipeNormalization::None, fields.ordered_entity_ids),
            field(6, RecipeNormalization::None, fields.ordered_fact_ids),
        ],
    })
}

pub struct ObjectiveInputSetFields {
    pub workspace_id: RecipeValue,
    pub analysis_context_ids: RecipeValue,
    pub fact_ids: RecipeValue,
    pub producer_identities: RecipeValue,
    pub policy_identity: RecipeValue,
    pub coverage_state: RecipeValue,
}

pub fn objective_input_set(fields: ObjectiveInputSetFields) -> Result<RecipeRecord, RecipeError> {
    expect(&fields.workspace_id, 7, 0)?;
    expect_id_set(&fields.analysis_context_ids)?;
    expect_id_set(&fields.fact_ids)?;
    expect_utf8_set(&fields.producer_identities)?;
    expect(&fields.policy_identity, 2, 0)?;
    expect(&fields.coverage_state, 2, 0)?;
    let RecipeValue::Utf8(coverage_state) = &fields.coverage_state else {
        return Err(RecipeError);
    };
    if !matches!(
        coverage_state.to_ascii_lowercase().as_str(),
        "complete" | "partial" | "indeterminate" | "unavailable"
    ) {
        return Err(RecipeError);
    }
    Ok(RecipeRecord {
        domain_code: 19,
        fields: vec![
            field(1, RecipeNormalization::None, fields.workspace_id),
            field(2, RecipeNormalization::None, fields.analysis_context_ids),
            field(3, RecipeNormalization::None, fields.fact_ids),
            field(4, RecipeNormalization::None, fields.producer_identities),
            field(5, RecipeNormalization::None, fields.policy_identity),
            field(6, RecipeNormalization::AsciiLower, fields.coverage_state),
        ],
    })
}

fn expect_typed_scalar(value: &RecipeValue) -> Result<(), RecipeError> {
    let RecipeValue::TaggedUnion(variant, value) = value else {
        return Err(RecipeError);
    };
    let expected = u8::try_from(*variant).map_err(|_| RecipeError)?;
    if !matches!(expected, 2 | 4 | 5 | 6) {
        return Err(RecipeError);
    }
    expect(value, expected, 0)
}

pub struct ObjectiveGroupFields {
    pub workspace_id: RecipeValue,
    pub analysis_context_id: RecipeValue,
    pub input_set_id: RecipeValue,
    pub grouping_dimensions: RecipeValue,
    pub canonical_group_key: RecipeValue,
    pub aggregate_function: RecipeValue,
    pub measure: RecipeValue,
    pub producer_identity: RecipeValue,
}

pub fn objective_group(fields: ObjectiveGroupFields) -> Result<RecipeRecord, RecipeError> {
    expect(&fields.workspace_id, 7, 0)?;
    expect(&fields.analysis_context_id, 7, 0)?;
    expect(&fields.input_set_id, 7, 0)?;
    expect(&fields.grouping_dimensions, 9, 0)?;
    let RecipeValue::OrderedList(dimensions) = &fields.grouping_dimensions else {
        return Err(RecipeError);
    };
    expect_members(dimensions, 2)?;
    expect(&fields.canonical_group_key, 11, 0)?;
    let RecipeValue::Map(entries) = &fields.canonical_group_key else {
        return Err(RecipeError);
    };
    entries.iter().try_for_each(|(key, value)| {
        expect(key, 2, 0)?;
        expect_typed_scalar(value)
    })?;
    expect(&fields.aggregate_function, 2, 0)?;
    expect(&fields.measure, 2, 0)?;
    expect(&fields.producer_identity, 2, 0)?;
    Ok(RecipeRecord {
        domain_code: 20,
        fields: vec![
            field(1, RecipeNormalization::None, fields.workspace_id),
            field(2, RecipeNormalization::None, fields.analysis_context_id),
            field(3, RecipeNormalization::None, fields.input_set_id),
            field(4, RecipeNormalization::None, fields.grouping_dimensions),
            field(5, RecipeNormalization::None, fields.canonical_group_key),
            field(
                6,
                RecipeNormalization::AsciiLower,
                fields.aggregate_function,
            ),
            field(7, RecipeNormalization::None, fields.measure),
            field(8, RecipeNormalization::None, fields.producer_identity),
        ],
    })
}

pub struct QuerySourceContextFields {
    pub workspace_id: RecipeValue,
    pub analysis_context_id: RecipeValue,
    pub snapshot_id: RecipeValue,
    pub entity_id: RecipeValue,
    pub source_file_id: RecipeValue,
    pub source_generation: RecipeValue,
    pub source_content_digest: RecipeValue,
    pub delivered_start_byte: RecipeValue,
    pub delivered_end_byte: RecipeValue,
    pub delivered_content_digest: RecipeValue,
    pub disclosure_scope_id: RecipeValue,
    pub policy_identity: RecipeValue,
    pub context_kind: RecipeValue,
}

pub fn query_source_context(fields: QuerySourceContextFields) -> Result<RecipeRecord, RecipeError> {
    for value in [
        &fields.workspace_id,
        &fields.analysis_context_id,
        &fields.snapshot_id,
        &fields.entity_id,
        &fields.source_file_id,
        &fields.disclosure_scope_id,
    ] {
        expect(value, 7, 0)?;
    }
    expect(&fields.source_generation, 4, 8)?;
    expect(&fields.source_content_digest, 8, 0)?;
    expect(&fields.delivered_start_byte, 4, 8)?;
    expect(&fields.delivered_end_byte, 4, 8)?;
    expect(&fields.delivered_content_digest, 8, 0)?;
    expect(&fields.policy_identity, 2, 0)?;
    expect(&fields.context_kind, 2, 0)?;
    Ok(RecipeRecord {
        domain_code: 21,
        fields: vec![
            field(1, RecipeNormalization::None, fields.workspace_id),
            field(2, RecipeNormalization::None, fields.analysis_context_id),
            field(3, RecipeNormalization::None, fields.snapshot_id),
            field(4, RecipeNormalization::None, fields.entity_id),
            field(5, RecipeNormalization::None, fields.source_file_id),
            field(6, RecipeNormalization::None, fields.source_generation),
            field(7, RecipeNormalization::None, fields.source_content_digest),
            field(8, RecipeNormalization::None, fields.delivered_start_byte),
            field(9, RecipeNormalization::None, fields.delivered_end_byte),
            field(
                10,
                RecipeNormalization::None,
                fields.delivered_content_digest,
            ),
            field(11, RecipeNormalization::None, fields.disclosure_scope_id),
            field(12, RecipeNormalization::None, fields.policy_identity),
            field(13, RecipeNormalization::AsciiLower, fields.context_kind),
        ],
    })
}

pub struct AccessScopeFields {
    pub workspace_id: RecipeValue,
    pub policy_identity: RecipeValue,
    pub principal_id: RecipeValue,
    pub agent_id: RecipeValue,
    pub credential_digest: RecipeValue,
    pub role: RecipeValue,
    pub operation: RecipeValue,
    pub allowed_relations: RecipeValue,
    pub allowed_columns: RecipeValue,
    pub allowed_functions: RecipeValue,
    pub allowed_extensions: RecipeValue,
    pub allowed_variables: RecipeValue,
    pub allowed_object_stores: RecipeValue,
    pub allowed_metadata: RecipeValue,
    pub row_policies: RecipeValue,
    pub execution_posture: RecipeValue,
    pub source_access: RecipeValue,
    pub source_file_ids: RecipeValue,
    pub authorized_ranges: RecipeValue,
}

pub fn access_scope(fields: AccessScopeFields) -> Result<RecipeRecord, RecipeError> {
    for value in [&fields.workspace_id, &fields.principal_id, &fields.agent_id] {
        expect(value, 7, 0)?;
    }
    expect(&fields.policy_identity, 2, 0)?;
    expect(&fields.credential_digest, 8, 0)?;
    expect(&fields.role, 2, 0)?;
    expect(&fields.operation, 2, 0)?;
    for value in [
        &fields.allowed_relations,
        &fields.allowed_functions,
        &fields.allowed_extensions,
        &fields.allowed_variables,
        &fields.allowed_object_stores,
        &fields.allowed_metadata,
        &fields.row_policies,
        &fields.execution_posture,
    ] {
        expect_utf8_set(value)?;
    }
    expect_utf8_to_utf8_set_map(&fields.allowed_columns)?;
    expect(&fields.source_access, 6, 0)?;
    expect_id_set(&fields.source_file_ids)?;
    expect(&fields.authorized_ranges, 10, 0)?;
    let RecipeValue::Set(ranges) = &fields.authorized_ranges else {
        return Err(RecipeError);
    };
    for range in ranges {
        expect(range, 9, 0)?;
        let RecipeValue::OrderedList(parts) = range else {
            return Err(RecipeError);
        };
        if parts.len() != 3 {
            return Err(RecipeError);
        }
        expect(&parts[0], 7, 0)?;
        expect(&parts[1], 4, 8)?;
        expect(&parts[2], 4, 8)?;
    }
    Ok(RecipeRecord {
        domain_code: 22,
        fields: vec![
            field(1, RecipeNormalization::None, fields.workspace_id),
            field(2, RecipeNormalization::None, fields.policy_identity),
            field(3, RecipeNormalization::None, fields.principal_id),
            field(4, RecipeNormalization::None, fields.agent_id),
            field(5, RecipeNormalization::None, fields.credential_digest),
            field(6, RecipeNormalization::AsciiLower, fields.role),
            field(7, RecipeNormalization::AsciiLower, fields.operation),
            field(8, RecipeNormalization::None, fields.allowed_relations),
            field(9, RecipeNormalization::None, fields.allowed_columns),
            field(10, RecipeNormalization::None, fields.allowed_functions),
            field(11, RecipeNormalization::None, fields.allowed_extensions),
            field(12, RecipeNormalization::None, fields.allowed_variables),
            field(13, RecipeNormalization::None, fields.allowed_object_stores),
            field(14, RecipeNormalization::None, fields.allowed_metadata),
            field(15, RecipeNormalization::None, fields.row_policies),
            field(16, RecipeNormalization::None, fields.execution_posture),
            field(17, RecipeNormalization::None, fields.source_access),
            field(18, RecipeNormalization::None, fields.source_file_ids),
            field(19, RecipeNormalization::None, fields.authorized_ranges),
        ],
    })
}

pub struct ResultArtifactV2Fields {
    pub workspace_id: RecipeValue,
    pub owning_agent_id: RecipeValue,
    pub fabric_epoch_id: RecipeValue,
    pub snapshot_id: RecipeValue,
    pub canonical_response_checksum: RecipeValue,
    pub format: RecipeValue,
    pub format_version: RecipeValue,
}

pub fn result_artifact_v2(fields: ResultArtifactV2Fields) -> Result<RecipeRecord, RecipeError> {
    expect(&fields.workspace_id, 7, 0)?;
    expect(&fields.owning_agent_id, 2, 0)?;
    expect(&fields.fabric_epoch_id, 7, 0)?;
    expect(&fields.snapshot_id, 7, 0)?;
    expect(&fields.canonical_response_checksum, 8, 0)?;
    expect(&fields.format, 2, 0)?;
    expect(&fields.format_version, 2, 0)?;
    Ok(RecipeRecord {
        domain_code: 23,
        fields: vec![
            field(1, RecipeNormalization::None, fields.workspace_id),
            field(2, RecipeNormalization::None, fields.owning_agent_id),
            field(3, RecipeNormalization::None, fields.fabric_epoch_id),
            field(4, RecipeNormalization::None, fields.snapshot_id),
            field(
                5,
                RecipeNormalization::None,
                fields.canonical_response_checksum,
            ),
            field(6, RecipeNormalization::AsciiLower, fields.format),
            field(7, RecipeNormalization::None, fields.format_version),
        ],
    })
}

fn field(tag: u16, normalization: RecipeNormalization, value: RecipeValue) -> RecipeField {
    RecipeField {
        tag,
        normalization,
        value,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticFingerprintDomain {
    UnframedId16,
    CapabilityScope,
    FactEvidence,
    SourceObservation,
    GitTopology,
    GitIndex,
    GitStateVector,
    LocalQueryLimits,
    PythonLiteralTokenSpelling,
}

impl SemanticFingerprintDomain {
    #[must_use]
    pub const fn bytes(self) -> &'static [u8] {
        match self {
            Self::UnframedId16 => b"",
            Self::CapabilityScope => b"codefabric-capability-scope-v1\0",
            Self::FactEvidence => b"codefabric-fact-evidence-v1\0",
            Self::SourceObservation => b"codefabric-source-observation-v1\0",
            Self::GitTopology => b"codefabric.git.topology.v1\0",
            Self::GitIndex => b"codefabric.git.index.v1\0",
            Self::GitStateVector => b"codefabric.git.state-vector.v1\0",
            Self::LocalQueryLimits => b"codefabric.local-query-limits.v1",
            Self::PythonLiteralTokenSpelling => b"codefabric:python-literal-token-spelling:v1\0",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityDomain {
    ArrowBatch,
    QueryResultChecksumV1,
    QueryResultChecksumV2,
    InventoryFile,
    InventoryDirectory,
    ProviderTextImage,
    RuffFrontendProjection,
    TreeSitterRawSyntaxFacts,
}

impl IntegrityDomain {
    #[must_use]
    pub const fn bytes(self) -> &'static [u8] {
        match self {
            Self::ArrowBatch => b"codefabric-arrow-batch-v1\0",
            Self::QueryResultChecksumV1 => b"codefabric.query-result-checksum.v1\0",
            Self::QueryResultChecksumV2 => b"codefabric.query-result-checksum.v2\0",
            Self::InventoryFile => b"codefabric.inventory.file.v1\0",
            Self::InventoryDirectory => b"codefabric.inventory.directory.v1\0",
            Self::ProviderTextImage => b"codefabric:provider-text-image:v1\0",
            Self::RuffFrontendProjection => b"codefabric:ruff-python-frontend-projection:v1\0",
            Self::TreeSitterRawSyntaxFacts => b"codefabric:tree-sitter-raw-syntax-facts:v1\0",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheKeyDomain {
    GitCandidateCachePayload,
}

impl CacheKeyDomain {
    #[must_use]
    pub const fn bytes(self) -> &'static [u8] {
        match self {
            Self::GitCandidateCachePayload => b"codefabric.git.candidate-cache-payload.v1\0",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecurityMacDomain {
    ResultLease,
    QueryResumeToken,
    QueryCancelToken,
    LocalCapabilityToken,
}

impl SecurityMacDomain {
    #[must_use]
    pub const fn bytes(self) -> &'static [u8] {
        match self {
            Self::ResultLease => b"codefabric.result-lease.v1\0",
            Self::QueryResumeToken => b"codefabric.query-result-resume-token.v1\0",
            Self::QueryCancelToken => b"codefabric.query-cancel-token.v1\0",
            Self::LocalCapabilityToken => b"codefabric.local-capability-token.v1\0",
        }
    }
}
