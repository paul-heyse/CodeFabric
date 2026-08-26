//! Typed, cached access to the model-derived packaged artifact index.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::jcs::{canonicalize_slice, checksum, validate_checksum};

/// Exact model-derived bytes compiled into Rust and packaged unchanged by the adapter.
pub static MODEL_ARTIFACT_INDEX_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json"
));

static INDEX: OnceLock<Result<ModelArtifactIndex, ModelArtifactIndexError>> = OnceLock::new();
static INDEX_DIGEST: OnceLock<String> = OnceLock::new();

/// One of the two closed resource-profile shapes in the packaged index contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ArtifactResourceProfile {
    /// Named aggregate profile when the family has no external driver.
    Named(NamedArtifactResourceProfile),
    /// Explicit bounds for an external model driver.
    External(ExternalArtifactResourceProfile),
}

/// Closed named aggregate resource profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NamedArtifactResourceProfile {
    /// Stable profile name.
    pub profile: String,
}

/// Closed external-driver resource bounds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalArtifactResourceProfile {
    /// Maximum source bytes read by one family action.
    pub max_source_bytes: u64,
    /// Maximum bytes emitted by one family action.
    pub max_output_bytes: u64,
    /// Maximum output count emitted by one family action.
    pub max_outputs: u64,
}

/// One detached source identity and its model-derived provenance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelArtifactIndexRecord {
    /// Stable public identity.
    pub artifact_id: String,
    /// Closed family-native artifact kind.
    pub artifact_kind: String,
    /// Current repository-relative source path.
    pub authority_path: PathBuf,
    /// Claiming model family.
    pub owner: String,
    /// Public artifact version.
    pub version: String,
    /// Compatible suite major.
    pub compatible_suite_major: u64,
    /// Family-native lifecycle status.
    pub status: String,
    /// Detached semantic identity.
    pub canonical_digest: String,
    /// Exact current-byte identity.
    pub source_digest: String,
    /// Named semantic projection.
    pub projection_profile: String,
    /// Owner-accepted release-census status.
    pub release_status: String,
    /// Owning model action.
    pub compilation_unit: String,
    /// Authority, evidence, acceptance, or derived role.
    pub source_role: String,
    /// Family resource bounds.
    pub resource_profile: ArtifactResourceProfile,
    /// Detached provenance claims.
    pub provenance: BTreeSet<String>,
}

/// Closed projection metadata for one generated output.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelOutputProjection {
    /// Optional family-native artifact kind.
    pub artifact_kind: Option<String>,
    /// Canonical artifact, schema, or generated language projection.
    pub projection_kind: String,
    /// Optional public schema identity projected into the output.
    pub public_identity: Option<String>,
}

/// One complete `DesiredTree` output with its producer and consumers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelOutputIndexRecord {
    /// Stable content-derived model output identity.
    pub output_id: String,
    /// Exact repository-relative output path.
    pub path: PathBuf,
    /// Owning model action.
    pub producer: String,
    /// Upstream action lineage.
    pub lineage: BTreeSet<String>,
    /// Real consuming capabilities.
    pub consumers: BTreeSet<String>,
    /// Optional public artifact identity associated with the output.
    pub public_artifact_id: Option<String>,
    /// Output projection metadata.
    pub projection: ModelOutputProjection,
    /// Family resource bounds.
    pub resource_profile: ArtifactResourceProfile,
    /// Executable validators for the output.
    pub validators: BTreeSet<String>,
}

/// Canonical model-derived packaged index.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelArtifactIndex {
    /// Wire schema version.
    pub schema_version: u64,
    /// Human-readable derivation authority.
    pub source: String,
    /// Records sorted by stable artifact identity.
    pub artifacts: Vec<ModelArtifactIndexRecord>,
    /// Complete `DesiredTree` output census sorted by repository-relative path.
    pub outputs: Vec<ModelOutputIndexRecord>,
}

/// Packaged model-index validation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ModelArtifactIndexError {
    /// Resource bytes were not strict canonical JSON.
    #[error("model artifact index is not canonical JSON: {0}")]
    Canonical(String),
    /// Resource bytes did not decode into the closed model.
    #[error("model artifact index typed decode failed: {0}")]
    Decode(String),
    /// A record carried malformed, duplicate, or incomplete provenance.
    #[error("model artifact index invariant failed: {0}")]
    Invariant(String),
}

fn safe_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && path
            .to_str()
            .is_some_and(|value| !value.is_empty() && !value.contains('\\'))
}

fn valid_resource_profile(profile: &ArtifactResourceProfile) -> bool {
    match profile {
        ArtifactResourceProfile::Named(named) => !named.profile.is_empty(),
        ArtifactResourceProfile::External(ExternalArtifactResourceProfile {
            max_source_bytes,
            max_output_bytes,
            max_outputs,
        }) => *max_source_bytes > 0 && *max_output_bytes > 0 && *max_outputs > 0,
    }
}

/// Decode exact canonical model-index bytes through the one cross-language contract.
///
/// # Errors
///
/// Returns the same closed canonical, typed-decode, and invariant failure classes used by the
/// packaged resource accessor.
pub fn decode_model_artifact_index(
    bytes: &[u8],
) -> Result<ModelArtifactIndex, ModelArtifactIndexError> {
    let canonical = canonicalize_slice(bytes)
        .map_err(|error| ModelArtifactIndexError::Canonical(error.to_string()))?;
    if canonical != bytes {
        return Err(ModelArtifactIndexError::Canonical(
            "resource bytes differ from RFC 8785 emission".to_owned(),
        ));
    }
    let index: ModelArtifactIndex = serde_json::from_slice(bytes)
        .map_err(|error| ModelArtifactIndexError::Decode(error.to_string()))?;
    if index.schema_version != 1
        || index.source != "RepositoryModel + accepted release census + complete DesiredTree census"
        || index.artifacts.is_empty()
        || index.outputs.is_empty()
    {
        return Err(ModelArtifactIndexError::Invariant(
            "index header is unsupported or empty".to_owned(),
        ));
    }
    let mut previous = None;
    for record in &index.artifacts {
        if previous.is_some_and(|value: &str| value >= record.artifact_id.as_str()) {
            return Err(ModelArtifactIndexError::Invariant(
                "artifact IDs are not unique and strictly sorted".to_owned(),
            ));
        }
        validate_checksum(&record.canonical_digest)
            .map_err(|error| ModelArtifactIndexError::Invariant(error.to_string()))?;
        validate_checksum(&record.source_digest)
            .map_err(|error| ModelArtifactIndexError::Invariant(error.to_string()))?;
        if record.compatible_suite_major != 1
            || record.artifact_kind.is_empty()
            || !safe_relative_path(&record.authority_path)
            || record.owner.is_empty()
            || record.version.is_empty()
            || record.projection_profile.is_empty()
            || record.compilation_unit.is_empty()
            || record.provenance.is_empty()
            || !valid_resource_profile(&record.resource_profile)
            || !matches!(record.release_status.as_str(), "released" | "unreleased")
            || !matches!(
                record.source_role.as_str(),
                "authority" | "evidence-authority" | "acceptance" | "derived"
            )
        {
            return Err(ModelArtifactIndexError::Invariant(format!(
                "artifact {} has incomplete model provenance",
                record.artifact_id
            )));
        }
        previous = Some(record.artifact_id.as_str());
    }
    let mut previous_path = None;
    let mut output_ids = BTreeSet::new();
    for record in &index.outputs {
        if previous_path
            .as_ref()
            .is_some_and(|value: &&PathBuf| *value >= &record.path)
            || !output_ids.insert(record.output_id.as_str())
            || record.output_id.is_empty()
            || record.path.as_os_str().is_empty()
            || !safe_relative_path(&record.path)
            || record.producer.is_empty()
            || record.lineage.is_empty()
            || record.consumers.is_empty()
            || record.projection.projection_kind.is_empty()
            || record.validators.is_empty()
            || !valid_resource_profile(&record.resource_profile)
        {
            return Err(ModelArtifactIndexError::Invariant(format!(
                "output {} has incomplete or unordered model provenance",
                record.output_id
            )));
        }
        previous_path = Some(&record.path);
    }
    Ok(index)
}

fn decode_index() -> Result<ModelArtifactIndex, ModelArtifactIndexError> {
    decode_model_artifact_index(MODEL_ARTIFACT_INDEX_BYTES)
}

/// Return the once-decoded model artifact index.
///
/// # Errors
///
/// Returns a stable typed failure when the packaged bytes are non-canonical or violate the model.
pub fn model_artifact_index() -> Result<&'static ModelArtifactIndex, ModelArtifactIndexError> {
    INDEX
        .get_or_init(decode_index)
        .as_ref()
        .map_err(Clone::clone)
}

/// Digest of the exact packaged model-index bytes.
#[must_use]
pub fn model_artifact_index_digest() -> &'static str {
    INDEX_DIGEST
        .get_or_init(|| checksum(MODEL_ARTIFACT_INDEX_BYTES))
        .as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_model_index_is_typed_canonical_and_sorted() {
        let index = model_artifact_index().unwrap();
        assert!(index.artifacts.len() > 100);
        assert_eq!(index.outputs.len(), 81);
        assert!(
            index
                .artifacts
                .windows(2)
                .all(|pair| pair[0].artifact_id < pair[1].artifact_id)
        );
        validate_checksum(model_artifact_index_digest()).unwrap();
    }

    #[derive(Deserialize)]
    struct DifferentialCase {
        mutation: String,
        accepted: bool,
    }

    #[derive(Deserialize)]
    struct DifferentialCorpus {
        cases: Vec<DifferentialCase>,
    }

    fn differential_bytes(mutation: &str) -> Vec<u8> {
        let mut value: serde_json::Value =
            serde_json::from_slice(MODEL_ARTIFACT_INDEX_BYTES).unwrap();
        match mutation {
            "none" => return MODEL_ARTIFACT_INDEX_BYTES.to_vec(),
            "unknown-root-field" => {
                value["unexpected"] = serde_json::json!(true);
            }
            "unsafe-authority-path" => {
                value["artifacts"][0]["authority_path"] = serde_json::json!("../escape.json");
            }
            "mixed-resource-profile" => {
                value["artifacts"][0]["resource_profile"]["max_source_bytes"] =
                    serde_json::json!(1);
            }
            "unsorted-artifacts" => {
                value["artifacts"].as_array_mut().unwrap().swap(0, 1);
            }
            "unsorted-output-paths" => {
                value["outputs"].as_array_mut().unwrap().swap(0, 1);
            }
            other => panic!("unknown differential mutation {other}"),
        }
        crate::contracts::jcs::canonicalize_value(&value).unwrap()
    }

    #[test]
    fn wp67_structural_acceptance_model_index_rust_differential_corpus() {
        let corpus: DifferentialCorpus = serde_json::from_slice(include_bytes!(
            "../../contracts/fixtures/model-index-decode-differential.json"
        ))
        .unwrap();
        for case in corpus.cases {
            assert_eq!(
                decode_model_artifact_index(&differential_bytes(&case.mutation)).is_ok(),
                case.accepted,
                "{}",
                case.mutation
            );
        }
    }
}
