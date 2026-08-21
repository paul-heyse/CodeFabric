//! Typed catalog and derivation graph for governed contract artifacts.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read as _, Take};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The sole production bootstrap path for the contract compiler.
pub const CATALOG_PATH: &str = "contracts/manifests/suite-manifest.json";

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

/// Purpose and representation of a generated output.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeneratedOutputKind {
    /// Canonical shared artifact-index JSON.
    ArtifactIndex,
    /// Canonical JSON derived from a registry.
    CanonicalRegistry,
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
}

/// Closed generator dispatch selected by catalog output edges.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeneratedOutputProducer {
    /// The model-based contract compiler.
    ContractCompiler,
    /// The single descriptor-first Protobuf compiler pipeline.
    ProtoCompiler,
    /// The Contract-IR to Pydantic model/schema compiler.
    AdapterModelCompiler,
}

const fn contract_compiler() -> GeneratedOutputProducer {
    GeneratedOutputProducer::ContractCompiler
}

/// One output edge owned by exactly one artifact descriptor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedOutput {
    /// Repository-relative output path.
    pub path: PathBuf,
    /// Output representation and generator dispatch.
    pub output_kind: GeneratedOutputKind,
    /// Generator selected for this derivation edge.
    #[serde(default = "contract_compiler")]
    pub producer: GeneratedOutputProducer,
    /// Named cap applied when another compiler consumes this generated IR.
    pub resource_budget_profile: Option<String>,
    /// Domains which consume or package the output.
    pub consumers: BTreeSet<ConsumerDomain>,
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
    /// Named resource budget.
    pub resource_budget_profile: String,
    /// Native schema/parser authority, when distinct from the source itself.
    pub parser_schema_authority: Option<String>,
    /// Output edges owned by this descriptor.
    pub generated_outputs: Vec<GeneratedOutput>,
    /// Domains consuming the source directly.
    pub consumers: BTreeSet<ConsumerDomain>,
    /// Provenance required in the compiled index.
    pub provenance_requirements: BTreeSet<ProvenanceRequirement>,
    /// Artifact IDs which must compile before this source.
    pub depends_on: BTreeSet<String>,
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
}

/// Validated deterministic views used by generators and consumers.
type ArtifactsById = BTreeMap<String, ArtifactDescriptor>;
type ArtifactIdByPath = BTreeMap<PathBuf, String>;
type OutputsByPath = BTreeMap<PathBuf, (String, GeneratedOutput)>;
type BudgetsById = BTreeMap<String, ResourceBudgetProfile>;

#[derive(Clone, Debug)]
pub struct CompiledCatalog {
    normalized_catalog: ContractCatalog,
    artifacts: ArtifactsById,
    artifact_by_path: ArtifactIdByPath,
    outputs: OutputsByPath,
    budgets: BudgetsById,
    topological_order: Vec<String>,
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

    /// Generated outputs sorted by repository-relative path.
    pub fn outputs(&self) -> impl Iterator<Item = (&Path, &str, &GeneratedOutput)> {
        self.outputs
            .iter()
            .map(|(path, (owner, output))| (path.as_path(), owner.as_str(), output))
    }

    /// Find the unique output of a selected kind.
    #[must_use]
    pub fn output_of_kind(&self, kind: GeneratedOutputKind) -> Option<&Path> {
        let mut matches = self
            .outputs()
            .filter(|(_, _, output)| output.output_kind == kind)
            .map(|(path, _, _)| path);
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }

    /// Find the unique output edge of a selected kind.
    #[must_use]
    pub fn output_record_of_kind(
        &self,
        kind: GeneratedOutputKind,
    ) -> Option<(&Path, &GeneratedOutput)> {
        let mut matches = self
            .outputs()
            .filter(|(_, _, output)| output.output_kind == kind)
            .map(|(path, _, output)| (path, output));
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }

    /// Named resource budget selected by an artifact.
    #[must_use]
    pub fn budget(&self, profile_id: &str) -> Option<&ResourceBudgetProfile> {
        self.budgets.get(profile_id)
    }

    /// Dependency-safe artifact IDs with deterministic tie breaking.
    #[must_use]
    pub fn topological_order(&self) -> &[String] {
        &self.topological_order
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
        let compiled = catalog.compile(repository_root, true)?;
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
        let outputs = compile_outputs(&artifacts)?;
        let topological_order = topological_order(&artifacts)?;
        validate_self_descriptor(self, &artifacts)?;
        Ok(CompiledCatalog {
            normalized_catalog,
            artifacts,
            artifact_by_path,
            outputs,
            budgets,
            topological_order,
        })
    }

    fn normalized(&self) -> Self {
        let mut normalized = self.clone();
        normalized
            .resource_budget_profiles
            .sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
        for artifact in &mut normalized.artifacts {
            artifact.generated_outputs.sort_by(|left, right| {
                left.path
                    .cmp(&right.path)
                    .then(left.output_kind.cmp(&right.output_kind))
            });
        }
        normalized
            .artifacts
            .sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
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
    for output in &artifact.generated_outputs {
        if output
            .resource_budget_profile
            .as_ref()
            .is_some_and(|profile| !budgets.contains_key(profile))
        {
            return Err(graph_error(
                &output.path,
                format!(
                    "unknown generated-output resource budget: {}",
                    output
                        .resource_budget_profile
                        .as_deref()
                        .unwrap_or_default()
                ),
            ));
        }
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

fn compile_outputs(artifacts: &ArtifactsById) -> Result<OutputsByPath, CatalogError> {
    let mut outputs = BTreeMap::new();
    let authority_paths = artifacts
        .values()
        .map(|artifact| &artifact.authority_path)
        .collect::<BTreeSet<_>>();
    for artifact in artifacts.values() {
        for output in &artifact.generated_outputs {
            if !safe_relative(&output.path) {
                return Err(graph_error(
                    &output.path,
                    "output path is not a safe repository-relative path",
                ));
            }
            if authority_paths.contains(&output.path) {
                return Err(graph_error(
                    &output.path,
                    "generated output conflicts with an authority path",
                ));
            }
            if let Some((previous, _)) = outputs.insert(
                output.path.clone(),
                (artifact.artifact_id.clone(), output.clone()),
            ) {
                return Err(graph_error(
                    &output.path,
                    format!("output also belongs to {previous}"),
                ));
            }
        }
    }
    Ok(outputs)
}

fn topological_order(artifacts: &ArtifactsById) -> Result<Vec<String>, CatalogError> {
    let mut indegree = BTreeMap::new();
    let mut dependants: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (artifact_id, artifact) in artifacts {
        indegree.insert(artifact_id.as_str(), artifact.depends_on.len());
        for dependency in &artifact.depends_on {
            if !artifacts.contains_key(dependency) {
                return Err(graph_error(
                    &artifact.authority_path,
                    format!("unknown dependency: {dependency}"),
                ));
            }
            dependants
                .entry(dependency)
                .or_default()
                .insert(artifact_id);
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(artifact_id, _)| *artifact_id)
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(artifacts.len());
    while let Some(artifact_id) = ready.pop_first() {
        ordered.push(artifact_id.to_owned());
        for dependant in dependants.get(artifact_id).into_iter().flatten() {
            let count = indegree
                .get_mut(dependant)
                .expect("dependant was initialized");
            *count -= 1;
            if *count == 0 {
                ready.insert(dependant);
            }
        }
    }
    if ordered.len() != artifacts.len() {
        return Err(graph_error(CATALOG_PATH, "artifact dependency cycle"));
    }
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
            resource_budget_profile: "test".to_owned(),
            parser_schema_authority: None,
            generated_outputs: Vec::new(),
            consumers: BTreeSet::from([ConsumerDomain::ContractTooling]),
            provenance_requirements: BTreeSet::from([
                ProvenanceRequirement::SourceDigest,
                ProvenanceRequirement::CanonicalDigest,
            ]),
            depends_on: BTreeSet::new(),
        }
    }

    fn catalog(mut artifacts: Vec<ArtifactDescriptor>) -> ContractCatalog {
        artifacts.push(descriptor("catalog", CATALOG_PATH));
        ContractCatalog {
            artifact_id: "catalog".to_owned(),
            artifact_kind: ArtifactKind::Manifest,
            version: "1.0".to_owned(),
            compatible_suite_major: 1,
            status: ArtifactStatus::Draft,
            canonical_digest: "b3:placeholder".to_owned(),
            digest_projection: DigestProjection::JsonJcsV1,
            catalog_schema_version: 1,
            resource_budget_profiles: vec![budget()],
            artifacts,
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
        assert_eq!(first.topological_order(), second.topological_order());
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
    fn synthetic_descriptor_derives_all_views() {
        let mut synthetic = descriptor("synthetic", "contracts/synthetic.json");
        synthetic.generated_outputs.push(GeneratedOutput {
            path: PathBuf::from("contracts/generated/synthetic.json"),
            output_kind: GeneratedOutputKind::CanonicalRegistry,
            producer: GeneratedOutputProducer::ContractCompiler,
            resource_budget_profile: None,
            consumers: BTreeSet::from([ConsumerDomain::PythonAdapter, ConsumerDomain::Packaging]),
        });
        let compiled = catalog(vec![synthetic])
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
        assert_eq!(compiled.outputs().count(), 1);
    }

    #[test]
    fn graph_rejects_duplicate_paths_outputs_cycles_and_escape() {
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
        first_output.generated_outputs.push(GeneratedOutput {
            path: PathBuf::from("contracts/generated/shared.json"),
            output_kind: GeneratedOutputKind::CanonicalRegistry,
            producer: GeneratedOutputProducer::ContractCompiler,
            resource_budget_profile: None,
            consumers: BTreeSet::from([ConsumerDomain::ContractTooling]),
        });
        let mut second_output = descriptor("second-output", "second-output.json");
        second_output.generated_outputs.push(GeneratedOutput {
            path: PathBuf::from("contracts/generated/shared.json"),
            output_kind: GeneratedOutputKind::CanonicalRegistry,
            producer: GeneratedOutputProducer::ContractCompiler,
            resource_budget_profile: None,
            consumers: BTreeSet::from([ConsumerDomain::ContractTooling]),
        });
        assert!(
            catalog(vec![first_output, second_output])
                .compile(Path::new("."), false)
                .is_err()
        );

        let mut first = descriptor("a", "a.json");
        let mut second = descriptor("b", "b.json");
        first.depends_on.insert("b".to_owned());
        second.depends_on.insert("a".to_owned());
        assert!(
            catalog(vec![first, second])
                .compile(Path::new("."), false)
                .is_err()
        );

        let escaped = catalog(vec![descriptor("escape", "../escape.json")]);
        assert!(escaped.compile(Path::new("."), false).is_err());

        let mut escaped_output = descriptor("escape-output", "safe.json");
        escaped_output.generated_outputs.push(GeneratedOutput {
            path: PathBuf::from("../generated.json"),
            output_kind: GeneratedOutputKind::CanonicalRegistry,
            producer: GeneratedOutputProducer::ContractCompiler,
            resource_budget_profile: None,
            consumers: BTreeSet::from([ConsumerDomain::ContractTooling]),
        });
        assert!(
            catalog(vec![escaped_output])
                .compile(Path::new("."), false)
                .is_err()
        );
    }

    #[test]
    fn graph_rejects_unknown_profiles_dependencies_and_missing_authorities() {
        let mut unknown_profile = descriptor("unknown-profile", "unknown-profile.json");
        unknown_profile.resource_budget_profile = "absent".to_owned();
        assert!(
            catalog(vec![unknown_profile])
                .compile(Path::new("."), false)
                .is_err()
        );

        let mut unknown_dependency = descriptor("unknown-dependency", "dependency.json");
        unknown_dependency.depends_on.insert("absent".to_owned());
        assert!(
            catalog(vec![unknown_dependency])
                .compile(Path::new("."), false)
                .is_err()
        );

        let temporary = tempfile::tempdir().unwrap();
        let missing = catalog(vec![descriptor("missing", "contracts/missing.json")]);
        assert!(missing.compile(temporary.path(), true).is_err());
    }

    #[test]
    fn closed_catalog_rejects_unknown_fields_and_kinds() {
        let unknown_field = r#"{
            "artifact_id":"catalog","artifact_kind":"manifest","version":"1.0",
            "compatible_suite_major":1,"status":"draft","canonical_digest":"b3:x",
            "digest_projection":"json-jcs-v1",
            "catalog_schema_version":1,"resource_budget_profiles":[],"artifacts":[],
            "surprise":true
        }"#;
        assert!(serde_json::from_str::<ContractCatalog>(unknown_field).is_err());
        let unknown_kind = unknown_field
            .replace("\"manifest\"", "\"universal-schema\"")
            .replace(",\n            \"surprise\":true", "");
        assert!(serde_json::from_str::<ContractCatalog>(&unknown_kind).is_err());
    }
}
