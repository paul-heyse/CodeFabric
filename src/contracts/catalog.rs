//! Typed catalog and derivation graph for governed contract artifacts.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read as _, Take};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The sole production bootstrap path for the contract compiler.
pub const CATALOG_PATH: &str = "contracts/manifests/suite-manifest.json";

/// Stable derivation IDs used by maintained generators.
pub const ARTIFACT_INDEX_DERIVATION_ID: &str = "codefabric.derivation.artifact-index";
pub const REGISTRY_DERIVATION_ID: &str = "codefabric.derivation.canonical-registries";
pub const PROVIDER_RAW_DERIVATION_ID: &str = "codefabric.derivation.provider-raw-catalogs";
pub const PRODUCTION_PROTO_PYTHON_DERIVATION_ID: &str =
    "codefabric.derivation.production-proto-descriptor-python";
pub const PRODUCTION_PROTO_RUST_DERIVATION_ID: &str = "codefabric.derivation.production-proto-rust";
pub const ADAPTER_MODEL_DERIVATION_ID: &str = "codefabric.derivation.adapter-models";

/// Hard bootstrap cap checked before the catalog can select its own named profile.
pub const CATALOG_BOOTSTRAP_MAX_BYTES: usize = 262_144;

/// Public release state carried by every catalog descriptor.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactStatus {
    /// Content is incomplete and release verification reports it.
    Draft,
    /// Content is release-governed.
    Released,
    /// Content remains decodable but should not be newly emitted.
    Deprecated,
}

/// Closed source-kind codes used by current governed artifacts.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    /// One of the authoritative design documents or subordinate roadmap.
    NormativeDocument,
    /// Suite catalog/bootstrap manifest.
    Manifest,
    /// Draft 2020-12 JSON Schema.
    JsonSchema,
    /// Line-framed typed JSON records.
    JsonLines,
    /// Semantic registry encoded as YAML.
    Registry,
    /// Non-registry YAML contract.
    YamlContract,
    /// Controlled-language grammar.
    EbnfGrammar,
    /// Protobuf source schema.
    ProtobufSchema,
    /// Compatibility bundle manifest.
    BundleManifest,
}

/// Native parser family selected by a descriptor.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeFormat {
    /// Markdown/prose UTF-8 bytes.
    Markdown,
    /// Strict JSON document.
    Json,
    /// Strict JSON Lines stream.
    Jsonl,
    /// Pinned YAML 1.1 semantic model.
    Yaml,
    /// Protobuf source compiled to a descriptor.
    Proto,
    /// Parsed EBNF metadata and grammar payload.
    Ebnf,
}

/// Versioned semantic digest projection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DigestProjection {
    /// Exact UTF-8 prose bytes.
    ProseUtf8V1,
    /// Strict JSON projected through RFC 8785.
    JsonJcsV1,
    /// YAML semantic JSON projected through RFC 8785.
    #[serde(rename = "yaml-ac-g-53-v1")]
    YamlAcG53V1,
    /// Typed JSONL records with JCS-plus-LF framing.
    JsonlJcsV1,
    /// Normalized `FileDescriptorSet` semantic model.
    ProtoDescriptorV1,
    /// Parsed metadata plus LF-normalized exact grammar payload.
    EbnfSourceV1,
    /// Artifact and bundle projections required by AC-G-07.
    #[serde(rename = "bundle-ac-g-07-v1")]
    BundleAcG07V1,
}

/// Permanent owner of an artifact's contract meaning.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContractOwner {
    /// Suite governance and release manifest.
    Suite,
    /// Fact ontology specification.
    Ontology,
    /// Fact generation specification.
    FactGeneration,
    /// Data-fabric specification.
    DataFabric,
    /// Continuous lifecycle specification.
    Lifecycle,
    /// Semantic-query specification.
    SemanticQuery,
    /// FastMCP serving specification.
    Serving,
    /// Subordinate implementation roadmap.
    Roadmap,
}

/// Compatibility family negotiated independently by consumers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompatibilityFamily {
    /// Cross-suite governance.
    Suite,
    /// Requirement and traceability records.
    Traceability,
    /// Ontology/categorical contracts.
    Ontology,
    /// ID and path identity contracts.
    Identity,
    /// Lifecycle and operational-state contracts.
    Lifecycle,
    /// Storage and public JSON schemas.
    Schema,
    /// Provider/generation contracts.
    Provider,
    /// Query language and plans.
    Query,
    /// RPC schemas and negotiated features.
    Rpc,
    /// Public adapter models.
    Adapter,
    /// Bundle identities.
    Bundle,
    /// Deployment behavior.
    Deployment,
    /// Fault/comparison/security acceptance.
    Conformance,
    /// Build/toolchain identity.
    Toolchain,
    /// Model-pack compatibility.
    ModelPack,
}

/// Compatibility bundles that may include a governed artifact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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

    /// All built-in bundle families in canonical order.
    pub const ALL: [Self; 8] = [
        Self::Derivation,
        Self::ModelPack,
        Self::Ontology,
        Self::Provider,
        Self::QueryLanguage,
        Self::Schema,
        Self::ToolContract,
        Self::Toolchain,
    ];
}

/// Domain that consumes an artifact or generated output.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConsumerDomain {
    /// Stable Rust package.
    RustCore,
    /// FastMCP Python adapter.
    PythonAdapter,
    /// Nightly rustc extractor.
    RustcExtractor,
    /// Pyrefly sidecar.
    PyreflySidecar,
    /// Contract generator/verifier.
    ContractTooling,
    /// Wheel/package-data assembly.
    Packaging,
    /// CI and release governance.
    Governance,
}

/// Provenance datum required from compilation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceRequirement {
    /// Exact checked-in bytes digest.
    SourceDigest,
    /// Semantic projection digest.
    CanonicalDigest,
    /// Generator source-control/package identity.
    GeneratorRevision,
    /// Human owner acceptance evidence.
    OwnerAcceptance,
    /// Native schema/dialect validation evidence.
    NativeValidation,
}

/// Purpose and representation of a derivation output.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DerivationOutputKind {
    /// Canonical shared artifact-index JSON.
    ArtifactIndex,
    /// Canonical JSON derived from a registry.
    CanonicalRegistry,
    /// Statically typed Rust lookups generated from all accepted registries.
    RustRegistryBindings,
    /// Statically typed Python lookups generated from all accepted registries.
    PythonRegistryBindings,
    /// One compiler-owned descriptor set shared by all language generators.
    ProtoDescriptorSet,
    /// Normalized typed view of the shared descriptor set.
    ProtoDescriptorCensus,
    /// Exact compiler and generator identity record.
    ProtoToolchainIdentity,
    /// Rust bindings generated from the shared descriptor set.
    RustProtoBindings,
    /// Python Protobuf message bindings.
    PythonProtoBindings,
    /// Python Protobuf static typing declarations.
    PythonProtoStub,
    /// Python gRPC service bindings.
    PythonGrpcBindings,
    /// Statically typed Pydantic wire-model source compiled from Contract IR.
    PythonAdapterModels,
    /// Validation and serialization JSON Schema views compiled by Pydantic.
    AdapterSchemaManifest,
    /// Named canonical schema and protocol fingerprints.
    AdapterFingerprintManifest,
    /// Public Draft 2020-12 schema compiled from adapter Contract IR.
    AdapterPublicSchema,
    /// Public Draft 2020-12 schema compiled from the schema Contract IR.
    PublicJsonSchema,
    /// Canonical `TableSpec` registry compiled from the schema Contract IR.
    TableSpecManifest,
    /// Rust `TableSpec` declarations compiled from the schema Contract IR.
    RustTableSpecBindings,
    /// `SQLite` operational-store DDL compiled from the schema Contract IR.
    OperationalStoreDdl,
    /// Complete provider-native kind inventory with one normalization disposition per key.
    ProviderRawKindCatalog,
    /// Statically typed Rust hot-path bindings for the provider-native kind inventories.
    RustProviderRawKindBindings,
}

/// Closed model-level derivation operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DerivationKind {
    /// Produce the one peer artifact/derivation index.
    ArtifactIndex,
    /// Produce canonical JSON for a closed set of registries.
    CanonicalRegistrySet,
    /// Compile one Protobuf source set to FDS and Python bindings.
    ProtobufDescriptorAndPython,
    /// Compile Rust bindings from an existing FDS output.
    ProtobufRustFromDescriptor,
    /// Compile Pydantic models, schema views, and fingerprints from Contract IR.
    AdapterModelCompilation,
    /// Compile `TableSpec` records, public schemas, and operational DDL from schema Contract IR.
    SchemaContractCompilation,
    /// Inventory pinned provider-native kinds and expand authored normalization policy.
    ProviderRawCatalogSet,
}

/// View of a governed artifact consumed by a derivation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactInputView {
    /// Exact checked-in authority bytes.
    SourceBytes,
    /// Typed semantic projection selected by the artifact descriptor.
    CompiledSemantic,
}

/// Stable reference to one derivation output.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OutputRef {
    /// Owning derivation unit.
    pub derivation_id: String,
    /// Repository-relative output path within that unit.
    pub path: PathBuf,
}

/// Source of the bytes used by an artifact's semantic projection.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "source_kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SemanticProjectionSource {
    /// Compile directly from the artifact's native authority.
    Native,
    /// Compile from a typed output owned by a derivation unit.
    DerivationOutput {
        /// Exact output reference.
        output: OutputRef,
    },
}

/// One typed derivation input; this is deliberately not a command/build DSL.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "input_kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DerivationInput {
    /// Consume one governed artifact through a declared view.
    Artifact {
        /// Stable artifact ID.
        artifact_id: String,
        /// Native or compiled view.
        view: ArtifactInputView,
    },
    /// Consume one output of another derivation.
    Output {
        /// Exact output reference.
        output: OutputRef,
    },
    /// Consume every compiled artifact in the catalog.
    AllCompiledArtifacts,
}

/// One output edge owned by exactly one derivation unit.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DerivationOutput {
    /// Repository-relative output path.
    pub path: PathBuf,
    /// Closed output representation.
    pub output_kind: DerivationOutputKind,
    /// Artifacts whose semantic identities must appear in generated provenance.
    pub primary_artifact_ids: BTreeSet<String>,
    /// Domains which consume or package the output.
    pub consumers: BTreeSet<ConsumerDomain>,
    /// Optional output-specific cap; the unit profile is the fallback.
    pub resource_budget_profile: Option<String>,
}

/// One closed, deterministic compilation/derivation unit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DerivationUnitDescriptor {
    /// Stable derivation identity.
    pub derivation_id: String,
    /// Model-level operation dispatched by maintained tooling.
    pub derivation_kind: DerivationKind,
    /// Sorted typed input set.
    pub inputs: Vec<DerivationInput>,
    /// Sorted output set.
    pub outputs: Vec<DerivationOutput>,
    /// Named unit-level resource cap.
    pub resource_budget_profile: String,
}

/// Derived, non-authored generator identity for one closed derivation kind.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratorIdentity {
    /// Stable maintained generator implementation.
    pub generator_id: String,
    /// Model schema/revision implemented by that generator.
    pub generator_revision: String,
    /// Exact relevant library/tool identities.
    pub toolchain: Vec<String>,
}

/// Ephemeral, resolved invocation consumed by generators.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedArtifactInput {
    /// Stable governed artifact ID.
    pub artifact_id: String,
    /// Repository-relative authority path.
    pub authority_path: PathBuf,
    /// Selected native or compiled view.
    pub view: ArtifactInputView,
    /// Semantic identity, populated by the artifact compiler for compiled views.
    pub canonical_digest: Option<String>,
    /// Exact source identity, populated at the repository boundary.
    pub source_digest: Option<String>,
}

/// Ephemeral model-resolved invocation passed to maintained generators.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedDerivationInvocation {
    /// Typed unit after deterministic normalization.
    pub derivation: DerivationUnitDescriptor,
    /// Derived generator identity; never authored in the catalog.
    pub generator: GeneratorIdentity,
    /// Artifact paths and identities resolved from typed inputs.
    pub artifact_inputs: Vec<ResolvedArtifactInput>,
}

/// Named resource limits applied by ingress and graph validation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBudgetProfile {
    /// Stable profile identifier selected by descriptors.
    pub profile_id: String,
    /// Maximum checked-in source bytes.
    pub max_bytes: usize,
    /// Maximum parsed nesting depth.
    pub max_depth: usize,
    /// Maximum aggregate semantic nodes.
    pub max_nodes: usize,
    /// Maximum members in one collection.
    pub max_collection_items: usize,
    /// Maximum decoded string or token bytes.
    pub max_string_bytes: usize,
    /// Maximum line records or graph edges.
    pub max_records_or_edges: usize,
    /// Maximum YAML aliases accepted before expansion.
    pub max_aliases: usize,
    /// Maximum accumulated diagnostics.
    pub max_diagnostics: usize,
}

/// One governed source and all of its derivation obligations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDescriptor {
    /// Stable artifact identity.
    pub artifact_id: String,
    /// Repository-relative native authority path.
    pub authority_path: PathBuf,
    /// Closed artifact kind.
    pub artifact_kind: ArtifactKind,
    /// Native source parser family.
    pub native_format: NativeFormat,
    /// Permanent contract owner.
    pub owner: ContractOwner,
    /// Two-component public version.
    pub version: String,
    /// Compatible suite major.
    pub compatible_suite_major: u16,
    /// Current release status.
    pub status: ArtifactStatus,
    /// Semantic digest projection.
    pub digest_projection: DigestProjection,
    /// Independently negotiated compatibility family.
    pub compatibility_family: CompatibilityFamily,
    /// Compatibility bundles whose identity includes this artifact.
    #[serde(default)]
    pub bundle_membership: BTreeSet<BundleKind>,
    /// Named resource budget.
    pub resource_budget_profile: String,
    /// Native schema/parser authority, when distinct from the source itself.
    pub parser_schema_authority: Option<String>,
    /// Native or derivation-owned source of this artifact's semantic projection.
    pub semantic_projection_source: SemanticProjectionSource,
    /// Domains consuming the source directly.
    pub consumers: BTreeSet<ConsumerDomain>,
    /// Provenance required in the compiled index.
    pub provenance_requirements: BTreeSet<ProvenanceRequirement>,
}

/// Sole declarative bootstrap for contract compilation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractCatalog {
    /// AC-G-02 identity of this catalog source.
    pub artifact_id: String,
    /// Catalog source kind.
    pub artifact_kind: ArtifactKind,
    /// Public catalog version.
    pub version: String,
    /// Compatible suite major.
    pub compatible_suite_major: u16,
    /// Catalog release state.
    pub status: ArtifactStatus,
    /// Current embedded semantic digest, verified by WP03/WP04.
    pub canonical_digest: String,
    /// Semantic self-projection selected by the catalog header.
    pub digest_projection: DigestProjection,
    /// Typed catalog schema version.
    pub catalog_schema_version: u16,
    /// Named ingress/graph budget definitions.
    pub resource_budget_profiles: Vec<ResourceBudgetProfile>,
    /// Governed sources; ordering is non-semantic.
    pub artifacts: Vec<ArtifactDescriptor>,
    /// Closed derivation units; ordering is non-semantic.
    pub derivations: Vec<DerivationUnitDescriptor>,
}

/// Validated deterministic views used by generators and consumers.
type ArtifactsById = BTreeMap<String, ArtifactDescriptor>;
type ArtifactIdByPath = BTreeMap<PathBuf, String>;
type DerivationsById = BTreeMap<String, DerivationUnitDescriptor>;
type OutputsByPath = BTreeMap<PathBuf, (String, DerivationOutput)>;
type BudgetsById = BTreeMap<String, ResourceBudgetProfile>;

#[derive(Clone, Debug)]
pub struct CompiledCatalog {
    normalized_catalog: ContractCatalog,
    artifacts: ArtifactsById,
    artifact_by_path: ArtifactIdByPath,
    derivations: DerivationsById,
    outputs: OutputsByPath,
    budgets: BudgetsById,
    derivation_order: Vec<String>,
}

impl CompiledCatalog {
    /// Deterministically ordered typed source model used by semantic projection.
    #[must_use]
    pub(crate) fn normalized_catalog(&self) -> &ContractCatalog {
        &self.normalized_catalog
    }

    /// Number of governed source descriptors.
    #[must_use]
    pub fn artifact_count(&self) -> usize {
        self.artifacts.len()
    }

    /// Artifact descriptors sorted by stable ID.
    pub fn artifacts(&self) -> impl Iterator<Item = &ArtifactDescriptor> {
        self.artifacts.values()
    }

    /// Artifact descriptor with the requested stable ID.
    #[must_use]
    pub fn artifact(&self, artifact_id: &str) -> Option<&ArtifactDescriptor> {
        self.artifacts.get(artifact_id)
    }

    /// Artifact descriptor which owns a repository-relative authority path.
    #[must_use]
    pub fn artifact_for_path(&self, path: &Path) -> Option<&ArtifactDescriptor> {
        self.artifact_by_path
            .get(path)
            .and_then(|artifact_id| self.artifacts.get(artifact_id))
    }

    /// Artifact descriptors consumed by one domain, sorted by stable ID.
    pub fn artifacts_for_consumer(
        &self,
        domain: ConsumerDomain,
    ) -> impl Iterator<Item = &ArtifactDescriptor> {
        self.artifacts()
            .filter(move |artifact| artifact.consumers.contains(&domain))
    }

    /// Draft artifact count used by verification policy.
    #[must_use]
    pub fn draft_count(&self) -> usize {
        self.artifacts()
            .filter(|artifact| artifact.status == ArtifactStatus::Draft)
            .count()
    }

    /// Derivation units sorted by stable ID.
    pub fn derivations(&self) -> impl Iterator<Item = &DerivationUnitDescriptor> {
        self.derivations.values()
    }

    /// Derivation unit with the requested stable ID.
    #[must_use]
    pub fn derivation(&self, derivation_id: &str) -> Option<&DerivationUnitDescriptor> {
        self.derivations.get(derivation_id)
    }

    /// Generated outputs sorted by repository-relative path.
    pub fn outputs(&self) -> impl Iterator<Item = (&Path, &str, &DerivationOutput)> {
        self.outputs
            .iter()
            .map(|(path, (owner, output))| (path.as_path(), owner.as_str(), output))
    }

    /// Outputs owned by one derivation, sorted by path.
    pub fn outputs_for_derivation(
        &self,
        derivation_id: &str,
    ) -> impl Iterator<Item = (&Path, &DerivationOutput)> {
        self.outputs()
            .filter(move |(_, owner, _)| *owner == derivation_id)
            .map(|(path, _, output)| (path, output))
    }

    /// Outputs of one kind scoped to one derivation; plural by design.
    pub fn outputs_of_kind(
        &self,
        derivation_id: &str,
        kind: DerivationOutputKind,
    ) -> impl Iterator<Item = (&Path, &DerivationOutput)> {
        self.outputs_for_derivation(derivation_id)
            .filter(move |(_, output)| output.output_kind == kind)
    }

    /// Resolve one output by its globally unique path.
    #[must_use]
    pub fn output_by_path(&self, path: &Path) -> Option<(&str, &DerivationOutput)> {
        self.outputs
            .get(path)
            .map(|(owner, output)| (owner.as_str(), output))
    }

    /// Resolve an exact typed output reference.
    #[must_use]
    pub fn output(&self, output_ref: &OutputRef) -> Option<&DerivationOutput> {
        self.outputs
            .get(&output_ref.path)
            .and_then(|(owner, output)| (owner == &output_ref.derivation_id).then_some(output))
    }

    /// Named resource budget selected by an artifact.
    #[must_use]
    pub fn budget(&self, profile_id: &str) -> Option<&ResourceBudgetProfile> {
        self.budgets.get(profile_id)
    }

    /// Dependency-safe derivation IDs with deterministic tie breaking.
    #[must_use]
    pub fn derivation_order(&self) -> &[String] {
        &self.derivation_order
    }

    /// Resolve the model-level invocation for one derivation.
    #[must_use]
    pub fn resolved_invocation(&self, derivation_id: &str) -> Option<ResolvedDerivationInvocation> {
        let derivation = self.derivation(derivation_id)?;
        let artifact_inputs = derivation
            .inputs
            .iter()
            .flat_map(|input| match input {
                DerivationInput::Artifact { artifact_id, view } => self
                    .artifact(artifact_id)
                    .map(|artifact| ResolvedArtifactInput {
                        artifact_id: artifact_id.clone(),
                        authority_path: artifact.authority_path.clone(),
                        view: *view,
                        canonical_digest: None,
                        source_digest: None,
                    })
                    .into_iter()
                    .collect::<Vec<_>>(),
                DerivationInput::AllCompiledArtifacts => self
                    .artifacts()
                    .map(|artifact| ResolvedArtifactInput {
                        artifact_id: artifact.artifact_id.clone(),
                        authority_path: artifact.authority_path.clone(),
                        view: ArtifactInputView::CompiledSemantic,
                        canonical_digest: None,
                        source_digest: None,
                    })
                    .collect(),
                DerivationInput::Output { .. } => Vec::new(),
            })
            .collect();
        Some(ResolvedDerivationInvocation {
            derivation: derivation.clone(),
            generator: generator_identity(derivation.derivation_kind),
            artifact_inputs,
        })
    }

    /// Package-data paths derived from output consumer edges.
    #[must_use]
    pub fn package_data(&self, domain: ConsumerDomain) -> Vec<&Path> {
        self.outputs()
            .filter(|(_, _, output)| output.consumers.contains(&domain))
            .map(|(path, _, _)| path)
            .collect()
    }
}

/// Derive maintained generator identity from the closed unit kind.
#[must_use]
pub fn generator_identity(kind: DerivationKind) -> GeneratorIdentity {
    match kind {
        DerivationKind::ArtifactIndex | DerivationKind::CanonicalRegistrySet => GeneratorIdentity {
            generator_id: "codefabric-contracts".to_owned(),
            generator_revision: "derivation-model-v2".to_owned(),
            toolchain: vec![
                "serde-json-canonicalizer=0.3.2".to_owned(),
                "blake3=1".to_owned(),
            ],
        },
        DerivationKind::ProtobufDescriptorAndPython => GeneratorIdentity {
            generator_id: "codefabric-proto-python".to_owned(),
            generator_revision: "production-descriptor-first-v1".to_owned(),
            toolchain: vec![
                "grpcio-tools=1.83.0".to_owned(),
                "protobuf=7.36.0".to_owned(),
                "libprotoc=35.1".to_owned(),
            ],
        },
        DerivationKind::ProtobufRustFromDescriptor => GeneratorIdentity {
            generator_id: "codefabric-proto-rust".to_owned(),
            generator_revision: "production-compile-fds-v1".to_owned(),
            toolchain: vec![
                "prost=0.14.4".to_owned(),
                "tonic-prost-build=0.14.6".to_owned(),
            ],
        },
        DerivationKind::AdapterModelCompilation => GeneratorIdentity {
            generator_id: "codefabric-adapter-models".to_owned(),
            generator_revision: "codefabric-adapter-model-compiler-v1".to_owned(),
            toolchain: vec!["pydantic=2.13.4".to_owned(), "fastmcp=3.4.7".to_owned()],
        },
        DerivationKind::SchemaContractCompilation => GeneratorIdentity {
            generator_id: "codefabric-schema-contracts".to_owned(),
            generator_revision: "codefabric-schema-contracts-v1".to_owned(),
            toolchain: vec![
                "serde=1".to_owned(),
                "serde-json-canonicalizer=0.3.2".to_owned(),
                "arrow-schema=58.4.0".to_owned(),
            ],
        },
        DerivationKind::ProviderRawCatalogSet => GeneratorIdentity {
            generator_id: "codefabric-provider-raw-catalogs".to_owned(),
            generator_revision: "provider-raw-catalogs-v1".to_owned(),
            toolchain: vec![
                "tree-sitter=0.26.12".to_owned(),
                "tree-sitter-python=0.25.0".to_owned(),
                "tree-sitter-rust=0.24.2".to_owned(),
                "grammar-abi=15".to_owned(),
            ],
        },
    }
}

/// Catalog parsing or graph-compilation failure.
#[derive(Debug, Error)]
pub enum CatalogError {
    /// Catalog source could not be read.
    #[error("cannot read contract catalog {path}: {source}")]
    Io {
        /// Catalog or authority path.
        path: PathBuf,
        /// Underlying I/O failure.
        source: std::io::Error,
    },
    /// The bootstrap catalog exceeded its pre-parse byte sentinel.
    #[error(
        "contract catalog resource limit at {path}: observed {observed} bytes, maximum {maximum}"
    )]
    ResourceLimit {
        /// Catalog path.
        path: PathBuf,
        /// Bytes observed, capped at maximum plus one.
        observed: usize,
        /// Bootstrap maximum.
        maximum: usize,
    },
    /// Closed JSON catalog decoding failed.
    #[error("invalid contract catalog {path}: {message}")]
    Decode {
        /// Catalog path.
        path: PathBuf,
        /// Parser diagnostic.
        message: String,
    },
    /// Typed graph invariant failed.
    #[error("invalid contract catalog graph at {path}: {message}")]
    Graph {
        /// Catalog or offending authority/output path.
        path: PathBuf,
        /// Stable bounded diagnostic.
        message: String,
    },
}

impl ContractCatalog {
    /// Load and compile the sole production catalog bootstrap.
    ///
    /// # Errors
    ///
    /// Returns an error for I/O, closed-model decoding, or any graph invariant.
    pub fn load(repository_root: &Path) -> Result<CompiledCatalog, CatalogError> {
        Self::load_with_source_requirement(repository_root, true)
    }

    /// Load and compile the catalog while requiring only the inputs selected by a caller.
    ///
    /// This is the generator/reproduction boundary: graph integrity is still complete, while
    /// isolated roots may copy only one derivation's declared sources.
    ///
    /// # Errors
    ///
    /// Returns an error for catalog I/O, closed-model decoding, or graph invariants.
    pub fn load_for_derivation(repository_root: &Path) -> Result<CompiledCatalog, CatalogError> {
        Self::load_with_source_requirement(repository_root, false)
    }

    fn load_with_source_requirement(
        repository_root: &Path,
        require_sources: bool,
    ) -> Result<CompiledCatalog, CatalogError> {
        let path = repository_root.join(CATALOG_PATH);
        let file = File::open(&path).map_err(|source| CatalogError::Io {
            path: path.clone(),
            source,
        })?;
        let read_limit = u64::try_from(CATALOG_BOOTSTRAP_MAX_BYTES + 1).unwrap_or(u64::MAX);
        let mut reader: Take<File> = file.take(read_limit);
        let mut bytes = Vec::with_capacity(CATALOG_BOOTSTRAP_MAX_BYTES);
        reader
            .read_to_end(&mut bytes)
            .map_err(|source| CatalogError::Io {
                path: path.clone(),
                source,
            })?;
        if bytes.len() > CATALOG_BOOTSTRAP_MAX_BYTES {
            return Err(CatalogError::ResourceLimit {
                path,
                observed: bytes.len(),
                maximum: CATALOG_BOOTSTRAP_MAX_BYTES,
            });
        }
        let catalog: Self =
            serde_json::from_slice(&bytes).map_err(|error| CatalogError::Decode {
                path: path.clone(),
                message: error.to_string(),
            })?;
        let compiled = catalog.compile(repository_root, require_sources)?;
        let self_descriptor = compiled
            .artifact(&catalog.artifact_id)
            .ok_or_else(|| graph_error(CATALOG_PATH, "catalog self-descriptor is absent"))?;
        let self_budget = compiled
            .budget(&self_descriptor.resource_budget_profile)
            .ok_or_else(|| {
                graph_error(
                    CATALOG_PATH,
                    "catalog self-descriptor resource budget is absent",
                )
            })?;
        if self_budget.max_bytes > CATALOG_BOOTSTRAP_MAX_BYTES {
            return Err(graph_error(
                CATALOG_PATH,
                format!(
                    "catalog profile max_bytes {} exceeds bootstrap cap {}",
                    self_budget.max_bytes, CATALOG_BOOTSTRAP_MAX_BYTES
                ),
            ));
        }
        Ok(compiled)
    }

    /// Compile an in-memory catalog for tests and tooling probes.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate, missing, unsafe, conflicting, or cyclic graph data.
    pub fn compile(
        &self,
        repository_root: &Path,
        require_sources: bool,
    ) -> Result<CompiledCatalog, CatalogError> {
        let normalized_catalog = self.normalized();
        let budgets = compile_budgets(&normalized_catalog.resource_budget_profiles)?;
        let (artifacts, artifact_by_path) = compile_artifacts(
            &normalized_catalog,
            repository_root,
            require_sources,
            &budgets,
        )?;
        let (derivations, outputs) =
            compile_derivations(&normalized_catalog.derivations, &artifacts, &budgets)?;
        validate_semantic_projection_sources(&artifacts, &outputs)?;
        let derivation_order = derivation_order(&derivations, &artifacts, &outputs)?;
        validate_self_descriptor(self, &artifacts)?;
        Ok(CompiledCatalog {
            normalized_catalog,
            artifacts,
            artifact_by_path,
            derivations,
            outputs,
            budgets,
            derivation_order,
        })
    }

    fn normalized(&self) -> Self {
        let mut normalized = self.clone();
        normalized
            .resource_budget_profiles
            .sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
        for derivation in &mut normalized.derivations {
            derivation.inputs.sort();
            derivation.outputs.sort_by(|left, right| {
                left.path
                    .cmp(&right.path)
                    .then(left.output_kind.cmp(&right.output_kind))
            });
        }
        normalized
            .artifacts
            .sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
        normalized
            .derivations
            .sort_by(|left, right| left.derivation_id.cmp(&right.derivation_id));
        normalized
    }
}

fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn graph_error(path: impl Into<PathBuf>, message: impl Into<String>) -> CatalogError {
    CatalogError::Graph {
        path: path.into(),
        message: message.into(),
    }
}

fn compile_budgets(profiles: &[ResourceBudgetProfile]) -> Result<BudgetsById, CatalogError> {
    let mut budgets = BTreeMap::new();
    for profile in profiles {
        if profile.profile_id.is_empty()
            || profile.max_bytes == 0
            || profile.max_depth == 0
            || profile.max_nodes == 0
            || profile.max_collection_items == 0
            || profile.max_string_bytes == 0
            || profile.max_records_or_edges == 0
            || profile.max_diagnostics == 0
        {
            return Err(graph_error(
                CATALOG_PATH,
                format!(
                    "resource budget {} has a zero required limit",
                    profile.profile_id
                ),
            ));
        }
        if budgets
            .insert(profile.profile_id.clone(), profile.clone())
            .is_some()
        {
            return Err(graph_error(
                CATALOG_PATH,
                format!("duplicate resource budget: {}", profile.profile_id),
            ));
        }
    }
    Ok(budgets)
}

fn compile_artifacts(
    catalog: &ContractCatalog,
    repository_root: &Path,
    require_sources: bool,
    budgets: &BudgetsById,
) -> Result<(ArtifactsById, ArtifactIdByPath), CatalogError> {
    let mut artifacts = BTreeMap::new();
    let mut artifact_by_path = BTreeMap::new();
    for artifact in &catalog.artifacts {
        validate_descriptor(artifact, budgets)?;
        if require_sources && !repository_root.join(&artifact.authority_path).is_file() {
            return Err(graph_error(
                &artifact.authority_path,
                "authority source is absent",
            ));
        }
        if artifacts
            .insert(artifact.artifact_id.clone(), artifact.clone())
            .is_some()
        {
            return Err(graph_error(
                &artifact.authority_path,
                format!("duplicate artifact ID: {}", artifact.artifact_id),
            ));
        }
        if let Some(previous) = artifact_by_path.insert(
            artifact.authority_path.clone(),
            artifact.artifact_id.clone(),
        ) {
            return Err(graph_error(
                &artifact.authority_path,
                format!("authority path also belongs to {previous}"),
            ));
        }
    }
    Ok((artifacts, artifact_by_path))
}

fn validate_descriptor(
    artifact: &ArtifactDescriptor,
    budgets: &BudgetsById,
) -> Result<(), CatalogError> {
    if artifact.artifact_id.is_empty() {
        return Err(graph_error(
            &artifact.authority_path,
            "artifact ID is empty",
        ));
    }
    if !safe_relative(&artifact.authority_path) {
        return Err(graph_error(
            &artifact.authority_path,
            "authority path is not a safe repository-relative path",
        ));
    }
    if !budgets.contains_key(&artifact.resource_budget_profile) {
        return Err(graph_error(
            &artifact.authority_path,
            format!(
                "unknown resource budget: {}",
                artifact.resource_budget_profile
            ),
        ));
    }
    if artifact.compatible_suite_major != 1 {
        return Err(graph_error(
            &artifact.authority_path,
            "compatible suite major must be 1",
        ));
    }
    let representation_matches = matches!(
        (
            artifact.artifact_kind,
            artifact.native_format,
            artifact.digest_projection
        ),
        (
            ArtifactKind::NormativeDocument,
            NativeFormat::Markdown,
            DigestProjection::ProseUtf8V1
        ) | (
            ArtifactKind::Manifest | ArtifactKind::JsonSchema,
            NativeFormat::Json,
            DigestProjection::JsonJcsV1
        ) | (
            ArtifactKind::JsonLines,
            NativeFormat::Jsonl,
            DigestProjection::JsonlJcsV1
        ) | (
            ArtifactKind::Registry | ArtifactKind::YamlContract,
            NativeFormat::Yaml,
            DigestProjection::YamlAcG53V1
        ) | (
            ArtifactKind::EbnfGrammar,
            NativeFormat::Ebnf,
            DigestProjection::EbnfSourceV1
        ) | (
            ArtifactKind::ProtobufSchema,
            NativeFormat::Proto,
            DigestProjection::ProtoDescriptorV1
        ) | (
            ArtifactKind::BundleManifest,
            NativeFormat::Json,
            DigestProjection::BundleAcG07V1
        )
    );
    if !representation_matches {
        return Err(graph_error(
            &artifact.authority_path,
            "artifact kind, native format, and digest projection disagree",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // One linear pass keeps cross-unit ownership checks atomic.
fn compile_derivations(
    descriptors: &[DerivationUnitDescriptor],
    artifacts: &ArtifactsById,
    budgets: &BudgetsById,
) -> Result<(DerivationsById, OutputsByPath), CatalogError> {
    let mut derivations = BTreeMap::new();
    let mut outputs = BTreeMap::new();
    let authority_paths = artifacts
        .values()
        .map(|artifact| (&artifact.authority_path, artifact))
        .collect::<BTreeMap<_, _>>();
    for derivation in descriptors {
        if derivation.derivation_id.is_empty() {
            return Err(graph_error(CATALOG_PATH, "derivation ID is empty"));
        }
        if !budgets.contains_key(&derivation.resource_budget_profile) {
            return Err(graph_error(
                CATALOG_PATH,
                format!(
                    "derivation {} selects unknown resource budget {}",
                    derivation.derivation_id, derivation.resource_budget_profile
                ),
            ));
        }
        if derivation.inputs.is_empty() || derivation.outputs.is_empty() {
            return Err(graph_error(
                CATALOG_PATH,
                format!(
                    "derivation {} has no inputs or outputs",
                    derivation.derivation_id
                ),
            ));
        }
        let mut seen_inputs = BTreeSet::new();
        for input in &derivation.inputs {
            if !seen_inputs.insert(input.clone()) {
                return Err(graph_error(
                    CATALOG_PATH,
                    format!(
                        "derivation {} has duplicate inputs",
                        derivation.derivation_id
                    ),
                ));
            }
            if let DerivationInput::Artifact { artifact_id, view } = input {
                let artifact = artifacts.get(artifact_id).ok_or_else(|| {
                    graph_error(
                        CATALOG_PATH,
                        format!(
                            "derivation {} references unknown artifact {artifact_id}",
                            derivation.derivation_id
                        ),
                    )
                })?;
                if *view == ArtifactInputView::SourceBytes
                    && matches!(
                        artifact.semantic_projection_source,
                        SemanticProjectionSource::DerivationOutput { .. }
                    )
                    && derivation.derivation_kind != DerivationKind::ProtobufDescriptorAndPython
                {
                    return Err(graph_error(
                        &artifact.authority_path,
                        "source-bytes view bypasses a derivation-owned semantic projection",
                    ));
                }
            }
        }
        for output in &derivation.outputs {
            if !safe_relative(&output.path) {
                return Err(graph_error(
                    &output.path,
                    "output path is not a safe repository-relative path",
                ));
            }
            if let Some(artifact) = authority_paths.get(&output.path) {
                let self_owned_generated_authority =
                    matches!(
                        &artifact.semantic_projection_source,
                        SemanticProjectionSource::DerivationOutput { output: source }
                            if source.derivation_id == derivation.derivation_id
                                && source.path == output.path
                    ) && output.primary_artifact_ids.contains(&artifact.artifact_id);
                if !self_owned_generated_authority {
                    return Err(graph_error(
                        &output.path,
                        "generated output conflicts with an unrelated authority path",
                    ));
                }
            }
            if output.primary_artifact_ids.is_empty() {
                return Err(graph_error(&output.path, "output has no primary artifacts"));
            }
            for artifact_id in &output.primary_artifact_ids {
                if !artifacts.contains_key(artifact_id) {
                    return Err(graph_error(
                        &output.path,
                        format!("output references unknown primary artifact {artifact_id}"),
                    ));
                }
            }
            if output
                .resource_budget_profile
                .as_ref()
                .is_some_and(|profile| !budgets.contains_key(profile))
            {
                return Err(graph_error(
                    &output.path,
                    "output selects an unknown resource budget",
                ));
            }
            if let Some((previous, _)) = outputs.insert(
                output.path.clone(),
                (derivation.derivation_id.clone(), output.clone()),
            ) {
                return Err(graph_error(
                    &output.path,
                    format!("output also belongs to {previous}"),
                ));
            }
        }
        validate_derivation_shape(derivation, artifacts)?;
        if derivations
            .insert(derivation.derivation_id.clone(), derivation.clone())
            .is_some()
        {
            return Err(graph_error(
                CATALOG_PATH,
                format!("duplicate derivation ID: {}", derivation.derivation_id),
            ));
        }
    }
    for derivation in derivations.values() {
        for input in &derivation.inputs {
            if let DerivationInput::Output { output } = input {
                validate_output_ref(output, &outputs)?;
            }
        }
    }
    Ok((derivations, outputs))
}

#[allow(clippy::too_many_lines)] // The closed-kind matrix is clearest as one exhaustive match.
fn validate_derivation_shape(
    derivation: &DerivationUnitDescriptor,
    artifacts: &ArtifactsById,
) -> Result<(), CatalogError> {
    let count = |kind| {
        derivation
            .outputs
            .iter()
            .filter(|output| output.output_kind == kind)
            .count()
    };
    let invalid = match derivation.derivation_kind {
        DerivationKind::ArtifactIndex => {
            derivation.inputs.as_slice() != [DerivationInput::AllCompiledArtifacts]
                || derivation.outputs.len() != 1
                || count(DerivationOutputKind::ArtifactIndex) != 1
        }
        DerivationKind::CanonicalRegistrySet => {
            derivation.inputs.iter().any(|input| {
                !matches!(
                    input,
                    DerivationInput::Artifact {
                        artifact_id,
                        view: ArtifactInputView::CompiledSemantic,
                    } if artifacts.get(artifact_id).is_some_and(|artifact| artifact.artifact_kind == ArtifactKind::Registry)
                )
            }) || count(DerivationOutputKind::CanonicalRegistry) != derivation.inputs.len()
                || count(DerivationOutputKind::RustRegistryBindings) != 1
                || count(DerivationOutputKind::PythonRegistryBindings) != 1
                || derivation.outputs.iter().any(|output| {
                    !matches!(
                        output.output_kind,
                        DerivationOutputKind::CanonicalRegistry
                            | DerivationOutputKind::RustRegistryBindings
                            | DerivationOutputKind::PythonRegistryBindings
                    )
                })
        }
        DerivationKind::ProtobufDescriptorAndPython => {
            derivation.inputs.iter().any(|input| {
                !matches!(
                    input,
                    DerivationInput::Artifact {
                        artifact_id,
                        view: ArtifactInputView::SourceBytes,
                    } if artifacts.get(artifact_id).is_some_and(|artifact| artifact.native_format == NativeFormat::Proto)
                )
            }) || count(DerivationOutputKind::ProtoDescriptorSet) != 1
                || count(DerivationOutputKind::ProtoDescriptorCensus) != 1
                || count(DerivationOutputKind::ProtoToolchainIdentity) != 1
                || count(DerivationOutputKind::PythonProtoBindings) == 0
                || count(DerivationOutputKind::PythonProtoStub) == 0
                || count(DerivationOutputKind::PythonGrpcBindings) == 0
                || derivation.outputs.iter().any(|output| {
                    !matches!(
                        output.output_kind,
                        DerivationOutputKind::ProtoDescriptorSet
                            | DerivationOutputKind::ProtoDescriptorCensus
                            | DerivationOutputKind::ProtoToolchainIdentity
                            | DerivationOutputKind::PythonProtoBindings
                            | DerivationOutputKind::PythonProtoStub
                            | DerivationOutputKind::PythonGrpcBindings
                    )
                })
        }
        DerivationKind::ProtobufRustFromDescriptor => {
            derivation.inputs.len() != 1
                || !matches!(derivation.inputs[0], DerivationInput::Output { .. })
                || derivation.outputs.is_empty()
                || derivation.outputs.iter().any(|output| {
                    output.output_kind != DerivationOutputKind::RustProtoBindings
                })
        }
        DerivationKind::AdapterModelCompilation => {
            derivation.inputs.len() != 1
                || !matches!(
                    derivation.inputs[0],
                    DerivationInput::Artifact {
                        view: ArtifactInputView::CompiledSemantic,
                        ..
                    }
                )
                || derivation.outputs.len() != 6
                || count(DerivationOutputKind::PythonAdapterModels) != 1
                || count(DerivationOutputKind::AdapterSchemaManifest) != 1
                || count(DerivationOutputKind::AdapterFingerprintManifest) != 1
                || count(DerivationOutputKind::AdapterPublicSchema) != 3
        }
        DerivationKind::SchemaContractCompilation => {
            derivation.inputs.len() != 1
                || !matches!(
                    derivation.inputs[0],
                    DerivationInput::Artifact {
                        view: ArtifactInputView::CompiledSemantic,
                        ..
                    }
                )
                || count(DerivationOutputKind::TableSpecManifest) != 1
                || count(DerivationOutputKind::RustTableSpecBindings) != 1
                || count(DerivationOutputKind::OperationalStoreDdl) != 1
                || count(DerivationOutputKind::PublicJsonSchema) != 8
                || derivation.outputs.len() != 11
        }
        DerivationKind::ProviderRawCatalogSet => {
            derivation.inputs.len() != 3
                || derivation.inputs.iter().any(|input| {
                    !matches!(
                        input,
                        DerivationInput::Artifact {
                            view: ArtifactInputView::CompiledSemantic,
                            ..
                        }
                    )
                })
                || count(DerivationOutputKind::ProviderRawKindCatalog) != 3
                || count(DerivationOutputKind::RustProviderRawKindBindings) != 1
                || derivation.outputs.len() != 4
        }
    };
    if invalid {
        return Err(graph_error(
            CATALOG_PATH,
            format!(
                "derivation {} violates {:?} input/output cardinality",
                derivation.derivation_id, derivation.derivation_kind
            ),
        ));
    }
    Ok(())
}

fn validate_output_ref(
    output_ref: &OutputRef,
    outputs: &OutputsByPath,
) -> Result<(), CatalogError> {
    let Some((owner, _)) = outputs.get(&output_ref.path) else {
        return Err(graph_error(
            &output_ref.path,
            "derivation output reference is missing",
        ));
    };
    if owner != &output_ref.derivation_id {
        return Err(graph_error(
            &output_ref.path,
            format!(
                "output belongs to {owner}, not {}",
                output_ref.derivation_id
            ),
        ));
    }
    Ok(())
}

fn validate_semantic_projection_sources(
    artifacts: &ArtifactsById,
    outputs: &OutputsByPath,
) -> Result<(), CatalogError> {
    for artifact in artifacts.values() {
        if let SemanticProjectionSource::DerivationOutput { output } =
            &artifact.semantic_projection_source
        {
            validate_output_ref(output, outputs)?;
            let (_, descriptor) = outputs
                .get(&output.path)
                .expect("validated output reference exists");
            if !descriptor
                .primary_artifact_ids
                .contains(&artifact.artifact_id)
            {
                return Err(graph_error(
                    &output.path,
                    format!(
                        "semantic projection output does not name primary artifact {}",
                        artifact.artifact_id
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn derivation_order(
    derivations: &DerivationsById,
    artifacts: &ArtifactsById,
    outputs: &OutputsByPath,
) -> Result<Vec<String>, CatalogError> {
    let mut indegree = BTreeMap::new();
    let mut dependants: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (derivation_id, derivation) in derivations {
        let mut dependencies = BTreeSet::new();
        for input in &derivation.inputs {
            match input {
                DerivationInput::Output { output } => {
                    dependencies.insert(output.derivation_id.as_str());
                }
                DerivationInput::Artifact {
                    artifact_id,
                    view: ArtifactInputView::CompiledSemantic,
                } => {
                    if let Some(artifact) = artifacts.get(artifact_id)
                        && let SemanticProjectionSource::DerivationOutput { output } =
                            &artifact.semantic_projection_source
                    {
                        dependencies.insert(output.derivation_id.as_str());
                    }
                }
                DerivationInput::AllCompiledArtifacts => {
                    for artifact in artifacts.values() {
                        if let SemanticProjectionSource::DerivationOutput { output } =
                            &artifact.semantic_projection_source
                            && output.derivation_id != *derivation_id
                        {
                            dependencies.insert(output.derivation_id.as_str());
                        }
                    }
                }
                DerivationInput::Artifact { .. } => {}
            }
        }
        indegree.insert(derivation_id.as_str(), dependencies.len());
        for dependency in dependencies {
            dependants
                .entry(dependency)
                .or_default()
                .insert(derivation_id);
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(artifact_id, _)| *artifact_id)
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(derivations.len());
    while let Some(derivation_id) = ready.pop_first() {
        ordered.push(derivation_id.to_owned());
        for dependant in dependants.get(derivation_id).into_iter().flatten() {
            let count = indegree
                .get_mut(dependant)
                .expect("dependant was initialized");
            *count -= 1;
            if *count == 0 {
                ready.insert(dependant);
            }
        }
    }
    if ordered.len() != derivations.len() {
        return Err(graph_error(CATALOG_PATH, "derivation dependency cycle"));
    }
    let _ = outputs;
    Ok(ordered)
}

fn validate_self_descriptor(
    catalog: &ContractCatalog,
    artifacts: &ArtifactsById,
) -> Result<(), CatalogError> {
    let Some(descriptor) = artifacts.get(&catalog.artifact_id) else {
        return Err(graph_error(
            CATALOG_PATH,
            "catalog self-descriptor is absent",
        ));
    };
    if descriptor.authority_path != Path::new(CATALOG_PATH)
        || descriptor.artifact_kind != catalog.artifact_kind
        || descriptor.version != catalog.version
        || descriptor.compatible_suite_major != catalog.compatible_suite_major
        || descriptor.status != catalog.status
        || descriptor.digest_projection != catalog.digest_projection
    {
        return Err(graph_error(
            CATALOG_PATH,
            "catalog self-descriptor disagrees with its root header",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> ResourceBudgetProfile {
        ResourceBudgetProfile {
            profile_id: "test".to_owned(),
            max_bytes: 1,
            max_depth: 1,
            max_nodes: 1,
            max_collection_items: 1,
            max_string_bytes: 1,
            max_records_or_edges: 1,
            max_aliases: 0,
            max_diagnostics: 1,
        }
    }

    fn descriptor(id: &str, path: &str) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_id: id.to_owned(),
            authority_path: PathBuf::from(path),
            artifact_kind: ArtifactKind::Manifest,
            native_format: NativeFormat::Json,
            owner: ContractOwner::Suite,
            version: "1.0".to_owned(),
            compatible_suite_major: 1,
            status: ArtifactStatus::Draft,
            digest_projection: DigestProjection::JsonJcsV1,
            compatibility_family: CompatibilityFamily::Suite,
            bundle_membership: BTreeSet::new(),
            resource_budget_profile: "test".to_owned(),
            parser_schema_authority: None,
            semantic_projection_source: SemanticProjectionSource::Native,
            consumers: BTreeSet::from([ConsumerDomain::ContractTooling]),
            provenance_requirements: BTreeSet::from([
                ProvenanceRequirement::SourceDigest,
                ProvenanceRequirement::CanonicalDigest,
            ]),
        }
    }

    fn catalog_with_derivations(
        mut artifacts: Vec<ArtifactDescriptor>,
        derivations: Vec<DerivationUnitDescriptor>,
    ) -> ContractCatalog {
        artifacts.push(descriptor("catalog", CATALOG_PATH));
        ContractCatalog {
            artifact_id: "catalog".to_owned(),
            artifact_kind: ArtifactKind::Manifest,
            version: "1.0".to_owned(),
            compatible_suite_major: 1,
            status: ArtifactStatus::Draft,
            canonical_digest: "b3:placeholder".to_owned(),
            digest_projection: DigestProjection::JsonJcsV1,
            catalog_schema_version: 2,
            resource_budget_profiles: vec![budget()],
            artifacts,
            derivations,
        }
    }

    fn catalog(artifacts: Vec<ArtifactDescriptor>) -> ContractCatalog {
        catalog_with_derivations(artifacts, Vec::new())
    }

    fn registry_derivation(id: &str, artifact_id: &str, output: &str) -> DerivationUnitDescriptor {
        DerivationUnitDescriptor {
            derivation_id: id.to_owned(),
            derivation_kind: DerivationKind::CanonicalRegistrySet,
            inputs: vec![DerivationInput::Artifact {
                artifact_id: artifact_id.to_owned(),
                view: ArtifactInputView::CompiledSemantic,
            }],
            outputs: vec![
                DerivationOutput {
                    path: PathBuf::from(output),
                    output_kind: DerivationOutputKind::CanonicalRegistry,
                    primary_artifact_ids: BTreeSet::from([artifact_id.to_owned()]),
                    consumers: BTreeSet::from([ConsumerDomain::ContractTooling]),
                    resource_budget_profile: None,
                },
                DerivationOutput {
                    path: PathBuf::from(format!("generated/{id}.rs")),
                    output_kind: DerivationOutputKind::RustRegistryBindings,
                    primary_artifact_ids: BTreeSet::from([artifact_id.to_owned()]),
                    consumers: BTreeSet::from([ConsumerDomain::ContractTooling]),
                    resource_budget_profile: None,
                },
                DerivationOutput {
                    path: PathBuf::from(format!("generated/{id}.py")),
                    output_kind: DerivationOutputKind::PythonRegistryBindings,
                    primary_artifact_ids: BTreeSet::from([artifact_id.to_owned()]),
                    consumers: BTreeSet::from([ConsumerDomain::ContractTooling]),
                    resource_budget_profile: None,
                },
            ],
            resource_budget_profile: "test".to_owned(),
        }
    }

    #[test]
    fn record_order_is_not_semantic() {
        let first = catalog(vec![descriptor("b", "b.json"), descriptor("a", "a.json")])
            .compile(Path::new("."), false)
            .unwrap();
        let second = catalog(vec![descriptor("a", "a.json"), descriptor("b", "b.json")])
            .compile(Path::new("."), false)
            .unwrap();
        assert_eq!(first.derivation_order(), second.derivation_order());
        assert_eq!(first.normalized_catalog(), second.normalized_catalog());
        assert_eq!(
            first
                .artifacts()
                .map(|value| &value.artifact_id)
                .collect::<Vec<_>>(),
            second
                .artifacts()
                .map(|value| &value.artifact_id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn wp06a_behavioral_acceptance() {
        let mut synthetic = descriptor("synthetic", "contracts/synthetic.json");
        synthetic.artifact_kind = ArtifactKind::Registry;
        synthetic.native_format = NativeFormat::Yaml;
        synthetic.digest_projection = DigestProjection::YamlAcG53V1;
        let mut derivation = registry_derivation(
            "registry-set",
            "synthetic",
            "contracts/generated/synthetic.json",
        );
        derivation.outputs[0].consumers =
            BTreeSet::from([ConsumerDomain::PythonAdapter, ConsumerDomain::Packaging]);
        let compiled = catalog_with_derivations(vec![synthetic], vec![derivation])
            .compile(Path::new("."), false)
            .unwrap();
        let artifact = compiled.artifact("synthetic").unwrap();
        assert!(
            artifact
                .provenance_requirements
                .contains(&ProvenanceRequirement::CanonicalDigest)
        );
        assert_eq!(
            compiled.package_data(ConsumerDomain::PythonAdapter),
            [Path::new("contracts/generated/synthetic.json")]
        );
        assert_eq!(compiled.outputs().count(), 3);
        assert_eq!(compiled.derivation_order(), ["registry-set"]);
        assert_eq!(
            compiled
                .outputs_of_kind("registry-set", DerivationOutputKind::CanonicalRegistry)
                .count(),
            1
        );
    }

    #[test]
    fn wp06a_negative_zero_state() {
        let duplicate_id = catalog(vec![
            descriptor("duplicate", "first.json"),
            descriptor("duplicate", "second.json"),
        ]);
        assert!(duplicate_id.compile(Path::new("."), false).is_err());

        let duplicate_path = catalog(vec![
            descriptor("a", "same.json"),
            descriptor("b", "same.json"),
        ]);
        assert!(duplicate_path.compile(Path::new("."), false).is_err());

        let mut first_output = descriptor("first-output", "first-output.json");
        first_output.artifact_kind = ArtifactKind::Registry;
        first_output.native_format = NativeFormat::Yaml;
        first_output.digest_projection = DigestProjection::YamlAcG53V1;
        let mut second_output = descriptor("second-output", "second-output.json");
        second_output.artifact_kind = ArtifactKind::Registry;
        second_output.native_format = NativeFormat::Yaml;
        second_output.digest_projection = DigestProjection::YamlAcG53V1;
        assert!(
            catalog_with_derivations(
                vec![first_output, second_output],
                vec![
                    registry_derivation("first", "first-output", "contracts/generated/shared.json"),
                    registry_derivation(
                        "second",
                        "second-output",
                        "contracts/generated/shared.json"
                    ),
                ],
            )
            .compile(Path::new("."), false)
            .is_err()
        );

        let mut first = descriptor("a", "a.json");
        first.artifact_kind = ArtifactKind::Registry;
        first.native_format = NativeFormat::Yaml;
        first.digest_projection = DigestProjection::YamlAcG53V1;
        let mut second = descriptor("b", "b.json");
        second.artifact_kind = ArtifactKind::Registry;
        second.native_format = NativeFormat::Yaml;
        second.digest_projection = DigestProjection::YamlAcG53V1;
        first.semantic_projection_source = SemanticProjectionSource::DerivationOutput {
            output: OutputRef {
                derivation_id: "b-unit".to_owned(),
                path: PathBuf::from("generated/b.json"),
            },
        };
        second.semantic_projection_source = SemanticProjectionSource::DerivationOutput {
            output: OutputRef {
                derivation_id: "a-unit".to_owned(),
                path: PathBuf::from("generated/a.json"),
            },
        };
        let mut first_unit = registry_derivation("a-unit", "a", "generated/a.json");
        let mut second_unit = registry_derivation("b-unit", "b", "generated/b.json");
        first_unit.outputs[0]
            .primary_artifact_ids
            .insert("b".to_owned());
        second_unit.outputs[0]
            .primary_artifact_ids
            .insert("a".to_owned());
        let cycle = catalog_with_derivations(vec![first, second], vec![first_unit, second_unit])
            .compile(Path::new("."), false)
            .unwrap_err();
        assert!(cycle.to_string().contains("cycle"));

        let escaped = catalog(vec![descriptor("escape", "../escape.json")]);
        assert!(escaped.compile(Path::new("."), false).is_err());

        let mut escaped_output = descriptor("escape-output", "safe.json");
        escaped_output.artifact_kind = ArtifactKind::Registry;
        escaped_output.native_format = NativeFormat::Yaml;
        escaped_output.digest_projection = DigestProjection::YamlAcG53V1;
        assert!(
            catalog_with_derivations(
                vec![escaped_output],
                vec![registry_derivation(
                    "escape-unit",
                    "escape-output",
                    "../generated.json"
                )],
            )
            .compile(Path::new("."), false)
            .is_err()
        );
    }

    #[test]
    fn wp06a_operational_acceptance() {
        let mut unknown_profile = descriptor("unknown-profile", "unknown-profile.json");
        unknown_profile.resource_budget_profile = "absent".to_owned();
        assert!(
            catalog(vec![unknown_profile])
                .compile(Path::new("."), false)
                .is_err()
        );

        let mut unknown_dependency = descriptor("unknown-dependency", "dependency.json");
        unknown_dependency.artifact_kind = ArtifactKind::Registry;
        unknown_dependency.native_format = NativeFormat::Yaml;
        unknown_dependency.digest_projection = DigestProjection::YamlAcG53V1;
        assert!(
            catalog_with_derivations(
                vec![unknown_dependency],
                vec![registry_derivation(
                    "missing",
                    "absent",
                    "generated/missing.json"
                )],
            )
            .compile(Path::new("."), false)
            .is_err()
        );

        let temporary = tempfile::tempdir().unwrap();
        let missing = catalog(vec![descriptor("missing", "contracts/missing.json")]);
        assert!(missing.compile(temporary.path(), true).is_err());
    }

    #[test]
    fn wp06a_structural_acceptance() {
        let unknown_field = r#"{
            "artifact_id":"catalog","artifact_kind":"manifest","version":"1.0",
            "compatible_suite_major":1,"status":"draft","canonical_digest":"b3:x",
            "digest_projection":"json-jcs-v1",
            "catalog_schema_version":2,"resource_budget_profiles":[],"artifacts":[],"derivations":[],
            "surprise":true
        }"#;
        assert!(serde_json::from_str::<ContractCatalog>(unknown_field).is_err());
        let unknown_kind = unknown_field
            .replace("\"manifest\"", "\"universal-schema\"")
            .replace(",\n            \"surprise\":true", "");
        assert!(serde_json::from_str::<ContractCatalog>(&unknown_kind).is_err());
    }
}
