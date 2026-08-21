//! Contract-tree generation and verification for the Wave 1 machine authority.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tempfile::NamedTempFile;
use thiserror::Error;

use super::catalog::{CatalogError, CompiledCatalog, ContractCatalog, GeneratedOutputKind};
use super::compiler::{ContractCompileError, compile_artifact, compile_artifact_for_generation};
use super::index::{
    ArtifactIndex, ArtifactIndexGeneration, ArtifactIndexOutput, ArtifactIndexRecord,
};
use super::jcs::{
    CanonicalJsonError, PROFILE, canonicalize_slice, canonicalize_value, checksum, decode_strict,
    non_string_map_records, validate_bytes, validate_checksum, validate_int64,
    validate_lowercase_public, validate_uint64,
};
use super::models::{RequirementRecord, TraceabilityRecord};

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
    /// The typed catalog could not be loaded or compiled.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// Native bounded compilation failed.
    #[error(transparent)]
    Compile(#[from] ContractCompileError),
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

fn fixture_failure(path: &Path, message: impl Into<String>) -> ContractArtifactError {
    ContractArtifactError::Fixture {
        path: path.to_owned(),
        message: message.into(),
    }
}

fn generated_registry_value(
    artifact: &super::catalog::ArtifactDescriptor,
    compiled: &super::compiler::CompiledArtifact,
    value: &Value,
) -> Value {
    json!({
        "_generated": {
            "generator_revision": "codefabric-contracts-wp06-v1",
            "profile": PROFILE,
            "source_artifact_id": artifact.artifact_id,
            "source": artifact.authority_path,
            "source_digest": compiled.source_digest,
            "canonical_digest": compiled.canonical_digest,
            "digest_projection": compiled.digest_projection,
        },
        "value": value,
    })
}

fn collect_artifact_records(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<Vec<ArtifactIndexRecord>, ContractArtifactError> {
    let mut records = Vec::new();
    for artifact in catalog.artifacts() {
        let compiled = compile_artifact(repository_root, catalog, artifact)?;
        records.push(ArtifactIndexRecord {
            artifact_id: artifact.artifact_id.clone(),
            authority_path: artifact.authority_path.clone(),
            artifact_kind: artifact.artifact_kind,
            owner: artifact.owner,
            version: artifact.version.clone(),
            compatible_suite_major: artifact.compatible_suite_major,
            status: artifact.status,
            digest_projection: artifact.digest_projection,
            canonical_digest: compiled.canonical_digest,
            source_digest: compiled.source_digest,
            bundle_digest: compiled.bundle_digest,
            compatibility_family: artifact.compatibility_family,
            provenance_requirements: artifact.provenance_requirements.iter().copied().collect(),
            consumers: artifact.consumers.iter().copied().collect(),
            generated_outputs: artifact
                .generated_outputs
                .iter()
                .map(|output| ArtifactIndexOutput {
                    path: output.path.clone(),
                    output_kind: output.output_kind,
                    producer: output.producer,
                    consumers: output.consumers.iter().copied().collect(),
                })
                .collect(),
        });
    }
    Ok(records)
}

fn render_registry_outputs(
    repository_root: &Path,
    catalog: &CompiledCatalog,
    outputs: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), ContractArtifactError> {
    for (output_path, owner, output) in catalog.outputs() {
        if output.output_kind != GeneratedOutputKind::CanonicalRegistry {
            continue;
        }
        let artifact = catalog
            .artifact(owner)
            .expect("compiled output owner must be a catalog artifact");
        let path = repository_root.join(&artifact.authority_path);
        let compiled = compile_artifact(repository_root, catalog, artifact)?;
        let value = decode_strict(&compiled.canonical_bytes).map_err(|source| {
            ContractArtifactError::Canonical {
                path: path.clone(),
                source,
            }
        })?;
        let generated = generated_registry_value(artifact, &compiled, &value);
        let encoded =
            canonicalize_value(&generated).map_err(|source| ContractArtifactError::Canonical {
                path: path.clone(),
                source,
            })?;
        outputs.insert(output_path.to_owned(), encoded);
    }
    Ok(())
}

fn render_index(
    repository_root: &Path,
    index_path: &Path,
    artifact_records: &[ArtifactIndexRecord],
) -> Result<Vec<u8>, ContractArtifactError> {
    let index = ArtifactIndex {
        generated: ArtifactIndexGeneration {
            catalog_artifact_id: "codefabric.manifests.suite-manifest".to_owned(),
            artifact_count: artifact_records.len(),
            generator_revision: "codefabric-contracts-model-v1".to_owned(),
            profile: PROFILE.to_owned(),
        },
        artifacts: artifact_records.to_vec(),
    };
    canonicalize_value(
        &serde_json::to_value(index).expect("typed artifact index serialization is infallible"),
    )
    .map_err(|source| ContractArtifactError::Canonical {
        path: repository_root.join(index_path),
        source,
    })
}

fn render_outputs(
    repository_root: &Path,
) -> Result<BTreeMap<PathBuf, Vec<u8>>, ContractArtifactError> {
    let catalog = ContractCatalog::load(repository_root)?;
    let mut outputs = BTreeMap::new();
    let artifact_records = collect_artifact_records(repository_root, &catalog)?;
    render_registry_outputs(repository_root, &catalog, &mut outputs)?;
    let generated_index = required_output(&catalog, GeneratedOutputKind::ArtifactIndex)?;
    let index_bytes = render_index(repository_root, &generated_index, &artifact_records)?;
    outputs.insert(generated_index, index_bytes);
    Ok(outputs)
}

fn required_output(
    catalog: &CompiledCatalog,
    output_kind: GeneratedOutputKind,
) -> Result<PathBuf, ContractArtifactError> {
    catalog
        .output_of_kind(output_kind)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            ContractArtifactError::Missing(PathBuf::from(format!(
                "catalog output kind {output_kind:?}"
            )))
        })
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

fn replace_unique_digest(
    path: &Path,
    bytes: &[u8],
    claimed: &str,
    computed: &str,
) -> Result<Option<Vec<u8>>, ContractArtifactError> {
    if claimed == computed {
        return Ok(None);
    }
    let matches = bytes
        .windows(claimed.len())
        .enumerate()
        .filter(|(_, window)| *window == claimed.as_bytes())
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    let [offset] = matches.as_slice() else {
        return Err(ContractArtifactError::Metadata(path.to_owned()));
    };
    let mut updated = bytes.to_vec();
    updated.splice(
        *offset..(*offset + claimed.len()),
        computed.as_bytes().iter().copied(),
    );
    Ok(Some(updated))
}

fn embed_bundle_digests(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<(), ContractArtifactError> {
    for artifact in catalog.artifacts().filter(|artifact| {
        artifact.digest_projection == super::catalog::DigestProjection::BundleAcG07V1
    }) {
        let path = repository_root.join(&artifact.authority_path);
        let compiled = compile_artifact_for_generation(repository_root, catalog, artifact)?;
        let computed = compiled
            .bundle_digest
            .expect("the bundle projection always computes a bundle identity");
        let bytes = read(&path)?;
        let mut value =
            decode_strict(&bytes).map_err(|source| ContractArtifactError::Canonical {
                path: path.clone(),
                source,
            })?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| ContractArtifactError::Metadata(path.clone()))?;
        if object.get("bundle_digest").and_then(Value::as_str) == Some(&computed) {
            continue;
        }
        object.insert("bundle_digest".to_owned(), Value::String(computed));
        let mut updated = serde_json::to_vec_pretty(&value).map_err(|source| {
            ContractArtifactError::Canonical {
                path: path.clone(),
                source: CanonicalJsonError::Serialization(source),
            }
        })?;
        updated.push(b'\n');
        write_atomic(&path, &updated)?;
    }
    Ok(())
}

fn embed_semantic_digests(repository_root: &Path) -> Result<(), ContractArtifactError> {
    let catalog = ContractCatalog::load(repository_root)?;
    embed_bundle_digests(repository_root, &catalog)?;

    let catalog = ContractCatalog::load(repository_root)?;
    for artifact in catalog.artifacts() {
        let compiled = compile_artifact_for_generation(repository_root, &catalog, artifact)?;
        let Some(claimed) = compiled.embedded_canonical_digest else {
            continue;
        };
        let path = repository_root.join(&artifact.authority_path);
        let bytes = read(&path)?;
        if let Some(updated) =
            replace_unique_digest(&path, &bytes, &claimed, &compiled.canonical_digest)?
        {
            write_atomic(&path, &updated)?;
        }
    }

    let catalog = ContractCatalog::load(repository_root)?;
    for artifact in catalog.artifacts() {
        compile_artifact(repository_root, &catalog, artifact)?;
    }
    Ok(())
}

/// Generate every committed model-derived contract output using atomic replacement.
///
/// # Errors
///
/// Returns an error for missing/invalid sources, canonicalization, or filesystem failure.
pub fn generate(repository_root: &Path) -> Result<usize, ContractArtifactError> {
    embed_semantic_digests(repository_root)?;
    let outputs = render_outputs(repository_root)?;
    for (relative, bytes) in &outputs {
        write_atomic(&repository_root.join(relative), bytes)?;
    }
    Ok(outputs.len())
}

fn verify_generated(repository_root: &Path) -> Result<(), ContractArtifactError> {
    let outputs = render_outputs(repository_root)?;
    verify_generated_census(repository_root, &outputs, Path::new("contracts/generated"))?;
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

fn typed_jsonl_records<T: DeserializeOwned>(
    path: &Path,
    canonical_bytes: &[u8],
) -> Result<Vec<T>, ContractArtifactError> {
    canonical_bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .skip(1)
        .map(|line| {
            let value =
                decode_strict(line).map_err(|error| ContractArtifactError::Traceability {
                    path: path.to_owned(),
                    message: error.to_string(),
                })?;
            serde_json::from_value(value).map_err(|error| ContractArtifactError::Traceability {
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

fn non_empty_strings(values: &[String]) -> bool {
    !values.is_empty() && values.iter().all(|value| !value.is_empty())
}

fn verify_traceability(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<(), ContractArtifactError> {
    let requirements = catalog
        .artifact("codefabric.manifests.requirements")
        .expect("the compiled catalog owns the requirements manifest");
    let traceability = catalog
        .artifact("codefabric.manifests.traceability")
        .expect("the compiled catalog owns the traceability manifest");
    let requirements_path = repository_root.join(&requirements.authority_path);
    let traceability_path = repository_root.join(&traceability.authority_path);
    let compiled_requirements = compile_artifact(repository_root, catalog, requirements)?;
    let compiled_traceability = compile_artifact(repository_root, catalog, traceability)?;
    let mut requirement_ids = BTreeSet::new();
    for record in typed_jsonl_records::<RequirementRecord>(
        &requirements_path,
        &compiled_requirements.canonical_bytes,
    )? {
        let identifier = record.requirement_id;
        if !valid_requirement_id(&identifier) || !requirement_ids.insert(identifier.clone()) {
            return Err(ContractArtifactError::Traceability {
                path: requirements_path.clone(),
                message: format!("invalid or duplicate requirement ID: {identifier}"),
            });
        }
        if record.normative_text_digest != checksum(record.normative_text.as_bytes())
            || !non_empty_strings(&record.implements)
            || !non_empty_strings(&record.verified_by)
            || record.owner_acceptance.approver.is_empty()
            || record.owner_acceptance.source_digest.is_empty()
        {
            return Err(ContractArtifactError::Traceability {
                path: requirements_path.clone(),
                message: format!(
                    "requirement {identifier} is incomplete or has a stale text digest"
                ),
            });
        }
    }
    if requirement_ids.is_empty() {
        return Err(ContractArtifactError::Traceability {
            path: requirements_path.clone(),
            message: "no CF-* requirement records exist".to_owned(),
        });
    }

    let mut traced_ids = BTreeSet::new();
    for record in typed_jsonl_records::<TraceabilityRecord>(
        &traceability_path,
        &compiled_traceability.canonical_bytes,
    )? {
        let identifier = record.requirement_id;
        if !requirement_ids.contains(&identifier)
            || !traced_ids.insert(identifier.clone())
            || !non_empty_strings(&record.implements)
            || !non_empty_strings(&record.verified_by)
        {
            return Err(ContractArtifactError::Traceability {
                path: traceability_path.clone(),
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

    let catalog = ContractCatalog::load(repository_root)?;
    let warning_count = catalog.draft_count();
    for artifact in catalog.artifacts() {
        compile_artifact(repository_root, &catalog, artifact)?;
    }
    verify_traceability(repository_root, &catalog)?;
    verify_generated(repository_root)?;
    verify_jcs_corpus(&contracts_root.join("fixtures/jcs/vectors.json"))?;
    verify_jcs_differential(&contracts_root.join("fixtures/jcs/differential-cases.json"))?;

    if profile == VerificationProfile::Released && warning_count != 0 {
        return Err(ContractArtifactError::ReleasedWarnings(warning_count));
    }
    Ok(VerificationReport {
        artifact_count: catalog.artifact_count(),
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

fn verify_jcs_differential(path: &Path) -> Result<(), ContractArtifactError> {
    let corpus =
        decode_strict(&read(path)?).map_err(|source| ContractArtifactError::Canonical {
            path: path.to_owned(),
            source,
        })?;
    let cases = corpus["cases"]
        .as_array()
        .ok_or_else(|| fixture_failure(path, "cases must be an array"))?;
    for case in cases {
        let identifier = case["id"].as_str().unwrap_or("differential");
        let inputs = case["inputs"].as_array().ok_or_else(|| {
            fixture_failure(path, format!("{identifier}: inputs must be an array"))
        })?;
        let mut outputs = inputs.iter().map(|input| {
            let input = input.as_str().ok_or_else(|| {
                fixture_failure(path, format!("{identifier}: input must be a string"))
            })?;
            let output = canonicalize_slice(input.as_bytes())
                .map_err(|error| fixture_failure(path, format!("{identifier}: {error}")))?;
            if canonicalize_slice(&output)
                .map_err(|error| fixture_failure(path, format!("{identifier}: {error}")))?
                != output
            {
                return Err(fixture_failure(
                    path,
                    format!("{identifier}: canonicalization is not idempotent"),
                ));
            }
            Ok(output)
        });
        let first = outputs
            .next()
            .transpose()?
            .ok_or_else(|| fixture_failure(path, format!("{identifier}: inputs are empty")))?;
        for output in outputs {
            if output? != first {
                return Err(fixture_failure(
                    path,
                    format!("{identifier}: equivalent inputs diverged"),
                ));
            }
        }
    }
    Ok(())
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
    fn verifier_profiles_parse_strictly() {
        assert_eq!(
            VerificationProfile::parse("full").unwrap(),
            VerificationProfile::Full
        );
        assert!(VerificationProfile::parse("Full").is_err());
    }
}
