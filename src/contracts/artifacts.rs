//! Contract-tree generation and verification for the Wave 1 machine authority.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Number, Value, json};
use serde_yaml_ng::Value as YamlValue;
use tempfile::NamedTempFile;
use thiserror::Error;

use super::jcs::{
    CanonicalJsonError, PROFILE, canonicalize_slice, canonicalize_value, checksum,
    non_string_map_records, validate_bytes, validate_checksum, validate_int64,
    validate_lowercase_public, validate_uint64,
};

/// Exact AC-G-05 machine-source files introduced across Wave 1.
pub const REQUIRED_SOURCE_ARTIFACTS: &[&str] = &[
    "manifests/suite-manifest.json",
    "manifests/deployment-profile.schema.json",
    "manifests/requirements.jsonl",
    "manifests/traceability.jsonl",
    "registry/enum-registry.yaml",
    "registry/flag-registry.yaml",
    "registry/ontology-entity-registry.yaml",
    "registry/ontology-relation-registry.yaml",
    "registry/ontology-property-registry.yaml",
    "registry/unknown-registry.yaml",
    "registry/projection-registry.yaml",
    "registry/summary-registry.yaml",
    "registry/capability-registry.yaml",
    "registry/error-registry.yaml",
    "registry/provider-registry.yaml",
    "registry/derivation-registry.yaml",
    "registry/phrase-registry.yaml",
    "registry/model-pack.schema.json",
    "identity/cbef-v1.yaml",
    "identity/type-algebra-v1.yaml",
    "identity/path-canonicalization-v1.yaml",
    "schema/analysis-context.schema.json",
    "schema/serving-snapshot.schema.json",
    "schema/public-snapshot-metadata.schema.json",
    "schema/source-context.schema.json",
    "schema/cpg-semantic-query-request.schema.json",
    "schema/cpg-semantic-query-response.schema.json",
    "schema/public-status.schema.json",
    "query/english-controlled-v1.ebnf",
    "query/planspec.schema.json",
    "rpc/cpg_query_service.proto",
    "rpc/provider_control.proto",
    "rpc/pyrefly_sidecar.proto",
    "rpc/rustc_extractor.proto",
    "rpc/feature-registry.yaml",
    "adapter/fastmcp-input.schema.json",
    "adapter/fastmcp-output.schema.json",
    "adapter/fastmcp-public-meta.schema.json",
    "bundles/ontology-bundle.json",
    "bundles/schema-bundle.json",
    "bundles/provider-bundle.json",
    "bundles/derivation-bundle.json",
    "bundles/query-language-bundle.json",
    "bundles/tool-contract-bundle.json",
    "bundles/toolchain-bundle.json",
    "bundles/model-pack-bundle.json",
    "deployment/local-workstation-v1.yaml",
    "faults/fault-point-registry.yaml",
    "comparison/comparison-ignore-registry.yaml",
    "security/security-corpus-manifest.yaml",
];

const REGISTRY_SOURCES: &[&str] = &[
    "enum-registry.yaml",
    "flag-registry.yaml",
    "ontology-entity-registry.yaml",
    "ontology-relation-registry.yaml",
    "ontology-property-registry.yaml",
    "unknown-registry.yaml",
    "projection-registry.yaml",
    "summary-registry.yaml",
    "capability-registry.yaml",
    "error-registry.yaml",
    "provider-registry.yaml",
    "derivation-registry.yaml",
    "phrase-registry.yaml",
];

const GENERATED_INDEX: &str = "contracts/generated/artifact-index.json";
const GENERATED_RUST: &str = "src/generated/contracts.rs";
const GENERATED_PYTHON: &str =
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/generated/_contract_index.py";
const GENERATED_PYTHON_STUB: &str =
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/generated/_contract_index.pyi";
const GENERATED_PYTHON_INIT: &str =
    "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/generated/__init__.py";

/// Verifier strictness profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationProfile {
    /// Validate drafts and report warnings without failing solely on draft status.
    Full,
    /// Require every artifact to be released and every warning to be resolved.
    Released,
}

impl VerificationProfile {
    /// Parse the stable CLI profile spelling.
    ///
    /// # Errors
    ///
    /// Returns an error for a profile other than full or released.
    pub fn parse(value: &str) -> Result<Self, ContractArtifactError> {
        match value {
            "full" => Ok(Self::Full),
            "released" => Ok(Self::Released),
            _ => Err(ContractArtifactError::UnknownProfile(value.to_owned())),
        }
    }
}

/// Successful verification evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationReport {
    /// Number of required source artifacts checked.
    pub artifact_count: usize,
    /// Number of draft-status warnings.
    pub warning_count: usize,
}

/// Contract generation or verification failure.
#[derive(Debug, Error)]
pub enum ContractArtifactError {
    /// A required path is absent.
    #[error("required contract path is absent: {0}")]
    Missing(PathBuf),
    /// A source artifact lacks its AC-G-02 metadata markers.
    #[error("artifact metadata is incomplete: {0}")]
    Metadata(PathBuf),
    /// A generated output differs from the deterministic rendering.
    #[error("generated contract output drifted: {0}")]
    Drift(PathBuf),
    /// Filesystem access failed.
    #[error("contract filesystem operation failed for {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// JSON or canonicalization rejected an artifact.
    #[error("canonical artifact failure for {path}: {source}")]
    Canonical {
        /// Affected path.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: CanonicalJsonError,
    },
    /// YAML source could not be converted into the JSON value domain.
    #[error("registry YAML failure for {path}: {message}")]
    Yaml {
        /// Affected path.
        path: PathBuf,
        /// Parser or conversion message.
        message: String,
    },
    /// A fixture document has the wrong shape.
    #[error("invalid verification fixture {path}: {message}")]
    Fixture {
        /// Affected path.
        path: PathBuf,
        /// Shape or expectation error.
        message: String,
    },
    /// Requirement and traceability records are orphaned or malformed.
    #[error("traceability failure for {path}: {message}")]
    Traceability {
        /// Affected manifest path.
        path: PathBuf,
        /// Failed structural obligation.
        message: String,
    },
    /// The released profile encountered unresolved warnings.
    #[error("released profile has {0} unresolved draft artifact warnings")]
    ReleasedWarnings(usize),
    /// The requested verifier profile is unknown.
    #[error("unknown verification profile: {0}")]
    UnknownProfile(String),
}

fn read(path: &Path) -> Result<Vec<u8>, ContractArtifactError> {
    fs::read(path).map_err(|source| ContractArtifactError::Io {
        path: path.to_owned(),
        source,
    })
}

fn yaml_failure(path: &Path, message: impl Into<String>) -> ContractArtifactError {
    ContractArtifactError::Yaml {
        path: path.to_owned(),
        message: message.into(),
    }
}

fn fixture_failure(path: &Path, message: impl Into<String>) -> ContractArtifactError {
    ContractArtifactError::Fixture {
        path: path.to_owned(),
        message: message.into(),
    }
}

fn yaml_number_to_json(
    path: &Path,
    logical_path: &str,
    number: &serde_yaml_ng::Number,
) -> Result<Value, ContractArtifactError> {
    if let Some(value) = number.as_i64() {
        return Ok(Value::Number(value.into()));
    }
    if let Some(value) = number.as_u64() {
        return Ok(Value::Number(value.into()));
    }
    let value = number.as_f64().ok_or_else(|| {
        yaml_failure(
            path,
            format!("unsupported numeric value at {logical_path}: {number:?}"),
        )
    })?;
    Number::from_f64(value).map(Value::Number).ok_or_else(|| {
        yaml_failure(
            path,
            format!("non-finite numeric value at {logical_path}: {number:?}"),
        )
    })
}

fn yaml_to_json(
    path: &Path,
    logical_path: &str,
    value: YamlValue,
) -> Result<Value, ContractArtifactError> {
    match value {
        YamlValue::Null => Ok(Value::Null),
        YamlValue::Bool(value) => Ok(Value::Bool(value)),
        YamlValue::Number(number) => yaml_number_to_json(path, logical_path, &number),
        YamlValue::String(value) => Ok(Value::String(value)),
        YamlValue::Sequence(values) => values
            .into_iter()
            .enumerate()
            .map(|(index, value)| yaml_to_json(path, &format!("{logical_path}[{index}]"), value))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        YamlValue::Mapping(mapping) => {
            let mut projected = Vec::with_capacity(mapping.len());
            let mut all_string_keys = true;
            for (index, (key, value)) in mapping.into_iter().enumerate() {
                if matches!(&key, YamlValue::String(key) if key == "<<") {
                    return Err(yaml_failure(
                        path,
                        format!("YAML merge keys are not supported at {logical_path}"),
                    ));
                }
                all_string_keys &= matches!(&key, YamlValue::String(_));
                let projected_key =
                    yaml_to_json(path, &format!("{logical_path}.key[{index}]"), key)?;
                let projected_value =
                    yaml_to_json(path, &format!("{logical_path}.value[{index}]"), value)?;
                projected.push((projected_key, projected_value));
            }

            if all_string_keys {
                let mut object = Map::with_capacity(projected.len());
                for (key, value) in projected {
                    let Value::String(key) = key else {
                        unreachable!("all YAML mapping keys were checked as strings");
                    };
                    object.insert(key, value);
                }
                Ok(Value::Object(object))
            } else {
                non_string_map_records(projected).map_err(|source| {
                    ContractArtifactError::Canonical {
                        path: path.to_owned(),
                        source,
                    }
                })
            }
        }
        YamlValue::Tagged(tagged) => Err(yaml_failure(
            path,
            format!("YAML tag {} is not supported at {logical_path}", tagged.tag),
        )),
    }
}

fn yaml_json_value(path: &Path, bytes: &[u8]) -> Result<Value, ContractArtifactError> {
    // `from_slice` on the pinned parser rejects duplicate mapping keys, excess
    // nesting, and streams containing more than one document. Aliases are
    // resolved into this semantic model; tags and merge keys remain explicit
    // and are rejected by `yaml_to_json`.
    let yaml: YamlValue =
        serde_yaml_ng::from_slice(bytes).map_err(|error| yaml_failure(path, error.to_string()))?;
    yaml_to_json(path, "$", yaml)
}

fn canonical_source_bytes(path: &Path, bytes: &[u8]) -> Result<Vec<u8>, ContractArtifactError> {
    match path.extension().and_then(OsStr::to_str) {
        Some("json") => {
            canonicalize_slice(bytes).map_err(|source| ContractArtifactError::Canonical {
                path: path.to_owned(),
                source,
            })
        }
        Some("jsonl") => {
            let mut canonical = Vec::new();
            for line in bytes
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
            {
                canonical.extend(canonicalize_slice(line).map_err(|source| {
                    ContractArtifactError::Canonical {
                        path: path.to_owned(),
                        source,
                    }
                })?);
                canonical.push(b'\n');
            }
            Ok(canonical)
        }
        Some("yaml" | "yml") => {
            let value = yaml_json_value(path, bytes)?;
            canonicalize_value(&value).map_err(|source| ContractArtifactError::Canonical {
                path: path.to_owned(),
                source,
            })
        }
        _ => Ok(bytes.to_vec()),
    }
}

fn generated_registry_value(source: &str, source_bytes: &[u8], value: &Value) -> Value {
    json!({
        "_generated": {
            "generator_revision": "codefabric-contracts-wp06-v1",
            "profile": PROFILE,
            "source": source,
            "source_digest": checksum(source_bytes),
        },
        "value": value,
    })
}

fn collect_artifact_records(
    contracts_root: &Path,
) -> Result<Vec<(String, String)>, ContractArtifactError> {
    let mut records = Vec::with_capacity(REQUIRED_SOURCE_ARTIFACTS.len());
    for relative in REQUIRED_SOURCE_ARTIFACTS {
        let path = contracts_root.join(relative);
        let bytes = read(&path)?;
        let canonical = canonical_source_bytes(&path, &bytes)?;
        records.push(((*relative).to_owned(), checksum(&canonical)));
    }
    Ok(records)
}

fn render_registry_outputs(
    contracts_root: &Path,
    outputs: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), ContractArtifactError> {
    for source_name in REGISTRY_SOURCES {
        let relative = format!("registry/{source_name}");
        let path = contracts_root.join(&relative);
        let bytes = read(&path)?;
        let value = yaml_json_value(&path, &bytes)?;
        let canonical_source = canonical_source_bytes(&path, &bytes)?;
        let generated = generated_registry_value(&relative, &canonical_source, &value);
        let encoded =
            canonicalize_value(&generated).map_err(|source| ContractArtifactError::Canonical {
                path: path.clone(),
                source,
            })?;
        outputs.insert(
            PathBuf::from(format!(
                "contracts/generated/registry/{}.json",
                source_name.trim_end_matches(".yaml")
            )),
            encoded,
        );
    }
    Ok(())
}

fn render_index(
    repository_root: &Path,
    artifact_records: &[(String, String)],
) -> Result<(Vec<u8>, String), ContractArtifactError> {
    let index_records = artifact_records
        .iter()
        .map(|(path, canonical_digest)| {
            json!({
                "canonical_digest": canonical_digest,
                "path": path,
            })
        })
        .collect::<Vec<_>>();
    let index = json!({
        "_generated": {
            "generator_revision": "codefabric-contracts-wp06-v1",
            "profile": PROFILE,
        },
        "artifacts": index_records,
    });
    let bytes = canonicalize_value(&index).map_err(|source| ContractArtifactError::Canonical {
        path: repository_root.join(GENERATED_INDEX),
        source,
    })?;
    let digest = checksum(&bytes);
    Ok((bytes, digest))
}

fn render_rust_index(artifact_records: &[(String, String)], index_digest: &str) -> Vec<u8> {
    let mut rust = format!(
        concat!(
            "// @generated from {generated_index} {index_digest}; do not edit.\n",
            "/// One source artifact and its canonical BLAKE3 digest.\n",
            "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n",
            "pub struct GeneratedContractArtifact {{\n",
            "    /// Repository-relative authority path.\n",
            "    pub path: &'static str,\n",
            "    /// BLAKE3-256 over the artifact's canonical bytes.\n",
            "    pub canonical_digest: &'static str,\n",
            "}}\n\n",
            "/// BLAKE3 digest of the generated Wave 1 artifact index.\n",
            "pub const CONTRACT_ARTIFACT_INDEX_DIGEST: &str = \"{index_digest}\";\n\n",
            "/// Exact AC-G-05 source-artifact index.\n",
            "pub const CONTRACT_ARTIFACTS: &[GeneratedContractArtifact] = &[\n",
        ),
        generated_index = GENERATED_INDEX,
        index_digest = index_digest
    );
    for (path, canonical_digest) in artifact_records {
        writeln!(
            rust,
            "    GeneratedContractArtifact {{ path: {path:?}, canonical_digest: {canonical_digest:?} }},"
        )
        .expect("writing to a String cannot fail");
    }
    rust.push_str("];\n");
    rust.into_bytes()
}

fn render_python_index(artifact_records: &[(String, String)], index_digest: &str) -> Vec<u8> {
    let mut python = format!(
        concat!(
            "# @generated from {generated_index} {index_digest}; do not edit.\n",
            "from typing import Final, NamedTuple\n\n\n",
            "class GeneratedContractArtifact(NamedTuple):\n",
            "    path: str\n",
            "    canonical_digest: str\n\n\n",
            "CONTRACT_ARTIFACT_INDEX_DIGEST: Final[str] = (\n",
            "    \"{index_digest}\"\n",
            ")\n",
            "CONTRACT_ARTIFACTS: Final[tuple[GeneratedContractArtifact, ...]] = (\n",
        ),
        generated_index = GENERATED_INDEX,
        index_digest = index_digest
    );
    for (path, canonical_digest) in artifact_records {
        writeln!(
            python,
            "    GeneratedContractArtifact(\n        path={path:?},\n        canonical_digest={canonical_digest:?},\n    ),"
        )
        .expect("writing to a String cannot fail");
    }
    python.push_str(")\n");
    python.into_bytes()
}

fn render_outputs(
    repository_root: &Path,
) -> Result<BTreeMap<PathBuf, Vec<u8>>, ContractArtifactError> {
    let contracts_root = repository_root.join("contracts");
    let mut outputs = BTreeMap::new();
    let artifact_records = collect_artifact_records(&contracts_root)?;
    render_registry_outputs(&contracts_root, &mut outputs)?;
    let (index_bytes, index_digest) = render_index(repository_root, &artifact_records)?;
    outputs.insert(PathBuf::from(GENERATED_INDEX), index_bytes);
    outputs.insert(
        PathBuf::from(GENERATED_RUST),
        render_rust_index(&artifact_records, &index_digest),
    );
    outputs.insert(
        PathBuf::from(GENERATED_PYTHON),
        render_python_index(&artifact_records, &index_digest),
    );

    let python_stub = format!(
        concat!(
            "# @generated from {generated_index} {index_digest}; do not edit.\n",
            "from typing import Final, NamedTuple\n\n",
            "class GeneratedContractArtifact(NamedTuple):\n",
            "    path: str\n",
            "    canonical_digest: str\n\n",
            "CONTRACT_ARTIFACT_INDEX_DIGEST: Final[str]\n",
            "CONTRACT_ARTIFACTS: Final[tuple[GeneratedContractArtifact, ...]]\n",
        ),
        generated_index = GENERATED_INDEX,
        index_digest = index_digest
    );
    outputs.insert(
        PathBuf::from(GENERATED_PYTHON_STUB),
        python_stub.into_bytes(),
    );
    let python_init = format!(
        concat!(
            "# @generated from {generated_index} {index_digest}; do not edit.\n",
            "\"\"\"Typed contract-artifact identities generated from AC-G-05 sources.\"\"\"\n\n",
            "from ._contract_index import (\n",
            "    CONTRACT_ARTIFACT_INDEX_DIGEST,\n",
            "    CONTRACT_ARTIFACTS,\n",
            "    GeneratedContractArtifact,\n",
            ")\n\n",
            "__all__ = [\n",
            "    \"CONTRACT_ARTIFACT_INDEX_DIGEST\",\n",
            "    \"CONTRACT_ARTIFACTS\",\n",
            "    \"GeneratedContractArtifact\",\n",
            "]\n",
        ),
        generated_index = GENERATED_INDEX,
        index_digest = index_digest
    );
    outputs.insert(
        PathBuf::from(GENERATED_PYTHON_INIT),
        python_init.into_bytes(),
    );
    Ok(outputs)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ContractArtifactError> {
    let parent = path
        .parent()
        .ok_or_else(|| ContractArtifactError::Missing(path.to_owned()))?;
    fs::create_dir_all(parent).map_err(|source| ContractArtifactError::Io {
        path: parent.to_owned(),
        source,
    })?;
    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|source| ContractArtifactError::Io {
            path: parent.to_owned(),
            source,
        })?;
    temporary
        .write_all(bytes)
        .map_err(|source| ContractArtifactError::Io {
            path: path.to_owned(),
            source,
        })?;
    temporary
        .persist(path)
        .map_err(|error| ContractArtifactError::Io {
            path: path.to_owned(),
            source: error.error,
        })?;
    Ok(())
}

/// Generate every committed WP06 derivative using atomic same-directory replacement.
///
/// # Errors
///
/// Returns an error for missing/invalid sources, canonicalization, or filesystem failure.
pub fn generate(repository_root: &Path) -> Result<usize, ContractArtifactError> {
    let outputs = render_outputs(repository_root)?;
    for (relative, bytes) in &outputs {
        write_atomic(&repository_root.join(relative), bytes)?;
    }
    Ok(outputs.len())
}

fn verify_generated(repository_root: &Path) -> Result<(), ContractArtifactError> {
    let outputs = render_outputs(repository_root)?;
    verify_generated_census(repository_root, &outputs, Path::new("contracts/generated"))?;
    verify_generated_census(
        repository_root,
        &outputs,
        Path::new("codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/generated"),
    )?;
    for (relative, expected) in outputs {
        let path = repository_root.join(&relative);
        let actual = read(&path)?;
        if actual != expected {
            return Err(ContractArtifactError::Drift(relative));
        }
    }
    Ok(())
}

fn verify_generated_census(
    repository_root: &Path,
    outputs: &BTreeMap<PathBuf, Vec<u8>>,
    relative_directory: &Path,
) -> Result<(), ContractArtifactError> {
    let expected = outputs
        .keys()
        .filter(|path| path.starts_with(relative_directory))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    collect_generated_files(
        repository_root,
        &repository_root.join(relative_directory),
        &mut actual,
    )?;
    if actual != expected {
        let drift = actual
            .symmetric_difference(&expected)
            .next()
            .cloned()
            .unwrap_or_else(|| relative_directory.to_owned());
        return Err(ContractArtifactError::Drift(drift));
    }
    Ok(())
}

fn collect_generated_files(
    repository_root: &Path,
    directory: &Path,
    files: &mut BTreeSet<PathBuf>,
) -> Result<(), ContractArtifactError> {
    let entries = fs::read_dir(directory).map_err(|source| ContractArtifactError::Io {
        path: directory.to_owned(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| ContractArtifactError::Io {
            path: directory.to_owned(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| ContractArtifactError::Io {
                path: path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            if entry.file_name() != "__pycache__" {
                collect_generated_files(repository_root, &path, files)?;
            }
        } else if file_type.is_file() {
            if path.extension() == Some(OsStr::new("pyc")) {
                continue;
            }
            let relative = path
                .strip_prefix(repository_root)
                .map_err(|_| ContractArtifactError::Drift(path.clone()))?;
            files.insert(relative.to_owned());
        } else {
            return Err(ContractArtifactError::Drift(path));
        }
    }
    Ok(())
}

fn has_metadata(bytes: &[u8]) -> bool {
    [
        b"artifact_id".as_slice(),
        b"artifact_kind".as_slice(),
        b"version".as_slice(),
        b"compatible_suite_major".as_slice(),
        b"status".as_slice(),
        b"canonical_digest".as_slice(),
    ]
    .iter()
    .all(|marker| bytes.windows(marker.len()).any(|window| window == *marker))
}

fn is_draft(bytes: &[u8]) -> bool {
    [
        b"status: draft".as_slice(),
        b"\"status\":\"draft\"".as_slice(),
        b"\"status\": \"draft\"".as_slice(),
    ]
    .iter()
    .any(|marker| bytes.windows(marker.len()).any(|window| window == *marker))
}

fn jsonl_records(path: &Path) -> Result<Vec<Value>, ContractArtifactError> {
    let bytes = read(path)?;
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_slice(line).map_err(|error| ContractArtifactError::Traceability {
                path: path.to_owned(),
                message: error.to_string(),
            })
        })
        .collect()
}

fn valid_requirement_id(identifier: &str) -> bool {
    let mut parts = identifier.split('-');
    let prefix = parts.next();
    let owner = parts.next();
    let number = parts.next();
    prefix == Some("CF")
        && owner.is_some_and(|value| {
            matches!(
                value,
                "ARCH" | "ONT" | "GEN" | "FAB" | "LIFE" | "QUERY" | "SERVE" | "SEC" | "TEST"
            )
        })
        && number.is_some_and(|value| {
            value.len() == 4 && value.bytes().all(|byte| byte.is_ascii_digit())
        })
        && parts.next().is_none()
}

fn non_empty_string_array(record: &Value, field: &str) -> bool {
    record[field].as_array().is_some_and(|values| {
        !values.is_empty()
            && values
                .iter()
                .all(|value| value.as_str().is_some_and(|text| !text.is_empty()))
    })
}

fn verify_traceability(contracts_root: &Path) -> Result<(), ContractArtifactError> {
    let requirements_path = contracts_root.join("manifests/requirements.jsonl");
    let traceability_path = contracts_root.join("manifests/traceability.jsonl");
    let mut requirement_ids = BTreeSet::new();
    for record in jsonl_records(&requirements_path)? {
        if record["record_kind"] == "metadata" {
            continue;
        }
        let Some(identifier) = record["requirement_id"].as_str() else {
            return Err(ContractArtifactError::Traceability {
                path: requirements_path,
                message: "requirement_id is absent".to_owned(),
            });
        };
        if !valid_requirement_id(identifier) || !requirement_ids.insert(identifier.to_owned()) {
            return Err(ContractArtifactError::Traceability {
                path: requirements_path,
                message: format!("invalid or duplicate requirement ID: {identifier}"),
            });
        }
        let normative_text = record["normative_text"].as_str().unwrap_or_default();
        if record["normative_text_digest"].as_str()
            != Some(checksum(normative_text.as_bytes()).as_str())
            || !non_empty_string_array(&record, "implements")
            || !non_empty_string_array(&record, "verified_by")
            || record["owner_acceptance"]["approver"].as_str().is_none()
            || record["owner_acceptance"]["source_digest"]
                .as_str()
                .is_none()
        {
            return Err(ContractArtifactError::Traceability {
                path: requirements_path,
                message: format!(
                    "requirement {identifier} is incomplete or has a stale text digest"
                ),
            });
        }
    }
    if requirement_ids.is_empty() {
        return Err(ContractArtifactError::Traceability {
            path: requirements_path,
            message: "no CF-* requirement records exist".to_owned(),
        });
    }

    let mut traced_ids = BTreeSet::new();
    for record in jsonl_records(&traceability_path)? {
        if record["record_kind"] == "metadata" {
            continue;
        }
        let Some(identifier) = record["requirement_id"].as_str() else {
            return Err(ContractArtifactError::Traceability {
                path: traceability_path,
                message: "trace requirement_id is absent".to_owned(),
            });
        };
        if !requirement_ids.contains(identifier)
            || !traced_ids.insert(identifier.to_owned())
            || !non_empty_string_array(&record, "implements")
            || !non_empty_string_array(&record, "verified_by")
        {
            return Err(ContractArtifactError::Traceability {
                path: traceability_path,
                message: format!("trace for {identifier} is unknown, duplicate, or orphaned"),
            });
        }
    }
    if traced_ids != requirement_ids {
        return Err(ContractArtifactError::Traceability {
            path: traceability_path,
            message: "one or more requirements have no trace record".to_owned(),
        });
    }
    Ok(())
}

/// Verify the AC-G-05 source layout, metadata, shared JCS corpus, and generated bytes.
///
/// # Errors
///
/// Returns an error for an absent/invalid artifact, generated drift, corpus mismatch,
/// or any warning under the released profile.
pub fn verify(
    repository_root: &Path,
    profile: VerificationProfile,
) -> Result<VerificationReport, ContractArtifactError> {
    let contracts_root = repository_root.join("contracts");
    let arrow_delta = contracts_root.join("schema/arrow-delta");
    if !arrow_delta.is_dir() {
        return Err(ContractArtifactError::Missing(arrow_delta));
    }

    let mut warning_count = 0;
    for relative in REQUIRED_SOURCE_ARTIFACTS {
        let path = contracts_root.join(relative);
        let bytes = read(&path)?;
        if !has_metadata(&bytes) {
            return Err(ContractArtifactError::Metadata(path));
        }
        warning_count += usize::from(is_draft(&bytes));
        canonical_source_bytes(&path, &bytes)?;
    }
    verify_traceability(&contracts_root)?;
    verify_generated(repository_root)?;
    verify_jcs_corpus(&contracts_root.join("fixtures/jcs/vectors.json"))?;

    if profile == VerificationProfile::Released && warning_count != 0 {
        return Err(ContractArtifactError::ReleasedWarnings(warning_count));
    }
    Ok(VerificationReport {
        artifact_count: REQUIRED_SOURCE_ARTIFACTS.len(),
        warning_count,
    })
}

fn required_vector_string<'a>(
    vector: &'a Value,
    path: &Path,
    identifier: &str,
    field: &str,
) -> Result<&'a str, ContractArtifactError> {
    vector[field]
        .as_str()
        .ok_or_else(|| fixture_failure(path, format!("{identifier}: {field} must be a string")))
}

fn verify_positive_jcs_vectors(corpus: &Value, path: &Path) -> Result<(), ContractArtifactError> {
    let positives = corpus["positive"]
        .as_array()
        .ok_or_else(|| fixture_failure(path, "positive must be an array"))?;
    for vector in positives {
        let identifier = required_vector_string(vector, path, "positive", "id")?;
        let input = required_vector_string(vector, path, identifier, "input_json")?;
        let expected = required_vector_string(vector, path, identifier, "canonical_utf8")?;
        let expected_checksum = required_vector_string(vector, path, identifier, "checksum")?;
        validate_checksum(expected_checksum)
            .map_err(|error| fixture_failure(path, format!("{identifier}: {error}")))?;
        let actual = canonicalize_slice(input.as_bytes())
            .map_err(|error| fixture_failure(path, format!("{identifier}: {error}")))?;
        if actual != expected.as_bytes() || checksum(&actual) != expected_checksum {
            return Err(fixture_failure(
                path,
                format!("{identifier}: canonical bytes or checksum drifted"),
            ));
        }
    }
    Ok(())
}

fn verify_negative_jcs_vectors(corpus: &Value, path: &Path) -> Result<(), ContractArtifactError> {
    let negatives = corpus["negative"]
        .as_array()
        .ok_or_else(|| fixture_failure(path, "negative must be an array"))?;
    for vector in negatives {
        let identifier = required_vector_string(vector, path, "negative", "id")?;
        let input = required_vector_string(vector, path, identifier, "input_json")?;
        let expected_class = required_vector_string(vector, path, identifier, "error")?;
        match canonicalize_slice(input.as_bytes()) {
            Ok(_) => {
                return Err(fixture_failure(
                    path,
                    format!("{identifier}: negative vector was accepted"),
                ));
            }
            Err(error) if error.failure_class() != expected_class => {
                return Err(fixture_failure(
                    path,
                    format!(
                        "{identifier}: expected failure class {expected_class:?}, got {:?}",
                        error.failure_class()
                    ),
                ));
            }
            Err(_) => {}
        }
    }
    Ok(())
}

fn verify_non_string_map_vector(corpus: &Value, path: &Path) -> Result<(), ContractArtifactError> {
    let map_fixture = &corpus["non_string_map"];
    let entries = map_fixture["entries"]
        .as_array()
        .ok_or_else(|| fixture_failure(path, "non_string_map.entries must be an array"))?
        .iter()
        .map(|record| (record["key"].clone(), record["value"].clone()))
        .collect::<Vec<_>>();
    let expected = required_vector_string(map_fixture, path, "non_string_map", "canonical_utf8")?;
    let records = non_string_map_records(entries)
        .map_err(|error| fixture_failure(path, format!("non-string map: {error}")))?;
    let actual = canonicalize_value(&records)
        .map_err(|error| fixture_failure(path, format!("non-string map: {error}")))?;
    if actual == expected.as_bytes() {
        Ok(())
    } else {
        Err(fixture_failure(
            path,
            "non-string map record ordering drifted",
        ))
    }
}

/// Replay the cross-language canonical JSON vectors.
///
/// # Errors
///
/// Returns an error when a vector's bytes, checksum, or expected failure drifts.
pub fn verify_jcs_corpus(path: &Path) -> Result<(), ContractArtifactError> {
    let bytes = read(path)?;
    let corpus: Value =
        serde_json::from_slice(&bytes).map_err(|error| fixture_failure(path, error.to_string()))?;
    verify_positive_jcs_vectors(&corpus, path)?;
    verify_negative_jcs_vectors(&corpus, path)?;
    if corpus["profile"].as_str() != Some(PROFILE) {
        return Err(fixture_failure(path, "canonical profile identity drifted"));
    }
    verify_format_vectors(&corpus, path, "int64", validate_int64)?;
    verify_format_vectors(&corpus, path, "uint64", validate_uint64)?;
    verify_format_vectors(&corpus, path, "bytes", validate_bytes)?;
    verify_format_vectors(&corpus, path, "lowercase_public", validate_lowercase_public)?;
    verify_format_vectors(&corpus, path, "checksum", validate_checksum)?;
    verify_non_string_map_vector(&corpus, path)
}

fn verify_format_vectors(
    corpus: &Value,
    path: &Path,
    group: &str,
    validator: impl Fn(&str) -> Result<(), CanonicalJsonError>,
) -> Result<(), ContractArtifactError> {
    let format = &corpus["formats"][group];
    for value in format["positive"].as_array().into_iter().flatten() {
        let value = value.as_str().unwrap_or_default();
        validator(value).map_err(|error| ContractArtifactError::Fixture {
            path: path.to_owned(),
            message: format!("{group} positive {value:?}: {error}"),
        })?;
    }
    for value in format["negative"].as_array().into_iter().flatten() {
        let value = value.as_str().unwrap_or_default();
        if validator(value).is_ok() {
            return Err(ContractArtifactError::Fixture {
                path: path.to_owned(),
                message: format!("{group} negative {value:?} was accepted"),
            });
        }
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct NegativeFixture {
    source_utf8: String,
    claimed_checksum: String,
}

/// Verify one intentionally drifted checksum fixture.
///
/// This function succeeds only when the claim is valid, so the repository gate invokes
/// it against committed negative fixtures and requires a non-zero process result.
///
/// # Errors
///
/// Returns an error for malformed input or checksum mismatch.
pub fn verify_checksum_fixture(path: &Path) -> Result<(), ContractArtifactError> {
    let fixture: NegativeFixture =
        serde_json::from_slice(&read(path)?).map_err(|error| ContractArtifactError::Fixture {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    validate_checksum(&fixture.claimed_checksum).map_err(|source| {
        ContractArtifactError::Canonical {
            path: path.to_owned(),
            source,
        }
    })?;
    let actual = checksum(fixture.source_utf8.as_bytes());
    if actual == fixture.claimed_checksum {
        Ok(())
    } else {
        Err(ContractArtifactError::Fixture {
            path: path.to_owned(),
            message: format!(
                "checksum mismatch: claimed {}, actual {actual}",
                fixture.claimed_checksum
            ),
        })
    }
}

/// Deterministic generator identity for the administrative CLI.
#[derive(Serialize)]
pub struct ContractToolIdentity<'a> {
    /// Executable identity.
    pub executable: &'a str,
    /// Package version.
    pub version: &'a str,
    /// Canonical JSON profile.
    pub canonical_json_profile: &'a str,
    /// Rust JCS library identity.
    pub rust_jcs: &'a str,
}

/// Construct the exact generator/verifier identity record.
#[must_use]
pub const fn identity() -> ContractToolIdentity<'static> {
    ContractToolIdentity {
        executable: "codefabric-contracts",
        version: env!("CARGO_PKG_VERSION"),
        canonical_json_profile: PROFILE,
        rust_jcs: "serde_json_canonicalizer 0.3.2",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_layout_has_the_exact_census() {
        assert_eq!(REQUIRED_SOURCE_ARTIFACTS.len(), 50);
        assert_eq!(REGISTRY_SOURCES.len(), 13);
    }

    #[test]
    fn verifier_profiles_parse_strictly() {
        assert_eq!(
            VerificationProfile::parse("full").unwrap(),
            VerificationProfile::Full
        );
        assert!(VerificationProfile::parse("Full").is_err());
    }

    #[test]
    fn yaml_projection_resolves_aliases_and_encodes_non_string_maps_as_records() {
        let path = Path::new("fixture.yaml");
        let aliased =
            canonical_source_bytes(path, b"base: &shared [1, 2]\ncopy: *shared\n").unwrap();
        let inline = canonical_source_bytes(path, b"base: [1, 2]\ncopy: [1, 2]\n").unwrap();
        assert_eq!(aliased, inline);

        let non_string = canonical_source_bytes(path, b"2: two\n1: one\n").unwrap();
        assert_eq!(
            non_string,
            br#"[{"key":1,"value":"one"},{"key":2,"value":"two"}]"#
        );
    }

    #[test]
    fn yaml_projection_rejects_ambiguous_constructs() {
        let path = Path::new("fixture.yaml");
        for source in [
            b"value: !application data\n".as_slice(),
            b"defaults: &defaults\n  a: 1\nvalue:\n  <<: *defaults\n".as_slice(),
            b"outer:\n  same: 1\n  same: 2\n".as_slice(),
            b"---\na: 1\n---\nb: 2\n".as_slice(),
        ] {
            assert!(canonical_source_bytes(path, source).is_err());
        }
    }
}
