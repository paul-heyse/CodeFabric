//! Resource-bounded native ingress and semantic digest projection.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read as _, Take};
use std::path::{Path, PathBuf};

use prost::Message as _;
use prost_types::{DescriptorProto, EnumDescriptorProto, FileDescriptorProto, FileDescriptorSet};
use serde::de::DeserializeOwned;
use serde::de::IntoDeserializer as _;
use serde_json::{Map, Value, json};
use serde_yaml_ng::Value as YamlValue;
use thiserror::Error;

use super::catalog::{
    ArtifactDescriptor, ArtifactKind, ArtifactStatus, CompiledCatalog, DigestProjection,
    NativeFormat, ResourceBudgetProfile, SemanticProjectionSource,
};
use super::jcs::{canonicalize_value, checksum, decode_strict};
use super::models::{
    ArtifactHeader, BundleDocument, FixtureOracleClass, FixtureOracleManifest, JsonlMetadata,
    RegistryDocument, RequirementRecord, ScaffoldDocument, TraceabilityRecord,
};

const MAX_DIAGNOSTIC_BYTES: usize = 512;

/// Post-parse resource observations retained for evidence and limit diagnostics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceUsage {
    /// Exact source bytes.
    pub bytes: usize,
    /// Deepest semantic value nesting level.
    pub depth: usize,
    /// Aggregate semantic nodes.
    pub nodes: usize,
    /// Largest one collection.
    pub collection_items: usize,
    /// Largest decoded string/token in UTF-8 bytes.
    pub string_bytes: usize,
    /// Ordered records or graph edges.
    pub records_or_edges: usize,
    /// YAML aliases observed before semantic materialization.
    pub aliases: usize,
    /// Diagnostics accumulated for this artifact.
    pub diagnostics: usize,
}

impl ResourceUsage {
    fn merge(&mut self, other: Self) {
        self.bytes = self.bytes.max(other.bytes);
        self.depth = self.depth.max(other.depth);
        self.nodes += other.nodes;
        self.collection_items = self.collection_items.max(other.collection_items);
        self.string_bytes = self.string_bytes.max(other.string_bytes);
        self.records_or_edges += other.records_or_edges;
        self.aliases += other.aliases;
        self.diagnostics += other.diagnostics;
    }
}

/// One compiled source identity and its exact semantic bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledArtifact {
    /// Stable catalog identity.
    pub artifact_id: String,
    /// Selected projection profile.
    pub digest_projection: DigestProjection,
    /// Exact checked-in bytes digest.
    pub source_digest: String,
    /// Semantic identity currently embedded in the source header, absent for prose.
    pub embedded_canonical_digest: Option<String>,
    /// Semantic projection digest.
    pub canonical_digest: String,
    /// Canonical semantic projection bytes.
    pub canonical_bytes: Vec<u8>,
    /// AC-G-07 bundle identity, only for bundle artifacts.
    pub bundle_digest: Option<String>,
    /// AC-G-07 identity currently embedded in a bundle source.
    pub embedded_bundle_digest: Option<String>,
    /// Bounded resource observations.
    pub usage: ResourceUsage,
}

/// Stable native-ingress failure with bounded path-aware diagnostics.
#[derive(Debug, Error)]
pub enum ContractCompileError {
    /// Source input could not be read.
    #[error("ingress-io path={path}: {message}")]
    Io {
        /// Affected source.
        path: PathBuf,
        /// Bounded operating-system diagnostic.
        message: String,
    },
    /// A named resource budget was exceeded.
    #[error(
        "resource-limit path={path} data_path={data_path} limit={limit} observed={observed} maximum={maximum}"
    )]
    Limit {
        /// Affected source.
        path: PathBuf,
        /// Stable logical location.
        data_path: String,
        /// Catalog budget field.
        limit: &'static str,
        /// Observed resource use.
        observed: usize,
        /// Configured maximum.
        maximum: usize,
    },
    /// Native parsing or typed decoding failed.
    #[error("{class} path={path} data_path={data_path}: {message}")]
    Parse {
        /// Stable failure class.
        class: &'static str,
        /// Affected source.
        path: PathBuf,
        /// Stable logical location or record number.
        data_path: String,
        /// Bounded parser diagnostic.
        message: String,
    },
    /// Embedded typed identity disagrees with the catalog.
    #[error("header-mismatch path={path} data_path={data_path}: {message}")]
    Header {
        /// Affected source.
        path: PathBuf,
        /// Stable header location.
        data_path: String,
        /// Bounded mismatch detail.
        message: String,
    },
    /// A non-placeholder semantic digest claim is stale or malformed.
    #[error("digest-mismatch path={path} claimed={claimed} computed={computed}")]
    Digest {
        /// Affected source.
        path: PathBuf,
        /// Embedded claim.
        claimed: String,
        /// Computed semantic identity.
        computed: String,
    },
}

impl ContractCompileError {
    /// Stable class for cross-language failure comparison.
    #[must_use]
    pub const fn failure_class(&self) -> &'static str {
        match self {
            Self::Io { .. } => "ingress-io",
            Self::Limit { .. } => "resource-limit",
            Self::Parse { class, .. } => class,
            Self::Header { .. } => "header-mismatch",
            Self::Digest { .. } => "digest-mismatch",
        }
    }
}

fn bounded(message: impl std::fmt::Display) -> String {
    let mut rendered = message.to_string();
    if rendered.len() > MAX_DIAGNOSTIC_BYTES {
        rendered.truncate(MAX_DIAGNOSTIC_BYTES);
    }
    rendered
}

fn io_error(path: &Path, error: impl std::fmt::Display) -> ContractCompileError {
    ContractCompileError::Io {
        path: path.to_owned(),
        message: bounded(error),
    }
}

fn parse_error(
    class: &'static str,
    path: &Path,
    data_path: impl Into<String>,
    message: impl std::fmt::Display,
) -> ContractCompileError {
    ContractCompileError::Parse {
        class,
        path: path.to_owned(),
        data_path: data_path.into(),
        message: bounded(message),
    }
}

fn limit_error(
    path: &Path,
    data_path: impl Into<String>,
    limit: &'static str,
    observed: usize,
    maximum: usize,
) -> ContractCompileError {
    ContractCompileError::Limit {
        path: path.to_owned(),
        data_path: data_path.into(),
        limit,
        observed,
        maximum,
    }
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, ContractCompileError> {
    let file = File::open(path).map_err(|error| io_error(path, error))?;
    let sentinel = maximum.saturating_add(1);
    let mut reader: Take<File> = file.take(u64::try_from(sentinel).unwrap_or(u64::MAX));
    let mut bytes = Vec::with_capacity(maximum.min(64 * 1024));
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(path, error))?;
    if bytes.len() > maximum {
        return Err(limit_error(path, "$", "max_bytes", bytes.len(), maximum));
    }
    Ok(bytes)
}

fn strict_json(path: &Path, data_path: &str, bytes: &[u8]) -> Result<Value, ContractCompileError> {
    decode_strict(bytes)
        .map_err(|error| parse_error(error.failure_class(), path, data_path, bounded(error)))
}

fn typed<T: DeserializeOwned>(
    path: &Path,
    data_path: &str,
    value: Value,
) -> Result<T, ContractCompileError> {
    serde_path_to_error::deserialize(value.into_deserializer()).map_err(|error| {
        let nested = error.path().to_string();
        let nested_path = if nested.is_empty() || nested == "." {
            data_path.to_owned()
        } else if data_path == "$" {
            format!("$.{nested}")
        } else {
            format!("{data_path}.{nested}")
        };
        parse_error("typed-record", path, nested_path, error.inner())
    })
}

fn observe_value(value: &Value, depth: usize) -> ResourceUsage {
    let mut usage = ResourceUsage {
        depth,
        nodes: 1,
        string_bytes: match value {
            Value::String(value) => value.len(),
            Value::Number(value) => value.to_string().len(),
            _ => 0,
        },
        ..ResourceUsage::default()
    };
    match value {
        Value::Array(values) => {
            usage.collection_items = values.len();
            for value in values {
                usage.merge(observe_value(value, depth + 1));
            }
        }
        Value::Object(values) => {
            usage.collection_items = values.len();
            for (key, value) in values {
                usage.string_bytes = usage.string_bytes.max(key.len());
                usage.merge(observe_value(value, depth + 1));
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    usage
}

fn enforce_usage(
    path: &Path,
    usage: ResourceUsage,
    budget: &ResourceBudgetProfile,
) -> Result<(), ContractCompileError> {
    for (name, observed, maximum) in [
        ("max_depth", usage.depth, budget.max_depth),
        ("max_nodes", usage.nodes, budget.max_nodes),
        (
            "max_collection_items",
            usage.collection_items,
            budget.max_collection_items,
        ),
        (
            "max_string_bytes",
            usage.string_bytes,
            budget.max_string_bytes,
        ),
        (
            "max_records_or_edges",
            usage.records_or_edges,
            budget.max_records_or_edges,
        ),
        ("max_aliases", usage.aliases, budget.max_aliases),
        ("max_diagnostics", usage.diagnostics, budget.max_diagnostics),
    ] {
        if observed > maximum {
            return Err(limit_error(path, "$", name, observed, maximum));
        }
    }
    Ok(())
}

fn header_fields(
    path: &Path,
    data_path: &str,
    object: &Map<String, Value>,
) -> Result<ArtifactHeader, ContractCompileError> {
    if object.contains_key("source_digest") {
        return Err(parse_error(
            "embedded-source-digest",
            path,
            data_path,
            "source_digest must be detached because it hashes complete source bytes",
        ));
    }
    let required = [
        "artifact_id",
        "artifact_kind",
        "version",
        "compatible_suite_major",
        "status",
        "canonical_digest",
    ];
    let optional = ["digest_projection", "generator_revision"];
    let mut header = Map::new();
    for field in required {
        let value = object.get(field).ok_or_else(|| {
            parse_error(
                "typed-header",
                path,
                data_path,
                format!("required field is absent: {field}"),
            )
        })?;
        header.insert(field.to_owned(), value.clone());
    }
    for field in optional {
        if let Some(value) = object.get(field) {
            header.insert(field.to_owned(), value.clone());
        }
    }
    typed(path, data_path, Value::Object(header))
}

fn validate_header(
    path: &Path,
    data_path: &str,
    header: &ArtifactHeader,
    descriptor: &ArtifactDescriptor,
) -> Result<(), ContractCompileError> {
    let valid_version = valid_version(&header.version);
    let matches = header.artifact_id == descriptor.artifact_id
        && header.artifact_kind == descriptor.artifact_kind
        && header.version == descriptor.version
        && header.compatible_suite_major == descriptor.compatible_suite_major
        && header.status == descriptor.status
        && header
            .digest_projection
            .is_none_or(|profile| profile == descriptor.digest_projection)
        && valid_version;
    if !matches {
        return Err(ContractCompileError::Header {
            path: path.to_owned(),
            data_path: data_path.to_owned(),
            message: bounded(format!(
                "catalog={} kind={:?} version={} suite={} status={:?}; source={} kind={:?} version={} suite={} status={:?}",
                descriptor.artifact_id,
                descriptor.artifact_kind,
                descriptor.version,
                descriptor.compatible_suite_major,
                descriptor.status,
                header.artifact_id,
                header.artifact_kind,
                header.version,
                header.compatible_suite_major,
                header.status
            )),
        });
    }
    Ok(())
}

fn verify_claim(path: &Path, claimed: &str, computed: &str) -> Result<(), ContractCompileError> {
    if claimed == computed {
        Ok(())
    } else {
        Err(ContractCompileError::Digest {
            path: path.to_owned(),
            claimed: claimed.to_owned(),
            computed: computed.to_owned(),
        })
    }
}

fn remove_identity_fields(object: &mut Map<String, Value>) {
    object.remove("canonical_digest");
    object.remove("source_digest");
}

fn valid_version(version: &str) -> bool {
    version.split_once('.').is_some_and(|(major, minor)| {
        !major.is_empty()
            && !minor.is_empty()
            && major.bytes().all(|byte| byte.is_ascii_digit())
            && minor.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn validate_bundle(
    path: &Path,
    descriptor: &ArtifactDescriptor,
    document: &mut BundleDocument,
) -> Result<(), ContractCompileError> {
    if document.artifact_kind != ArtifactKind::BundleManifest {
        return Err(parse_error(
            "typed-record",
            path,
            "$.artifact_kind",
            "bundle document must use artifact_kind=bundle-manifest",
        ));
    }
    let expected_id = format!(
        "codefabric.bundles.{}-bundle",
        document.bundle_kind.artifact_slug()
    );
    let bundle_version_major = document
        .bundle_version
        .split_once('.')
        .and_then(|(major, _)| major.parse::<u16>().ok());
    if document.artifact_id != expected_id {
        return Err(parse_error(
            "bundle-kind",
            path,
            "$.bundle_kind",
            format!(
                "bundle kind implies artifact_id={expected_id}, got {}",
                document.artifact_id
            ),
        ));
    }
    if !valid_version(&document.bundle_version)
        || document.bundle_version != descriptor.version
        || bundle_version_major != Some(document.bundle_major)
    {
        return Err(parse_error(
            "bundle-version",
            path,
            "$.bundle_version",
            "bundle_version, bundle_major, and catalog version disagree",
        ));
    }
    if document.compatibility.minimum_consumer_minor > document.compatibility.maximum_consumer_minor
    {
        return Err(parse_error(
            "bundle-compatibility",
            path,
            "$.compatibility",
            "minimum_consumer_minor exceeds maximum_consumer_minor",
        ));
    }
    if document.created_by.generator_id.is_empty()
        || !valid_version(&document.created_by.generator_version)
    {
        return Err(parse_error(
            "bundle-generator",
            path,
            "$.created_by",
            "generator_id must be nonempty and generator_version must be major.minor",
        ));
    }

    document
        .artifacts
        .sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    for (index, member) in document.artifacts.iter().enumerate() {
        if member.artifact_id.is_empty() || !valid_version(&member.version) {
            return Err(parse_error(
                "bundle-member",
                path,
                format!("$.artifacts[{index}]"),
                "artifact_id must be nonempty and version must be major.minor",
            ));
        }
        super::jcs::validate_checksum(&member.canonical_digest).map_err(|error| {
            parse_error(
                error.failure_class(),
                path,
                format!("$.artifacts[{index}].canonical_digest"),
                error,
            )
        })?;
    }
    if document
        .artifacts
        .windows(2)
        .any(|pair| pair[0].artifact_id == pair[1].artifact_id)
    {
        return Err(parse_error(
            "duplicate-bundle-member",
            path,
            "$.artifacts",
            "duplicate artifact_id",
        ));
    }
    Ok(())
}

fn collect_regular_files(
    repository_root: &Path,
    directory: &Path,
    files: &mut BTreeSet<String>,
) -> Result<(), ContractCompileError> {
    let entries = fs::read_dir(directory).map_err(|error| io_error(directory, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error(directory, error))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| io_error(&path, error))?;
        if file_type.is_dir() {
            collect_regular_files(repository_root, &path, files)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(repository_root)
                .map_err(|error| parse_error("fixture-path", &path, "$", error))?;
            if !matches!(
                relative.file_name().and_then(|name| name.to_str()),
                Some("README.md" | "CHANGELOG.md")
            ) {
                files.insert(relative.to_string_lossy().into_owned());
            }
        } else {
            return Err(parse_error(
                "fixture-path",
                &path,
                "$",
                "fixture entries must be regular files",
            ));
        }
    }
    Ok(())
}

fn classified_fixture_paths(
    repository_root: &Path,
) -> Result<BTreeSet<String>, ContractCompileError> {
    let mut paths = BTreeSet::new();
    for relative in ["contracts/fixtures", "fuzz/corpus/contract_ingress"] {
        collect_regular_files(repository_root, &repository_root.join(relative), &mut paths)?;
    }
    for relative in [
        "tooling/proto/compatibility-baseline.json",
        "tooling/proto/descriptor-census.json",
        "tooling/proto/toolchain-identity.json",
        "tooling/proto/wave0-probe-descriptor.pb",
    ] {
        if !repository_root.join(relative).is_file() {
            return Err(parse_error(
                "fixture-path",
                &repository_root.join(relative),
                "$",
                "classified protocol evidence is absent",
            ));
        }
        paths.insert(relative.to_owned());
    }
    Ok(paths)
}

fn validate_fixture_oracles(
    repository_root: &Path,
    path: &Path,
    document: &mut FixtureOracleManifest,
) -> Result<(), ContractCompileError> {
    document
        .records
        .sort_by(|left, right| left.path.cmp(&right.path));
    for pair in document.records.windows(2) {
        if pair[0].path == pair[1].path {
            return Err(parse_error(
                "fixture-oracle",
                path,
                "$.records",
                format!("duplicate fixture path: {}", pair[0].path),
            ));
        }
    }
    for record in &document.records {
        let relative = Path::new(&record.path);
        if relative.is_absolute()
            || record.path.is_empty()
            || relative
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
            || record.origin.is_empty()
            || record.version.is_empty()
            || !record
                .change_record
                .starts_with("contracts/fixtures/CHANGELOG.md#")
        {
            return Err(parse_error(
                "fixture-oracle",
                path,
                "$.records",
                format!(
                    "incomplete or unsafe fixture classification: {}",
                    record.path
                ),
            ));
        }
        if record.oracle_class == FixtureOracleClass::NormativeKat
            && record
                .origin
                .to_ascii_lowercase()
                .contains("generated by codefabric")
        {
            return Err(parse_error(
                "fixture-oracle",
                path,
                "$.records",
                format!("normative KAT lacks an independent origin: {}", record.path),
            ));
        }
    }
    let declared = document
        .records
        .iter()
        .map(|record| record.path.clone())
        .collect::<BTreeSet<_>>();
    let actual = classified_fixture_paths(repository_root)?;
    if declared != actual {
        let mismatch = declared
            .symmetric_difference(&actual)
            .next()
            .cloned()
            .unwrap_or_else(|| "unknown".to_owned());
        return Err(parse_error(
            "fixture-oracle-census",
            path,
            "$.records",
            format!("unclassified or missing fixture: {mismatch}"),
        ));
    }
    Ok(())
}

type CanonicalJson = (
    ArtifactHeader,
    Vec<u8>,
    Option<String>,
    Option<String>,
    ResourceUsage,
);

fn canonical_bundle_json(
    path: &Path,
    descriptor: &ArtifactDescriptor,
    value: Value,
    mut usage: ResourceUsage,
) -> Result<CanonicalJson, ContractCompileError> {
    let mut document: BundleDocument = typed(path, "$", value)?;
    let header = document.header();
    validate_header(path, "$", &header, descriptor)?;
    validate_bundle(path, descriptor, &mut document)?;
    usage.records_or_edges = document.artifacts.len();
    let embedded_bundle_digest = Some(document.bundle_digest.clone());
    let mut artifact_value =
        serde_json::to_value(document).expect("typed bundle serialization is infallible");
    remove_identity_fields(
        artifact_value
            .as_object_mut()
            .expect("typed bundle serializes as an object"),
    );
    let mut bundle_value = artifact_value.clone();
    let bundle_object = bundle_value
        .as_object_mut()
        .expect("typed bundle clone remains an object");
    bundle_object.remove("bundle_digest");
    bundle_object.remove("signature");
    let bundle_digest = checksum(
        &canonicalize_value(&bundle_value)
            .map_err(|error| parse_error(error.failure_class(), path, "$", error))?,
    );
    let canonical = canonicalize_value(&artifact_value)
        .map_err(|error| parse_error(error.failure_class(), path, "$", error))?;
    Ok((
        header,
        canonical,
        Some(bundle_digest),
        embedded_bundle_digest,
        usage,
    ))
}

fn canonical_json(
    repository_root: &Path,
    path: &Path,
    descriptor: &ArtifactDescriptor,
    catalog: &CompiledCatalog,
    bytes: &[u8],
) -> Result<CanonicalJson, ContractCompileError> {
    let mut value = strict_json(path, "$", bytes)?;
    let usage = observe_value(&value, 1);
    if descriptor.digest_projection == DigestProjection::BundleAcG07V1 {
        return canonical_bundle_json(path, descriptor, value, usage);
    }
    let mut usage = usage;
    let object = value
        .as_object_mut()
        .ok_or_else(|| parse_error("typed-header", path, "$", "root must be an object"))?;

    let (header, header_path) = if descriptor.artifact_kind == ArtifactKind::JsonSchema {
        let header_value = object.get("x-codefabric-artifact").ok_or_else(|| {
            parse_error(
                "typed-header",
                path,
                "$.x-codefabric-artifact",
                "schema artifact header is absent",
            )
        })?;
        let header: ArtifactHeader = typed(path, "$.x-codefabric-artifact", header_value.clone())?;
        (header, "$.x-codefabric-artifact")
    } else {
        (header_fields(path, "$", object)?, "$")
    };
    validate_header(path, header_path, &header, descriptor)?;

    if descriptor.authority_path == Path::new(super::catalog::CATALOG_PATH) {
        usage.records_or_edges = catalog.artifact_count()
            + catalog
                .derivations()
                .map(|unit| {
                    unit.inputs.len()
                        + unit.outputs.len()
                        + unit
                            .outputs
                            .iter()
                            .map(|output| output.consumers.len())
                            .sum::<usize>()
                })
                .sum::<usize>();
        value = serde_json::to_value(catalog.normalized_catalog())
            .expect("typed catalog serialization is infallible");
        remove_identity_fields(
            value
                .as_object_mut()
                .expect("typed catalog serializes as an object"),
        );
    } else if descriptor.artifact_id == "codefabric.manifests.fixture-oracles" {
        let mut document: FixtureOracleManifest = typed(path, "$", value)?;
        validate_fixture_oracles(repository_root, path, &mut document)?;
        usage.records_or_edges = document.records.len();
        value = serde_json::to_value(document)
            .expect("typed fixture-oracle manifest serialization is infallible");
        remove_identity_fields(
            value
                .as_object_mut()
                .expect("typed fixture manifest serializes as an object"),
        );
    } else if descriptor.artifact_kind == ArtifactKind::JsonSchema {
        let header = value
            .get_mut("x-codefabric-artifact")
            .and_then(Value::as_object_mut)
            .expect("schema header was checked as an object");
        remove_identity_fields(header);
    } else {
        remove_identity_fields(
            value
                .as_object_mut()
                .expect("JSON root was checked as an object"),
        );
    }

    let canonical = canonicalize_value(&value)
        .map_err(|error| parse_error(error.failure_class(), path, "$", error))?;
    Ok((header, canonical, None, None, usage))
}

fn yaml_to_json(
    path: &Path,
    data_path: &str,
    value: YamlValue,
) -> Result<Value, ContractCompileError> {
    match value {
        YamlValue::Null => Ok(Value::Null),
        YamlValue::Bool(value) => Ok(Value::Bool(value)),
        YamlValue::Number(number) => serde_json::to_value(number)
            .map_err(|error| parse_error("yaml-number", path, data_path, error)),
        YamlValue::String(value) => Ok(Value::String(value)),
        YamlValue::Sequence(values) => values
            .into_iter()
            .enumerate()
            .map(|(index, value)| yaml_to_json(path, &format!("{data_path}[{index}]"), value))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        YamlValue::Mapping(mapping) => {
            let mut projected = Vec::with_capacity(mapping.len());
            let mut all_string_keys = true;
            for (index, (key, value)) in mapping.into_iter().enumerate() {
                if matches!(&key, YamlValue::String(key) if key == "<<") {
                    return Err(parse_error(
                        "yaml-merge-key",
                        path,
                        data_path,
                        "merge keys are not supported",
                    ));
                }
                all_string_keys &= matches!(&key, YamlValue::String(_));
                projected.push((
                    yaml_to_json(path, &format!("{data_path}.key[{index}]"), key)?,
                    yaml_to_json(path, &format!("{data_path}.value[{index}]"), value)?,
                ));
            }
            if all_string_keys {
                let mut object = Map::new();
                for (key, value) in projected {
                    let Value::String(key) = key else {
                        unreachable!("all keys were checked as strings")
                    };
                    if object.insert(key.clone(), value).is_some() {
                        return Err(parse_error(
                            "duplicate-key",
                            path,
                            data_path,
                            format!("duplicate YAML key: {key}"),
                        ));
                    }
                }
                Ok(Value::Object(object))
            } else {
                super::jcs::non_string_map_records(projected)
                    .map_err(|error| parse_error(error.failure_class(), path, data_path, error))
            }
        }
        YamlValue::Tagged(tagged) => Err(parse_error(
            "yaml-tag",
            path,
            data_path,
            format!("tag {} is not supported", tagged.tag),
        )),
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct YamlIndicators {
    anchors: usize,
    aliases: usize,
    tags: usize,
    merge_keys: usize,
}

fn indicator_boundary_before(line: &[u8], index: usize) -> bool {
    index == 0
        || line[index - 1].is_ascii_whitespace()
        || matches!(line[index - 1], b'[' | b'{' | b',' | b':' | b'-' | b'?')
}

fn scan_yaml_subset(bytes: &[u8]) -> YamlIndicators {
    let mut found = YamlIndicators::default();
    let mut block_indent = None;
    for raw_line in bytes.split(|byte| *byte == b'\n') {
        let indentation = raw_line.iter().take_while(|byte| **byte == b' ').count();
        if let Some(parent_indent) = block_indent {
            if raw_line.is_empty() || indentation > parent_indent {
                continue;
            }
            block_indent = None;
        }

        let mut single_quoted = false;
        let mut double_quoted = false;
        let mut escaped = false;
        let mut index = 0;
        let mut visible_end = raw_line.len();
        while index < raw_line.len() {
            let byte = raw_line[index];
            if double_quoted {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    double_quoted = false;
                }
                index += 1;
                continue;
            }
            if single_quoted {
                if byte == b'\'' {
                    if raw_line.get(index + 1) == Some(&b'\'') {
                        index += 2;
                        continue;
                    }
                    single_quoted = false;
                }
                index += 1;
                continue;
            }
            match byte {
                b'#' if indicator_boundary_before(raw_line, index) => {
                    visible_end = index;
                    break;
                }
                b'"' => double_quoted = true,
                b'\'' => single_quoted = true,
                b'&' if indicator_boundary_before(raw_line, index) => found.anchors += 1,
                b'*' if indicator_boundary_before(raw_line, index) => found.aliases += 1,
                b'!' if indicator_boundary_before(raw_line, index) => found.tags += 1,
                b'<' if indicator_boundary_before(raw_line, index)
                    && raw_line.get(index + 1) == Some(&b'<')
                    && raw_line
                        .get(index + 2)
                        .is_some_and(|next| next.is_ascii_whitespace() || *next == b':') =>
                {
                    found.merge_keys += 1;
                }
                _ => {}
            }
            index += 1;
        }
        let visible = raw_line[..visible_end].trim_ascii_end();
        let block_token = visible
            .rsplit(|byte| byte.is_ascii_whitespace() || *byte == b':')
            .next()
            .unwrap_or_default();
        if block_token
            .first()
            .is_some_and(|byte| matches!(byte, b'|' | b'>'))
            && block_token[1..]
                .iter()
                .all(|byte| matches!(byte, b'+' | b'-' | b'1'..=b'9'))
        {
            block_indent = Some(indentation);
        }
    }
    found
}

fn canonical_yaml(
    path: &Path,
    descriptor: &ArtifactDescriptor,
    bytes: &[u8],
) -> Result<(ArtifactHeader, Vec<u8>, ResourceUsage), ContractCompileError> {
    let indicators = scan_yaml_subset(bytes);
    let aliases = indicators.anchors + indicators.aliases;
    if aliases > 0 {
        return Err(limit_error(path, "$", "max_aliases", aliases, 0));
    }
    if indicators.tags > 0 {
        return Err(parse_error(
            "yaml-tag",
            path,
            "$",
            "explicit YAML tags are outside the accepted subset",
        ));
    }
    if indicators.merge_keys > 0 {
        return Err(parse_error(
            "yaml-merge-key",
            path,
            "$",
            "YAML merge keys are outside the accepted subset",
        ));
    }
    let yaml: YamlValue = serde_yaml_ng::from_slice(bytes)
        .map_err(|error| parse_error("yaml-parse", path, "$", error))?;
    let mut value = yaml_to_json(path, "$", yaml)?;
    let mut usage = observe_value(&value, 1);
    usage.aliases = aliases;

    let (header, records) = match descriptor.artifact_kind {
        ArtifactKind::Registry => {
            let document: RegistryDocument<Value> = typed(path, "$", value.clone())?;
            let records = document.records.len();
            (document.header(), records)
        }
        ArtifactKind::YamlContract => {
            let document: ScaffoldDocument<Value> = typed(path, "$", value.clone())?;
            if document.records.is_some() == document.rules.is_some() {
                return Err(parse_error(
                    "typed-record",
                    path,
                    "$",
                    "exactly one of records or rules is required",
                ));
            }
            let records = document
                .records
                .as_ref()
                .or(document.rules.as_ref())
                .map_or(0, Vec::len);
            (document.header(), records)
        }
        _ => {
            return Err(parse_error(
                "typed-record",
                path,
                "$",
                "YAML profile selected for a non-YAML artifact kind",
            ));
        }
    };
    usage.records_or_edges = records;
    validate_header(path, "$", &header, descriptor)?;
    remove_identity_fields(
        value
            .as_object_mut()
            .ok_or_else(|| parse_error("typed-header", path, "$", "root must be a mapping"))?,
    );
    let canonical = canonicalize_value(&value)
        .map_err(|error| parse_error(error.failure_class(), path, "$", error))?;
    Ok((header, canonical, usage))
}

fn canonical_jsonl(
    path: &Path,
    descriptor: &ArtifactDescriptor,
    bytes: &[u8],
) -> Result<(ArtifactHeader, Vec<u8>, ResourceUsage), ContractCompileError> {
    if !bytes.ends_with(b"\n") {
        return Err(parse_error(
            "jsonl-framing",
            path,
            "$",
            "JSON Lines source must end with LF",
        ));
    }
    let mut values = Vec::new();
    let mut usage = ResourceUsage::default();
    for (index, line) in bytes[..bytes.len() - 1]
        .split(|byte| *byte == b'\n')
        .enumerate()
    {
        let record_number = index + 1;
        if line.is_empty() {
            return Err(parse_error(
                "jsonl-framing",
                path,
                format!("record[{record_number}]"),
                "blank records are not permitted",
            ));
        }
        let value = strict_json(path, &format!("record[{record_number}]"), line)?;
        usage.merge(observe_value(&value, 1));
        values.push(value);
    }
    usage.records_or_edges += values.len();
    let metadata_value = values.first().ok_or_else(|| {
        parse_error(
            "typed-header",
            path,
            "record[1]",
            "metadata record is absent",
        )
    })?;
    let metadata: JsonlMetadata = typed(path, "record[1]", metadata_value.clone())?;
    let header = metadata.header();
    validate_header(path, "record[1]", &header, descriptor)?;

    for (index, value) in values.iter().skip(1).cloned().enumerate() {
        let data_path = format!("record[{}]", index + 2);
        if descriptor.artifact_id == "codefabric.manifests.requirements" {
            let record: RequirementRecord = typed(path, &data_path, value)?;
            for digest in [
                &record.normative_text_digest,
                &record.owner_acceptance.source_digest,
            ] {
                super::jcs::validate_checksum(digest)
                    .map_err(|error| parse_error(error.failure_class(), path, &data_path, error))?;
            }
        } else if descriptor.artifact_id == "codefabric.manifests.traceability" {
            let _: TraceabilityRecord = typed(path, &data_path, value)?;
        } else {
            return Err(parse_error(
                "typed-record",
                path,
                &data_path,
                "no JSONL record model is registered for this artifact",
            ));
        }
    }

    let first = values
        .first_mut()
        .and_then(Value::as_object_mut)
        .expect("metadata record was decoded as an object");
    remove_identity_fields(first);
    let mut canonical = Vec::new();
    for value in &values {
        canonical.extend(
            canonicalize_value(value)
                .map_err(|error| parse_error(error.failure_class(), path, "$", error))?,
        );
        canonical.push(b'\n');
    }
    Ok((header, canonical, usage))
}

fn comment_header(
    path: &Path,
    bytes: &[u8],
    prefix: &str,
    suffix: &str,
) -> Result<(ArtifactHeader, usize), ContractCompileError> {
    let source =
        std::str::from_utf8(bytes).map_err(|error| parse_error("utf8", path, "$", error))?;
    let known = BTreeSet::from([
        "artifact_id",
        "artifact_kind",
        "version",
        "compatible_suite_major",
        "status",
        "canonical_digest",
        "digest_projection",
        "generator_revision",
    ]);
    let mut fields = Map::new();
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let logical = line.trim_end_matches(['\r', '\n']).trim();
        if !logical.starts_with(prefix) || !logical.ends_with(suffix) {
            break;
        }
        let inner = logical
            .strip_prefix(prefix)
            .and_then(|value| value.strip_suffix(suffix))
            .expect("comment delimiters were checked")
            .trim();
        let Some((key, raw_value)) = inner.split_once(':') else {
            break;
        };
        let key = key.trim();
        if !known.contains(key) {
            return Err(parse_error(
                "typed-header",
                path,
                "$header",
                format!("unknown metadata field: {key}"),
            ));
        }
        if fields.contains_key(key) {
            return Err(parse_error(
                "duplicate-key",
                path,
                "$header",
                format!("duplicate metadata field: {key}"),
            ));
        }
        let raw_value = raw_value.trim();
        let value = if key == "compatible_suite_major" {
            Value::from(
                raw_value
                    .parse::<u16>()
                    .map_err(|error| parse_error("typed-header", path, "$header", error))?,
            )
        } else {
            Value::String(raw_value.to_owned())
        };
        fields.insert(key.to_owned(), value);
        offset += line.len();
    }
    let header: ArtifactHeader = typed(path, "$header", Value::Object(fields))?;
    Ok((header, offset))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum EbnfToken {
    Identifier(String),
    Literal,
    Equals,
    Semicolon,
    Alternative,
    Comma,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
}

fn ebnf_literal_end(
    path: &Path,
    payload: &str,
    start: usize,
) -> Result<usize, ContractCompileError> {
    let bytes = payload.as_bytes();
    let mut index = start + 1;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        index += 1;
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            return Ok(index);
        }
    }
    Err(parse_error(
        "ebnf-string",
        path,
        format!("$grammar.byte[{start}]"),
        "unterminated string literal",
    ))
}

fn tokenize_ebnf(
    path: &Path,
    payload: &str,
) -> Result<(Vec<EbnfToken>, usize), ContractCompileError> {
    let bytes = payload.as_bytes();
    let mut tokens = Vec::new();
    let mut max_token = 0usize;
    let mut index = 0usize;
    let mut comment_depth = 0usize;
    while index < bytes.len() {
        if bytes.get(index..index + 2) == Some(b"(*") {
            comment_depth += 1;
            index += 2;
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"*)") {
            if comment_depth == 0 {
                return Err(parse_error(
                    "ebnf-comment",
                    path,
                    format!("$grammar.byte[{index}]"),
                    "unmatched comment terminator",
                ));
            }
            comment_depth -= 1;
            index += 2;
            continue;
        }
        if comment_depth > 0 {
            index += 1;
            continue;
        }
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            let identifier = payload[start..index].to_owned();
            max_token = max_token.max(identifier.len());
            tokens.push(EbnfToken::Identifier(identifier));
            continue;
        }
        if bytes[index] == b'"' {
            let start = index;
            index = ebnf_literal_end(path, payload, start)?;
            max_token = max_token.max(index - start);
            tokens.push(EbnfToken::Literal);
            continue;
        }
        let token = match bytes[index] {
            b'=' => EbnfToken::Equals,
            b';' => EbnfToken::Semicolon,
            b'|' => EbnfToken::Alternative,
            b',' => EbnfToken::Comma,
            b'(' => EbnfToken::LeftParen,
            b')' => EbnfToken::RightParen,
            b'[' => EbnfToken::LeftBracket,
            b']' => EbnfToken::RightBracket,
            b'{' => EbnfToken::LeftBrace,
            b'}' => EbnfToken::RightBrace,
            byte => {
                return Err(parse_error(
                    "ebnf-token",
                    path,
                    format!("$grammar.byte[{index}]"),
                    format!("unsupported byte 0x{byte:02x}"),
                ));
            }
        };
        tokens.push(token);
        index += 1;
    }
    if comment_depth != 0 {
        return Err(parse_error(
            "ebnf-comment",
            path,
            "$grammar",
            "unterminated comment",
        ));
    }
    Ok((tokens, max_token))
}

struct EbnfParser<'a> {
    path: &'a Path,
    tokens: Vec<EbnfToken>,
    cursor: usize,
    definitions: BTreeMap<String, usize>,
    references: Vec<(String, usize)>,
    usage: ResourceUsage,
}

impl EbnfParser<'_> {
    fn error(&self, class: &'static str, message: impl std::fmt::Display) -> ContractCompileError {
        parse_error(
            class,
            self.path,
            format!("$grammar.token[{}]", self.cursor),
            message,
        )
    }

    fn parse(mut self, max_token: usize) -> Result<ResourceUsage, ContractCompileError> {
        while self.cursor < self.tokens.len() {
            let position = self.cursor;
            let Some(EbnfToken::Identifier(name)) = self.tokens.get(self.cursor).cloned() else {
                return Err(self.error("ebnf-production", "expected production name"));
            };
            self.cursor += 1;
            if self.definitions.insert(name.clone(), position).is_some() {
                return Err(self.error(
                    "ebnf-duplicate-production",
                    format!("duplicate production: {name}"),
                ));
            }
            if self.tokens.get(self.cursor) != Some(&EbnfToken::Equals) {
                return Err(self.error("ebnf-production", "expected '=' after production name"));
            }
            self.cursor += 1;
            self.parse_expression(1, &[EbnfToken::Semicolon])?;
            if self.tokens.get(self.cursor) != Some(&EbnfToken::Semicolon) {
                return Err(self.error("ebnf-production", "expected ';' after expression"));
            }
            self.cursor += 1;
            self.usage.nodes += 1;
        }
        if self.definitions.is_empty() {
            return Err(self.error("ebnf-production", "at least one production is required"));
        }
        for (reference, position) in &self.references {
            if !self.definitions.contains_key(reference) {
                self.cursor = *position;
                return Err(self.error(
                    "ebnf-unresolved-reference",
                    format!("undefined production: {reference}"),
                ));
            }
        }
        self.usage.collection_items = self.usage.collection_items.max(self.definitions.len());
        self.usage.string_bytes = max_token;
        self.usage.records_or_edges = self.definitions.len() + self.references.len();
        Ok(self.usage)
    }

    fn parse_expression(
        &mut self,
        depth: usize,
        terminators: &[EbnfToken],
    ) -> Result<(), ContractCompileError> {
        self.usage.depth = self.usage.depth.max(depth);
        let mut alternatives = 1usize;
        self.parse_sequence(depth, terminators)?;
        while self.tokens.get(self.cursor) == Some(&EbnfToken::Alternative) {
            alternatives += 1;
            self.cursor += 1;
            self.parse_sequence(depth, terminators)?;
        }
        self.usage.collection_items = self.usage.collection_items.max(alternatives);
        Ok(())
    }

    fn parse_sequence(
        &mut self,
        depth: usize,
        terminators: &[EbnfToken],
    ) -> Result<(), ContractCompileError> {
        let mut items = 0usize;
        let mut comma_pending = false;
        while let Some(token) = self.tokens.get(self.cursor) {
            if *token == EbnfToken::Alternative || terminators.contains(token) {
                break;
            }
            if *token == EbnfToken::Comma {
                if items == 0 || comma_pending {
                    return Err(self.error("ebnf-sequence", "misplaced sequence comma"));
                }
                comma_pending = true;
                self.cursor += 1;
                continue;
            }
            self.parse_primary(depth)?;
            items += 1;
            comma_pending = false;
        }
        if items == 0 || comma_pending {
            return Err(self.error("ebnf-empty-alternative", "empty sequence or alternative"));
        }
        self.usage.collection_items = self.usage.collection_items.max(items);
        Ok(())
    }

    fn parse_primary(&mut self, depth: usize) -> Result<(), ContractCompileError> {
        let position = self.cursor;
        let token = self
            .tokens
            .get(self.cursor)
            .cloned()
            .ok_or_else(|| self.error("ebnf-primary", "expected expression"))?;
        self.cursor += 1;
        match token {
            EbnfToken::Identifier(name) => self.references.push((name, position)),
            EbnfToken::Literal => {}
            EbnfToken::LeftParen | EbnfToken::LeftBracket | EbnfToken::LeftBrace => {
                let closing = match token {
                    EbnfToken::LeftParen => EbnfToken::RightParen,
                    EbnfToken::LeftBracket => EbnfToken::RightBracket,
                    EbnfToken::LeftBrace => EbnfToken::RightBrace,
                    _ => unreachable!(),
                };
                self.parse_expression(depth + 1, std::slice::from_ref(&closing))?;
                if self.tokens.get(self.cursor) != Some(&closing) {
                    return Err(self.error("ebnf-delimiter", "unbalanced group delimiter"));
                }
                self.cursor += 1;
            }
            EbnfToken::Equals
            | EbnfToken::Semicolon
            | EbnfToken::Alternative
            | EbnfToken::Comma
            | EbnfToken::RightParen
            | EbnfToken::RightBracket
            | EbnfToken::RightBrace => {
                return Err(self.error("ebnf-primary", "unexpected grammar punctuation"));
            }
        }
        self.usage.nodes += 1;
        Ok(())
    }
}

fn validate_ebnf(path: &Path, payload: &str) -> Result<ResourceUsage, ContractCompileError> {
    let (tokens, max_token) = tokenize_ebnf(path, payload)?;
    EbnfParser {
        path,
        tokens,
        cursor: 0,
        definitions: BTreeMap::new(),
        references: Vec::new(),
        usage: ResourceUsage::default(),
    }
    .parse(max_token)
}

fn canonical_ebnf(
    path: &Path,
    descriptor: &ArtifactDescriptor,
    bytes: &[u8],
) -> Result<(ArtifactHeader, Vec<u8>, ResourceUsage), ContractCompileError> {
    if bytes.contains(&b'\r') {
        let normalized = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
        if normalized.as_bytes().contains(&b'\r') {
            return Err(parse_error(
                "ebnf-line-ending",
                path,
                "$",
                "bare carriage return is not permitted",
            ));
        }
    }
    let normalized = String::from_utf8(bytes.to_vec())
        .map_err(|error| parse_error("utf8", path, "$", error))?
        .replace("\r\n", "\n");
    let (header, payload_start) = comment_header(path, normalized.as_bytes(), "(*", "*)")?;
    validate_header(path, "$header", &header, descriptor)?;
    let payload = normalized
        .get(payload_start..)
        .ok_or_else(|| parse_error("ebnf-structure", path, "$grammar", "invalid boundary"))?;
    let usage = validate_ebnf(path, payload)?;
    Ok((header, payload.as_bytes().to_vec(), usage))
}

fn canonical_proto(
    repository_root: &Path,
    path: &Path,
    descriptor: &ArtifactDescriptor,
    bytes: &[u8],
    catalog: &CompiledCatalog,
) -> Result<(ArtifactHeader, Vec<u8>, ResourceUsage), ContractCompileError> {
    let (header, payload_start) = comment_header(path, bytes, "//", "")?;
    validate_header(path, "$header", &header, descriptor)?;
    if descriptor.status == ArtifactStatus::Draft
        && descriptor.semantic_projection_source == SemanticProjectionSource::Native
    {
        let normalized = String::from_utf8(bytes.to_vec())
            .map_err(|error| parse_error("utf8", path, "$", error))?
            .replace("\r\n", "\n");
        let payload = normalized
            .get(payload_start..)
            .ok_or_else(|| parse_error("proto-source", path, "$", "invalid header boundary"))?
            .as_bytes()
            .to_vec();
        return Ok((header, payload, ResourceUsage::default()));
    }
    let SemanticProjectionSource::DerivationOutput { output } =
        &descriptor.semantic_projection_source
    else {
        return Err(parse_error(
            "descriptor-source",
            path,
            "$",
            "released Protobuf artifacts require a derivation-owned descriptor set",
        ));
    };
    let output_record = catalog.output(output).ok_or_else(|| {
        parse_error(
            "descriptor-source",
            path,
            "$",
            "semantic descriptor output is absent",
        )
    })?;
    let maximum = output_record
        .resource_budget_profile
        .as_deref()
        .and_then(|profile| catalog.budget(profile))
        .unwrap_or_else(|| {
            catalog
                .derivation(&output.derivation_id)
                .and_then(|unit| catalog.budget(&unit.resource_budget_profile))
                .expect("compiled derivation selects a resource budget")
        })
        .max_bytes;
    let descriptor_path = repository_root.join(&output.path);
    let descriptor_bytes = read_bounded(&descriptor_path, maximum)?;
    let descriptor_set = FileDescriptorSet::decode(descriptor_bytes.as_slice())
        .map_err(|error| parse_error("descriptor-set", &descriptor_path, "$", error))?;
    let raw_files = raw_messages(&descriptor_bytes, 1)
        .map_err(|message| parse_error("descriptor-set", &descriptor_path, "$", message))?;
    if raw_files.len() != descriptor_set.file.len() {
        return Err(parse_error(
            "descriptor-set",
            &descriptor_path,
            "$.files",
            "typed and raw file counts disagree",
        ));
    }
    let authority = descriptor.authority_path.to_string_lossy();
    let compiler_name = descriptor
        .authority_path
        .strip_prefix("tooling/proto/source")
        .unwrap_or(&descriptor.authority_path)
        .to_string_lossy();
    let (_, selected, raw_selected) = descriptor_set
        .file
        .iter()
        .zip(&raw_files)
        .enumerate()
        .find(|(_, (file, _))| {
            file.name
                .as_deref()
                .is_some_and(|name| name == authority || name == compiler_name)
        })
        .ok_or_else(|| {
            parse_error(
                "descriptor-set",
                &descriptor_path,
                "$.files",
                format!("descriptor is absent for {authority}"),
            )
        })
        .map(|(index, (file, raw))| (index, file, *raw))?;
    let selected = normalized_proto_file(selected, raw_selected)
        .map_err(|message| parse_error("descriptor-set", &descriptor_path, "$.files", message))?;
    let projection = json!({"files": [selected]});
    let mut usage = observe_value(&projection, 1);
    usage.records_or_edges = proto_records_or_edges(&projection);
    let canonical = canonicalize_value(&projection)
        .map_err(|error| parse_error(error.failure_class(), path, "$", error))?;
    Ok((header, canonical, usage))
}

fn read_proto_varint(bytes: &[u8], cursor: &mut usize) -> Result<u64, String> {
    let mut value = 0_u64;
    for shift in (0..70).step_by(7) {
        let byte = *bytes
            .get(*cursor)
            .ok_or_else(|| "truncated protobuf varint".to_owned())?;
        *cursor += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("protobuf varint exceeds 10 bytes".to_owned())
}

fn raw_messages(bytes: &[u8], wanted_tag: u32) -> Result<Vec<&[u8]>, String> {
    let mut cursor = 0;
    let mut matches = Vec::new();
    while cursor < bytes.len() {
        let key = read_proto_varint(bytes, &mut cursor)?;
        let tag = u32::try_from(key >> 3).map_err(|_| "protobuf tag overflow".to_owned())?;
        match key & 7 {
            0 => {
                let _ = read_proto_varint(bytes, &mut cursor)?;
            }
            1 => cursor = cursor.checked_add(8).ok_or("protobuf offset overflow")?,
            2 => {
                let length = usize::try_from(read_proto_varint(bytes, &mut cursor)?)
                    .map_err(|_| "protobuf length overflow".to_owned())?;
                let end = cursor
                    .checked_add(length)
                    .ok_or_else(|| "protobuf length overflow".to_owned())?;
                let payload = bytes
                    .get(cursor..end)
                    .ok_or_else(|| "truncated protobuf message".to_owned())?;
                if tag == wanted_tag {
                    matches.push(payload);
                }
                cursor = end;
            }
            5 => cursor = cursor.checked_add(4).ok_or("protobuf offset overflow")?,
            wire => return Err(format!("unsupported protobuf wire type {wire}")),
        }
        if cursor > bytes.len() {
            return Err("truncated protobuf field".to_owned());
        }
    }
    Ok(matches)
}

fn wire_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn normalized_options(raw_parent: &[u8], options_tag: u32) -> Result<Value, String> {
    let options = raw_messages(raw_parent, options_tag)?;
    match options.as_slice() {
        [] | [[]] => Ok(json!({})),
        [bytes] => Ok(json!({"$wire_hex": wire_hex(bytes)})),
        _ => Err(format!("duplicate options field {options_tag}")),
    }
}

fn proto_full_name(package: &str, parents: &[String], name: &str) -> String {
    std::iter::once(package)
        .chain(parents.iter().map(String::as_str))
        .chain(std::iter::once(name))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(".")
}

fn normalized_proto_enum(
    package: &str,
    parents: &[String],
    descriptor: &EnumDescriptorProto,
    raw: &[u8],
) -> Result<Value, String> {
    let raw_values = raw_messages(raw, 2)?;
    if raw_values.len() != descriptor.value.len() {
        return Err("typed and raw enum value counts disagree".to_owned());
    }
    let mut values = descriptor
        .value
        .iter()
        .zip(raw_values)
        .map(|(value, raw_value)| {
            Ok(json!({
                "name": value.name.as_deref().unwrap_or_default(),
                "number": value.number.unwrap_or_default(),
                "options": normalized_options(raw_value, 3)?,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    values.sort_by_key(|value| {
        (
            value["number"].as_i64().unwrap_or_default(),
            value["name"].as_str().unwrap_or_default().to_owned(),
        )
    });
    let mut reserved_names = descriptor.reserved_name.clone();
    reserved_names.sort();
    let mut reserved_ranges = descriptor
        .reserved_range
        .iter()
        .map(|range| {
            json!({
                "start": range.start.unwrap_or_default(),
                "end_inclusive": range.end.unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    reserved_ranges.sort_by_key(|range| {
        (
            range["start"].as_i64().unwrap_or_default(),
            range["end_inclusive"].as_i64().unwrap_or_default(),
        )
    });
    Ok(json!({
        "full_name": proto_full_name(package, parents, descriptor.name.as_deref().unwrap_or_default()),
        "options": normalized_options(raw, 3)?,
        "reserved_names": reserved_names,
        "reserved_ranges": reserved_ranges,
        "values": values,
    }))
}

#[allow(clippy::too_many_lines)] // Mirrors the closed DescriptorProto semantic record in one seam.
fn normalized_proto_message(
    package: &str,
    parents: &[String],
    syntax: &str,
    descriptor: &DescriptorProto,
    raw: &[u8],
) -> Result<(Vec<Value>, Vec<Value>), String> {
    let raw_fields = raw_messages(raw, 2)?;
    let raw_nested = raw_messages(raw, 3)?;
    let raw_enums = raw_messages(raw, 4)?;
    let raw_oneofs = raw_messages(raw, 8)?;
    if raw_fields.len() != descriptor.field.len()
        || raw_nested.len() != descriptor.nested_type.len()
        || raw_enums.len() != descriptor.enum_type.len()
        || raw_oneofs.len() != descriptor.oneof_decl.len()
    {
        return Err("typed and raw message child counts disagree".to_owned());
    }
    let oneof_names = descriptor
        .oneof_decl
        .iter()
        .map(|oneof| oneof.name.as_deref().unwrap_or_default())
        .collect::<Vec<_>>();
    let mut fields = descriptor
        .field
        .iter()
        .zip(raw_fields)
        .map(|(field, raw_field)| {
            let label = field
                .label
                .and_then(|value| prost_types::field_descriptor_proto::Label::try_from(value).ok())
                .map_or("LABEL_OPTIONAL", |value| value.as_str_name());
            let field_type = field
                .r#type
                .and_then(|value| prost_types::field_descriptor_proto::Type::try_from(value).ok())
                .map_or("TYPE_DOUBLE", |value| value.as_str_name());
            let oneof = field.oneof_index.and_then(|index| {
                usize::try_from(index)
                    .ok()
                    .and_then(|index| oneof_names.get(index).copied())
            });
            let has_presence = syntax != "proto3"
                || field.proto3_optional.unwrap_or(false)
                || field.oneof_index.is_some()
                || matches!(field_type, "TYPE_MESSAGE" | "TYPE_GROUP");
            Ok(json!({
                "name": field.name.as_deref().unwrap_or_default(),
                "number": field.number.unwrap_or_default(),
                "label": label,
                "type": field_type,
                "type_name": field.type_name.as_deref().unwrap_or_default(),
                "json_name": field.json_name.as_deref().unwrap_or_default(),
                "oneof": oneof,
                "proto3_optional": field.proto3_optional.unwrap_or(false),
                "has_presence": has_presence,
                "default_value": field.default_value,
                "options": normalized_options(raw_field, 8)?,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    fields.sort_by_key(|field| {
        (
            field["number"].as_i64().unwrap_or_default(),
            field["name"].as_str().unwrap_or_default().to_owned(),
        )
    });
    let oneofs = descriptor
        .oneof_decl
        .iter()
        .zip(raw_oneofs)
        .map(|(oneof, raw_oneof)| {
            Ok(json!({
                "name": oneof.name.as_deref().unwrap_or_default(),
                "options": normalized_options(raw_oneof, 2)?,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut reserved_names = descriptor.reserved_name.clone();
    reserved_names.sort();
    let mut reserved_ranges = descriptor
        .reserved_range
        .iter()
        .map(|range| {
            json!({
                "start": range.start.unwrap_or_default(),
                "end_exclusive": range.end.unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    reserved_ranges.sort_by_key(|range| {
        (
            range["start"].as_i64().unwrap_or_default(),
            range["end_exclusive"].as_i64().unwrap_or_default(),
        )
    });
    let mut extension_ranges = descriptor
        .extension_range
        .iter()
        .map(|range| {
            json!({
                "start": range.start.unwrap_or_default(),
                "end_exclusive": range.end.unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    extension_ranges.sort_by_key(|range| {
        (
            range["start"].as_i64().unwrap_or_default(),
            range["end_exclusive"].as_i64().unwrap_or_default(),
        )
    });
    let name = descriptor.name.as_deref().unwrap_or_default();
    let mut nested_parents = parents.to_vec();
    nested_parents.push(name.to_owned());
    let mut messages = vec![json!({
        "full_name": proto_full_name(package, parents, name),
        "fields": fields,
        "oneofs": oneofs,
        "options": normalized_options(raw, 7)?,
        "reserved_names": reserved_names,
        "reserved_ranges": reserved_ranges,
        "extension_ranges": extension_ranges,
    })];
    let mut enums = descriptor
        .enum_type
        .iter()
        .zip(raw_enums)
        .map(|(item, raw_item)| normalized_proto_enum(package, &nested_parents, item, raw_item))
        .collect::<Result<Vec<_>, _>>()?;
    for (nested, raw_nested) in descriptor.nested_type.iter().zip(raw_nested) {
        let (nested_messages, nested_enums) =
            normalized_proto_message(package, &nested_parents, syntax, nested, raw_nested)?;
        messages.extend(nested_messages);
        enums.extend(nested_enums);
    }
    Ok((messages, enums))
}

fn normalized_proto_file(descriptor: &FileDescriptorProto, raw: &[u8]) -> Result<Value, String> {
    let package = descriptor.package.as_deref().unwrap_or_default();
    let syntax = descriptor.syntax.as_deref().unwrap_or("proto2");
    let raw_messages_list = raw_messages(raw, 4)?;
    let raw_enums = raw_messages(raw, 5)?;
    let raw_services = raw_messages(raw, 6)?;
    if raw_messages_list.len() != descriptor.message_type.len()
        || raw_enums.len() != descriptor.enum_type.len()
        || raw_services.len() != descriptor.service.len()
    {
        return Err("typed and raw file child counts disagree".to_owned());
    }
    let mut messages = Vec::new();
    let mut enums = descriptor
        .enum_type
        .iter()
        .zip(raw_enums)
        .map(|(item, raw_item)| normalized_proto_enum(package, &[], item, raw_item))
        .collect::<Result<Vec<_>, _>>()?;
    for (message, raw_message) in descriptor.message_type.iter().zip(raw_messages_list) {
        let (nested_messages, nested_enums) =
            normalized_proto_message(package, &[], syntax, message, raw_message)?;
        messages.extend(nested_messages);
        enums.extend(nested_enums);
    }
    messages.sort_by_key(|message| message["full_name"].as_str().unwrap_or_default().to_owned());
    enums.sort_by_key(|item| item["full_name"].as_str().unwrap_or_default().to_owned());
    let mut services = descriptor
        .service
        .iter()
        .zip(raw_services)
        .map(|(service, raw_service)| {
            let raw_methods = raw_messages(raw_service, 2)?;
            if raw_methods.len() != service.method.len() {
                return Err("typed and raw service method counts disagree".to_owned());
            }
            let mut methods = service
                .method
                .iter()
                .zip(raw_methods)
                .map(|(method, raw_method)| {
                    Ok(json!({
                        "name": method.name.as_deref().unwrap_or_default(),
                        "input_type": method.input_type.as_deref().unwrap_or_default(),
                        "output_type": method.output_type.as_deref().unwrap_or_default(),
                        "client_streaming": method.client_streaming.unwrap_or(false),
                        "server_streaming": method.server_streaming.unwrap_or(false),
                        "options": normalized_options(raw_method, 4)?,
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?;
            methods.sort_by_key(|method| method["name"].as_str().unwrap_or_default().to_owned());
            Ok(json!({
                "full_name": proto_full_name(package, &[], service.name.as_deref().unwrap_or_default()),
                "options": normalized_options(raw_service, 3)?,
                "methods": methods,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    services.sort_by_key(|service| service["full_name"].as_str().unwrap_or_default().to_owned());
    let mut dependencies = descriptor.dependency.clone();
    dependencies.sort();
    let dependency = |index: &i32| {
        usize::try_from(*index)
            .ok()
            .and_then(|index| descriptor.dependency.get(index))
            .cloned()
            .ok_or_else(|| "descriptor dependency index is out of range".to_owned())
    };
    let mut public_dependencies = descriptor
        .public_dependency
        .iter()
        .map(dependency)
        .collect::<Result<Vec<_>, _>>()?;
    public_dependencies.sort();
    let mut weak_dependencies = descriptor
        .weak_dependency
        .iter()
        .map(dependency)
        .collect::<Result<Vec<_>, _>>()?;
    weak_dependencies.sort();
    Ok(json!({
        "name": descriptor.name.as_deref().unwrap_or_default(),
        "package": package,
        "syntax": syntax,
        "edition": Value::Null,
        "dependencies": dependencies,
        "public_dependencies": public_dependencies,
        "weak_dependencies": weak_dependencies,
        "options": normalized_options(raw, 8)?,
        "messages": messages,
        "enums": enums,
        "services": services,
    }))
}

fn proto_records_or_edges(value: &Value) -> usize {
    const REPEATED_DESCRIPTOR_FIELDS: &[&str] = &[
        "dependencies",
        "enums",
        "extension_ranges",
        "fields",
        "messages",
        "methods",
        "oneofs",
        "public_dependencies",
        "reserved_names",
        "reserved_ranges",
        "services",
        "values",
        "weak_dependencies",
    ];
    match value {
        Value::Array(values) => values.iter().map(proto_records_or_edges).sum(),
        Value::Object(fields) => fields
            .iter()
            .map(|(name, value)| {
                let records = if REPEATED_DESCRIPTOR_FIELDS.contains(&name.as_str()) {
                    value.as_array().map_or(0, Vec::len)
                } else {
                    0
                };
                records + proto_records_or_edges(value)
            })
            .sum(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
    }
}

/// Compile one catalog descriptor through its bounded native adapter.
///
/// # Errors
///
/// Returns a stable typed error for I/O, parse, header, digest, descriptor, or resource
/// failures. Diagnostic payloads are capped and never contain complete source input.
fn compile_artifact_inner(
    repository_root: &Path,
    catalog: &CompiledCatalog,
    descriptor: &ArtifactDescriptor,
    verify_embedded_digest: bool,
) -> Result<CompiledArtifact, ContractCompileError> {
    let path = repository_root.join(&descriptor.authority_path);
    let budget = catalog
        .budget(&descriptor.resource_budget_profile)
        .expect("compiled descriptors reference an existing budget");
    let bytes = read_bounded(&path, budget.max_bytes)?;
    let source_digest = checksum(&bytes);

    let (header, canonical_bytes, bundle_digest, embedded_bundle_digest, mut usage) =
        match descriptor.native_format {
            NativeFormat::Markdown => {
                if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
                    return Err(parse_error(
                        "utf8-bom",
                        &path,
                        "$",
                        "UTF-8 BOM is forbidden",
                    ));
                }
                std::str::from_utf8(&bytes)
                    .map_err(|error| parse_error("utf8", &path, "$", error))?;
                let usage = ResourceUsage {
                    depth: 1,
                    nodes: 1,
                    collection_items: 1,
                    string_bytes: bytes.len(),
                    records_or_edges: 1,
                    ..ResourceUsage::default()
                };
                (None, bytes.clone(), None, None, usage)
            }
            NativeFormat::Json => {
                let (header, canonical, bundle, embedded_bundle, usage) =
                    canonical_json(repository_root, &path, descriptor, catalog, &bytes)?;
                (Some(header), canonical, bundle, embedded_bundle, usage)
            }
            NativeFormat::Yaml => {
                let (header, canonical, usage) = canonical_yaml(&path, descriptor, &bytes)?;
                (Some(header), canonical, None, None, usage)
            }
            NativeFormat::Jsonl => {
                let (header, canonical, usage) = canonical_jsonl(&path, descriptor, &bytes)?;
                (Some(header), canonical, None, None, usage)
            }
            NativeFormat::Proto => {
                let (header, canonical, usage) =
                    canonical_proto(repository_root, &path, descriptor, &bytes, catalog)?;
                (Some(header), canonical, None, None, usage)
            }
            NativeFormat::Ebnf => {
                let (header, canonical, usage) = canonical_ebnf(&path, descriptor, &bytes)?;
                (Some(header), canonical, None, None, usage)
            }
        };
    usage.bytes = bytes.len();
    enforce_usage(&path, usage, budget)?;
    let canonical_digest = checksum(&canonical_bytes);
    if verify_embedded_digest {
        if let Some(computed) = &bundle_digest {
            let claimed = embedded_bundle_digest.as_deref().ok_or_else(|| {
                parse_error(
                    "bundle-digest",
                    &path,
                    "$.bundle_digest",
                    "bundle identity is absent",
                )
            })?;
            super::jcs::validate_checksum(claimed).map_err(|error| {
                parse_error(error.failure_class(), &path, "$.bundle_digest", error)
            })?;
            verify_claim(&path, claimed, computed)?;
        }
        if let Some(header) = &header {
            verify_claim(&path, &header.canonical_digest, &canonical_digest)?;
        }
    }
    Ok(CompiledArtifact {
        artifact_id: descriptor.artifact_id.clone(),
        digest_projection: descriptor.digest_projection,
        source_digest,
        embedded_canonical_digest: header.map(|value| value.canonical_digest),
        canonical_digest,
        canonical_bytes,
        bundle_digest,
        embedded_bundle_digest,
        usage,
    })
}

/// Compile one artifact while allowing the generator to replace a stale embedded digest.
pub(crate) fn compile_artifact_for_generation(
    repository_root: &Path,
    catalog: &CompiledCatalog,
    descriptor: &ArtifactDescriptor,
) -> Result<CompiledArtifact, ContractCompileError> {
    compile_artifact_inner(repository_root, catalog, descriptor, false)
}

/// Compile one catalog descriptor and verify its embedded semantic identity.
///
/// # Errors
///
/// Returns a stable typed error for I/O, parse, header, digest, descriptor, or resource
/// failures. Diagnostic payloads are capped and never contain complete source input.
pub fn compile_artifact(
    repository_root: &Path,
    catalog: &CompiledCatalog,
    descriptor: &ArtifactDescriptor,
) -> Result<CompiledArtifact, ContractCompileError> {
    compile_artifact_inner(repository_root, catalog, descriptor, true)
}

/// Replay one native parser boundary without requiring a catalog or filesystem fixture.
///
/// This is the shared entry point for bounded fuzzing. It intentionally stops before
/// artifact-specific header/record validation, while exercising the exact strict decoders,
/// YAML-subset scanner, EBNF parser, and semantic resource observer used by compilation.
///
/// # Errors
///
/// Returns the same bounded parse and resource-limit classes as catalog compilation.
pub fn replay_bounded_ingress(
    format: NativeFormat,
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<ResourceUsage, ContractCompileError> {
    let path = Path::new("<fuzz-input>");
    if bytes.len() > maximum_bytes {
        return Err(limit_error(
            path,
            "$",
            "max_bytes",
            maximum_bytes.saturating_add(1),
            maximum_bytes,
        ));
    }
    let mut usage = match format {
        NativeFormat::Markdown => {
            std::str::from_utf8(bytes).map_err(|error| parse_error("utf8", path, "$", error))?;
            ResourceUsage {
                depth: 1,
                nodes: 1,
                collection_items: 1,
                string_bytes: bytes.len(),
                records_or_edges: 1,
                ..ResourceUsage::default()
            }
        }
        NativeFormat::Json => observe_value(&strict_json(path, "$", bytes)?, 1),
        NativeFormat::Jsonl => {
            if !bytes.ends_with(b"\n") {
                return Err(parse_error(
                    "jsonl-framing",
                    path,
                    "$",
                    "JSON Lines input must end with LF",
                ));
            }
            let mut usage = ResourceUsage::default();
            for (index, line) in bytes[..bytes.len() - 1]
                .split(|byte| *byte == b'\n')
                .enumerate()
            {
                if line.is_empty() {
                    return Err(parse_error(
                        "jsonl-framing",
                        path,
                        format!("record[{}]", index + 1),
                        "blank records are not permitted",
                    ));
                }
                usage.merge(observe_value(
                    &strict_json(path, &format!("record[{}]", index + 1), line)?,
                    1,
                ));
                usage.records_or_edges += 1;
            }
            usage
        }
        NativeFormat::Yaml => {
            let indicators = scan_yaml_subset(bytes);
            let aliases = indicators.anchors + indicators.aliases;
            if aliases > 0 {
                return Err(limit_error(path, "$", "max_aliases", aliases, 0));
            }
            if indicators.tags > 0 || indicators.merge_keys > 0 {
                return Err(parse_error(
                    "yaml-subset",
                    path,
                    "$",
                    "tags and merge keys are outside the accepted subset",
                ));
            }
            let yaml: YamlValue = serde_yaml_ng::from_slice(bytes)
                .map_err(|error| parse_error("yaml-parse", path, "$", error))?;
            observe_value(&yaml_to_json(path, "$", yaml)?, 1)
        }
        NativeFormat::Proto => {
            let (_, offset) = comment_header(path, bytes, "//", "")?;
            ResourceUsage {
                depth: 1,
                nodes: 1,
                collection_items: 1,
                string_bytes: offset,
                records_or_edges: 1,
                ..ResourceUsage::default()
            }
        }
        NativeFormat::Ebnf => {
            let source = std::str::from_utf8(bytes)
                .map_err(|error| parse_error("utf8", path, "$", error))?;
            validate_ebnf(path, &source.replace("\r\n", "\n"))?
        }
    };
    usage.bytes = bytes.len();
    Ok(usage)
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;
    use crate::contracts::catalog::{
        ArtifactStatus, CompatibilityFamily, ConsumerDomain, ContractOwner, ProvenanceRequirement,
    };

    fn descriptor(
        format: NativeFormat,
        kind: ArtifactKind,
        profile: DigestProjection,
    ) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_id: "codefabric.test".to_owned(),
            authority_path: PathBuf::from("contracts/test"),
            artifact_kind: kind,
            native_format: format,
            owner: ContractOwner::Suite,
            version: "1.0".to_owned(),
            compatible_suite_major: 1,
            status: ArtifactStatus::Draft,
            digest_projection: profile,
            compatibility_family: CompatibilityFamily::Suite,
            resource_budget_profile: "test".to_owned(),
            parser_schema_authority: None,
            semantic_projection_source: SemanticProjectionSource::Native,
            consumers: BTreeSet::from([ConsumerDomain::ContractTooling]),
            provenance_requirements: BTreeSet::from([
                ProvenanceRequirement::CanonicalDigest,
                ProvenanceRequirement::SourceDigest,
            ]),
        }
    }

    #[test]
    fn bundle_projection_uses_the_closed_sorted_model_and_retains_member_identity() {
        let mut descriptor = descriptor(
            NativeFormat::Json,
            ArtifactKind::BundleManifest,
            DigestProjection::BundleAcG07V1,
        );
        descriptor.artifact_id = "codefabric.bundles.schema-bundle".to_owned();
        let source = br#"{
            "artifact_id":"codefabric.bundles.schema-bundle","artifact_kind":"bundle-manifest",
            "version":"1.0","compatible_suite_major":1,"status":"draft",
            "canonical_digest":"b3:1111111111111111111111111111111111111111111111111111111111111111",
            "bundle_kind":"schema","bundle_version":"1.0","bundle_major":1,
            "artifacts":[
                {"artifact_id":"z","version":"1.0","canonical_digest":"b3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","required":false,"feature_bits":[]},
                {"artifact_id":"a","version":"1.0","canonical_digest":"b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","required":true,"feature_bits":["z","a"]}
            ],
            "compatibility":{"minimum_consumer_minor":0,"maximum_consumer_minor":1},
            "created_by":{"generator_id":"codefabric-contracts","generator_version":"1.0"},
            "bundle_digest":"b3:2222222222222222222222222222222222222222222222222222222222222222",
            "signature":"test-signature"
        }"#;
        let value = strict_json(Path::new("test.json"), "$", source).unwrap();
        let (_, canonical, bundle_digest, embedded_bundle_digest, _) = canonical_bundle_json(
            Path::new("test.json"),
            &descriptor,
            value,
            ResourceUsage::default(),
        )
        .unwrap();
        let mut projected = decode_strict(&canonical).unwrap();
        assert!(projected.get("canonical_digest").is_none());
        assert_eq!(projected["artifacts"][0]["artifact_id"], "a");
        assert_eq!(
            projected["artifacts"][0]["canonical_digest"],
            "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(projected["artifacts"][0]["feature_bits"], json!(["a", "z"]));
        assert_eq!(projected["signature"], "test-signature");
        assert_eq!(
            embedded_bundle_digest.as_deref(),
            Some("b3:2222222222222222222222222222222222222222222222222222222222222222")
        );
        let mut reordered = strict_json(Path::new("test.json"), "$", source).unwrap();
        reordered["artifacts"].as_array_mut().unwrap().reverse();
        let (_, reordered_canonical, reordered_bundle_digest, _, _) = canonical_bundle_json(
            Path::new("test.json"),
            &descriptor,
            reordered,
            ResourceUsage::default(),
        )
        .unwrap();
        assert_eq!(canonical, reordered_canonical);
        assert_eq!(bundle_digest, reordered_bundle_digest);

        let mut semantic_mutation = strict_json(Path::new("test.json"), "$", source).unwrap();
        semantic_mutation["artifacts"][0]["canonical_digest"] =
            "b3:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".into();
        let (_, mutated_canonical, mutated_bundle_digest, _, _) = canonical_bundle_json(
            Path::new("test.json"),
            &descriptor,
            semantic_mutation,
            ResourceUsage::default(),
        )
        .unwrap();
        assert_ne!(canonical, mutated_canonical);
        assert_ne!(bundle_digest, mutated_bundle_digest);

        let object = projected.as_object_mut().unwrap();
        object.remove("bundle_digest");
        object.remove("signature");
        assert_eq!(
            bundle_digest,
            Some(checksum(&canonicalize_value(&projected).unwrap()))
        );
    }

    #[test]
    fn bundle_model_rejects_unknown_missing_and_mistyped_fields() {
        let source = json!({
            "artifact_id":"codefabric.bundles.schema-bundle",
            "artifact_kind":"bundle-manifest",
            "version":"1.0",
            "compatible_suite_major":1,
            "status":"draft",
            "canonical_digest":"b3:1111111111111111111111111111111111111111111111111111111111111111",
            "bundle_kind":"schema",
            "bundle_version":"1.0",
            "bundle_major":1,
            "artifacts":[{
                "artifact_id":"nested",
                "version":"1.0",
                "canonical_digest":"b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "required":true,
                "feature_bits":[]
            }],
            "compatibility":{"minimum_consumer_minor":0,"maximum_consumer_minor":1},
            "created_by":{"generator_id":"codefabric-contracts","generator_version":"1.0"},
            "bundle_digest":"b3:2222222222222222222222222222222222222222222222222222222222222222"
        });

        let mut unknown_root = source.clone();
        unknown_root["unexpected"] = true.into();
        assert!(matches!(
            typed::<BundleDocument>(Path::new("test.json"), "$", unknown_root),
            Err(ContractCompileError::Parse {
                class: "typed-record",
                ..
            })
        ));

        let mut missing_root = source.clone();
        missing_root.as_object_mut().unwrap().remove("created_by");
        assert!(matches!(
            typed::<BundleDocument>(Path::new("test.json"), "$", missing_root),
            Err(ContractCompileError::Parse {
                class: "typed-record",
                ..
            })
        ));

        let mut unknown_member = source.clone();
        unknown_member["artifacts"][0]["unexpected"] = true.into();
        assert!(matches!(
            typed::<BundleDocument>(Path::new("test.json"), "$", unknown_member),
            Err(ContractCompileError::Parse { class: "typed-record", data_path, .. })
                if data_path.starts_with("$.artifacts[0]")
        ));

        let mut mistyped_member = source;
        mistyped_member["artifacts"][0]["required"] = "yes".into();
        assert!(matches!(
            typed::<BundleDocument>(Path::new("test.json"), "$", mistyped_member),
            Err(ContractCompileError::Parse { class: "typed-record", data_path, .. })
                if data_path == "$.artifacts[0].required"
        ));
    }

    #[test]
    fn yaml_alias_scan_ignores_quotes_and_comments_but_rejects_alias_tokens() {
        assert_eq!(
            scan_yaml_subset(
                b"value: '*literal'\nother: \"&literal !tag\"\n# *comment\ntext: |\n  *literal\n"
            ),
            YamlIndicators::default()
        );
        assert_eq!(
            scan_yaml_subset(b"base: &base [1]\ncopy: *base\n").anchors,
            1
        );
        assert_eq!(
            scan_yaml_subset(b"base: &base [1]\ncopy: *base\n").aliases,
            1
        );
        assert_eq!(scan_yaml_subset(b"tagged: !thing value\n").tags, 1);
        assert_eq!(scan_yaml_subset(b"<<: value\n").merge_keys, 1);
    }

    #[test]
    fn ebnf_validator_rejects_unbounded_or_malformed_structure() {
        assert!(validate_ebnf(Path::new("test.ebnf"), "document = \"\";\n").is_ok());
        assert!(validate_ebnf(Path::new("test.ebnf"), "document \"\";\n").is_err());
        assert!(validate_ebnf(Path::new("test.ebnf"), "(* open\n").is_err());
        assert!(validate_ebnf(Path::new("test.ebnf"), "a = b;\n").is_err());
        assert!(validate_ebnf(Path::new("test.ebnf"), "a = \"x\"; a = \"y\";\n").is_err());
        assert!(validate_ebnf(Path::new("test.ebnf"), "a = \"x\" | ;\n").is_err());
        assert!(validate_ebnf(Path::new("test.ebnf"), "a = [\"x\";\n").is_err());
        assert!(validate_ebnf(Path::new("test.ebnf"), "a = [b] {\"x\"}; b = \"y\";\n").is_ok());
    }

    #[test]
    fn diagnostic_payloads_are_bounded() {
        let error = parse_error("test", Path::new("test"), "$", "x".repeat(4096));
        let ContractCompileError::Parse { message, .. } = error else {
            panic!("expected parse error")
        };
        assert_eq!(message.len(), MAX_DIAGNOSTIC_BYTES);
    }

    fn edge_budget(maximum: usize) -> ResourceBudgetProfile {
        ResourceBudgetProfile {
            profile_id: "edge".to_owned(),
            max_bytes: maximum,
            max_depth: maximum,
            max_nodes: maximum,
            max_collection_items: maximum,
            max_string_bytes: maximum,
            max_records_or_edges: maximum,
            max_aliases: maximum,
            max_diagnostics: maximum,
        }
    }

    #[test]
    fn every_resource_limit_is_inclusive_and_reports_just_over() {
        let path = Path::new("edge");
        let budget = edge_budget(2);
        let at_limit = ResourceUsage {
            depth: 2,
            nodes: 2,
            collection_items: 2,
            string_bytes: 2,
            records_or_edges: 2,
            aliases: 2,
            diagnostics: 2,
            ..ResourceUsage::default()
        };
        enforce_usage(path, at_limit, &budget).unwrap();

        for (expected, usage) in [
            (
                "max_depth",
                ResourceUsage {
                    depth: 3,
                    ..ResourceUsage::default()
                },
            ),
            (
                "max_nodes",
                ResourceUsage {
                    nodes: 3,
                    ..ResourceUsage::default()
                },
            ),
            (
                "max_collection_items",
                ResourceUsage {
                    collection_items: 3,
                    ..ResourceUsage::default()
                },
            ),
            (
                "max_string_bytes",
                ResourceUsage {
                    string_bytes: 3,
                    ..ResourceUsage::default()
                },
            ),
            (
                "max_records_or_edges",
                ResourceUsage {
                    records_or_edges: 3,
                    ..ResourceUsage::default()
                },
            ),
            (
                "max_aliases",
                ResourceUsage {
                    aliases: 3,
                    ..ResourceUsage::default()
                },
            ),
            (
                "max_diagnostics",
                ResourceUsage {
                    diagnostics: 3,
                    ..ResourceUsage::default()
                },
            ),
        ] {
            let error = enforce_usage(path, usage, &budget).unwrap_err();
            assert!(matches!(
                error,
                ContractCompileError::Limit {
                    limit,
                    observed: 3,
                    maximum: 2,
                    ..
                } if limit == expected
            ));
        }
    }

    #[test]
    fn byte_limit_uses_a_limit_plus_one_sentinel() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"abc").unwrap();
        assert_eq!(read_bounded(file.path(), 3).unwrap(), b"abc");
        assert!(matches!(
            read_bounded(file.path(), 2).unwrap_err(),
            ContractCompileError::Limit {
                limit: "max_bytes",
                observed: 3,
                maximum: 2,
                ..
            }
        ));
    }

    #[test]
    fn governed_sources_fit_their_named_resource_profiles() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let catalog = super::super::catalog::ContractCatalog::load(root).unwrap();
        for descriptor in catalog.artifacts() {
            let compiled = compile_artifact(root, &catalog, descriptor).unwrap();
            eprintln!(
                "{}\t{}\t{:?}",
                descriptor.resource_budget_profile, descriptor.artifact_id, compiled.usage
            );
        }
    }

    #[test]
    fn normative_projection_vectors_have_exact_blake3_identities() {
        let corpus: Value = serde_json::from_str(include_str!(
            "../../contracts/fixtures/projections/vectors.json"
        ))
        .unwrap();
        for vector in corpus["vectors"].as_array().unwrap() {
            let source = vector["source_utf8"].as_str().unwrap().as_bytes();
            let canonical = vector["canonical_utf8"].as_str().unwrap().as_bytes();
            assert_eq!(checksum(source), vector["source_digest"]);
            assert_eq!(checksum(canonical), vector["canonical_digest"]);
            if let Some(identity) = vector["bundle_identity_utf8"].as_str() {
                assert_eq!(checksum(identity.as_bytes()), vector["bundle_digest"]);
            }
        }
    }
}
