//! Runtime projection of the single model-compiler `CompiledOntology` value.

/// Digest-pinned owner authority for one compiled record family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledAuthority {
    pub authority_id: &'static str,
    pub authority_version: &'static str,
    pub canonical_digest: &'static str,
    pub canonical_source_path: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledEnumValue {
    pub domain: &'static str,
    pub code: i32,
    pub name: &'static str,
    pub authority: CompiledAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledEntityKind {
    pub code: i32,
    pub name: &'static str,
    pub family_code: i16,
    pub language_applicability: &'static str,
    pub query_visible: bool,
    pub authority: CompiledAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledRelationKind {
    pub code: i32,
    pub name: &'static str,
    pub family_code: i16,
    pub family_name: &'static str,
    pub cardinality: &'static str,
    pub symmetric: bool,
    pub transitive: bool,
    pub self_edge_policy: &'static str,
    pub owner_selection_rule: &'static str,
    pub query_visible: bool,
    pub authority: CompiledAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledPropertyKind {
    pub code: i32,
    pub name: &'static str,
    pub value_kind_code: i16,
    pub cardinality: &'static str,
    pub storage_mapping: &'static str,
    pub authority: CompiledAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledFactKind {
    pub code: i16,
    pub name: &'static str,
    pub fact_form: &'static str,
    pub authority: CompiledAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledProviderRawKind {
    pub provider_code: i16,
    pub raw_catalog_id: &'static str,
    pub raw_namespace: &'static str,
    pub raw_kind_code: i32,
    pub raw_name: &'static str,
    pub normalized_kind_code: Option<i32>,
    pub authority: CompiledAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledOntologyEdge {
    pub subject_term_id: &'static str,
    pub predicate_term_id: &'static str,
    pub object_term_id: &'static str,
    pub ordinal: i32,
    pub authority: CompiledAuthority,
}

/// Closed mechanically expressible ontology validation operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompiledRuleOperationKind {
    ForeignKeyAntiJoin,
    GovernedCodeAntiJoin,
    PrimaryKeyUniquenessAggregate,
    IdDomainConformance,
    OntologyMembershipAntiJoin,
    RelationFamilyConformanceJoin,
    RelationCardinalityAggregate,
    RelationOwnerConformanceJoin,
    RelationSelfEdgeJoin,
    PropertyValueOneOf,
    SourceSpanAllOrNone,
}

/// One source-fenced typed rule contract compiled with the ontology vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledRuleContract {
    pub rule_id: &'static str,
    pub operation_kind: CompiledRuleOperationKind,
    pub input_contract: &'static str,
    pub output_contract: &'static str,
    pub determinism_class: &'static str,
    pub diagnostic_code: &'static str,
}

/// Complete generated runtime vocabulary projection. Table/result/identity contracts remain
/// accessible through the same schema-registry output family and are joined by stable IDs.
pub struct RuntimeCompiledOntology {
    pub enum_values: &'static [CompiledEnumValue],
    pub entity_kinds: &'static [CompiledEntityKind],
    pub relation_kinds: &'static [CompiledRelationKind],
    pub property_kinds: &'static [CompiledPropertyKind],
    pub fact_kinds: &'static [CompiledFactKind],
    pub provider_raw_kinds: &'static [CompiledProviderRawKind],
    pub phrase_authority: CompiledAuthority,
    pub query_form_authority: CompiledAuthority,
    pub edges: &'static [CompiledOntologyEdge],
    pub rules: &'static [CompiledRuleContract],
}

include!("generated/compiled_ontology.rs");

#[must_use]
pub const fn compiled_ontology() -> &'static RuntimeCompiledOntology {
    &COMPILED_ONTOLOGY
}
