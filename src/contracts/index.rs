//! Typed, cached access to the one packaged contract artifact-index resource.

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::catalog::{
    ArtifactKind, ArtifactStatus, CompatibilityFamily, ConsumerDomain, ContractOwner,
    DigestProjection, GeneratedOutputKind, GeneratedOutputProducer, ProvenanceRequirement,
};
use super::jcs::{canonicalize_slice, checksum, validate_checksum};

/// Exact bytes compiled into Rust and packaged unchanged by the Python adapter.
pub static ARTIFACT_INDEX_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/artifact-index.json"
));

static INDEX: OnceLock<Result<ArtifactIndex, ArtifactIndexError>> = OnceLock::new();
static INDEX_DIGEST: OnceLock<String> = OnceLock::new();

/// Generator provenance for the canonical index resource.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactIndexGeneration {
    /// Source catalog identity.
    pub catalog_artifact_id: String,
    /// Number of source records encoded below.
    pub artifact_count: usize,
    /// Generator implementation revision.
    pub generator_revision: String,
    /// Canonical JSON profile used for these exact bytes.
    pub profile: String,
}

/// One catalog-declared generated derivation edge.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactIndexOutput {
    /// Repository-relative output path.
    pub path: PathBuf,
    /// Closed output representation.
    pub output_kind: GeneratedOutputKind,
    /// Closed generator dispatch.
    pub producer: GeneratedOutputProducer,
    /// Domains which consume or package the output.
    pub consumers: Vec<ConsumerDomain>,
}

/// One fully compiled governed-source identity and consumer view.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactIndexRecord {
    /// Stable catalog identity.
    pub artifact_id: String,
    /// Repository-relative native authority.
    pub authority_path: PathBuf,
    /// Native source kind.
    pub artifact_kind: ArtifactKind,
    /// Permanent contract owner.
    pub owner: ContractOwner,
    /// Public artifact version.
    pub version: String,
    /// Compatible suite major.
    pub compatible_suite_major: u16,
    /// Release state.
    pub status: ArtifactStatus,
    /// Named semantic projection.
    pub digest_projection: DigestProjection,
    /// Compiled semantic identity.
    pub canonical_digest: String,
    /// Exact checked-in source identity.
    pub source_digest: String,
    /// Distinct AC-G-07 identity for bundle artifacts.
    pub bundle_digest: Option<String>,
    /// Independently negotiated compatibility family.
    pub compatibility_family: CompatibilityFamily,
    /// Required provenance views.
    pub provenance_requirements: Vec<ProvenanceRequirement>,
    /// Direct source consumers.
    pub consumers: Vec<ConsumerDomain>,
    /// Catalog-owned generated output edges.
    pub generated_outputs: Vec<ArtifactIndexOutput>,
}

/// Canonical shared artifact-index document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactIndex {
    /// Generator provenance.
    #[serde(rename = "_generated")]
    pub generated: ArtifactIndexGeneration,
    /// Records sorted by stable artifact identity.
    pub artifacts: Vec<ArtifactIndexRecord>,
}

/// Packaged artifact-index validation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ArtifactIndexError {
    /// Resource bytes were not strict canonical JSON.
    #[error("artifact index is not canonical JSON: {0}")]
    Canonical(String),
    /// Resource bytes did not decode into the closed model.
    #[error("artifact index typed decode failed: {0}")]
    Decode(String),
    /// A record carried a malformed or duplicate identity.
    #[error("artifact index invariant failed: {0}")]
    Invariant(String),
}

fn decode_index() -> Result<ArtifactIndex, ArtifactIndexError> {
    let canonical = canonicalize_slice(ARTIFACT_INDEX_BYTES)
        .map_err(|error| ArtifactIndexError::Canonical(error.to_string()))?;
    if canonical != ARTIFACT_INDEX_BYTES {
        return Err(ArtifactIndexError::Canonical(
            "resource bytes differ from RFC 8785 emission".to_owned(),
        ));
    }
    let index: ArtifactIndex = serde_json::from_slice(ARTIFACT_INDEX_BYTES)
        .map_err(|error| ArtifactIndexError::Decode(error.to_string()))?;
    if index.generated.artifact_count != index.artifacts.len() {
        return Err(ArtifactIndexError::Invariant(
            "generated artifact_count disagrees with records".to_owned(),
        ));
    }
    let mut previous = None;
    for record in &index.artifacts {
        if previous.is_some_and(|value: &str| value >= record.artifact_id.as_str()) {
            return Err(ArtifactIndexError::Invariant(
                "artifact IDs are not unique and strictly sorted".to_owned(),
            ));
        }
        validate_checksum(&record.canonical_digest)
            .map_err(|error| ArtifactIndexError::Invariant(error.to_string()))?;
        validate_checksum(&record.source_digest)
            .map_err(|error| ArtifactIndexError::Invariant(error.to_string()))?;
        if let Some(digest) = &record.bundle_digest {
            validate_checksum(digest)
                .map_err(|error| ArtifactIndexError::Invariant(error.to_string()))?;
        }
        previous = Some(record.artifact_id.as_str());
    }
    Ok(index)
}

/// Decode and validate the packaged index exactly once.
///
/// # Errors
///
/// Returns a stable owned error when the compiled resource is non-canonical or violates the
/// closed index model.
pub fn artifact_index() -> Result<&'static ArtifactIndex, ArtifactIndexError> {
    match INDEX.get_or_init(decode_index) {
        Ok(index) => Ok(index),
        Err(error) => Err(error.clone()),
    }
}

/// BLAKE3 identity of the exact packaged index bytes, computed once.
#[must_use]
pub fn artifact_index_digest() -> &'static str {
    INDEX_DIGEST
        .get_or_init(|| checksum(ARTIFACT_INDEX_BYTES))
        .as_str()
}
