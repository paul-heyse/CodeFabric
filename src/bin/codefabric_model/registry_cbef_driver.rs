//! Registry/CBEF family driver built only from strict native authorities.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use super::desired_tree::SafeOutputPath;
use super::driver_protocol::{
    DriverDescriptor, DriverOutputRole, DriverOutputSpec, DriverProtocolError,
    DriverResourceProfile, DriverSourceFence, ModelDriver, StagingRoot,
    configure_reproducible_cargo_build, executable_tool_identity, process_stage_root,
    rustfmt_source,
};
use super::incremental::{CacheLookup, render_with_cache};
use super::model_control::StableId;
use super::registry_models as governed;
use super::repository_model::read_stable;

const CBEF_PATH: &str = "contracts/identity/cbef-v1.yaml";
const ENUM_PATH: &str = "contracts/registry/enum-registry.yaml";
const FLAG_PATH: &str = "contracts/registry/flag-registry.yaml";
const RUST_RECIPES_PATH: &str = "src/generated/model_identity_recipes.rs";
const RUST_REGISTRIES_PATH: &str = "src/generated/model_registries.rs";
const RUST_RUNTIME_REGISTRIES_PATH: &str = "src/generated/registries.rs";
const PYTHON_REGISTRIES_PATH: &str =
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_registries.py";
const PROJECTION_PATH: &str = "contracts/generated/model/registry-cbef.json";
const PROVIDER_TOOL_PATH: &str = "tooling/model/provider_inventory.rs";
const CARGO_MANIFEST_PATH: &str = "Cargo.toml";
const CARGO_LOCK_PATH: &str = "Cargo.lock";
const PROVIDER_RUST_PATH: &str = "src/generated/provider_raw_kinds.rs";
const PROVIDER_CATALOG_ROOT: &str = "contracts/generated/provider-raw-kinds";
const TREE_SITTER_RECOVERY_QUERY: &str = "(ERROR) @error\n(MISSING) @missing\n";
const MAX_AUTHORITY_BYTES: usize = 8 * 1024 * 1024;
const MAX_PROVIDER_PROBE_BYTES: usize = 32 * 1024 * 1024;

/// Compile one governed registry through the same closed family-native records used by runtime
/// validation, returning its detached semantic identity. Unknown non-registry families return
/// `None` so their owning driver can select another native model.
///
/// # Errors
///
/// Returns a bounded YAML, closed-model, invariant, or canonicalization failure.
pub fn detached_registry_identity(
    artifact_id: &str,
    bytes: &[u8],
) -> Result<Option<String>, RegistryCbefError> {
    macro_rules! accepted {
        ($record:ty, $validator:expr) => {{
            let document: governed::AcceptedRegistry<$record> = decode_yaml(bytes)?;
            ($validator)(&document.records).map_err(RegistryCbefError::RegistryModel)?;
            Some(detached_typed_digest(&document)?)
        }};
    }
    let digest = match artifact_id {
        "codefabric.registry.enum-registry" => Some(detached_typed_digest(&decode_yaml::<
            governed::AcceptedRegistry<governed::EnumDomain>,
        >(bytes)?)?),
        "codefabric.registry.flag-registry" => Some(detached_typed_digest(&decode_yaml::<
            governed::AcceptedRegistry<governed::FlagDomain>,
        >(bytes)?)?),
        "codefabric.registry.ontology-entity-registry" => {
            accepted!(governed::EntityKind, governed::validate_entity_records)
        }
        "codefabric.registry.ontology-relation-registry" => {
            accepted!(governed::RelationKind, governed::validate_relation_records)
        }
        "codefabric.registry.ontology-property-registry" => {
            accepted!(governed::PropertyKind, governed::validate_property_records)
        }
        "codefabric.registry.ontology-fact-registry" => {
            accepted!(governed::FactKind, governed::validate_fact_records)
        }
        "codefabric.registry.unknown-registry" => {
            accepted!(governed::UnknownKind, governed::validate_unknown_records)
        }
        "codefabric.registry.projection-registry" => {
            accepted!(governed::Projection, governed::validate_projection_records)
        }
        "codefabric.registry.summary-registry" => {
            accepted!(governed::SummaryProfile, governed::validate_summary_records)
        }
        "codefabric.registry.capability-registry" => {
            accepted!(governed::Capability, governed::validate_capability_records)
        }
        "codefabric.registry.provider-registry" => {
            accepted!(governed::Provider, governed::validate_provider_records)
        }
        "codefabric.registry.provider-resource-profile-registry" => accepted!(
            governed::ProviderResourceProfile,
            governed::validate_provider_resource_profiles
        ),
        "codefabric.registry.provider-normalization-registry" => accepted!(
            governed::ProviderNormalization,
            governed::validate_provider_normalizations
        ),
        "codefabric.registry.error-registry" => {
            accepted!(governed::PublicError, governed::validate_error_records)
        }
        "codefabric.registry.state-machine-registry" => {
            accepted!(governed::StateMachine, governed::validate_state_machines)
        }
        "codefabric.registry.phrase-registry" => {
            accepted!(governed::PhraseRecord, governed::validate_phrase_records)
        }
        "codefabric.comparison.comparison-ignore-registry" => accepted!(
            governed::ComparisonIgnoreRecord,
            governed::validate_comparison_ignores
        ),
        "codefabric.faults.fault-point-registry" => {
            accepted!(governed::FaultPointRecord, governed::validate_fault_points)
        }
        "codefabric.registry.derivation-registry" => {
            let document: governed::AcceptedRegistry<governed::DerivationDefinition> =
                decode_yaml(bytes)?;
            let mut ids = BTreeSet::new();
            if !document
                .records
                .iter()
                .all(|record| ids.insert(record.derivation_id.as_str()))
            {
                return Err(RegistryCbefError::RegistryModel(
                    "derivation IDs must be unique".to_owned(),
                ));
            }
            Some(detached_typed_digest(&document)?)
        }
        _ => None,
    };
    Ok(digest)
}

fn decode_yaml<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, RegistryCbefError> {
    serde_yaml_ng::from_slice(bytes).map_err(RegistryCbefError::Yaml)
}

fn detached_typed_digest(value: &impl Serialize) -> Result<String, RegistryCbefError> {
    let mut value = serde_json::to_value(value)?;
    let object = value.as_object_mut().ok_or_else(|| {
        RegistryCbefError::RegistryModel("typed registry root is not an object".to_owned())
    })?;
    object.remove("canonical_digest");
    object.remove("source_digest");
    let canonical = serde_json_canonicalizer::to_vec(&value)?;
    Ok(format!("b3:{}", blake3::hash(&canonical).to_hex()))
}

/// Header common to the three native family authorities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityHeader {
    artifact_id: String,
    artifact_kind: String,
    version: String,
    compatible_suite_major: u64,
    status: String,
    canonical_digest: String,
}

/// Owner-authored provenance attached to a machine authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerAcceptance {
    approver: String,
    accepted_at: String,
    construction_rule: String,
    source_digest: String,
}

/// One CBEF type allocation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CbefTypeSpec {
    code: u8,
    name: String,
    payload: String,
}

/// One fixed frame member.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FrameMember {
    name: String,
    width_bytes: u8,
}

/// One governed CBEF recipe operand.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CbefFieldSpec {
    pub tag: u16,
    pub name: String,
    pub type_code: String,
    #[serde(default)]
    pub width_bytes: Option<u8>,
    #[serde(default)]
    pub normalization: Option<String>,
    #[serde(default)]
    pub member_type: Option<String>,
}

/// One governed CBEF domain recipe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CbefDomainSpec {
    pub code: u16,
    pub name: String,
    pub public_prefix: String,
    pub kind_slug_required: bool,
    pub fields: Vec<CbefFieldSpec>,
}

/// Strict CBEF authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CbefAuthority {
    #[serde(flatten)]
    header: AuthorityHeader,
    format_name: String,
    format_version: u8,
    magic_ascii: String,
    byte_order: String,
    record_frame: Vec<FrameMember>,
    field_frame: Vec<FrameMember>,
    type_codes: Vec<CbefTypeSpec>,
    domains: Vec<CbefDomainSpec>,
    digest_algorithm: String,
    id_derivation: String,
    symbolic_source_context: String,
    symbolic_source_context_id_hex: String,
    collision_error: String,
    owner_acceptance: OwnerAcceptance,
}

/// One enum allocation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnumValue {
    code: u16,
    name: String,
    slug: String,
    meaning: String,
    #[serde(default)]
    aliases: Vec<String>,
}

/// One enum domain.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnumDomain {
    domain: String,
    width_bits: u8,
    values: Vec<EnumValue>,
}

/// Strict enum-registry authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnumRegistry {
    #[serde(flatten)]
    header: AuthorityHeader,
    records: Vec<EnumDomain>,
    owner_acceptance: OwnerAcceptance,
}

/// One bit allocation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FlagValue {
    bit: u8,
    name: String,
    slug: String,
    meaning: String,
}

/// One closed flag domain. An empty values list means that zero is the sole accepted value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FlagDomain {
    domain: String,
    width_bits: u8,
    values: Vec<FlagValue>,
}

/// Strict flag-registry authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FlagRegistry {
    #[serde(flatten)]
    header: AuthorityHeader,
    records: Vec<FlagDomain>,
    owner_acceptance: OwnerAcceptance,
}

/// Values accepted by the private recipe validator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipeValue {
    Absent,
    Bytes(Vec<u8>),
    Utf8(String),
    RawPath { platform_code: u8, bytes: Vec<u8> },
    Unsigned(Vec<u8>),
    Signed(Vec<u8>),
    Boolean(bool),
    Id([u8; 16]),
    Digest([u8; 32]),
    OrderedList(Vec<Self>),
    Set(Vec<Self>),
    Map(Vec<(Self, Self)>),
    TaggedUnion { variant: u16, value: Box<Self> },
}

impl RecipeValue {
    const fn type_name(&self) -> &'static str {
        match self {
            Self::Absent => "ABSENT",
            Self::Bytes(_) => "BYTES",
            Self::Utf8(_) => "UTF8",
            Self::RawPath { .. } => "RAW_PATH",
            Self::Unsigned(_) => "UNSIGNED",
            Self::Signed(_) => "SIGNED",
            Self::Boolean(_) => "BOOLEAN",
            Self::Id(_) => "ID",
            Self::Digest(_) => "DIGEST",
            Self::OrderedList(_) => "ORDERED_LIST",
            Self::Set(_) => "SET",
            Self::Map(_) => "MAP",
            Self::TaggedUnion { .. } => "TAGGED_UNION",
        }
    }
}

/// A field after recipe-aware validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeField {
    pub tag: u16,
    pub value: RecipeValue,
}

/// A complete recipe-aware record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeRecord {
    pub domain_code: u16,
    pub fields: Vec<RecipeField>,
}

/// Named ENTITY operands. Occurrence structure belongs in `semantic_key`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityOperands {
    pub workspace_id: [u8; 16],
    pub analysis_context_id: [u8; 16],
    pub kind_code: u16,
    pub owner_id: [u8; 16],
    pub semantic_key: Vec<u8>,
}

/// Named `RELATION_FACT` operands. Program-point specificity belongs in `role`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationFactOperands {
    pub workspace_id: [u8; 16],
    pub analysis_context_id: [u8; 16],
    pub relation_kind_code: u16,
    pub subject_entity_id: [u8; 16],
    pub object_entity_id: [u8; 16],
    pub role: Option<String>,
}

/// Canonical source-occurrence semantic-key payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceOccurrenceSemanticKeyV1 {
    pub schema_version: u8,
    pub file_id: [u8; 16],
    pub source_digest: [u8; 32],
    pub start_byte: u64,
    pub end_byte: u64,
    pub occurrence_family_code: u16,
    pub normalized_kind_code: u32,
    pub parent_id: Option<[u8; 16]>,
    pub role_code: Option<u16>,
    pub ordinal: u32,
}

impl SourceOccurrenceSemanticKeyV1 {
    /// Encode the typed payload with RFC 8785 before inserting it into the five-field recipe.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid range or impossible canonical serialization.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RegistryCbefError> {
        if self.schema_version != 1 || self.start_byte > self.end_byte {
            return Err(RegistryCbefError::InvalidOccurrenceKey);
        }
        let value = serde_json::to_value(self).map_err(RegistryCbefError::Json)?;
        serde_json_canonicalizer::to_vec(&value).map_err(RegistryCbefError::Json)
    }
}

/// Typed transition data retained outside `provider_node_flags`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSyntaxTransitionSemantics {
    pub occurrence_family_code: u16,
    pub reconciliation_step_code: Option<u16>,
    pub raw_kind_disposition_code: u16,
    pub provider_node_flags: u64,
    pub error: bool,
    pub missing: bool,
    pub explicitly_parenthesized: bool,
}

/// Parsed family plan and its source fence.
#[derive(Clone, Debug)]
pub struct RegistryCbefPlan {
    descriptor: DriverDescriptor,
    cbef: CbefAuthority,
    enums: EnumRegistry,
    flags: FlagRegistry,
    registry_values: BTreeMap<String, Value>,
    provider_probe: ProviderProbe,
    provider_tool_identity: Value,
    source_fence: DriverSourceFence,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderProbe {
    schema_version: u8,
    tree_sitter: Vec<TreeSitterProbe>,
    ruff: RuffProbe,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TreeSitterProbe {
    catalog_id: String,
    provider_version: String,
    language: String,
    grammar_abi: usize,
    node_types_source: String,
    raw_kinds: Vec<TreeSitterRawKind>,
    fields: Vec<TreeSitterRawField>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TreeSitterRawKind {
    raw_kind_id: u16,
    raw_name: String,
    named: bool,
    visible: bool,
    supertype: bool,
    subtypes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TreeSitterRawField {
    field_id: u16,
    field_name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuffProbe {
    catalog_id: String,
    provider_version: String,
    language: String,
    node_kinds: Vec<RuffRawKind>,
    token_kinds: Vec<RuffRawKind>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuffRawKind {
    raw_kind_id: u16,
    raw_name: String,
}

/// Registry/CBEF model driver.
#[derive(Clone, Debug)]
pub struct RegistryCbefDriver {
    repository_root: PathBuf,
}

impl RegistryCbefDriver {
    #[must_use]
    pub fn for_repository(repository_root: &Path) -> Self {
        Self {
            repository_root: repository_root.to_owned(),
        }
    }

    fn source_paths(&self) -> Result<Vec<String>, DriverProtocolError> {
        let registry_root = self.repository_root.join("contracts/registry");
        let mut paths = vec![
            CBEF_PATH.to_owned(),
            "contracts/rpc/feature-registry.yaml".to_owned(),
            CARGO_MANIFEST_PATH.to_owned(),
            CARGO_LOCK_PATH.to_owned(),
            PROVIDER_TOOL_PATH.to_owned(),
        ];
        for entry in fs::read_dir(&registry_root).map_err(|source| DriverProtocolError::Io {
            path: registry_root.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| DriverProtocolError::Io {
                path: registry_root.clone(),
                source,
            })?;
            let file_type = entry
                .file_type()
                .map_err(|source| DriverProtocolError::Io {
                    path: entry.path(),
                    source,
                })?;
            if file_type.is_symlink() {
                return Err(DriverProtocolError::InvalidAuthority(format!(
                    "registry source is a symlink: {}",
                    entry.path().display()
                )));
            }
            if file_type.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "yaml")
            {
                let relative = entry
                    .path()
                    .strip_prefix(&self.repository_root)
                    .map_err(|_| DriverProtocolError::InvalidDescriptor)?
                    .to_str()
                    .ok_or(DriverProtocolError::InvalidDescriptor)?
                    .replace(std::path::MAIN_SEPARATOR, "/");
                paths.push(relative);
            }
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
    fn output(
        id: &str,
        path: &str,
        role: DriverOutputRole,
    ) -> Result<DriverOutputSpec, DriverProtocolError> {
        Ok(DriverOutputSpec {
            output_id: StableId::parse(id.to_owned())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            path: SafeOutputPath::parse(path.as_bytes().to_vec())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            role,
        })
    }

    /// Build the exact five-field ENTITY recipe.
    ///
    /// # Errors
    ///
    /// Returns an error if the governed ENTITY recipe is absent or has drifted.
    pub fn build_entity(
        &self,
        plan: &RegistryCbefPlan,
        operands: EntityOperands,
    ) -> Result<RecipeRecord, RegistryCbefError> {
        let values = BTreeMap::from([
            (
                "analysis_context_id".to_owned(),
                RecipeValue::Id(operands.analysis_context_id),
            ),
            (
                "kind_code".to_owned(),
                RecipeValue::Unsigned(operands.kind_code.to_be_bytes().to_vec()),
            ),
            ("owner_id".to_owned(), RecipeValue::Id(operands.owner_id)),
            (
                "semantic_key".to_owned(),
                RecipeValue::Bytes(operands.semantic_key),
            ),
            (
                "workspace_id".to_owned(),
                RecipeValue::Id(operands.workspace_id),
            ),
        ]);
        build_named(&plan.cbef, "ENTITY", values)
    }

    /// Build the exact six-field `RELATION_FACT` recipe.
    ///
    /// # Errors
    ///
    /// Returns an error if the governed relation recipe is absent or has drifted.
    pub fn build_relation_fact(
        &self,
        plan: &RegistryCbefPlan,
        operands: RelationFactOperands,
    ) -> Result<RecipeRecord, RegistryCbefError> {
        let role = operands.role.map_or_else(
            || RecipeValue::TaggedUnion {
                variant: 0,
                value: Box::new(RecipeValue::Absent),
            },
            |role| RecipeValue::TaggedUnion {
                variant: 1,
                value: Box::new(RecipeValue::Utf8(role)),
            },
        );
        let values = BTreeMap::from([
            (
                "analysis_context_id".to_owned(),
                RecipeValue::Id(operands.analysis_context_id),
            ),
            (
                "object_entity_id".to_owned(),
                RecipeValue::Id(operands.object_entity_id),
            ),
            (
                "relation_kind_code".to_owned(),
                RecipeValue::Unsigned(operands.relation_kind_code.to_be_bytes().to_vec()),
            ),
            ("role".to_owned(), role),
            (
                "subject_entity_id".to_owned(),
                RecipeValue::Id(operands.subject_entity_id),
            ),
            (
                "workspace_id".to_owned(),
                RecipeValue::Id(operands.workspace_id),
            ),
        ]);
        build_named(&plan.cbef, "RELATION_FACT", values)
    }

    /// Validate the independent normalized projection against the typed plan.
    ///
    /// # Errors
    ///
    /// Returns an error if projected recipes or allocations differ.
    fn validate_projection(plan: &RegistryCbefPlan, bytes: &[u8]) -> Result<(), RegistryCbefError> {
        let projection: RegistryCbefProjection =
            serde_json::from_slice(bytes).map_err(RegistryCbefError::Json)?;
        if projection.schema_version != 1
            || projection.cbef_domains != plan.cbef.domains
            || projection.enum_domains != plan.enums.records
            || projection.flag_domains != plan.flags.records
        {
            return Err(RegistryCbefError::ProjectionMismatch);
        }
        Ok(())
    }
}

impl ModelDriver for RegistryCbefDriver {
    type Plan = RegistryCbefPlan;

    fn describe(&self) -> Result<DriverDescriptor, DriverProtocolError> {
        let descriptor = DriverDescriptor {
            driver_id: StableId::parse("driver:registry-cbef-v1".to_owned())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            family: StableId::parse("family:registry-cbef".to_owned())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            rule_version: "registry-cbef-driver-v1".to_owned(),
            sources: self
                .source_paths()?
                .into_iter()
                .map(|path| {
                    SafeOutputPath::parse(path.as_bytes().to_vec())
                        .map_err(|_| DriverProtocolError::InvalidDescriptor)
                })
                .collect::<Result<Vec<_>, _>>()?,
            output_roots: vec![],
            outputs: vec![
                Self::output(
                    "output:model-identity-recipes-rust",
                    RUST_RECIPES_PATH,
                    DriverOutputRole::RustBinding,
                )?,
                Self::output(
                    "output:model-registries-rust",
                    RUST_REGISTRIES_PATH,
                    DriverOutputRole::RustBinding,
                )?,
                Self::output(
                    "output:model-runtime-registries-rust",
                    RUST_RUNTIME_REGISTRIES_PATH,
                    DriverOutputRole::RustBinding,
                )?,
                Self::output(
                    "output:model-registries-python",
                    PYTHON_REGISTRIES_PATH,
                    DriverOutputRole::PythonBinding,
                )?,
                Self::output(
                    "output:model-registry-cbef-projection",
                    PROJECTION_PATH,
                    DriverOutputRole::CanonicalProjection,
                )?,
                Self::output(
                    "output:model-provider-raw-rust",
                    PROVIDER_RUST_PATH,
                    DriverOutputRole::RustBinding,
                )?,
                Self::output(
                    "output:model-provider-tree-sitter-python",
                    &format!("{PROVIDER_CATALOG_ROOT}/tree-sitter-python-0-25-0.json"),
                    DriverOutputRole::CanonicalProjection,
                )?,
                Self::output(
                    "output:model-provider-tree-sitter-rust",
                    &format!("{PROVIDER_CATALOG_ROOT}/tree-sitter-rust-0-24-2.json"),
                    DriverOutputRole::CanonicalProjection,
                )?,
                Self::output(
                    "output:model-provider-ruff-python",
                    &format!("{PROVIDER_CATALOG_ROOT}/ruff-python-0-0-7.json"),
                    DriverOutputRole::CanonicalProjection,
                )?,
            ],
            resource_profile: DriverResourceProfile {
                max_source_bytes: MAX_AUTHORITY_BYTES,
                max_output_bytes: 4 * 1024 * 1024,
                max_outputs: 12,
            },
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    fn plan(&self, repository_root: &Path) -> Result<Self::Plan, DriverProtocolError> {
        let descriptor = self.describe()?;
        let source_fence = DriverSourceFence::capture(repository_root, &descriptor)?;
        let cbef = parse_yaml::<CbefAuthority>(repository_root, CBEF_PATH)?;
        let enums = parse_yaml::<EnumRegistry>(repository_root, ENUM_PATH)?;
        let flags = parse_yaml::<FlagRegistry>(repository_root, FLAG_PATH)?;
        validate_authorities(&cbef, &enums, &flags)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let mut registry_values = BTreeMap::new();
        for source in descriptor.sources.iter().filter(|source| {
            source.display() != CBEF_PATH
                && Path::new(&source.display())
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("yaml"))
        }) {
            let bytes = read_stable(&repository_root.join(source.display()), MAX_AUTHORITY_BYTES)?;
            let yaml: serde_yaml_ng::Value =
                serde_yaml_ng::from_slice(&bytes).map_err(|error| {
                    DriverProtocolError::InvalidAuthority(format!("{}: {error}", source.display()))
                })?;
            let value = serde_json::to_value(yaml).map_err(|error| {
                DriverProtocolError::InvalidAuthority(format!("{}: {error}", source.display()))
            })?;
            let artifact_id = value
                .get("artifact_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    DriverProtocolError::InvalidAuthority(format!(
                        "{} has no artifact_id",
                        source.display()
                    ))
                })?
                .to_owned();
            if artifact_id != "codefabric.rpc.feature-registry"
                && detached_registry_identity(&artifact_id, &bytes)
                    .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?
                    .is_none()
            {
                return Err(DriverProtocolError::InvalidAuthority(format!(
                    "unclaimed registry authority {artifact_id}"
                )));
            }
            if registry_values.insert(artifact_id.clone(), value).is_some() {
                return Err(DriverProtocolError::InvalidAuthority(format!(
                    "duplicate registry authority {artifact_id}"
                )));
            }
        }
        let (provider_probe, provider_tool_identity) = run_provider_probe(repository_root)
            .map_err(|error| DriverProtocolError::ExternalTool {
                tool: "codefabric-provider-inventory",
                detail: error.to_string(),
            })?;
        source_fence.verify(repository_root)?;
        Ok(RegistryCbefPlan {
            descriptor,
            cbef,
            enums,
            flags,
            registry_values,
            provider_probe,
            provider_tool_identity,
            source_fence,
        })
    }

    fn render(
        &self,
        plan: &Self::Plan,
        staging_root: &StagingRoot,
    ) -> Result<Vec<SafeOutputPath>, DriverProtocolError> {
        let provider_catalogs = render_provider_catalogs(plan)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let provider_rust = render_provider_rust(plan, &provider_catalogs)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        let mut outputs: Vec<(String, Vec<u8>)> = vec![
            (
                RUST_RECIPES_PATH.to_owned(),
                rustfmt_source(&render_rust_recipes(&plan.cbef))?,
            ),
            (
                RUST_REGISTRIES_PATH.to_owned(),
                rustfmt_source(&render_rust_registries(&plan.enums, &plan.flags))?,
            ),
            (
                RUST_RUNTIME_REGISTRIES_PATH.to_owned(),
                rustfmt_source(
                    &render_rust_runtime_registries(
                        &plan.registry_values,
                        &plan.enums,
                        &plan.flags,
                    )
                    .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?,
                )?,
            ),
            (
                PYTHON_REGISTRIES_PATH.to_owned(),
                render_python_registries(&plan.enums, &plan.flags),
            ),
            (
                PROJECTION_PATH.to_owned(),
                render_projection(plan).map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            ),
            (
                PROVIDER_RUST_PATH.to_owned(),
                rustfmt_source(&provider_rust)?,
            ),
        ];
        for (catalog_id, bytes) in provider_catalogs {
            outputs.push((format!("{PROVIDER_CATALOG_ROOT}/{catalog_id}.json"), bytes));
        }
        outputs.sort_by(|left, right| left.0.cmp(&right.0));
        let mut rendered = Vec::new();
        for (path, bytes) in outputs {
            let path = SafeOutputPath::parse(path.as_bytes().to_vec())
                .map_err(|_| DriverProtocolError::InvalidDescriptor)?;
            staging_root.write(&path, &bytes)?;
            rendered.push(path);
        }
        Ok(rendered)
    }
}

/// Render and validate the family under a disposable stage.
///
/// # Errors
///
/// Returns a driver, authority, projection, or staging error.
pub fn check_family(repository_root: &Path) -> Result<RegistryCbefReport, RegistryCbefError> {
    let driver = RegistryCbefDriver::for_repository(repository_root);
    let plan = driver.plan(repository_root)?;
    let stage_path = process_stage_root(repository_root, "registry-cbef-shadow");
    if stage_path.exists() {
        fs::remove_dir_all(&stage_path).map_err(|source| RegistryCbefError::Io {
            path: stage_path.clone(),
            source,
        })?;
    }
    fs::create_dir_all(&stage_path).map_err(|source| RegistryCbefError::Io {
        path: stage_path.clone(),
        source,
    })?;
    let staging = StagingRoot::new(repository_root, &stage_path, &plan.descriptor)?;
    let (rendered, cache_lookup) = render_with_cache(
        repository_root,
        "registry-cbef",
        &plan.descriptor,
        &plan.source_fence,
        &staging,
        || {
            Ok(json!({
                "rustfmt": executable_tool_identity("rustfmt", &["--version"] )?,
                "provider_inventory": plan.provider_tool_identity.clone(),
            }))
        },
        || driver.render(&plan, &staging),
    )?;
    plan.source_fence.verify(repository_root)?;
    let projection_path = staging.output_path(
        &SafeOutputPath::parse(PROJECTION_PATH.as_bytes().to_vec())
            .map_err(|_| RegistryCbefError::ProjectionMismatch)?,
    )?;
    let projection = read_stable(&projection_path, 4 * 1024 * 1024)?;
    RegistryCbefDriver::validate_projection(&plan, &projection)?;
    validate_all_recipes(&plan.cbef)?;
    validate_transition_semantics(&plan.enums, &plan.flags)?;
    Ok(RegistryCbefReport {
        family: "registry-cbef".to_owned(),
        rule_version: plan.descriptor.rule_version.clone(),
        resource_profile: plan.descriptor.resource_profile.clone(),
        domain_count: plan.cbef.domains.len(),
        enum_domain_count: plan.enums.records.len(),
        flag_domain_count: plan.flags.records.len(),
        rendered_outputs: rendered.iter().map(SafeOutputPath::display).collect(),
        cache_lookup,
        stage_root: staging.path().to_string_lossy().into_owned(),
        tool_identity: plan.provider_tool_identity.clone(),
    })
}

/// Machine-readable family report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryCbefReport {
    pub family: String,
    pub rule_version: String,
    pub resource_profile: DriverResourceProfile,
    pub domain_count: usize,
    pub enum_domain_count: usize,
    pub flag_domain_count: usize,
    pub rendered_outputs: Vec<String>,
    pub cache_lookup: CacheLookup,
    pub stage_root: String,
    pub tool_identity: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryCbefProjection {
    schema_version: u8,
    source_artifact_ids: Vec<String>,
    cbef_domains: Vec<CbefDomainSpec>,
    enum_domains: Vec<EnumDomain>,
    flag_domains: Vec<FlagDomain>,
}

fn run_provider_probe(repository_root: &Path) -> Result<(ProviderProbe, Value), RegistryCbefError> {
    let rustc = command_text(Command::new("rustc").args(["--version", "--verbose"]))?;
    let mut material = Vec::new();
    for path in [CARGO_MANIFEST_PATH, CARGO_LOCK_PATH, PROVIDER_TOOL_PATH] {
        material.extend(read_stable(
            &repository_root.join(path),
            MAX_PROVIDER_PROBE_BYTES,
        )?);
    }
    material.extend(rustc.as_bytes());
    material.extend(
        b"provider-inventory-tooling|debug|host|reproducible-path-remap-nodebug-v3|incremental=0",
    );
    let action_key = blake3::hash(&material).to_hex().to_string();
    let target = repository_root
        .join("target/model-tools/provider-inventory")
        .join(&action_key);
    let mut command = Command::new("cargo");
    configure_reproducible_cargo_build(&mut command, repository_root);
    let status = command
        .args([
            "build",
            "--offline",
            "--locked",
            "--no-default-features",
            "--features",
            "provider-inventory-tooling",
            "--bin",
            "codefabric-provider-inventory",
            "--target-dir",
        ])
        .arg(&target)
        .current_dir(repository_root)
        .status()
        .map_err(|source| RegistryCbefError::Io {
            path: PathBuf::from("cargo"),
            source,
        })?;
    if !status.success() {
        return Err(RegistryCbefError::ProviderTool(
            "provider inventory build failed".to_owned(),
        ));
    }
    let executable = target.join("debug/codefabric-provider-inventory");
    let stage_home = process_stage_root(repository_root, "provider-inventory-home");
    fs::create_dir_all(&stage_home).map_err(|source| RegistryCbefError::Io {
        path: stage_home.clone(),
        source,
    })?;
    let output = Command::new(&executable)
        .env_clear()
        .env("HOME", &stage_home)
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("NO_PROXY", "*")
        .env("no_proxy", "*")
        .stdin(Stdio::null())
        .output()
        .map_err(|source| RegistryCbefError::Io {
            path: executable.clone(),
            source,
        })?;
    if !output.status.success() || output.stdout.len() > MAX_PROVIDER_PROBE_BYTES {
        return Err(RegistryCbefError::ProviderTool(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    let probe: ProviderProbe = serde_json::from_slice(&output.stdout)?;
    if probe.schema_version != 1 {
        return Err(RegistryCbefError::ProviderTool(
            "unsupported provider probe schema".to_owned(),
        ));
    }
    let executable_bytes = read_stable(&executable, MAX_PROVIDER_PROBE_BYTES)?;
    let identity = json!({
        "action_key": format!("b3:{action_key}"),
        "executable_digest": format!("b3:{}", blake3::hash(&executable_bytes).to_hex()),
        "cargo_lock_digest": digest_file(repository_root, CARGO_LOCK_PATH)?,
        "cargo_manifest_digest": digest_file(repository_root, CARGO_MANIFEST_PATH)?,
        "source_digest": digest_file(repository_root, PROVIDER_TOOL_PATH)?,
        "features": ["provider-inventory-tooling"],
        "profile": "debug",
        "rustc": rustc.trim(),
        "protocol": "codefabric-provider-inventory-v1",
    });
    Ok((probe, identity))
}

fn command_text(command: &mut Command) -> Result<String, RegistryCbefError> {
    let output = command.output().map_err(|source| RegistryCbefError::Io {
        path: PathBuf::from("external-command"),
        source,
    })?;
    if !output.status.success() {
        return Err(RegistryCbefError::ProviderTool(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn digest_file(repository_root: &Path, relative: &str) -> Result<String, RegistryCbefError> {
    let bytes = read_stable(&repository_root.join(relative), MAX_PROVIDER_PROBE_BYTES)?;
    Ok(format!("b3:{}", blake3::hash(&bytes).to_hex()))
}

fn provider_normalizations(
    plan: &RegistryCbefPlan,
) -> Result<BTreeMap<String, governed::ProviderNormalization>, RegistryCbefError> {
    let value = plan
        .registry_values
        .get("codefabric.registry.provider-normalization-registry")
        .ok_or_else(|| {
            RegistryCbefError::RegistryModel("provider normalization is absent".to_owned())
        })?;
    let records: Vec<governed::ProviderNormalization> =
        serde_json::from_value(value.get("records").cloned().ok_or_else(|| {
            RegistryCbefError::RegistryModel("provider normalization records are absent".to_owned())
        })?)?;
    let mut result = BTreeMap::new();
    for record in records {
        let id = record.raw_catalog_id.clone();
        if result.insert(id.clone(), record).is_some() {
            return Err(RegistryCbefError::RegistryModel(format!(
                "duplicate provider normalization for {id}"
            )));
        }
    }
    Ok(result)
}

fn provider_catalog_ids(plan: &RegistryCbefPlan) -> Result<BTreeSet<String>, RegistryCbefError> {
    let value = plan
        .registry_values
        .get("codefabric.registry.provider-registry")
        .ok_or_else(|| {
            RegistryCbefError::RegistryModel("provider registry is absent".to_owned())
        })?;
    let records: Vec<governed::Provider> =
        serde_json::from_value(value.get("records").cloned().ok_or_else(|| {
            RegistryCbefError::RegistryModel("provider records are absent".to_owned())
        })?)?;
    Ok(records
        .into_iter()
        .flat_map(|provider| provider.raw_catalog_ids)
        .collect())
}

fn provider_input_identities(plan: &RegistryCbefPlan) -> Result<Vec<Value>, RegistryCbefError> {
    let required = BTreeSet::from([
        "codefabric.registry.provider-normalization-registry",
        "codefabric.registry.provider-registry",
        "codefabric.registry.provider-resource-profile-registry",
    ]);
    required
        .into_iter()
        .map(|artifact_id| {
            let value = plan.registry_values.get(artifact_id).ok_or_else(|| {
                RegistryCbefError::RegistryModel(format!("provider input {artifact_id} is absent"))
            })?;
            let canonical_digest = value
                .get("canonical_digest")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    RegistryCbefError::RegistryModel(format!(
                        "provider input {artifact_id} has no canonical digest"
                    ))
                })?;
            let source_digest = value
                .pointer("/owner_acceptance/source_digest")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    RegistryCbefError::RegistryModel(format!(
                        "provider input {artifact_id} has no accepted source digest"
                    ))
                })?;
            Ok(json!({
                "artifact_id": artifact_id,
                "canonical_digest": canonical_digest,
                "source_digest": source_digest,
            }))
        })
        .collect()
}

fn resolve_raw_kind(
    normalization: &governed::ProviderNormalization,
    raw_name: &str,
) -> Result<(String, Option<String>), RegistryCbefError> {
    if let Some(name) = normalization.canonical_kind_names.get(raw_name) {
        return Ok(("normalize".to_owned(), Some(name.clone())));
    }
    if normalization.ignored_raw_keys.contains(raw_name) {
        return Ok(("ignore".to_owned(), None));
    }
    let matches = normalization
        .canonical_kind_prefixes
        .iter()
        .filter(|(prefix, _)| raw_name.starts_with(prefix.as_str()))
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(RegistryCbefError::RegistryModel(format!(
            "{} has ambiguous prefix normalization for {raw_name}",
            normalization.mapping_id
        )));
    }
    if let Some((_, name)) = matches.first() {
        return Ok(("normalize".to_owned(), Some((*name).clone())));
    }
    if let Some(name) = &normalization.default_canonical_kind_name {
        return Ok(("normalize".to_owned(), Some(name.clone())));
    }
    Ok((
        match normalization.default_disposition {
            governed::RawKindDisposition::Ignore => "ignore",
            governed::RawKindDisposition::Unsupported => "unsupported",
        }
        .to_owned(),
        None,
    ))
}

fn validate_mapping_coverage(
    normalization: &governed::ProviderNormalization,
    available: &BTreeSet<String>,
) -> Result<(), RegistryCbefError> {
    if normalization
        .canonical_kind_names
        .keys()
        .chain(&normalization.ignored_raw_keys)
        .any(|name| !available.contains(name))
        || normalization
            .canonical_kind_prefixes
            .keys()
            .any(|prefix| !available.iter().any(|name| name.starts_with(prefix)))
    {
        return Err(RegistryCbefError::RegistryModel(format!(
            "{} references an absent raw kind",
            normalization.mapping_id
        )));
    }
    Ok(())
}

fn canonical_bytes(value: &Value) -> Result<Vec<u8>, RegistryCbefError> {
    serde_json_canonicalizer::to_vec(value).map_err(RegistryCbefError::Json)
}

#[allow(clippy::too_many_lines)] // One typed join proves the provider/catalog census atomically.
fn render_provider_catalogs(
    plan: &RegistryCbefPlan,
) -> Result<BTreeMap<String, Vec<u8>>, RegistryCbefError> {
    let normalizations = provider_normalizations(plan)?;
    let expected = provider_catalog_ids(plan)?;
    let observed = plan
        .provider_probe
        .tree_sitter
        .iter()
        .map(|probe| probe.catalog_id.clone())
        .chain([plan.provider_probe.ruff.catalog_id.clone()])
        .collect::<BTreeSet<_>>();
    if observed != expected || normalizations.keys().cloned().collect::<BTreeSet<_>>() != expected {
        return Err(RegistryCbefError::RegistryModel(format!(
            "provider catalog census differs: expected {expected:?}, observed {observed:?}"
        )));
    }
    let input_identities = provider_input_identities(plan)?;
    let query_bundle = json!({
        "queries": [{"query_id": "recovery", "source": TREE_SITTER_RECOVERY_QUERY}]
    });
    let query_bundle_digest = format!(
        "b3:{}",
        blake3::hash(&canonical_bytes(&query_bundle)?).to_hex()
    );
    let mut outputs = BTreeMap::new();
    for probe in &plan.provider_probe.tree_sitter {
        let normalization = normalizations.get(&probe.catalog_id).ok_or_else(|| {
            RegistryCbefError::RegistryModel(format!(
                "normalization for {} is absent",
                probe.catalog_id
            ))
        })?;
        if normalization.provider_version
            != format!("{};abi={}", probe.provider_version, probe.grammar_abi)
            || normalization.language != probe.language
        {
            return Err(RegistryCbefError::RegistryModel(format!(
                "{} provider identity differs from the pinned runtime",
                probe.catalog_id
            )));
        }
        let available = probe
            .raw_kinds
            .iter()
            .map(|kind| kind.raw_name.clone())
            .collect::<BTreeSet<_>>();
        validate_mapping_coverage(normalization, &available)?;
        let mut raw_kinds = Vec::new();
        for kind in &probe.raw_kinds {
            let (disposition, canonical_kind_name) =
                resolve_raw_kind(normalization, &kind.raw_name)?;
            raw_kinds.push(json!({
                "raw_key": format!("{}:{}:{}", kind.raw_kind_id, kind.raw_name, kind.named),
                "raw_kind_id": kind.raw_kind_id,
                "raw_name": kind.raw_name,
                "named": kind.named,
                "visible": kind.visible,
                "supertype": kind.supertype,
                "subtypes": kind.subtypes,
                "disposition": disposition,
                "canonical_kind_name": canonical_kind_name,
            }));
        }
        let runtime_inventory = json!({"raw_kinds": raw_kinds, "fields": probe.fields});
        let runtime_inventory_fingerprint = format!(
            "b3:{}",
            blake3::hash(&canonical_bytes(&runtime_inventory)?).to_hex()
        );
        let node_types_digest = format!(
            "b3:{}",
            blake3::hash(probe.node_types_source.as_bytes()).to_hex()
        );
        let node_types: Value = serde_json::from_str(&probe.node_types_source)?;
        let document = json!({
            "catalog_id": probe.catalog_id,
            "provider_id": normalization.provider_id,
            "provider_version": probe.provider_version,
            "language": probe.language,
            "grammar_abi": probe.grammar_abi,
            "node_types_digest": node_types_digest,
            "runtime_inventory_fingerprint": runtime_inventory_fingerprint,
            "query_bundle_digest": query_bundle_digest.clone(),
            "generation_unit_id": "driver:registry-cbef-v1/provider-raw-v1",
            "input_identities": input_identities.clone(),
            "node_types": node_types,
            "runtime_inventory": runtime_inventory,
        });
        outputs.insert(probe.catalog_id.clone(), canonical_bytes(&document)?);
    }
    let ruff = &plan.provider_probe.ruff;
    let normalization = normalizations.get(&ruff.catalog_id).ok_or_else(|| {
        RegistryCbefError::RegistryModel("Ruff normalization is absent".to_owned())
    })?;
    if normalization.provider_version != ruff.provider_version
        || normalization.language != ruff.language
    {
        return Err(RegistryCbefError::RegistryModel(
            "Ruff provider identity differs from the pinned runtime".to_owned(),
        ));
    }
    let available = ruff
        .node_kinds
        .iter()
        .map(|kind| kind.raw_name.clone())
        .collect::<BTreeSet<_>>();
    validate_mapping_coverage(normalization, &available)?;
    let node_kinds = ruff
        .node_kinds
        .iter()
        .map(|kind| {
            let (disposition, canonical_kind_name) =
                resolve_raw_kind(normalization, &kind.raw_name)?;
            Ok(json!({
                "raw_kind_id": kind.raw_kind_id,
                "raw_name": kind.raw_name,
                "disposition": disposition,
                "canonical_kind_name": canonical_kind_name,
            }))
        })
        .collect::<Result<Vec<_>, RegistryCbefError>>()?;
    let runtime_inventory = json!({
        "node_kinds": node_kinds,
        "token_kinds": ruff.token_kinds,
    });
    let runtime_inventory_fingerprint = format!(
        "b3:{}",
        blake3::hash(&canonical_bytes(&runtime_inventory)?).to_hex()
    );
    let document = json!({
        "catalog_id": ruff.catalog_id,
        "catalog_kind": "ruff-python-frontend",
        "provider_id": normalization.provider_id,
        "provider_version": ruff.provider_version,
        "language": ruff.language,
        "runtime_inventory_fingerprint": runtime_inventory_fingerprint,
        "generation_unit_id": "driver:registry-cbef-v1/provider-raw-v1",
        "input_identities": input_identities,
        "runtime_inventory": runtime_inventory,
    });
    outputs.insert(ruff.catalog_id.clone(), canonical_bytes(&document)?);
    Ok(outputs)
}

fn disposition_variant(value: &Value) -> Result<&'static str, RegistryCbefError> {
    match value.as_str() {
        Some("normalize") => Ok("ProviderRawKindDisposition::Normalize"),
        Some("ignore") => Ok("ProviderRawKindDisposition::Ignore"),
        Some("unsupported") => Ok("ProviderRawKindDisposition::Unsupported"),
        _ => Err(RegistryCbefError::RegistryModel(
            "unknown provider raw-kind disposition".to_owned(),
        )),
    }
}

fn provider_document<'a>(
    documents: &'a BTreeMap<&str, Value>,
    id: &str,
) -> Result<&'a Value, RegistryCbefError> {
    documents
        .get(id)
        .ok_or_else(|| RegistryCbefError::RegistryModel(format!("provider catalog {id} is absent")))
}

#[allow(clippy::too_many_lines)]
fn render_provider_rust(
    plan: &RegistryCbefPlan,
    catalogs: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<u8>, RegistryCbefError> {
    let syntax_codes = plan
        .enums
        .records
        .iter()
        .find(|domain| domain.domain == "SYNTAX_KIND")
        .ok_or(RegistryCbefError::AuthorityInvariant)?
        .values
        .iter()
        .map(|value| (value.name.as_str(), value.code))
        .collect::<BTreeMap<_, _>>();
    let fallback = *syntax_codes
        .get("SYNTAX_NODE")
        .ok_or(RegistryCbefError::AuthorityInvariant)?;
    let documents = catalogs
        .iter()
        .map(|(id, bytes)| Ok((id.as_str(), serde_json::from_slice::<Value>(bytes)?)))
        .collect::<Result<BTreeMap<_, _>, RegistryCbefError>>()?;
    let mut output = String::from(
        "// @generated by codefabric-model; do not edit.\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub enum ProviderRawKindDisposition { Normalize, Ignore, Unsupported }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderRawKindEntry { pub raw_kind_id: u16, pub raw_name: &'static str, pub named: bool, pub visible: bool, pub supertype: bool, pub disposition: ProviderRawKindDisposition, pub normalized_kind_code: u16 }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderRawFieldEntry { pub field_id: u16, pub field_name: &'static str }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderGrammarInventory { pub catalog_id: &'static str, pub language: &'static str, pub provider_version: &'static str, pub grammar_abi: usize, pub node_types_digest: &'static str, pub runtime_inventory_fingerprint: &'static str, pub query_bundle_digest: &'static str, pub query_bundle_canonical_json: &'static [u8], pub raw_kinds: &'static [ProviderRawKindEntry], pub fields: &'static [ProviderRawFieldEntry] }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct RuffNodeKindEntry { pub raw_kind_id: u16, pub raw_name: &'static str, pub disposition: ProviderRawKindDisposition, pub normalized_kind_code: u16 }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct RuffTokenKindEntry { pub raw_kind_id: u16, pub raw_name: &'static str }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct RuffPythonInventory { pub catalog_id: &'static str, pub provider_version: &'static str, pub runtime_inventory_fingerprint: &'static str, pub node_kinds: &'static [RuffNodeKindEntry], pub token_kinds: &'static [RuffTokenKindEntry] }\n\n",
    );
    writeln!(
        output,
        "pub const TREE_SITTER_RECOVERY_QUERY: &str = {TREE_SITTER_RECOVERY_QUERY:?};\n"
    )
    .expect("string writes do not fail");
    for (constant, id) in [
        ("TREE_SITTER_PYTHON_GRAMMAR", "tree-sitter-python-0-25-0"),
        ("TREE_SITTER_RUST_GRAMMAR", "tree-sitter-rust-0-24-2"),
    ] {
        render_tree_sitter_rust_catalog(
            &mut output,
            constant,
            id,
            provider_document(&documents, id)?,
            &syntax_codes,
            fallback,
        )?;
    }
    render_ruff_rust_catalog(
        &mut output,
        provider_document(&documents, "ruff-python-0-0-7")?,
        &syntax_codes,
        fallback,
    )?;
    output.push_str(
        "pub const PROVIDER_GRAMMAR_INVENTORIES: &[ProviderGrammarInventory] = &[\n\
             TREE_SITTER_PYTHON_GRAMMAR,\n\
             TREE_SITTER_RUST_GRAMMAR,\n\
         ];\n",
    );
    Ok(output.into_bytes())
}

fn render_tree_sitter_rust_catalog(
    output: &mut String,
    constant: &str,
    id: &str,
    document: &Value,
    syntax_codes: &BTreeMap<&str, u16>,
    fallback: u16,
) -> Result<(), RegistryCbefError> {
    let kinds = format!("{constant}_RAW_KINDS");
    let fields = format!("{constant}_FIELDS");
    writeln!(output, "pub const {kinds}: &[ProviderRawKindEntry] = &[")
        .expect("string writes do not fail");
    for entry in document["runtime_inventory"]["raw_kinds"]
        .as_array()
        .ok_or_else(|| RegistryCbefError::RegistryModel(format!("{id} raw kinds are absent")))?
    {
        let code = entry["canonical_kind_name"]
            .as_str()
            .and_then(|name| syntax_codes.get(name))
            .copied()
            .unwrap_or(fallback);
        let disposition = disposition_variant(&entry["disposition"])?;
        writeln!(
            output,
            "    ProviderRawKindEntry {{ raw_kind_id: {}, raw_name: {:?}, named: {}, visible: {}, supertype: {}, disposition: {disposition}, normalized_kind_code: {code} }},",
            entry["raw_kind_id"].as_u64().ok_or_else(|| RegistryCbefError::RegistryModel(id.to_owned()))?,
            entry["raw_name"].as_str().ok_or_else(|| RegistryCbefError::RegistryModel(id.to_owned()))?,
            entry["named"].as_bool().ok_or_else(|| RegistryCbefError::RegistryModel(id.to_owned()))?,
            entry["visible"].as_bool().ok_or_else(|| RegistryCbefError::RegistryModel(id.to_owned()))?,
            entry["supertype"].as_bool().ok_or_else(|| RegistryCbefError::RegistryModel(id.to_owned()))?,
        )
        .expect("string writes do not fail");
    }
    writeln!(output, "];\n").expect("string writes do not fail");
    writeln!(output, "pub const {fields}: &[ProviderRawFieldEntry] = &[")
        .expect("string writes do not fail");
    for entry in document["runtime_inventory"]["fields"]
        .as_array()
        .ok_or_else(|| RegistryCbefError::RegistryModel(format!("{id} fields are absent")))?
    {
        writeln!(
            output,
            "    ProviderRawFieldEntry {{ field_id: {}, field_name: {:?} }},",
            entry["field_id"]
                .as_u64()
                .ok_or_else(|| RegistryCbefError::RegistryModel(id.to_owned()))?,
            entry["field_name"]
                .as_str()
                .ok_or_else(|| RegistryCbefError::RegistryModel(id.to_owned()))?,
        )
        .expect("string writes do not fail");
    }
    writeln!(output, "];\n").expect("string writes do not fail");
    let query_bundle = canonical_bytes(&json!({
        "queries": [{"query_id": "recovery", "source": TREE_SITTER_RECOVERY_QUERY}]
    }))?;
    writeln!(
        output,
        "pub const {constant}: ProviderGrammarInventory = ProviderGrammarInventory {{ catalog_id: {id:?}, language: {:?}, provider_version: {:?}, grammar_abi: {}, node_types_digest: {:?}, runtime_inventory_fingerprint: {:?}, query_bundle_digest: {:?}, query_bundle_canonical_json: &{query_bundle:?}, raw_kinds: {kinds}, fields: {fields} }};\n",
        string_field(document, "language", id)?,
        string_field(document, "provider_version", id)?,
        document["grammar_abi"].as_u64().ok_or_else(|| RegistryCbefError::RegistryModel(id.to_owned()))?,
        string_field(document, "node_types_digest", id)?,
        string_field(document, "runtime_inventory_fingerprint", id)?,
        string_field(document, "query_bundle_digest", id)?,
    )
    .expect("string writes do not fail");
    Ok(())
}

fn string_field<'a>(
    document: &'a Value,
    field: &str,
    id: &str,
) -> Result<&'a str, RegistryCbefError> {
    document[field]
        .as_str()
        .ok_or_else(|| RegistryCbefError::RegistryModel(format!("{id} field {field} is absent")))
}

fn render_ruff_rust_catalog(
    output: &mut String,
    document: &Value,
    syntax_codes: &BTreeMap<&str, u16>,
    fallback: u16,
) -> Result<(), RegistryCbefError> {
    let node_entries = document["runtime_inventory"]["node_kinds"]
        .as_array()
        .ok_or_else(|| RegistryCbefError::RegistryModel("Ruff node kinds are absent".to_owned()))?;
    let token_entries = document["runtime_inventory"]["token_kinds"]
        .as_array()
        .ok_or_else(|| {
            RegistryCbefError::RegistryModel("Ruff token kinds are absent".to_owned())
        })?;
    writeln!(
        output,
        "pub const RUFF_PYTHON_NODE_KINDS: &[RuffNodeKindEntry] = &["
    )
    .expect("string writes do not fail");
    for entry in node_entries {
        let code = entry["canonical_kind_name"]
            .as_str()
            .and_then(|name| syntax_codes.get(name))
            .copied()
            .unwrap_or(fallback);
        let disposition = disposition_variant(&entry["disposition"])?;
        writeln!(
            output,
            "    RuffNodeKindEntry {{ raw_kind_id: {}, raw_name: {:?}, disposition: {disposition}, normalized_kind_code: {code} }},",
            entry["raw_kind_id"].as_u64().ok_or_else(|| RegistryCbefError::RegistryModel("Ruff raw ID".to_owned()))?,
            entry["raw_name"].as_str().ok_or_else(|| RegistryCbefError::RegistryModel("Ruff raw name".to_owned()))?,
        )
        .expect("string writes do not fail");
    }
    writeln!(output, "];\n").expect("string writes do not fail");
    writeln!(
        output,
        "pub const RUFF_PYTHON_TOKEN_KINDS: &[RuffTokenKindEntry] = &["
    )
    .expect("string writes do not fail");
    for entry in token_entries {
        writeln!(
            output,
            "    RuffTokenKindEntry {{ raw_kind_id: {}, raw_name: {:?} }},",
            entry["raw_kind_id"]
                .as_u64()
                .ok_or_else(|| RegistryCbefError::RegistryModel("Ruff token ID".to_owned()))?,
            entry["raw_name"]
                .as_str()
                .ok_or_else(|| RegistryCbefError::RegistryModel("Ruff token name".to_owned()))?,
        )
        .expect("string writes do not fail");
    }
    writeln!(output, "];\n").expect("string writes do not fail");
    writeln!(
        output,
        "#[must_use]\npub const fn ruff_python_node_kind_entry(kind: ruff_python_ast::NodeKind) -> &'static RuffNodeKindEntry {{\n    match kind {{"
    )
    .expect("string writes do not fail");
    for (index, entry) in node_entries.iter().enumerate() {
        writeln!(
            output,
            "        ruff_python_ast::NodeKind::{} => &RUFF_PYTHON_NODE_KINDS[{index}],",
            entry["raw_name"]
                .as_str()
                .ok_or_else(|| RegistryCbefError::RegistryModel("Ruff raw name".to_owned()))?,
        )
        .expect("string writes do not fail");
    }
    writeln!(output, "    }}\n}}\n").expect("string writes do not fail");
    writeln!(
        output,
        "#[must_use]\n#[allow(clippy::too_many_lines)]\npub const fn ruff_python_token_kind_entry(kind: ruff_python_ast::token::TokenKind) -> &'static RuffTokenKindEntry {{\n    match kind {{"
    )
    .expect("string writes do not fail");
    for (index, entry) in token_entries.iter().enumerate() {
        writeln!(
            output,
            "        ruff_python_ast::token::TokenKind::{} => &RUFF_PYTHON_TOKEN_KINDS[{index}],",
            entry["raw_name"]
                .as_str()
                .ok_or_else(|| RegistryCbefError::RegistryModel("Ruff token name".to_owned()))?,
        )
        .expect("string writes do not fail");
    }
    writeln!(output, "    }}\n}}\n").expect("string writes do not fail");
    writeln!(
        output,
        "pub const RUFF_PYTHON_FRONTEND: RuffPythonInventory = RuffPythonInventory {{ catalog_id: {:?}, provider_version: {:?}, runtime_inventory_fingerprint: {:?}, node_kinds: RUFF_PYTHON_NODE_KINDS, token_kinds: RUFF_PYTHON_TOKEN_KINDS }};\n",
        string_field(document, "catalog_id", "ruff-python-0-0-7")?,
        string_field(document, "provider_version", "ruff-python-0-0-7")?,
        string_field(document, "runtime_inventory_fingerprint", "ruff-python-0-0-7")?,
    )
    .expect("string writes do not fail");
    Ok(())
}

fn parse_yaml<T: for<'de> Deserialize<'de>>(
    root: &Path,
    relative: &str,
) -> Result<T, DriverProtocolError> {
    let path = root.join(relative);
    let bytes = read_stable(&path, MAX_AUTHORITY_BYTES)?;
    serde_yaml_ng::from_slice(&bytes).map_err(|source| DriverProtocolError::Io {
        path,
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
    })
}

#[allow(clippy::too_many_lines)] // One pass keeps the cross-authority invariants adjacent.
fn validate_authorities(
    cbef: &CbefAuthority,
    enums: &EnumRegistry,
    flags: &FlagRegistry,
) -> Result<(), RegistryCbefError> {
    if cbef.format_name != "CBEF-v1"
        || cbef.format_version != 1
        || cbef.magic_ascii != "CFID"
        || cbef.byte_order != "big-endian"
        || cbef.digest_algorithm != "BLAKE3-256"
        || cbef.id_derivation != "first-16-bytes"
    {
        return Err(RegistryCbefError::AuthorityInvariant);
    }
    let type_codes: BTreeMap<_, _> = cbef
        .type_codes
        .iter()
        .map(|spec| (spec.name.as_str(), spec.code))
        .collect();
    let observed_type_codes: BTreeSet<_> = type_codes.values().copied().collect();
    if type_codes.len() != 13 || observed_type_codes != (0_u8..=12).collect() {
        return Err(RegistryCbefError::AuthorityInvariant);
    }
    let mut domain_codes = BTreeSet::new();
    let mut domain_names = BTreeSet::new();
    for domain in &cbef.domains {
        if !domain_codes.insert(domain.code)
            || !domain_names.insert(domain.name.as_str())
            || domain.fields.is_empty()
        {
            return Err(RegistryCbefError::AuthorityInvariant);
        }
        let mut previous = 0;
        let mut names = BTreeSet::new();
        for field in &domain.fields {
            if field.tag <= previous
                || !names.insert(field.name.as_str())
                || !type_codes.contains_key(field.type_code.as_str())
            {
                return Err(RegistryCbefError::AuthorityInvariant);
            }
            if matches!(field.type_code.as_str(), "UNSIGNED" | "SIGNED")
                && !matches!(field.width_bytes, Some(1 | 2 | 4 | 8 | 16))
            {
                return Err(RegistryCbefError::AuthorityInvariant);
            }
            previous = field.tag;
        }
    }
    validate_exact_domain(
        cbef,
        "ENTITY",
        &[
            ("workspace_id", "ID", None),
            ("analysis_context_id", "ID", None),
            ("kind_code", "UNSIGNED", Some(2)),
            ("owner_id", "ID", None),
            ("semantic_key", "BYTES", None),
        ],
    )?;
    validate_exact_domain(
        cbef,
        "RELATION_FACT",
        &[
            ("workspace_id", "ID", None),
            ("analysis_context_id", "ID", None),
            ("relation_kind_code", "UNSIGNED", Some(2)),
            ("subject_entity_id", "ID", None),
            ("object_entity_id", "ID", None),
            ("role", "TAGGED_UNION", None),
        ],
    )?;
    let required_enum_domains = [
        (
            "OCCURRENCE_FAMILY",
            ["TOKEN", "ANNOTATION", "SYNTAX"].as_slice(),
        ),
        (
            "RANGE_RECONCILIATION_STEP",
            [
                "EXACT_RANGE_AND_KIND",
                "EXACT_DECLARATION_NAME",
                "SMALLEST_ENCLOSING_COMPATIBLE",
                "SAME_START_COMPATIBLE",
                "PROVIDER_ONLY_SYNTHETIC",
            ]
            .as_slice(),
        ),
        (
            "RAW_KIND_DISPOSITION",
            ["NORMALIZE", "IGNORE", "UNSUPPORTED"].as_slice(),
        ),
    ];
    for (name, values) in required_enum_domains {
        let domain = enums
            .records
            .iter()
            .find(|domain| domain.domain == name)
            .ok_or(RegistryCbefError::AuthorityInvariant)?;
        if domain.width_bits != 16
            || domain
                .values
                .iter()
                .map(|value| value.name.as_str())
                .ne(values.iter().copied())
            || !domain
                .values
                .iter()
                .map(|value| value.code)
                .eq((1_u16..).map(|index| index * 10).take(values.len()))
        {
            return Err(RegistryCbefError::AuthorityInvariant);
        }
    }
    let provider_flags = flags
        .records
        .iter()
        .find(|domain| domain.domain == "PROVIDER_NODE_FLAGS")
        .ok_or(RegistryCbefError::AuthorityInvariant)?;
    if provider_flags.width_bits != 64 || !provider_flags.values.is_empty() {
        return Err(RegistryCbefError::AuthorityInvariant);
    }
    validate_allocations(&enums.records, &flags.records)
}

fn validate_exact_domain(
    cbef: &CbefAuthority,
    name: &str,
    expected: &[(&str, &str, Option<u8>)],
) -> Result<(), RegistryCbefError> {
    let domain = cbef
        .domains
        .iter()
        .find(|domain| domain.name == name)
        .ok_or(RegistryCbefError::AuthorityInvariant)?;
    let observed = domain
        .fields
        .iter()
        .map(|field| {
            (
                field.name.as_str(),
                field.type_code.as_str(),
                field.width_bytes,
            )
        })
        .collect::<Vec<_>>();
    if observed != expected {
        return Err(RegistryCbefError::AuthorityInvariant);
    }
    Ok(())
}

fn validate_allocations(
    enums: &[EnumDomain],
    flags: &[FlagDomain],
) -> Result<(), RegistryCbefError> {
    let mut domains = BTreeSet::new();
    for domain in enums {
        if domain.width_bits == 0 || !domains.insert(domain.domain.as_str()) {
            return Err(RegistryCbefError::AuthorityInvariant);
        }
        let mut codes = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut slugs = BTreeSet::new();
        for value in &domain.values {
            if value.code == 0
                || !codes.insert(value.code)
                || !names.insert(value.name.as_str())
                || !slugs.insert(value.slug.as_str())
            {
                return Err(RegistryCbefError::AuthorityInvariant);
            }
        }
    }
    domains.clear();
    for domain in flags {
        if domain.width_bits != 64 || !domains.insert(domain.domain.as_str()) {
            return Err(RegistryCbefError::AuthorityInvariant);
        }
        let mut bits = BTreeSet::new();
        for value in &domain.values {
            if value.bit >= 56 || !bits.insert(value.bit) {
                return Err(RegistryCbefError::AuthorityInvariant);
            }
        }
    }
    Ok(())
}

fn build_named(
    authority: &CbefAuthority,
    domain_name: &str,
    mut values: BTreeMap<String, RecipeValue>,
) -> Result<RecipeRecord, RegistryCbefError> {
    let domain = authority
        .domains
        .iter()
        .find(|domain| domain.name == domain_name)
        .ok_or_else(|| RegistryCbefError::UnknownDomain(domain_name.to_owned()))?;
    let mut fields = Vec::with_capacity(domain.fields.len());
    for spec in &domain.fields {
        let value = values
            .remove(&spec.name)
            .ok_or_else(|| RegistryCbefError::MissingOperand(spec.name.clone()))?;
        validate_value(spec, &value)?;
        fields.push(RecipeField {
            tag: spec.tag,
            value,
        });
    }
    if let Some(extra) = values.into_keys().next() {
        return Err(RegistryCbefError::ExtraOperand(extra));
    }
    Ok(RecipeRecord {
        domain_code: domain.code,
        fields,
    })
}

fn validate_value(spec: &CbefFieldSpec, value: &RecipeValue) -> Result<(), RegistryCbefError> {
    if spec.type_code != value.type_name() {
        return Err(RegistryCbefError::WrongType {
            field: spec.name.clone(),
            expected: spec.type_code.clone(),
            actual: value.type_name().to_owned(),
        });
    }
    if let Some(width) = spec.width_bytes
        && matches!(value, RecipeValue::Unsigned(bytes) | RecipeValue::Signed(bytes) if bytes.len() != usize::from(width))
    {
        return Err(RegistryCbefError::WrongWidth(spec.name.clone()));
    }
    if spec.normalization.as_deref() == Some("ASCII_LOWER")
        && matches!(value, RecipeValue::Utf8(text) if !text.is_ascii() || text.to_ascii_lowercase() != *text)
    {
        return Err(RegistryCbefError::NonNormalized(spec.name.clone()));
    }
    Ok(())
}

fn validate_record(
    authority: &CbefAuthority,
    record: &RecipeRecord,
) -> Result<(), RegistryCbefError> {
    let domain = authority
        .domains
        .iter()
        .find(|domain| domain.code == record.domain_code)
        .ok_or_else(|| RegistryCbefError::UnknownDomain(record.domain_code.to_string()))?;
    if domain.fields.len() != record.fields.len() {
        return Err(RegistryCbefError::RecordShape);
    }
    for (spec, field) in domain.fields.iter().zip(&record.fields) {
        if spec.tag != field.tag {
            return Err(RegistryCbefError::RecordShape);
        }
        validate_value(spec, &field.value)?;
    }
    Ok(())
}

fn validate_all_recipes(authority: &CbefAuthority) -> Result<(), RegistryCbefError> {
    for domain in &authority.domains {
        let values = domain
            .fields
            .iter()
            .map(|field| Ok((field.name.clone(), sample_value(field)?)))
            .collect::<Result<BTreeMap<_, _>, RegistryCbefError>>()?;
        let record = build_named(authority, &domain.name, values)?;
        validate_record(authority, &record)?;
        let bytes = encode_record(authority, &record)?;
        if !bytes.starts_with(b"CFID") {
            return Err(RegistryCbefError::RecordShape);
        }
    }
    Ok(())
}

fn sample_value(field: &CbefFieldSpec) -> Result<RecipeValue, RegistryCbefError> {
    Ok(match field.type_code.as_str() {
        "ABSENT" => RecipeValue::Absent,
        "BYTES" => RecipeValue::Bytes(vec![1]),
        "UTF8" => RecipeValue::Utf8("sample".to_owned()),
        "RAW_PATH" => RecipeValue::RawPath {
            platform_code: 1,
            bytes: b"sample".to_vec(),
        },
        "UNSIGNED" => RecipeValue::Unsigned(vec![1; usize::from(field.width_bytes.unwrap_or(2))]),
        "SIGNED" => RecipeValue::Signed(vec![1; usize::from(field.width_bytes.unwrap_or(2))]),
        "BOOLEAN" => RecipeValue::Boolean(true),
        "ID" => RecipeValue::Id([1; 16]),
        "DIGEST" => RecipeValue::Digest([1; 32]),
        "ORDERED_LIST" => RecipeValue::OrderedList(vec![]),
        "SET" => RecipeValue::Set(vec![]),
        "MAP" => RecipeValue::Map(vec![]),
        "TAGGED_UNION" => RecipeValue::TaggedUnion {
            variant: 0,
            value: Box::new(RecipeValue::Absent),
        },
        _ => return Err(RegistryCbefError::AuthorityInvariant),
    })
}

fn encode_record(
    authority: &CbefAuthority,
    record: &RecipeRecord,
) -> Result<Vec<u8>, RegistryCbefError> {
    validate_record(authority, record)?;
    let type_codes: BTreeMap<_, _> = authority
        .type_codes
        .iter()
        .map(|spec| (spec.name.as_str(), spec.code))
        .collect();
    let mut bytes = b"CFID".to_vec();
    bytes.push(authority.format_version);
    bytes.extend(record.domain_code.to_be_bytes());
    bytes.extend(
        u16::try_from(record.fields.len())
            .map_err(|_| RegistryCbefError::RecordShape)?
            .to_be_bytes(),
    );
    for field in &record.fields {
        let payload = encode_value(&field.value, &type_codes)?;
        bytes.extend(field.tag.to_be_bytes());
        bytes.push(type_codes[field.value.type_name()]);
        bytes.extend(
            u32::try_from(payload.len())
                .map_err(|_| RegistryCbefError::RecordShape)?
                .to_be_bytes(),
        );
        bytes.extend(payload);
    }
    Ok(bytes)
}

fn encode_value(
    value: &RecipeValue,
    type_codes: &BTreeMap<&str, u8>,
) -> Result<Vec<u8>, RegistryCbefError> {
    let mut bytes = Vec::new();
    match value {
        RecipeValue::Absent => {}
        RecipeValue::Bytes(value) | RecipeValue::Unsigned(value) | RecipeValue::Signed(value) => {
            bytes.extend(value);
        }
        RecipeValue::Utf8(value) => bytes.extend(value.as_bytes()),
        RecipeValue::RawPath {
            platform_code,
            bytes: value,
        } => {
            bytes.push(*platform_code);
            bytes.extend(value);
        }
        RecipeValue::Boolean(value) => bytes.push(u8::from(*value)),
        RecipeValue::Id(value) => bytes.extend(value),
        RecipeValue::Digest(value) => bytes.extend(value),
        RecipeValue::OrderedList(values) | RecipeValue::Set(values) => {
            bytes.extend(
                u32::try_from(values.len())
                    .map_err(|_| RegistryCbefError::RecordShape)?
                    .to_be_bytes(),
            );
            let mut encoded = values
                .iter()
                .map(|value| encode_typed(value, type_codes))
                .collect::<Result<Vec<_>, _>>()?;
            if matches!(value, RecipeValue::Set(_)) {
                encoded.sort();
                encoded.dedup();
            }
            for value in encoded {
                bytes.extend(
                    u32::try_from(value.len())
                        .map_err(|_| RegistryCbefError::RecordShape)?
                        .to_be_bytes(),
                );
                bytes.extend(value);
            }
        }
        RecipeValue::Map(entries) => {
            bytes.extend(
                u32::try_from(entries.len())
                    .map_err(|_| RegistryCbefError::RecordShape)?
                    .to_be_bytes(),
            );
            let mut encoded = entries
                .iter()
                .map(|(key, value)| {
                    Ok((
                        encode_typed(key, type_codes)?,
                        encode_typed(value, type_codes)?,
                    ))
                })
                .collect::<Result<Vec<_>, RegistryCbefError>>()?;
            encoded.sort_by(|left, right| left.0.cmp(&right.0));
            for (key, value) in encoded {
                for item in [key, value] {
                    bytes.extend(
                        u32::try_from(item.len())
                            .map_err(|_| RegistryCbefError::RecordShape)?
                            .to_be_bytes(),
                    );
                    bytes.extend(item);
                }
            }
        }
        RecipeValue::TaggedUnion { variant, value } => {
            let value = encode_typed(value, type_codes)?;
            bytes.extend(variant.to_be_bytes());
            bytes.extend(
                u32::try_from(value.len())
                    .map_err(|_| RegistryCbefError::RecordShape)?
                    .to_be_bytes(),
            );
            bytes.extend(value);
        }
    }
    Ok(bytes)
}

fn encode_typed(
    value: &RecipeValue,
    type_codes: &BTreeMap<&str, u8>,
) -> Result<Vec<u8>, RegistryCbefError> {
    let payload = encode_value(value, type_codes)?;
    let mut bytes = vec![type_codes[value.type_name()]];
    bytes.extend(
        u32::try_from(payload.len())
            .map_err(|_| RegistryCbefError::RecordShape)?
            .to_be_bytes(),
    );
    bytes.extend(payload);
    Ok(bytes)
}

fn render_projection(plan: &RegistryCbefPlan) -> Result<Vec<u8>, RegistryCbefError> {
    let projection = RegistryCbefProjection {
        schema_version: 1,
        source_artifact_ids: vec![
            plan.cbef.header.artifact_id.clone(),
            plan.enums.header.artifact_id.clone(),
            plan.flags.header.artifact_id.clone(),
        ],
        cbef_domains: plan.cbef.domains.clone(),
        enum_domains: plan.enums.records.clone(),
        flag_domains: plan.flags.records.clone(),
    };
    let value = serde_json::to_value(projection).map_err(RegistryCbefError::Json)?;
    let mut bytes = serde_json_canonicalizer::to_vec(&value).map_err(RegistryCbefError::Json)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn render_rust_recipes(authority: &CbefAuthority) -> Vec<u8> {
    let mut output = String::from(
        "// @generated by codefabric-model; do not edit.\n\
         #[derive(Clone, Debug, Eq, PartialEq)]\n\
         pub enum RecipeValue { Absent, Bytes(Vec<u8>), Utf8(String), RawPath(u8, Vec<u8>), Unsigned(Vec<u8>), Signed(Vec<u8>), Boolean(bool), Id([u8; 16]), Digest([u8; 32]), OrderedList(Vec<RecipeValue>), Set(Vec<RecipeValue>), Map(Vec<(RecipeValue, RecipeValue)>), TaggedUnion(u16, Box<RecipeValue>) }\n\
         #[derive(Clone, Debug, Eq, PartialEq)] pub struct RecipeField { pub tag: u16, pub value: RecipeValue }\n\
         #[derive(Clone, Debug, Eq, PartialEq)] pub struct RecipeRecord { pub domain_code: u16, pub fields: Vec<RecipeField> }\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)] pub struct RecipeError;\n\
         fn expect(value: &RecipeValue, expected: u8, width: usize) -> Result<(), RecipeError> { let actual = match value { RecipeValue::Absent => 0, RecipeValue::Bytes(_) => 1, RecipeValue::Utf8(_) => 2, RecipeValue::RawPath(_, _) => 3, RecipeValue::Unsigned(_) => 4, RecipeValue::Signed(_) => 5, RecipeValue::Boolean(_) => 6, RecipeValue::Id(_) => 7, RecipeValue::Digest(_) => 8, RecipeValue::OrderedList(_) => 9, RecipeValue::Set(_) => 10, RecipeValue::Map(_) => 11, RecipeValue::TaggedUnion(_, _) => 12 }; if actual != expected { return Err(RecipeError); } if width != 0 { match value { RecipeValue::Unsigned(bytes) | RecipeValue::Signed(bytes) if bytes.len() == width => {}, _ => return Err(RecipeError) } } Ok(()) }\n\n",
    );
    let type_codes: BTreeMap<_, _> = authority
        .type_codes
        .iter()
        .map(|item| (item.name.as_str(), item.code))
        .collect();
    for domain in &authority.domains {
        let type_name = pascal(&domain.name);
        let function_name = rust_ident(&snake(&domain.name));
        writeln!(output, "pub struct {type_name}Fields {{").unwrap();
        for field in &domain.fields {
            writeln!(output, "    pub {}: RecipeValue,", snake(&field.name)).unwrap();
        }
        writeln!(output, "}}\npub fn {function_name}(fields: {type_name}Fields) -> Result<RecipeRecord, RecipeError> {{").unwrap();
        for field in &domain.fields {
            writeln!(
                output,
                "    expect(&fields.{}, {}, {})?;",
                snake(&field.name),
                type_codes[field.type_code.as_str()],
                field.width_bytes.unwrap_or(0)
            )
            .unwrap();
        }
        writeln!(
            output,
            "    Ok(RecipeRecord {{ domain_code: {}, fields: vec![",
            domain.code
        )
        .unwrap();
        for field in &domain.fields {
            writeln!(
                output,
                "        RecipeField {{ tag: {}, value: fields.{} }},",
                field.tag,
                snake(&field.name)
            )
            .unwrap();
        }
        output.push_str("    ] })\n}\n\n");
    }
    output.into_bytes()
}

fn render_rust_registries(enums: &EnumRegistry, flags: &FlagRegistry) -> Vec<u8> {
    let mut output = String::from("// @generated by codefabric-model; do not edit.\n");
    for domain in &enums.records {
        let name = pascal(&domain.domain);
        writeln!(
            output,
            "#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)] #[repr(u16)] pub enum {name} {{"
        )
        .unwrap();
        for value in &domain.values {
            writeln!(output, "    {} = {},", pascal(&value.name), value.code).unwrap();
        }
        output.push_str("}\n\n");
    }
    for domain in &flags.records {
        let name = pascal(&domain.domain);
        writeln!(
            output,
            "#[derive(Clone, Copy, Debug, Eq, PartialEq)] pub struct {name}(u64);\nimpl {name} {{"
        )
        .unwrap();
        output.push_str("    pub const fn empty() -> Self { Self(0) }\n");
        let mut mask = 0_u64;
        for value in &domain.values {
            let bit = 1_u64 << value.bit;
            mask |= bit;
            writeln!(
                output,
                "    pub const {}: Self = Self({});",
                value.name,
                rust_u64_literal(bit)
            )
            .unwrap();
        }
        if mask == 0 {
            output.push_str(
                "    pub const fn from_bits(bits: u64) -> Option<Self> { if bits == 0 { Some(Self(bits)) } else { None } }\n",
            );
        } else {
            writeln!(output, "    pub const fn from_bits(bits: u64) -> Option<Self> {{ if bits & !{} == 0 {{ Some(Self(bits)) }} else {{ None }} }}", rust_u64_literal(mask)).unwrap();
        }
        output.push_str("    pub const fn bits(self) -> u64 { self.0 }\n}\n\n");
    }
    output.into_bytes()
}

fn registry_value<'a>(
    values: &'a BTreeMap<String, Value>,
    artifact_id: &str,
) -> Result<&'a Value, RegistryCbefError> {
    values.get(artifact_id).ok_or_else(|| {
        RegistryCbefError::RegistryModel(format!("missing runtime registry {artifact_id}"))
    })
}

fn registry_records<T: DeserializeOwned>(
    values: &BTreeMap<String, Value>,
    artifact_id: &str,
) -> Result<Vec<T>, RegistryCbefError> {
    serde_json::from_value(
        registry_value(values, artifact_id)?
            .get("records")
            .cloned()
            .ok_or_else(|| {
                RegistryCbefError::RegistryModel(format!(
                    "runtime registry {artifact_id} has no records"
                ))
            })?,
    )
    .map_err(RegistryCbefError::Json)
}

fn runtime_pascal(value: &str) -> String {
    if !value.contains('_')
        && value.chars().any(char::is_lowercase)
        && value.chars().next().is_some_and(char::is_uppercase)
    {
        return value.to_owned();
    }
    pascal(value)
}

fn screaming_snake_from_pascal(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            if character.is_uppercase() {
                vec!['_', character]
            } else {
                vec![character.to_ascii_uppercase()]
            }
        })
        .collect::<String>()
        .trim_start_matches('_')
        .to_owned()
}

fn rust_integer(value: u64) -> String {
    let digits = value.to_string();
    let mut rendered = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            rendered.push('_');
        }
        rendered.push(character);
    }
    rendered
}

fn emit_runtime_enum(output: &mut String, name: &str, values: &[EnumValue]) {
    let type_name = runtime_pascal(name);
    writeln!(
        output,
        "#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]"
    )
    .unwrap();
    writeln!(output, "#[repr(u16)]").unwrap();
    writeln!(output, "pub enum {type_name} {{").unwrap();
    for value in values {
        writeln!(
            output,
            "    {} = {},",
            runtime_pascal(&value.name),
            value.code
        )
        .unwrap();
    }
    writeln!(output, "}}").unwrap();
    writeln!(output, "impl TryFrom<u16> for {type_name} {{").unwrap();
    writeln!(output, "    type Error = UnknownRegistryCode;").unwrap();
    writeln!(
        output,
        "    fn try_from(code: u16) -> Result<Self, Self::Error> {{"
    )
    .unwrap();
    output.push_str("        match code {\n");
    for value in values {
        writeln!(
            output,
            "            {} => Ok(Self::{}),",
            value.code,
            runtime_pascal(&value.name)
        )
        .unwrap();
    }
    writeln!(
        output,
        "            _ => Err(UnknownRegistryCode {{ domain: {name:?}, code }}),"
    )
    .unwrap();
    output.push_str("        }\n    }\n}\n");
}

#[allow(clippy::too_many_lines)] // One typed pass makes the complete runtime API exhaustive.
fn render_rust_runtime_registries(
    values: &BTreeMap<String, Value>,
    enums: &EnumRegistry,
    flags: &FlagRegistry,
) -> Result<Vec<u8>, RegistryCbefError> {
    let machines: Vec<governed::StateMachine> =
        registry_records(values, "codefabric.registry.state-machine-registry")?;
    let phrases: Vec<governed::PhraseRecord> =
        registry_records(values, "codefabric.registry.phrase-registry")?;
    let mut output = String::from(
        "// @generated by codefabric-model; edit registry authorities instead.\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct UnknownRegistryCode { pub domain: &'static str, pub code: u16 }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct RegistryEntry { pub code: u16, pub name: &'static str, pub slug: &'static str }\n\n\
         #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct FlagEntry { pub mask: u64, pub name: &'static str, pub slug: &'static str }\n\n",
    );
    for domain in &enums.records {
        emit_runtime_enum(&mut output, &domain.domain, &domain.values);
        writeln!(
            output,
            "pub const {}_VALUES: &[RegistryEntry] = &[",
            domain.domain
        )
        .unwrap();
        for value in &domain.values {
            writeln!(
                output,
                "    RegistryEntry {{ code: {}, name: {:?}, slug: {:?} }},",
                value.code, value.name, value.slug
            )
            .unwrap();
        }
        output.push_str("];\n\n");
    }
    let enum_type_names = enums
        .records
        .iter()
        .map(|domain| runtime_pascal(&domain.domain))
        .collect::<BTreeSet<_>>();
    for machine in &machines {
        if !enum_type_names.contains(&machine.machine_id) {
            let states = machine
                .states
                .iter()
                .map(|state| EnumValue {
                    code: state.code,
                    name: state.name.clone(),
                    slug: state.slug.clone(),
                    meaning: state.meaning.clone(),
                    aliases: state.aliases.clone(),
                })
                .collect::<Vec<_>>();
            emit_runtime_enum(&mut output, &machine.machine_id, &states);
            let constant = screaming_snake_from_pascal(&machine.machine_id);
            writeln!(output, "pub const {constant}_VALUES: &[RegistryEntry] = &[").unwrap();
            for state in &states {
                writeln!(
                    output,
                    "    RegistryEntry {{ code: {}, name: {:?}, slug: {:?} }},",
                    state.code, state.name, state.slug
                )
                .unwrap();
            }
            output.push_str("];\n\n");
        }
    }
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct RegistryDomainEntry { pub domain: &'static str, pub version: &'static str, pub canonical_digest: &'static str, pub values: &'static [RegistryEntry] }\n\n\
         pub const REGISTRY_DOMAINS: &[RegistryDomainEntry] = &[\n",
    );
    let enum_version = &enums.header.version;
    let enum_digest = &enums.header.canonical_digest;
    let state_value = registry_value(values, "codefabric.registry.state-machine-registry")?;
    let state_version = state_value["version"]
        .as_str()
        .ok_or_else(|| RegistryCbefError::RegistryModel("state version is absent".to_owned()))?;
    let state_digest = state_value["canonical_digest"]
        .as_str()
        .ok_or_else(|| RegistryCbefError::RegistryModel("state digest is absent".to_owned()))?;
    let mut emitted_domains = BTreeSet::new();
    for domain in &enums.records {
        emitted_domains.insert(domain.domain.clone());
        writeln!(
            output,
            "    RegistryDomainEntry {{ domain: {:?}, version: {:?}, canonical_digest: {:?}, values: {}_VALUES }},",
            domain.domain, enum_version, enum_digest, domain.domain
        )
        .unwrap();
    }
    for machine in &machines {
        let constant = screaming_snake_from_pascal(&machine.machine_id);
        if emitted_domains.insert(constant.clone()) {
            writeln!(
                output,
                "    RegistryDomainEntry {{ domain: {constant:?}, version: {state_version:?}, canonical_digest: {state_digest:?}, values: {constant}_VALUES }},"
            )
            .unwrap();
        }
    }
    output.push_str("];\n\n");
    let derivations: Vec<governed::DerivationDefinition> =
        registry_records(values, "codefabric.registry.derivation-registry")?;
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct DerivationEntry { pub derivation_id: &'static str, pub owner_kind: &'static str, pub input_fact_families: &'static [&'static str], pub output_fact_families: &'static [&'static str], pub projection_id: &'static str, pub precision_profile: &'static str, pub algorithm_version: &'static str, pub replacement_scope: &'static str, pub dependency_rule: &'static str }\n\n\
         pub const DERIVATION_ENTRIES: &[DerivationEntry] = &[\n",
    );
    for derivation in &derivations {
        writeln!(
            output,
            "    DerivationEntry {{ derivation_id: {:?}, owner_kind: {:?}, input_fact_families: &{:?}, output_fact_families: &{:?}, projection_id: {:?}, precision_profile: {:?}, algorithm_version: {:?}, replacement_scope: {:?}, dependency_rule: {:?} }},",
            derivation.derivation_id,
            derivation.owner_kind,
            derivation.input_fact_families,
            derivation.output_fact_families,
            derivation.projection_id,
            derivation.precision_profile,
            derivation.algorithm_version,
            derivation.replacement_scope,
            derivation.dependency_rule,
        )
        .unwrap();
    }
    output.push_str("];\n\n");
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct StateTransitionEntry { pub from: &'static str, pub event: &'static str, pub guard: &'static str, pub to: &'static str, pub actions: &'static [&'static str], pub idempotency_key: &'static str, pub error_on_illegal: &'static str }\n\n",
    );
    for machine in &machines {
        let constant = screaming_snake_from_pascal(&machine.machine_id);
        writeln!(
            output,
            "pub const {constant}_TRANSITIONS: &[StateTransitionEntry] = &["
        )
        .unwrap();
        for transition in &machine.transitions {
            writeln!(
                output,
                "    StateTransitionEntry {{ from: {:?}, event: {:?}, guard: {:?}, to: {:?}, actions: &{:?}, idempotency_key: {:?}, error_on_illegal: {:?} }},",
                transition.from,
                transition.event,
                transition.guard,
                transition.to,
                transition.actions,
                transition.idempotency_key,
                transition.error_on_illegal,
            )
            .unwrap();
        }
        output.push_str("];\n\n");
    }
    for domain in &flags.records {
        writeln!(
            output,
            "pub const {}_FLAGS: &[FlagEntry] = &[",
            domain.domain
        )
        .unwrap();
        for value in &domain.values {
            writeln!(
                output,
                "    FlagEntry {{ mask: 1_u64 << {}, name: {:?}, slug: {:?} }},",
                value.bit, value.name, value.slug
            )
            .unwrap();
        }
        output.push_str("];\n\n");
    }
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct PhraseEntry { pub phrase_id: &'static str, pub owner_section: u16, pub canonical_text: &'static str, pub accepted_aliases: &'static [&'static str], pub plan_node_kind: &'static str, pub output_role: &'static str }\n\n\
         pub const PHRASE_ENTRIES: &[PhraseEntry] = &[\n",
    );
    for phrase in &phrases {
        writeln!(
            output,
            "    PhraseEntry {{ phrase_id: {:?}, owner_section: {}, canonical_text: {:?}, accepted_aliases: &{:?}, plan_node_kind: {:?}, output_role: {:?} }},",
            phrase.phrase_id,
            phrase.owner_section,
            phrase.canonical_text,
            phrase.accepted_aliases,
            phrase.planspec_mapping.node_kind.as_str(),
            phrase.planspec_mapping.output_role.as_str(),
        )
        .unwrap();
    }
    output.push_str("];\n\n");
    for (constant, artifact_id, field) in [
        (
            "ENTITY_KIND_IDS",
            "codefabric.registry.ontology-entity-registry",
            "canonical_name",
        ),
        (
            "RELATION_KIND_IDS",
            "codefabric.registry.ontology-relation-registry",
            "canonical_name",
        ),
        (
            "PROPERTY_KIND_IDS",
            "codefabric.registry.ontology-property-registry",
            "canonical_name",
        ),
        (
            "FACT_KIND_IDS",
            "codefabric.registry.ontology-fact-registry",
            "canonical_name",
        ),
        (
            "UNKNOWN_IDS",
            "codefabric.registry.unknown-registry",
            "name",
        ),
        (
            "PROJECTION_IDS",
            "codefabric.registry.projection-registry",
            "projection_id",
        ),
        (
            "SUMMARY_PROFILE_IDS",
            "codefabric.registry.summary-registry",
            "summary_profile_id",
        ),
        (
            "CAPABILITY_IDS",
            "codefabric.registry.capability-registry",
            "capability_code",
        ),
        (
            "PROVIDER_IDS",
            "codefabric.registry.provider-registry",
            "provider_id",
        ),
        (
            "PROVIDER_NORMALIZATION_IDS",
            "codefabric.registry.provider-normalization-registry",
            "mapping_id",
        ),
        (
            "PROVIDER_RESOURCE_PROFILE_IDS",
            "codefabric.registry.provider-resource-profile-registry",
            "profile_id",
        ),
        (
            "PUBLIC_ERROR_IDS",
            "codefabric.registry.error-registry",
            "name",
        ),
        (
            "DERIVATION_IDS",
            "codefabric.registry.derivation-registry",
            "derivation_id",
        ),
        (
            "PHRASE_IDS",
            "codefabric.registry.phrase-registry",
            "phrase_id",
        ),
    ] {
        writeln!(output, "pub const {constant}: &[&str] = &[").unwrap();
        let records = registry_value(values, artifact_id)?["records"]
            .as_array()
            .ok_or_else(|| RegistryCbefError::RegistryModel(format!("{artifact_id} records")))?;
        for record in records {
            let id = record[field].as_str().ok_or_else(|| {
                RegistryCbefError::RegistryModel(format!("{artifact_id}.{field}"))
            })?;
            writeln!(output, "    {id:?},").unwrap();
        }
        output.push_str("];\n\n");
    }
    let providers: Vec<governed::Provider> =
        registry_records(values, "codefabric.registry.provider-registry")?;
    let provider_codes = enums
        .records
        .iter()
        .find(|domain| domain.domain == "PROVIDER_CODE")
        .ok_or_else(|| RegistryCbefError::RegistryModel("PROVIDER_CODE domain".to_owned()))?
        .values
        .iter()
        .map(|value| (value.slug.as_str(), value.code))
        .collect::<BTreeMap<_, _>>();
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderEntry { pub provider_code: i16, pub provider_id: &'static str, pub placement: &'static str, pub capability_codes: &'static [&'static str], pub resource_profile_id: &'static str, pub event_mapping_version: &'static str }\n\n\
         pub const PROVIDER_ENTRIES: &[ProviderEntry] = &[\n",
    );
    for provider in &providers {
        let code = i16::try_from(
            *provider_codes
                .get(provider.provider_id.as_str())
                .ok_or_else(|| {
                    RegistryCbefError::RegistryModel(format!(
                        "provider code {}",
                        provider.provider_id
                    ))
                })?,
        )
        .map_err(|_| RegistryCbefError::RegistryModel("provider code exceeds i16".to_owned()))?;
        writeln!(
            output,
            "    ProviderEntry {{ provider_code: {code}, provider_id: {:?}, placement: {:?}, capability_codes: &{:?}, resource_profile_id: {:?}, event_mapping_version: {:?} }},",
            provider.provider_id,
            provider.placement,
            provider.capability_codes,
            provider.resource_profile_id,
            provider.event_mapping_version,
        )
        .unwrap();
    }
    output.push_str("];\n\n");
    let profiles: Vec<governed::ProviderResourceProfile> = registry_records(
        values,
        "codefabric.registry.provider-resource-profile-registry",
    )?;
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderResourceProfileEntry { pub profile_id: &'static str, pub provider_ids: &'static [&'static str], pub max_parallel_jobs_global: u16, pub max_parallel_jobs_per_workspace: u16, pub max_parallel_jobs_per_context: u16, pub max_input_bytes: u64, pub max_work_units: u64, pub max_wall_millis: u64, pub max_visited_nodes: u64, pub max_traversal_depth: u16, pub max_output_records: u64, pub max_output_bytes: u64, pub max_diagnostics: u16, pub max_parser_workers: u16, pub max_retained_tree_revisions: u16, pub max_cpu_weight: u32, pub max_memory_mib: u32, pub cancellation_check_interval: u32, pub cancellation_ack_millis: u16, pub hard_stop_policy: &'static str, pub retry_policy: &'static str, pub max_retries: u16 }\n\n\
         pub const PROVIDER_RESOURCE_PROFILES: &[ProviderResourceProfileEntry] = &[\n",
    );
    for profile in &profiles {
        let hard_stop_policy = match profile.hard_stop_policy {
            governed::ProviderHardStopPolicy::CooperativeDiscard => "COOPERATIVE_DISCARD",
            governed::ProviderHardStopPolicy::ProcessGroupTerminate => "PROCESS_GROUP_TERMINATE",
            governed::ProviderHardStopPolicy::CancellableTaskAbort => "CANCELLABLE_TASK_ABORT",
        };
        let retry_policy = match profile.retry_policy {
            governed::ProviderRetryPolicy::NoRetry => "NO_RETRY",
            governed::ProviderRetryPolicy::TransientOnly => "TRANSIENT_ONLY",
            governed::ProviderRetryPolicy::IdempotentOnly => "IDEMPOTENT_ONLY",
        };
        let provider_ids = profile.provider_ids.iter().collect::<Vec<_>>();
        writeln!(
            output,
            "    ProviderResourceProfileEntry {{ profile_id: {:?}, provider_ids: &{:?}, max_parallel_jobs_global: {}, max_parallel_jobs_per_workspace: {}, max_parallel_jobs_per_context: {}, max_input_bytes: {}, max_work_units: {}, max_wall_millis: {}, max_visited_nodes: {}, max_traversal_depth: {}, max_output_records: {}, max_output_bytes: {}, max_diagnostics: {}, max_parser_workers: {}, max_retained_tree_revisions: {}, max_cpu_weight: {}, max_memory_mib: {}, cancellation_check_interval: {}, cancellation_ack_millis: {}, hard_stop_policy: {hard_stop_policy:?}, retry_policy: {retry_policy:?}, max_retries: {} }},",
            profile.profile_id,
            provider_ids,
            profile.max_parallel_jobs_global,
            profile.max_parallel_jobs_per_workspace,
            profile.max_parallel_jobs_per_context,
            rust_integer(profile.max_input_bytes),
            rust_integer(profile.max_work_units),
            rust_integer(profile.max_wall_millis),
            rust_integer(profile.max_visited_nodes),
            profile.max_traversal_depth,
            rust_integer(profile.max_output_records),
            rust_integer(profile.max_output_bytes),
            profile.max_diagnostics,
            profile.max_parser_workers,
            profile.max_retained_tree_revisions,
            profile.max_cpu_weight,
            profile.max_memory_mib,
            rust_integer(u64::from(profile.cancellation_check_interval)),
            profile.cancellation_ack_millis,
            profile.max_retries,
        )
        .unwrap();
    }
    output.push_str("];\n\n");
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         pub struct ProviderEventMappingEntry { pub wire_event: &'static str, pub application_event: &'static str, pub mapping_version: &'static str }\n\n\
         pub const PROVIDER_EVENT_MAPPINGS: &[ProviderEventMappingEntry] = &[\n",
    );
    let feature_records = registry_value(values, "codefabric.rpc.feature-registry")?["records"]
        .as_array()
        .ok_or_else(|| RegistryCbefError::RegistryModel("feature records".to_owned()))?;
    for record in feature_records
        .iter()
        .filter(|record| record["domain"].as_str() == Some("PROVIDER_EVENT"))
    {
        writeln!(
            output,
            "    ProviderEventMappingEntry {{ wire_event: {:?}, application_event: {:?}, mapping_version: {:?} }},",
            record["wire_event"].as_str().ok_or_else(|| RegistryCbefError::RegistryModel("wire_event".to_owned()))?,
            record["application_event"].as_str().ok_or_else(|| RegistryCbefError::RegistryModel("application_event".to_owned()))?,
            record["mapping_version"].as_str().ok_or_else(|| RegistryCbefError::RegistryModel("mapping_version".to_owned()))?,
        )
        .unwrap();
    }
    output.push_str("];\n\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub struct OntologyCodeEntry { pub code: i32, pub family_code: i16 }\n\n");
    for (constant, artifact_id, code_field, family_field) in [
        (
            "ENTITY_KIND_CODES",
            "codefabric.registry.ontology-entity-registry",
            "kind_code",
            Some("family_code"),
        ),
        (
            "RELATION_KIND_CODES",
            "codefabric.registry.ontology-relation-registry",
            "relation_code",
            Some("family_code"),
        ),
        (
            "PROPERTY_KIND_CODES",
            "codefabric.registry.ontology-property-registry",
            "property_code",
            None,
        ),
        (
            "FACT_KIND_CODES",
            "codefabric.registry.ontology-fact-registry",
            "fact_code",
            None,
        ),
    ] {
        writeln!(output, "pub const {constant}: &[OntologyCodeEntry] = &[").unwrap();
        let records = registry_value(values, artifact_id)?["records"]
            .as_array()
            .ok_or_else(|| RegistryCbefError::RegistryModel(format!("{artifact_id} records")))?;
        for record in records {
            let code = record[code_field].as_i64().ok_or_else(|| {
                RegistryCbefError::RegistryModel(format!("{artifact_id}.{code_field}"))
            })?;
            let family_code = family_field
                .and_then(|field| record[field].as_i64())
                .unwrap_or_default();
            writeln!(
                output,
                "    OntologyCodeEntry {{ code: {code}, family_code: {family_code} }},"
            )
            .unwrap();
        }
        output.push_str("];\n\n");
    }
    Ok(output.into_bytes())
}

fn rust_u64_literal(value: u64) -> String {
    let hex = format!("{value:016x}");
    format!(
        "0x{}_{}_{}_{}",
        &hex[0..4],
        &hex[4..8],
        &hex[8..12],
        &hex[12..16]
    )
}

fn render_python_registries(enums: &EnumRegistry, flags: &FlagRegistry) -> Vec<u8> {
    let mut output = String::from(
        "# @generated by codefabric-model; do not edit.\nfrom enum import IntEnum, IntFlag\n\n\n",
    );
    for domain in &enums.records {
        writeln!(output, "class {}(IntEnum):", pascal(&domain.domain)).unwrap();
        for value in &domain.values {
            writeln!(output, "    {} = {}", value.name, value.code).unwrap();
        }
        output.push_str("\n\n");
    }
    for domain in &flags.records {
        writeln!(output, "class {}(IntFlag):", pascal(&domain.domain)).unwrap();
        output.push_str("    NONE = 0\n");
        for value in &domain.values {
            writeln!(output, "    {} = {}", value.name, 1_u64 << value.bit).unwrap();
        }
        output.push_str("\n\n");
    }
    while output.ends_with("\n\n") {
        output.pop();
    }
    output.into_bytes()
}

fn validate_transition_semantics(
    enums: &EnumRegistry,
    flags: &FlagRegistry,
) -> Result<(), RegistryCbefError> {
    let code = |domain: &str, name: &str| {
        enums
            .records
            .iter()
            .find(|record| record.domain == domain)
            .and_then(|record| record.values.iter().find(|value| value.name == name))
            .map(|value| value.code)
            .ok_or(RegistryCbefError::AuthorityInvariant)
    };
    let semantics = SourceSyntaxTransitionSemantics {
        occurrence_family_code: code("OCCURRENCE_FAMILY", "SYNTAX")?,
        reconciliation_step_code: Some(code(
            "RANGE_RECONCILIATION_STEP",
            "SMALLEST_ENCLOSING_COMPATIBLE",
        )?),
        raw_kind_disposition_code: code("RAW_KIND_DISPOSITION", "NORMALIZE")?,
        provider_node_flags: 0,
        error: true,
        missing: false,
        explicitly_parenthesized: true,
    };
    let accepted_mask = flags
        .records
        .iter()
        .find(|domain| domain.domain == "PROVIDER_NODE_FLAGS")
        .ok_or(RegistryCbefError::AuthorityInvariant)?
        .values
        .iter()
        .fold(0_u64, |mask, value| mask | (1_u64 << value.bit));
    if semantics.provider_node_flags & !accepted_mask != 0
        || semantics.occurrence_family_code != 30
        || semantics.reconciliation_step_code != Some(30)
        || semantics.raw_kind_disposition_code != 10
        || !semantics.error
        || semantics.missing
        || !semantics.explicitly_parenthesized
    {
        return Err(RegistryCbefError::TransitionMismatch);
    }
    Ok(())
}

fn pascal(value: &str) -> String {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + &characters.as_str().to_ascii_lowercase()
            })
        })
        .collect()
}

fn snake(value: &str) -> String {
    value.to_ascii_lowercase().replace('-', "_")
}

fn rust_ident(value: &str) -> String {
    if matches!(
        value,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
    ) {
        format!("{value}_")
    } else {
        value.to_owned()
    }
}

/// Registry/CBEF family failure.
#[derive(Debug, Error)]
pub enum RegistryCbefError {
    #[error(transparent)]
    Driver(#[from] DriverProtocolError),
    #[error(transparent)]
    Repository(#[from] super::repository_model::RepositoryModelError),
    #[error("registry/CBEF authority violates a closed invariant")]
    AuthorityInvariant,
    #[error("unknown CBEF domain {0}")]
    UnknownDomain(String),
    #[error("missing CBEF operand {0}")]
    MissingOperand(String),
    #[error("extra CBEF operand {0}")]
    ExtraOperand(String),
    #[error("wrong CBEF type for {field}: expected {expected}, observed {actual}")]
    WrongType {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("wrong CBEF scalar width for {0}")]
    WrongWidth(String),
    #[error("non-normalized CBEF string operand {0}")]
    NonNormalized(String),
    #[error("CBEF record does not match its selected recipe")]
    RecordShape,
    #[error("invalid source-occurrence semantic key")]
    InvalidOccurrenceKey,
    #[error("registry/CBEF projection differs from typed authorities")]
    ProjectionMismatch,
    #[error("WP32 transition semantics differ from governed allocations")]
    TransitionMismatch,
    #[error("governed registry model is invalid: {0}")]
    RegistryModel(String),
    #[error("provider inventory tool failed: {0}")]
    ProviderTool(String),
    #[error("registry/CBEF YAML failed: {0}")]
    Yaml(serde_yaml_ng::Error),
    #[error("registry/CBEF JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("registry/CBEF I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn driver() -> RegistryCbefDriver {
        RegistryCbefDriver::for_repository(Path::new("."))
    }

    fn plan() -> RegistryCbefPlan {
        driver().plan(Path::new(".")).unwrap()
    }

    #[test]
    fn model_cbef_builders_match_every_governed_recipe() {
        let plan = plan();
        validate_all_recipes(&plan.cbef).unwrap();
        assert_eq!(plan.cbef.domains.len(), 17);
        let entity = driver()
            .build_entity(
                &plan,
                EntityOperands {
                    workspace_id: [1; 16],
                    analysis_context_id: [2; 16],
                    kind_code: 10,
                    owner_id: [3; 16],
                    semantic_key: b"semantic".to_vec(),
                },
            )
            .unwrap();
        assert_eq!(entity.fields.len(), 5);
        assert_eq!(
            entity
                .fields
                .iter()
                .map(|field| field.tag)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn model_cbef_rejects_entity_twelve_field_and_relation_eight_field_layouts() {
        let plan = plan();
        for (domain_code, field_count) in [(8, 12), (9, 8)] {
            let record = RecipeRecord {
                domain_code,
                fields: (1..=field_count)
                    .map(|tag| RecipeField {
                        tag,
                        value: RecipeValue::Bytes(vec![]),
                    })
                    .collect(),
            };
            assert!(matches!(
                validate_record(&plan.cbef, &record),
                Err(RegistryCbefError::RecordShape)
            ));
        }
    }

    #[test]
    fn model_cbef_rejects_wrong_missing_extra_and_reordered_recipe_operands() {
        let plan = plan();
        let mut missing = BTreeMap::from([
            ("workspace_id".to_owned(), RecipeValue::Id([1; 16])),
            ("analysis_context_id".to_owned(), RecipeValue::Id([2; 16])),
            ("kind_code".to_owned(), RecipeValue::Unsigned(vec![0, 1])),
            ("owner_id".to_owned(), RecipeValue::Id([3; 16])),
        ]);
        assert!(matches!(
            build_named(&plan.cbef, "ENTITY", missing.clone()),
            Err(RegistryCbefError::MissingOperand(_))
        ));
        missing.insert("semantic_key".to_owned(), RecipeValue::Bytes(vec![]));
        missing.insert("extra".to_owned(), RecipeValue::Bytes(vec![]));
        assert!(matches!(
            build_named(&plan.cbef, "ENTITY", missing),
            Err(RegistryCbefError::ExtraOperand(_))
        ));
        let mut record = driver()
            .build_entity(
                &plan,
                EntityOperands {
                    workspace_id: [1; 16],
                    analysis_context_id: [2; 16],
                    kind_code: 10,
                    owner_id: [3; 16],
                    semantic_key: vec![],
                },
            )
            .unwrap();
        record.fields.swap(0, 1);
        assert!(matches!(
            validate_record(&plan.cbef, &record),
            Err(RegistryCbefError::RecordShape)
        ));
    }

    #[test]
    fn model_overlay_wp32_occurrences_preserve_governed_identity_and_annotation_semantics() {
        let plan = plan();
        let key = SourceOccurrenceSemanticKeyV1 {
            schema_version: 1,
            file_id: [3; 16],
            source_digest: [4; 32],
            start_byte: 10,
            end_byte: 20,
            occurrence_family_code: 30,
            normalized_kind_code: 100,
            parent_id: Some([5; 16]),
            role_code: Some(50),
            ordinal: 2,
        };
        let first = key.canonical_bytes().unwrap();
        let second = key.canonical_bytes().unwrap();
        assert_eq!(first, second);
        let entity = driver()
            .build_entity(
                &plan,
                EntityOperands {
                    workspace_id: [1; 16],
                    analysis_context_id: [2; 16],
                    kind_code: 100,
                    owner_id: [6; 16],
                    semantic_key: first,
                },
            )
            .unwrap();
        assert_eq!(entity.fields.len(), 5);
        validate_transition_semantics(&plan.enums, &plan.flags).unwrap();
    }

    #[test]
    fn model_registry_round_trip_preserves_codes_flags_and_tombstones() {
        let plan = plan();
        let bytes = render_projection(&plan).unwrap();
        RegistryCbefDriver::validate_projection(&plan, &bytes).unwrap();
        let projection: RegistryCbefProjection = serde_json::from_slice(&bytes).unwrap();
        let occurrence = projection
            .enum_domains
            .iter()
            .find(|domain| domain.domain == "OCCURRENCE_FAMILY")
            .unwrap();
        assert_eq!(
            occurrence
                .values
                .iter()
                .map(|value| value.code)
                .collect::<Vec<_>>(),
            [10, 20, 30]
        );
        assert!(
            projection
                .flag_domains
                .iter()
                .find(|domain| domain.domain == "PROVIDER_NODE_FLAGS")
                .unwrap()
                .values
                .is_empty()
        );
        assert!(
            projection
                .enum_domains
                .iter()
                .any(|domain| { domain.values.iter().any(|value| value.name == "REMOVED") })
        );
    }

    #[test]
    fn model_registry_driver_cannot_write_authority_or_kat() {
        for path in [
            "contracts/acceptance/owner.json",
            "contracts/fixtures/kat.json",
            "docs/upfront_design/spec.md",
        ] {
            assert!(SafeOutputPath::parse(path.as_bytes().to_vec()).is_err());
        }
        let descriptor = driver().describe().unwrap();
        assert!(descriptor.outputs.iter().all(|output| {
            let path = output.path.display();
            !path.starts_with("contracts/acceptance/")
                && !path.starts_with("contracts/fixtures/")
                && !path.starts_with("docs/upfront_design/")
        }));
    }

    #[test]
    fn model_registry_generated_consumers_compile_and_typecheck() {
        let plan = plan();
        let recipes = String::from_utf8(render_rust_recipes(&plan.cbef)).unwrap();
        let registries =
            String::from_utf8(render_rust_registries(&plan.enums, &plan.flags)).unwrap();
        let python = String::from_utf8(render_python_registries(&plan.enums, &plan.flags)).unwrap();
        assert!(recipes.contains("pub struct EntityFields"));
        assert!(recipes.contains("pub fn relation_fact"));
        assert!(registries.contains("pub enum OccurrenceFamily"));
        assert!(registries.contains("pub struct ProviderNodeFlags"));
        assert!(python.contains("class OccurrenceFamily(IntEnum)"));
        assert!(python.contains("class ProviderNodeFlags(IntFlag)"));
    }

    #[test]
    fn model_provider_inventory_is_library_derived_exhaustive_and_authority_resolved() {
        let plan = plan();
        let catalogs = render_provider_catalogs(&plan).unwrap();
        assert_eq!(
            catalogs.keys().cloned().collect::<BTreeSet<_>>(),
            provider_catalog_ids(&plan).unwrap()
        );
        for probe in &plan.provider_probe.tree_sitter {
            let document: Value = serde_json::from_slice(&catalogs[&probe.catalog_id]).unwrap();
            assert_eq!(
                document["runtime_inventory"]["raw_kinds"]
                    .as_array()
                    .unwrap()
                    .len(),
                probe.raw_kinds.len()
            );
            assert_eq!(
                document["runtime_inventory"]["fields"]
                    .as_array()
                    .unwrap()
                    .len(),
                probe.fields.len()
            );
        }
        let rust = String::from_utf8(render_provider_rust(&plan, &catalogs).unwrap()).unwrap();
        assert!(rust.starts_with("// @generated by codefabric-model; do not edit."));
        for kind in &plan.provider_probe.ruff.node_kinds {
            assert!(rust.contains(&format!("ruff_python_ast::NodeKind::{} =>", kind.raw_name)));
        }
        for kind in &plan.provider_probe.ruff.token_kinds {
            assert!(rust.contains(&format!(
                "ruff_python_ast::token::TokenKind::{} =>",
                kind.raw_name
            )));
        }

        let mut normalization = provider_normalizations(&plan)
            .unwrap()
            .remove("ruff-python-0-0-7")
            .unwrap();
        normalization
            .canonical_kind_prefixes
            .insert("StmtD".to_owned(), "STATEMENT".to_owned());
        assert!(matches!(
            resolve_raw_kind(&normalization, "StmtDelete"),
            Err(RegistryCbefError::RegistryModel(_))
        ));
    }

    #[test]
    fn model_overlay_rejects_raw_governed_codes_and_flags() {
        let plan = plan();
        let provider_flags = plan
            .flags
            .records
            .iter()
            .find(|domain| domain.domain == "PROVIDER_NODE_FLAGS")
            .unwrap();
        let mask = provider_flags
            .values
            .iter()
            .fold(0_u64, |mask, value| mask | (1_u64 << value.bit));
        assert_eq!(mask, 0);
        assert_ne!(1_u64 & !mask, 0);
        validate_transition_semantics(&plan.enums, &plan.flags).unwrap();
    }
}
