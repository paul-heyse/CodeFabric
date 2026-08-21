//! Closed contract records shared by native ingress adapters.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::catalog::ContractOwner;
use super::catalog::{ArtifactKind, ArtifactStatus, DigestProjection};

/// Typed identity header embedded in machine-readable contract authorities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactHeader {
    /// Stable identity which must equal the catalog descriptor.
    pub artifact_id: String,
    /// Closed artifact kind.
    pub artifact_kind: ArtifactKind,
    /// Two-component public version.
    pub version: String,
    /// Compatible suite major.
    pub compatible_suite_major: u16,
    /// Release state.
    pub status: ArtifactStatus,
    /// Embedded semantic identity or the prose `external` sentinel.
    pub canonical_digest: String,
    /// Optional explicit projection; the catalog remains authoritative when absent.
    pub digest_projection: Option<DigestProjection>,
    /// Optional generator identity on generated authorities.
    pub generator_revision: Option<String>,
}

/// Closed compatibility-bundle family defined by AC-G-07.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BundleKind {
    /// Derived-analysis contracts.
    Derivation,
    /// Externally extensible model-pack contracts.
    ModelPack,
    /// Fact ontology contracts.
    Ontology,
    /// Provider contracts.
    Provider,
    /// Semantic query-language contracts.
    QueryLanguage,
    /// Public storage and response schema contracts.
    Schema,
    /// MCP tool-contract models.
    ToolContract,
    /// Exact compiler and storage-substrate identities.
    Toolchain,
}

impl BundleKind {
    /// Stable artifact-ID component for this bundle family.
    #[must_use]
    pub const fn artifact_slug(self) -> &'static str {
        match self {
            Self::Derivation => "derivation",
            Self::ModelPack => "model-pack",
            Self::Ontology => "ontology",
            Self::Provider => "provider",
            Self::QueryLanguage => "query-language",
            Self::Schema => "schema",
            Self::ToolContract => "tool-contract",
            Self::Toolchain => "toolchain",
        }
    }
}

/// One compatibility-sensitive artifact retained in an AC-G-07 bundle identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleMember {
    /// Stable governed artifact identity.
    pub artifact_id: String,
    /// Exact member contract version.
    pub version: String,
    /// Exact member semantic identity; this is never omitted by bundle projection.
    pub canonical_digest: String,
    /// Whether a consumer must understand this member.
    pub required: bool,
    /// Closed-feature bits interpreted by the owning bundle family.
    pub feature_bits: BTreeSet<String>,
}

/// Consumer-minor compatibility interval for a bundle.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleCompatibility {
    /// Oldest consumer minor version accepted by the bundle.
    pub minimum_consumer_minor: u16,
    /// Newest consumer minor version accepted by the bundle.
    pub maximum_consumer_minor: u16,
}

/// Generator provenance carried by an AC-G-07 bundle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleCreatedBy {
    /// Stable generator implementation identity.
    pub generator_id: String,
    /// Exact generator contract version.
    pub generator_version: String,
}

/// Closed typed AC-G-07 bundle authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleDocument {
    /// Stable identity which must equal the catalog descriptor.
    pub artifact_id: String,
    /// Closed artifact kind; always `bundle-manifest` for this model.
    pub artifact_kind: ArtifactKind,
    /// Two-component public version.
    pub version: String,
    /// Compatible suite major.
    pub compatible_suite_major: u16,
    /// Release state.
    pub status: ArtifactStatus,
    /// Embedded semantic identity.
    pub canonical_digest: String,
    /// Optional explicit projection; the catalog remains authoritative when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest_projection: Option<DigestProjection>,
    /// Optional generator identity on generated authorities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator_revision: Option<String>,
    /// Independently negotiated bundle family.
    pub bundle_kind: BundleKind,
    /// Public bundle contract version.
    pub bundle_version: String,
    /// Public major extracted from `bundle_version`.
    pub bundle_major: u16,
    /// Compatibility-sensitive members, normalized by `artifact_id`.
    pub artifacts: Vec<BundleMember>,
    /// Supported consumer-minor interval.
    pub compatibility: BundleCompatibility,
    /// Generator provenance.
    pub created_by: BundleCreatedBy,
    /// Embedded AC-G-07 projection identity.
    pub bundle_digest: String,
    /// Optional external model-pack signature; omitted from the bundle identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// The first record of a governed JSON Lines source.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JsonlMetadata {
    /// Stable artifact identity.
    pub artifact_id: String,
    /// Closed artifact kind.
    pub artifact_kind: ArtifactKind,
    /// Public version.
    pub version: String,
    /// Compatible suite major.
    pub compatible_suite_major: u16,
    /// Release state.
    pub status: ArtifactStatus,
    /// Embedded semantic identity.
    pub canonical_digest: String,
    /// Optional explicit projection.
    pub digest_projection: Option<DigestProjection>,
    /// Optional generator identity.
    pub generator_revision: Option<String>,
    /// Stable discriminator.
    pub record_kind: MetadataRecordKind,
}

/// Closed metadata-record discriminator.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetadataRecordKind {
    /// Artifact header record.
    Metadata,
}

/// Requirement lifecycle state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RequirementStatus {
    /// The requirement is enforced.
    Active,
    /// The requirement is retained for compatibility but no longer newly authored.
    Deprecated,
}

/// Closed cross-domain trace references carried by a requirement.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementTraces {
    /// Ontology kinds used by the requirement.
    pub ontology_kinds: Vec<String>,
    /// Capability codes used by the requirement.
    pub capability_codes: Vec<String>,
    /// Table fields used by the requirement.
    pub table_fields: Vec<String>,
    /// Query phrase identities used by the requirement.
    pub query_phrase_ids: Vec<String>,
    /// Response fields used by the requirement.
    pub response_fields: Vec<String>,
    /// Error codes used by the requirement.
    pub error_codes: Vec<String>,
}

/// Human-owned provenance attached to normative transcription.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerAcceptance {
    /// Accountable approver identity.
    pub approver: String,
    /// ISO date of acceptance.
    pub accepted_at: String,
    /// How the evidence was constructed.
    pub construction_rule: String,
    /// Detached digest of the source authority used by the approver.
    pub source_digest: String,
}

/// Header shared by the three closed Wave-1 identity authorities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityContractHeader {
    /// Stable identity which must equal the catalog descriptor.
    pub artifact_id: String,
    /// Closed artifact kind.
    pub artifact_kind: ArtifactKind,
    /// Two-component public version.
    pub version: String,
    /// Compatible suite major.
    pub compatible_suite_major: u16,
    /// Release state.
    pub status: ArtifactStatus,
    /// Embedded semantic identity.
    pub canonical_digest: String,
    /// Optional explicit projection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest_projection: Option<DigestProjection>,
    /// Optional generator identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator_revision: Option<String>,
}

impl IdentityContractHeader {
    /// Borrow the identity authority header as the shared validation model.
    #[must_use]
    pub fn artifact_header(&self) -> ArtifactHeader {
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

/// One fixed-width member of a CBEF frame.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CbefFrameMember {
    /// Stable field name.
    pub name: String,
    /// Width in bytes, or zero for the variable payload.
    pub width_bytes: u8,
}

/// One closed CBEF core type-code allocation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CbefTypeCodeRecord {
    /// Append-only numeric code.
    pub code: u8,
    /// Canonical `SCREAMING_SNAKE_CASE` name.
    pub name: String,
    /// Exact payload rule.
    pub payload: String,
}

/// One owner-accepted field in a domain recipe.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CbefRecipeField {
    /// One-based append-only field tag.
    pub tag: u16,
    /// Stable semantic field name.
    pub name: String,
    /// Name from the CBEF type-code table.
    pub type_code: String,
    /// Fixed integer width where applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_bytes: Option<u8>,
    /// Schema-owned UTF-8 normalization rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalization: Option<String>,
    /// Container member shape where applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_type: Option<String>,
}

/// One owner-accepted CBEF domain and its ordered recipe.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CbefDomainRecipe {
    /// Declaration-order domain code.
    pub code: u16,
    /// Canonical domain name.
    pub name: String,
    /// Registry-owned public prefix.
    pub public_prefix: String,
    /// Whether the public form carries a kind slug.
    pub kind_slug_required: bool,
    /// One-based field recipe in tag order.
    pub fields: Vec<CbefRecipeField>,
}

/// Closed, typed AC-G-13 identity authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CbefContract {
    /// Common governed artifact header.
    #[serde(flatten)]
    pub header: IdentityContractHeader,
    /// Format name; fixed to `CBEF-v1`.
    pub format_name: String,
    /// Binary format version byte.
    pub format_version: u8,
    /// Four-byte ASCII magic.
    pub magic_ascii: String,
    /// Fixed byte order.
    pub byte_order: String,
    /// Exact record framing.
    pub record_frame: Vec<CbefFrameMember>,
    /// Exact field framing.
    pub field_frame: Vec<CbefFrameMember>,
    /// Closed core type-code table.
    pub type_codes: Vec<CbefTypeCodeRecord>,
    /// Closed ordered domain recipe table.
    pub domains: Vec<CbefDomainRecipe>,
    /// Full digest algorithm.
    pub digest_algorithm: String,
    /// Truncation rule for internal IDs.
    pub id_derivation: String,
    /// Sole symbolic public identity.
    pub symbolic_source_context: String,
    /// Suite-defined internal bytes for the symbolic source context.
    pub symbolic_source_context_id_hex: String,
    /// Blocking collision error code.
    pub collision_error: String,
    /// Human acceptance of the initial generated allocation.
    pub owner_acceptance: OwnerAcceptance,
}

/// One closed AC-G-18 platform allocation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PathPlatformRule {
    /// Reserved platform byte.
    pub code: u8,
    /// Canonical platform name.
    pub name: String,
    /// Released runtime support state.
    pub runtime_status: String,
    /// Exact comparison-key rule.
    pub comparison: String,
}

/// Closed, typed AC-G-18 path authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PathCanonicalizationContract {
    /// Common governed artifact header.
    #[serde(flatten)]
    pub header: IdentityContractHeader,
    /// Internal component separator.
    pub component_separator: String,
    /// Bytes always percent-escaped inside a component.
    pub escaped_bytes: Vec<String>,
    /// Required percent-hex case.
    pub percent_hex_case: String,
    /// Whether decoding restores the exact component bytes.
    pub reversible: bool,
    /// Whether symlinks participate in path canonicalization.
    pub resolves_symlinks: bool,
    /// Closed platform-code registry.
    pub platforms: Vec<PathPlatformRule>,
    /// Canonical URI template.
    pub canonical_uri_template: String,
    /// Ordered list of comparison tuple members.
    pub ordering: Vec<String>,
    /// Collision policy for distinct raw paths.
    pub collision_error: String,
    /// Human acceptance of the initial allocation.
    pub owner_acceptance: OwnerAcceptance,
}

/// One operand in a canonical type constructor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypeOperand {
    /// Stable operand name.
    pub name: String,
    /// Closed semantic operand role.
    pub role: String,
}

/// One append-only AC-G-15 constructor allocation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypeConstructorRecord {
    /// One-based constructor code.
    pub code: u16,
    /// Canonical constructor name.
    pub name: String,
    /// Ordered constructor operands.
    pub operands: Vec<TypeOperand>,
}

/// Closed canonicalization switches for type terms.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
pub struct TypeNormalizationRules {
    /// Flatten nested unions/intersections before identity.
    pub flatten_set_constructors: bool,
    /// Sort and deduplicate union/intersection member IDs.
    pub sort_unique_member_ids: bool,
    /// Rewrite Python Optional as Union with `NullNone`.
    pub python_optional_to_union: bool,
    /// Preserve aliases as first-class terms.
    pub aliases_are_first_class: bool,
    /// Encode bound variables with de Bruijn indexes.
    pub de_bruijn_binders: bool,
    /// Forbid debug strings as canonical values.
    pub provider_debug_strings_forbidden: bool,
}

/// Closed, typed AC-G-15 type-algebra authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypeAlgebraContract {
    /// Common governed artifact header.
    #[serde(flatten)]
    pub header: IdentityContractHeader,
    /// Compatibility-sensitive algebra version.
    pub algebra_version: u16,
    /// Append-only constructor registry.
    pub constructors: Vec<TypeConstructorRecord>,
    /// Canonical term normalization contract.
    pub normalization: TypeNormalizationRules,
    /// CBEF domain that owns type IDs.
    pub identity_domain: String,
    /// Scope inputs required by type identity.
    pub identity_scope: Vec<String>,
    /// Human acceptance of the initial allocation.
    pub owner_acceptance: OwnerAcceptance,
}

/// One closed requirements.jsonl data record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementRecord {
    /// Stable requirement identity.
    pub requirement_id: String,
    /// Owning prose artifact.
    pub source_artifact: String,
    /// Owning section.
    pub source_section: String,
    /// Exact normalized normative statement.
    pub normative_text: String,
    /// Digest of the normative statement.
    pub normative_text_digest: String,
    /// Implementation surfaces.
    pub implements: Vec<String>,
    /// Typed cross-domain trace groups.
    pub traces_to: RequirementTraces,
    /// Verification obligations.
    pub verified_by: Vec<String>,
    /// Human provenance.
    pub owner_acceptance: OwnerAcceptance,
    /// Requirement lifecycle state.
    pub status: RequirementStatus,
}

/// One closed traceability.jsonl data record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TraceabilityRecord {
    /// Stable requirement identity.
    pub requirement_id: String,
    /// Implementation surfaces.
    pub implements: Vec<String>,
    /// Verification obligations.
    pub verified_by: Vec<String>,
}

/// Closed fixture-oracle evidence class.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FixtureOracleClass {
    /// Small owner-reviewed known-answer authority.
    NormativeKat,
    /// Output-free cross-implementation equivalence input.
    Differential,
    /// Property or fuzz seed with no stored output answer.
    Property,
    /// Stable expected failure class.
    NegativeClass,
    /// Compiler-owned illustrative or baseline output.
    GeneratedExample,
}

/// One classified fixture with review provenance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureOracleRecord {
    /// Repository-relative fixture path.
    pub path: String,
    /// Evidence and mutability class.
    pub oracle_class: FixtureOracleClass,
    /// Independent origin or derivation statement.
    pub origin: String,
    /// Accountable design owner.
    pub owner: ContractOwner,
    /// Fixture-contract version.
    pub version: String,
    /// Versioned human-readable change record.
    pub change_record: String,
}

/// Typed fixture-oracle manifest authority.
pub type FixtureOracleManifest = RegistryDocument<FixtureOracleRecord>;

/// Typed registry envelope; record models remain owned by each registry family.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryDocument<T> {
    /// Stable artifact identity.
    pub artifact_id: String,
    /// Closed artifact kind.
    pub artifact_kind: ArtifactKind,
    /// Public version.
    pub version: String,
    /// Compatible suite major.
    pub compatible_suite_major: u16,
    /// Release state.
    pub status: ArtifactStatus,
    /// Embedded semantic identity.
    pub canonical_digest: String,
    /// Optional explicit projection.
    pub digest_projection: Option<DigestProjection>,
    /// Optional generator identity.
    pub generator_revision: Option<String>,
    /// Family-owned registry records.
    pub records: Vec<T>,
}

/// Generic YAML-contract envelope used by current empty Wave-1 scaffolds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScaffoldDocument<T> {
    /// Stable artifact identity.
    pub artifact_id: String,
    /// Closed artifact kind.
    pub artifact_kind: ArtifactKind,
    /// Public version.
    pub version: String,
    /// Compatible suite major.
    pub compatible_suite_major: u16,
    /// Release state.
    pub status: ArtifactStatus,
    /// Embedded semantic identity.
    pub canonical_digest: String,
    /// Optional explicit projection.
    pub digest_projection: Option<DigestProjection>,
    /// Optional generator identity.
    pub generator_revision: Option<String>,
    /// Family-owned records, when the authority is record-shaped.
    pub records: Option<Vec<T>>,
    /// Family-owned rules, when the authority is rule-shaped.
    pub rules: Option<Vec<T>>,
}

impl JsonlMetadata {
    /// Borrow the metadata as a common typed header.
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

impl BundleDocument {
    /// Borrow the bundle envelope as a common typed header.
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

impl<T> RegistryDocument<T> {
    /// Borrow the registry envelope as a common typed header.
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

impl<T> ScaffoldDocument<T> {
    /// Borrow the scaffold envelope as a common typed header.
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
