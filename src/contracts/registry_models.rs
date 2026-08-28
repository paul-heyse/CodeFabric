//! Closed models and semantic validation for Wave-1 registry authorities.

// These field names intentionally preserve the normative wire keys on closed Serde
// models; renaming them internally would create a second vocabulary to maintain.
#![allow(clippy::struct_field_names)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::catalog::{ArtifactKind, ArtifactStatus, ContractOwner, DigestProjection};
use super::models::{ArtifactHeader, OwnerAcceptance};

/// Registry envelope used once a family has an owner-accepted initial allocation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedRegistry<T> {
    pub artifact_id: String,
    pub artifact_kind: ArtifactKind,
    pub version: String,
    pub compatible_suite_major: u16,
    pub status: ArtifactStatus,
    pub canonical_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest_projection: Option<DigestProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator_revision: Option<String>,
    pub records: Vec<T>,
    pub owner_acceptance: OwnerAcceptance,
}

/// One operational field explicitly excluded from semantic snapshot comparison.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonIgnoreRecord {
    pub field_name: String,
    pub category: String,
    pub rationale: String,
    pub semantic: bool,
}

/// Closed deterministic actions admitted by the fault harness.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultAction {
    ReturnError,
    MutateFixture,
    BlockOnBarrier,
    TerminateProcess,
    DropMessage,
    DuplicateMessage,
    ReorderWithNext,
    CloseChannel,
}

/// One named deterministic fault point.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FaultPointRecord {
    pub code: String,
    pub owner: ContractOwner,
    pub allowed_actions: BTreeSet<FaultAction>,
    pub production_exposable: bool,
    pub expected_invariants: BTreeSet<String>,
    pub scenarios: BTreeSet<String>,
}

impl<T> AcceptedRegistry<T> {
    #[must_use]
    pub fn header(&self) -> ArtifactHeader {
        ArtifactHeader {
            artifact_id: self.artifact_id.clone(),
            artifact_kind: self.artifact_kind,
            version: self.version.clone(),
            compatible_suite_major: self.compatible_suite_major,
            status: self.status,
            canonical_digest: self.canonical_digest.clone(),
            digest_projection: self.digest_projection,
            generator_revision: self.generator_revision.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnumValue {
    pub code: u16,
    pub name: String,
    pub slug: String,
    pub meaning: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnumDomain {
    pub domain: String,
    pub width_bits: u8,
    pub values: Vec<EnumValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FlagValue {
    pub bit: u8,
    pub name: String,
    pub slug: String,
    pub meaning: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FlagDomain {
    pub domain: String,
    pub width_bits: u8,
    pub values: Vec<FlagValue>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VersionLifecycle {
    pub introduced: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EntityKind {
    pub kind_code: u16,
    pub kind_slug: String,
    pub canonical_name: String,
    pub family_code: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_kind_code: Option<u16>,
    pub language_profile: String,
    pub abstract_kind: bool,
    pub representation: String,
    #[serde(default)]
    pub allowed_owner_kinds: Vec<String>,
    #[serde(default)]
    pub required_property_codes: Vec<u16>,
    #[serde(default)]
    pub optional_property_codes: Vec<u16>,
    pub default_capability_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_extension_table: Option<String>,
    #[serde(default)]
    pub query_phrase_ids: Vec<String>,
    pub query_visibility: String,
    pub public_display_template_id: String,
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RelationKind {
    pub relation_code: u16,
    pub relation_slug: String,
    pub canonical_name: String,
    pub family_code: u8,
    pub family: String,
    pub language_profile: String,
    pub abstract_relation: bool,
    pub representation: String,
    pub allowed_subject_families: Vec<String>,
    pub allowed_object_families: Vec<String>,
    pub role_requirement: String,
    pub ordinal_requirement: String,
    pub self_edge_policy: String,
    pub cardinality: String,
    pub symmetric: bool,
    pub transitive: bool,
    pub certainty_applicability: String,
    pub resolution_applicability: String,
    pub directness_applicability: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unknown_remainder_relation_code: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inverse_relation_code: Option<u16>,
    pub projection_memberships: Vec<String>,
    pub owner_selection_rule: String,
    pub storage_table: String,
    pub default_capability_code: String,
    #[serde(default)]
    pub query_phrase_ids: Vec<String>,
    pub query_visibility: String,
    pub public_display_template_id: String,
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyValueType {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scalar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_element: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyStorage {
    pub canonical_table: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denormalized_entity_column: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_table_column: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyKind {
    pub property_code: u16,
    pub property_slug: String,
    pub canonical_name: String,
    pub subject_kind_constraints: Vec<String>,
    pub value_type: PropertyValueType,
    pub cardinality: String,
    pub required_profiles: Vec<String>,
    pub owner_rule: String,
    pub context_rule: String,
    pub source_span_allowed: bool,
    pub certainty_required: bool,
    pub resolution_applicability: String,
    pub directness_applicability: String,
    pub null_semantics: String,
    pub unknown_value_policy: String,
    pub canonicalization_rule: String,
    pub storage: PropertyStorage,
    #[serde(default)]
    pub query_phrase_ids: Vec<String>,
    pub statement_template_id: String,
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FactKind {
    pub fact_code: u16,
    pub fact_slug: String,
    pub canonical_name: String,
    pub shape: String,
    pub statement_template_id: String,
    pub response_roles: Vec<String>,
    pub evidence_semantics: String,
    pub completeness_semantics: String,
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnknownKind {
    pub category: String,
    pub code: u16,
    pub name: String,
    pub slug: String,
    pub identity_scope: Vec<String>,
    pub meaning: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Projection {
    pub projection_id: String,
    pub version: String,
    pub node_kind_families: Vec<String>,
    pub edge_kind_codes: Vec<String>,
    pub context_policy: String,
    pub representation_policy: String,
    pub certainty_filter: Vec<String>,
    pub resolution_filter: Vec<String>,
    pub directness_filter: Vec<String>,
    pub include_unknown_edges: bool,
    pub include_external_endpoints: bool,
    pub normal_exception_unwind_policy: String,
    pub edge_directionality: String,
    pub parallel_edge_policy: String,
    pub weight_semantics: String,
    pub owner_boundary_applicability: String,
    pub materialization_policy: String,
    #[serde(default)]
    pub query_phrase_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryProfile {
    pub summary_profile_id: String,
    pub version: String,
    pub input_projection: String,
    pub precision_profile: String,
    pub included_direct_fact_families: Vec<String>,
    pub call_projection: String,
    pub aggregation_operators: Vec<String>,
    pub unknown_external_propagation: String,
    pub fixpoint_algorithm: String,
    pub widening: String,
    pub support_witness_policy: String,
    pub completeness_rule: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub capability_code: String,
    pub capability_slug: String,
    pub fact_families: Vec<String>,
    pub allowed_scope_kinds: Vec<String>,
    pub required_producer: String,
    #[serde(default)]
    pub prerequisite_capabilities: Vec<String>,
    pub applicability_predicate: String,
    pub completeness_proof_rule: String,
    pub supported_precision_profiles: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub provider_id: String,
    pub provider_slug: String,
    pub placement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_service: Option<String>,
    pub toolchain_or_bundle_digest_fields: Vec<String>,
    pub capability_codes: Vec<String>,
    pub event_mapping_version: String,
    pub resource_profile_id: String,
    #[serde(default)]
    pub raw_catalog_ids: BTreeSet<String>,
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle,
}

/// Closed execution budget selected by every provider job before admission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResourceProfile {
    pub profile_id: String,
    pub provider_ids: BTreeSet<String>,
    pub max_parallel_jobs_global: u16,
    pub max_parallel_jobs_per_workspace: u16,
    pub max_parallel_jobs_per_context: u16,
    pub max_input_bytes: u64,
    pub max_work_units: u64,
    pub max_wall_millis: u64,
    pub max_visited_nodes: u64,
    pub max_traversal_depth: u16,
    pub max_output_records: u64,
    pub max_output_bytes: u64,
    pub max_diagnostics: u16,
    pub max_parser_workers: u16,
    pub max_retained_tree_revisions: u16,
    pub max_cpu_weight: u32,
    pub max_memory_mib: u32,
    pub cancellation_check_interval: u32,
    pub cancellation_ack_millis: u16,
    pub hard_stop_policy: ProviderHardStopPolicy,
    pub retry_policy: ProviderRetryPolicy,
    pub max_retries: u16,
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle,
}

/// Closed action after cooperative provider cancellation exceeds its acknowledgement bound.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderHardStopPolicy {
    CooperativeDiscard,
    ProcessGroupTerminate,
    CancellableTaskAbort,
}

/// Closed bounded-retry behavior for one provider class.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderRetryPolicy {
    NoRetry,
    TransientOnly,
    IdempotentOnly,
}

/// Fallback applied to a provider-native kind not present in an authored mapping.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RawKindDisposition {
    Ignore,
    Unsupported,
}

/// Authored normalization policy expanded against one generated raw-kind inventory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderNormalization {
    pub mapping_id: String,
    pub raw_catalog_id: String,
    pub provider_id: String,
    pub provider_version: String,
    pub language: String,
    pub canonical_kind_names: BTreeMap<String, String>,
    /// Provider-native child/field role to generated language-neutral role name.
    pub field_role_names: BTreeMap<String, String>,
    #[serde(default)]
    pub canonical_kind_prefixes: BTreeMap<String, String>,
    #[serde(default)]
    pub ignored_raw_keys: BTreeSet<String>,
    #[serde(default)]
    pub default_canonical_kind_name: Option<String>,
    pub default_disposition: RawKindDisposition,
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublicError {
    pub code: u16,
    pub name: String,
    pub owning_layer: String,
    pub severity: String,
    pub retryability: String,
    pub scope: String,
    pub public_message_template: String,
    pub allowed_public_detail_fields: Vec<String>,
    pub diagnostic_linkage: String,
    pub grpc_status: String,
    pub mcp_mapping: String,
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DerivationDefinition {
    pub derivation_id: String,
    #[serde(default)]
    pub owner_packet: String,
    pub owner_kind: String,
    pub input_fact_families: Vec<String>,
    pub output_fact_families: Vec<String>,
    pub projection_id: String,
    pub precision_profile: String,
    #[serde(default)]
    pub algorithm_id: String,
    pub algorithm_version: String,
    #[serde(default)]
    pub derivation_bundle_id: String,
    pub replacement_scope: String,
    pub dependency_rule: String,
    #[serde(default)]
    pub context_fingerprint_inputs: Vec<String>,
    #[serde(default)]
    pub source_fingerprint_inputs: Vec<String>,
    #[serde(default)]
    pub invalidation_closure: Vec<String>,
    #[serde(default)]
    pub resource_profile_id: String,
    #[serde(default)]
    pub pass_contract_id: String,
    #[serde(default)]
    pub implementation_symbol: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StateTransition {
    pub from: String,
    pub event: String,
    pub guard: String,
    pub to: String,
    pub actions: Vec<String>,
    pub idempotency_key: String,
    pub error_on_illegal: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StateMachine {
    pub machine_id: String,
    pub version: String,
    pub states: Vec<EnumValue>,
    pub initial_state: String,
    pub terminal_states: Vec<String>,
    /// Append-only historical codes accepted for decode/recovery but forbidden as transition
    /// targets or active scheduler states.
    #[serde(default)]
    pub decode_only_states: Vec<String>,
    pub transitions: Vec<StateTransition>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PhraseAstNodeKind {
    #[serde(rename = "entity-phrase")]
    Entity,
    #[serde(rename = "fact-phrase")]
    Fact,
    #[serde(rename = "relationship-phrase")]
    Relationship,
    #[serde(rename = "condition-phrase")]
    Condition,
    #[serde(rename = "projection-phrase")]
    Projection,
    #[serde(rename = "source-boundary-phrase")]
    SourceBoundary,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhraseSlotType {
    EntityKind,
    FactKind,
    RelationKind,
    PropertyKind,
    ProjectionId,
    EffectKind,
    ResourceKind,
    UnknownKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhraseReferenceFamily {
    EntityKind,
    FactKind,
    RelationKind,
    PropertyKind,
    Projection,
    EffectKind,
    ResourceKind,
    UnknownKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhraseContractReference {
    pub family: PhraseReferenceFamily,
    pub code: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RequestForm {
    FindEntities,
    RetrieveFacts,
    FollowRelationships,
    FindPaths,
    MatchPattern,
    CombineSets,
    SummarizeFacts,
    FetchSourceContext,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhraseRole {
    Subject,
    Object,
    Owner,
    Endpoint,
    Source,
    Target,
    Result,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanNodeKind {
    FindEntities,
    RetrieveFacts,
    FollowRelationships,
    FindPaths,
    MatchPattern,
    CombineSets,
    SummarizeFacts,
    FetchSourceContext,
}

impl PlanNodeKind {
    pub const fn request_form(self) -> RequestForm {
        match self {
            Self::FindEntities => RequestForm::FindEntities,
            Self::RetrieveFacts => RequestForm::RetrieveFacts,
            Self::FollowRelationships => RequestForm::FollowRelationships,
            Self::FindPaths => RequestForm::FindPaths,
            Self::MatchPattern => RequestForm::MatchPattern,
            Self::CombineSets => RequestForm::CombineSets,
            Self::SummarizeFacts => RequestForm::SummarizeFacts,
            Self::FetchSourceContext => RequestForm::FetchSourceContext,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FindEntities => "find-entities",
            Self::RetrieveFacts => "retrieve-facts",
            Self::FollowRelationships => "follow-relationships",
            Self::FindPaths => "find-paths",
            Self::MatchPattern => "match-pattern",
            Self::CombineSets => "combine-sets",
            Self::SummarizeFacts => "summarize-facts",
            Self::FetchSourceContext => "fetch-source-context",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SlotValueType {
    QuotedIdentifier,
    EntitySelector,
    ResultReference,
    Literal,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanField {
    Selector,
    SubjectSelector,
    ObjectSelector,
    EntityKindIds,
    FactKindIds,
    RelationKindIds,
    PropertyKindIds,
    ProjectionId,
    EffectKind,
    ResourceKind,
    LanguageProfile,
    ConditionKind,
    SourceBoundary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSlotBinding {
    pub slot: String,
    pub value_type: SlotValueType,
    pub target_field: PlanField,
    pub required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "value_kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PhraseConstantValue {
    Text { text: String },
    TextList { values: Vec<String> },
    Boolean { value: bool },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConstantFieldBinding {
    pub target_field: PlanField,
    pub value: PhraseConstantValue,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhraseOutputRole {
    EntitySet,
    FactSet,
    PathSet,
    BindingTable,
    GroupSet,
    SourceContextSet,
    ScalarSummary,
    CoverageProof,
}

impl PhraseOutputRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EntitySet => "entity-set",
            Self::FactSet => "fact-set",
            Self::PathSet => "path-set",
            Self::BindingTable => "binding-table",
            Self::GroupSet => "group-set",
            Self::SourceContextSet => "source-context-set",
            Self::ScalarSummary => "scalar-summary",
            Self::CoverageProof => "coverage-proof",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanSpecMapping {
    pub node_kind: PlanNodeKind,
    pub typed_slot_bindings: Vec<TypedSlotBinding>,
    pub constant_fields: Vec<ConstantFieldBinding>,
    pub output_role: PhraseOutputRole,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhraseRecord {
    pub phrase_id: String,
    pub owner_section: u8,
    pub canonical_text: String,
    #[serde(default)]
    pub accepted_aliases: Vec<String>,
    pub ast_node_kind: PhraseAstNodeKind,
    pub slot_type: PhraseSlotType,
    pub contract_reference: PhraseContractReference,
    pub allowed_request_forms: Vec<RequestForm>,
    #[serde(default)]
    pub allowed_subject_roles: Vec<PhraseRole>,
    #[serde(default)]
    pub allowed_object_roles: Vec<PhraseRole>,
    #[serde(default)]
    pub required_modifiers: Vec<String>,
    #[serde(default)]
    pub incompatible_modifiers: Vec<String>,
    pub planspec_mapping: PlanSpecMapping,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub negative_fixtures: Vec<String>,
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle,
}

/// Closed operators admitted by phrase-driven relational predicate compilation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhrasePredicateOperator {
    Equals,
    InSet,
}

/// Null/unknown behavior is part of the compiled predicate contract, never an implicit
/// DataFusion default selected at a call site.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhraseNullPolicy {
    UnknownIsFalse,
    RejectUnknown,
}

/// Logical compiler paths that must consume the same phrase operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhraseOperationIngress {
    Relational,
    Graph,
}

/// One strongly typed phrase-to-predicate binding owned by the phrase registry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhraseOperationBinding {
    pub phrase_id: String,
    pub canonical_text: String,
    pub column_role: String,
    pub operator: PhrasePredicateOperator,
    pub operand_domain: String,
    pub operand_names: Vec<String>,
    pub null_policy: PhraseNullPolicy,
    pub output_role: String,
    pub ingresses: BTreeSet<PhraseOperationIngress>,
    pub diagnostic_code: String,
}

/// Ontology-code family selected by one governed query phrase.
// The repeated suffix is intentional: these names serialize directly to the governed
// `entity_kind`, `relation_kind`, and `property_kind` registry vocabulary.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhraseProjectionTarget {
    EntityKind,
    RelationKind,
    PropertyKind,
}

/// Authored phrase-to-ontology operands consumed by both relational and graph compilers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhraseProjectionBinding {
    pub phrase_id: String,
    pub target: PhraseProjectionTarget,
    pub operand_names: Vec<String>,
}

/// Phrase authority envelope extended with compiled semantic operations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhraseRegistry {
    pub artifact_id: String,
    pub artifact_kind: ArtifactKind,
    pub version: String,
    pub compatible_suite_major: u16,
    pub status: ArtifactStatus,
    pub canonical_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest_projection: Option<DigestProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator_revision: Option<String>,
    pub records: Vec<PhraseRecord>,
    pub semantic_operation_bindings: Vec<PhraseOperationBinding>,
    pub semantic_projection_bindings: Vec<PhraseProjectionBinding>,
    pub owner_acceptance: OwnerAcceptance,
}

pub trait RegistryRecord {
    fn code(&self) -> Option<u16>;
    fn name(&self) -> &str;
    fn slug(&self) -> &str;
}

macro_rules! registry_record {
    ($type:ty, $code:ident, $name:ident, $slug:ident) => {
        impl RegistryRecord for $type {
            fn code(&self) -> Option<u16> {
                Some(self.$code)
            }
            fn name(&self) -> &str {
                &self.$name
            }
            fn slug(&self) -> &str {
                &self.$slug
            }
        }
    };
}

registry_record!(EntityKind, kind_code, canonical_name, kind_slug);
registry_record!(RelationKind, relation_code, canonical_name, relation_slug);
registry_record!(PropertyKind, property_code, canonical_name, property_slug);
registry_record!(FactKind, fact_code, canonical_name, fact_slug);
registry_record!(UnknownKind, code, name, slug);

fn upper_snake(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn kebab(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Validate the closed initial comparison-ignore allocation.
pub fn validate_comparison_ignores(records: &[ComparisonIgnoreRecord]) -> Result<(), String> {
    let mut fields = BTreeSet::new();
    for record in records {
        if record.field_name.is_empty()
            || record.category.is_empty()
            || record.rationale.is_empty()
            || record.semantic
            || !fields.insert(&record.field_name)
        {
            return Err(
                "comparison ignores require unique fields, rationale, and semantic=false".into(),
            );
        }
    }
    if records.is_empty() {
        return Err("the released comparison-ignore registry cannot be empty".into());
    }
    Ok(())
}

/// Validate append-only deterministic fault-point records.
pub fn validate_fault_points(records: &[FaultPointRecord]) -> Result<(), String> {
    let mut codes = BTreeSet::new();
    for record in records {
        if !upper_snake(&record.code)
            || !codes.insert(&record.code)
            || record.allowed_actions.is_empty()
            || record.production_exposable
            || record.expected_invariants.is_empty()
            || record.scenarios.is_empty()
        {
            return Err("fault points require a unique code, actions, invariants, scenarios, and production_exposable=false".into());
        }
    }
    Ok(())
}

pub fn validate_records<T: RegistryRecord>(records: &[T]) -> Result<(), String> {
    let mut codes = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut slugs = BTreeSet::new();
    for (index, record) in records.iter().enumerate() {
        if let Some(code) = record.code() {
            let expected =
                u16::try_from((index + 1) * 10).map_err(|_| "registry allocation overflow")?;
            if code != expected || !codes.insert(code) {
                return Err("registry codes must follow append-only increments of ten".into());
            }
        }
        if !upper_snake(record.name()) || !names.insert(record.name()) {
            return Err("registry names must be unique UPPER_SNAKE values".into());
        }
        if !kebab(record.slug()) || !slugs.insert(record.slug()) {
            return Err("registry slugs must be unique lowercase-kebab values".into());
        }
    }
    Ok(())
}

fn validate_enum_values(domain: &str, values: &[EnumValue]) -> Result<(), String> {
    let mut codes = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut slugs = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let expected = u16::try_from((index + 1) * 10).map_err(|_| "enum allocation overflow")?;
        if value.code != expected
            || !codes.insert(value.code)
            || !upper_snake(&value.name)
            || !names.insert(&value.name)
            || !kebab(&value.slug)
            || !slugs.insert(&value.slug)
        {
            return Err(format!(
                "enum domain {domain} violates the append-only code/name/slug allocation"
            ));
        }
    }
    Ok(())
}

// The owner-fixed declaration table is intentionally adjacent to its validator so
// additions cannot bypass the exact-name/order check.
#[allow(clippy::items_after_statements, clippy::too_many_lines)]
pub fn validate_enum_domains(records: &[EnumDomain]) -> Result<(), String> {
    let mut domains = BTreeSet::new();
    for domain in records {
        if !upper_snake(&domain.domain) || !domains.insert(&domain.domain) || domain.width_bits == 0
        {
            return Err("enum domains must be unique UPPER_SNAKE values with a fixed width".into());
        }
        validate_enum_values(&domain.domain, &domain.values)?;
    }
    const EXPECTED: &[(&str, &[&str])] = &[
        (
            "EVIDENCE_CERTAINTY",
            &[
                "SOURCE_EXACT",
                "COMPILER_EXACT",
                "STATIC_SEMANTIC",
                "SOUND_MAY",
                "MODELLED",
                "HEURISTIC",
                "UNRESOLVED",
            ],
        ),
        (
            "RESOLUTION_CLASS",
            &[
                "EXACT",
                "STATICALLY_RESOLVED",
                "SOUND_POSSIBLE",
                "POSSIBLE",
                "MODELLED",
                "HEURISTIC",
                "UNRESOLVED",
                "UNAVAILABLE",
                "NOT_APPLICABLE",
            ],
        ),
        (
            "DIRECTNESS",
            &["DIRECT", "TRANSITIVE", "SUMMARY", "NOT_APPLICABLE"],
        ),
        (
            "COMPLETENESS",
            &[
                "COMPLETE",
                "PARTIAL",
                "INDETERMINATE",
                "UNAVAILABLE",
                "NOT_APPLICABLE",
            ],
        ),
        (
            "OWNER_CAPABILITY_STATE",
            &[
                "CURRENT",
                "PENDING",
                "INVALIDATED",
                "PARTIAL",
                "UNAVAILABLE_PARSE",
                "UNAVAILABLE_COMPILE",
                "UNAVAILABLE_PROVIDER",
                "UNAVAILABLE_DERIVATION",
                "EXCLUDED",
                "UNSUPPORTED",
                "REMOVED",
                "NOT_APPLICABLE",
            ],
        ),
        (
            "PROVIDER_RUN_STATE",
            &[
                "QUEUED",
                "RUNNING",
                "SUCCEEDED",
                "PARTIAL",
                "FAILED",
                "TIMED_OUT",
                "CANCELLED",
                "SUPERSEDED",
                "CRASHED",
                "PROTOCOL_ERROR",
                "STALE_RESULT",
                "STALE_GIT_BASELINE",
            ],
        ),
        (
            "QUERY_EXECUTION_STATE",
            &[
                "ACCEPTED",
                "RUNNING",
                "COMPLETE",
                "FAILED",
                "CANCELLED",
                "DEADLINE_EXCEEDED",
                "NOT_EXECUTED_DEPENDENCY",
            ],
        ),
        (
            "QUERY_AVAILABILITY_STATE",
            &["AVAILABLE", "PARTIAL", "UNAVAILABLE", "NOT_APPLICABLE"],
        ),
        (
            "COMPLETENESS_STATE",
            &[
                "COMPLETE",
                "PARTIAL",
                "INDETERMINATE",
                "UNAVAILABLE",
                "NOT_APPLICABLE",
            ],
        ),
        (
            "FRESHNESS_STATE",
            &["CURRENT", "POTENTIALLY_STALE", "UNAVAILABLE"],
        ),
        (
            "LIMIT_STATE",
            &[
                "NOT_APPLIED",
                "EXPLICIT_LIMIT_REACHED",
                "HARD_LIMIT_REJECTED",
            ],
        ),
        (
            "DEPENDENCY_STATE",
            &["READY", "FAILED_DEPENDENCY", "NOT_APPLICABLE"],
        ),
        (
            "DURABLE_PUBLICATION_STATE",
            &[
                "STAGING",
                "VALIDATING",
                "VALIDATED",
                "COMMITTING",
                "COMPLETE",
                "FAILED",
                "ABANDONED",
            ],
        ),
        (
            "SERVING_ACTIVATION_STATE",
            &[
                "BUILDING",
                "VALIDATING",
                "READY",
                "ACTIVE",
                "RETIRED",
                "FAILED",
            ],
        ),
        (
            "SNAPSHOT_LEASE_KIND",
            &["QUERY", "RESULT_ARTIFACT", "RESOURCE_READ", "MAINTENANCE"],
        ),
        (
            "SNAPSHOT_LEASE_STATE",
            &["ACTIVE", "RELEASING", "RELEASED", "EXPIRED", "ORPHANED"],
        ),
        (
            "SOURCE_TRUST_STATE",
            &[
                "UNVERIFIED",
                "VERIFYING",
                "CURRENT",
                "POTENTIALLY_STALE",
                "UNAVAILABLE",
            ],
        ),
        (
            "EVENT_STREAM_HEALTH",
            &["HEALTHY", "RESCAN_REQUIRED", "DEGRADED", "UNAVAILABLE"],
        ),
        (
            "GIT_ACCELERATION_STATUS",
            &[
                "NOT_A_GIT_WORKTREE",
                "GIT_UNAVAILABLE",
                "GIT_READY",
                "GIT_METADATA_DIRTY",
                "GIT_SCANNING",
                "GIT_OPERATION_IN_PROGRESS",
                "GIT_BULK_RECONCILING",
                "GIT_DEGRADED",
            ],
        ),
        (
            "EFFECT_KIND",
            &[
                "READ_MEMORY",
                "WRITE_MEMORY",
                "ALLOCATE_MEMORY",
                "DEALLOCATE_MEMORY",
                "READ_FILE",
                "WRITE_FILE",
                "READ_NETWORK",
                "WRITE_NETWORK",
                "READ_DATABASE",
                "WRITE_DATABASE",
                "BEGIN_TRANSACTION",
                "COMMIT_TRANSACTION",
                "ROLLBACK_TRANSACTION",
                "READ_STANDARD_INPUT",
                "WRITE_STANDARD_OUTPUT",
                "LOG_OR_TELEMETRY",
                "READ_ENVIRONMENT",
                "WRITE_ENVIRONMENT",
                "SPAWN_PROCESS",
                "SPAWN_THREAD_OR_TASK",
                "BLOCK_THREAD",
                "SLEEP_OR_WAIT",
                "LOAD_DYNAMIC_LIBRARY",
                "ACQUIRE_LOCK",
                "RELEASE_LOCK",
                "SEND_CHANNEL",
                "RECEIVE_CHANNEL",
                "READ_TIME",
                "READ_RANDOMNESS",
                "READ_GLOBAL_STATE",
                "WRITE_GLOBAL_STATE",
                "RAISE_EXCEPTION",
                "PANIC_OR_ABORT",
                "UNSAFE_OPERATION",
                "FFI_CALL",
                "DYNAMIC_CODE_EXECUTION",
                "UNKNOWN_EXTERNAL_EFFECT",
            ],
        ),
        (
            "RESOURCE_KIND",
            &[
                "FILE_HANDLE",
                "SOCKET_OR_CONNECTION",
                "DATABASE_CONNECTION_OR_TRANSACTION",
                "LOCK_GUARD",
                "CHANNEL_ENDPOINT",
                "PROCESS_HANDLE",
                "THREAD_OR_TASK_HANDLE",
                "MEMORY_ALLOCATION",
                "USER_DEFINED_MODELLED_RESOURCE",
                "UNKNOWN_RESOURCE",
            ],
        ),
        (
            "PROVIDER_CODE",
            &[
                "TREE_SITTER",
                "RUFF_PYTHON",
                "PYREFLY_PYTHON",
                "RUSTC_MIR",
                "CODEFABRIC_DERIVATION",
                "SOURCE_SUBSTRATE",
            ],
        ),
        (
            "TOKEN_KIND",
            &[
                "IDENTIFIER",
                "KEYWORD",
                "OPERATOR",
                "PUNCTUATION",
                "LITERAL",
                "STRING",
                "NUMBER",
                "UNKNOWN",
            ],
        ),
        (
            "ANNOTATION_KIND",
            &[
                "COMMENT",
                "DOCUMENTATION",
                "PRAGMA_OR_DIRECTIVE",
                "PARSE_ERROR",
                "MISSING_SYNTAX",
            ],
        ),
        (
            "SYNTAX_KIND",
            &[
                "SYNTAX_NODE",
                "STATEMENT",
                "EXPRESSION",
                "PATTERN",
                "DECLARATION_SYNTAX",
                "TYPE_SYNTAX",
                "PARAMETER_SYNTAX",
                "ARGUMENT_SYNTAX",
                "BLOCK",
                "LITERAL",
                "OPERATION",
                "ATTRIBUTE_ACCESS",
                "MEMBER_ACCESS",
                "SUBSCRIPT_ACCESS",
                "INDEX_ACCESS",
                "CALL_EXPRESSION",
                "ASSIGNMENT",
                "BRANCH",
                "LOOP",
                "RETURN",
                "YIELD",
                "AWAIT",
                "RAISE_OR_PANIC_SYNTAX",
                "IMPORT_OR_USE_SYNTAX",
            ],
        ),
        (
            "SYNTAX_FIELD_ROLE",
            &[
                "NAME",
                "PARAMETERS",
                "DECORATOR",
                "RETURNS",
                "BODY",
                "CONDITION",
                "TARGET",
                "VALUE",
                "RECEIVER",
                "CALLEE",
                "ARGUMENT",
                "KEYWORD_ARGUMENT",
                "ITERABLE",
                "GUARD",
                "PATTERN",
                "HANDLER",
                "FINALLY_BODY",
            ],
        ),
        ("LANGUAGE", &["COMMON", "PYTHON", "RUST", "UNKNOWN"]),
        (
            "PATH_ENCODING",
            &["UNIX_BYTES", "MACOS_BYTES", "WINDOWS_WTF8"],
        ),
        ("NEWLINE_KIND", &["NONE", "LF", "CRLF", "CR", "MIXED"]),
        ("WORKSPACE_KIND", &["NON_GIT_ROOT", "GIT_WORKTREE"]),
        ("ANALYSIS_CONTEXT_KIND", &["SOURCE", "PYTHON", "RUST"]),
        (
            "VALUE_KIND",
            &[
                "ENTITY", "BOOLEAN", "INTEGER", "FLOAT", "TEXT", "BYTES", "TYPE",
            ],
        ),
        ("SEVERITY", &["INFO", "WARNING", "ERROR", "FATAL"]),
    ];
    if records.len() != EXPECTED.len() {
        return Err("enum registry must contain every §62, effect, and resource domain".into());
    }
    for (domain, names) in EXPECTED {
        let Some(record) = records.iter().find(|record| record.domain == *domain) else {
            return Err(format!("enum registry is missing {domain}"));
        };
        let actual: Vec<_> = record
            .values
            .iter()
            .map(|value| value.name.as_str())
            .collect();
        if actual != *names {
            return Err(format!(
                "enum domain {domain} differs from the owner-fixed declaration order"
            ));
        }
    }
    Ok(())
}

pub fn validate_flag_domains(records: &[FlagDomain]) -> Result<(), String> {
    let mut domains = BTreeSet::new();
    for domain in records {
        if domain.width_bits != 64
            || !upper_snake(&domain.domain)
            || !domains.insert(&domain.domain)
        {
            return Err("flag domains must be unique 64-bit UPPER_SNAKE values".into());
        }
        let mut bits = BTreeSet::new();
        for value in &domain.values {
            if value.bit >= 56
                || value.bit == 63
                || !bits.insert(value.bit)
                || !upper_snake(&value.name)
                || !kebab(&value.slug)
            {
                return Err(
                    "flag bits must be unique allocations below the reserved 56..=63 range".into(),
                );
            }
        }
    }
    Ok(())
}

pub fn validate_entity_records(records: &[EntityKind]) -> Result<(), String> {
    validate_records(records)?;
    let by_code: BTreeMap<_, _> = records
        .iter()
        .map(|record| (record.kind_code, record))
        .collect();
    for record in records {
        if !(1..=15).contains(&record.family_code)
            || !matches!(
                record.language_profile.as_str(),
                "core" | "python" | "rust" | "generated"
            )
            || !matches!(
                record.representation.as_str(),
                "source" | "syntax" | "semantic" | "compiler" | "derived" | "unknown"
            )
            || (!record.abstract_kind
                && (record.default_capability_code.is_empty()
                    || record.storage_extension_table.is_none()))
            || (record.query_phrase_ids.is_empty() && record.query_visibility != "ID_ONLY")
        {
            return Err(format!(
                "entity {} violates AC-G-70 mappings",
                record.canonical_name
            ));
        }
        let mut seen = BTreeSet::new();
        let mut cursor = record.parent_kind_code;
        while let Some(code) = cursor {
            if !seen.insert(code) || code == record.kind_code {
                return Err("entity parent graph contains a cycle".into());
            }
            cursor = by_code
                .get(&code)
                .and_then(|parent| parent.parent_kind_code);
        }
        if record
            .parent_kind_code
            .is_some_and(|code| !by_code.contains_key(&code))
        {
            return Err("entity parent references an unknown kind".into());
        }
    }
    Ok(())
}

pub fn validate_property_records(records: &[PropertyKind]) -> Result<(), String> {
    validate_records(records)?;
    for record in records {
        if record.null_semantics != "prohibited"
            || record.storage.canonical_table != "property_fact"
            || !matches!(
                record.cardinality.as_str(),
                "EXACTLY_ONE" | "ZERO_OR_ONE" | "ZERO_OR_MORE" | "ONE_OR_MORE"
            )
            || !matches!(
                record.context_rule.as_str(),
                "source" | "semantic" | "inherited-from-subject"
            )
        {
            return Err(format!(
                "property {} violates AC-G-71",
                record.canonical_name
            ));
        }
    }
    Ok(())
}

pub fn validate_relation_records(records: &[RelationKind]) -> Result<(), String> {
    validate_records(records)?;
    for record in records {
        if !(1..=15).contains(&record.family_code)
            || !matches!(
                record.language_profile.as_str(),
                "core" | "python" | "rust" | "generated"
            )
            || !matches!(
                record.representation.as_str(),
                "source" | "syntax" | "semantic" | "compiler" | "derived" | "unknown"
            )
            || (!record.abstract_relation && record.default_capability_code.is_empty())
            || (record.query_phrase_ids.is_empty() && record.query_visibility != "ID_ONLY")
            || record.allowed_subject_families.is_empty()
            || record.allowed_object_families.is_empty()
            || record.projection_memberships.is_empty()
            || record.owner_selection_rule.is_empty()
            || record.storage_table != "relation"
        {
            return Err(format!(
                "relation {} violates AC-G-70 mappings",
                record.canonical_name
            ));
        }
    }
    Ok(())
}

pub fn validate_fact_records(records: &[FactKind]) -> Result<(), String> {
    validate_records(records)?;
    let shapes: BTreeSet<_> = records.iter().map(|record| record.shape.as_str()).collect();
    if shapes != BTreeSet::from(["entity-existence", "property", "relation"]) {
        return Err(
            "fact registry must unify entity-existence, property, and relation facts".into(),
        );
    }
    Ok(())
}

fn exact_names<'a, I>(actual: I, expected: &[&str], family: &str) -> Result<(), String>
where
    I: IntoIterator<Item = &'a str>,
{
    let actual: BTreeSet<_> = actual.into_iter().collect();
    let expected: BTreeSet<_> = expected.iter().copied().collect();
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{family} allocation is incomplete or contains an unowned value"
        ))
    }
}

pub fn validate_unknown_records(records: &[UnknownKind]) -> Result<(), String> {
    let mut codes = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut slugs = BTreeSet::new();
    for record in records {
        if record.code == 0
            || !codes.insert((&record.category, record.code))
            || !upper_snake(&record.name)
            || !names.insert(&record.name)
            || !kebab(&record.slug)
            || !slugs.insert(&record.slug)
        {
            return Err(
                "unknown records require category-local codes and global names/slugs".into(),
            );
        }
    }
    for category in ["unknown-kind", "reason-class", "negative-fact-family"] {
        let selected: Vec<_> = records
            .iter()
            .filter(|record| record.category == category)
            .collect();
        for (index, record) in selected.iter().enumerate() {
            if record.code
                != u16::try_from((index + 1) * 10).map_err(|_| "unknown allocation overflow")?
            {
                return Err(format!(
                    "{category} codes must follow declaration order in increments of ten"
                ));
            }
        }
    }
    exact_names(
        records
            .iter()
            .filter(|record| record.category == "unknown-kind")
            .map(|record| record.name.as_str()),
        &[
            "UNKNOWN_SYMBOL",
            "UNKNOWN_TYPE",
            "UNKNOWN_MODULE",
            "UNKNOWN_MEMBER",
            "UNKNOWN_CALL_TARGET",
            "UNKNOWN_EXTERNAL_IMPLEMENTATION",
            "UNKNOWN_VALUE",
            "UNKNOWN_MEMORY_LOCATION",
            "UNKNOWN_EFFECT",
            "UNKNOWN_RESOURCE",
            "UNKNOWN_FFI_TARGET",
            "UNKNOWN_CONCURRENCY_TARGET",
        ],
        "unknown-kind",
    )?;
    exact_names(
        records
            .iter()
            .filter(|record| record.category == "reason-class")
            .map(|record| record.name.as_str()),
        &[
            "DYNAMIC_LANGUAGE_OPEN_WORLD",
            "EXTERNAL_BODY_NOT_INDEXED",
            "PROVIDER_UNAVAILABLE",
            "ANALYSIS_WIDENED",
            "REFLECTION_OR_CODE_GENERATION",
            "FFI_UNRESOLVED",
            "UNSUPPORTED_CONSTRUCT",
            "CONFLICTING_EXACT_EVIDENCE",
            "SOURCE_INVALID",
        ],
        "unknown-reason",
    )?;
    exact_names(
        records
            .iter()
            .filter(|record| record.category == "negative-fact-family")
            .map(|record| record.name.as_str()),
        &[
            "PROVEN_DOES_NOT_ALIAS_UNDER_PROFILE",
            "PROVEN_NO_PATH_WITHIN_PROJECTION_AND_BOUNDARY",
            "PROVEN_NOT_SUBTYPE_IN_CLOSED_TYPE_UNIVERSE",
            "PROVEN_NO_RESOLVED_MEMBER_IN_CLOSED_MEMBER_SET",
        ],
        "negative-fact",
    )
}

pub fn validate_projection_records(records: &[Projection]) -> Result<(), String> {
    exact_names(
        records.iter().map(|record| record.projection_id.as_str()),
        &[
            "SYNTAX_TREE_V1",
            "SYMBOL_BINDING_V1",
            "TYPE_GRAPH_V1",
            "CALL_EXACT_V1",
            "CALL_SOUND_V1",
            "CFG_NORMAL_V1",
            "CFG_FULL_V1",
            "DATAFLOW_V1",
            "ALIAS_V1",
            "OWNERSHIP_V1",
            "EFFECT_V1",
            "DEPENDENCY_V1",
            "CONCURRENCY_V1",
        ],
        "projection",
    )?;
    if records.iter().any(|record| {
        record.version != "1.0"
            || record.node_kind_families.is_empty()
            || record.edge_kind_codes.is_empty()
    }) {
        return Err("projection records require complete membership and policy".into());
    }
    Ok(())
}

pub fn validate_summary_records(records: &[SummaryProfile]) -> Result<(), String> {
    exact_names(
        records
            .iter()
            .map(|record| record.summary_profile_id.as_str()),
        &["CALLABLE_SUMMARY_BALANCED_V1"],
        "summary-profile",
    )?;
    if records[0].call_projection != "CALL_SOUND_V1"
        || records[0].included_direct_fact_families.is_empty()
    {
        return Err("balanced summary must propagate over CALL_SOUND_V1".into());
    }
    Ok(())
}

pub fn validate_capability_records(records: &[Capability]) -> Result<(), String> {
    exact_names(
        records.iter().map(|record| record.capability_code.as_str()),
        &[
            "SOURCE_BYTES",
            "SOURCE_INVENTORY",
            "TOKENS",
            "CST",
            "TYPED_AST",
            "SCOPES_BINDINGS",
            "IMPORT_RESOLUTION",
            "DECLARED_TYPES",
            "COMPUTED_TYPES",
            "MEMBER_RESOLUTION",
            "CALL_TARGETS",
            "RUST_MIR",
            "BORROW_LOANS",
            "CFG",
            "DOMINANCE",
            "CONTROL_DEPENDENCE",
            "DEF_USE",
            "LIVENESS",
            "POINTS_TO_ALIAS",
            "EFFECTS",
            "CONCURRENCY",
            "CALLABLE_SUMMARIES",
        ],
        "capability",
    )?;
    let scopes = BTreeSet::from([
        "workspace",
        "analysis_context",
        "build_unit",
        "module_or_crate",
        "source_file",
        "semantic_owner",
        "callable_or_MIR_body",
        "workspace_global_derivation",
    ]);
    let mut slugs = BTreeSet::new();
    for record in records {
        if !upper_snake(&record.capability_code)
            || !kebab(&record.capability_slug)
            || !slugs.insert(&record.capability_slug)
            || record.fact_families.is_empty()
            || record.allowed_scope_kinds.is_empty()
            || record
                .allowed_scope_kinds
                .iter()
                .any(|scope| !scopes.contains(scope.as_str()))
        {
            return Err(format!(
                "capability {} violates AC-G-36",
                record.capability_code
            ));
        }
    }
    Ok(())
}

pub fn validate_provider_records(records: &[Provider]) -> Result<(), String> {
    let capabilities: BTreeSet<_> = [
        "SOURCE_BYTES",
        "SOURCE_INVENTORY",
        "TOKENS",
        "CST",
        "TYPED_AST",
        "SCOPES_BINDINGS",
        "IMPORT_RESOLUTION",
        "DECLARED_TYPES",
        "COMPUTED_TYPES",
        "MEMBER_RESOLUTION",
        "CALL_TARGETS",
        "RUST_MIR",
        "BORROW_LOANS",
        "CFG",
        "DOMINANCE",
        "CONTROL_DEPENDENCE",
        "DEF_USE",
        "LIVENESS",
        "POINTS_TO_ALIAS",
        "EFFECTS",
        "CONCURRENCY",
        "CALLABLE_SUMMARIES",
    ]
    .into_iter()
    .collect();
    let mut ids = BTreeSet::new();
    let mut slugs = BTreeSet::new();
    for record in records {
        let remote = matches!(record.placement.as_str(), "SIDECAR" | "COMPILER_GROUP");
        if !ids.insert(&record.provider_id)
            || !kebab(&record.provider_slug)
            || !slugs.insert(&record.provider_slug)
            || !matches!(
                record.placement.as_str(),
                "IN_PROCESS" | "SIDECAR" | "COMPILER_GROUP"
            )
            || remote != (record.protocol_package.is_some() && record.protocol_service.is_some())
            || record.resource_profile_id.is_empty()
            || record
                .capability_codes
                .iter()
                .any(|code| !capabilities.contains(code.as_str()))
        {
            return Err(format!("provider {} violates AC-G-36", record.provider_id));
        }
    }
    Ok(())
}

pub fn validate_provider_resource_profiles(
    records: &[ProviderResourceProfile],
) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for record in records {
        if !kebab(&record.profile_id)
            || !ids.insert(&record.profile_id)
            || record.provider_ids.is_empty()
            || record.max_parallel_jobs_global == 0
            || record.max_parallel_jobs_per_workspace == 0
            || record.max_parallel_jobs_per_context == 0
            || record.max_parallel_jobs_per_context > record.max_parallel_jobs_per_workspace
            || record.max_parallel_jobs_per_workspace > record.max_parallel_jobs_global
            || record.max_input_bytes == 0
            || record.max_work_units == 0
            || record.max_wall_millis == 0
            || record.max_visited_nodes == 0
            || record.max_traversal_depth == 0
            || record.max_output_records == 0
            || record.max_output_bytes == 0
            || record.max_diagnostics == 0
            || record.max_parser_workers == 0
            || record.max_retained_tree_revisions == 0
            || record.max_cpu_weight == 0
            || record.max_memory_mib == 0
            || record.cancellation_check_interval == 0
            || record.cancellation_ack_millis == 0
            || matches!(record.retry_policy, ProviderRetryPolicy::NoRetry)
                != (record.max_retries == 0)
        {
            return Err(format!(
                "provider resource profile {} is not closed and bounded",
                record.profile_id
            ));
        }
    }
    Ok(())
}

pub fn validate_provider_normalizations(records: &[ProviderNormalization]) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    let mut catalogs = BTreeSet::new();
    for record in records {
        if !kebab(&record.mapping_id)
            || !kebab(&record.raw_catalog_id)
            || !ids.insert(&record.mapping_id)
            || !catalogs.insert(&record.raw_catalog_id)
            || record.provider_id.is_empty()
            || record.provider_version.is_empty()
            || !matches!(record.language.as_str(), "python" | "rust")
            || record.canonical_kind_names.is_empty()
            || record.field_role_names.is_empty()
            || record
                .canonical_kind_names
                .keys()
                .any(|key| record.ignored_raw_keys.contains(key))
            || record
                .canonical_kind_names
                .values()
                .any(|name| !upper_snake(name))
            || record
                .field_role_names
                .iter()
                .any(|(raw_role, name)| raw_role.is_empty() || !upper_snake(name))
            || record
                .canonical_kind_prefixes
                .iter()
                .any(|(prefix, name)| prefix.is_empty() || !upper_snake(name))
            || record.canonical_kind_prefixes.keys().any(|prefix| {
                record
                    .canonical_kind_prefixes
                    .keys()
                    .any(|other| prefix != other && prefix.starts_with(other))
            })
            || record
                .default_canonical_kind_name
                .as_deref()
                .is_some_and(|name| !upper_snake(name))
        {
            return Err(format!(
                "provider normalization {} is incomplete or ambiguous",
                record.mapping_id
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // One exhaustive public-error census is easier to audit in place.
pub fn validate_error_records(records: &[PublicError]) -> Result<(), String> {
    let mut codes = BTreeSet::new();
    let mut names = BTreeSet::new();
    if records.iter().any(|record| {
        record.code == 0
            || !codes.insert(record.code)
            || !upper_snake(&record.name)
            || !names.insert(&record.name)
    }) {
        return Err("public error codes and names must be positive and unique".into());
    }
    exact_names(
        records.iter().map(|record| record.name.as_str()),
        &[
            "INCOMPATIBLE_MAJOR",
            "UNSUPPORTED_MINOR",
            "BUNDLE_DIGEST_MISMATCH",
            "WORKSPACE_NOT_AUTHORIZED",
            "PATH_OUTSIDE_AUTHORIZED_ROOT",
            "SOURCE_ACCESS_DENIED",
            "BLOCKED_PATH_COLLISION",
            "FRESHNESS_DEADLINE_EXCEEDED",
            "CAPABILITY_UNAVAILABLE",
            "NEGATIVE_PROOF_INDETERMINATE",
            "SOURCE_SNAPSHOT_MISMATCH",
            "PROVIDER_PROTOCOL_ERROR",
            "SANDBOX_UNAVAILABLE",
            "RUFF_SEMANTIC_UNAVAILABLE_PARSE",
            "RUFF_SEMANTIC_CLEANUP_FAILED",
            "SEMANTIC_LANE_FAILED",
            "QUERY_HARD_LIMIT_EXCEEDED",
            "ENTITY_AMBIGUOUS",
            "SEMANTIC_PHRASE_AMBIGUOUS",
            "SEMANTIC_PHRASE_UNRECOGNIZED",
            "CURRENT_POINTER_CONFLICT",
            "ID_COLLISION",
            "OVERLAY_GENERATION_CONFLICT",
            "CREDENTIAL_REPLAY_DETECTED",
            "IDEMPOTENCY_CONFLICT",
            "RESUME_WINDOW_EXPIRED",
            "RESULT_TOO_LARGE_FOR_HOST",
            "ARTIFACT_ID_COLLISION",
            "RESOURCE_EXPIRED",
            "STATE_TRANSITION_VIOLATION",
            "INTERNAL_INVARIANT_VIOLATION",
            "INVALID_REQUEST_SCHEMA",
            "CONTEXT_NOT_INDEXED",
            "COMPOSITE_SNAPSHOT_UNSUPPORTED",
            "RESOURCE_LIMIT_REJECTED",
            "CANCELLED",
            "ADAPTER_INPUT_NOT_JSON",
            "ADAPTER_INPUT_LIMIT",
            "ADAPTER_INPUT_VALIDATION",
            "ADAPTER_OUTPUT_CONTRACT",
            "UNSUPPORTED_BINARY",
            "UNSUPPORTED_CONTENT",
            "CURRENT_FACTS_UNAVAILABLE",
            "DEFAULT_CONTEXT_UNAVAILABLE",
            "WORKSPACE_BOOTSTRAPPING",
            "FACT_FAMILY_UNAVAILABLE",
            "SNAPSHOT_FRESHNESS_MISMATCH",
            "QUERY_LOST_DAEMON_RESTART",
            "COMPARISON_DOMAIN_MISMATCH",
            "SCHEMA_MISMATCH",
            "TABLE_SET_MISMATCH",
            "ROW_MISMATCH",
            "CAPABILITY_MISMATCH",
            "SNAPSHOT_METADATA_MISMATCH",
            "ID_COLLISION_DETECTED",
            "COMPARATOR_ERROR",
            "PUBLICATION_REFERENTIAL_INTEGRITY",
            "GOVERNED_PLAN_INGRESS_REJECTED",
            "SEMANTIC_PHRASE_UNSUPPORTED",
            "ONTOLOGY_GATE_ROW_LIMIT",
            "ONTOLOGY_GATE_BYTE_LIMIT",
            "ONTOLOGY_GATE_BATCH_LIMIT",
            "ONTOLOGY_GATE_COUNTER_OVERFLOW",
            "ONTOLOGY_PROGRAM_RESOURCE_LIMIT",
            "REQUIRED_FEATURE_UNSUPPORTED",
            "SCHEMA_DIGEST_MISMATCH",
            "TOOLCHAIN_MISMATCH",
            "MODEL_PACK_INCOMPATIBLE",
            "PLATFORM_UNSUPPORTED",
            "DAEMON_UNAVAILABLE",
            "CONTRACT_MISMATCH",
            "ONTOLOGY_PROGRAM_CONTRACT_INVALID",
            "ONTOLOGY_PROGRAM_DECODE_INVALID",
            "ONTOLOGY_PROGRAM_DIGEST_MISMATCH",
            "ONTOLOGY_PROGRAM_UNSUPPORTED",
            "ONTOLOGY_CANDIDATE_CLOSURE_INVALID",
            "ONTOLOGY_ACTIVATION_TRANSACTION_INVALID",
            "ONTOLOGY_PROGRAM_ARTIFACT_INVALID",
            "INTERNAL",
        ],
        "public-error",
    )?;
    for record in records {
        if !(1000..=9999).contains(&record.code)
            || !matches!(
                record.severity.as_str(),
                "INFO" | "WARNING" | "ERROR" | "FATAL"
            )
            || !matches!(
                record.retryability.as_str(),
                "NEVER" | "SAME_SNAPSHOT" | "NEW_SNAPSHOT" | "AFTER_RECONFIGURATION" | "TRANSIENT"
            )
            || !matches!(
                record.scope.as_str(),
                "REQUEST" | "QUERY_BLOCK" | "PROVIDER_RUN" | "WORKSPACE" | "DAEMON"
            )
        {
            return Err(format!("error {} violates AC-G-65", record.name));
        }
    }
    Ok(())
}

fn output_role_matches(node: PlanNodeKind, output: PhraseOutputRole) -> bool {
    match node {
        PlanNodeKind::FindEntities => output == PhraseOutputRole::EntitySet,
        PlanNodeKind::RetrieveFacts | PlanNodeKind::FollowRelationships => {
            matches!(
                output,
                PhraseOutputRole::FactSet | PhraseOutputRole::CoverageProof
            )
        }
        PlanNodeKind::FindPaths => output == PhraseOutputRole::PathSet,
        PlanNodeKind::MatchPattern => output == PhraseOutputRole::BindingTable,
        PlanNodeKind::CombineSets => matches!(
            output,
            PhraseOutputRole::EntitySet
                | PhraseOutputRole::FactSet
                | PhraseOutputRole::BindingTable
        ),
        PlanNodeKind::SummarizeFacts => matches!(
            output,
            PhraseOutputRole::GroupSet
                | PhraseOutputRole::ScalarSummary
                | PhraseOutputRole::CoverageProof
        ),
        PlanNodeKind::FetchSourceContext => output == PhraseOutputRole::SourceContextSet,
    }
}

fn forbidden_mapping_text(record: &PhraseRecord) -> bool {
    let forbidden = |text: &str| {
        let folded = text.to_ascii_lowercase();
        folded.contains("deferred-mapping")
            || folded.contains("placeholder")
            || folded.contains("todo")
    };
    forbidden(&record.phrase_id)
        || forbidden(&record.canonical_text)
        || record.accepted_aliases.iter().any(|text| forbidden(text))
        || forbidden(&record.contract_reference.code)
        || record.required_modifiers.iter().any(|text| forbidden(text))
        || record
            .incompatible_modifiers
            .iter()
            .any(|text| forbidden(text))
        || record.examples.iter().any(|text| forbidden(text))
        || record.negative_fixtures.iter().any(|text| forbidden(text))
        || record
            .planspec_mapping
            .typed_slot_bindings
            .iter()
            .any(|binding| forbidden(&binding.slot))
        || record
            .planspec_mapping
            .constant_fields
            .iter()
            .any(|binding| match &binding.value {
                PhraseConstantValue::Text { text } => forbidden(text),
                PhraseConstantValue::TextList { values } => {
                    values.iter().any(|text| forbidden(text))
                }
                PhraseConstantValue::Boolean { .. } => false,
            })
}

fn validate_phrase_record<'a>(
    record: &'a PhraseRecord,
    ids: &mut BTreeSet<&'a String>,
    canonical_texts: &mut BTreeSet<String>,
) -> Result<(), String> {
    let normalized = record
        .canonical_text
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if !upper_snake(&record.phrase_id)
        || !ids.insert(&record.phrase_id)
        || record.canonical_text.is_empty()
        || !record.canonical_text.is_ascii()
        || !canonical_texts.insert(normalized.clone())
        || record.contract_reference.code.is_empty()
        || record.allowed_request_forms.is_empty()
        || !record
            .allowed_request_forms
            .contains(&record.planspec_mapping.node_kind.request_form())
        || record.planspec_mapping.constant_fields.is_empty()
        || !output_role_matches(
            record.planspec_mapping.node_kind,
            record.planspec_mapping.output_role,
        )
        || record.examples.is_empty()
        || record.negative_fixtures.is_empty()
        || forbidden_mapping_text(record)
        || contains_evaluative_kind(
            &serde_json::to_value(record).expect("typed phrase record serialization is infallible"),
        )
    {
        return Err(format!(
            "phrase {} is not a complete executable AC-G-44 mapping",
            record.phrase_id
        ));
    }
    let mut aliases = BTreeSet::new();
    for alias in &record.accepted_aliases {
        let alias = alias
            .split_ascii_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        if alias.is_empty() || !alias.is_ascii() || alias == normalized || !aliases.insert(alias) {
            return Err(format!(
                "phrase {} has an invalid or duplicate alias",
                record.phrase_id
            ));
        }
    }
    let mut slots = BTreeSet::new();
    if record
        .planspec_mapping
        .typed_slot_bindings
        .iter()
        .any(|binding| binding.slot.is_empty() || !slots.insert(&binding.slot))
    {
        return Err(format!(
            "phrase {} has duplicate or empty typed slots",
            record.phrase_id
        ));
    }
    let mut constants = BTreeSet::new();
    if record
        .planspec_mapping
        .constant_fields
        .iter()
        .any(|binding| !constants.insert(binding.target_field))
    {
        return Err(format!(
            "phrase {} assigns a PlanSpec constant field twice",
            record.phrase_id
        ));
    }
    if record.contract_reference.family == PhraseReferenceFamily::Projection
        && !record
            .planspec_mapping
            .constant_fields
            .iter()
            .any(|binding| {
                binding.target_field == PlanField::ProjectionId
                    && matches!(
                        &binding.value,
                        PhraseConstantValue::Text { text }
                            if text == &record.contract_reference.code
                    )
            })
    {
        return Err(format!(
            "phrase {} does not bind its projection authority into PlanSpec",
            record.phrase_id
        ));
    }
    Ok(())
}

pub fn validate_phrase_records(records: &[PhraseRecord]) -> Result<(), String> {
    let expected_sections: BTreeSet<_> = (50_u8..=94).collect();
    let actual_sections: BTreeSet<_> = records.iter().map(|record| record.owner_section).collect();
    if records.len() != expected_sections.len() || actual_sections != expected_sections {
        return Err(
            "phrase registry must contain exactly one owner record for Query sections 50..=94"
                .into(),
        );
    }
    let mut ids = BTreeSet::new();
    let mut canonical_texts = BTreeSet::new();
    for record in records {
        validate_phrase_record(record, &mut ids, &mut canonical_texts)?;
    }
    Ok(())
}

pub fn validate_phrase_operation_bindings(
    bindings: &[PhraseOperationBinding],
) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    let mut texts = BTreeSet::new();
    let required_ingresses = BTreeSet::from([
        PhraseOperationIngress::Relational,
        PhraseOperationIngress::Graph,
    ]);
    for binding in bindings {
        if !upper_snake(&binding.phrase_id)
            || !ids.insert(binding.phrase_id.as_str())
            || binding.canonical_text.trim().is_empty()
            || !texts.insert(binding.canonical_text.as_str())
            || binding.column_role.trim().is_empty()
            || !upper_snake(&binding.operand_domain)
            || binding.operand_names.is_empty()
            || binding.operand_names.iter().any(|name| !upper_snake(name))
            || binding.output_role != "predicate"
            || binding.ingresses != required_ingresses
            || !upper_snake(&binding.diagnostic_code)
        {
            return Err(format!(
                "phrase operation {} is incomplete or not closed over both compiler paths",
                binding.phrase_id
            ));
        }
        if binding.operator == PhrasePredicateOperator::Equals && binding.operand_names.len() != 1 {
            return Err(format!(
                "phrase operation {} uses equals with a non-scalar operand",
                binding.phrase_id
            ));
        }
    }
    if bindings.is_empty() {
        return Err("phrase registry must define compiled semantic operations".into());
    }
    Ok(())
}

pub fn validate_phrase_projection_bindings(
    phrases: &[PhraseRecord],
    bindings: &[PhraseProjectionBinding],
) -> Result<(), String> {
    let phrase_ids = phrases
        .iter()
        .map(|phrase| phrase.phrase_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut keys = BTreeSet::new();
    for binding in bindings {
        let target = match binding.target {
            PhraseProjectionTarget::EntityKind => "entity_kind",
            PhraseProjectionTarget::RelationKind => "relation_kind",
            PhraseProjectionTarget::PropertyKind => "property_kind",
        };
        if !phrase_ids.contains(binding.phrase_id.as_str())
            || !keys.insert((binding.phrase_id.as_str(), target))
            || binding.operand_names.is_empty()
            || binding.operand_names.iter().any(|name| !upper_snake(name))
        {
            return Err(format!(
                "phrase projection {}:{target} is dangling, duplicated, or operand-free",
                binding.phrase_id
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // One pass keeps every cross-machine registry invariant adjacent.
pub fn validate_state_machines(records: &[StateMachine]) -> Result<(), String> {
    const REQUIRED: [&str; 14] = [
        "WorkspaceLifecycle",
        "SourceTrustState",
        "EventStreamHealth",
        "GitAccelerationStatus",
        "UpdateWaveState",
        "ProviderRunState",
        "OwnerCapabilityState",
        "DurablePublicationState",
        "ServingActivationState",
        "SnapshotLeaseState",
        "QueryExecutionState",
        "ArtifactState",
        "WorkspaceRegistryLifecycle",
        "OntologyCandidateLifecycle",
    ];
    let ids: BTreeSet<_> = records
        .iter()
        .map(|record| record.machine_id.as_str())
        .collect();
    if ids != REQUIRED.into_iter().collect() {
        return Err("state-machine registry must contain the governed lifecycle roster".into());
    }
    for machine in records {
        let states: BTreeSet<_> = machine
            .states
            .iter()
            .map(|state| state.name.as_str())
            .collect();
        validate_enum_values(&machine.machine_id, &machine.states)?;
        if !states.contains(machine.initial_state.as_str())
            || machine
                .terminal_states
                .iter()
                .any(|state| !states.contains(state.as_str()))
        {
            return Err(format!(
                "machine {} has invalid initial/terminal states",
                machine.machine_id
            ));
        }
        let decode_only: BTreeSet<_> = machine
            .decode_only_states
            .iter()
            .map(String::as_str)
            .collect();
        if decode_only.contains(machine.initial_state.as_str())
            || machine
                .terminal_states
                .iter()
                .any(|state| decode_only.contains(state.as_str()))
            || !decode_only.is_subset(&states)
        {
            return Err(format!(
                "machine {} has invalid decode-only states",
                machine.machine_id
            ));
        }
        let mut reachable = BTreeSet::from([machine.initial_state.as_str()]);
        loop {
            let before = reachable.len();
            for transition in &machine.transitions {
                if !states.contains(transition.from.as_str())
                    || !states.contains(transition.to.as_str())
                    || transition.event.is_empty()
                    || transition.guard.is_empty()
                    || transition.actions.is_empty()
                    || transition.idempotency_key.is_empty()
                    || transition.error_on_illegal != "STATE_TRANSITION_VIOLATION"
                    || decode_only.contains(transition.from.as_str())
                    || decode_only.contains(transition.to.as_str())
                {
                    return Err(format!(
                        "machine {} has an incomplete transition",
                        machine.machine_id
                    ));
                }
                if reachable.contains(transition.from.as_str()) {
                    reachable.insert(&transition.to);
                }
            }
            if reachable.len() == before {
                break;
            }
        }
        let active_states = states
            .difference(&decode_only)
            .copied()
            .collect::<BTreeSet<_>>();
        if reachable != active_states {
            return Err(format!(
                "machine {} contains unreachable states",
                machine.machine_id
            ));
        }
        let terminal: BTreeSet<_> = machine.terminal_states.iter().map(String::as_str).collect();
        let mut can_terminate = terminal.clone();
        loop {
            let before = can_terminate.len();
            for transition in &machine.transitions {
                if can_terminate.contains(transition.to.as_str()) {
                    can_terminate.insert(&transition.from);
                }
            }
            if can_terminate.len() == before {
                break;
            }
        }
        if can_terminate != active_states {
            return Err(format!(
                "machine {} has a state without a terminal path",
                machine.machine_id
            ));
        }
    }
    Ok(())
}

pub fn contains_evaluative_kind(value: &serde_json::Value) -> bool {
    const DENIED: [&str; 4] = [
        "SAFE_TO_REFACTOR",
        "TEST_IMPACTED",
        "HIGH_RISK",
        "SHOULD_CHANGE",
    ];
    match value {
        serde_json::Value::String(text) => DENIED.iter().any(|denied| text.contains(denied)),
        serde_json::Value::Array(values) => values.iter().any(contains_evaluative_kind),
        serde_json::Value::Object(values) => values.values().any(contains_evaluative_kind),
        _ => false,
    }
}

/// Replay bounded registry ingress families used by the fuzz target.
pub fn replay_bounded_registry_ingress(selector: u8, source: &[u8]) {
    match selector % 4 {
        0 => {
            if let Ok(document) = serde_yaml_ng::from_slice::<AcceptedRegistry<EnumDomain>>(source)
            {
                let _ = validate_enum_domains(&document.records);
            }
        }
        1 => {
            if let Ok(document) = serde_yaml_ng::from_slice::<AcceptedRegistry<FlagDomain>>(source)
            {
                let _ = validate_flag_domains(&document.records);
            }
        }
        2 => {
            if let Ok(document) =
                serde_yaml_ng::from_slice::<AcceptedRegistry<StateMachine>>(source)
            {
                let _ = validate_state_machines(&document.records);
            }
        }
        _ => {
            if let Ok(document) = serde_yaml_ng::from_slice::<PhraseRegistry>(source) {
                let _ = validate_phrase_records(&document.records);
                let _ = validate_phrase_operation_bindings(&document.semantic_operation_bindings);
                let _ = validate_phrase_projection_bindings(
                    &document.records,
                    &document.semantic_projection_bindings,
                );
            }
        }
    }
}

/// Reject one ontology concern slug claimed by more than one authority family.
pub fn validate_duplicate_authorities<'a, I>(entries: I) -> Result<(), String>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut owners = BTreeMap::new();
    for (authority, slug) in entries {
        if let Some(previous) = owners.insert(slug, authority)
            && previous != authority
        {
            return Err(format!(
                "ontology slug {slug} is authoritative in both {previous} and {authority}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phrases() -> PhraseRegistry {
        serde_yaml_ng::from_str(include_str!(
            "../../contracts/registry/phrase-registry.yaml"
        ))
        .unwrap()
    }

    #[test]
    fn wp08b_structural_acceptance() {
        let phrases = phrases();
        validate_phrase_records(&phrases.records).unwrap();
        validate_phrase_operation_bindings(&phrases.semantic_operation_bindings).unwrap();
        validate_phrase_projection_bindings(
            &phrases.records,
            &phrases.semantic_projection_bindings,
        )
        .unwrap();
        assert_eq!(phrases.records.len(), 45);
        assert_eq!(
            phrases
                .records
                .iter()
                .map(|record| record.owner_section)
                .collect::<BTreeSet<_>>(),
            (50_u8..=94).collect()
        );
        assert!(phrases.records.iter().all(|record| {
            !record.planspec_mapping.constant_fields.is_empty()
                && record
                    .allowed_request_forms
                    .contains(&record.planspec_mapping.node_kind.request_form())
        }));
    }

    #[test]
    fn wp08b_negative_zero_state() {
        let phrases = phrases();

        let mut out_of_scope = phrases.records.clone();
        out_of_scope[0].owner_section = 95;
        assert!(validate_phrase_records(&out_of_scope).is_err());

        let mut deferred = phrases.records.clone();
        deferred[0].canonical_text = "deferred-mapping".into();
        assert!(validate_phrase_records(&deferred).is_err());

        let mut evaluative = phrases.records.clone();
        evaluative[0].canonical_text = "SAFE_TO_REFACTOR".into();
        assert!(validate_phrase_records(&evaluative).is_err());

        let mut missing_slots = serde_json::to_value(&phrases.records[0]).unwrap();
        missing_slots["planspec_mapping"]
            .as_object_mut()
            .unwrap()
            .remove("typed_slot_bindings");
        assert!(serde_json::from_value::<PhraseRecord>(missing_slots).is_err());

        let mut unknown_node = serde_json::to_value(&phrases.records[0]).unwrap();
        unknown_node["planspec_mapping"]["node_kind"] = serde_json::json!("deferred-node");
        assert!(serde_json::from_value::<PhraseRecord>(unknown_node).is_err());
    }

    #[test]
    fn wp08_negative_zero_state() {
        assert!(contains_evaluative_kind(&serde_json::json!({
            "canonical_name": "SAFE_TO_REFACTOR"
        })));
        let mut domains: AcceptedRegistry<EnumDomain> =
            serde_yaml_ng::from_str(include_str!("../../contracts/registry/enum-registry.yaml"))
                .unwrap();
        domains.records[0].values[0].code = 0;
        assert!(validate_enum_domains(&domains.records).is_err());
        assert!(
            validate_duplicate_authorities([
                ("entity-registry", "duplicate"),
                ("relation-registry", "duplicate"),
            ])
            .is_err()
        );
    }

    #[test]
    fn wp08_each_registry_family_rejects_an_invalid_semantic_record() {
        let mut flags: AcceptedRegistry<FlagDomain> =
            serde_yaml_ng::from_str(include_str!("../../contracts/registry/flag-registry.yaml"))
                .unwrap();
        flags.records[0].width_bits = 32;
        assert!(validate_flag_domains(&flags.records).is_err());

        let mut entities: AcceptedRegistry<EntityKind> = serde_yaml_ng::from_str(include_str!(
            "../../contracts/registry/ontology-entity-registry.yaml"
        ))
        .unwrap();
        entities.records[0].kind_code = 0;
        assert!(validate_records(&entities.records).is_err());
        entities.records[0].kind_code = 10;
        entities.records[0].family_code = 0;
        assert!(validate_entity_records(&entities.records).is_err());

        let mut properties: AcceptedRegistry<PropertyKind> = serde_yaml_ng::from_str(include_str!(
            "../../contracts/registry/ontology-property-registry.yaml"
        ))
        .unwrap();
        properties.records[0].null_semantics = "allowed".into();
        assert!(validate_property_records(&properties.records).is_err());

        let mut relations: AcceptedRegistry<RelationKind> = serde_yaml_ng::from_str(include_str!(
            "../../contracts/registry/ontology-relation-registry.yaml"
        ))
        .unwrap();
        relations.records[0].storage_table = "shadow_relation".into();
        assert!(validate_relation_records(&relations.records).is_err());

        let mut facts: AcceptedRegistry<FactKind> = serde_yaml_ng::from_str(include_str!(
            "../../contracts/registry/ontology-fact-registry.yaml"
        ))
        .unwrap();
        facts.records[0].shape = "property".into();
        assert!(validate_fact_records(&facts.records).is_err());

        let mut unknowns: AcceptedRegistry<UnknownKind> = serde_yaml_ng::from_str(include_str!(
            "../../contracts/registry/unknown-registry.yaml"
        ))
        .unwrap();
        unknowns.records.pop();
        assert!(validate_unknown_records(&unknowns.records).is_err());

        let mut projections: AcceptedRegistry<Projection> = serde_yaml_ng::from_str(include_str!(
            "../../contracts/registry/projection-registry.yaml"
        ))
        .unwrap();
        projections.records.pop();
        assert!(validate_projection_records(&projections.records).is_err());

        let mut summaries: AcceptedRegistry<SummaryProfile> = serde_yaml_ng::from_str(
            include_str!("../../contracts/registry/summary-registry.yaml"),
        )
        .unwrap();
        summaries.records[0].call_projection = "CALL_EXACT_V1".into();
        assert!(validate_summary_records(&summaries.records).is_err());

        let mut capabilities: AcceptedRegistry<Capability> = serde_yaml_ng::from_str(include_str!(
            "../../contracts/registry/capability-registry.yaml"
        ))
        .unwrap();
        capabilities.records.pop();
        assert!(validate_capability_records(&capabilities.records).is_err());

        let mut providers: AcceptedRegistry<Provider> = serde_yaml_ng::from_str(include_str!(
            "../../contracts/registry/provider-registry.yaml"
        ))
        .unwrap();
        providers.records[0].placement = "REMOTE".into();
        assert!(validate_provider_records(&providers.records).is_err());

        let mut errors: AcceptedRegistry<PublicError> =
            serde_yaml_ng::from_str(include_str!("../../contracts/registry/error-registry.yaml"))
                .unwrap();
        errors.records.pop();
        assert!(validate_error_records(&errors.records).is_err());
    }

    #[test]
    fn wp08_operational_acceptance() {
        let machines: AcceptedRegistry<StateMachine> = serde_yaml_ng::from_str(include_str!(
            "../../contracts/registry/state-machine-registry.yaml"
        ))
        .unwrap();
        validate_state_machines(&machines.records).unwrap();
        let mut unreachable = machines.records;
        unreachable[0]
            .transitions
            .retain(|transition| transition.to != "DEGRADED");
        assert!(validate_state_machines(&unreachable).is_err());
    }

    /// PC-WP70-STR/OPS: every released fault record names an executable seam.
    #[cfg(feature = "daemon")]
    #[test]
    fn wp70_fault_registry_runtime_census() {
        let registry: AcceptedRegistry<FaultPointRecord> = serde_yaml_ng::from_str(include_str!(
            "../../contracts/faults/fault-point-registry.yaml"
        ))
        .unwrap();
        validate_fault_points(&registry.records).unwrap();
        let released = registry
            .records
            .iter()
            .map(|record| record.code.as_str())
            .collect::<BTreeSet<_>>();
        let mut executable = crate::source_image::SOURCE_IMAGE_FAULT_POINT_CODES
            .into_iter()
            .collect::<BTreeSet<_>>();
        executable.extend(
            crate::fabric::MutationFaultPoint::ALL
                .into_iter()
                .map(crate::fabric::MutationFaultPoint::code),
        );
        executable.extend(
            crate::fabric::PublicationFaultPoint::ALL
                .into_iter()
                .map(crate::fabric::PublicationFaultPoint::code),
        );
        executable.extend(
            crate::fabric::OverlayRebaseFaultPoint::ALL
                .into_iter()
                .map(crate::fabric::OverlayRebaseFaultPoint::code),
        );
        executable.extend(
            crate::query_service::QueryArtifactFaultPoint::ALL
                .into_iter()
                .map(crate::query_service::QueryArtifactFaultPoint::code),
        );
        executable.extend(crate::provider_runtime::SEMANTIC_PROVIDER_FAULT_POINT_CODES);
        executable.extend(crate::operational_store::ONTOLOGY_ACTIVATION_FAULT_POINT_CODES);
        assert_eq!(released, executable);
    }
}
