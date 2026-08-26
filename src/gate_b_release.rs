//! Accountable Gate B acceptance and immutable golden-corpus release.
//!
//! Candidate generation has no path to these writes. The only production entry point requires
//! an explicit reviewed authorization whose identity is checked against a separate authority
//! registry. Released corpus versions are created once and never overwritten.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::contracts::jcs::canonicalize_slice;
use crate::gate_b_candidate::{
    GateBCandidateError, check_released_candidate_payload, read_candidate_artifact,
    verify_candidate_bundle,
};
use crate::golden_corpus::{
    CORPUS_INDEX_ARTIFACT_ID, CORPUS_INDEX_PATH, CorpusError, CorpusIndex, CorpusIndexEntry,
    CorpusManifest, CorpusStatus, CorpusSupersedes, CoverageProfile, GATE_B_PROFILE_ID,
    LEGACY_CORPUS_DIRECTORY, LEGACY_CORPUS_ID, LEGACY_CORPUS_VERSION, OwnerAcceptance,
    RELEASED_CORPUS_DIRECTORY, RELEASED_CORPUS_ID, RELEASED_CORPUS_VERSION,
    compute_required_profile, current_released_corpus_root, execute_gate_b_artifacts,
    validate_profile,
};

pub const CANDIDATE_DIRECTORY: &str =
    "tests/golden/review-candidates/codefabric-golden-v2.0.0-candidate.1";
pub const ACCEPTANCE_ARTIFACT: &str = "tests/golden/codefabric-golden-v2/owner-acceptance.json";
pub const AUTHORITY_REGISTRY: &str = "tests/golden/acceptance-authorities/gate-b-owner-v1.json";

const CANDIDATE_FILE: &str = "candidate.json";
const CANDIDATE_MANIFEST_FILE: &str = "candidate-manifest.json";
const CANDIDATE_DIGEST_FILE: &str = "candidate-digest.json";
const EXPECTED_DIFF_FILE: &str = "expected-vs-candidate-diff.json";
const ACCEPTANCE_FILE: &str = "owner-acceptance.json";
const MANIFEST_FILE: &str = "corpus-manifest.json";
const ACCEPTANCE_ARTIFACT_ID: &str = "codefabric.acceptance.gate-b-v2";
const EXPECTED_CANDIDATE_ID: &str = "codefabric-golden-v2.0.0-candidate.1";
const EXPECTED_AUTHORITY_ID: &str = "codefabric.acceptance.gate-b-authority-v1";
const EXPECTED_OWNER_IDENTITY: &str = "codefabric-repository-owner";
const RELEASE_STAGE: &str = "target/gate-b-owner-accept-stage";

#[derive(Debug, Error)]
pub enum GateBReleaseError {
    #[error(transparent)]
    Candidate(#[from] GateBCandidateError),
    #[error(transparent)]
    Corpus(#[from] CorpusError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("Gate B release I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Gate B release invariant failed: {0}")]
    Invariant(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceAuthorization {
    pub owner_identity: String,
    pub acceptance_provenance: String,
    pub accepted_at_unix_seconds: u64,
    pub reviewed_candidate: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct AcceptanceAuthority {
    schema_version: u16,
    artifact_id: String,
    artifact_kind: String,
    version: String,
    status: String,
    owner_identity: String,
    scope: String,
    allowed_decisions: Vec<AcceptanceDecision>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum AcceptanceDecision {
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum AcceptanceStatus {
    Accepted,
}

#[derive(Clone, Debug, Deserialize)]
struct CandidateManifestView {
    schema_version: u16,
    candidate_id: String,
    candidate_status: String,
    proposed_corpus_version: String,
    supersedes_corpus_id: String,
    supersedes_corpus_version: String,
    scenario_count: usize,
    gate_b_item_count: usize,
    expectation_inputs: BTreeMap<String, String>,
    owner_acceptance: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
struct CandidatePayloadView {
    schema_version: u16,
    candidate_id: String,
    candidate_status: String,
    proposed_corpus_version: String,
    scenario_executions: Vec<Value>,
    gate_b_items: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize)]
struct CandidateDiffView {
    schema_version: u16,
    candidate_id: String,
    all_expected_items_match: bool,
    groups: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize)]
struct CandidateDigestView {
    schema_version: u16,
    artifact_kind: String,
    domain: String,
    manifest: String,
    digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateBOwnerAcceptance {
    schema_version: u16,
    artifact_id: String,
    artifact_kind: String,
    version: String,
    status: AcceptanceStatus,
    decision: AcceptanceDecision,
    candidate_id: String,
    candidate_digest: String,
    candidate_manifest_digest: String,
    source_inputs: BTreeMap<String, String>,
    authority_id: String,
    authority_registry_digest: String,
    owner_identity: String,
    accepted_at_unix_seconds: u64,
    acceptance_provenance: String,
    supersedes: CorpusSupersedes,
    released_corpus_id: String,
    released_corpus_version: String,
    released_profile_id: String,
    released_profile_digest: String,
    acceptance_digest: String,
}

fn invariant(message: impl std::fmt::Display) -> GateBReleaseError {
    GateBReleaseError::Invariant(message.to_string())
}

fn io_error(path: &Path, source: std::io::Error) -> GateBReleaseError {
    GateBReleaseError::Io {
        path: path.to_owned(),
        source,
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 67
        && value.starts_with("b3:")
        && value[3..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn canonical_bytes(value: &impl Serialize) -> Result<Vec<u8>, GateBReleaseError> {
    let mut bytes = canonicalize_slice(&serde_json::to_vec(value)?).map_err(invariant)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), GateBReleaseError> {
    let parent = path
        .parent()
        .ok_or_else(|| invariant("release output has no parent"))?;
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
    .map_err(|error| {
        invariant(format!(
            "exclusive create failed at {}: {error}",
            path.display()
        ))
    })?;
    let mut file = fs::File::from(descriptor);
    file.write_all(bytes)
        .map_err(|error| io_error(path, error))?;
    file.sync_all().map_err(|error| io_error(path, error))
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), GateBReleaseError> {
    fs::create_dir(destination).map_err(|error| io_error(destination, error))?;
    let mut entries = fs::read_dir(source)
        .map_err(|error| io_error(source, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(source, error))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let kind = entry
            .file_type()
            .map_err(|error| io_error(&entry.path(), error))?;
        if kind.is_symlink() {
            return Err(invariant(format!(
                "release input contains a symlink: {}",
                entry.path().display()
            )));
        }
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target).map_err(|error| io_error(&target, error))?;
        } else {
            return Err(invariant("release input contains a non-file member"));
        }
    }
    Ok(())
}

fn decode<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, GateBReleaseError> {
    Ok(serde_json::from_slice(&read_candidate_artifact(path)?)?)
}

fn acceptance_digest(acceptance: &GateBOwnerAcceptance) -> Result<String, GateBReleaseError> {
    let mut projection = serde_json::to_value(acceptance)?;
    projection
        .as_object_mut()
        .ok_or_else(|| invariant("acceptance projection is not an object"))?
        .remove("acceptance_digest");
    let canonical = canonicalize_slice(&serde_json::to_vec(&projection)?).map_err(invariant)?;
    let mut hasher = crate::integrity::IntegrityHasher::for_domain(
        crate::integrity::IntegrityDomain::GateBOwnerAcceptance,
    );
    hasher.update(&(canonical.len() as u64).to_be_bytes());
    hasher.update(&canonical);
    Ok(crate::integrity::frame_digest(hasher.finalize()))
}

fn authority(path: &Path) -> Result<(AcceptanceAuthority, String), GateBReleaseError> {
    let bytes = read_candidate_artifact(path)?;
    let authority: AcceptanceAuthority = serde_json::from_slice(&bytes)?;
    if authority.schema_version != 1
        || authority.artifact_id != EXPECTED_AUTHORITY_ID
        || authority.artifact_kind != "gate-b-acceptance-authority"
        || authority.version != "1.0"
        || authority.status != "released"
        || authority.owner_identity != EXPECTED_OWNER_IDENTITY
        || authority.scope != "codefabric-golden-v2"
        || authority.allowed_decisions != [AcceptanceDecision::Accepted]
    {
        return Err(invariant("Gate B acceptance authority differs"));
    }
    let digest = crate::integrity::framed_digest(&bytes);
    Ok((authority, digest))
}

fn candidate_views(
    candidate_root: &Path,
) -> Result<
    (
        CandidateManifestView,
        CandidatePayloadView,
        CandidateDiffView,
        CandidateDigestView,
    ),
    GateBReleaseError,
> {
    verify_candidate_bundle(candidate_root)?;
    let manifest: CandidateManifestView = decode(&candidate_root.join(CANDIDATE_MANIFEST_FILE))?;
    let payload: CandidatePayloadView = decode(&candidate_root.join(CANDIDATE_FILE))?;
    let diff: CandidateDiffView = decode(&candidate_root.join(EXPECTED_DIFF_FILE))?;
    let detached: CandidateDigestView = decode(&candidate_root.join(CANDIDATE_DIGEST_FILE))?;
    let groups = payload
        .gate_b_items
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if manifest.schema_version != 1
        || payload.schema_version != 1
        || diff.schema_version != 1
        || detached.schema_version != 1
        || manifest.candidate_id != EXPECTED_CANDIDATE_ID
        || payload.candidate_id != EXPECTED_CANDIDATE_ID
        || diff.candidate_id != EXPECTED_CANDIDATE_ID
        || manifest.candidate_status != "CANDIDATE"
        || payload.candidate_status != "CANDIDATE"
        || manifest.proposed_corpus_version != RELEASED_CORPUS_VERSION
        || payload.proposed_corpus_version != RELEASED_CORPUS_VERSION
        || manifest.supersedes_corpus_id != LEGACY_CORPUS_ID
        || manifest.supersedes_corpus_version != LEGACY_CORPUS_VERSION
        || manifest.owner_acceptance.is_some()
        || manifest.scenario_count != 16
        || payload.scenario_executions.len() != 16
        || manifest.gate_b_item_count != 11
        || groups
            != crate::golden_corpus::REQUIRED_EXPECTED_GROUPS
                .into_iter()
                .map(str::to_owned)
                .collect()
        || !diff.all_expected_items_match
        || diff.groups.keys().cloned().collect::<BTreeSet<_>>() != groups
        || detached.artifact_kind != "detached-gate-b-review-candidate-digest"
        || detached.domain != "GATE_B_REVIEW_CANDIDATE"
        || detached.manifest != CANDIDATE_MANIFEST_FILE
        || !valid_digest(&detached.digest)
    {
        return Err(invariant("candidate release projection differs"));
    }
    Ok((manifest, payload, diff, detached))
}

#[allow(clippy::too_many_lines)] // One ordered release transaction keeps the authority, acceptance, corpus, and index chain auditable.
fn build_release(
    repository_root: &Path,
    candidate_root: &Path,
    staged_corpus: &Path,
    authorization: &AcceptanceAuthorization,
) -> Result<(GateBOwnerAcceptance, CorpusIndex), GateBReleaseError> {
    let (candidate_manifest, payload, _diff, candidate_digest) = candidate_views(candidate_root)?;
    let (authority, authority_registry_digest) =
        authority(&repository_root.join(AUTHORITY_REGISTRY))?;
    if !authorization.reviewed_candidate
        || authorization.owner_identity != authority.owner_identity
        || authorization.acceptance_provenance.trim().is_empty()
        || authorization.accepted_at_unix_seconds == 0
    {
        return Err(invariant(
            "explicit authorization from the registered accountable owner is required",
        ));
    }

    fs::create_dir(staged_corpus).map_err(|error| io_error(staged_corpus, error))?;
    let legacy = repository_root.join(LEGACY_CORPUS_DIRECTORY);
    copy_directory(&legacy.join("workspace"), &staged_corpus.join("workspace"))?;
    copy_directory(&legacy.join("scenarios"), &staged_corpus.join("scenarios"))?;
    fs::create_dir(staged_corpus.join("expected"))
        .map_err(|error| io_error(&staged_corpus.join("expected"), error))?;
    for (group, value) in &payload.gate_b_items {
        let directory = staged_corpus.join("expected").join(group);
        fs::create_dir(&directory).map_err(|error| io_error(&directory, error))?;
        write_new(&directory.join("gate-b.json"), &canonical_bytes(value)?)?;
    }

    let (file_count, profile_digest) = compute_required_profile(staged_corpus)?;
    let legacy_manifest: CorpusManifest = decode(&legacy.join(MANIFEST_FILE))?;
    let candidate_manifest_bytes =
        read_candidate_artifact(&candidate_root.join(CANDIDATE_MANIFEST_FILE))?;
    let mut acceptance = GateBOwnerAcceptance {
        schema_version: 1,
        artifact_id: ACCEPTANCE_ARTIFACT_ID.to_owned(),
        artifact_kind: "gate-b-owner-acceptance".to_owned(),
        version: "2.0".to_owned(),
        status: AcceptanceStatus::Accepted,
        decision: AcceptanceDecision::Accepted,
        candidate_id: EXPECTED_CANDIDATE_ID.to_owned(),
        candidate_digest: candidate_digest.digest.clone(),
        candidate_manifest_digest: crate::integrity::framed_digest(&candidate_manifest_bytes),
        source_inputs: candidate_manifest.expectation_inputs,
        authority_id: authority.artifact_id,
        authority_registry_digest,
        owner_identity: authorization.owner_identity.clone(),
        accepted_at_unix_seconds: authorization.accepted_at_unix_seconds,
        acceptance_provenance: authorization.acceptance_provenance.clone(),
        supersedes: CorpusSupersedes {
            corpus_id: LEGACY_CORPUS_ID.to_owned(),
            corpus_version: LEGACY_CORPUS_VERSION.to_owned(),
        },
        released_corpus_id: RELEASED_CORPUS_ID.to_owned(),
        released_corpus_version: RELEASED_CORPUS_VERSION.to_owned(),
        released_profile_id: GATE_B_PROFILE_ID.to_owned(),
        released_profile_digest: profile_digest.clone(),
        acceptance_digest: String::new(),
    };
    acceptance.acceptance_digest = acceptance_digest(&acceptance)?;

    let manifest = CorpusManifest {
        corpus_id: RELEASED_CORPUS_ID.to_owned(),
        corpus_version: RELEASED_CORPUS_VERSION.to_owned(),
        corpus_status: CorpusStatus::Released,
        coverage_profiles: vec![CoverageProfile {
            profile_id: GATE_B_PROFILE_ID.to_owned(),
            profile_version: "2.0".to_owned(),
            profile_status: CorpusStatus::Released,
            member_roots: vec![
                "expected".to_owned(),
                "scenarios".to_owned(),
                "workspace".to_owned(),
            ],
            file_count,
            canonical_digest: profile_digest.clone(),
            owned_requirements: vec![
                "AC-G-78".to_owned(),
                "READINESS-GATE-B".to_owned(),
                "RM-W5".to_owned(),
            ],
            acceptance: OwnerAcceptance {
                accepted_by: authorization.owner_identity.clone(),
                accepted_at: authorization.accepted_at_unix_seconds.to_string(),
                acceptance_basis: authorization.acceptance_provenance.clone(),
            },
        }],
        accepted_profile_digests: vec![profile_digest.clone()],
        source_archive_digest: profile_digest.clone(),
        workspace_registration_digest: legacy_manifest.workspace_registration_digest,
        context_manifest_digests: legacy_manifest.context_manifest_digests,
        provider_bundle_digests: legacy_manifest.provider_bundle_digests,
        model_pack_bundle_digest: legacy_manifest.model_pack_bundle_digest,
        ontology_bundle_digest: legacy_manifest.ontology_bundle_digest,
        schema_bundle_digest: legacy_manifest.schema_bundle_digest,
        derivation_bundle_digest: legacy_manifest.derivation_bundle_digest,
        query_bundle_digest: legacy_manifest.query_bundle_digest,
        tool_contract_bundle_digest: legacy_manifest.tool_contract_bundle_digest,
        supersedes: Some(acceptance.supersedes.clone()),
        acceptance_artifact: Some(ACCEPTANCE_FILE.to_owned()),
        acceptance_digest: Some(acceptance.acceptance_digest.clone()),
        released_candidate_digest: Some(acceptance.candidate_digest.clone()),
    };
    write_new(
        &staged_corpus.join(ACCEPTANCE_FILE),
        &canonical_bytes(&acceptance)?,
    )?;
    let manifest_bytes = canonical_bytes(&manifest)?;
    write_new(&staged_corpus.join(MANIFEST_FILE), &manifest_bytes)?;
    validate_profile(staged_corpus, GATE_B_PROFILE_ID)?;

    let legacy_profile = validate_profile(&legacy, GATE_B_PROFILE_ID)?;
    let legacy_manifest_bytes = read_candidate_artifact(&legacy.join(MANIFEST_FILE))?;
    let index = CorpusIndex {
        schema_version: 1,
        artifact_id: CORPUS_INDEX_ARTIFACT_ID.to_owned(),
        artifact_kind: "golden-corpus-index".to_owned(),
        version: "1.0".to_owned(),
        status: CorpusStatus::Released,
        current_corpus_id: RELEASED_CORPUS_ID.to_owned(),
        current_corpus_version: RELEASED_CORPUS_VERSION.to_owned(),
        entries: vec![
            CorpusIndexEntry {
                corpus_id: LEGACY_CORPUS_ID.to_owned(),
                corpus_version: LEGACY_CORPUS_VERSION.to_owned(),
                corpus_status: CorpusStatus::Candidate,
                path: LEGACY_CORPUS_DIRECTORY.to_owned(),
                manifest_digest: crate::integrity::framed_digest(&legacy_manifest_bytes),
                profile_id: GATE_B_PROFILE_ID.to_owned(),
                profile_digest: legacy_profile.canonical_digest,
                acceptance_digest: None,
            },
            CorpusIndexEntry {
                corpus_id: RELEASED_CORPUS_ID.to_owned(),
                corpus_version: RELEASED_CORPUS_VERSION.to_owned(),
                corpus_status: CorpusStatus::Released,
                path: RELEASED_CORPUS_DIRECTORY.to_owned(),
                manifest_digest: crate::integrity::framed_digest(&manifest_bytes),
                profile_id: GATE_B_PROFILE_ID.to_owned(),
                profile_digest,
                acceptance_digest: Some(acceptance.acceptance_digest.clone()),
            },
        ],
    };
    Ok((acceptance, index))
}

/// Create the owner-accepted corpus and atomically publish its current-version index.
///
/// # Errors
///
/// Returns an error unless the candidate and authority chains are exact, authorization is
/// explicit, all destinations are new, and the released corpus validates before publication.
pub fn accept_candidate(
    repository_root: &Path,
    candidate_relative: &Path,
    acceptance_relative: &Path,
    authorization: &AcceptanceAuthorization,
) -> Result<GateBOwnerAcceptance, GateBReleaseError> {
    if !safe_relative(candidate_relative)
        || !safe_relative(acceptance_relative)
        || acceptance_relative != Path::new(ACCEPTANCE_ARTIFACT)
    {
        return Err(invariant("unsafe or unexpected Gate B acceptance path"));
    }
    let candidate_root = repository_root.join(candidate_relative);
    let release_root = repository_root.join(RELEASED_CORPUS_DIRECTORY);
    let index_path = repository_root.join(CORPUS_INDEX_PATH);
    let stage_root = repository_root.join(RELEASE_STAGE);
    if release_root.exists() || index_path.exists() || stage_root.exists() {
        return Err(invariant(
            "Gate B v2 release or staging output already exists",
        ));
    }
    fs::create_dir_all(&stage_root).map_err(|error| io_error(&stage_root, error))?;
    let staged_corpus = stage_root.join("codefabric-golden-v2");
    let result = (|| {
        let (acceptance, index) = build_release(
            repository_root,
            &candidate_root,
            &staged_corpus,
            authorization,
        )?;
        let index_temporary = stage_root.join("corpus-index.json");
        write_new(&index_temporary, &canonical_bytes(&index)?)?;
        let release_parent = release_root
            .parent()
            .ok_or_else(|| invariant("released corpus has no parent"))?;
        fs::create_dir_all(release_parent).map_err(|error| io_error(release_parent, error))?;
        fs::rename(&staged_corpus, &release_root)
            .map_err(|error| io_error(&release_root, error))?;
        if let Err(error) = fs::rename(&index_temporary, &index_path) {
            fs::remove_dir_all(&release_root)
                .map_err(|cleanup| io_error(&release_root, cleanup))?;
            return Err(io_error(&index_path, error));
        }
        Ok(acceptance)
    })();
    fs::remove_dir_all(&stage_root).map_err(|error| io_error(&stage_root, error))?;
    let acceptance = result?;
    verify_release_chain(repository_root)?;
    Ok(acceptance)
}

/// Verify the owner authority, acceptance, candidate, immutable corpus, and current index chain.
///
/// # Errors
///
/// Returns an error for any missing member, wrong authority, rejected or self-authored decision,
/// digest drift, corpus mutation, index mismatch, or Gate B expectation mismatch.
pub fn verify_release_chain(repository_root: &Path) -> Result<(), GateBReleaseError> {
    let candidate_root = repository_root.join(CANDIDATE_DIRECTORY);
    let (candidate_manifest, payload, _diff, candidate_digest) = candidate_views(&candidate_root)?;
    let (authority, authority_digest) = authority(&repository_root.join(AUTHORITY_REGISTRY))?;
    let corpus_root = repository_root.join(RELEASED_CORPUS_DIRECTORY);
    let acceptance: GateBOwnerAcceptance = decode(&corpus_root.join(ACCEPTANCE_FILE))?;
    let manifest: CorpusManifest = decode(&corpus_root.join(MANIFEST_FILE))?;
    let index: CorpusIndex = decode(&repository_root.join(CORPUS_INDEX_PATH))?;
    let profile = validate_profile(&corpus_root, GATE_B_PROFILE_ID)?;
    let legacy_root = repository_root.join(LEGACY_CORPUS_DIRECTORY);
    let legacy_profile = validate_profile(&legacy_root, GATE_B_PROFILE_ID)?;
    let manifest_bytes = read_candidate_artifact(&corpus_root.join(MANIFEST_FILE))?;
    let legacy_manifest_bytes = read_candidate_artifact(&legacy_root.join(MANIFEST_FILE))?;
    let candidate_manifest_bytes =
        read_candidate_artifact(&candidate_root.join(CANDIDATE_MANIFEST_FILE))?;

    if acceptance.schema_version != 1
        || acceptance.artifact_id != ACCEPTANCE_ARTIFACT_ID
        || acceptance.artifact_kind != "gate-b-owner-acceptance"
        || acceptance.version != "2.0"
        || acceptance.status != AcceptanceStatus::Accepted
        || acceptance.decision != AcceptanceDecision::Accepted
        || acceptance.candidate_id != EXPECTED_CANDIDATE_ID
        || acceptance.candidate_digest != candidate_digest.digest
        || acceptance.candidate_manifest_digest
            != crate::integrity::framed_digest(&candidate_manifest_bytes)
        || acceptance.source_inputs != candidate_manifest.expectation_inputs
        || acceptance.authority_id != authority.artifact_id
        || acceptance.authority_registry_digest != authority_digest
        || acceptance.owner_identity != authority.owner_identity
        || acceptance.accepted_at_unix_seconds == 0
        || acceptance.acceptance_provenance.trim().is_empty()
        || acceptance.supersedes.corpus_id != LEGACY_CORPUS_ID
        || acceptance.supersedes.corpus_version != LEGACY_CORPUS_VERSION
        || acceptance.released_corpus_id != RELEASED_CORPUS_ID
        || acceptance.released_corpus_version != RELEASED_CORPUS_VERSION
        || acceptance.released_profile_id != GATE_B_PROFILE_ID
        || acceptance.released_profile_digest != profile.canonical_digest
        || acceptance.acceptance_digest != acceptance_digest(&acceptance)?
        || manifest.acceptance_digest.as_deref() != Some(acceptance.acceptance_digest.as_str())
        || manifest.released_candidate_digest.as_deref()
            != Some(acceptance.candidate_digest.as_str())
    {
        return Err(invariant("Gate B owner acceptance chain differs"));
    }

    let expected_entries = vec![
        CorpusIndexEntry {
            corpus_id: LEGACY_CORPUS_ID.to_owned(),
            corpus_version: LEGACY_CORPUS_VERSION.to_owned(),
            corpus_status: CorpusStatus::Candidate,
            path: LEGACY_CORPUS_DIRECTORY.to_owned(),
            manifest_digest: crate::integrity::framed_digest(&legacy_manifest_bytes),
            profile_id: GATE_B_PROFILE_ID.to_owned(),
            profile_digest: legacy_profile.canonical_digest,
            acceptance_digest: None,
        },
        CorpusIndexEntry {
            corpus_id: RELEASED_CORPUS_ID.to_owned(),
            corpus_version: RELEASED_CORPUS_VERSION.to_owned(),
            corpus_status: CorpusStatus::Released,
            path: RELEASED_CORPUS_DIRECTORY.to_owned(),
            manifest_digest: crate::integrity::framed_digest(&manifest_bytes),
            profile_id: GATE_B_PROFILE_ID.to_owned(),
            profile_digest: profile.canonical_digest,
            acceptance_digest: Some(acceptance.acceptance_digest.clone()),
        },
    ];
    if index.schema_version != 1
        || index.artifact_id != CORPUS_INDEX_ARTIFACT_ID
        || index.artifact_kind != "golden-corpus-index"
        || index.version != "1.0"
        || index.status != CorpusStatus::Released
        || index.current_corpus_id != RELEASED_CORPUS_ID
        || index.current_corpus_version != RELEASED_CORPUS_VERSION
        || index.entries != expected_entries
    {
        return Err(invariant("golden corpus index differs"));
    }

    for (group, candidate) in payload.gate_b_items {
        let released =
            read_candidate_artifact(&corpus_root.join("expected").join(group).join("gate-b.json"))?;
        if canonicalize_slice(&released).map_err(invariant)?
            != canonicalize_slice(&serde_json::to_vec(&candidate)?).map_err(invariant)?
        {
            return Err(invariant(
                "released Gate B expected bytes differ from candidate",
            ));
        }
    }
    if execute_gate_b_artifacts(&corpus_root)?
        .artifact_digests
        .len()
        != 11
    {
        return Err(invariant("released Gate B item census differs"));
    }
    if current_released_corpus_root(repository_root)? != corpus_root {
        return Err(invariant("current corpus resolver differs"));
    }
    Ok(())
}

/// Re-execute the accepted semantic payload and compare it to the immutable released corpus.
///
/// # Errors
///
/// Returns an error when either the acceptance chain, semantic payload regeneration, or released
/// Gate B execution differs. Review-time input digests stay frozen in the accepted candidate.
pub fn check_released_gate_b(
    repository_root: &Path,
    scratch_root: &Path,
) -> Result<(), GateBReleaseError> {
    verify_release_chain(repository_root)?;
    check_released_candidate_payload(
        repository_root,
        &repository_root.join(LEGACY_CORPUS_DIRECTORY),
        scratch_root,
        &repository_root.join(CANDIDATE_DIRECTORY),
    )?;
    execute_gate_b_artifacts(&repository_root.join(RELEASED_CORPUS_DIRECTORY))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn authorization(owner: &str) -> AcceptanceAuthorization {
        AcceptanceAuthorization {
            owner_identity: owner.to_owned(),
            acceptance_provenance: "Explicit accountable-owner review of the exact WP71 bundle"
                .to_owned(),
            accepted_at_unix_seconds: 1_777_000_000,
            reviewed_candidate: true,
        }
    }

    fn fixture_repository() -> tempfile::TempDir {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        for relative in [LEGACY_CORPUS_DIRECTORY, CANDIDATE_DIRECTORY] {
            let source = repository_root().join(relative);
            let destination = root.join(relative);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            copy_directory(&source, &destination).unwrap();
        }
        let authority_source = repository_root().join(AUTHORITY_REGISTRY);
        let authority_target = root.join(AUTHORITY_REGISTRY);
        fs::create_dir_all(authority_target.parent().unwrap()).unwrap();
        fs::copy(authority_source, authority_target).unwrap();
        temporary
    }

    fn accepted_fixture() -> tempfile::TempDir {
        let temporary = fixture_repository();
        accept_candidate(
            temporary.path(),
            Path::new(CANDIDATE_DIRECTORY),
            Path::new(ACCEPTANCE_ARTIFACT),
            &authorization(EXPECTED_OWNER_IDENTITY),
        )
        .unwrap();
        temporary
    }

    #[test]
    fn wp76_behavioral_acceptance() {
        let temporary = accepted_fixture();
        verify_release_chain(temporary.path()).unwrap();
        let execution =
            execute_gate_b_artifacts(&temporary.path().join(RELEASED_CORPUS_DIRECTORY)).unwrap();
        assert_eq!(execution.artifact_digests.len(), 11);
    }

    #[test]
    fn wp76_structural_acceptance() {
        let temporary = fixture_repository();
        let legacy_manifest_path = temporary
            .path()
            .join(LEGACY_CORPUS_DIRECTORY)
            .join(MANIFEST_FILE);
        let legacy_manifest_before = read_candidate_artifact(&legacy_manifest_path).unwrap();
        let before = validate_profile(
            &temporary.path().join(LEGACY_CORPUS_DIRECTORY),
            GATE_B_PROFILE_ID,
        )
        .unwrap();
        let accepted = accept_candidate(
            temporary.path(),
            Path::new(CANDIDATE_DIRECTORY),
            Path::new(ACCEPTANCE_ARTIFACT),
            &authorization(EXPECTED_OWNER_IDENTITY),
        )
        .unwrap();
        let after = validate_profile(
            &temporary.path().join(LEGACY_CORPUS_DIRECTORY),
            GATE_B_PROFILE_ID,
        )
        .unwrap();
        assert_eq!(before, after);
        assert_eq!(
            legacy_manifest_before,
            read_candidate_artifact(&legacy_manifest_path).unwrap()
        );
        assert!(valid_digest(&accepted.acceptance_digest));
        verify_release_chain(temporary.path()).unwrap();
    }

    #[test]
    fn wp76_negative_zero_state() {
        let unauthorized = fixture_repository();
        assert!(
            accept_candidate(
                unauthorized.path(),
                Path::new(CANDIDATE_DIRECTORY),
                Path::new(ACCEPTANCE_ARTIFACT),
                &authorization("codefabric-implementation-agent"),
            )
            .is_err()
        );
        assert!(!unauthorized.path().join(RELEASED_CORPUS_DIRECTORY).exists());

        let wrong_digest = accepted_fixture();
        let acceptance_path = wrong_digest.path().join(ACCEPTANCE_ARTIFACT);
        let mut acceptance: Value =
            serde_json::from_slice(&read_candidate_artifact(&acceptance_path).unwrap()).unwrap();
        acceptance["candidate_digest"] = Value::String(
            "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        );
        fs::write(&acceptance_path, canonical_bytes(&acceptance).unwrap()).unwrap();
        assert!(verify_release_chain(wrong_digest.path()).is_err());

        let rejected = accepted_fixture();
        let acceptance_path = rejected.path().join(ACCEPTANCE_ARTIFACT);
        let mut acceptance: Value =
            serde_json::from_slice(&read_candidate_artifact(&acceptance_path).unwrap()).unwrap();
        acceptance["decision"] = Value::String("REJECTED".to_owned());
        fs::write(&acceptance_path, canonical_bytes(&acceptance).unwrap()).unwrap();
        assert!(verify_release_chain(rejected.path()).is_err());

        let missing = accepted_fixture();
        fs::remove_file(missing.path().join(CORPUS_INDEX_PATH)).unwrap();
        assert!(verify_release_chain(missing.path()).is_err());
    }

    #[test]
    fn wp76_operational_acceptance() {
        let justfile = include_str!("../justfile");
        assert!(justfile.contains(
            "gate-b-owner-acceptance-check:\n    cargo run --locked --bin \
             codefabric-gate-b-candidate -- verify-release ."
        ));
        assert!(justfile.contains(
            "gate-b-check: gate-b-owner-acceptance-check wave5-integration-check \
             wave6-integration-check adapter-wheel-test model-release-census-check"
        ));
        assert!(justfile.contains(
            "cargo run --locked --bin codefabric-gate-b-candidate -- check-release . \
             target/gate-b-release-check-scratch"
        ));
        assert!(
            justfile.contains(
                "ci-pr: ci-fast policy sidecar-policy wave-acceptance-check gate-b-check"
            )
        );

        let temporary = accepted_fixture();
        verify_release_chain(temporary.path()).unwrap();
        assert!(temporary.path().join(RELEASED_CORPUS_DIRECTORY).is_dir());
        assert!(temporary.path().join(CORPUS_INDEX_PATH).is_file());
    }
}
