//! Closed contract records shared by native ingress adapters.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::catalog::ContractOwner;
use super::catalog::{ArtifactKind, ArtifactStatus, BundleKind, DigestProjection};

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

/// Exact storage, protocol, adapter, and provider boundary identities in the toolchain bundle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainIdentityDocument {
    /// Common governed artifact header.
    #[serde(flatten)]
    pub header: ArtifactHeader,
    /// Declared Rust compatibility floor.
    pub rust_version: String,
    /// Exact Arrow family version.
    pub arrow_version: String,
    /// Exact Parquet version.
    pub parquet_version: String,
    /// Exact DataFusion version.
    pub datafusion_version: String,
    /// Exact `object_store` version.
    pub object_store_version: String,
    /// Immutable delta-rs source revision.
    pub delta_rs_git_rev: String,
    /// Declared pre-release deltalake package version.
    pub deltalake_declared_version: String,
    /// Exact TOML parser used at the daemon configuration boundary.
    pub toml_version: String,
    /// Exact BLAKE3 identity of the root Cargo lock.
    pub cargo_lock_digest: String,
    /// Python serving-boundary pins.
    pub adapter: AdapterToolchainIdentity,
    /// Shared Protobuf compilation-boundary pins.
    pub protobuf: ProtobufToolchainIdentity,
    /// Isolated nightly extractor identity.
    pub rustc_extractor: ExtractorToolchainIdentity,
    /// Isolated Pyrefly sidecar identity.
    pub pyrefly: PyreflyToolchainIdentity,
    /// Provider pins recorded before their later adoption waves.
    pub recorded_provider_pins: ProviderPinSet,
}

/// Exact Python adapter dependency identities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterToolchainIdentity {
    pub python: String,
    pub fastmcp: String,
    pub pydantic: String,
    pub pydantic_settings: String,
    pub grpcio: String,
    pub protobuf: String,
    pub jsonschema: String,
    pub pyyaml: String,
    pub source_digest: String,
}

/// Exact single-FDS generator/runtime identities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProtobufToolchainIdentity {
    pub grpcio_tools: String,
    pub libprotoc: String,
    pub prost: String,
    pub tonic: String,
    pub source_digest: String,
}

/// Exact isolated rustc extractor identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractorToolchainIdentity {
    pub toolchain: String,
    pub rustc_release: String,
    pub rustc_commit_hash: String,
    pub source_digest: String,
}

/// Exact isolated Pyrefly sidecar identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PyreflyToolchainIdentity {
    pub version: String,
    pub git_commit: String,
    pub locked_source_blake3: String,
    pub source_digest: String,
}

/// Exact later-wave provider versions recorded without adopting them early.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderPinSet {
    pub rayon: String,
    pub tree_sitter: String,
    pub tree_sitter_python: String,
    pub tree_sitter_rust: String,
    pub ruff: String,
    pub ruff_component_crates: String,
    pub petgraph: String,
}

/// One supported deployment platform code.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DeploymentPlatform {
    #[serde(rename = "linux-x86_64")]
    LinuxX86_64,
    #[serde(rename = "linux-aarch64")]
    LinuxAarch64,
    #[serde(rename = "macos-aarch64")]
    MacosAarch64,
    #[serde(rename = "macos-x86_64")]
    MacosX86_64,
}

/// Platform-specific root selection and private-mode contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformRootProfile {
    pub platform_family: String,
    pub state_root_options: Vec<String>,
    pub runtime_root_options: Vec<String>,
    pub config_root_options: Vec<String>,
    pub directory_mode: String,
    pub private_file_mode: String,
}

/// Bounded AC-G-33 source-image capture defaults.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceImageLimitProfile {
    pub ordinary_maximum_bytes: u64,
    pub explicit_maximum_bytes: u64,
    pub stable_read_retry_count: u8,
    pub orphan_grace_seconds: u64,
    pub garbage_collection_batch_size: u32,
}

/// One magic-byte prefix admitted by the AC-G-43 binary classifier.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BinarySignatureProfile {
    pub name: String,
    pub prefix_hex: String,
}

/// Model-owned source admission and classification policy.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAdmissionProfile {
    pub binary_sample_bytes: u32,
    pub maximum_single_line_bytes: u32,
    pub maximum_path_components: u16,
    pub maximum_path_bytes: u16,
    pub excluded_directory_names: BTreeSet<String>,
    pub vendored_directory_names: BTreeSet<String>,
    pub generated_directory_names: BTreeSet<String>,
    pub binary_signatures: Vec<BinarySignatureProfile>,
}

/// Six independently enforceable generic-inventory bounds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryLimitProfile {
    pub maximum_file_count: u64,
    pub maximum_directory_count: u64,
    pub maximum_directory_depth: u32,
    pub maximum_total_bytes_considered: u64,
    pub maximum_duration_ms: u64,
    pub maximum_entries_per_directory: u64,
}

/// Bounded continuous-update configuration owned by the deployment profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleLimitProfile {
    pub watch_debounce_timeout_ms: u64,
    pub watch_tick_rate_ms: u64,
    pub watch_ingress_capacity: u16,
    pub maximum_watch_paths_per_batch: u32,
    pub gather_window_ms: u64,
    pub dirty_path_bulk_threshold: u32,
    pub default_await_current_timeout_ms: u64,
    pub overlay_flush_maximum_rows: u64,
    pub overlay_flush_maximum_bytes: u64,
    pub overlay_flush_maximum_touched_owners: u64,
    pub overlay_flush_maximum_generations: u64,
}

/// Closed AC-G-08 local workstation deployment profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentProfileDocument {
    /// Common governed artifact header.
    #[serde(flatten)]
    pub header: ArtifactHeader,
    pub profile_id: String,
    pub supported_platforms: BTreeSet<DeploymentPlatform>,
    pub windows_support: String,
    pub network_listeners: String,
    pub workspace_registration: String,
    pub operational_store: String,
    pub fact_store: String,
    pub object_store: String,
    pub hot_overlay_journal: String,
    pub source_blob_persistence: String,
    pub result_artifact_ttl_seconds: u32,
    pub source_result_artifact_ttl_seconds: u32,
    pub coordinator_command_capacity: u16,
    pub maximum_concurrent_source_reads: u16,
    pub maximum_concurrent_gix_jobs: u16,
    pub source_image_limits: SourceImageLimitProfile,
    pub source_admission: SourceAdmissionProfile,
    pub inventory_limits: InventoryLimitProfile,
    pub lifecycle_limits: LifecycleLimitProfile,
    pub default_query_freshness: String,
    pub provider_sandbox: String,
    pub follow_directory_symlinks: bool,
    pub follow_internal_file_symlinks: bool,
    pub index_external_dependency_bodies: bool,
    pub semantic_query_language: String,
    pub canonical_json: String,
    pub platform_roots: Vec<PlatformRootProfile>,
}

/// One versioned adversarial security case.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityCaseRecord {
    pub case_id: String,
    pub threat_class: String,
    pub required_platforms: Vec<String>,
    pub fixture_path: String,
    pub operation: String,
    pub expected_status_or_error: String,
    pub expected_public_fields: Vec<String>,
    pub forbidden_observations: Vec<String>,
    pub resource_bounds: SecurityResourceBounds,
    pub cleanup_assertions: Vec<String>,
}

/// Bounded resource contract for one adversarial case.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityResourceBounds {
    pub maximum_input_bytes: u64,
    pub maximum_output_bytes: u64,
    pub maximum_duration_ms: u64,
}

/// Closed AC-G-84 security-corpus manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityCorpusManifest {
    /// Common governed artifact header.
    #[serde(flatten)]
    pub header: ArtifactHeader,
    pub corpus_id: String,
    pub corpus_version: String,
    pub records: Vec<SecurityCaseRecord>,
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
    /// The requirement was replaced while its stable identity remains reserved.
    Superseded,
}

/// Closed catalog-backed expansions used to derive trace edges without duplicating
/// registry and schema inventories in the requirements authority.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TraceSelector {
    /// Every released ontology entity, relation, property, fact, and unknown kind.
    AllOntologyKinds,
    /// Every released generation capability code.
    AllCapabilityCodes,
    /// Every released schema-contract table field.
    AllTableFields,
    /// Every released semantic-query phrase identity.
    AllQueryPhraseIds,
    /// Every released public response-schema field.
    AllResponseFields,
    /// Every released public error code.
    AllErrorCodes,
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
    /// Catalog-backed groups expanded into `traces_to` by the contract generator.
    #[serde(default)]
    pub trace_selectors: BTreeSet<TraceSelector>,
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
    /// Fully expanded typed cross-domain trace edges.
    pub traces_to: RequirementTraces,
    /// Verification obligations.
    pub verified_by: Vec<String>,
}

/// Committed expected-failure trace record exercised by the release verifier.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrokenTraceEdgeFixture {
    /// Stable negative-fixture identity.
    pub fixture_id: String,
    /// Governed traceability artifact receiving the candidate edge.
    pub target_artifact: String,
    /// Candidate trace record that must be rejected.
    pub trace: TraceabilityRecord,
    /// Stable target failure class.
    pub expected_failure_class: String,
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
