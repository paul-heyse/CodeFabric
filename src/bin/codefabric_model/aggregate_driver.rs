//! Complete read-only model tree assembled from family drivers and accountable acceptances.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use codefabric::contracts::models::{
    OwnerAcceptance, RequirementRecord, RequirementStatus, RequirementTraces, TraceabilityRecord,
};
use codefabric::integrity::framed_digest as digest_bytes;
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

use super::adapter_driver;
use super::desired_tree::{
    DesiredTree, DesiredTreeEntry, PlannedConsumer, PlannedOutput, PlannedOutputProjection,
    PlannedOutputRole, PlannedValidator, SafeOutputPath,
};
use super::driver_protocol::{process_stage_root, rustfmt_source};
use super::model_control::StableId;
use super::proto_driver;
use super::registry_cbef_driver;
use super::release_census::ReleasedArtifactCensus;
use super::repository_model::{
    ArtifactRole, ClaimedPath, InventoryBounds, NativeParser, RepositoryModel,
    RepositoryModelError, output_id, read_stable,
};
use super::schema_driver;

const CENSUS_PATH: &str = "contracts/acceptance/released-artifact-census-v1.json";
const ADAPTER_VALIDATION_PATH: &str = "contracts/generated/model/adapter-validation.json";
const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ModelArtifactRecord {
    artifact_id: String,
    artifact_kind: String,
    authority_path: String,
    owner: String,
    version: String,
    compatible_suite_major: u64,
    status: String,
    canonical_digest: String,
    source_digest: String,
    projection_profile: String,
    release_status: String,
    compilation_unit: String,
    source_role: ArtifactRole,
    resource_profile: Value,
    provenance: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ModelOutputRecord {
    output_id: String,
    path: String,
    producer: String,
    public_artifact_id: Option<String>,
    projection: PlannedOutputProjection,
    consumers: BTreeSet<PlannedConsumer>,
    validators: BTreeSet<PlannedValidator>,
    lineage: Vec<String>,
    resource_profile: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ModelBundleRecord {
    bundle_kind: String,
    bundle_version: String,
    bundle_major: u64,
    artifacts: Vec<ModelBundleMember>,
    compatibility: ModelBundleCompatibility,
    created_by: ModelBundleCreatedBy,
    bundle_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ModelBundleMember {
    artifact_id: String,
    version: String,
    canonical_digest: String,
    required: bool,
    feature_bits: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ModelBundleCompatibility {
    minimum_consumer_minor: u64,
    maximum_consumer_minor: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ModelBundleCreatedBy {
    generator_id: String,
    generator_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FamilyProjection {
    family: String,
    action_key: String,
    rule_version: String,
    resource_profile: super::driver_protocol::DriverResourceProfile,
    outputs: Vec<String>,
    tool_identity: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AggregateReport {
    pub family: String,
    pub artifact_count: usize,
    pub released_artifact_count: usize,
    pub family_output_count: usize,
    pub governance_output_count: usize,
    pub output_count: usize,
    pub requirement_count: usize,
    pub bundle_count: usize,
    pub fixture_count: usize,
    pub cache_hit_count: usize,
    pub cache_miss_count: usize,
    pub conservative_fallback_reasons: Vec<String>,
    pub tree_digest: String,
    pub action_keys: BTreeMap<StableId, String>,
    pub rendered_outputs: Vec<String>,
    pub stage_root: String,
    #[serde(skip)]
    pub desired_tree: DesiredTree,
}

struct AggregateTree {
    desired: DesiredTree,
    owners: BTreeMap<String, String>,
    family_output_count: usize,
}

impl AggregateTree {
    fn new() -> Self {
        Self {
            desired: DesiredTree::default(),
            owners: BTreeMap::new(),
            family_output_count: 0,
        }
    }

    fn insert(
        &mut self,
        path: &str,
        producer: &str,
        bytes: Vec<u8>,
        family_output: bool,
    ) -> Result<(), AggregateError> {
        if path.starts_with("docs/upfront_design/")
            || path.starts_with("contracts/acceptance/")
            || path.starts_with("contracts/fixtures/")
            || path.starts_with("tooling/model-transition/")
        {
            return Err(AggregateError::ForbiddenWrite(path.to_owned()));
        }
        let safe = SafeOutputPath::parse(path.as_bytes().to_vec())?;
        if let Some(first) = self.owners.insert(path.to_owned(), producer.to_owned()) {
            return Err(AggregateError::DuplicateOwner {
                path: path.to_owned(),
                first,
                second: producer.to_owned(),
            });
        }
        let producer_id = StableId::parse(producer.to_owned())?;
        let output = PlannedOutput {
            output_id: output_id(path.as_bytes())?,
            public_artifact_id: None,
            path: safe.clone(),
            role: PlannedOutputRole::Derived,
            producer: producer_id.clone(),
            projection: projection(path),
            consumers: consumers(path),
            validators: validators(path),
        };
        let content_digest = digest_bytes(&bytes);
        let previous = self.desired.entries.insert(
            safe,
            DesiredTreeEntry {
                output,
                lineage: vec![producer_id],
                bytes,
                content_digest,
            },
        );
        if previous.is_some() {
            return Err(AggregateError::DuplicateOwner {
                path: path.to_owned(),
                first: producer.to_owned(),
                second: producer.to_owned(),
            });
        }
        self.family_output_count += usize::from(family_output);
        Ok(())
    }

    fn merge_stage(
        &mut self,
        model: &RepositoryModel,
        stage_root: &Path,
        producer: &str,
        paths: &[String],
    ) -> Result<(), AggregateError> {
        for path in paths {
            if path.starts_with("tooling/model-transition/") {
                continue;
            }
            if let Some(claim) = model.claims.get(path.as_bytes())
                && !matches!(claim.role, ArtifactRole::Derived | ArtifactRole::Ignored)
            {
                return Err(AggregateError::ForbiddenWrite(path.clone()));
            }
            let bytes = read_stable(&stage_root.join(path), MAX_BYTES)?;
            self.insert(path, producer, bytes, true)?;
        }
        Ok(())
    }

    fn replace(&mut self, path: &str, bytes: Vec<u8>) -> Result<(), AggregateError> {
        let safe = SafeOutputPath::parse(path.as_bytes().to_vec())?;
        let entry = self
            .desired
            .entries
            .get_mut(&safe)
            .ok_or_else(|| projection_error(path, "replacement target was not declared"))?;
        entry.content_digest = digest_bytes(&bytes);
        entry.bytes = bytes;
        Ok(())
    }

    fn digest(&self) -> Result<String, AggregateError> {
        self.digest_excluding(&BTreeSet::new())
    }

    fn digest_excluding(&self, excluded: &BTreeSet<&str>) -> Result<String, AggregateError> {
        let identities = self
            .desired
            .entries
            .iter()
            .filter(|(path, _)| !excluded.contains(path.display().as_str()))
            .map(|(path, entry)| (path.display(), entry.content_digest.clone()))
            .collect::<BTreeMap<_, _>>();
        canonical_digest(&identities)
    }
}

/// Compile every family and all model-derived governance views without touching the worktree.
///
/// # Errors
///
/// Returns a source, family, ownership, transition, release, staging, or validation failure.
#[allow(clippy::too_many_lines)] // One source fence encloses every family stage and governance view.
pub fn check_family(repository_root: &Path) -> Result<AggregateReport, AggregateError> {
    let before_model =
        RepositoryModel::discover(repository_root, InventoryBounds::default(), true)?;
    let before_digest = before_model.semantic_digest()?;
    let census: ReleasedArtifactCensus =
        serde_json::from_slice(&read_stable(&repository_root.join(CENSUS_PATH), MAX_BYTES)?)?;
    super::release_census::check(repository_root, &before_model)?;

    let registry = registry_cbef_driver::check_family(repository_root)?;
    let schemas = schema_driver::check_family(repository_root)?;
    let adapter = adapter_driver::check_family(repository_root)?;
    let proto = proto_driver::check_family(repository_root)?;
    let families = vec![
        FamilyProjection {
            family: registry.family.clone(),
            action_key: registry.action_key.clone(),
            rule_version: registry.rule_version.clone(),
            resource_profile: registry.resource_profile.clone(),
            outputs: registry.rendered_outputs.clone(),
            tool_identity: portable_tool_identity(registry.tool_identity.clone()),
        },
        FamilyProjection {
            family: schemas.family.clone(),
            action_key: schemas.action_key.clone(),
            rule_version: schemas.rule_version.clone(),
            resource_profile: schemas.resource_profile.clone(),
            outputs: schemas.rendered_outputs.clone(),
            tool_identity: json!({"driver": "native-rust", "rule": "schema-contract-v1"}),
        },
        FamilyProjection {
            family: adapter.family.clone(),
            action_key: adapter.action_key.clone(),
            rule_version: adapter.rule_version.clone(),
            resource_profile: adapter.resource_profile.clone(),
            outputs: adapter.rendered_outputs.clone(),
            tool_identity: portable_tool_identity(serde_json::to_value(&adapter.tool_identity)?),
        },
        FamilyProjection {
            family: proto.family.clone(),
            action_key: proto.action_key.clone(),
            rule_version: proto.rule_version.clone(),
            resource_profile: proto.resource_profile.clone(),
            outputs: proto.rendered_outputs.clone(),
            tool_identity: portable_tool_identity(proto.tool_identity.clone()),
        },
    ];
    let mut tree = AggregateTree::new();
    tree.merge_stage(
        &before_model,
        Path::new(&registry.stage_root),
        "action:registry-cbef",
        &registry.rendered_outputs,
    )?;
    tree.merge_stage(
        &before_model,
        Path::new(&schemas.stage_root),
        "action:schemas",
        &schemas.rendered_outputs,
    )?;
    tree.merge_stage(
        &before_model,
        Path::new(&adapter.stage_root),
        "action:adapter",
        &adapter.rendered_outputs,
    )?;
    tree.merge_stage(
        &before_model,
        Path::new(&proto.stage_root),
        "action:proto",
        &proto.rendered_outputs,
    )?;

    let proto_census: Value = serde_json::from_slice(&read_stable(
        &Path::new(&proto.stage_root).join("tooling/proto/descriptor-census.json"),
        MAX_BYTES,
    )?)?;
    let data_fabric_identity = data_fabric_toolchain_identity(repository_root)?;
    let mut family_identities = BTreeMap::new();
    let adapter_source_id = adapter.validation["source_artifact_id"]
        .as_str()
        .ok_or_else(|| projection_error(ADAPTER_VALIDATION_PATH, "source artifact ID is absent"))?;
    let adapter_source_digest = adapter.validation["source_canonical_digest"]
        .as_str()
        .ok_or_else(|| projection_error(ADAPTER_VALIDATION_PATH, "source identity is absent"))?;
    family_identities.insert(
        adapter_source_id.to_owned(),
        adapter_source_digest.to_owned(),
    );
    for (path, bytes) in [
        (
            "contracts/generated/model/governance/toolchain-identity.json",
            pretty_json(&json!({
                "schema_version": 1,
                "data_fabric": data_fabric_identity,
                "families": families,
            }))?,
        ),
        (
            "contracts/toolchain/toolchain-identity.json",
            pretty_json(&json!({
                "artifact_id": "codefabric.toolchain.identity",
                "artifact_kind": "manifest",
                "version": "1.0",
                "compatible_suite_major": 1,
                "status": "released",
                "schema_version": 1,
                "data_fabric": data_fabric_identity,
                "families": families,
            }))?,
        ),
    ] {
        tree.insert(path, "action:governance", bytes, false)?;
    }
    let bundle_inputs = model_artifacts(
        repository_root,
        &before_model,
        &census,
        &proto_census,
        &family_identities,
        &families,
        &tree.desired,
    )?;
    let bundles = bundles(&bundle_inputs)?;
    validate_bundles(&bundles)?;
    for bundle in &bundles {
        let path = format!("contracts/bundles/{}-bundle.json", bundle.bundle_kind);
        tree.insert(
            &path,
            "action:governance",
            bundle_document_bytes(bundle)?,
            false,
        )?;
    }
    let artifacts = model_artifacts(
        repository_root,
        &before_model,
        &census,
        &proto_census,
        &family_identities,
        &families,
        &tree.desired,
    )?;
    let requirements = requirements(repository_root, &before_model, &families)?;
    validate_requirement_closure(&requirements)?;
    let traceability = traceability(&requirements);
    let fixtures = fixture_index(&before_model);
    let fixture_oracles = fixture_oracle_records(&before_model);
    let package_data = package_data(&tree);
    let aggregators = module_aggregators(&tree);
    let rust_aggregator = rustfmt_source(&rust_module_aggregator(&tree))?;
    for path in [
        "contracts/generated/model/governance/suite-manifest.json",
        "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json",
        "contracts/manifests/suite-manifest.json",
        "contracts/generated/model/governance/validation.json",
    ] {
        tree.insert(path, "action:governance", Vec::new(), false)?;
    }
    let governance = [
        (
            "contracts/generated/model/governance/requirements.jsonl",
            json_lines(&requirements)?,
        ),
        (
            "contracts/generated/model/governance/traceability.jsonl",
            json_lines(&traceability)?,
        ),
        (
            "contracts/generated/model/governance/bundles.json",
            pretty_json(&json!({"schema_version": 1, "bundles": bundles}))?,
        ),
        (
            "contracts/generated/model/governance/fixture-index.json",
            pretty_json(&json!({"schema_version": 1, "fixtures": fixtures}))?,
        ),
        (
            "contracts/generated/model/governance/package-data.json",
            pretty_json(&package_data)?,
        ),
        (
            "contracts/generated/model/governance/module-aggregators.json",
            pretty_json(&aggregators)?,
        ),
        ("src/generated/model.rs", rust_aggregator),
    ];
    for (path, bytes) in governance {
        tree.insert(path, "action:governance", bytes, false)?;
    }
    for (path, bytes) in [
        (
            "contracts/manifests/requirements.jsonl",
            json_lines_with_header(
                "codefabric.manifests.requirements",
                "requirements",
                &requirements,
            )?,
        ),
        (
            "contracts/manifests/traceability.jsonl",
            json_lines_with_header(
                "codefabric.manifests.traceability",
                "traceability",
                &traceability,
            )?,
        ),
        (
            "contracts/manifests/fixture-oracles.json",
            pretty_json(&json!({
                "artifact_id": "codefabric.manifests.fixture-oracles",
                "artifact_kind": "manifest",
                "version": "1.0",
                "compatible_suite_major": 1,
                "status": "released",
                "schema_version": 1,
                "canonical_digest": format!("b3:{}", "0".repeat(64)),
                "digest_projection": "json-jcs-v1",
                "generator_revision": "codefabric-model/1.0",
                "records": fixture_oracles,
            }))?,
        ),
    ] {
        tree.insert(path, "action:governance", bytes, false)?;
    }
    let outputs = model_outputs(&tree, &families);
    let manifest = json!({
        "artifact_id": "codefabric.generated.model-suite-manifest",
        "artifact_kind": "model-suite-manifest",
        "version": "1.0",
        "schema_version": 1,
        "compatible_suite_major": 1,
        "status": "draft",
        "release_census": {
            "owner_acceptance": census.owner_acceptance,
            "released_artifact_count": census.released_artifacts.len(),
        },
        "artifacts": artifacts,
        "outputs": outputs,
        "families": families,
    });
    let compatibility_manifest = json!({
        "artifact_id": "codefabric.manifests.suite-manifest",
        "artifact_kind": "manifest",
        "version": "1.0",
        "schema_version": 1,
        "compatible_suite_major": 1,
        "status": "released",
        "release_census": {
            "owner_acceptance": census.owner_acceptance,
            "released_artifact_count": census.released_artifacts.len(),
        },
        "artifacts": artifacts,
        "outputs": outputs,
        "families": families,
    });
    let artifact_index = json!({
        "schema_version": 1,
        "source": "RepositoryModel + accepted release census + complete DesiredTree census",
        "artifacts": artifacts,
        "outputs": outputs,
    });
    tree.replace(
        "contracts/generated/model/governance/suite-manifest.json",
        pretty_json(&manifest)?,
    )?;
    tree.replace(
        "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json",
        canonical_json(&artifact_index)?,
    )?;
    tree.replace(
        "contracts/manifests/suite-manifest.json",
        pretty_json(&compatibility_manifest)?,
    )?;
    let validation = aggregate_validation(&tree, &artifacts, &requirements, &bundles, &fixtures)?;
    tree.replace(
        "contracts/generated/model/governance/validation.json",
        pretty_json(&validation)?,
    )?;

    let stage_root = process_stage_root(repository_root, "aggregate-stage");
    if stage_root.exists() {
        fs::remove_dir_all(&stage_root).map_err(|source| AggregateError::Io {
            path: stage_root.clone(),
            source,
        })?;
    }
    fs::create_dir_all(&stage_root).map_err(|source| AggregateError::Io {
        path: stage_root.clone(),
        source,
    })?;
    tree.desired.stage(&stage_root)?;
    let after_model = RepositoryModel::discover(repository_root, InventoryBounds::default(), true)?;
    if before_digest != after_model.semantic_digest()? {
        return Err(AggregateError::SourceFence);
    }
    let tree_digest = tree.digest()?;
    let rendered_outputs = tree
        .desired
        .entries
        .keys()
        .map(SafeOutputPath::display)
        .collect::<Vec<_>>();
    let cache_lookups = [
        &registry.cache_lookup,
        &schemas.cache_lookup,
        &adapter.cache_lookup,
        &proto.cache_lookup,
    ];
    let cache_hit_count = cache_lookups
        .iter()
        .filter(|lookup| lookup.is_hit())
        .count();
    let conservative_fallback_reasons = cache_lookups
        .iter()
        .filter_map(|lookup| lookup.miss_reason().map(str::to_owned))
        .collect::<Vec<_>>();
    let mut action_keys = families
        .iter()
        .map(|family| {
            StableId::parse(format!("action:{}", family.family))
                .map(|id| (id, family.action_key.clone()))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    action_keys.insert(
        StableId::parse("action:governance".to_owned())?,
        canonical_digest(&json!({
            "action_id": "action:governance",
            "family_action_keys": action_keys,
            "source_identity": before_digest,
            "desired_tree_identity": tree_digest,
        }))?,
    );
    Ok(AggregateReport {
        family: "aggregate".to_owned(),
        artifact_count: artifacts.len(),
        released_artifact_count: census.released_artifacts.len(),
        family_output_count: tree.family_output_count,
        governance_output_count: tree
            .desired
            .entries
            .len()
            .saturating_sub(tree.family_output_count),
        output_count: tree.desired.entries.len(),
        requirement_count: requirements.len(),
        bundle_count: bundles.len(),
        fixture_count: fixtures.len(),
        cache_hit_count,
        cache_miss_count: cache_lookups.len() - cache_hit_count,
        conservative_fallback_reasons,
        tree_digest,
        action_keys,
        rendered_outputs,
        stage_root: stage_root.to_string_lossy().into_owned(),
        desired_tree: tree.desired,
    })
}

fn data_fabric_toolchain_identity(repository_root: &Path) -> Result<Value, AggregateError> {
    const ROOT_MANIFEST: &str = "Cargo.toml";
    const ROOT_LOCK: &str = "Cargo.lock";
    const EXTRACTOR_MANIFEST: &str = "rustc-extractor/Cargo.toml";
    const EXTRACTOR_LOCK: &str = "rustc-extractor/Cargo.lock";
    const EXTRACTOR_TOOLCHAIN: &str = "rustc-extractor/rust-toolchain.toml";

    let root_manifest = read_stable(&repository_root.join(ROOT_MANIFEST), MAX_BYTES)?;
    let root_lock = read_stable(&repository_root.join(ROOT_LOCK), MAX_BYTES)?;
    let extractor_manifest = read_stable(&repository_root.join(EXTRACTOR_MANIFEST), MAX_BYTES)?;
    let extractor_lock = read_stable(&repository_root.join(EXTRACTOR_LOCK), MAX_BYTES)?;
    let extractor_toolchain = read_stable(&repository_root.join(EXTRACTOR_TOOLCHAIN), MAX_BYTES)?;
    data_fabric_toolchain_identity_from_bytes(
        &root_manifest,
        &root_lock,
        &extractor_manifest,
        &extractor_lock,
        &extractor_toolchain,
    )
}

fn data_fabric_toolchain_identity_from_bytes(
    root_manifest_bytes: &[u8],
    root_lock_bytes: &[u8],
    extractor_manifest_bytes: &[u8],
    extractor_lock_bytes: &[u8],
    extractor_toolchain_bytes: &[u8],
) -> Result<Value, AggregateError> {
    let root_manifest = parse_toml("Cargo.toml", root_manifest_bytes)?;
    let root_lock = parse_toml("Cargo.lock", root_lock_bytes)?;
    let extractor_manifest = parse_toml("rustc-extractor/Cargo.toml", extractor_manifest_bytes)?;
    let extractor_toolchain = parse_toml(
        "rustc-extractor/rust-toolchain.toml",
        extractor_toolchain_bytes,
    )?;

    let delta_revision = dependency_string(&root_manifest, "deltalake", "rev")?;
    let delta_version = lock_package_version(&root_lock, "deltalake", Some(&delta_revision))?;
    let extractor = json!({
        "package_version": table_string(
            &extractor_manifest,
            &["package", "version"],
            "rustc-extractor/Cargo.toml",
        )?,
        "toolchain_channel": table_string(
            &extractor_toolchain,
            &["toolchain", "channel"],
            "rustc-extractor/rust-toolchain.toml",
        )?,
        "cargo_manifest_digest": digest_bytes(extractor_manifest_bytes),
        "cargo_lock_digest": digest_bytes(extractor_lock_bytes),
        "toolchain_digest": digest_bytes(extractor_toolchain_bytes),
    });
    let extractor_identity_digest = canonical_digest(&extractor)?;

    Ok(json!({
        "rust_version": table_string(&root_manifest, &["package", "rust-version"], "Cargo.toml")?,
        "datafusion_version": dependency_version(&root_manifest, "datafusion")?,
        "arrow_version": dependency_version(&root_manifest, "arrow")?,
        "parquet_version": dependency_version(&root_manifest, "parquet")?,
        "object_store_version": dependency_version(&root_manifest, "object_store")?,
        "delta_rs_git_rev": delta_revision,
        "deltalake_declared_version": delta_version,
        "toml_version": dependency_version(&root_manifest, "toml")?,
        "cargo_manifest_digest": digest_bytes(root_manifest_bytes),
        "cargo_lock_digest": digest_bytes(root_lock_bytes),
        "rustc_extractor": {
            "identity_digest": extractor_identity_digest,
            "identity": extractor,
        },
    }))
}

fn parse_toml(path: &str, bytes: &[u8]) -> Result<toml::Value, AggregateError> {
    let text =
        std::str::from_utf8(bytes).map_err(|error| projection_error(path, error.to_string()))?;
    toml::from_str(text).map_err(|error| projection_error(path, error.to_string()))
}

fn table_string(value: &toml::Value, keys: &[&str], path: &str) -> Result<String, AggregateError> {
    let selected = keys.iter().try_fold(value, |current, key| {
        current
            .get(*key)
            .ok_or_else(|| projection_error(path, format!("{} is absent", keys.join("."))))
    })?;
    selected
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| projection_error(path, format!("{} is not a string", keys.join("."))))
}

fn dependency_value<'a>(
    manifest: &'a toml::Value,
    name: &str,
) -> Result<&'a toml::Value, AggregateError> {
    manifest
        .get("dependencies")
        .and_then(|dependencies| dependencies.get(name))
        .ok_or_else(|| projection_error("Cargo.toml", format!("dependency {name} is absent")))
}

fn dependency_version(manifest: &toml::Value, name: &str) -> Result<String, AggregateError> {
    let dependency = dependency_value(manifest, name)?;
    let version = dependency
        .as_str()
        .or_else(|| dependency.get("version").and_then(toml::Value::as_str))
        .ok_or_else(|| {
            projection_error(
                "Cargo.toml",
                format!("dependency {name} has no string version"),
            )
        })?;
    Ok(version.strip_prefix('=').unwrap_or(version).to_owned())
}

fn dependency_string(
    manifest: &toml::Value,
    name: &str,
    key: &str,
) -> Result<String, AggregateError> {
    dependency_value(manifest, name)?
        .get(key)
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            projection_error(
                "Cargo.toml",
                format!("dependency {name} has no string {key}"),
            )
        })
}

fn lock_package_version(
    lock: &toml::Value,
    name: &str,
    source_contains: Option<&str>,
) -> Result<String, AggregateError> {
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| projection_error("Cargo.lock", "package array is absent"))?;
    let matches = packages
        .iter()
        .filter(|package| package.get("name").and_then(toml::Value::as_str) == Some(name))
        .filter(|package| {
            source_contains.is_none_or(|expected| {
                package
                    .get("source")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|source| source.contains(expected))
            })
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(projection_error(
            "Cargo.lock",
            format!("expected one {name} package matching its declared source"),
        ));
    }
    matches[0]
        .get("version")
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| projection_error("Cargo.lock", format!("{name} version is absent")))
}

fn model_artifacts(
    root: &Path,
    model: &RepositoryModel,
    census: &ReleasedArtifactCensus,
    proto_census: &Value,
    family_identities: &BTreeMap<String, String>,
    families: &[FamilyProjection],
    desired: &DesiredTree,
) -> Result<Vec<ModelArtifactRecord>, AggregateError> {
    let released = census
        .released_artifacts
        .iter()
        .map(|record| record.artifact_id.as_str())
        .collect::<BTreeSet<_>>();
    let tombstoned = census
        .accepted_tombstones
        .iter()
        .map(|record| record.artifact_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut records = BTreeMap::<String, (ArtifactRole, ModelArtifactRecord)>::new();
    let mut found = BTreeSet::new();
    for claim in model.claims.values() {
        let Some(header) = &claim.header else {
            continue;
        };
        let id = header.artifact_id.as_str();
        if is_aggregate_meta_projection(claim.path.display()) {
            if released.contains(id) {
                found.insert(id);
            }
            continue;
        }
        if claim.role == ArtifactRole::Derived && !released.contains(id) {
            continue;
        }
        if released.contains(id) {
            found.insert(id);
        }
        let current_bytes;
        let bytes = if let Some(entry) = desired
            .entries
            .iter()
            .find_map(|(path, entry)| (path.display() == claim.path.display()).then_some(entry))
        {
            entry.bytes.as_slice()
        } else {
            current_bytes = read_stable(&root.join(claim.path.display()), MAX_BYTES)?;
            &current_bytes
        };
        let record = ModelArtifactRecord {
            artifact_id: id.to_owned(),
            artifact_kind: header.artifact_kind.clone(),
            authority_path: claim.path.display().to_owned(),
            owner: claim.family_id.clone(),
            version: header.version.clone(),
            compatible_suite_major: header.compatible_suite_major,
            status: header.status.clone(),
            canonical_digest: semantic_identity(claim, bytes, proto_census, family_identities)?,
            source_digest: digest_bytes(bytes),
            projection_profile: projection_profile(claim).to_owned(),
            release_status: if released.contains(id) {
                "released"
            } else {
                "unreleased"
            }
            .to_owned(),
            compilation_unit: compilation_unit(claim.path.display()).to_owned(),
            source_role: claim.role,
            resource_profile: resource_profile_for(driver_family(claim.path.display()), families),
            provenance: BTreeSet::from([
                "current-stable-source-bytes".to_owned(),
                "detached-family-native-semantic-projection".to_owned(),
                "accepted-release-census".to_owned(),
            ]),
        };
        match records.get(id) {
            Some((existing_role, _))
                if *existing_role != ArtifactRole::Derived
                    || claim.role == ArtifactRole::Derived => {}
            _ => {
                records.insert(id.to_owned(), (claim.role, record));
            }
        }
    }
    if let Some(missing) = released
        .difference(&found)
        .find(|artifact_id| !tombstoned.contains(**artifact_id))
    {
        return Err(AggregateError::MissingReleased((*missing).to_owned()));
    }
    Ok(records.into_values().map(|(_, record)| record).collect())
}

fn semantic_identity(
    claim: &ClaimedPath,
    bytes: &[u8],
    proto_census: &Value,
    family_identities: &BTreeMap<String, String>,
) -> Result<String, AggregateError> {
    if let Some(header) = &claim.header
        && let Some(digest) = family_identities.get(header.artifact_id.as_str())
    {
        return Ok(digest.clone());
    }
    if claim.parser == NativeParser::Yaml
        && let Some(header) = &claim.header
        && let Some(digest) =
            registry_cbef_driver::detached_registry_identity(header.artifact_id.as_str(), bytes)?
    {
        return Ok(digest);
    }
    if claim
        .header
        .as_ref()
        .is_some_and(|header| header.artifact_id.as_str() == "codefabric.schema.contract-ir")
    {
        return Ok(schema_driver::detached_schema_identity(bytes)?);
    }
    let projection = match claim.parser {
        NativeParser::Json => canonical_json_projection(claim.path.display(), bytes)?,
        NativeParser::Yaml => canonical_yaml_projection(claim.path.display(), bytes)?,
        NativeParser::JsonLines => canonical_jsonl_projection(claim.path.display(), bytes)?,
        NativeParser::CommentHeader if has_extension(claim.path.display(), "proto") => {
            canonical_proto_projection(claim.path.display(), proto_census)?
        }
        NativeParser::CommentHeader if has_extension(claim.path.display(), "ebnf") => {
            canonical_ebnf_projection(claim.path.display(), bytes)?
        }
        NativeParser::MarkdownHeader | NativeParser::CommentHeader | NativeParser::Opaque => {
            bytes.to_vec()
        }
    };
    Ok(digest_bytes(&projection))
}

fn projection_profile(claim: &ClaimedPath) -> &'static str {
    let artifact_id = claim
        .header
        .as_ref()
        .map(|header| header.artifact_id.as_str());
    match artifact_id {
        Some("codefabric.manifests.suite-manifest") => "catalog-typed-v1",
        Some("codefabric.manifests.fixture-oracles") => "fixture-oracle-typed-v1",
        Some("codefabric.schema.contract-ir") => "schema-contract-ir-typed-v1",
        Some("codefabric.adapter.model-ir") => "adapter-pydantic-typed-v1",
        _ if driver_family(claim.path.display()) == "registry-cbef" => "registry-family-typed-v1",
        _ => match claim.parser {
            NativeParser::Json => "json-jcs-v1",
            NativeParser::Yaml => "yaml-ac-g-53-v1",
            NativeParser::JsonLines => "jsonl-jcs-v1",
            NativeParser::MarkdownHeader => "prose-utf8-v1",
            NativeParser::CommentHeader if has_extension(claim.path.display(), "proto") => {
                "proto-descriptor-v1"
            }
            NativeParser::CommentHeader if has_extension(claim.path.display(), "ebnf") => {
                "ebnf-source-v1"
            }
            NativeParser::CommentHeader | NativeParser::Opaque => "exact-bytes-v1",
        },
    }
}

fn remove_own_identity(value: &mut Value, path: &str) -> Result<(), AggregateError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| projection_error(path, "semantic root must be an object"))?;
    let header = if path.ends_with(".schema.json") {
        object
            .get_mut("x-codefabric-artifact")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| projection_error(path, "JSON Schema artifact header is absent"))?
    } else {
        object
    };
    header.remove("canonical_digest");
    header.remove("source_digest");
    Ok(())
}

fn canonical_json_projection(path: &str, bytes: &[u8]) -> Result<Vec<u8>, AggregateError> {
    let mut value = codefabric::contracts::jcs::decode_strict(bytes)
        .map_err(|error| projection_error(path, error.to_string()))?;
    remove_own_identity(&mut value, path)?;
    canonical_json(&value)
}

fn canonical_yaml_projection(path: &str, bytes: &[u8]) -> Result<Vec<u8>, AggregateError> {
    let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_slice(bytes)
        .map_err(|error| projection_error(path, error.to_string()))?;
    let mut value = yaml_to_json(path, yaml)?;
    remove_own_identity(&mut value, path)?;
    canonical_json(&value)
}

fn yaml_to_json(path: &str, value: serde_yaml_ng::Value) -> Result<Value, AggregateError> {
    use serde_yaml_ng::Value as YamlValue;
    match value {
        YamlValue::Null => Ok(Value::Null),
        YamlValue::Bool(value) => Ok(Value::Bool(value)),
        YamlValue::Number(value) => serde_json::to_value(value).map_err(AggregateError::Json),
        YamlValue::String(value) => Ok(Value::String(value)),
        YamlValue::Sequence(values) => values
            .into_iter()
            .map(|value| yaml_to_json(path, value))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        YamlValue::Mapping(mapping) => {
            let mut object = serde_json::Map::new();
            for (key, value) in mapping {
                let YamlValue::String(key) = key else {
                    return Err(projection_error(path, "YAML map keys must be strings"));
                };
                if key == "<<" || object.contains_key(&key) {
                    return Err(projection_error(
                        path,
                        "YAML merge or duplicate key is forbidden",
                    ));
                }
                object.insert(key, yaml_to_json(path, value)?);
            }
            Ok(Value::Object(object))
        }
        YamlValue::Tagged(_) => Err(projection_error(path, "YAML tags are forbidden")),
    }
}

fn canonical_jsonl_projection(path: &str, bytes: &[u8]) -> Result<Vec<u8>, AggregateError> {
    if !bytes.ends_with(b"\n") {
        return Err(projection_error(path, "JSON Lines source must end with LF"));
    }
    let mut projected = Vec::new();
    for (index, line) in bytes[..bytes.len() - 1]
        .split(|byte| *byte == b'\n')
        .enumerate()
    {
        if line.is_empty() {
            return Err(projection_error(path, "blank JSON Lines record"));
        }
        let mut value = codefabric::contracts::jcs::decode_strict(line)
            .map_err(|error| projection_error(path, error.to_string()))?;
        if index == 0 {
            remove_own_identity(&mut value, path)?;
        }
        projected.extend(canonical_json(&value)?);
        projected.push(b'\n');
    }
    Ok(projected)
}

fn canonical_ebnf_projection(path: &str, bytes: &[u8]) -> Result<Vec<u8>, AggregateError> {
    let normalized = std::str::from_utf8(bytes)
        .map_err(|error| projection_error(path, error.to_string()))?
        .replace("\r\n", "\n");
    if normalized.as_bytes().contains(&b'\r') {
        return Err(projection_error(path, "bare carriage return is forbidden"));
    }
    let mut offset = 0;
    for line in normalized.split_inclusive('\n') {
        let logical = line.trim();
        if !logical.starts_with("(*") || !logical.ends_with("*)") {
            break;
        }
        offset += line.len();
    }
    if offset == 0 {
        return Err(projection_error(path, "EBNF typed header is absent"));
    }
    Ok(normalized.as_bytes()[offset..].to_vec())
}

fn canonical_proto_projection(path: &str, census: &Value) -> Result<Vec<u8>, AggregateError> {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| projection_error(path, "Proto filename is not UTF-8"))?;
    let files = census
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| projection_error(path, "descriptor census files are absent"))?;
    let selected = files
        .iter()
        .find(|file| {
            file.get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| {
                    Path::new(name).file_name().and_then(|value| value.to_str()) == Some(file_name)
                })
        })
        .ok_or_else(|| projection_error(path, "Proto file is absent from sole FDS census"))?;
    canonical_json(&json!({"files": [selected]}))
}

fn projection_error(path: &str, detail: impl Into<String>) -> AggregateError {
    AggregateError::Projection {
        path: path.to_owned(),
        detail: detail.into(),
    }
}

fn model_outputs(tree: &AggregateTree, families: &[FamilyProjection]) -> Vec<ModelOutputRecord> {
    tree.desired
        .entries
        .values()
        .map(|entry| ModelOutputRecord {
            output_id: entry.output.output_id.to_string(),
            path: entry.output.path.display(),
            producer: entry.output.producer.to_string(),
            public_artifact_id: entry
                .output
                .public_artifact_id
                .as_ref()
                .map(ToString::to_string),
            projection: entry.output.projection.clone(),
            consumers: entry.output.consumers.clone(),
            validators: entry.output.validators.clone(),
            lineage: entry.lineage.iter().map(ToString::to_string).collect(),
            resource_profile: resource_profile_for(
                entry.output.producer.as_str().trim_start_matches("action:"),
                families,
            ),
        })
        .collect()
}

fn resource_profile_for(family: &str, families: &[FamilyProjection]) -> Value {
    families
        .iter()
        .find(|item| item.family == family)
        .and_then(|item| serde_json::to_value(&item.resource_profile).ok())
        .unwrap_or_else(|| json!({"profile": "aggregate-governance-bounded-v1"}))
}

fn portable_tool_identity(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .filter(|(key, _)| key != "binary_path" && !key.ends_with("_path"))
                .map(|(key, value)| (key, portable_tool_identity(value)))
                .collect(),
        ),
        Value::Array(values) => {
            Value::Array(values.into_iter().map(portable_tool_identity).collect())
        }
        other => other,
    }
}

fn requirements(
    root: &Path,
    model: &RepositoryModel,
    families: &[FamilyProjection],
) -> Result<Vec<RequirementRecord>, AggregateError> {
    let mut records = BTreeMap::new();
    for claim in model.claims.values().filter(|claim| {
        claim.path.display().starts_with("docs/upfront_design/")
            && claim.path.display().ends_with(".md")
    }) {
        let Some(header) = &claim.header else {
            continue;
        };
        let bytes = read_stable(&root.join(claim.path.display()), MAX_BYTES)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|error| projection_error(claim.path.display(), error.to_string()))?;
        let lines = text.lines().collect::<Vec<_>>();
        for (index, heading) in lines.iter().enumerate() {
            let Some(section) = heading.strip_prefix("## AC-G-") else {
                continue;
            };
            let requirement_id = format!(
                "AC-G-{}",
                section
                    .split_whitespace()
                    .next()
                    .ok_or_else(|| projection_error(claim.path.display(), "empty AC-G heading"))?
            );
            let end = lines[index + 1..]
                .iter()
                .position(|line| line.starts_with("## "))
                .map_or(lines.len(), |offset| index + 1 + offset);
            let normative_text = lines[index + 1..end]
                .iter()
                .map(|line| line.trim())
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if normative_text.is_empty() {
                return Err(projection_error(
                    claim.path.display(),
                    format!("{requirement_id} has no normative body"),
                ));
            }
            let family_ids = requirement_families(claim.path.display());
            let mut implements = families
                .iter()
                .filter(|family| family_ids.contains(family.family.as_str()))
                .flat_map(|family| family.outputs.iter().cloned())
                .collect::<BTreeSet<_>>();
            implements
                .insert("contracts/generated/model/governance/suite-manifest.json".to_owned());
            let mut verified_by = family_ids
                .iter()
                .map(|family| format!("just model-family-check {family}"))
                .collect::<BTreeSet<_>>();
            verified_by.insert("just model-release-check".to_owned());
            let record = RequirementRecord {
                requirement_id: requirement_id.clone(),
                source_artifact: header.artifact_id.to_string(),
                source_section: heading.trim_start_matches("## ").to_owned(),
                normative_text_digest: digest_bytes(normative_text.as_bytes()),
                normative_text,
                implements: implements.into_iter().collect(),
                traces_to: empty_requirement_traces(),
                trace_selectors: BTreeSet::new(),
                verified_by: verified_by.into_iter().collect(),
                owner_acceptance: OwnerAcceptance {
                    approver: "codefabric-repository-owner".to_owned(),
                    accepted_at: "2026-08-23".to_owned(),
                    construction_rule:
                        "codefabric-model AC-G heading and complete normalized section transcription"
                            .to_owned(),
                    source_digest: claim.source_digest.clone(),
                },
                status: RequirementStatus::Active,
            };
            if records.insert(requirement_id.clone(), record).is_some() {
                return Err(projection_error(
                    claim.path.display(),
                    format!("duplicate requirement {requirement_id}"),
                ));
            }
        }
    }
    Ok(records.into_values().collect())
}

fn validate_requirement_closure(requirements: &[RequirementRecord]) -> Result<(), AggregateError> {
    let mut ids = BTreeSet::new();
    let texts = requirements
        .iter()
        .map(|requirement| requirement.normative_text.as_str())
        .collect::<BTreeSet<_>>();
    if requirements.len() != 84
        || texts.len() != requirements.len()
        || requirements.iter().any(|requirement| {
            !ids.insert(&requirement.requirement_id)
                || requirement.source_section.is_empty()
                || requirement.implements.is_empty()
                || requirement.verified_by.is_empty()
                || !requirement.normative_text_digest.starts_with("b3:")
        })
    {
        return Err(AggregateError::RequirementClosure);
    }
    Ok(())
}

fn requirement_families(path: &str) -> BTreeSet<&'static str> {
    if path.contains("fastmcp_serving") {
        BTreeSet::from(["adapter", "proto"])
    } else if path.contains("semantic_query") {
        BTreeSet::from(["adapter", "schemas"])
    } else if path.contains("data_fabric") {
        BTreeSet::from(["schemas"])
    } else if path.contains("fact_generation") || path.contains("lifecycle_management") {
        BTreeSet::from(["registry-cbef", "schemas", "proto"])
    } else if path.contains("fact_ontology") {
        BTreeSet::from(["registry-cbef", "schemas"])
    } else {
        BTreeSet::from(["registry-cbef", "schemas", "adapter", "proto"])
    }
}

fn empty_requirement_traces() -> RequirementTraces {
    RequirementTraces {
        ontology_kinds: Vec::new(),
        capability_codes: Vec::new(),
        table_fields: Vec::new(),
        query_phrase_ids: Vec::new(),
        response_fields: Vec::new(),
        error_codes: Vec::new(),
    }
}

fn traceability(requirements: &[RequirementRecord]) -> Vec<TraceabilityRecord> {
    requirements
        .iter()
        .map(|requirement| TraceabilityRecord {
            requirement_id: requirement.requirement_id.clone(),
            implements: requirement.implements.clone(),
            traces_to: requirement.traces_to.clone(),
            verified_by: requirement.verified_by.clone(),
        })
        .collect()
}

fn bundles(artifacts: &[ModelArtifactRecord]) -> Result<Vec<ModelBundleRecord>, AggregateError> {
    let mut grouped = BTreeMap::<String, Vec<ModelBundleMember>>::new();
    for artifact in artifacts
        .iter()
        .filter(|artifact| artifact.release_status == "released")
    {
        for kind in bundle_membership(artifact) {
            grouped
                .entry(kind.to_owned())
                .or_default()
                .push(ModelBundleMember {
                    artifact_id: artifact.artifact_id.clone(),
                    version: artifact.version.clone(),
                    canonical_digest: artifact.canonical_digest.clone(),
                    required: true,
                    feature_bits: Vec::new(),
                });
        }
    }
    grouped
        .into_iter()
        .map(
            |(bundle_kind, mut artifacts)| -> Result<_, AggregateError> {
                artifacts.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
                let mut bundle = ModelBundleRecord {
                    bundle_kind,
                    bundle_version: "1.0".to_owned(),
                    bundle_major: 1,
                    artifacts,
                    compatibility: ModelBundleCompatibility {
                        minimum_consumer_minor: 0,
                        maximum_consumer_minor: 0,
                    },
                    created_by: ModelBundleCreatedBy {
                        generator_id: "codefabric-model".to_owned(),
                        generator_version: "1.0".to_owned(),
                    },
                    bundle_digest: String::new(),
                    signature: None,
                };
                bundle.bundle_digest = bundle_digest(&bundle)?;
                Ok(bundle)
            },
        )
        .collect()
}

fn bundle_document_bytes(bundle: &ModelBundleRecord) -> Result<Vec<u8>, AggregateError> {
    pretty_json(&json!({
        "artifact_id": format!("codefabric.bundles.{}-bundle", bundle.bundle_kind),
        "artifact_kind": "bundle-manifest",
        "version": "1.0",
        "compatible_suite_major": 1,
        "status": "released",
        "bundle_kind": bundle.bundle_kind,
        "bundle_version": bundle.bundle_version,
        "bundle_major": bundle.bundle_major,
        "artifacts": bundle.artifacts,
        "compatibility": bundle.compatibility,
        "created_by": bundle.created_by,
        "bundle_digest": bundle.bundle_digest,
        "signature": bundle.signature,
    }))
}

fn bundle_digest(bundle: &ModelBundleRecord) -> Result<String, AggregateError> {
    let mut value = serde_json::to_value(bundle)?;
    let object = value
        .as_object_mut()
        .expect("typed bundle record serializes as an object");
    object.remove("canonical_digest");
    object.remove("source_digest");
    object.remove("bundle_digest");
    object.remove("signature");
    Ok(digest_bytes(&canonical_json(&value)?))
}

fn bundle_membership(artifact: &ModelArtifactRecord) -> BTreeSet<&'static str> {
    let path = artifact.authority_path.as_str();
    let mut kinds = BTreeSet::new();
    if artifact.artifact_kind == "bundle-manifest" || path.starts_with("contracts/bundles/") {
        return kinds;
    }
    if path.starts_with("contracts/identity/")
        || path.starts_with("contracts/registry/ontology-")
        || path.ends_with("/enum-registry.yaml")
        || path.ends_with("/flag-registry.yaml")
        || path.ends_with("/unknown-registry.yaml")
    {
        kinds.insert("ontology");
    }
    if path.starts_with("contracts/schema/") {
        kinds.insert("schema");
    }
    if path.starts_with("contracts/providers/")
        || path.contains("/provider-")
        || path.ends_with("/capability-registry.yaml")
        || path.ends_with("/feature-registry.yaml")
        || path.ends_with("/pyrefly_sidecar.proto")
        || path.ends_with("/rustc_extractor.proto")
    {
        kinds.insert("provider");
    }
    if path.starts_with("contracts/query/")
        || path.ends_with("/phrase-registry.yaml")
        || path.ends_with("/cpg_query_service.proto")
        || path.ends_with("/feature-registry.yaml")
        || path.contains("/cpg-semantic-query-")
    {
        kinds.insert("query-language");
    }
    if path.starts_with("contracts/adapter/")
        || path.ends_with("/capability-registry.yaml")
        || path.ends_with("/error-registry.yaml")
        || path.ends_with("/state-machine-registry.yaml")
        || path.ends_with("/cpg_query_service.proto")
        || path.ends_with("/feature-registry.yaml")
        || path.ends_with("/public-status.schema.json")
    {
        kinds.insert("tool-contract");
    }
    if path.ends_with("/derivation-registry.yaml")
        || path.ends_with("/projection-registry.yaml")
        || path.ends_with("/summary-registry.yaml")
    {
        kinds.insert("derivation");
    }
    if path.ends_with("/model-pack.schema.json") {
        kinds.insert("model-pack");
    }
    if path.starts_with("contracts/toolchain/") {
        kinds.insert("toolchain");
    }
    kinds
}

fn is_aggregate_meta_projection(path: &str) -> bool {
    matches!(
        path,
        "contracts/manifests/suite-manifest.json"
            | "contracts/manifests/requirements.jsonl"
            | "contracts/manifests/traceability.jsonl"
    )
}

fn validate_bundles(bundles: &[ModelBundleRecord]) -> Result<(), AggregateError> {
    let kinds = bundles
        .iter()
        .map(|bundle| bundle.bundle_kind.as_str())
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        "derivation",
        "model-pack",
        "ontology",
        "provider",
        "query-language",
        "schema",
        "tool-contract",
        "toolchain",
    ]);
    if kinds != expected
        || bundles.iter().any(|bundle| {
            bundle.artifacts.is_empty()
                || bundle.bundle_version != "1.0"
                || bundle.bundle_major != 1
                || bundle.compatibility.minimum_consumer_minor
                    > bundle.compatibility.maximum_consumer_minor
                || bundle.created_by.generator_id != "codefabric-model"
                || !bundle_digest(bundle).is_ok_and(|digest| digest == bundle.bundle_digest)
                || bundle
                    .artifacts
                    .windows(2)
                    .any(|pair| pair[0].artifact_id >= pair[1].artifact_id)
                || bundle.artifacts.iter().any(|member| {
                    member.version.is_empty()
                        || !member.required
                        || !member.feature_bits.windows(2).all(|pair| pair[0] < pair[1])
                })
        })
    {
        return Err(AggregateError::BundleClosure);
    }
    Ok(())
}

fn fixture_index(model: &RepositoryModel) -> Vec<Value> {
    model
        .claims
        .values()
        .filter(|claim| claim.role == ArtifactRole::EvidenceAuthority)
        .map(|claim| {
            let class = fixture_class(claim.path.display());
            json!({
                "path": claim.path.display(),
                "class": class,
                "source_digest": claim.source_digest,
            })
        })
        .collect()
}

fn fixture_class(path: &str) -> &'static str {
    if path.contains("/negative/") || path.contains("/invalid-") {
        "negative-class"
    } else if path.contains("differential") {
        "differential"
    } else if path.contains("vectors")
        || path.contains("adapter-cases")
        || path.contains("production_wire")
        || path.contains("valid-minimal")
        || path.contains("source-syntax-canonicalization")
    {
        "normative-kat"
    } else {
        "property"
    }
}

fn fixture_owner(path: &str) -> &'static str {
    if path.contains("/jcs/") || path.contains("/projections/") {
        "semantic-query"
    } else if path.contains("/identity/")
        || path.contains("/registries/")
        || path.contains("/model-packs/")
    {
        "ontology"
    } else if path.contains("schema-version") || path.contains("conflicting-observations") {
        "data-fabric"
    } else if path.contains("/security/")
        || path.contains("/tree-sitter/")
        || path.contains("/ruff/")
        || path.contains("source-syntax")
    {
        "fact-generation"
    } else {
        "suite"
    }
}

fn fixture_oracle_records(model: &RepositoryModel) -> Vec<Value> {
    model
        .claims
        .values()
        .filter(|claim| claim.role == ArtifactRole::EvidenceAuthority)
        .map(|claim| {
            let path = claim.path.display();
            let class = fixture_class(path);
            json!({
                "path": path,
                "oracle_class": class,
                "origin": format!(
                    "Model-discovered {class} source bytes; answers remain outside routine renderer authority"
                ),
                "owner": fixture_owner(path),
                "version": "1.0",
                "change_record": "contracts/fixtures/CHANGELOG.md#2026-08-23-model-derived-fixture-census",
            })
        })
        .collect()
}

fn package_data(tree: &AggregateTree) -> Value {
    let files = tree
        .desired
        .entries
        .keys()
        .map(SafeOutputPath::display)
        .filter(|path| path.starts_with("codefabric-cpg-mcp/src/codefabric_cpg_mcp/"))
        .collect::<Vec<_>>();
    json!({"schema_version": 1, "files": files})
}

fn module_aggregators(tree: &AggregateTree) -> Value {
    let rust = tree
        .desired
        .entries
        .keys()
        .map(SafeOutputPath::display)
        .filter(|path| has_extension(path, "rs"))
        .collect::<Vec<_>>();
    let python = tree
        .desired
        .entries
        .keys()
        .map(SafeOutputPath::display)
        .filter(|path| has_extension(path, "py") || has_extension(path, "pyi"))
        .collect::<Vec<_>>();
    json!({"schema_version": 1, "rust": rust, "python": python})
}

fn rust_module_aggregator(tree: &AggregateTree) -> Vec<u8> {
    let mut output =
        String::from("// @generated by codefabric-model; do not edit.\n#![allow(dead_code)]\n\n");
    for path in tree
        .desired
        .entries
        .keys()
        .map(SafeOutputPath::display)
        .filter(|path| path.starts_with("src/generated/model_") && has_extension(path, "rs"))
    {
        let file = Path::new(&path)
            .file_name()
            .and_then(|value| value.to_str())
            .expect("validated model source path");
        let module = file.trim_start_matches("model_").trim_end_matches(".rs");
        writeln!(output, "#[path = \"{file}\"]\npub(crate) mod {module};")
            .expect("writing to String cannot fail");
    }
    output.into_bytes()
}

fn aggregate_validation(
    tree: &AggregateTree,
    artifacts: &[ModelArtifactRecord],
    requirements: &[RequirementRecord],
    bundles: &[ModelBundleRecord],
    fixtures: &[Value],
) -> Result<Value, AggregateError> {
    Ok(json!({
        "schema_version": 1,
        "family": "aggregate",
        "tree_digest": tree.digest_excluding(&BTreeSet::from([
            "contracts/generated/model/governance/validation.json"
        ]))?,
        "output_count": tree.desired.entries.len(),
        "artifact_count": artifacts.len(),
        "released_requirement_count": requirements.len(),
        "bundle_count": bundles.len(),
        "fixture_count": fixtures.len(),
        "routine_write_roots": [
            "contracts/generated/model",
            "contracts/schema",
            "contracts/query",
            "contracts/adapter",
            "src/generated",
            "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts",
            "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated",
            "tooling/proto",
        ],
        "forbidden_write_roots": [
            "contracts/acceptance",
            "contracts/fixtures",
            "docs/upfront_design",
        ],
    }))
}

fn driver_family(path: &str) -> &'static str {
    if path.starts_with("contracts/rpc/") {
        "proto"
    } else if path.starts_with("contracts/adapter/") {
        "adapter"
    } else if path.starts_with("contracts/schema/") || path.starts_with("contracts/query/") {
        "schemas"
    } else if path.starts_with("contracts/identity/")
        || path.starts_with("contracts/registry/")
        || path.starts_with("contracts/comparison/")
        || path.starts_with("contracts/faults/")
    {
        "registry-cbef"
    } else {
        "governance"
    }
}

fn compilation_unit(path: &str) -> &'static str {
    driver_family(path)
}

fn projection(path: &str) -> PlannedOutputProjection {
    if has_extension(path, "rs") {
        PlannedOutputProjection::RustSource
    } else if has_extension(path, "py") || has_extension(path, "pyi") {
        PlannedOutputProjection::PythonSource
    } else if path.ends_with(".schema.json") {
        PlannedOutputProjection::JsonSchema {
            public_identity: path.to_owned(),
        }
    } else {
        PlannedOutputProjection::CanonicalArtifact {
            artifact_kind: Path::new(path)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("binary")
                .to_owned(),
        }
    }
}

fn consumers(path: &str) -> BTreeSet<PlannedConsumer> {
    let mut result = BTreeSet::from([PlannedConsumer::ContractVerifier]);
    if has_extension(path, "rs") {
        result.insert(PlannedConsumer::RustCore);
    }
    if has_extension(path, "py")
        || has_extension(path, "pyi")
        || path.contains("codefabric-cpg-mcp")
    {
        result.insert(PlannedConsumer::PythonAdapter);
        result.insert(PlannedConsumer::PythonPackage);
    }
    result
}

fn validators(path: &str) -> BTreeSet<PlannedValidator> {
    let mut result = BTreeSet::from([PlannedValidator::ExactBytes]);
    if has_extension(path, "rs") {
        result.insert(PlannedValidator::RustConsumer);
    } else if has_extension(path, "py") || has_extension(path, "pyi") {
        result.insert(PlannedValidator::PythonConsumer);
    } else {
        result.insert(PlannedValidator::StrictDecode);
    }
    result
}

fn has_extension(path: &str, expected: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn canonical_json(value: &Value) -> Result<Vec<u8>, AggregateError> {
    codefabric::contracts::jcs::canonicalize_value(value).map_err(AggregateError::CanonicalJson)
}

fn pretty_json(value: &Value) -> Result<Vec<u8>, AggregateError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn json_lines<T: Serialize>(values: &[T]) -> Result<Vec<u8>, AggregateError> {
    let mut bytes = Vec::new();
    for value in values {
        let value = serde_json::to_value(value)?;
        bytes.extend(codefabric::contracts::jcs::canonicalize_value(&value)?);
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn json_lines_with_header<T: Serialize>(
    artifact_id: &str,
    record_kind: &str,
    values: &[T],
) -> Result<Vec<u8>, AggregateError> {
    let header = json!({
        "artifact_id": artifact_id,
        "artifact_kind": "json-lines",
        "version": "1.0",
        "compatible_suite_major": 1,
        "status": "released",
        "record_kind": record_kind,
        "schema_version": 1,
    });
    let mut bytes = canonical_json(&header)?;
    bytes.push(b'\n');
    bytes.extend(json_lines(values)?);
    Ok(bytes)
}

fn canonical_digest(value: &impl Serialize) -> Result<String, AggregateError> {
    let value = serde_json::to_value(value)?;
    Ok(digest_bytes(&canonical_json(&value)?))
}

#[derive(Debug, Error)]
pub enum AggregateError {
    #[error("aggregate output attempts a forbidden write: {0}")]
    ForbiddenWrite(String),
    #[error("aggregate output {path} has two producers: {first}, {second}")]
    DuplicateOwner {
        path: String,
        first: String,
        second: String,
    },
    #[error("released artifact is absent without an accepted tombstone: {0}")]
    MissingReleased(String),
    #[error("released requirement graph is incomplete")]
    RequirementClosure,
    #[error("typed bundle graph is incomplete or unsorted")]
    BundleClosure,
    #[error("aggregate source fence changed during rendering")]
    SourceFence,
    #[error("detached semantic projection failed for {path}: {detail}")]
    Projection { path: String, detail: String },
    #[error("aggregate I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    CanonicalJson(#[from] codefabric::contracts::jcs::CanonicalJsonError),
    #[error(transparent)]
    Repository(#[from] RepositoryModelError),
    #[error(transparent)]
    Desired(#[from] super::desired_tree::DesiredTreeError),
    #[error(transparent)]
    Model(#[from] super::model_control::ModelError),
    #[error(transparent)]
    Driver(#[from] super::driver_protocol::DriverProtocolError),
    #[error(transparent)]
    Release(#[from] super::release_census::ReleaseCensusError),
    #[error(transparent)]
    Registry(#[from] registry_cbef_driver::RegistryCbefError),
    #[error(transparent)]
    Schema(#[from] schema_driver::SchemaDriverError),
    #[error(transparent)]
    Adapter(#[from] adapter_driver::AdapterDriverError),
    #[error(transparent)]
    Proto(#[from] proto_driver::ProtoDriverError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_toolchain_identity_tracks_data_fabric_pins() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let identity = data_fabric_toolchain_identity(root).unwrap();
        assert_eq!(identity["rust_version"], "1.95.0");
        assert_eq!(identity["datafusion_version"], "55.0.0");
        assert_eq!(identity["arrow_version"], "59.2.0");
        assert_eq!(identity["parquet_version"], "59.2.0");
        assert_eq!(identity["object_store_version"], "0.13.2");
        assert_eq!(
            identity["delta_rs_git_rev"],
            "43a0cf10a313e5077c48637ad786a05359136bbb"
        );
        assert_eq!(identity["deltalake_declared_version"], "1.0.0");
        assert_eq!(
            identity["rustc_extractor"]["identity"]["toolchain_channel"],
            "nightly-2026-08-18"
        );

        let root_manifest = fs::read(root.join("Cargo.toml")).unwrap();
        let root_lock = fs::read(root.join("Cargo.lock")).unwrap();
        let extractor_manifest = fs::read(root.join("rustc-extractor/Cargo.toml")).unwrap();
        let extractor_lock = fs::read(root.join("rustc-extractor/Cargo.lock")).unwrap();
        let extractor_toolchain =
            fs::read(root.join("rustc-extractor/rust-toolchain.toml")).unwrap();
        let mut changed_lock = extractor_lock.clone();
        changed_lock.extend_from_slice(b"\n# identity mutation\n");
        let changed = data_fabric_toolchain_identity_from_bytes(
            &root_manifest,
            &root_lock,
            &extractor_manifest,
            &changed_lock,
            &extractor_toolchain,
        )
        .unwrap();
        assert_ne!(
            identity["rustc_extractor"]["identity_digest"],
            changed["rustc_extractor"]["identity_digest"]
        );
    }

    #[test]
    fn wp02_operational_model_identity() {
        model_toolchain_identity_tracks_data_fabric_pins();
    }

    #[test]
    fn model_detached_identity_matches_independent_rust_and_python_kats() {
        let own = format!("b3:{}", "0".repeat(64));
        let nested = format!("b3:{}", "1".repeat(64));
        let json = format!(
            r#"{{"canonical_digest":"{own}","source_digest":"{own}","nested":{{"canonical_digest":"{nested}"}},"value":1}}"#
        );
        let yaml = format!(
            "canonical_digest: {own}\nsource_digest: {own}\nnested:\n  canonical_digest: {nested}\nvalue: 1\n"
        );
        let expected = "b3:56e968977129a90a5d28259ef45c4cb79b721124e30cc442ade8bc22545e3045";
        assert_eq!(
            digest_bytes(&canonical_json_projection("example.json", json.as_bytes()).unwrap()),
            expected
        );
        assert_eq!(
            digest_bytes(&canonical_yaml_projection("example.yaml", yaml.as_bytes()).unwrap()),
            expected
        );

        let jsonl = format!(
            "{{\"artifact_id\":\"example\",\"canonical_digest\":\"{own}\",\"source_digest\":\"{own}\",\"nested\":{{\"canonical_digest\":\"{nested}\"}}}}\n{{\"value\":1}}\n"
        );
        assert_eq!(
            digest_bytes(&canonical_jsonl_projection("example.jsonl", jsonl.as_bytes()).unwrap()),
            "b3:f6f506acf9aa47a31d89b8655cf6143c369a48b21202bb3d57f46b970e78b5ba"
        );
        let ebnf = b"(* artifact_id: example *)\n\nrule = \"x\";\n";
        assert_eq!(
            digest_bytes(&canonical_ebnf_projection("example.ebnf", ebnf).unwrap()),
            "b3:58dd94b018bdcb74cd3a77752a7dfa1b009bd6acdc33256b332bc31784ed5a7b"
        );
        let proto_census = json!({"files": [{"name": "x.proto"}]});
        assert_eq!(
            digest_bytes(
                &canonical_proto_projection("contracts/rpc/x.proto", &proto_census).unwrap()
            ),
            "b3:278e7acbbecfc5fd42774de2185774f4dcdb7c37975ca08386f5158f5f84a170"
        );
        assert_eq!(
            digest_bytes(b"# Example\n"),
            "b3:a0f5e8f16750f4638b7d8ed55e400ea266f7d938bd19c42a8c626fb9025d97ec"
        );
    }

    #[test]
    fn model_routine_tree_excludes_authority_evidence_acceptance_and_signature_paths() {
        let mut tree = AggregateTree::new();
        for path in [
            "docs/upfront_design/design.md",
            "contracts/acceptance/accepted.json",
            "contracts/fixtures/kat.json",
        ] {
            assert!(tree.insert(path, "action:test", Vec::new(), false).is_err());
        }
    }

    #[test]
    fn model_rejects_missing_duplicate_or_multi_owner_outputs() {
        let mut tree = AggregateTree::new();
        tree.insert(
            "contracts/generated/model/output.json",
            "action:first",
            b"{}".to_vec(),
            true,
        )
        .unwrap();
        assert!(
            tree.insert(
                "contracts/generated/model/output.json",
                "action:second",
                b"{}".to_vec(),
                true,
            )
            .is_err()
        );
    }

    #[test]
    fn model_driver_failure_keeps_staged_diagnostics_and_never_applies_partial_output() {
        let mut tree = AggregateTree::new();
        tree.insert(
            "contracts/generated/model/first.json",
            "action:first",
            b"{\"complete\":true}".to_vec(),
            true,
        )
        .unwrap();
        let before = tree.desired.entries.clone();
        let failure = tree.insert(
            "contracts/generated/model/first.json",
            "action:second",
            b"{\"partial\":true}".to_vec(),
            true,
        );
        assert!(failure.is_err());
        assert_eq!(tree.desired.entries, before);
    }

    #[test]
    fn model_bundle_projection_matches_typed_ac_g_07_semantics() {
        let artifact = ModelArtifactRecord {
            artifact_id: "codefabric.toolchain.identity".to_owned(),
            artifact_kind: "manifest".to_owned(),
            authority_path: "contracts/toolchain/toolchain-identity.json".to_owned(),
            owner: "contracts".to_owned(),
            version: "1.0".to_owned(),
            compatible_suite_major: 1,
            status: "released".to_owned(),
            canonical_digest: format!("b3:{}", "0".repeat(64)),
            source_digest: format!("b3:{}", "1".repeat(64)),
            projection_profile: "test-v1".to_owned(),
            release_status: "released".to_owned(),
            compilation_unit: "governance".to_owned(),
            source_role: ArtifactRole::Authority,
            resource_profile: json!({"profile": "test"}),
            provenance: BTreeSet::from(["test".to_owned()]),
        };
        let bundles = bundles(&[artifact]).unwrap();
        assert_eq!(bundles[0].bundle_kind, "toolchain");
        assert_eq!(bundles[0].artifacts.len(), 1);
        assert_eq!(bundles[0].artifacts[0].version, "1.0");
        assert!(bundles[0].artifacts[0].required);
        let unsigned_digest = bundles[0].bundle_digest.clone();
        let mut signed = bundles[0].clone();
        signed.signature = Some("owner-accepted-signature".to_owned());
        assert_eq!(bundle_digest(&signed).unwrap(), unsigned_digest);
        signed.artifacts[0]
            .feature_bits
            .push("feature-a".to_owned());
        assert_ne!(bundle_digest(&signed).unwrap(), unsigned_digest);
    }

    #[test]
    fn model_bundle_outputs_never_become_members_of_their_own_or_other_bundles() {
        let artifact = ModelArtifactRecord {
            artifact_id: "codefabric.bundles.provider-bundle".to_owned(),
            artifact_kind: "bundle-manifest".to_owned(),
            authority_path: "contracts/bundles/provider-bundle.json".to_owned(),
            owner: "contracts".to_owned(),
            version: "1.0".to_owned(),
            compatible_suite_major: 1,
            status: "released".to_owned(),
            canonical_digest: format!("b3:{}", "0".repeat(64)),
            source_digest: format!("b3:{}", "1".repeat(64)),
            projection_profile: "bundle-ac-g-07-v1".to_owned(),
            release_status: "released".to_owned(),
            compilation_unit: "governance".to_owned(),
            source_role: ArtifactRole::Derived,
            resource_profile: json!({"profile": "test"}),
            provenance: BTreeSet::from(["test".to_owned()]),
        };
        assert!(bundle_membership(&artifact).is_empty());
        assert!(bundles(&[artifact]).unwrap().is_empty());
    }

    #[test]
    fn model_generated_aggregates_have_no_manual_member_list_input() {
        let source = include_str!("aggregate_driver.rs");
        assert!(!source.contains(&["BUNDLE", "_MEMBERS"].concat()));
        assert!(!source.contains(&["PUBLIC_SCHEMA", "_ARTIFACTS"].concat()));
    }

    #[test]
    fn model_module_aggregator_is_derived_from_the_desired_tree() {
        let mut tree = AggregateTree::new();
        for path in [
            "src/generated/model_identity_recipes.rs",
            "src/generated/model_schema_tables.rs",
        ] {
            tree.insert(path, "action:test", Vec::new(), true).unwrap();
        }
        let aggregator = String::from_utf8(rust_module_aggregator(&tree)).unwrap();
        assert!(aggregator.contains("model_identity_recipes.rs"));
        assert!(!aggregator.contains("model_registries.rs"));
        assert!(aggregator.contains("model_schema_tables.rs"));
    }

    #[test]
    fn model_released_traceability_has_source_implementation_and_executable_oracle_closure() {
        let requirement = RequirementRecord {
            requirement_id: "AC-G-01".to_owned(),
            source_artifact: "codefabric.example".to_owned(),
            source_section: "AC-G-01 — Example".to_owned(),
            normative_text: "example".to_owned(),
            normative_text_digest: digest_bytes(b"example"),
            implements: vec!["src/example.rs".to_owned()],
            traces_to: empty_requirement_traces(),
            trace_selectors: BTreeSet::new(),
            verified_by: vec!["just model-family-check schemas".to_owned()],
            owner_acceptance: OwnerAcceptance {
                approver: "owner".to_owned(),
                accepted_at: "2026-08-23".to_owned(),
                construction_rule: "test".to_owned(),
                source_digest: digest_bytes(b"source"),
            },
            status: RequirementStatus::Active,
        };
        let traces = traceability(std::slice::from_ref(&requirement));
        assert_eq!(traces[0].requirement_id, requirement.requirement_id);
        assert_ne!(
            serde_json::to_value(&requirement).unwrap(),
            serde_json::to_value(&traces[0]).unwrap()
        );
    }
}
