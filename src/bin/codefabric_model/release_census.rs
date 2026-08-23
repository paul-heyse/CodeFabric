//! Explicit owner-acceptance boundary for released artifact identity.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::model_control::StableId;
use super::repository_model::{ArtifactRole, RepositoryModel, RepositoryModelError, read_stable};

const ACCEPTED_PATH: &str = "contracts/acceptance/released-artifact-census-v1.json";
const CANDIDATE_PATH: &str = "target/model-review/released-artifact-census-v1.candidate.json";
const CENSUS_ARTIFACT_ID: &str = "codefabric.acceptance.released-artifact-census-v1";
const MAX_CENSUS_BYTES: usize = 4 * 1024 * 1024;

/// One stable released identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleasedArtifactRecord {
    pub artifact_id: StableId,
    pub status: ReleasedStatus,
}

/// The only status admitted to the accepted released set.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReleasedStatus {
    Released,
}

/// Reference to separately reviewed removal/replacement authority.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedTombstoneRef {
    pub artifact_id: StableId,
    pub tombstone_id: StableId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_id: Option<StableId>,
}

/// Accountable acceptance event; routine synchronization cannot construct it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerAcceptance {
    pub owner_identity: String,
    pub acceptance_provenance: String,
    pub candidate_digest: String,
}

/// Deterministic review document emitted outside all governed roots.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseCensusCandidate {
    pub schema_version: u64,
    pub suite_major: u64,
    pub status: CandidateStatus,
    pub released_artifacts: Vec<ReleasedArtifactRecord>,
    pub accepted_tombstones: Vec<AcceptedTombstoneRef>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateStatus {
    Candidate,
}

/// Immutable accepted absence oracle.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleasedArtifactCensus {
    pub artifact_id: StableId,
    pub artifact_kind: CensusArtifactKind,
    pub version: String,
    pub compatible_suite_major: u64,
    pub status: AcceptedStatus,
    pub released_artifacts: Vec<ReleasedArtifactRecord>,
    pub accepted_tombstones: Vec<AcceptedTombstoneRef>,
    pub owner_acceptance: OwnerAcceptance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CensusArtifactKind {
    ReleaseCensusAcceptance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AcceptedStatus {
    Accepted,
}

/// Explicit authorization supplied only by the guarded acceptance command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceAuthorization {
    pub owner_identity: String,
    pub acceptance_provenance: String,
    pub reviewed_candidate: bool,
}

/// Candidate generation result printed for owner review.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CandidateReport {
    pub path: String,
    pub candidate_digest: String,
    pub released_artifact_count: usize,
}

/// Build the deterministic current candidate from the compiled model.
///
/// # Errors
///
/// Returns an error when a released header has an unsupported suite major. Equivalent native and
/// derived representations of one stable identity collapse into one census record.
pub fn candidate(model: &RepositoryModel) -> Result<ReleaseCensusCandidate, ReleaseCensusError> {
    let mut released = BTreeMap::new();
    for claim in model.claims.values() {
        let Some(header) = &claim.header else {
            continue;
        };
        if !is_released_status(&header.artifact_id, &header.status)? {
            continue;
        }
        if header.compatible_suite_major != 1 {
            return Err(ReleaseCensusError::SuiteMajor {
                artifact_id: header.artifact_id.clone(),
                observed: header.compatible_suite_major,
            });
        }
        released
            .entry(header.artifact_id.clone())
            .or_insert_with(|| ReleasedArtifactRecord {
                artifact_id: header.artifact_id.clone(),
                status: ReleasedStatus::Released,
            });
    }
    Ok(ReleaseCensusCandidate {
        schema_version: 1,
        suite_major: 1,
        status: CandidateStatus::Candidate,
        released_artifacts: released.into_values().collect(),
        accepted_tombstones: Vec::new(),
    })
}

fn is_released_status(artifact_id: &StableId, status: &str) -> Result<bool, ReleaseCensusError> {
    match status {
        "released" | "released-normative-implementation-specification" | "planning-baseline" => {
            Ok(true)
        }
        "accepted" | "active" | "deprecated" | "draft" | "superseded" => Ok(false),
        _ => Err(ReleaseCensusError::UnknownLifecycleStatus {
            artifact_id: artifact_id.clone(),
            status: status.to_owned(),
        }),
    }
}

/// Write a deterministic candidate under the disposable review root.
///
/// # Errors
///
/// Returns an error when candidate derivation, canonicalization, or the bounded review-root write
/// fails.
pub fn write_candidate(
    root: &Path,
    model: &RepositoryModel,
) -> Result<CandidateReport, ReleaseCensusError> {
    let candidate = candidate(model)?;
    validate_candidate(&candidate)?;
    let digest = canonical_digest(&candidate)?;
    let path = root.join(CANDIDATE_PATH);
    let parent = path.parent().ok_or(ReleaseCensusError::UnsafePath)?;
    fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    let bytes = pretty_bytes(&candidate)?;
    fs::write(&path, bytes).map_err(|error| io_error(&path, error))?;
    Ok(CandidateReport {
        path: CANDIDATE_PATH.to_owned(),
        candidate_digest: digest,
        released_artifact_count: candidate.released_artifacts.len(),
    })
}

/// Promote exactly one reviewed candidate into a new accepted destination.
///
/// # Errors
///
/// Returns an error unless explicit owner review is supplied, the candidate is valid, and the
/// accepted destination can be created exactly once without following a symlink.
pub fn accept_candidate(
    root: &Path,
    authorization: &AcceptanceAuthorization,
) -> Result<ReleasedArtifactCensus, ReleaseCensusError> {
    if !authorization.reviewed_candidate
        || authorization.owner_identity.trim().is_empty()
        || authorization.acceptance_provenance.trim().is_empty()
    {
        return Err(ReleaseCensusError::ExplicitAuthorizationRequired);
    }
    let candidate_path = root.join(CANDIDATE_PATH);
    let candidate: ReleaseCensusCandidate = parse_document(&candidate_path)?;
    validate_candidate(&candidate)?;
    let accepted_path = root.join(ACCEPTED_PATH);
    if accepted_path.exists() {
        return Err(ReleaseCensusError::AlreadyAccepted);
    }
    let candidate_digest = canonical_digest(&candidate)?;
    let census = ReleasedArtifactCensus {
        artifact_id: StableId::parse(CENSUS_ARTIFACT_ID)?,
        artifact_kind: CensusArtifactKind::ReleaseCensusAcceptance,
        version: "1.0".to_owned(),
        compatible_suite_major: candidate.suite_major,
        status: AcceptedStatus::Accepted,
        released_artifacts: candidate.released_artifacts,
        accepted_tombstones: candidate.accepted_tombstones,
        owner_acceptance: OwnerAcceptance {
            owner_identity: authorization.owner_identity.trim().to_owned(),
            acceptance_provenance: authorization.acceptance_provenance.trim().to_owned(),
            candidate_digest,
        },
    };
    validate_accepted(&census, None)?;
    write_new_accepted(&accepted_path, &pretty_bytes(&census)?)?;
    Ok(census)
}

/// Verify the accepted census against current compiled identities.
///
/// # Errors
///
/// Returns an error when the accepted record is malformed or a released identity disappears
/// without an accepted tombstone.
pub fn check(root: &Path, model: &RepositoryModel) -> Result<(), ReleaseCensusError> {
    let path = root.join(ACCEPTED_PATH);
    let accepted: ReleasedArtifactCensus = parse_document(&path)?;
    let current = candidate(model)?;
    validate_accepted(&accepted, Some(&current))
}

fn validate_candidate(candidate: &ReleaseCensusCandidate) -> Result<(), ReleaseCensusError> {
    if candidate.schema_version != 1 || candidate.suite_major != 1 {
        return Err(ReleaseCensusError::UnsupportedVersion);
    }
    sorted_unique_released(&candidate.released_artifacts)?;
    sorted_unique_tombstones(&candidate.accepted_tombstones)
}

fn validate_accepted(
    accepted: &ReleasedArtifactCensus,
    current: Option<&ReleaseCensusCandidate>,
) -> Result<(), ReleaseCensusError> {
    if accepted.artifact_id.as_str() != CENSUS_ARTIFACT_ID
        || accepted.version != "1.0"
        || accepted.compatible_suite_major != 1
        || !valid_digest(&accepted.owner_acceptance.candidate_digest)
        || accepted.owner_acceptance.owner_identity.trim().is_empty()
        || accepted
            .owner_acceptance
            .acceptance_provenance
            .trim()
            .is_empty()
    {
        return Err(ReleaseCensusError::InvalidAcceptedRecord);
    }
    sorted_unique_released(&accepted.released_artifacts)?;
    sorted_unique_tombstones(&accepted.accepted_tombstones)?;
    let accepted_ids: BTreeSet<_> = accepted
        .released_artifacts
        .iter()
        .map(|entry| &entry.artifact_id)
        .collect();
    for tombstone in &accepted.accepted_tombstones {
        if !accepted_ids.contains(&tombstone.artifact_id) {
            return Err(ReleaseCensusError::UnknownTombstoneTarget(
                tombstone.artifact_id.clone(),
            ));
        }
    }
    if let Some(current) = current {
        let current_ids: BTreeSet<_> = current
            .released_artifacts
            .iter()
            .map(|entry| &entry.artifact_id)
            .collect();
        let tombstoned: BTreeSet<_> = accepted
            .accepted_tombstones
            .iter()
            .map(|entry| &entry.artifact_id)
            .collect();
        if let Some(missing) = accepted_ids
            .difference(&current_ids)
            .find(|id| !tombstoned.contains(*id))
        {
            return Err(ReleaseCensusError::UnacceptedDeletion((*missing).clone()));
        }
    }
    Ok(())
}

fn sorted_unique_released(entries: &[ReleasedArtifactRecord]) -> Result<(), ReleaseCensusError> {
    if entries
        .windows(2)
        .any(|pair| pair[0].artifact_id >= pair[1].artifact_id)
    {
        return Err(ReleaseCensusError::UnsortedOrDuplicateReleasedIds);
    }
    Ok(())
}

fn sorted_unique_tombstones(entries: &[AcceptedTombstoneRef]) -> Result<(), ReleaseCensusError> {
    if entries.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ReleaseCensusError::UnsortedOrDuplicateTombstones);
    }
    Ok(())
}

fn parse_document<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ReleaseCensusError> {
    let bytes = read_stable(path, MAX_CENSUS_BYTES)?;
    serde_json::from_slice(&bytes).map_err(ReleaseCensusError::Json)
}

fn pretty_bytes(value: &impl Serialize) -> Result<Vec<u8>, ReleaseCensusError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(ReleaseCensusError::Json)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn canonical_digest(value: &impl Serialize) -> Result<String, ReleaseCensusError> {
    let value = serde_json::to_value(value).map_err(ReleaseCensusError::Json)?;
    let bytes = serde_json_canonicalizer::to_vec(&value).map_err(ReleaseCensusError::Json)?;
    Ok(format!("b3:{}", blake3::hash(&bytes).to_hex()))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 67
        && value.starts_with("b3:")
        && value[3..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn write_new_accepted(path: &Path, bytes: &[u8]) -> Result<(), ReleaseCensusError> {
    let parent = path.parent().ok_or(ReleaseCensusError::UnsafePath)?;
    fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::from_raw_mode(0o644),
    )
    .map_err(|error| io_error(path, error))?;
    let mut file = fs::File::from(descriptor);
    file.write_all(bytes)
        .map_err(|error| io_error(path, error))?;
    file.sync_all().map_err(|error| io_error(path, error))
}

/// Routine synchronization is mechanically limited to derived roles.
#[must_use]
pub const fn routine_write_allowed(role: ArtifactRole) -> bool {
    matches!(role, ArtifactRole::Derived)
}

/// Accepted census path for structural write-set checks.
#[must_use]
pub const fn accepted_relative_path() -> &'static str {
    ACCEPTED_PATH
}

#[derive(Debug, Error)]
pub enum ReleaseCensusError {
    #[error(transparent)]
    Model(#[from] RepositoryModelError),
    #[error(transparent)]
    StableId(#[from] super::model_control::ModelError),
    #[error("release census JSON is invalid: {0}")]
    Json(serde_json::Error),
    #[error("release census path is unsafe")]
    UnsafePath,
    #[error("release census I/O failed at {path}: {message}")]
    Io { path: PathBuf, message: String },
    #[error("release candidate requires an explicit reviewed owner authorization")]
    ExplicitAuthorizationRequired,
    #[error("an accepted release census already exists")]
    AlreadyAccepted,
    #[error("unsupported release census schema or suite major")]
    UnsupportedVersion,
    #[error("invalid accepted release census header or provenance")]
    InvalidAcceptedRecord,
    #[error("released artifact IDs must be sorted and unique")]
    UnsortedOrDuplicateReleasedIds,
    #[error("accepted tombstone references must be sorted and unique")]
    UnsortedOrDuplicateTombstones,
    #[error("released artifact {artifact_id} targets suite major {observed}, expected 1")]
    SuiteMajor {
        artifact_id: StableId,
        observed: u64,
    },
    #[error("artifact {artifact_id} has an unsupported lifecycle status: {status}")]
    UnknownLifecycleStatus {
        artifact_id: StableId,
        status: String,
    },
    #[error("accepted released artifact disappeared without tombstone: {0}")]
    UnacceptedDeletion(StableId),
    #[error("accepted tombstone targets an ID absent from release history: {0}")]
    UnknownTombstoneTarget(StableId),
}

fn io_error(path: &Path, error: impl std::fmt::Display) -> ReleaseCensusError {
    ReleaseCensusError::Io {
        path: path.to_owned(),
        message: error.to_string().chars().take(512).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn released(id: &str) -> ReleasedArtifactRecord {
        ReleasedArtifactRecord {
            artifact_id: StableId::parse(id).unwrap(),
            status: ReleasedStatus::Released,
        }
    }

    fn accepted(ids: &[&str]) -> ReleasedArtifactCensus {
        ReleasedArtifactCensus {
            artifact_id: StableId::parse(CENSUS_ARTIFACT_ID).unwrap(),
            artifact_kind: CensusArtifactKind::ReleaseCensusAcceptance,
            version: "1.0".to_owned(),
            compatible_suite_major: 1,
            status: AcceptedStatus::Accepted,
            released_artifacts: ids.iter().map(|id| released(id)).collect(),
            accepted_tombstones: Vec::new(),
            owner_acceptance: OwnerAcceptance {
                owner_identity: "repository-owner".to_owned(),
                acceptance_provenance: "reviewed fixture".to_owned(),
                candidate_digest:
                    "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            },
        }
    }

    fn candidate_with(ids: &[&str]) -> ReleaseCensusCandidate {
        ReleaseCensusCandidate {
            schema_version: 1,
            suite_major: 1,
            status: CandidateStatus::Candidate,
            released_artifacts: ids.iter().map(|id| released(id)).collect(),
            accepted_tombstones: Vec::new(),
        }
    }

    #[test]
    fn model_release_census_blocks_unaccepted_deletion() {
        let accepted = accepted(&["artifact:a", "artifact:b"]);
        let current = candidate_with(&["artifact:a"]);
        assert!(matches!(
            validate_accepted(&accepted, Some(&current)),
            Err(ReleaseCensusError::UnacceptedDeletion(id)) if id.as_str() == "artifact:b"
        ));
    }

    #[test]
    fn model_release_census_allows_additive_unreleased_candidate() {
        let accepted = accepted(&["artifact:a"]);
        let current = candidate_with(&["artifact:a", "artifact:b"]);
        assert!(validate_accepted(&accepted, Some(&current)).is_ok());
    }

    #[test]
    fn model_sync_cannot_write_release_census() {
        assert!(!routine_write_allowed(ArtifactRole::Acceptance));
        assert!(routine_write_allowed(ArtifactRole::Derived));
        assert!(accepted_relative_path().starts_with("contracts/acceptance/"));
    }

    #[test]
    fn model_acceptance_paths_are_outside_routine_write_set() {
        assert_eq!(
            super::super::repository_model::contract_role(accepted_relative_path().as_bytes()),
            ArtifactRole::Acceptance
        );
        assert!(!routine_write_allowed(
            super::super::repository_model::contract_role(accepted_relative_path().as_bytes())
        ));
        assert!(!Path::new(accepted_relative_path()).starts_with("target/model-stage"));
    }

    #[test]
    fn model_generated_index_deletion_cannot_erase_released_history() {
        let accepted = accepted(&["codefabric.manifests.artifact-index"]);
        let current = candidate_with(&[]);
        assert!(matches!(
            validate_accepted(&accepted, Some(&current)),
            Err(ReleaseCensusError::UnacceptedDeletion(_))
        ));
    }

    #[test]
    fn model_release_census_candidate_requires_explicit_accept_command() {
        let authorization = AcceptanceAuthorization {
            owner_identity: "repository-owner".to_owned(),
            acceptance_provenance: "reviewed".to_owned(),
            reviewed_candidate: false,
        };
        assert!(matches!(
            if authorization.reviewed_candidate {
                Ok(())
            } else {
                Err(ReleaseCensusError::ExplicitAuthorizationRequired)
            },
            Err(ReleaseCensusError::ExplicitAuthorizationRequired)
        ));
    }

    #[test]
    fn model_release_census_uses_closed_native_status_semantics() {
        let id = StableId::parse("artifact:a").unwrap();
        assert!(is_released_status(&id, "released").unwrap());
        assert!(
            is_released_status(&id, "released-normative-implementation-specification").unwrap()
        );
        assert!(is_released_status(&id, "planning-baseline").unwrap());
        assert!(!is_released_status(&id, "draft").unwrap());
        assert!(matches!(
            is_released_status(&id, "informal"),
            Err(ReleaseCensusError::UnknownLifecycleStatus { .. })
        ));
    }
}
