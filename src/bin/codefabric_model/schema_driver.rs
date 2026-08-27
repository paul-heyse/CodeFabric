//! Contract-IR-driven schema, `TableSpec`, DDL, and row-encoder family driver.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use super::desired_tree::SafeOutputPath;
use super::driver_protocol::{
    DriverDescriptor, DriverOutputRole, DriverOutputSpec, DriverProtocolError,
    DriverResourceProfile, DriverSourceFence, ModelDriver, StagingRoot, executable_tool_identity,
    process_stage_root, rustfmt_source,
};
use super::incremental::{CacheLookup, render_with_cache};
use super::model_control::StableId;
use super::repository_model::read_stable;

const SCHEMA_IR_PATH: &str = "contracts/schema/schema-contract-ir.json";
const QUERY_FORM_CONTRACT_PATH: &str = "contracts/query/query-form-contract.json";
const RUST_QUERY_FORM_BINDINGS_PATH: &str = "src/generated/model_query_forms.rs";
const PYTHON_QUERY_FORM_BINDINGS_PATH: &str =
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/query_forms.py";
const PYTHON_QUERY_FORM_CONTRACT_PATH: &str =
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/query-form-contract.json";
const TABLE_MANIFEST_PATH: &str = "contracts/generated/model/schema/table-specs.json";
const DDL_PATH: &str = "contracts/generated/model/schema/operational-store.sql";
const RUST_BINDINGS_PATH: &str = "src/generated/model_schema_tables.rs";
const RUST_RUNTIME_BINDINGS_PATH: &str = "src/generated/table_specs.rs";
const RUST_ROW_ENCODERS_PATH: &str = "src/generated/fact_row_encoders.rs";
const VALIDATION_PATH: &str = "contracts/generated/model/schema/schema-validation.json";
const EVOLUTION_POLICY_PATH: &str = "contracts/generated/model/schema/schema-evolution-policy.json";
const PUBLIC_SCHEMA_INSTANCES_SOURCE_PATH: &str =
    "tooling/model/fixtures/public-schema-golden-instances.json";
const PUBLIC_SCHEMA_INSTANCES_PATH: &str =
    "contracts/generated/model/schema/public-schema-golden-instances.json";
const ENUM_REGISTRY_PATH: &str = "contracts/registry/enum-registry.yaml";
const TYPE_ALGEBRA_PATH: &str = "contracts/identity/type-algebra-v1.yaml";
const ENTITY_REGISTRY_PATH: &str = "contracts/registry/ontology-entity-registry.yaml";
const RELATION_REGISTRY_PATH: &str = "contracts/registry/ontology-relation-registry.yaml";
const PROPERTY_REGISTRY_PATH: &str = "contracts/registry/ontology-property-registry.yaml";
const FACT_REGISTRY_PATH: &str = "contracts/registry/ontology-fact-registry.yaml";
const CAPABILITY_REGISTRY_PATH: &str = "contracts/registry/capability-registry.yaml";
const MAX_AUTHORITY_BYTES: usize = 16 * 1024 * 1024;
const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum QueryFieldType {
    BoundedString,
    BoundedStringList,
    SemanticReferenceList,
    PriorResultList,
    PatternBindingList,
    PatternRelationshipList,
    PositiveInteger,
    ReturnSpec,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryFormFieldContract {
    name: String,
    rust_name: String,
    field_type: QueryFieldType,
    required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryFormContractEntry {
    code: u16,
    name: String,
    slug: String,
    rust_variant: String,
    node_kind: String,
    owner_section: u16,
    output_role: String,
    accepted_input_roles: Vec<String>,
    canonical_order: Vec<String>,
    fields: Vec<QueryFormFieldContract>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryFormContract {
    artifact_id: String,
    artifact_kind: String,
    version: String,
    compatible_suite_major: u64,
    status: String,
    canonical_digest: String,
    registry_domain: String,
    specification: String,
    specification_version: String,
    result_roles: Vec<String>,
    forms: Vec<QueryFormContractEntry>,
}

impl QueryFormContract {
    fn validate(
        &self,
        source_bytes: &[u8],
        enum_registry_bytes: &[u8],
    ) -> Result<(), SchemaDriverError> {
        if self.artifact_id != "codefabric.query.form-contract"
            || self.artifact_kind != "semantic-contract"
            || self.version != "1.0"
            || self.compatible_suite_major != 1
            || self.status != "released"
            || self.registry_domain != "QUERY_FORM"
            || self.specification != "composable semantic CPG fact query"
            || self.specification_version != "1.3"
        {
            return invalid("$", "invalid query-form contract header");
        }
        if detached_query_form_identity(source_bytes)? != self.canonical_digest {
            return invalid("$.canonical_digest", "detached query-form identity differs");
        }
        let expected_roles = BTreeSet::from([
            "entities",
            "facts",
            "paths",
            "pattern_bindings",
            "groups",
            "summary",
            "source_contexts",
        ]);
        if self
            .result_roles
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != expected_roles
        {
            return invalid("$.result_roles", "result-role census differs");
        }
        if self.forms.len() != 8 {
            return invalid("$.forms", "all eight QRY 1.3 forms are required");
        }
        let mut codes = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut slugs = BTreeSet::new();
        let mut variants = BTreeSet::new();
        let mut node_kinds = BTreeSet::new();
        let mut owners = BTreeSet::new();
        for (index, form) in self.forms.iter().enumerate() {
            let path = format!("$.forms[{index}]");
            if !codes.insert(form.code)
                || !names.insert(form.name.as_str())
                || !slugs.insert(form.slug.as_str())
                || !variants.insert(form.rust_variant.as_str())
                || !node_kinds.insert(form.node_kind.as_str())
                || !owners.insert(form.owner_section)
                || !expected_roles.contains(form.output_role.as_str())
                || form.canonical_order.is_empty()
            {
                return invalid(&path, "duplicate or invalid query-form identity");
            }
            let mut field_names = BTreeSet::new();
            let mut rust_names = BTreeSet::new();
            for field in &form.fields {
                if !field_names.insert(field.name.as_str())
                    || !rust_names.insert(field.rust_name.as_str())
                    || !identifier(field.rust_name.trim_start_matches("r#"))
                {
                    return invalid(&format!("{path}.fields"), "duplicate or invalid form field");
                }
            }
        }
        if codes != BTreeSet::from([10, 20, 30, 40, 50, 60, 70, 80])
            || owners != BTreeSet::from([13, 14, 15, 16, 17, 18, 19, 20])
        {
            return invalid("$.forms", "QRY form codes or owner sections differ");
        }
        validate_query_form_registry(self, enum_registry_bytes)
    }
}

/// Header carried temporarily by the legacy native authority during detached-identity cutover.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityHeader {
    artifact_id: String,
    artifact_kind: String,
    version: String,
    compatible_suite_major: u64,
    status: String,
    canonical_digest: String,
    generator_revision: String,
}

/// Contract-owned logical types. Their Arrow mapping is defined once by this driver.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogicalType {
    Id16,
    Hash32,
    Code16,
    Code32,
    Bucket16,
    Int16,
    Int32,
    Int64,
    Float64,
    Boolean,
    Utf8,
    Binary,
    TimestampUtc,
    IdList,
    Int64List,
    StringMap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum DurableMutationClass {
    StaticDimension,
    CurrentSingleton,
    OwnerReplacedFact,
    PublicationAppend,
    DerivedOwnerReplaced,
    GlobalDerivedReplacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum OverlayMutationPolicy {
    OwnerReplace,
    PrimaryKeyUpsert,
    FullTableReplace,
    BaseImmutable,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum MaterializationRole {
    DurableEffective,
    BundleDimension,
    QueryTimeDerived,
    OperationalProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum PublicationPinRole {
    PinnedData,
    ManifestControl,
    PointerControl,
    NotPublished,
}

/// Closed implementation kinds whose typed encoders are emitted from table columns.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RowEncoderKind {
    Owners,
    CapabilityStatuses,
    Entities,
    Relations,
    Properties,
    Evidence,
    SourceFiles,
    SourceTokens,
    SourceAnnotations,
    SyntaxDetails,
    TypeDetails,
    TypeFactDetails,
}

impl RowEncoderKind {
    const fn rust_function(self) -> &'static str {
        match self {
            Self::Owners => "encode_owners",
            Self::CapabilityStatuses => "encode_capability_statuses",
            Self::Entities => "encode_entities",
            Self::Relations => "encode_relations",
            Self::Properties => "encode_properties",
            Self::Evidence => "encode_evidence",
            Self::SourceFiles => "encode_source_files",
            Self::SourceTokens => "encode_source_tokens",
            Self::SourceAnnotations => "encode_source_annotations",
            Self::SyntaxDetails => "encode_syntax_details",
            Self::TypeDetails => "encode_type_details",
            Self::TypeFactDetails => "encode_type_fact_details",
        }
    }

    const fn rust_row_type(self) -> &'static str {
        match self {
            Self::Owners => "OwnerRow",
            Self::CapabilityStatuses => "CapabilityStatusRow",
            Self::Entities => "EntityRow",
            Self::Relations => "RelationRow",
            Self::Properties => "PropertyFactRow",
            Self::Evidence => "FactEvidenceRow",
            Self::SourceFiles => "SourceFileRow",
            Self::SourceTokens => "SourceTokenRow",
            Self::SourceAnnotations => "SourceAnnotationRow",
            Self::SyntaxDetails => "SyntaxDetailRow",
            Self::TypeDetails => "TypeDetailRow",
            Self::TypeFactDetails => "TypeFactDetailRow",
        }
    }
}

/// P21's exhaustive six-class disposition for schema-IR annotations.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum MetadataClass {
    Enforced,
    PlannerConsumed,
    Contractual,
    Governance,
    Lineage,
    Advisory,
}

/// One annotation classification and its concrete non-advisory consumer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataAnnotationContract {
    annotation: String,
    class: MetadataClass,
    #[serde(default)]
    consumer_path: Option<String>,
    #[serde(default)]
    consumer_symbol: Option<String>,
}

/// Registry or contract authority that gives a semantic type its meaning.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SemanticAuthority {
    EnumRegistry,
    TypeAlgebra,
    OntologyEntityRegistry,
    OntologyRelationRegistry,
    OntologyPropertyRegistry,
    OntologyFactRegistry,
    CapabilityRegistry,
    SchemaIr,
    Intrinsic,
    ProviderCatalog,
    DiagnosticProtocol,
}

/// Digest-pinned link from the schema IR to one external semantic authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticAuthorityContract {
    authority: SemanticAuthority,
    artifact_id: String,
    path: String,
    canonical_digest: String,
}

/// Resolution rule for one distinct `semantic_type` value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticTypeBindingContract {
    semantic_type: String,
    authority: SemanticAuthority,
    #[serde(default)]
    domain: Option<String>,
}

/// Explicit compatibility class for the versioned schema-evolution policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SchemaCompatibilityClass {
    ExactPin,
}

/// SQLite cannot enforce references whose target rows live in Delta candidate state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SqliteForeignKeyPosture {
    NotEmittedCrossStore,
}

/// Versioned migration and acceptance contract; the IR remains its sole authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaEvolutionPolicyContract {
    policy_id: String,
    version: String,
    compatibility_class: SchemaCompatibilityClass,
    require_schema_digest_equality: bool,
    allow_type_widening: bool,
    column_mapping_mode: String,
    migration_route: Vec<String>,
}

/// One ordered physical field. `field_id` is derived as `<table>.<name>`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColumnContract {
    name: String,
    logical_type: LogicalType,
    nullable: bool,
    #[serde(default)]
    semantic_type: Option<String>,
    #[serde(default)]
    foreign_key: Option<String>,
    #[serde(default)]
    hidden_operational: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TableContract {
    table_code: i16,
    name: String,
    publication_pin_role: PublicationPinRole,
    family: String,
    grain: String,
    schema_version: String,
    #[serde(default)]
    row_encoder: Option<RowEncoderKind>,
    columns: Vec<ColumnContract>,
    primary_key: Vec<String>,
    partition_columns: Vec<String>,
    zorder_columns: Vec<String>,
    durable_mutation: DurableMutationClass,
    overlay_mutation: OverlayMutationPolicy,
    materialization_role: MaterializationRole,
    dependencies: Vec<i16>,
    required_for_publication: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TableScopeContract {
    table_code: i16,
    #[serde(default)]
    workspace_column: Option<String>,
    #[serde(default)]
    analysis_context_column: Option<String>,
    #[serde(default)]
    source_generation_column: Option<String>,
    #[serde(default)]
    analysis_context_set_column: Option<String>,
    #[serde(default)]
    owner_column: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ServingProjectionRole {
    EffectiveFact,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ServingProjectionContract {
    view_name: String,
    source_table_code: i16,
    availability_wave: u16,
    projection_role: ServingProjectionRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ControlProjectionRole {
    OperationalSource,
    DerivedOperational,
    ActiveServingSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlProjectionContract {
    view_name: String,
    availability_wave: u16,
    projection_role: ControlProjectionRole,
    #[serde(default)]
    source_table: Option<String>,
    #[serde(default)]
    columns: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SqliteType {
    Integer,
    Real,
    Text,
    Blob,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationalColumnContract {
    name: String,
    sqlite_type: SqliteType,
    nullable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
enum OperationalWorkspaceScopeContract {
    Direct {
        workspace_column: String,
    },
    ViaParent {
        parent_table: String,
        child_column: String,
        parent_column: String,
        workspace_column: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationalTableContract {
    name: String,
    columns: Vec<OperationalColumnContract>,
    primary_key: Vec<String>,
    #[serde(default)]
    unique: Vec<Vec<String>>,
    #[serde(default)]
    workspace_scope: Option<OperationalWorkspaceScopeContract>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ServingResourceProfileContract {
    batch_size: usize,
    max_output_rows: usize,
    max_output_bytes: usize,
    max_output_batches: usize,
    max_control_rows: usize,
    max_control_bytes: usize,
    max_control_batches: usize,
    max_snapshot_validation_rows: usize,
    max_snapshot_validation_bytes: usize,
    max_snapshot_validation_batches: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicSchemaContract {
    schema_kind: String,
    artifact_id: String,
    path: String,
    title: String,
    /// JSON Schema is validated by the independent pinned Draft 2020-12 implementation.
    schema: Value,
}

/// Closed physical types allowed on provider-to-daemon Arrow observation streams.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProviderObservationLogicalType {
    Utf8,
    Binary,
    Boolean,
    #[serde(rename = "uint64")]
    UInt64,
    Utf8List,
}

/// One required field in a provider observation Arrow schema.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderObservationFieldContract {
    name: String,
    logical_type: ProviderObservationLogicalType,
    nullable: bool,
}

/// One provider-native Arrow schema admitted before canonical reconciliation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderObservationSchemaContract {
    schema_id: String,
    provider_id: String,
    observation_family_code: u16,
    fields: Vec<ProviderObservationFieldContract>,
}

/// Single typed source for every schema-family projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaContractIr {
    #[serde(flatten)]
    header: AuthorityHeader,
    schema_version: u16,
    ontology_version: String,
    compatibility_mode: String,
    metadata_dictionary: Vec<MetadataAnnotationContract>,
    semantic_authorities: Vec<SemanticAuthorityContract>,
    semantic_type_bindings: Vec<SemanticTypeBindingContract>,
    schema_evolution_policy: SchemaEvolutionPolicyContract,
    sqlite_foreign_key_posture: SqliteForeignKeyPosture,
    owner_bucket_count: u16,
    tables: Vec<TableContract>,
    table_scopes: Vec<TableScopeContract>,
    operational_tables: Vec<OperationalTableContract>,
    serving_projections: Vec<ServingProjectionContract>,
    control_projections: Vec<ControlProjectionContract>,
    serving_resource_profile: ServingResourceProfileContract,
    provider_observation_schemas: Vec<ProviderObservationSchemaContract>,
    public_schemas: Vec<PublicSchemaContract>,
}

impl SchemaContractIr {
    #[allow(clippy::too_many_lines)] // One pass keeps cross-projection diagnostics path-stable.
    fn validate(&self) -> Result<(), SchemaDriverError> {
        if self.header.artifact_id != "codefabric.schema.contract-ir"
            || self.header.artifact_kind != "manifest"
            || self.header.compatible_suite_major != 1
            || self.header.status != "released"
            || self.schema_version != 1
            || self.owner_bucket_count != 256
        {
            return invalid("$", "invalid schema Contract IR header or version");
        }
        if self.tables.is_empty() {
            return invalid("$.tables", "at least one TableSpec is required");
        }
        let expected_annotations = BTreeSet::from([
            "compatibility_mode",
            "dependencies",
            "durable_mutation",
            "family",
            "foreign_key",
            "grain",
            "hidden_operational",
            "integrity",
            "logical_type",
            "materialization_role",
            "name",
            "nullable",
            "ontology_version",
            "overlay_mutation",
            "partition_columns",
            "primary_key",
            "publication_pin_role",
            "required_for_publication",
            "row_encoder",
            "schema_version",
            "semantic_type",
            "sqlite_foreign_key_posture",
            "table_code",
            "zorder_columns",
        ]);
        let mut annotation_names = BTreeSet::new();
        let mut metadata_classes = BTreeSet::new();
        for (index, annotation) in self.metadata_dictionary.iter().enumerate() {
            if !identifier(&annotation.annotation)
                || !annotation_names.insert(annotation.annotation.as_str())
            {
                return invalid(
                    &format!("$.metadata_dictionary[{index}].annotation"),
                    "duplicate or invalid annotation name",
                );
            }
            metadata_classes.insert(annotation.class);
            let has_consumer = annotation
                .consumer_path
                .as_deref()
                .is_some_and(|path| path.starts_with("src/") && path.ends_with(".rs"))
                && annotation
                    .consumer_symbol
                    .as_deref()
                    .is_some_and(|symbol| !symbol.trim().is_empty());
            if (annotation.class == MetadataClass::Advisory) == has_consumer {
                return invalid(
                    &format!("$.metadata_dictionary[{index}]"),
                    "advisory annotations omit consumers; every other class names one",
                );
            }
        }
        if annotation_names != expected_annotations
            || metadata_classes
                != BTreeSet::from([
                    MetadataClass::Enforced,
                    MetadataClass::PlannerConsumed,
                    MetadataClass::Contractual,
                    MetadataClass::Governance,
                    MetadataClass::Lineage,
                    MetadataClass::Advisory,
                ])
        {
            return invalid(
                "$.metadata_dictionary",
                "annotation census or six-class coverage differs",
            );
        }
        if self.schema_evolution_policy.policy_id != "codefabric.schema.evolution-policy"
            || self.schema_evolution_policy.version != "1.0"
            || self.schema_evolution_policy.compatibility_class
                != SchemaCompatibilityClass::ExactPin
            || !self.schema_evolution_policy.require_schema_digest_equality
            || self.schema_evolution_policy.allow_type_widening
            || self.schema_evolution_policy.column_mapping_mode != "none"
            || self.schema_evolution_policy.migration_route.len() < 4
            || self
                .schema_evolution_policy
                .migration_route
                .iter()
                .any(|step| step.trim().is_empty())
        {
            return invalid(
                "$.schema_evolution_policy",
                "exact-pin evolution and migration contract differs",
            );
        }
        if self.sqlite_foreign_key_posture != SqliteForeignKeyPosture::NotEmittedCrossStore {
            return invalid(
                "$.sqlite_foreign_key_posture",
                "cross-store foreign keys must not claim SQLite enforcement",
            );
        }
        let expected_authorities = BTreeMap::from([
            (SemanticAuthority::EnumRegistry, ENUM_REGISTRY_PATH),
            (SemanticAuthority::TypeAlgebra, TYPE_ALGEBRA_PATH),
            (
                SemanticAuthority::OntologyEntityRegistry,
                ENTITY_REGISTRY_PATH,
            ),
            (
                SemanticAuthority::OntologyRelationRegistry,
                RELATION_REGISTRY_PATH,
            ),
            (
                SemanticAuthority::OntologyPropertyRegistry,
                PROPERTY_REGISTRY_PATH,
            ),
            (SemanticAuthority::OntologyFactRegistry, FACT_REGISTRY_PATH),
            (
                SemanticAuthority::CapabilityRegistry,
                CAPABILITY_REGISTRY_PATH,
            ),
        ]);
        let mut authorities = BTreeMap::new();
        for (index, authority) in self.semantic_authorities.iter().enumerate() {
            if expected_authorities.get(&authority.authority).copied()
                != Some(authority.path.as_str())
                || !authority.canonical_digest.starts_with("b3:")
                || authority.canonical_digest.len() != 67
                || authority.artifact_id.trim().is_empty()
                || authorities.insert(authority.authority, authority).is_some()
            {
                return invalid(
                    &format!("$.semantic_authorities[{index}]"),
                    "semantic authority path, digest, or identity is invalid",
                );
            }
        }
        if authorities.len() != expected_authorities.len() {
            return invalid(
                "$.semantic_authorities",
                "external semantic-authority census differs",
            );
        }
        let mut codes = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut tables = BTreeMap::new();
        let mut semantic_types = BTreeSet::new();
        let mut encoders = BTreeMap::new();
        for (table_index, table) in self.tables.iter().enumerate() {
            let path = format!("$.tables[{table_index}]");
            if !identifier(&table.name)
                || !codes.insert(table.table_code)
                || !names.insert(table.name.as_str())
            {
                return invalid(&path, "duplicate or invalid table identity");
            }
            if table.primary_key.is_empty() {
                return invalid(&format!("{path}.primary_key"), "primary key is empty");
            }
            let mut fields = BTreeSet::new();
            for (column_index, column) in table.columns.iter().enumerate() {
                if !identifier(&column.name) || !fields.insert(column.name.as_str()) {
                    return invalid(
                        &format!("{path}.columns[{column_index}].name"),
                        "duplicate or invalid field name",
                    );
                }
                if let Some(semantic_type) = column.semantic_type.as_deref() {
                    semantic_types.insert(semantic_type);
                }
            }
            if let Some(encoder) = table.row_encoder
                && encoders.insert(encoder, table.table_code).is_some()
            {
                return invalid(&format!("{path}.row_encoder"), "duplicate row encoder kind");
            }
            for key in table
                .primary_key
                .iter()
                .chain(&table.partition_columns)
                .chain(&table.zorder_columns)
            {
                if !fields.contains(key.as_str()) {
                    return invalid(&path, format!("unknown field reference {key}"));
                }
            }
            tables.insert(table.name.as_str(), table);
        }
        let expected_encoders = BTreeMap::from([
            (RowEncoderKind::Owners, 8),
            (RowEncoderKind::CapabilityStatuses, 9),
            (RowEncoderKind::Entities, 100),
            (RowEncoderKind::Relations, 110),
            (RowEncoderKind::Properties, 120),
            (RowEncoderKind::Evidence, 130),
            (RowEncoderKind::SourceFiles, 140),
            (RowEncoderKind::SourceTokens, 150),
            (RowEncoderKind::SourceAnnotations, 160),
            (RowEncoderKind::SyntaxDetails, 170),
            (RowEncoderKind::TypeDetails, 180),
            (RowEncoderKind::TypeFactDetails, 190),
        ]);
        if encoders != expected_encoders {
            return invalid(
                "$.tables[*].row_encoder",
                "generated fact-row encoder census differs",
            );
        }
        let mut bindings = BTreeMap::new();
        for (index, binding) in self.semantic_type_bindings.iter().enumerate() {
            if binding.semantic_type.trim().is_empty()
                || bindings
                    .insert(binding.semantic_type.as_str(), binding)
                    .is_some()
            {
                return invalid(
                    &format!("$.semantic_type_bindings[{index}]"),
                    "duplicate or empty semantic-type binding",
                );
            }
            let external = expected_authorities.contains_key(&binding.authority);
            if external
                != binding.domain.as_deref().is_some_and(|domain| {
                    !domain.is_empty()
                        && domain.bytes().all(|byte| {
                            byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'
                        })
                })
            {
                return invalid(
                    &format!("$.semantic_type_bindings[{index}].domain"),
                    "external bindings require one UPPER_SNAKE domain",
                );
            }
            if binding.semantic_type.starts_with("enum:")
                != (binding.authority == SemanticAuthority::EnumRegistry)
            {
                return invalid(
                    &format!("$.semantic_type_bindings[{index}]"),
                    "only enum-registry bindings may use the enum namespace",
                );
            }
        }
        if bindings.keys().copied().collect::<BTreeSet<_>>() != semantic_types {
            return invalid(
                "$.semantic_type_bindings",
                "semantic-type bindings do not exactly cover schema fields",
            );
        }
        for (table_index, table) in self.tables.iter().enumerate() {
            for (column_index, column) in table.columns.iter().enumerate() {
                let Some(foreign_key) = column.foreign_key.as_deref() else {
                    continue;
                };
                let Some((foreign_table, foreign_column)) = foreign_key.split_once('.') else {
                    return invalid(
                        &format!("$.tables[{table_index}].columns[{column_index}].foreign_key"),
                        "foreign key must be <table>.<field>",
                    );
                };
                let Some(target) = tables.get(foreign_table).and_then(|target| {
                    target
                        .columns
                        .iter()
                        .find(|field| field.name == foreign_column)
                }) else {
                    return invalid(
                        &format!("$.tables[{table_index}].columns[{column_index}].foreign_key"),
                        format!("unresolved foreign key {foreign_key}"),
                    );
                };
                if target.logical_type != column.logical_type {
                    return invalid(
                        &format!("$.tables[{table_index}].columns[{column_index}].foreign_key"),
                        "foreign-key logical types differ",
                    );
                }
            }
            for dependency in &table.dependencies {
                if !codes.contains(dependency) {
                    return invalid(
                        &format!("$.tables[{table_index}].dependencies"),
                        format!("unknown table dependency {dependency}"),
                    );
                }
            }
        }
        let mut dependency_order = BTreeSet::new();
        while dependency_order.len() < self.tables.len() {
            let before = dependency_order.len();
            for table in &self.tables {
                if table
                    .dependencies
                    .iter()
                    .all(|dependency| dependency_order.contains(dependency))
                {
                    dependency_order.insert(table.table_code);
                }
            }
            if before == dependency_order.len() {
                return invalid("$.tables[*].dependencies", "table dependency cycle");
            }
        }
        let syntax = tables
            .get("syntax_detail")
            .ok_or_else(|| SchemaDriverError::Invalid {
                path: "$.tables".to_owned(),
                detail: "syntax_detail is absent".to_owned(),
            })?;
        for field in [
            "occurrence_family_code",
            "reconciliation_step_code",
            "raw_kind_disposition_code",
        ] {
            if syntax
                .columns
                .iter()
                .find(|column| column.name == field)
                .is_none_or(|column| column.logical_type != LogicalType::Code16 || column.nullable)
            {
                return invalid(
                    "$.tables[syntax_detail].columns",
                    format!("corrected field {field} must be required code16"),
                );
            }
        }
        let mut observation_schema_ids = BTreeSet::new();
        let mut observation_family_codes = BTreeSet::new();
        for (schema_index, schema) in self.provider_observation_schemas.iter().enumerate() {
            let path = format!("$.provider_observation_schemas[{schema_index}]");
            if schema.schema_id.is_empty()
                || !schema.schema_id.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-".contains(&byte)
                })
                || schema.provider_id.is_empty()
                || schema.observation_family_code == 0
                || schema.fields.is_empty()
                || !observation_schema_ids.insert(schema.schema_id.as_str())
                || !observation_family_codes.insert(schema.observation_family_code)
            {
                return invalid(&path, "invalid or duplicate provider observation schema");
            }
            let mut fields = BTreeSet::new();
            for (field_index, field) in schema.fields.iter().enumerate() {
                if !identifier(&field.name) || !fields.insert(field.name.as_str()) {
                    return invalid(
                        &format!("{path}.fields[{field_index}].name"),
                        "invalid or duplicate provider observation field",
                    );
                }
            }
        }
        let mut operational_names = BTreeSet::new();
        for (table_index, table) in self.operational_tables.iter().enumerate() {
            let path = format!("$.operational_tables[{table_index}]");
            if !identifier(&table.name) || !operational_names.insert(table.name.as_str()) {
                return invalid(&path, "duplicate or invalid SQLite table identity");
            }
            let fields = table
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<BTreeSet<_>>();
            if fields.len() != table.columns.len() || table.primary_key.is_empty() {
                return invalid(&path, "duplicate field or empty SQLite primary key");
            }
            for key in table
                .primary_key
                .iter()
                .chain(table.unique.iter().flatten())
            {
                if !fields.contains(key.as_str()) {
                    return invalid(&path, format!("unknown SQLite key field {key}"));
                }
            }
        }
        let operational_tables = self
            .operational_tables
            .iter()
            .map(|table| (table.name.as_str(), table))
            .collect::<BTreeMap<_, _>>();
        let mut control_views = BTreeSet::new();
        for (projection_index, projection) in self.control_projections.iter().enumerate() {
            let path = format!("$.control_projections[{projection_index}]");
            if !identifier(&projection.view_name)
                || !control_views.insert(projection.view_name.as_str())
                || projection.availability_wave == 0
            {
                return invalid(&path, "duplicate or invalid control projection identity");
            }
            match projection.projection_role {
                ControlProjectionRole::OperationalSource => {
                    if projection.source_table.as_deref() != Some(projection.view_name.as_str())
                        || !projection.columns.is_empty()
                        || !operational_tables.contains_key(projection.view_name.as_str())
                    {
                        return invalid(
                            &path,
                            "operational-source projection must name its governed table",
                        );
                    }
                }
                ControlProjectionRole::DerivedOperational => {
                    let source_name = projection.source_table.as_deref().ok_or_else(|| {
                        SchemaDriverError::Invalid {
                            path: format!("{path}.source_table"),
                            detail: "derived operational projection source is absent".into(),
                        }
                    })?;
                    let source = operational_tables.get(source_name).ok_or_else(|| {
                        SchemaDriverError::Invalid {
                            path: format!("{path}.source_table"),
                            detail: "derived operational projection source is unknown".into(),
                        }
                    })?;
                    let source_columns = source
                        .columns
                        .iter()
                        .map(|column| column.name.as_str())
                        .collect::<BTreeSet<_>>();
                    let selected = projection
                        .columns
                        .iter()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>();
                    if selected.len() != projection.columns.len()
                        || selected.is_empty()
                        || selected
                            .iter()
                            .any(|column| !identifier(column) || !source_columns.contains(column))
                    {
                        return invalid(
                            &format!("{path}.columns"),
                            "derived operational projection columns are invalid",
                        );
                    }
                }
                ControlProjectionRole::ActiveServingSnapshot => {
                    if projection.view_name != "active_serving_snapshot"
                        || projection.source_table.is_some()
                        || !projection.columns.is_empty()
                    {
                        return invalid(
                            &path,
                            "active serving snapshot is a runtime pinned-session projection",
                        );
                    }
                }
            }
        }
        let expected_kinds = BTreeSet::from([
            "analysis-context",
            "serving-snapshot",
            "public-snapshot-metadata",
            "source-context",
            "public-status",
            "cpg-semantic-query-request",
            "cpg-semantic-query-response",
            "plan-spec",
        ]);
        let mut kinds = BTreeSet::new();
        let mut artifact_ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for (index, schema) in self.public_schemas.iter().enumerate() {
            let path = format!("$.public_schemas[{index}]");
            if !kinds.insert(schema.schema_kind.as_str())
                || !artifact_ids.insert(schema.artifact_id.as_str())
                || !paths.insert(schema.path.as_str())
                || !schema.path.starts_with("contracts/")
                || !schema.path.ends_with(".schema.json")
                || schema.schema.as_object().is_none()
            {
                return invalid(&path, "invalid or duplicate public schema declaration");
            }
            let body = schema.schema.as_object().expect("checked schema body");
            if [
                "$schema",
                "$id",
                "title",
                "x-codefabric-generated",
                "x-codefabric-artifact",
            ]
            .iter()
            .any(|reserved| body.contains_key(*reserved))
            {
                return invalid(&format!("{path}.schema"), "body contains a derived header");
            }
        }
        if kinds != expected_kinds {
            return invalid("$.public_schemas", "public schema kind census differs");
        }
        let resources = self.serving_resource_profile;
        if [
            resources.batch_size,
            resources.max_output_rows,
            resources.max_output_bytes,
            resources.max_output_batches,
            resources.max_control_rows,
            resources.max_control_bytes,
            resources.max_control_batches,
            resources.max_snapshot_validation_rows,
            resources.max_snapshot_validation_bytes,
            resources.max_snapshot_validation_batches,
        ]
        .contains(&0)
        {
            return invalid(
                "$.serving_resource_profile",
                "resource limits must be positive",
            );
        }
        Ok(())
    }
}

fn validate_semantic_authorities(
    repository_root: &Path,
    ir: &SchemaContractIr,
) -> Result<(), SchemaDriverError> {
    let mut enum_domains = BTreeSet::new();
    for authority in &ir.semantic_authorities {
        let path = repository_root.join(&authority.path);
        let bytes = read_stable(&path, MAX_AUTHORITY_BYTES)?;
        let yaml: serde_yaml_ng::Value =
            serde_yaml_ng::from_slice(&bytes).map_err(|source| SchemaDriverError::Io {
                path: path.clone(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
            })?;
        let value = serde_json::to_value(yaml)?;
        let actual_artifact = value.get("artifact_id").and_then(Value::as_str);
        let actual_digest = value.get("canonical_digest").and_then(Value::as_str);
        let detached =
            super::registry_cbef_driver::detached_registry_identity(&authority.artifact_id, &bytes)
                .map_err(|error| SchemaDriverError::Invalid {
                    path: authority.path.clone(),
                    detail: error.to_string(),
                })?;
        if actual_artifact != Some(authority.artifact_id.as_str())
            || actual_digest != Some(authority.canonical_digest.as_str())
            || detached.as_deref() != Some(authority.canonical_digest.as_str())
        {
            return invalid(
                "$.semantic_authorities",
                format!(
                    "digest-pinned semantic authority {} drifted: header={actual_digest:?}, expected={:?}, detached={detached:?}",
                    authority.path, authority.canonical_digest,
                ),
            );
        }
        if authority.authority == SemanticAuthority::EnumRegistry {
            enum_domains.extend(
                value["records"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|record| record["domain"].as_str().map(str::to_owned)),
            );
        } else if authority.authority == SemanticAuthority::TypeAlgebra {
            if value["constructors"].as_array().is_none_or(Vec::is_empty) {
                return invalid(
                    "$.semantic_authorities",
                    format!("semantic authority {} has no constructors", authority.path),
                );
            }
        } else if value["records"].as_array().is_none_or(Vec::is_empty) {
            return invalid(
                "$.semantic_authorities",
                format!("semantic authority {} has no records", authority.path),
            );
        }
    }
    for binding in &ir.semantic_type_bindings {
        let valid_domain = match binding.authority {
            SemanticAuthority::EnumRegistry => binding
                .domain
                .as_deref()
                .is_some_and(|domain| enum_domains.contains(domain)),
            SemanticAuthority::TypeAlgebra => binding.domain.as_deref() == Some("TYPE_CONSTRUCTOR"),
            SemanticAuthority::OntologyEntityRegistry => {
                matches!(
                    binding.domain.as_deref(),
                    Some("ENTITY_KIND" | "ENTITY_FAMILY")
                )
            }
            SemanticAuthority::OntologyRelationRegistry => matches!(
                binding.domain.as_deref(),
                Some("RELATION_KIND" | "RELATION_FAMILY")
            ),
            SemanticAuthority::OntologyPropertyRegistry => {
                binding.domain.as_deref() == Some("PROPERTY_KIND")
            }
            SemanticAuthority::OntologyFactRegistry => {
                binding.domain.as_deref() == Some("FACT_KIND")
            }
            SemanticAuthority::CapabilityRegistry => {
                binding.domain.as_deref() == Some("CAPABILITY")
            }
            SemanticAuthority::SchemaIr
            | SemanticAuthority::Intrinsic
            | SemanticAuthority::ProviderCatalog
            | SemanticAuthority::DiagnosticProtocol => binding.domain.is_none(),
        };
        if !valid_domain {
            return invalid(
                "$.semantic_type_bindings",
                format!(
                    "semantic type {} does not resolve in its authority",
                    binding.semantic_type
                ),
            );
        }
    }
    Ok(())
}

/// Resolved, source-fenced schema plan.
pub struct SchemaPlan {
    descriptor: DriverDescriptor,
    ir: SchemaContractIr,
    semantic_fragments: super::semantic_fragment_driver::SemanticFragmentSet,
    query_forms: QueryFormContract,
    query_form_bytes: Vec<u8>,
    public_schema_instances: Value,
    source_digest: String,
    query_form_source_digest: String,
    semantic_digest: String,
    source_fence: DriverSourceFence,
}

/// Pure schema family driver.
pub struct SchemaDriver;

impl SchemaDriver {
    fn output(
        id: impl Into<String>,
        path: &str,
        role: DriverOutputRole,
    ) -> Result<DriverOutputSpec, DriverProtocolError> {
        Ok(DriverOutputSpec {
            output_id: StableId::parse(id.into())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            path: SafeOutputPath::parse(path.as_bytes().to_vec())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            role,
        })
    }

    fn outputs(plan: &SchemaPlan) -> Result<Vec<(SafeOutputPath, Vec<u8>)>, SchemaDriverError> {
        let mut outputs = vec![
            (safe(TABLE_MANIFEST_PATH)?, render_table_manifest(plan)?),
            (safe(DDL_PATH)?, render_ddl(plan)),
            (
                safe(RUST_BINDINGS_PATH)?,
                rustfmt_source(&render_rust(&plan.ir))?,
            ),
            (
                safe(RUST_RUNTIME_BINDINGS_PATH)?,
                rustfmt_source(&render_runtime_rust(&plan.ir, &plan.source_digest))?,
            ),
            (
                safe(RUST_ROW_ENCODERS_PATH)?,
                rustfmt_source(&render_row_encoders(&plan.ir, &plan.source_digest)?)?,
            ),
            (safe(VALIDATION_PATH)?, render_validation(plan)?),
            (safe(EVOLUTION_POLICY_PATH)?, render_evolution_policy(plan)?),
            (
                safe(PUBLIC_SCHEMA_INSTANCES_PATH)?,
                pretty(&plan.public_schema_instances)?,
            ),
            (
                safe(RUST_QUERY_FORM_BINDINGS_PATH)?,
                rustfmt_source(render_query_form_rust(&plan.query_forms).as_bytes())?,
            ),
            (
                safe(PYTHON_QUERY_FORM_BINDINGS_PATH)?,
                render_query_form_python(&plan.query_forms).into_bytes(),
            ),
            (
                safe(PYTHON_QUERY_FORM_CONTRACT_PATH)?,
                plan.query_form_bytes.clone(),
            ),
            (
                safe(super::semantic_fragment_driver::JSON_PROJECTION_PATH)?,
                plan.semantic_fragments.render_json()?,
            ),
            (
                safe(super::semantic_fragment_driver::RUST_PROJECTION_PATH)?,
                rustfmt_source(plan.semantic_fragments.render_rust().as_bytes())?,
            ),
        ];
        for schema in &plan.ir.public_schemas {
            outputs.push((
                safe(&schema.path)?,
                render_public_schema(
                    schema,
                    &plan.source_digest,
                    &plan.query_forms,
                    &plan.query_form_source_digest,
                )?,
            ));
        }
        outputs.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(outputs)
    }
}

impl ModelDriver for SchemaDriver {
    type Plan = SchemaPlan;

    fn describe(&self) -> Result<DriverDescriptor, DriverProtocolError> {
        let outputs = vec![
            Self::output(
                "output:model-schema-tables",
                TABLE_MANIFEST_PATH,
                DriverOutputRole::TableSpec,
            )?,
            Self::output(
                "output:model-schema-ddl",
                DDL_PATH,
                DriverOutputRole::SqliteDdl,
            )?,
            Self::output(
                "output:model-schema-rust",
                RUST_BINDINGS_PATH,
                DriverOutputRole::RustBinding,
            )?,
            Self::output(
                "output:model-schema-runtime-rust",
                RUST_RUNTIME_BINDINGS_PATH,
                DriverOutputRole::RustBinding,
            )?,
            Self::output(
                "output:model-schema-row-encoders-rust",
                RUST_ROW_ENCODERS_PATH,
                DriverOutputRole::RustBinding,
            )?,
            Self::output(
                "output:model-schema-validation",
                VALIDATION_PATH,
                DriverOutputRole::ValidationReport,
            )?,
            Self::output(
                "output:model-schema-evolution-policy",
                EVOLUTION_POLICY_PATH,
                DriverOutputRole::CanonicalProjection,
            )?,
            Self::output(
                "output:model-schema-public-golden-instances",
                PUBLIC_SCHEMA_INSTANCES_PATH,
                DriverOutputRole::CanonicalProjection,
            )?,
            Self::output(
                "output:model-query-form-rust",
                RUST_QUERY_FORM_BINDINGS_PATH,
                DriverOutputRole::RustBinding,
            )?,
            Self::output(
                "output:model-query-form-python",
                PYTHON_QUERY_FORM_BINDINGS_PATH,
                DriverOutputRole::PythonBinding,
            )?,
            Self::output(
                "output:model-query-form-python-contract",
                PYTHON_QUERY_FORM_CONTRACT_PATH,
                DriverOutputRole::CanonicalProjection,
            )?,
            Self::output(
                "output:model-semantic-lane-fragments-json",
                super::semantic_fragment_driver::JSON_PROJECTION_PATH,
                DriverOutputRole::CanonicalProjection,
            )?,
            Self::output(
                "output:model-semantic-lane-fragments-rust",
                super::semantic_fragment_driver::RUST_PROJECTION_PATH,
                DriverOutputRole::RustBinding,
            )?,
        ];
        let descriptor = DriverDescriptor {
            driver_id: StableId::parse("driver:schema-contract-v1".to_owned())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            family: StableId::parse("family:schemas".to_owned())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            rule_version: "schema-contract-driver-v1".to_owned(),
            sources: [
                SCHEMA_IR_PATH,
                QUERY_FORM_CONTRACT_PATH,
                ENUM_REGISTRY_PATH,
                ENTITY_REGISTRY_PATH,
                RELATION_REGISTRY_PATH,
                PROPERTY_REGISTRY_PATH,
                FACT_REGISTRY_PATH,
                CAPABILITY_REGISTRY_PATH,
                PUBLIC_SCHEMA_INSTANCES_SOURCE_PATH,
                super::semantic_fragment_driver::FRAGMENT_PATHS[0],
                super::semantic_fragment_driver::FRAGMENT_PATHS[1],
                super::semantic_fragment_driver::FRAGMENT_PATHS[2],
            ]
            .into_iter()
            .map(safe_protocol)
            .collect::<Result<Vec<_>, _>>()?,
            output_roots: vec![
                safe_protocol("contracts/schema")?,
                safe_protocol("contracts/query")?,
            ],
            outputs,
            resource_profile: DriverResourceProfile {
                max_source_bytes: MAX_AUTHORITY_BYTES,
                max_output_bytes: 8 * 1024 * 1024,
                max_outputs: 24,
            },
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    fn plan(&self, repository_root: &Path) -> Result<Self::Plan, DriverProtocolError> {
        let mut descriptor = self.describe()?;
        let source_fence = DriverSourceFence::capture(repository_root, &descriptor)?;
        let bytes = read_stable(&repository_root.join(SCHEMA_IR_PATH), MAX_AUTHORITY_BYTES)?;
        let semantic_fragments =
            super::semantic_fragment_driver::SemanticFragmentSet::load(repository_root)
                .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let mut ir_value = codefabric::contracts::jcs::decode_strict(&bytes)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        semantic_fragments
            .compose_schema(&mut ir_value)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let ir: SchemaContractIr = serde_json::from_value(ir_value)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        ir.validate()
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        validate_semantic_authorities(repository_root, &ir)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let query_form_bytes = read_stable(
            &repository_root.join(QUERY_FORM_CONTRACT_PATH),
            MAX_AUTHORITY_BYTES,
        )?;
        let query_forms = decode_query_form_contract(&query_form_bytes)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let enum_registry_bytes = read_stable(
            &repository_root.join(ENUM_REGISTRY_PATH),
            MAX_AUTHORITY_BYTES,
        )?;
        let property_registry_bytes = read_stable(
            &repository_root.join(PROPERTY_REGISTRY_PATH),
            MAX_AUTHORITY_BYTES,
        )?;
        let mut enum_registry_value = serde_json::to_value(
            serde_yaml_ng::from_slice::<serde_yaml_ng::Value>(&enum_registry_bytes)
                .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?,
        )
        .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let mut property_registry_value = serde_json::to_value(
            serde_yaml_ng::from_slice::<serde_yaml_ng::Value>(&property_registry_bytes)
                .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?,
        )
        .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        semantic_fragments
            .compose_registries(&mut property_registry_value, &mut enum_registry_value)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        query_forms
            .validate(&query_form_bytes, &enum_registry_bytes)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let public_schema_instances: Value = serde_json::from_slice(&read_stable(
            &repository_root.join(PUBLIC_SCHEMA_INSTANCES_SOURCE_PATH),
            MAX_AUTHORITY_BYTES,
        )?)
        .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        for schema in &ir.public_schemas {
            let output = Self::output(
                format!("output:model-schema-{}", schema.schema_kind),
                &schema.path,
                DriverOutputRole::PublicJsonSchema,
            )?;
            if !descriptor.output_roots.iter().any(|root| {
                output.path.as_bytes().starts_with(root.as_bytes())
                    && output.path.as_bytes().get(root.as_bytes().len()) == Some(&b'/')
            }) {
                return Err(DriverProtocolError::InvalidAuthority(format!(
                    "public schema output escapes declared roots: {}",
                    output.path.display()
                )));
            }
            descriptor.outputs.push(output);
        }
        descriptor.validate()?;
        let base_semantic_digest = detached_schema_identity(&bytes)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let base_source_digest = codefabric::integrity::framed_digest(&bytes);
        let semantic_digest = semantic_fragments
            .composed_source_digest(&base_semantic_digest)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let source_digest = semantic_fragments
            .composed_source_digest(&base_source_digest)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        Ok(SchemaPlan {
            descriptor,
            ir,
            semantic_fragments,
            query_forms,
            query_form_source_digest: codefabric::integrity::framed_digest(&query_form_bytes),
            query_form_bytes,
            public_schema_instances,
            source_digest,
            semantic_digest,
            source_fence,
        })
    }

    fn render(
        &self,
        plan: &Self::Plan,
        staging_root: &StagingRoot,
    ) -> Result<Vec<SafeOutputPath>, DriverProtocolError> {
        let outputs = Self::outputs(plan)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let mut rendered = Vec::with_capacity(outputs.len());
        for (path, bytes) in outputs {
            staging_root.write(&path, &bytes)?;
            rendered.push(path);
        }
        Ok(rendered)
    }
}

/// Render and internally cross-check the schema family under a disposable stage.
///
/// # Errors
///
/// Returns typed ingress, validation, staging, or projection failures.
pub fn check_family(repository_root: &Path) -> Result<SchemaReport, SchemaDriverError> {
    let driver = SchemaDriver;
    let plan = driver.plan(repository_root)?;
    let stage_path = process_stage_root(repository_root, "schemas-stage");
    if stage_path.exists() {
        fs::remove_dir_all(&stage_path).map_err(|source| SchemaDriverError::Io {
            path: stage_path.clone(),
            source,
        })?;
    }
    fs::create_dir_all(&stage_path).map_err(|source| SchemaDriverError::Io {
        path: stage_path.clone(),
        source,
    })?;
    let staging = StagingRoot::new(repository_root, &stage_path, &plan.descriptor)?;
    let (rendered, cache_lookup, action_key) = render_with_cache(
        repository_root,
        "schemas",
        &plan.descriptor,
        &plan.source_fence,
        &staging,
        || executable_tool_identity("rustfmt", &["--version"]),
        || driver.render(&plan, &staging),
    )?;
    plan.source_fence.verify(repository_root)?;
    let manifest: Value = serde_json::from_slice(&read_stable(
        &stage_path.join(TABLE_MANIFEST_PATH),
        MAX_AUTHORITY_BYTES,
    )?)?;
    if manifest["tables"].as_array().map(Vec::len) != Some(plan.ir.tables.len())
        || manifest["operational_tables"].as_array().map(Vec::len)
            != Some(plan.ir.operational_tables.len())
    {
        return Err(SchemaDriverError::ProjectionMismatch);
    }
    let syntax_fields = plan
        .ir
        .tables
        .iter()
        .find(|table| table.name == "syntax_detail")
        .ok_or(SchemaDriverError::ProjectionMismatch)?
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect();
    Ok(SchemaReport {
        family: "schemas".to_owned(),
        action_key,
        rule_version: plan.descriptor.rule_version.clone(),
        resource_profile: plan.descriptor.resource_profile.clone(),
        table_count: plan.ir.tables.len(),
        operational_table_count: plan.ir.operational_tables.len(),
        public_schema_count: plan.ir.public_schemas.len(),
        rendered_outputs: rendered.iter().map(SafeOutputPath::display).collect(),
        cache_lookup,
        syntax_detail_fields: syntax_fields,
        stage_root: staging.path().to_string_lossy().into_owned(),
    })
}

/// Machine-readable family result consumed by the command contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaReport {
    pub family: String,
    pub action_key: String,
    pub rule_version: String,
    pub resource_profile: DriverResourceProfile,
    pub table_count: usize,
    pub operational_table_count: usize,
    pub public_schema_count: usize,
    pub rendered_outputs: Vec<String>,
    pub cache_lookup: CacheLookup,
    pub syntax_detail_fields: Vec<String>,
    pub stage_root: String,
}

fn render_table_manifest(plan: &SchemaPlan) -> Result<Vec<u8>, SchemaDriverError> {
    let tables = plan
        .ir
        .tables
        .iter()
        .map(|table| {
            let columns = table.columns.iter().map(|column| json!({
                "field_id": format!("{}.{}", table.name, column.name),
                "name": column.name,
                "logical_type": column.logical_type,
                "arrow_type": arrow_type(column.logical_type),
                "nullable": column.nullable,
                "semantic_type": column.semantic_type,
                "foreign_key": column.foreign_key,
                "hidden_operational": column.hidden_operational,
                "key_role": if table.primary_key.contains(&column.name) { "primary" } else { "none" },
            })).collect::<Vec<_>>();
            json!({
                "table_id": format!("table:{}", table.name),
                "table_code": table.table_code,
                "name": table.name,
                "family": table.family,
                "grain": table.grain,
                "schema_version": table.schema_version,
                "row_encoder": table.row_encoder,
                "columns": columns,
                "primary_key": table.primary_key,
                "partition_columns": table.partition_columns,
                "zorder_columns": table.zorder_columns,
                "durable_mutation": table.durable_mutation,
                "overlay_mutation": table.overlay_mutation,
                "materialization_role": table.materialization_role,
                "publication_pin_role": table.publication_pin_role,
                "dependencies": table.dependencies,
                "required_for_publication": table.required_for_publication,
            })
        })
        .collect::<Vec<_>>();
    pretty(&json!({
        "model_version": 1,
        "source": {"artifact_id": plan.ir.header.artifact_id, "source_digest": plan.source_digest},
        "ontology_version": plan.ir.ontology_version,
        "compatibility_mode": plan.ir.compatibility_mode,
        "metadata_dictionary": plan.ir.metadata_dictionary,
        "semantic_authorities": plan.ir.semantic_authorities,
        "semantic_type_bindings": plan.ir.semantic_type_bindings,
        "schema_evolution_policy": plan.ir.schema_evolution_policy,
        "sqlite_foreign_key_posture": plan.ir.sqlite_foreign_key_posture,
        "owner_bucket_count": plan.ir.owner_bucket_count,
        "tables": tables,
        "table_scopes": plan.ir.table_scopes,
        "operational_tables": plan.ir.operational_tables,
        "serving_projections": plan.ir.serving_projections,
        "control_projections": plan.ir.control_projections,
        "serving_resource_profile": plan.ir.serving_resource_profile,
        "public_schema_instances": PUBLIC_SCHEMA_INSTANCES_PATH,
        "public_schemas": plan.ir.public_schemas.iter().map(|schema| json!({
            "schema_kind": schema.schema_kind,
            "artifact_id": schema.artifact_id,
            "path": schema.path,
            "title": schema.title,
        })).collect::<Vec<_>>(),
    }))
}

fn query_reference_schema() -> Value {
    json!({
        "oneOf": [
            {"type": "string", "minLength": 1, "maxLength": 4096},
            {"$ref": "#/$defs/prior_result_reference"},
            {"type": "object", "additionalProperties": false, "required": ["entity_id"], "properties": {"entity_id": {"type": "string", "minLength": 1}}},
            {"type": "object", "additionalProperties": false, "required": ["fact_id"], "properties": {"fact_id": {"type": "string", "minLength": 1}}}
        ]
    })
}

fn rust_query_field_type(field: &QueryFormFieldContract) -> String {
    let base = match field.field_type {
        QueryFieldType::BoundedString => "String",
        QueryFieldType::BoundedStringList => "Vec<String>",
        QueryFieldType::SemanticReferenceList => "Vec<SemanticReference>",
        QueryFieldType::PriorResultList => "Vec<PriorResultReference>",
        QueryFieldType::PatternBindingList => "Vec<PatternBinding>",
        QueryFieldType::PatternRelationshipList => "Vec<PatternRelationship>",
        QueryFieldType::PositiveInteger => "usize",
        QueryFieldType::ReturnSpec => "ReturnSpec",
    };
    if field.required {
        base.to_owned()
    } else if matches!(
        field.field_type,
        QueryFieldType::BoundedStringList
            | QueryFieldType::SemanticReferenceList
            | QueryFieldType::PriorResultList
            | QueryFieldType::PatternBindingList
            | QueryFieldType::PatternRelationshipList
    ) {
        base.to_owned()
    } else {
        format!("Option<{base}>")
    }
}

fn render_query_form_rust(contract: &QueryFormContract) -> String {
    let mut output = format!(
        "// @generated by codefabric-model from {} {} ({}); do not edit.\n\
         #![allow(clippy::match_same_arms)]\n\
         use serde::{{Deserialize, Serialize}};\n\
         use crate::registries::QueryForm;\n\n\
         pub const QUERY_FORM_CONTRACT_ID: &str = {:?};\n\
         pub const QUERY_FORM_CONTRACT_VERSION: &str = {:?};\n\
         pub const QUERY_FORM_CONTRACT_DIGEST: &str = {:?};\n\n",
        contract.artifact_id,
        contract.version,
        contract.canonical_digest,
        contract.artifact_id,
        contract.version,
        contract.canonical_digest,
    );
    output.push_str(
        "#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]\n\
         #[serde(rename_all = \"snake_case\")]\n\
         pub enum ResultRole { Entities, Facts, Paths, PatternBindings, Groups, Summary, SourceContexts }\n\n\
         #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]\n\
         #[serde(deny_unknown_fields)]\n\
         pub struct PriorResultReference { pub results_of: String, pub select: ResultRole }\n\n\
         #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]\n\
         #[serde(untagged)]\n\
         pub enum SemanticReference {\n\
             Phrase(String),\n\
             PriorResult(PriorResultReference),\n\
             Entity { entity_id: String },\n\
             Fact { fact_id: String },\n\
         }\n\n\
         #[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]\n\
         #[serde(deny_unknown_fields)]\n\
         pub struct ReturnLimit { pub maximum_results: usize, pub per: Option<String>, pub when_exceeded: Option<String> }\n\n\
         #[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]\n\
         #[serde(deny_unknown_fields)]\n\
         pub struct ReturnSpec {\n\
             #[serde(default)] pub include: Vec<String>,\n\
             #[serde(default)] pub exclude: Vec<String>,\n\
             pub result_shape: Option<String>,\n\
             #[serde(default)] pub group_by: Vec<String>,\n\
             #[serde(default)] pub order_by: Vec<String>,\n\
             pub deduplicate_by: Option<String>,\n\
             pub supporting_facts: Option<String>,\n\
             pub include_query_result: Option<bool>,\n\
             pub limit: Option<ReturnLimit>,\n\
         }\n\n\
         #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]\n\
         #[serde(deny_unknown_fields)]\n\
         pub struct PatternBinding {\n\
             pub name: String, pub looking_for: String, pub within: Option<SemanticReference>,\n\
             #[serde(default, rename = \"where\")] pub where_conditions: Vec<String>,\n\
         }\n\n\
         #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]\n\
         #[serde(deny_unknown_fields)]\n\
         pub struct PatternRelationship {\n\
             pub from: String, pub to: String, pub relationship: String,\n\
             pub direction: Option<String>, pub distance: Option<String>,\n\
         }\n\n\
         #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]\n\
         #[serde(tag = \"request\", deny_unknown_fields)]\n\
         pub enum SemanticQueryClause {\n",
    );
    for form in &contract.forms {
        writeln!(output, "    #[serde(rename = {:?})]", form.slug).unwrap();
        writeln!(output, "    {} {{", form.rust_variant).unwrap();
        output.push_str("        query_id: String,\n        label: Option<String>,\n");
        for field in &form.fields {
            if field.name != field.rust_name {
                writeln!(output, "        #[serde(rename = {:?})]", field.name).unwrap();
            }
            if !field.required
                && matches!(
                    field.field_type,
                    QueryFieldType::BoundedStringList
                        | QueryFieldType::SemanticReferenceList
                        | QueryFieldType::PriorResultList
                        | QueryFieldType::PatternBindingList
                        | QueryFieldType::PatternRelationshipList
                )
            {
                output.push_str("        #[serde(default)]\n");
            }
            writeln!(
                output,
                "        {}: {},",
                field.rust_name,
                rust_query_field_type(field)
            )
            .unwrap();
        }
        output.push_str("    },\n");
    }
    output.push_str("}\n\nimpl SemanticQueryClause {\n");
    output.push_str("    #[must_use]\n    pub fn query_id(&self) -> &str { match self {\n");
    for form in &contract.forms {
        writeln!(
            output,
            "        Self::{} {{ query_id, .. }} => query_id,",
            form.rust_variant
        )
        .unwrap();
    }
    output.push_str(
        "    } }\n    #[must_use]\n    pub const fn form(&self) -> QueryForm { match self {\n",
    );
    for form in &contract.forms {
        writeln!(
            output,
            "        Self::{} {{ .. }} => QueryForm::{},",
            form.rust_variant, form.rust_variant
        )
        .unwrap();
    }
    output.push_str(
        "    } }\n    #[must_use]\n    pub fn label(&self) -> Option<&str> { match self {\n",
    );
    for form in &contract.forms {
        writeln!(
            output,
            "        Self::{} {{ label, .. }} => label.as_deref(),",
            form.rust_variant
        )
        .unwrap();
    }
    output.push_str("    } }\n    #[must_use]\n    pub const fn output_role(&self) -> ResultRole { match self {\n");
    for form in &contract.forms {
        writeln!(
            output,
            "        Self::{} {{ .. }} => ResultRole::{},",
            form.rust_variant,
            upper_camel(&form.output_role)
        )
        .unwrap();
    }
    output.push_str(
        "    } }\n    #[must_use]\n    pub fn maximum_results(&self) -> usize {\n        let spec = match self {\n",
    );
    for form in &contract.forms {
        let field = form
            .fields
            .iter()
            .find(|field| field.field_type == QueryFieldType::ReturnSpec);
        if field.is_some() {
            writeln!(
                output,
                "            Self::{} {{ return_spec, .. }} => return_spec.as_ref(),",
                form.rust_variant
            )
            .unwrap();
        } else {
            writeln!(
                output,
                "            Self::{} {{ .. }} => None,",
                form.rust_variant
            )
            .unwrap();
        }
    }
    output.push_str("        }; spec.and_then(|value| value.limit.as_ref()).map_or(100, |limit| limit.maximum_results)\n    }\n");
    output.push_str("    #[must_use]\n    pub fn result_references(&self) -> Vec<&PriorResultReference> { let mut result = Vec::new(); match self {\n");
    for form in &contract.forms {
        let fields = form
            .fields
            .iter()
            .filter(|field| {
                matches!(
                    field.field_type,
                    QueryFieldType::SemanticReferenceList
                        | QueryFieldType::PriorResultList
                        | QueryFieldType::PatternBindingList
                )
            })
            .collect::<Vec<_>>();
        if fields.is_empty() {
            writeln!(
                output,
                "            Self::{} {{ .. }} => {{}},",
                form.rust_variant
            )
            .unwrap();
            continue;
        }
        write!(output, "            Self::{} {{ ", form.rust_variant).unwrap();
        for field in &fields {
            write!(output, "{}, ", field.rust_name).unwrap();
        }
        output.push_str(".. } => {\n");
        for field in fields {
            match field.field_type {
                QueryFieldType::SemanticReferenceList => writeln!(output, "                for value in {0} {{ if let SemanticReference::PriorResult(reference) = value {{ result.push(reference); }} }}", field.rust_name).unwrap(),
                QueryFieldType::PriorResultList => writeln!(output, "                result.extend({}.iter());", field.rust_name).unwrap(),
                QueryFieldType::PatternBindingList => writeln!(output, "                for binding in {0} {{ if let Some(SemanticReference::PriorResult(reference)) = &binding.within {{ result.push(reference); }} }}", field.rust_name).unwrap(),
                _ => unreachable!(),
            }
        }
        output.push_str("            },\n");
    }
    output.push_str("        } result }\n");
    output.push_str("    #[must_use]\n    pub fn direct_entity_ids(&self) -> Vec<&str> { self.semantic_references().into_iter().filter_map(|value| if let SemanticReference::Entity { entity_id } = value { Some(entity_id.as_str()) } else { None }).collect() }\n");
    output.push_str("    #[must_use]\n    pub fn direct_fact_ids(&self) -> Vec<&str> { self.semantic_references().into_iter().filter_map(|value| if let SemanticReference::Fact { fact_id } = value { Some(fact_id.as_str()) } else { None }).collect() }\n");
    output.push_str("    #[must_use]\n    pub fn semantic_references(&self) -> Vec<&SemanticReference> { let mut result = Vec::new(); match self {\n");
    for form in &contract.forms {
        let fields = form
            .fields
            .iter()
            .filter(|field| {
                matches!(
                    field.field_type,
                    QueryFieldType::SemanticReferenceList | QueryFieldType::PatternBindingList
                )
            })
            .collect::<Vec<_>>();
        if fields.is_empty() {
            writeln!(
                output,
                "            Self::{} {{ .. }} => {{}},",
                form.rust_variant
            )
            .unwrap();
            continue;
        }
        write!(output, "            Self::{} {{ ", form.rust_variant).unwrap();
        for field in &fields {
            write!(output, "{}, ", field.rust_name).unwrap();
        }
        output.push_str(".. } => {\n");
        for field in fields {
            match field.field_type {
                QueryFieldType::SemanticReferenceList => writeln!(output, "                result.extend({}.iter());", field.rust_name).unwrap(),
                QueryFieldType::PatternBindingList => writeln!(output, "                result.extend({0}.iter().filter_map(|binding| binding.within.as_ref()));", field.rust_name).unwrap(),
                _ => unreachable!(),
            }
        }
        output.push_str("            },\n");
    }
    output.push_str("        } result }\n}\n\n");
    output.push_str("#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub struct QueryFormDescriptor { pub code: u16, pub slug: &'static str, pub node_kind: &'static str, pub output_role: ResultRole, pub accepted_input_roles: &'static [ResultRole], pub canonical_order: &'static [&'static str], pub owner_section: u16 }\n\n");
    output.push_str("pub const QUERY_FORM_CONTRACTS: &[QueryFormDescriptor] = &[\n");
    for form in &contract.forms {
        let roles = form
            .accepted_input_roles
            .iter()
            .map(|role| format!("ResultRole::{}", upper_camel(role)))
            .collect::<Vec<_>>()
            .join(", ");
        let order = form
            .canonical_order
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, "    QueryFormDescriptor {{ code: {}, slug: {:?}, node_kind: {:?}, output_role: ResultRole::{}, accepted_input_roles: &[{}], canonical_order: &[{}], owner_section: {} }},", form.code, form.slug, form.node_kind, upper_camel(&form.output_role), roles, order, form.owner_section).unwrap();
    }
    output.push_str("];\n");
    output
}

fn upper_camel(value: &str) -> String {
    value
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect()
}

fn render_query_form_python(contract: &QueryFormContract) -> String {
    let slugs = contract
        .forms
        .iter()
        .map(|form| format!("    {:?},", form.slug))
        .collect::<Vec<_>>()
        .join("\n");
    let literals = contract
        .forms
        .iter()
        .map(|form| format!("    {:?},", form.slug))
        .collect::<Vec<_>>()
        .join("\n");
    let node_kinds = contract
        .forms
        .iter()
        .map(|form| format!("    {:?}: {:?},", form.slug, form.node_kind))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "# @generated by codefabric-model from {} {} ({}); do not edit.\nfrom typing import Final, Literal\n\nQUERY_FORM_CONTRACT_ID: Final = {:?}\nQUERY_FORM_CONTRACT_VERSION: Final = {:?}\nQUERY_FORM_CONTRACT_DIGEST: Final = (\n    {:?}\n)\nQUERY_FORMS: Final = (\n{}\n)\ntype QueryForm = Literal[\n{}\n]\nQUERY_FORM_NODE_KINDS: Final = {{\n{}\n}}\n",
        contract.artifact_id,
        contract.version,
        contract.canonical_digest,
        contract.artifact_id,
        contract.version,
        contract.canonical_digest,
        slugs,
        literals,
        node_kinds,
    )
}

fn query_field_schema(field_type: QueryFieldType) -> Value {
    match field_type {
        QueryFieldType::BoundedString => {
            json!({"type": "string", "minLength": 1, "maxLength": 4096})
        }
        QueryFieldType::BoundedStringList => {
            json!({"type": "array", "maxItems": 256, "items": {"type": "string", "minLength": 1, "maxLength": 4096}})
        }
        QueryFieldType::SemanticReferenceList => {
            json!({"type": "array", "maxItems": 256, "items": query_reference_schema()})
        }
        QueryFieldType::PriorResultList => {
            json!({"type": "array", "maxItems": 256, "items": {"$ref": "#/$defs/prior_result_reference"}})
        }
        QueryFieldType::PatternBindingList => {
            json!({"type": "array", "maxItems": 128, "items": {"$ref": "#/$defs/pattern_binding"}})
        }
        QueryFieldType::PatternRelationshipList => {
            json!({"type": "array", "maxItems": 256, "items": {"$ref": "#/$defs/pattern_relationship"}})
        }
        QueryFieldType::PositiveInteger => {
            json!({"type": "integer", "minimum": 1, "maximum": 10000})
        }
        QueryFieldType::ReturnSpec => json!({"$ref": "#/$defs/return_spec"}),
    }
}

fn query_schema_defs(contract: &QueryFormContract) -> Value {
    json!({
        "result_role": {"type": "string", "enum": contract.result_roles},
        "prior_result_reference": {
            "type": "object", "additionalProperties": false,
            "required": ["results_of", "select"],
            "properties": {
                "results_of": {"type": "string", "pattern": "^[A-Za-z][A-Za-z0-9_.-]*$"},
                "select": {"$ref": "#/$defs/result_role"}
            }
        },
        "return_limit": {
            "type": "object", "additionalProperties": false,
            "required": ["maximum_results"],
            "properties": {
                "maximum_results": {"type": "integer", "minimum": 1, "maximum": 10000},
                "per": {"type": "string", "minLength": 1, "maxLength": 4096},
                "when_exceeded": {"type": "string", "minLength": 1, "maxLength": 4096}
            }
        },
        "return_spec": {
            "type": "object", "additionalProperties": false,
            "properties": {
                "include": {"type": "array", "maxItems": 256, "items": {"type": "string", "minLength": 1, "maxLength": 4096}},
                "exclude": {"type": "array", "maxItems": 256, "items": {"type": "string", "minLength": 1, "maxLength": 4096}},
                "result_shape": {"type": "string", "minLength": 1, "maxLength": 4096},
                "group_by": {"type": "array", "maxItems": 256, "items": {"type": "string", "minLength": 1, "maxLength": 4096}},
                "order_by": {"type": "array", "maxItems": 256, "items": {"type": "string", "minLength": 1, "maxLength": 4096}},
                "deduplicate_by": {"type": "string", "minLength": 1, "maxLength": 4096},
                "supporting_facts": {"type": "string", "minLength": 1, "maxLength": 4096},
                "include_query_result": {"type": "boolean"},
                "limit": {"$ref": "#/$defs/return_limit"}
            }
        },
        "pattern_binding": {
            "type": "object", "additionalProperties": false,
            "required": ["name", "looking_for"],
            "properties": {
                "name": {"type": "string", "pattern": "^[A-Za-z][A-Za-z0-9_.-]*$"},
                "looking_for": {"type": "string", "minLength": 1, "maxLength": 4096},
                "within": query_reference_schema(),
                "where": {"type": "array", "maxItems": 256, "items": {"type": "string", "minLength": 1, "maxLength": 4096}}
            }
        },
        "pattern_relationship": {
            "type": "object", "additionalProperties": false,
            "required": ["from", "to", "relationship"],
            "properties": {
                "from": {"type": "string", "pattern": "^[A-Za-z][A-Za-z0-9_.-]*$"},
                "to": {"type": "string", "pattern": "^[A-Za-z][A-Za-z0-9_.-]*$"},
                "relationship": {"type": "string", "minLength": 1, "maxLength": 4096},
                "direction": {"type": "string", "minLength": 1, "maxLength": 4096},
                "distance": {"type": "string", "minLength": 1, "maxLength": 4096}
            }
        }
    })
}

fn query_variant_schema(form: &QueryFormContractEntry) -> Value {
    let mut properties = serde_json::Map::from_iter([
        (
            "query_id".to_owned(),
            json!({"type": "string", "pattern": "^[A-Za-z][A-Za-z0-9_.-]*$"}),
        ),
        ("request".to_owned(), json!({"const": form.slug})),
        (
            "label".to_owned(),
            json!({"type": "string", "minLength": 1, "maxLength": 4096}),
        ),
    ]);
    let mut required = vec![
        Value::String("query_id".to_owned()),
        Value::String("request".to_owned()),
    ];
    for field in &form.fields {
        let mut schema = query_field_schema(field.field_type);
        if field.required
            && matches!(
                field.field_type,
                QueryFieldType::BoundedStringList
                    | QueryFieldType::SemanticReferenceList
                    | QueryFieldType::PriorResultList
                    | QueryFieldType::PatternBindingList
                    | QueryFieldType::PatternRelationshipList
            )
        {
            schema
                .as_object_mut()
                .expect("array schema")
                .insert("minItems".to_owned(), Value::from(1));
        }
        properties.insert(field.name.clone(), schema);
        if field.required {
            required.push(Value::String(field.name.clone()));
        }
    }
    json!({"type": "object", "additionalProperties": false, "required": required, "properties": properties})
}

fn render_query_request_schema(contract: &QueryFormContract) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["specification", "version", "semantic_request_id", "workspace_id", "freshness_policy", "queries"],
        "properties": {
            "specification": {"const": contract.specification},
            "version": {"const": contract.specification_version},
            "semantic_request_id": {"type": "string", "minLength": 1},
            "workspace_id": {"type": "string", "pattern": "^workspace:[0-9a-f]{32}$"},
            "freshness_policy": {"type": "string", "enum": ["current_required", "wait_for_current", "best_available_snapshot"]},
            "queries": {"type": "array", "minItems": 1, "maxItems": 32, "items": {"oneOf": contract.forms.iter().map(query_variant_schema).collect::<Vec<_>>() }},
            "response_projection": {"type": ["object", "null"]},
            "cost_budget": {"type": ["object", "null"]}
        },
        "$defs": query_schema_defs(contract)
    })
}

fn render_plan_spec_schema(contract: &QueryFormContract) -> Value {
    let queries = contract.forms.iter().map(|form| json!({
        "type": "object", "additionalProperties": false,
        "required": ["node_kind", "query_id", "source_pointer", "input_roles", "output_role", "dependencies", "bound_semantics", "coverage_prerequisites", "coverage_effects", "canonical_order", "resource_contract"],
        "properties": {
            "node_kind": {"const": form.node_kind},
            "query_id": {"type": "string"},
            "source_pointer": {"type": "string"},
            "input_roles": {"type": "array", "items": {"type": "string", "enum": form.accepted_input_roles}},
            "output_role": {"const": form.output_role},
            "dependencies": {"type": "array", "items": {"$ref": "#/$defs/prior_result_reference"}},
            "bound_semantics": {"type": "object"},
            "coverage_prerequisites": {"type": "array", "items": {"type": "string"}},
            "coverage_effects": {"type": "array", "items": {"type": "string"}},
            "canonical_order": {"const": form.canonical_order},
            "resource_contract": {"type": "object"}
        }
    })).collect::<Vec<_>>();
    json!({
        "type": "object", "additionalProperties": false,
        "required": ["plan_spec_version", "binding_state", "bound_snapshot_id", "semantic_request_id", "workspace_id", "queries", "canonical_digest"],
        "properties": {
            "plan_spec_version": {"const": "1.0"},
            "binding_state": {"type": "string", "enum": ["unbound", "snapshot-bound"]},
            "bound_snapshot_id": {"type": ["string", "null"]},
            "semantic_request_id": {"type": "string"},
            "workspace_id": {"type": "string"},
            "queries": {"type": "array", "items": {"oneOf": queries}},
            "canonical_digest": {"type": "string", "pattern": "^b3:[0-9a-f]{64}$"}
        },
        "$defs": query_schema_defs(contract)
    })
}

fn render_public_schema(
    contract: &PublicSchemaContract,
    source_digest: &str,
    query_forms: &QueryFormContract,
    query_form_source_digest: &str,
) -> Result<Vec<u8>, SchemaDriverError> {
    let schema = match contract.schema_kind.as_str() {
        "cpg-semantic-query-request" => render_query_request_schema(query_forms),
        "plan-spec" => render_plan_spec_schema(query_forms),
        _ => contract.schema.clone(),
    };
    let mut body = schema
        .as_object()
        .cloned()
        .ok_or_else(|| SchemaDriverError::Invalid {
            path: format!("$.public_schemas[{}].schema", contract.schema_kind),
            detail: "schema body is not an object".to_owned(),
        })?;
    body.insert("$schema".to_owned(), Value::String(DIALECT.to_owned()));
    body.insert(
        "$id".to_owned(),
        Value::String(format!("https://codefabric.dev/{}", contract.path)),
    );
    body.insert("title".to_owned(), Value::String(contract.title.clone()));
    body.insert(
        "x-codefabric-artifact".to_owned(),
        json!({
            "artifact_id": contract.artifact_id,
            "artifact_kind": "json-schema",
            "version": "1.0",
            "compatible_suite_major": 1,
            "status": "released",
            "generator_revision": "codefabric-model-schema-driver-v1",
        }),
    );
    let (authority_id, authority_digest) = if matches!(
        contract.schema_kind.as_str(),
        "cpg-semantic-query-request" | "plan-spec"
    ) {
        (query_forms.artifact_id.as_str(), query_form_source_digest)
    } else {
        ("codefabric.schema.contract-ir", source_digest)
    };
    body.insert(
        "x-codefabric-generated".to_owned(),
        json!({
            "driver": "schema-contract-driver-v1",
            "source_artifact_id": authority_id,
            "source_digest": authority_digest,
        }),
    );
    pretty(&Value::Object(body))
}

fn render_ddl(plan: &SchemaPlan) -> Vec<u8> {
    let mut output = format!(
        "-- @generated from codefabric.schema.contract-ir semantic={} source={}; schema-contract-driver-v1; do not edit.\n-- Cross-store Arrow/Delta foreign keys are generated as application contracts, not SQLite reference clauses.\n\n",
        plan.semantic_digest, plan.source_digest
    );
    for table in &plan.ir.operational_tables {
        output.push_str(&render_operational_table_ddl(table));
        output.push('\n');
    }
    for projection in &plan.ir.control_projections {
        if projection.projection_role != ControlProjectionRole::DerivedOperational {
            continue;
        }
        let source = projection
            .source_table
            .as_deref()
            .expect("validated derived projection source");
        writeln!(
            output,
            "CREATE VIEW {} AS\nSELECT {}\nFROM {};\n",
            projection.view_name,
            projection.columns.join(", "),
            source,
        )
        .unwrap();
    }
    output.into_bytes()
}

fn render_operational_table_ddl(table: &OperationalTableContract) -> String {
    let mut output = format!("CREATE TABLE {} (\n", table.name);
    let mut definitions = table
        .columns
        .iter()
        .map(|column| {
            format!(
                "  {} {}{}",
                column.name,
                match column.sqlite_type {
                    SqliteType::Integer => "INTEGER",
                    SqliteType::Real => "REAL",
                    SqliteType::Text => "TEXT",
                    SqliteType::Blob => "BLOB",
                },
                if column.nullable { "" } else { " NOT NULL" }
            )
        })
        .collect::<Vec<_>>();
    definitions.push(format!("  PRIMARY KEY ({})", table.primary_key.join(", ")));
    definitions.extend(
        table
            .unique
            .iter()
            .map(|columns| format!("  UNIQUE ({})", columns.join(", "))),
    );
    writeln!(output, "{}\n) STRICT;", definitions.join(",\n")).unwrap();
    output
}

fn render_rust(ir: &SchemaContractIr) -> Vec<u8> {
    let mut output = String::from(
        "// generated by schema-contract-driver-v1; do not edit.\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub enum ModelLogicalType { Id16, Hash32, Code16, Code32, Bucket16, Int16, Int32, Int64, Float64, Boolean, Utf8, Binary, TimestampUtc, IdList, Int64List, StringMap }\n\
         #[derive(Clone, Copy, Debug)]\n\
         pub struct ModelColumn { pub field_id: &'static str, pub name: &'static str, pub logical_type: ModelLogicalType, pub nullable: bool, pub semantic_type: Option<&'static str>, pub foreign_key: Option<&'static str> }\n\
         #[derive(Clone, Copy, Debug)]\n\
         pub struct ModelTable { pub table_code: i16, pub table_id: &'static str, pub name: &'static str, pub columns: &'static [ModelColumn], pub primary_key: &'static [&'static str] }\n\n\
         pub const MODEL_TABLES: &[ModelTable] = &[\n",
    );
    for table in &ir.tables {
        writeln!(
            output,
            "    ModelTable {{ table_code: {}, table_id: {:?}, name: {:?}, columns: &[",
            table.table_code,
            format!("table:{}", table.name),
            table.name
        )
        .unwrap();
        for column in &table.columns {
            writeln!(
                output,
                "        ModelColumn {{ field_id: {:?}, name: {:?}, logical_type: ModelLogicalType::{:?}, nullable: {}, semantic_type: {:?}, foreign_key: {:?} }},",
                format!("{}.{}", table.name, column.name),
                column.name,
                column.logical_type,
                column.nullable,
                column.semantic_type.as_deref(),
                column.foreign_key.as_deref(),
            )
            .unwrap();
        }
        writeln!(output, "    ], primary_key: &{:?} }},", table.primary_key).unwrap();
    }
    output.push_str("];\n\n");
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub enum ProviderObservationLogicalType { Utf8, Binary, Boolean, UInt64, Utf8List }\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderObservationField { pub name: &'static str, pub logical_type: ProviderObservationLogicalType, pub nullable: bool }\n\
         #[derive(Clone, Copy, Debug)]\n\
         pub struct ProviderObservationSchema { pub schema_id: &'static str, pub provider_id: &'static str, pub observation_family_code: u16, pub canonical_descriptor: &'static str, pub schema_digest: &'static str, pub fields: &'static [ProviderObservationField] }\n\n\
         pub const PROVIDER_OBSERVATION_SCHEMAS: &[ProviderObservationSchema] = &[\n",
    );
    for schema in &ir.provider_observation_schemas {
        let descriptor = provider_observation_descriptor(schema);
        let digest = codefabric::integrity::framed_digest(descriptor.as_bytes());
        writeln!(
            output,
            "    ProviderObservationSchema {{ schema_id: {:?}, provider_id: {:?}, observation_family_code: {}, canonical_descriptor: {:?}, schema_digest: {:?}, fields: &[",
            schema.schema_id,
            schema.provider_id,
            schema.observation_family_code,
            descriptor,
            digest,
        )
        .unwrap();
        for field in &schema.fields {
            writeln!(
                output,
                "        ProviderObservationField {{ name: {:?}, logical_type: ProviderObservationLogicalType::{:?}, nullable: {} }},",
                field.name, field.logical_type, field.nullable,
            )
            .unwrap();
        }
        output.push_str("    ] },\n");
    }
    output.push_str("];\n");
    output.into_bytes()
}

fn provider_observation_descriptor(schema: &ProviderObservationSchemaContract) -> String {
    let mut descriptor = schema.schema_id.clone();
    for field in &schema.fields {
        descriptor.push(':');
        descriptor.push_str(&field.name);
        descriptor.push(':');
        descriptor.push_str(match field.logical_type {
            ProviderObservationLogicalType::Utf8 => "utf8",
            ProviderObservationLogicalType::Binary => "binary",
            ProviderObservationLogicalType::Boolean => "boolean",
            ProviderObservationLogicalType::UInt64 => "uint64",
            ProviderObservationLogicalType::Utf8List => "list<utf8>",
        });
        if field.nullable {
            descriptor.push('?');
        }
    }
    descriptor
}

#[allow(clippy::too_many_lines)] // One linear pass keeps the complete runtime view tied to one IR.
fn render_runtime_rust(ir: &SchemaContractIr, source_digest: &str) -> Vec<u8> {
    let mut output = format!(
        "// @generated from codefabric.schema.contract-ir {source_digest}; schema-contract-driver-v1; do not edit.\n\npub const GENERATED_ONTOLOGY_VERSION: &str = {:?};\npub const GENERATED_COMPATIBILITY_MODE: &str = {:?};\nconst GENERATED_REQUIRE_SCHEMA_DIGEST_EQUALITY: bool = {};\nconst GENERATED_ALLOW_TYPE_WIDENING: bool = {};\nconst GENERATED_COLUMN_MAPPING_MODE: &str = {:?};\n\nconst GENERATED_METADATA_DICTIONARY: &[MetadataAnnotationSpec] = &[\n",
        ir.ontology_version,
        ir.compatibility_mode,
        ir.schema_evolution_policy.require_schema_digest_equality,
        ir.schema_evolution_policy.allow_type_widening,
        ir.schema_evolution_policy.column_mapping_mode,
    );
    for annotation in &ir.metadata_dictionary {
        writeln!(
            output,
            "    MetadataAnnotationSpec {{ annotation: {:?}, class: MetadataClass::{:?}, consumer_path: {:?}, consumer_symbol: {:?} }},",
            annotation.annotation,
            annotation.class,
            annotation.consumer_path.as_deref(),
            annotation.consumer_symbol.as_deref(),
        )
        .unwrap();
    }
    output.push_str(
        "]\n;\n\nconst GENERATED_SEMANTIC_TYPE_BINDINGS: &[SemanticTypeBindingSpec] = &[\n",
    );
    for binding in &ir.semantic_type_bindings {
        let authority = ir
            .semantic_authorities
            .iter()
            .find(|authority| authority.authority == binding.authority);
        writeln!(
            output,
            "    SemanticTypeBindingSpec {{ semantic_type: {:?}, authority: SemanticAuthority::{:?}, domain: {:?}, authority_artifact_id: {:?}, authority_digest: {:?} }},",
            binding.semantic_type,
            binding.authority,
            binding.domain.as_deref(),
            authority.map(|authority| authority.artifact_id.as_str()),
            authority.map(|authority| authority.canonical_digest.as_str()),
        )
        .unwrap();
    }
    output.push_str("]\n;\n\nconst GENERATED_FOREIGN_KEY_CONTRACTS: &[ForeignKeyContract] = &[\n");
    let by_name = ir
        .tables
        .iter()
        .map(|table| (table.name.as_str(), table))
        .collect::<BTreeMap<_, _>>();
    for table in &ir.tables {
        for (source_column_index, column) in table.columns.iter().enumerate() {
            let Some(foreign_key) = column.foreign_key.as_deref() else {
                continue;
            };
            let (target_table_name, target_column_name) = foreign_key
                .split_once('.')
                .expect("validated foreign-key syntax");
            let target_table = by_name[target_table_name];
            let target_column_index = target_table
                .columns
                .iter()
                .position(|candidate| candidate.name == target_column_name)
                .expect("validated foreign-key target");
            writeln!(
                output,
                "    ForeignKeyContract {{ source_table_code: {}, source_column_index: {}, source_column: {:?}, target_table_code: {}, target_column_index: {}, target_column: {:?} }},",
                table.table_code,
                source_column_index,
                column.name,
                target_table.table_code,
                target_column_index,
                target_column_name,
            )
            .unwrap();
        }
    }
    output.push_str("]\n;\n\nconst GENERATED_TABLE_SPECS: &[GeneratedTableSpec] = &[\n");
    for table in &ir.tables {
        writeln!(output, "    GeneratedTableSpec {{").unwrap();
        writeln!(
            output,
            "        table_code: {}, name: {:?}, family: {:?}, grain: {:?}, schema_version: {:?},",
            table.table_code, table.name, table.family, table.grain, table.schema_version
        )
        .unwrap();
        output.push_str("        columns: &[\n");
        for column in &table.columns {
            writeln!(
                output,
                "            GeneratedColumn {{ name: {:?}, logical_type: LogicalType::{:?}, nullable: {}, semantic_type: {:?}, foreign_key: {:?}, hidden_operational: {} }},",
                column.name,
                column.logical_type,
                column.nullable,
                column.semantic_type.as_deref(),
                column.foreign_key.as_deref(),
                column.hidden_operational,
            )
            .unwrap();
        }
        output.push_str("        ],\n");
        writeln!(
            output,
            "        primary_key: {}, partition_columns: {}, zorder_columns: {},",
            rust_strings(&table.primary_key),
            rust_strings(&table.partition_columns),
            rust_strings(&table.zorder_columns),
        )
        .unwrap();
        writeln!(
            output,
            "        durable_mutation: DurableMutationClass::{:?}, overlay_mutation: OverlayMutationPolicy::{:?}, materialization_role: MaterializationRole::{:?}, publication_pin_role: PublicationPinRole::{:?},",
            table.durable_mutation,
            table.overlay_mutation,
            table.materialization_role,
            table.publication_pin_role,
        )
        .unwrap();
        writeln!(
            output,
            "        dependencies: &{:?}, required_for_publication: {},",
            table.dependencies, table.required_for_publication
        )
        .unwrap();
        output.push_str("    },\n");
    }
    output.push_str(
        "];\n\nconst GENERATED_OPERATIONAL_TABLE_SPECS: &[GeneratedOperationalTableSpec] = &[\n",
    );
    for table in &ir.operational_tables {
        writeln!(output, "    GeneratedOperationalTableSpec {{").unwrap();
        writeln!(output, "        name: {:?},", table.name).unwrap();
        writeln!(
            output,
            "        sqlite_ddl: {:?},",
            render_operational_table_ddl(table)
        )
        .unwrap();
        output.push_str("        columns: &[\n");
        for column in &table.columns {
            writeln!(
                output,
                "            GeneratedOperationalColumn {{ name: {:?}, sqlite_type: OperationalSqliteType::{:?}, nullable: {} }},",
                column.name, column.sqlite_type, column.nullable
            )
            .unwrap();
        }
        output.push_str("        ],\n");
        writeln!(
            output,
            "        primary_key: {},",
            rust_strings(&table.primary_key)
        )
        .unwrap();
        writeln!(
            output,
            "        workspace_scope: {},",
            rust_operational_scope(table.workspace_scope.as_ref())
        )
        .unwrap();
        output.push_str("    },\n");
    }
    output.push_str("];\n\nconst GENERATED_TABLE_SCOPE_SPECS: &[TableScopeSpec] = &[\n");
    for scope in &ir.table_scopes {
        writeln!(
            output,
            "    TableScopeSpec {{ table_code: {}, workspace_column: {:?}, analysis_context_column: {:?}, source_generation_column: {:?}, analysis_context_set_column: {:?}, owner_column: {:?} }},",
            scope.table_code,
            scope.workspace_column.as_deref(),
            scope.analysis_context_column.as_deref(),
            scope.source_generation_column.as_deref(),
            scope.analysis_context_set_column.as_deref(),
            scope.owner_column.as_deref(),
        )
        .unwrap();
    }
    output.push_str(
        "];\n\nconst GENERATED_SERVING_PROJECTION_SPECS: &[ServingProjectionSpec] = &[\n",
    );
    for projection in &ir.serving_projections {
        writeln!(
            output,
            "    ServingProjectionSpec {{ view_name: {:?}, source_table_code: {}, availability_wave: {}, projection_role: ServingProjectionRole::{:?} }},",
            projection.view_name,
            projection.source_table_code,
            projection.availability_wave,
            projection.projection_role,
        )
        .unwrap();
    }
    output.push_str(
        "];\n\nconst GENERATED_CONTROL_PROJECTION_SPECS: &[ControlProjectionSpec] = &[\n",
    );
    for projection in &ir.control_projections {
        writeln!(
            output,
            "    ControlProjectionSpec {{ view_name: {:?}, availability_wave: {}, projection_role: ControlProjectionRole::{:?}, source_table: {:?}, columns: {} }},",
            projection.view_name,
            projection.availability_wave,
            projection.projection_role,
            projection.source_table.as_deref(),
            rust_strings(&projection.columns),
        )
        .unwrap();
    }
    let resources = ir.serving_resource_profile;
    writeln!(
        output,
        "];\n\nconst GENERATED_SERVING_RESOURCE_PROFILE: ServingResourceProfile = ServingResourceProfile {{ batch_size: {}, max_output_rows: {}, max_output_bytes: {}, max_output_batches: {}, max_control_rows: {}, max_control_bytes: {}, max_control_batches: {}, max_snapshot_validation_rows: {}, max_snapshot_validation_bytes: {}, max_snapshot_validation_batches: {} }};",
        rust_usize_literal(resources.batch_size),
        rust_usize_literal(resources.max_output_rows),
        rust_usize_literal(resources.max_output_bytes),
        rust_usize_literal(resources.max_output_batches),
        rust_usize_literal(resources.max_control_rows),
        rust_usize_literal(resources.max_control_bytes),
        rust_usize_literal(resources.max_control_batches),
        rust_usize_literal(resources.max_snapshot_validation_rows),
        rust_usize_literal(resources.max_snapshot_validation_bytes),
        rust_usize_literal(resources.max_snapshot_validation_batches),
    )
    .unwrap();
    output.into_bytes()
}

fn render_row_encoders(
    ir: &SchemaContractIr,
    source_digest: &str,
) -> Result<Vec<u8>, SchemaDriverError> {
    let mut output = format!(
        "// @generated from codefabric.schema.contract-ir {source_digest}; schema-contract-driver-v1; do not edit.\n\n"
    );
    for table in ir.tables.iter().filter(|table| table.row_encoder.is_some()) {
        let encoder = table.row_encoder.expect("filtered generated row encoder");
        writeln!(
            output,
            "/// Encode `{}` rows in the exact generated schema order.\n///\n/// # Errors\n///\n/// Returns an Arrow error if a typed accessor and its generated physical field diverge.\npub fn {}(rows: &[{}]) -> Result<RecordBatch, FactIngestError> {{\n    generated_fact_batch(\n        {},\n        vec![",
            table.name,
            encoder.rust_function(),
            encoder.rust_row_type(),
            table.table_code,
        )
        .unwrap();
        for column in &table.columns {
            writeln!(
                output,
                "            {},",
                render_encoder_column(table.table_code, column)?
            )
            .unwrap();
        }
        output.push_str("        ],\n    )\n}\n\n");
    }
    Ok(output.into_bytes())
}

fn render_encoder_column(
    table_code: i16,
    column: &ColumnContract,
) -> Result<String, SchemaDriverError> {
    let name = column.name.as_str();
    let direct = match name {
        "workspace_id" | "analysis_context_id" | "source_generation" | "owner_id" => {
            format!("row.scope.{name}")
        }
        _ => format!("row.{name}"),
    };
    let expression = match name {
        "owner_bucket" => "i16s(rows, |row| Some(i16::from(row.scope.owner_id[0])))".into(),
        "source_bucket" => "i16s(rows, |row| Some(i16::from(row.source_id[0])))".into(),
        "target_bucket" => "i16s(rows, |row| Some(i16::from(row.target_id[0])))".into(),
        "value_kind_code" => "i16s(rows, |row| Some(row.value.code()))".into(),
        "value_entity_id" => "id16s(rows, |row| match &row.value { PropertyValue::Entity(value) => Some(value), _ => None })".into(),
        "value_bool" => "bools(rows, |row| match row.value { PropertyValue::Boolean(value) => Some(value), _ => None })".into(),
        "value_int64" => "i64s(rows, |row| match row.value { PropertyValue::Integer(value) => Some(value), _ => None })".into(),
        "value_float64" => "f64s(rows, |row| match row.value { PropertyValue::Float(value) => Some(value), _ => None })".into(),
        "value_text" => "utf8(rows, |row| match &row.value { PropertyValue::Text(value) => Some(value.as_str()), _ => None })".into(),
        "value_bytes" => "binary(rows, |row| match &row.value { PropertyValue::Bytes(value) => Some(value.as_slice()), _ => None })".into(),
        "value_type_id" => "id16s(rows, |row| match &row.value { PropertyValue::Type(value) => Some(value), _ => None })".into(),
        _ => match column.logical_type {
            LogicalType::Id16 => {
                if column.nullable {
                    format!("id16s(rows, |row| {direct}.as_ref())")
                } else {
                    format!("id16s(rows, |row| Some(&{direct}))")
                }
            }
            LogicalType::Hash32 => {
                if column.nullable {
                    format!("binary(rows, |row| {direct}.as_ref().map(<[u8; 32]>::as_slice))")
                } else {
                    format!("binary(rows, |row| Some({direct}.as_slice()))")
                }
            }
            LogicalType::Binary => {
                if column.nullable {
                    format!("binary(rows, |row| {direct}.as_deref())")
                } else {
                    format!("binary(rows, |row| Some({direct}.as_slice()))")
                }
            }
            LogicalType::Utf8 => {
                if column.nullable {
                    format!("utf8(rows, |row| {direct}.as_deref())")
                } else {
                    format!("utf8(rows, |row| Some({direct}.as_str()))")
                }
            }
            LogicalType::Code16 | LogicalType::Bucket16 | LogicalType::Int16 => {
                if column.nullable {
                    format!("i16s(rows, |row| {direct})")
                } else {
                    format!("i16s(rows, |row| Some({direct}))")
                }
            }
            LogicalType::Code32 | LogicalType::Int32 => {
                if column.nullable {
                    format!("i32s(rows, |row| {direct})")
                } else {
                    format!("i32s(rows, |row| Some({direct}))")
                }
            }
            LogicalType::Int64 => {
                if column.nullable {
                    format!("i64s(rows, |row| {direct})")
                } else {
                    format!("i64s(rows, |row| Some({direct}))")
                }
            }
            LogicalType::Float64 => {
                if column.nullable {
                    format!("f64s(rows, |row| {direct})")
                } else {
                    format!("f64s(rows, |row| Some({direct}))")
                }
            }
            LogicalType::Boolean => {
                if column.nullable {
                    format!("bools(rows, |row| {direct})")
                } else {
                    format!("bools(rows, |row| Some({direct}))")
                }
            }
            LogicalType::Int64List if !column.nullable => format!(
                "i64_lists({table_code}, {name:?}, rows, |row| {direct}.as_slice())"
            ),
            LogicalType::TimestampUtc
            | LogicalType::IdList
            | LogicalType::Int64List
            | LogicalType::StringMap => {
                return invalid(
                    "$.tables[*].row_encoder",
                    format!("unsupported generated encoder field {table_code}.{name}"),
                );
            }
        },
    };
    Ok(expression)
}

fn rust_strings(values: &[String]) -> String {
    format!(
        "&[{}]",
        values
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn rust_operational_scope(scope: Option<&OperationalWorkspaceScopeContract>) -> String {
    match scope {
        None => "None".to_owned(),
        Some(OperationalWorkspaceScopeContract::Direct { workspace_column }) => format!(
            "Some(OperationalWorkspaceScope::Direct {{ workspace_column: {workspace_column:?} }})"
        ),
        Some(OperationalWorkspaceScopeContract::ViaParent {
            parent_table,
            child_column,
            parent_column,
            workspace_column,
        }) => format!(
            "Some(OperationalWorkspaceScope::ViaParent {{ parent_table: {parent_table:?}, child_column: {child_column:?}, parent_column: {parent_column:?}, workspace_column: {workspace_column:?} }})"
        ),
    }
}

fn rust_usize_literal(value: usize) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push('_');
        }
        grouped.push(digit);
    }
    grouped.chars().rev().collect()
}

fn render_validation(plan: &SchemaPlan) -> Result<Vec<u8>, SchemaDriverError> {
    let compatibility_cases = [
        ("exact-schema", "unchanged", true),
        ("nullable-change", "change-nullability", false),
        ("logical-type-change", "change-logical-type", false),
        ("field-order-change", "reorder-fields", false),
        ("field-addition", "add-field", false),
        ("field-removal", "remove-field", false),
        ("metadata-change", "change-contract-metadata", false),
    ]
    .into_iter()
    .map(|(case_id, mutation, accepted)| {
        json!({"case_id": case_id, "mutation": mutation, "accepted": accepted})
    })
    .collect::<Vec<_>>();
    pretty(&json!({
        "schema_version": 1,
        "family": "schemas",
        "source_digest": plan.source_digest,
        "table_count": plan.ir.tables.len(),
        "operational_table_count": plan.ir.operational_tables.len(),
        "public_schema_count": plan.ir.public_schemas.len(),
        "stable_field_id_rule": "<table-name>.<field-name>",
        "compatibility_acceptance_generated": true,
        "compatibility_class": plan.ir.schema_evolution_policy.compatibility_class,
        "compatibility_cases": compatibility_cases,
        "native_validators": ["arrow-schema-59.2.0", "datafusion-55.0.0", "sqlite-strict", "jsonschema-draft-2020-12"],
    }))
}

fn render_evolution_policy(plan: &SchemaPlan) -> Result<Vec<u8>, SchemaDriverError> {
    pretty(&json!({
        "artifact_id": plan.ir.schema_evolution_policy.policy_id,
        "artifact_kind": "schema-evolution-policy",
        "version": plan.ir.schema_evolution_policy.version,
        "source_artifact_id": plan.ir.header.artifact_id,
        "source_digest": plan.source_digest,
        "compatibility_class": plan.ir.schema_evolution_policy.compatibility_class,
        "require_schema_digest_equality": plan.ir.schema_evolution_policy.require_schema_digest_equality,
        "allow_type_widening": plan.ir.schema_evolution_policy.allow_type_widening,
        "column_mapping_mode": plan.ir.schema_evolution_policy.column_mapping_mode,
        "migration_route": plan.ir.schema_evolution_policy.migration_route,
        "acceptance_suite": VALIDATION_PATH,
    }))
}

fn arrow_type(logical_type: LogicalType) -> Value {
    match logical_type {
        LogicalType::Id16 => {
            json!({"name":"fixed_size_binary","byte_width":16,"extension":{"name":"codefabric.id16","metadata":"version=1"}})
        }
        LogicalType::Hash32 => json!({"name":"binary","byte_width":32}),
        LogicalType::Code16 | LogicalType::Bucket16 | LogicalType::Int16 => json!({"name":"int16"}),
        LogicalType::Code32 | LogicalType::Int32 => json!({"name":"int32"}),
        LogicalType::Int64 => json!({"name":"int64"}),
        LogicalType::Float64 => json!({"name":"float64"}),
        LogicalType::Boolean => json!({"name":"boolean"}),
        LogicalType::Utf8 => json!({"name":"utf8"}),
        LogicalType::Binary => json!({"name":"binary"}),
        LogicalType::TimestampUtc => {
            json!({"name":"timestamp","unit":"microsecond","timezone":"UTC"})
        }
        LogicalType::IdList => {
            json!({"name":"list","element":{"name":"fixed_size_binary","byte_width":16,"nullable":false,"extension":{"name":"codefabric.id16","metadata":"version=1"}}})
        }
        LogicalType::Int64List => {
            json!({"name":"list","element":{"name":"int64","nullable":false}})
        }
        LogicalType::StringMap => {
            json!({"name":"map","key":{"name":"utf8","nullable":false},"value":{"name":"utf8","nullable":false},"sorted":false})
        }
    }
}

fn pretty(value: &Value) -> Result<Vec<u8>, SchemaDriverError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn safe(path: &str) -> Result<SafeOutputPath, SchemaDriverError> {
    SafeOutputPath::parse(path.as_bytes().to_vec()).map_err(|_| SchemaDriverError::Invalid {
        path: path.to_owned(),
        detail: "unsafe output path".to_owned(),
    })
}

fn safe_protocol(path: &str) -> Result<SafeOutputPath, DriverProtocolError> {
    SafeOutputPath::parse(path.as_bytes().to_vec())
        .map_err(|_| DriverProtocolError::InvalidDescriptor)
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && value.as_bytes()[0].is_ascii_lowercase()
}

fn invalid<T>(path: &str, detail: impl Into<String>) -> Result<T, SchemaDriverError> {
    Err(SchemaDriverError::Invalid {
        path: path.to_owned(),
        detail: detail.into(),
    })
}

fn decode_ir(bytes: &[u8]) -> Result<SchemaContractIr, SchemaDriverError> {
    let value = codefabric::contracts::jcs::decode_strict(bytes)?;
    serde_json::from_value(value).map_err(SchemaDriverError::Json)
}

fn decode_query_form_contract(bytes: &[u8]) -> Result<QueryFormContract, SchemaDriverError> {
    let value = codefabric::contracts::jcs::decode_strict(bytes)?;
    serde_json::from_value(value).map_err(SchemaDriverError::Json)
}

fn detached_query_form_identity(bytes: &[u8]) -> Result<String, SchemaDriverError> {
    let mut value = codefabric::contracts::jcs::decode_strict(bytes)?;
    value
        .as_object_mut()
        .ok_or_else(|| SchemaDriverError::Invalid {
            path: "$".to_owned(),
            detail: "typed query-form contract root is not an object".to_owned(),
        })?
        .remove("canonical_digest");
    let canonical = codefabric::contracts::jcs::canonicalize_value(&value)?;
    Ok(codefabric::integrity::framed_digest(&canonical))
}

fn validate_query_form_registry(
    contract: &QueryFormContract,
    bytes: &[u8],
) -> Result<(), SchemaDriverError> {
    let registry: serde_yaml_ng::Value =
        serde_yaml_ng::from_slice(bytes).map_err(|source| SchemaDriverError::Invalid {
            path: ENUM_REGISTRY_PATH.to_owned(),
            detail: source.to_string(),
        })?;
    let records = registry
        .get("records")
        .and_then(serde_yaml_ng::Value::as_sequence)
        .ok_or_else(|| SchemaDriverError::Invalid {
            path: ENUM_REGISTRY_PATH.to_owned(),
            detail: "registry records are absent".to_owned(),
        })?;
    let query_form = records
        .iter()
        .find(|record| {
            record.get("domain").and_then(serde_yaml_ng::Value::as_str) == Some("QUERY_FORM")
        })
        .ok_or_else(|| SchemaDriverError::Invalid {
            path: ENUM_REGISTRY_PATH.to_owned(),
            detail: "QUERY_FORM domain is absent".to_owned(),
        })?;
    let values = query_form
        .get("values")
        .and_then(serde_yaml_ng::Value::as_sequence)
        .ok_or_else(|| SchemaDriverError::Invalid {
            path: ENUM_REGISTRY_PATH.to_owned(),
            detail: "QUERY_FORM values are absent".to_owned(),
        })?;
    let projected = values
        .iter()
        .map(|value| {
            let code = value.get("code").and_then(serde_yaml_ng::Value::as_u64);
            let name = value.get("name").and_then(serde_yaml_ng::Value::as_str);
            let slug = value.get("slug").and_then(serde_yaml_ng::Value::as_str);
            (code, name, slug)
        })
        .collect::<Vec<_>>();
    let expected = contract
        .forms
        .iter()
        .map(|form| {
            (
                Some(u64::from(form.code)),
                Some(form.name.as_str()),
                Some(form.slug.as_str()),
            )
        })
        .collect::<Vec<_>>();
    if projected != expected {
        return invalid(
            "$.forms",
            "query-form contract differs from the QUERY_FORM registry",
        );
    }
    Ok(())
}

/// Compile the schema Contract IR through its closed family-native model and return the detached
/// semantic identity used by aggregate provenance.
///
/// # Errors
///
/// Returns a duplicate-key, closed-model, schema-invariant, or canonicalization failure.
pub fn detached_schema_identity(bytes: &[u8]) -> Result<String, SchemaDriverError> {
    let document = decode_ir(bytes)?;
    document.validate()?;
    let mut value = serde_json::to_value(document)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| SchemaDriverError::Invalid {
            path: "$".to_owned(),
            detail: "typed schema Contract IR root is not an object".to_owned(),
        })?;
    object.remove("canonical_digest");
    object.remove("source_digest");
    let canonical = codefabric::contracts::jcs::canonicalize_value(&value)?;
    Ok(codefabric::integrity::framed_digest(&canonical))
}

/// Schema family error with bounded JSON-path diagnostics.
#[derive(Debug, Error)]
pub enum SchemaDriverError {
    #[error(transparent)]
    Driver(#[from] DriverProtocolError),
    #[error(transparent)]
    Repository(#[from] super::repository_model::RepositoryModelError),
    #[error("schema authority invalid at {path}: {detail}")]
    Invalid { path: String, detail: String },
    #[error("schema projection differs from typed plan")]
    ProjectionMismatch,
    #[error("schema JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    CanonicalJson(#[from] codefabric::contracts::jcs::CanonicalJsonError),
    #[error(transparent)]
    SemanticFragment(#[from] super::semantic_fragment_driver::SemanticFragmentError),
    #[error("schema I/O failed at {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authority() -> SchemaContractIr {
        let bytes = read_stable(Path::new(SCHEMA_IR_PATH), MAX_AUTHORITY_BYTES).unwrap();
        let mut value = codefabric::contracts::jcs::decode_strict(&bytes).unwrap();
        super::super::semantic_fragment_driver::SemanticFragmentSet::load(Path::new("."))
            .unwrap()
            .compose_schema(&mut value)
            .unwrap();
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn model_schema_semantic_authority_digests_are_exact() {
        for path in [
            ENUM_REGISTRY_PATH,
            ENTITY_REGISTRY_PATH,
            RELATION_REGISTRY_PATH,
            PROPERTY_REGISTRY_PATH,
            FACT_REGISTRY_PATH,
            CAPABILITY_REGISTRY_PATH,
        ] {
            let bytes = read_stable(Path::new(path), MAX_AUTHORITY_BYTES).unwrap();
            let value: Value = serde_yaml_ng::from_slice(&bytes).unwrap();
            let artifact_id = value["artifact_id"].as_str().unwrap();
            let computed =
                super::super::registry_cbef_driver::detached_registry_identity(artifact_id, &bytes)
                    .unwrap()
                    .unwrap();
            assert_eq!(computed, value["canonical_digest"], "{path}");
        }
    }

    #[test]
    fn model_schema_semantic_identity_is_exact() {
        let bytes = read_stable(Path::new(SCHEMA_IR_PATH), MAX_AUTHORITY_BYTES).unwrap();
        let computed = detached_schema_identity(&bytes).unwrap();
        assert_eq!(computed, authority().header.canonical_digest);
    }

    #[test]
    fn model_tablespec_projects_equivalent_arrow_json_schema_and_ddl() {
        let ir = authority();
        ir.validate().unwrap();
        assert_eq!(ir.public_schemas.len(), 8);
        assert_eq!(ir.operational_tables.len(), 27);
        for table in &ir.tables {
            let ids = table
                .columns
                .iter()
                .map(|column| format!("{}.{}", table.name, column.name))
                .collect::<BTreeSet<_>>();
            assert_eq!(ids.len(), table.columns.len());
            for column in &table.columns {
                assert!(arrow_type(column.logical_type).is_object());
            }
        }
        let plan = SchemaDriver.plan(Path::new(".")).unwrap();
        let ddl = String::from_utf8(render_ddl(&plan)).unwrap();
        for projection in &ir.control_projections {
            let declaration = format!("CREATE VIEW {} AS", projection.view_name);
            match projection.projection_role {
                ControlProjectionRole::DerivedOperational => {
                    assert!(ddl.contains(&declaration));
                }
                ControlProjectionRole::OperationalSource
                | ControlProjectionRole::ActiveServingSnapshot => {
                    assert!(!ddl.contains(&declaration));
                }
            }
        }
    }

    #[test]
    fn model_row_encoder_round_trips_every_supported_field_shape() {
        let variants = [
            LogicalType::Id16,
            LogicalType::Hash32,
            LogicalType::Code16,
            LogicalType::Code32,
            LogicalType::Bucket16,
            LogicalType::Int16,
            LogicalType::Int32,
            LogicalType::Int64,
            LogicalType::Float64,
            LogicalType::Boolean,
            LogicalType::Utf8,
            LogicalType::Binary,
            LogicalType::TimestampUtc,
            LogicalType::IdList,
            LogicalType::Int64List,
            LogicalType::StringMap,
        ];
        for logical_type in variants {
            let encoded = serde_json::to_vec(&logical_type).unwrap();
            let decoded: LogicalType = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(decoded, logical_type);
            assert!(arrow_type(decoded).is_object());
        }
    }

    #[test]
    fn model_schema_compatibility_diagnostics_are_path_aware() {
        let mut ir = authority();
        ir.tables[0].columns[0].foreign_key = Some("missing.field".to_owned());
        let error = ir.validate().unwrap_err().to_string();
        assert!(error.contains("$.tables[0].columns[0].foreign_key"));
        assert!(error.contains("missing.field"));
    }

    #[test]
    fn model_schema_rejects_unknown_duplicate_and_incompatible_fields() {
        let bytes = read_stable(Path::new(SCHEMA_IR_PATH), MAX_AUTHORITY_BYTES).unwrap();
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        value["unknown"] = Value::Bool(true);
        assert!(decode_ir(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(decode_ir(b"{\"schema_version\":1,\"schema_version\":2}").is_err());
        let mut ir = authority();
        ir.tables[0].columns[0].nullable = true;
        ir.tables[0].primary_key.push("missing".to_owned());
        assert!(ir.validate().is_err());
    }

    #[test]
    fn model_schema_outputs_have_one_producer_and_no_manual_public_include_list() {
        let mut descriptor = SchemaDriver.describe().unwrap();
        let ir = authority();
        for schema in &ir.public_schemas {
            descriptor.outputs.push(
                SchemaDriver::output(
                    format!("output:model-schema-{}", schema.schema_kind),
                    &schema.path,
                    DriverOutputRole::PublicJsonSchema,
                )
                .unwrap(),
            );
        }
        let paths = descriptor
            .outputs
            .iter()
            .map(|output| output.path.display())
            .collect::<BTreeSet<_>>();
        assert_eq!(paths.len(), descriptor.outputs.len());
        assert_eq!(
            descriptor
                .outputs
                .iter()
                .filter(|output| output.role == DriverOutputRole::PublicJsonSchema)
                .count(),
            8
        );
        assert_eq!(descriptor.sources.len(), 9);
        assert_eq!(descriptor.output_roots.len(), 2);
    }

    #[test]
    fn model_driver_generates_compatibility_acceptance() {
        let descriptor = SchemaDriver.describe().unwrap();
        assert!(
            descriptor
                .outputs
                .iter()
                .any(|output| { output.path.display() == VALIDATION_PATH })
        );
        assert!(
            descriptor
                .outputs
                .iter()
                .any(|output| { output.path.display() == EVOLUTION_POLICY_PATH })
        );
    }

    #[test]
    fn wp57_structural_acceptance() {
        let ir = authority();
        ir.validate().unwrap();
        assert_eq!(ir.metadata_dictionary.len(), 24);
        assert_eq!(
            ir.metadata_dictionary
                .iter()
                .map(|entry| entry.class)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                MetadataClass::Enforced,
                MetadataClass::PlannerConsumed,
                MetadataClass::Contractual,
                MetadataClass::Governance,
                MetadataClass::Lineage,
                MetadataClass::Advisory,
            ])
        );
        for entry in &ir.metadata_dictionary {
            let (Some(path), Some(symbol)) = (&entry.consumer_path, &entry.consumer_symbol) else {
                assert_eq!(entry.class, MetadataClass::Advisory);
                continue;
            };
            let source = fs::read_to_string(path).unwrap();
            assert!(source.contains(symbol), "{path} lacks {symbol}");
        }
        let declared = ir
            .semantic_type_bindings
            .iter()
            .map(|binding| binding.semantic_type.as_str())
            .collect::<BTreeSet<_>>();
        let used = ir
            .tables
            .iter()
            .flat_map(|table| &table.columns)
            .filter_map(|column| column.semantic_type.as_deref())
            .collect::<BTreeSet<_>>();
        assert_eq!(declared, used);
    }

    #[test]
    fn wp57_negative_zero_state() {
        let mut unknown_semantic_type = authority();
        unknown_semantic_type.tables[0].columns[0].semantic_type =
            Some("enum:not_registered".to_owned());
        assert!(unknown_semantic_type.validate().is_err());

        let mut widened = authority();
        widened.schema_evolution_policy.allow_type_widening = true;
        assert!(widened.validate().is_err());

        let ddl =
            String::from_utf8(render_ddl(&SchemaDriver.plan(Path::new(".")).unwrap())).unwrap();
        assert!(!ddl.contains("PRAGMA foreign_keys=ON"));
        assert!(ddl.contains("foreign keys are generated as application contracts"));
    }

    #[test]
    fn wp57_operational_acceptance() {
        let plan = SchemaDriver.plan(Path::new(".")).unwrap();
        let outputs = SchemaDriver::outputs(&plan).unwrap();
        let paths = outputs
            .iter()
            .map(|(path, _)| path.display())
            .collect::<BTreeSet<_>>();
        assert!(paths.contains(RUST_ROW_ENCODERS_PATH));
        assert!(paths.contains(EVOLUTION_POLICY_PATH));
        assert!(paths.contains(VALIDATION_PATH));

        let first = render_row_encoders(&plan.ir, &plan.source_digest).unwrap();
        let second = render_row_encoders(&plan.ir, &plan.source_digest).unwrap();
        assert_eq!(first, second);
        let rendered = String::from_utf8(first).unwrap();
        for function in [
            "encode_owners",
            "encode_capability_statuses",
            "encode_entities",
            "encode_relations",
            "encode_properties",
            "encode_evidence",
            "encode_source_files",
            "encode_source_tokens",
            "encode_source_annotations",
            "encode_syntax_details",
            "encode_type_details",
            "encode_type_fact_details",
        ] {
            assert!(rendered.contains(function), "missing {function}");
        }
    }

    #[test]
    fn observation_schema_contract_ir_composition() {
        let ir = authority();
        ir.validate().unwrap();
        let schemas = ir
            .provider_observation_schemas
            .iter()
            .map(|schema| {
                (
                    schema.provider_id.as_str(),
                    schema.observation_family_code,
                    schema.fields.len(),
                )
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            schemas,
            BTreeSet::from([("pyrefly-python", 110, 5), ("rustc-mir", 120, 9)])
        );
        let pyrefly = ir
            .provider_observation_schemas
            .iter()
            .find(|schema| schema.provider_id == "pyrefly-python")
            .unwrap();
        assert!(pyrefly.fields.iter().any(|field| {
            field.name == "type_table_json"
                && field.logical_type == ProviderObservationLogicalType::Binary
        }));
    }

    #[test]
    fn common_semantic_type_tables_bind_canonical_type_authority() {
        let ir = authority();
        ir.validate().unwrap();
        let type_detail = ir
            .tables
            .iter()
            .find(|table| table.name == "type_detail")
            .unwrap();
        let type_fact_detail = ir
            .tables
            .iter()
            .find(|table| table.name == "type_fact_detail")
            .unwrap();
        assert_eq!(type_detail.table_code, 180);
        assert_eq!(type_fact_detail.table_code, 190);
        assert_eq!(
            type_detail.partition_columns,
            ["type_kind_code", "owner_bucket"]
        );
        assert_eq!(type_fact_detail.partition_columns, ["owner_bucket"]);
        assert_eq!(type_detail.row_encoder, Some(RowEncoderKind::TypeDetails));
        assert_eq!(
            type_fact_detail.row_encoder,
            Some(RowEncoderKind::TypeFactDetails)
        );
        assert!(ir.semantic_type_bindings.iter().any(|binding| {
            binding.semantic_type == "identity:type-constructor"
                && binding.authority == SemanticAuthority::TypeAlgebra
                && binding.domain.as_deref() == Some("TYPE_CONSTRUCTOR")
        }));
        assert!(ir.semantic_authorities.iter().any(|authority| {
            authority.authority == SemanticAuthority::TypeAlgebra
                && authority.artifact_id == "codefabric.identity.type-algebra-v1"
        }));
    }

    #[test]
    fn provider_observation_schema_projection_parity() {
        let ir = authority();
        let rendered = String::from_utf8(render_rust(&ir)).unwrap();
        for schema in &ir.provider_observation_schemas {
            let descriptor = provider_observation_descriptor(schema);
            let digest = codefabric::integrity::framed_digest(descriptor.as_bytes());
            assert!(rendered.contains(&descriptor), "{}", schema.schema_id);
            assert!(rendered.contains(&digest), "{}", schema.schema_id);
        }
        let sidecar = fs::read_to_string("pyrefly-sidecar/src/pyrefly_link.rs").unwrap();
        let extractor = fs::read_to_string("rustc-extractor/src/protocol.rs").unwrap();
        let daemon = fs::read_to_string("src/pyrefly_service.rs").unwrap();
        assert!(sidecar.contains("/../src/generated/model_schema_tables.rs"));
        assert!(extractor.contains("/../src/generated/model_schema_tables.rs"));
        assert!(daemon.contains("PROVIDER_OBSERVATION_SCHEMAS"));
    }

    #[test]
    fn handwritten_observation_schema_falsification() {
        assert!(
            !Path::new("contracts/schema/provider-observations/pyrefly-module-v1.json").exists()
        );
        for path in [
            "pyrefly-sidecar/src/pyrefly_link.rs",
            "src/pyrefly_service.rs",
        ] {
            let source = fs::read_to_string(path).unwrap();
            assert!(
                !source.contains("contracts/schema/provider-observations"),
                "{path}"
            );
        }
        let ir = authority();
        let baseline = render_rust(&ir);
        let mut drifted = ir;
        drifted.provider_observation_schemas[0].fields[0].name = "drifted_module_id".to_owned();
        assert_ne!(baseline, render_rust(&drifted));
    }

    #[test]
    fn successor_intake_operational_gate() {
        let fragments =
            super::super::semantic_fragment_driver::SemanticFragmentSet::load(Path::new("."))
                .unwrap();
        assert_eq!(
            fs::read(super::super::semantic_fragment_driver::JSON_PROJECTION_PATH).unwrap(),
            fragments.render_json().unwrap()
        );
        assert_eq!(
            fs::read(super::super::semantic_fragment_driver::RUST_PROJECTION_PATH).unwrap(),
            rustfmt_source(fragments.render_rust().as_bytes()).unwrap()
        );

        let descriptor = SchemaDriver.describe().unwrap();
        let sources = descriptor
            .sources
            .iter()
            .map(SafeOutputPath::display)
            .collect::<BTreeSet<_>>();
        for path in super::super::semantic_fragment_driver::FRAGMENT_PATHS {
            assert!(sources.contains(path));
        }
        let justfile = fs::read_to_string("justfile").unwrap();
        for recipe in [
            "wave8-integration-check:",
            "property-registry-closure-check:",
            "semantic-provider-legacy-zero-state-check",
        ] {
            assert!(justfile.contains(recipe), "missing {recipe}");
        }
        for path in [
            "src/core_facts.rs",
            "src/analysis_context.rs",
            "src/operational_store.rs",
            "src/lifecycle.rs",
        ] {
            assert!(
                fs::read_to_string(path)
                    .unwrap()
                    .contains("semantic_lane_fragments"),
                "{path} does not consume the frozen fragment projection"
            );
        }
        let state: Value = serde_json::from_slice(
            &fs::read("docs/plans/state/codefabric-waves-8-12-semantic-profiles_v2_state.json")
                .unwrap(),
        )
        .unwrap();
        assert!(
            state["baseline_failures"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
        assert!(
            state["discovered_obligations"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
    }
}
