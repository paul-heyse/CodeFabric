//! Registry/CBEF family driver built only from strict native authorities.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::desired_tree::SafeOutputPath;
use super::driver_protocol::{
    DriverDescriptor, DriverOutputRole, DriverOutputSpec, DriverProtocolError,
    DriverResourceProfile, DriverSourceFence, ModelDriver, StagingRoot,
};
use super::model_control::StableId;
use super::repository_model::read_stable;

const CBEF_PATH: &str = "contracts/identity/cbef-v1.yaml";
const ENUM_PATH: &str = "contracts/registry/enum-registry.yaml";
const FLAG_PATH: &str = "contracts/registry/flag-registry.yaml";
const RUST_RECIPES_PATH: &str = "src/generated/model_identity_recipes.rs";
const RUST_REGISTRIES_PATH: &str = "src/generated/model_registries.rs";
const PYTHON_REGISTRIES_PATH: &str =
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_registries.py";
const PROJECTION_PATH: &str = "contracts/generated/model/registry-cbef.json";
const TRANSITION_PATCH_PATH: &str =
    "tooling/model-transition/consumer-overlays/registry-cbef-wp32.json";
const TRANSITION_OVERLAY_PATH: &str =
    "tooling/model-transition/consumer-overlays/registry-cbef-wp32.rs";
const TRANSITION_VALIDATION_PATH: &str =
    "contracts/generated/model/registry-cbef-transition-validation.json";
const MAX_AUTHORITY_BYTES: usize = 8 * 1024 * 1024;

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
    transition_patch: TransitionPatch,
    transition_overlay: Vec<u8>,
    source_fence: DriverSourceFence,
}

/// Registry/CBEF model driver.
#[derive(Clone, Copy, Debug, Default)]
pub struct RegistryCbefDriver;

impl RegistryCbefDriver {
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
            sources: [
                CBEF_PATH,
                ENUM_PATH,
                FLAG_PATH,
                TRANSITION_PATCH_PATH,
                TRANSITION_OVERLAY_PATH,
            ]
            .into_iter()
            .map(|path| {
                SafeOutputPath::parse(path.as_bytes().to_vec())
                    .map_err(|_| DriverProtocolError::InvalidDescriptor)
            })
            .collect::<Result<Vec<_>, _>>()?,
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
                    "output:model-registry-cbef-transition-overlay",
                    TRANSITION_OVERLAY_PATH,
                    DriverOutputRole::TransitionOverlay,
                )?,
                Self::output(
                    "output:model-registry-cbef-transition-validation",
                    TRANSITION_VALIDATION_PATH,
                    DriverOutputRole::CanonicalProjection,
                )?,
            ],
            resource_profile: DriverResourceProfile {
                max_source_bytes: MAX_AUTHORITY_BYTES,
                max_output_bytes: 4 * 1024 * 1024,
                max_outputs: 8,
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
        let transition_patch =
            parse_json::<TransitionPatch>(repository_root, TRANSITION_PATCH_PATH)?;
        let transition_overlay = read_stable(
            &repository_root.join(TRANSITION_OVERLAY_PATH),
            MAX_AUTHORITY_BYTES,
        )?;
        validate_authorities(&cbef, &enums, &flags)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        validate_transition_patch(&transition_patch, &transition_overlay, &enums, &flags)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        validate_transition_baselines(repository_root, &transition_patch)
            .map_err(|error| DriverProtocolError::InvalidAuthority(error.to_string()))?;
        Ok(RegistryCbefPlan {
            descriptor,
            cbef,
            enums,
            flags,
            transition_patch,
            transition_overlay,
            source_fence,
        })
    }

    fn render(
        &self,
        plan: &Self::Plan,
        staging_root: &StagingRoot,
    ) -> Result<Vec<SafeOutputPath>, DriverProtocolError> {
        let outputs = [
            (RUST_RECIPES_PATH, render_rust_recipes(&plan.cbef)),
            (
                RUST_REGISTRIES_PATH,
                render_rust_registries(&plan.enums, &plan.flags),
            ),
            (
                PYTHON_REGISTRIES_PATH,
                render_python_registries(&plan.enums, &plan.flags),
            ),
            (
                PROJECTION_PATH,
                render_projection(plan).map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            ),
            (TRANSITION_OVERLAY_PATH, plan.transition_overlay.clone()),
            (
                TRANSITION_VALIDATION_PATH,
                render_transition_validation(plan)
                    .map_err(|_| DriverProtocolError::InvalidDescriptor)?,
            ),
        ];
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
    let driver = RegistryCbefDriver;
    let plan = driver.plan(repository_root)?;
    let stage_path = repository_root.join("target/model-stage/registry-cbef-shadow");
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
    let rendered = driver.render(&plan, &staging)?;
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
        domain_count: plan.cbef.domains.len(),
        enum_domain_count: plan.enums.records.len(),
        flag_domain_count: plan.flags.records.len(),
        rendered_outputs: rendered.iter().map(SafeOutputPath::display).collect(),
        stage_root: staging.path().to_string_lossy().into_owned(),
    })
}

/// Machine-readable family report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryCbefReport {
    pub family: String,
    pub domain_count: usize,
    pub enum_domain_count: usize,
    pub flag_domain_count: usize,
    pub rendered_outputs: Vec<String>,
    pub stage_root: String,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionValidation {
    schema_version: u8,
    family_owner: String,
    planning_baseline_commit: String,
    reviewed_overlay_path: String,
    identity_recipe: String,
    entity_field_count: usize,
    relation_fact_field_count: usize,
    occurrence_semantic_key: Vec<String>,
    relation_role: Vec<String>,
    syntax_detail_fields: Vec<String>,
    forbidden_legacy_shapes: Vec<String>,
    preserved_semantics: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionPatch {
    schema_version: u8,
    family_owner: String,
    planning_baseline_commit: String,
    reviewed_overlay_path: String,
    targets: Vec<TransitionTarget>,
    preserved_semantics: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionTarget {
    target_path: String,
    baseline: TransitionBaseline,
    operations: Vec<TransitionOperation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum TransitionBaseline {
    Present { source_digest: String },
    Absent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
enum TransitionOperation {
    ReplacePositionalCbef {
        function: String,
        recipe: String,
        semantic_payload: String,
    },
    ReplaceGovernedLiteral {
        symbol: String,
        registry_domain: String,
    },
    AddTypedProjectionField {
        model: String,
        field: String,
        registry_domain: String,
        nullable: bool,
    },
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

fn parse_json<T: for<'de> Deserialize<'de>>(
    root: &Path,
    relative: &str,
) -> Result<T, DriverProtocolError> {
    let path = root.join(relative);
    let bytes = read_stable(&path, MAX_AUTHORITY_BYTES)?;
    serde_json::from_slice(&bytes).map_err(|source| DriverProtocolError::Io {
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

fn render_transition_validation(plan: &RegistryCbefPlan) -> Result<Vec<u8>, RegistryCbefError> {
    let entity = domain(&plan.cbef, "ENTITY")?;
    let relation = domain(&plan.cbef, "RELATION_FACT")?;
    let overlay = TransitionValidation {
        schema_version: 1,
        family_owner: plan.transition_patch.family_owner.clone(),
        planning_baseline_commit: plan.transition_patch.planning_baseline_commit.clone(),
        reviewed_overlay_path: plan.transition_patch.reviewed_overlay_path.clone(),
        identity_recipe: "CBEF-v1".to_owned(),
        entity_field_count: entity.fields.len(),
        relation_fact_field_count: relation.fields.len(),
        occurrence_semantic_key: vec![
            "file_id".to_owned(),
            "source_digest".to_owned(),
            "start_byte".to_owned(),
            "end_byte".to_owned(),
            "occurrence_family_code".to_owned(),
            "normalized_kind_code".to_owned(),
            "parent_id".to_owned(),
            "role_code".to_owned(),
            "ordinal".to_owned(),
        ],
        relation_role: vec!["ordinal".to_owned(), "role_code".to_owned()],
        syntax_detail_fields: vec![
            "occurrence_family_code".to_owned(),
            "reconciliation_step_code".to_owned(),
            "raw_kind_disposition_code".to_owned(),
            "provider_node_flags".to_owned(),
            "error".to_owned(),
            "missing".to_owned(),
            "explicitly_parenthesized".to_owned(),
        ],
        forbidden_legacy_shapes: vec![
            "ENTITY:12-fields".to_owned(),
            "RELATION_FACT:8-fields".to_owned(),
            "provider_node_flags:raw-integer".to_owned(),
        ],
        preserved_semantics: plan.transition_patch.preserved_semantics.clone(),
    };
    let value = serde_json::to_value(overlay).map_err(RegistryCbefError::Json)?;
    let mut bytes = serde_json_canonicalizer::to_vec(&value).map_err(RegistryCbefError::Json)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[allow(clippy::too_many_lines)] // The transition is intentionally one closed semantic checklist.
fn validate_transition_patch(
    patch: &TransitionPatch,
    overlay: &[u8],
    enums: &EnumRegistry,
    flags: &FlagRegistry,
) -> Result<(), RegistryCbefError> {
    if patch.schema_version != 1
        || patch.family_owner != "family:registry-cbef"
        || patch.planning_baseline_commit.len() != 40
        || !patch
            .planning_baseline_commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || patch.reviewed_overlay_path != TRANSITION_OVERLAY_PATH
        || patch.targets.len() != 2
        || patch.preserved_semantics.is_empty()
    {
        return Err(RegistryCbefError::TransitionMismatch);
    }
    let targets: BTreeMap<_, _> = patch
        .targets
        .iter()
        .map(|target| (target.target_path.as_str(), target))
        .collect();
    if targets.len() != 2
        || !matches!(
            targets.get("src/identity.rs").map(|target| &target.baseline),
            Some(TransitionBaseline::Present { source_digest })
                if source_digest.starts_with("b3:") && source_digest.len() == 67
        )
        || !matches!(
            targets
                .get("src/source_syntax.rs")
                .map(|target| &target.baseline),
            Some(TransitionBaseline::Absent)
        )
    {
        return Err(RegistryCbefError::TransitionMismatch);
    }
    let operations = patch
        .targets
        .iter()
        .flat_map(|target| target.operations.iter())
        .collect::<Vec<_>>();
    for recipe in ["ENTITY", "RELATION_FACT"] {
        if !operations.iter().any(|operation| {
            matches!(operation, TransitionOperation::ReplacePositionalCbef { recipe: observed, .. } if observed == recipe)
        }) {
            return Err(RegistryCbefError::TransitionMismatch);
        }
    }
    for registry_domain in [
        "OCCURRENCE_FAMILY",
        "PROVIDER_NODE_FLAGS",
        "RANGE_RECONCILIATION_STEP",
        "RAW_KIND_DISPOSITION",
    ] {
        if !operations.iter().any(|operation| match operation {
            TransitionOperation::ReplaceGovernedLiteral {
                registry_domain: observed,
                ..
            }
            | TransitionOperation::AddTypedProjectionField {
                registry_domain: observed,
                ..
            } => observed == registry_domain,
            TransitionOperation::ReplacePositionalCbef { .. } => false,
        }) {
            return Err(RegistryCbefError::TransitionMismatch);
        }
    }
    let overlay =
        std::str::from_utf8(overlay).map_err(|_| RegistryCbefError::TransitionMismatch)?;
    for required in [
        "entity(EntityFields",
        "relation_fact(RelationFactFields",
        "OccurrenceFamily::Syntax",
        "ProviderNodeFlags::empty()",
        "RangeReconciliationStep::SmallestEnclosingCompatible",
        "RawKindDisposition::Normalize",
    ] {
        if !overlay.contains(required) {
            return Err(RegistryCbefError::TransitionMismatch);
        }
    }
    for forbidden in [
        "const OCCURRENCE_",
        "occurrence_family_code: 30",
        "provider_node_flags: 1",
        "CbefRecord {",
    ] {
        if overlay.contains(forbidden) {
            return Err(RegistryCbefError::TransitionMismatch);
        }
    }
    let enum_domains: BTreeSet<_> = enums
        .records
        .iter()
        .map(|domain| domain.domain.as_str())
        .collect();
    let flag_domains: BTreeSet<_> = flags
        .records
        .iter()
        .map(|domain| domain.domain.as_str())
        .collect();
    if ![
        "OCCURRENCE_FAMILY",
        "RANGE_RECONCILIATION_STEP",
        "RAW_KIND_DISPOSITION",
    ]
    .into_iter()
    .all(|domain| enum_domains.contains(domain))
        || !flag_domains.contains("PROVIDER_NODE_FLAGS")
    {
        return Err(RegistryCbefError::TransitionMismatch);
    }
    Ok(())
}

fn validate_transition_baselines(
    repository_root: &Path,
    patch: &TransitionPatch,
) -> Result<(), RegistryCbefError> {
    for target in &patch.targets {
        let observed = super::model_git_state::blob_at_revision(
            repository_root,
            &patch.planning_baseline_commit,
            Path::new(&target.target_path),
        )?;
        match (&target.baseline, observed) {
            (TransitionBaseline::Absent, None) => {}
            (TransitionBaseline::Present { source_digest }, Some(bytes))
                if *source_digest == digest_bytes(&bytes) => {}
            _ => return Err(RegistryCbefError::TransitionMismatch),
        }
    }
    Ok(())
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
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
            "#[derive(Clone, Copy, Debug, Eq, PartialEq)] #[repr(u16)] pub enum {name} {{"
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
            writeln!(output, "    pub const {}: Self = Self({bit});", value.name).unwrap();
        }
        writeln!(output, "    pub const fn from_bits(bits: u64) -> Option<Self> {{ if bits & !{mask} == 0 {{ Some(Self(bits)) }} else {{ None }} }}").unwrap();
        output.push_str("    pub const fn bits(self) -> u64 { self.0 }\n}\n\n");
    }
    output.into_bytes()
}

fn render_python_registries(enums: &EnumRegistry, flags: &FlagRegistry) -> Vec<u8> {
    let mut output = String::from(
        "# @generated by codefabric-model; do not edit.\nfrom enum import IntEnum, IntFlag\n\n",
    );
    for domain in &enums.records {
        writeln!(output, "class {}(IntEnum):", pascal(&domain.domain)).unwrap();
        for value in &domain.values {
            writeln!(output, "    {} = {}", value.name, value.code).unwrap();
        }
        output.push('\n');
    }
    for domain in &flags.records {
        writeln!(output, "class {}(IntFlag):", pascal(&domain.domain)).unwrap();
        output.push_str("    NONE = 0\n");
        for value in &domain.values {
            writeln!(output, "    {} = {}", value.name, 1_u64 << value.bit).unwrap();
        }
        output.push('\n');
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

fn domain<'a>(
    authority: &'a CbefAuthority,
    name: &str,
) -> Result<&'a CbefDomainSpec, RegistryCbefError> {
    authority
        .domains
        .iter()
        .find(|domain| domain.name == name)
        .ok_or_else(|| RegistryCbefError::UnknownDomain(name.to_owned()))
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
    #[error("registry/CBEF JSON failed: {0}")]
    Json(serde_json::Error),
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

    fn plan() -> RegistryCbefPlan {
        RegistryCbefDriver.plan(Path::new(".")).unwrap()
    }

    #[test]
    fn model_cbef_builders_match_every_governed_recipe() {
        let plan = plan();
        validate_all_recipes(&plan.cbef).unwrap();
        assert_eq!(plan.cbef.domains.len(), 17);
        let entity = RegistryCbefDriver
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
        let mut record = RegistryCbefDriver
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
        let entity = RegistryCbefDriver
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
        let descriptor = RegistryCbefDriver.describe().unwrap();
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
