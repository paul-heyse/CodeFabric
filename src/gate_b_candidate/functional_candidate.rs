//! Decoded, behavior-first Gate B successor candidate and human review dossier.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{
    GateBCandidateError, GeneratedCandidateBundle, canonical_bytes, file_bytes, invariant, vertical,
};

pub const FUNCTIONAL_CANDIDATE_ID: &str = "codefabric-golden-v4.0.0-candidate.1";
pub const FUNCTIONAL_CANDIDATE_DIRECTORY: &str =
    "tests/golden/review-candidates/codefabric-golden-v4.0.0-candidate.1";

const DOSSIER_FILE: &str = "review-dossier.json";
const EVIDENCE_FILE: &str = "semantic-evidence.json";
const MUTANTS_FILE: &str = "semantic-mutants.json";
const MANIFEST_FILE: &str = "candidate-manifest.json";
const DIGEST_FILE: &str = "candidate-digest.json";
const MEMBERS: [&str; 5] = [
    DOSSIER_FILE,
    EVIDENCE_FILE,
    MUTANTS_FILE,
    MANIFEST_FILE,
    DIGEST_FILE,
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ReviewStatus {
    Matched,
    Limitation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ClaimDossier {
    claim_id: String,
    normative_reference: String,
    checkpoint: String,
    source_pointer: Option<Value>,
    source_anchor_text: Option<String>,
    expected_logical_answer: Value,
    decoded_actual: Value,
    semantic_diff: String,
    status: ReviewStatus,
    limitations: Vec<String>,
    mutant_ids: Vec<String>,
    convergence: Value,
    integrity: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewDossier {
    schema_version: u16,
    candidate_id: String,
    authority_id: String,
    review_status: String,
    claims: Vec<ClaimDossier>,
    queries: Value,
    causal_interventions: Vec<String>,
    limitations: Vec<String>,
    integrity_metadata_is_not_semantic_authority: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CandidateMember {
    path: String,
    digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FunctionalCandidateManifest {
    schema_version: u16,
    artifact_kind: String,
    candidate_id: String,
    candidate_status: String,
    proposed_corpus_version: String,
    expectation_authority: String,
    semantic_conformance: bool,
    limitation_count: usize,
    corpus_index_advanced: bool,
    owner_acceptance: Option<Value>,
    members: Vec<CandidateMember>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FunctionalCandidateDigest {
    schema_version: u16,
    artifact_kind: String,
    candidate_id: String,
    manifest: String,
    digest: String,
}

fn claim_source_anchor(
    repository_root: &Path,
    contract: &crate::functional_golden::FunctionalGoldenContract,
    claim: &crate::functional_golden::SemanticClaim,
) -> Result<Option<String>, GateBCandidateError> {
    let Some(selector) = claim.selector.source.as_ref() else {
        return Ok(None);
    };
    let source = contract
        .sources
        .iter()
        .find(|source| source.source_name == selector.source_name)
        .ok_or_else(|| invariant(format!("dossier source {} is absent", selector.source_name)))?;
    let anchor = source
        .anchors
        .get(&selector.anchor)
        .ok_or_else(|| invariant(format!("dossier anchor {} is absent", selector.anchor)))?;
    let contents = fs::read_to_string(
        repository_root
            .join(crate::functional_golden::FUNCTIONAL_AUTHORITY_ROOT)
            .join(&source.path),
    )?;
    let matches = contents
        .lines()
        .filter(|line| line.trim_start() == anchor)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(invariant(format!(
            "dossier anchor {}.{} is not unique",
            selector.source_name, selector.anchor
        )));
    }
    Ok(Some(matches[0].to_owned()))
}

fn evidence_for_claim(execution: &vertical::VerticalExecution, claim_id: &str) -> Value {
    let providers = &execution.planes["provider_observations"];
    let canonical = &execution.planes["canonical_tables"]["decoded_semantics"];
    let response = &execution.planes["queries"]["decoded_response"];
    match claim_id {
        "claim.py.owner" | "claim.py.return" => json!({
            "pyrefly_module": providers["pyrefly"]["decoded_semantics"]
                .as_array()
                .and_then(|modules| modules.iter().find(|module| module["module_name"] == "golden_pkg.core")),
        }),
        "claim.py.call-target" => json!({
            "canonical_relation": canonical["relations"].as_array().and_then(|rows| rows.iter().find(|row| {
                row["source_name"].as_str().is_some_and(|name| name.starts_with("golden_pkg.core:"))
                    && row["target_name"] == "golden_pkg.core.scale"
            })),
            "public_result": response["query_results"].as_array().and_then(|rows| rows.iter().find(|row| row["query_id"] == "q.relationships")),
        }),
        "claim.rust.owner" | "claim.rust.mir-branch" => json!({
            "rustc_owners": providers["rustc_mir"]["decoded_semantics"],
            "canonical_entities": canonical["entities"],
        }),
        "claim.ffi.call-target" => json!({
            "pyrefly_module": providers["pyrefly"]["decoded_semantics"]
                .as_array()
                .and_then(|modules| modules.iter().find(|module| module["module_name"] == "ffi.boundary")),
            "canonical_relation": canonical["relations"].as_array().and_then(|rows| rows.iter().find(|row| {
                row["source_name"].as_str().is_some_and(|name| name.starts_with("ffi.boundary:"))
                    && row["target_name"] == "golden_pkg.core.pipeline"
            })),
        }),
        "claim.unknown.parse" | "claim.diagnostic.parse" => json!({
            "malformed_module": providers["pyrefly"]["decoded_semantics"]
                .as_array()
                .and_then(|modules| modules.iter().find(|module| module["module_name"] == "malformed.broken")),
            "capabilities": canonical["capabilities"],
        }),
        "claim.capability.current" => json!({"capabilities": canonical["capabilities"]}),
        "claim.terminal.current" => json!({
            "execution_state": response["execution_state"],
            "availability_state": response["availability_state"],
            "completeness_state": response["completeness_state"],
        }),
        "claim.delivery.equivalent" => json!({
            "uds_artifact_equals_fastmcp": &execution.planes["mcp"]["structured_content"]["delivery"]["response"] == response,
            "event_kinds": execution.planes["rpc"]["event_kinds"],
            "artifact_persisted": execution.planes["diagnostics"]["artifact_persisted"],
        }),
        _ => json!({
            "status": "not exercised by the current functional vertical",
            "provider_module_count": providers["pyrefly"]["module_count"],
            "rustc_owner_count": providers["rustc_mir"]["owner_count"],
            "canonical_capability_row_count": canonical["capabilities"].as_array().map_or(0, Vec::len)
        }),
    }
}

fn decoded_evidence(execution: &vertical::VerticalExecution) -> Value {
    json!({
        "source_inventory": execution.planes["source_inventory"],
        "provider_observations": execution.planes["provider_observations"],
        "canonical_semantics": execution.planes["canonical_tables"]["decoded_semantics"],
        "publication": execution.planes["publications"],
        "serving_snapshot": execution.planes["serving_snapshots"],
        "query_response": execution.planes["queries"]["decoded_response"],
        "rpc": execution.planes["rpc"],
        "fastmcp": execution.planes["mcp"],
        "diagnostics": execution.planes["diagnostics"],
        "rebuild_comparison": execution.planes["rebuild_comparison"],
        "integrity": {
            "execution_digest": execution.execution_digest,
            "publication_id": execution.publication_id,
            "snapshot_id": execution.snapshot_id,
        }
    })
}

fn dossier(
    repository_root: &Path,
    contract: &crate::functional_golden::FunctionalGoldenContract,
    execution: &vertical::VerticalExecution,
    validated: &BTreeSet<String>,
) -> Result<ReviewDossier, GateBCandidateError> {
    let convergence = execution.planes["rebuild_comparison"].clone();
    let integrity = json!({
        "execution_digest": execution.execution_digest,
        "semantic_authority": false,
    });
    let mut claims = Vec::with_capacity(contract.claims.len());
    for claim in &contract.claims {
        let matched = validated.contains(&claim.claim_id);
        let limitations = if matched {
            Vec::new()
        } else {
            vec![format!(
                "{} is authored and mutant-covered but is not exercised by the current real-provider/public vertical",
                claim.claim_id
            )]
        };
        claims.push(ClaimDossier {
            claim_id: claim.claim_id.clone(),
            normative_reference: claim.normative_reference.clone(),
            checkpoint: claim.checkpoint.clone(),
            source_pointer: claim
                .selector
                .source
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?,
            source_anchor_text: claim_source_anchor(repository_root, contract, claim)?,
            expected_logical_answer: json!({
                "selector": &claim.selector,
                "predicate": &claim.predicate,
                "cardinality": &claim.cardinality,
                "ordering": &claim.ordering,
                "surfaces": &claim.surfaces,
            }),
            decoded_actual: evidence_for_claim(execution, &claim.claim_id),
            semantic_diff: if matched {
                "no semantic difference in the validated functional slice".to_owned()
            } else {
                "not compared: required producing scenario/seam is not exercised".to_owned()
            },
            status: if matched {
                ReviewStatus::Matched
            } else {
                ReviewStatus::Limitation
            },
            limitations,
            mutant_ids: contract
                .mutants
                .iter()
                .filter(|mutant| mutant.must_fail_claims.contains(&claim.claim_id))
                .map(|mutant| mutant.mutant_id.clone())
                .collect(),
            convergence: convergence.clone(),
            integrity: integrity.clone(),
        });
    }
    let limitations = claims
        .iter()
        .flat_map(|claim| claim.limitations.iter().cloned())
        .collect::<Vec<_>>();
    Ok(ReviewDossier {
        schema_version: 1,
        candidate_id: FUNCTIONAL_CANDIDATE_ID.to_owned(),
        authority_id: contract.authority_id.clone(),
        review_status: if limitations.is_empty() {
            "READY_FOR_OWNER_REVIEW".to_owned()
        } else {
            "BLOCKED_BY_DISCLOSED_SEMANTIC_LIMITATIONS".to_owned()
        },
        claims,
        queries: execution.planes["queries"]["decoded_response"].clone(),
        causal_interventions: vec![
            "PyreflySourceAdmission".to_owned(),
            "ReconciliationAuthority".to_owned(),
            "DeltaPublication".to_owned(),
            "SnapshotActivation".to_owned(),
            "ArtifactPersistence".to_owned(),
            "ArtifactReadback".to_owned(),
            "FastMcpAdaptation".to_owned(),
        ],
        limitations,
        integrity_metadata_is_not_semantic_authority: true,
    })
}

fn detached_digest(manifest: &FunctionalCandidateManifest) -> Result<String, GateBCandidateError> {
    let bytes = canonical_bytes(manifest)?;
    let mut hasher = crate::integrity::IntegrityHasher::for_domain(
        crate::integrity::IntegrityDomain::GateBReviewCandidate,
    );
    hasher.update(&(bytes.len() as u64).to_be_bytes());
    hasher.update(&bytes);
    Ok(crate::integrity::frame_digest(hasher.finalize()))
}

/// Execute the functional vertical and build a detached, owner-unaccepted review bundle.
///
/// # Errors
///
/// Returns an error for a pre-existing scratch root, failed real vertical, failed semantic
/// validator, mutant survivor, or noncanonical artifact construction.
pub(crate) fn generate_functional_review_bundle(
    repository_root: &Path,
    scratch_root: &Path,
) -> Result<GeneratedCandidateBundle, GateBCandidateError> {
    if scratch_root.exists() {
        return Err(invariant(
            "functional candidate scratch root already exists",
        ));
    }
    fs::create_dir(scratch_root)?;
    let generated = (|| {
        let contract =
            crate::functional_golden::load_contract(repository_root).map_err(invariant)?;
        let corpus_root = repository_root.join(crate::functional_golden::FUNCTIONAL_AUTHORITY_ROOT);
        let execution =
            vertical::execute_functional_candidate(repository_root, &corpus_root, scratch_root)?;
        let validated = super::validated_functional_claims(&contract, &execution)?;
        let mutant_report =
            crate::functional_golden::execute_required_mutants(&contract).map_err(invariant)?;
        if !mutant_report.passed {
            return Err(invariant("functional semantic mutant matrix is not closed"));
        }
        let dossier = dossier(repository_root, &contract, &execution, &validated)?;
        let evidence = decoded_evidence(&execution);
        let dossier_bytes = file_bytes(&dossier)?;
        let evidence_bytes = file_bytes(&evidence)?;
        let mutant_bytes = file_bytes(&mutant_report)?;
        let limitation_count = dossier.limitations.len();
        let semantic_conformance = limitation_count == 0;
        let manifest = FunctionalCandidateManifest {
            schema_version: 1,
            artifact_kind: "functional-gate-b-review-candidate".to_owned(),
            candidate_id: FUNCTIONAL_CANDIDATE_ID.to_owned(),
            candidate_status: if semantic_conformance {
                "CANDIDATE".to_owned()
            } else {
                "BLOCKED_REVIEW".to_owned()
            },
            proposed_corpus_version: "4.0.0".to_owned(),
            expectation_authority: crate::functional_golden::FUNCTIONAL_AUTHORITY_ROOT.to_owned(),
            semantic_conformance,
            limitation_count,
            corpus_index_advanced: false,
            owner_acceptance: None,
            members: [
                (DOSSIER_FILE, &dossier_bytes),
                (EVIDENCE_FILE, &evidence_bytes),
                (MUTANTS_FILE, &mutant_bytes),
            ]
            .into_iter()
            .map(|(path, bytes)| CandidateMember {
                path: path.to_owned(),
                digest: crate::integrity::framed_digest(bytes),
            })
            .collect(),
        };
        let manifest_bytes = file_bytes(&manifest)?;
        let detached = FunctionalCandidateDigest {
            schema_version: 1,
            artifact_kind: "detached-functional-gate-b-candidate-digest".to_owned(),
            candidate_id: FUNCTIONAL_CANDIDATE_ID.to_owned(),
            manifest: MANIFEST_FILE.to_owned(),
            digest: detached_digest(&manifest)?,
        };
        let digest_bytes = file_bytes(&detached)?;
        Ok(GeneratedCandidateBundle {
            files: BTreeMap::from([
                (DOSSIER_FILE.to_owned(), dossier_bytes),
                (EVIDENCE_FILE.to_owned(), evidence_bytes),
                (MUTANTS_FILE.to_owned(), mutant_bytes),
                (MANIFEST_FILE.to_owned(), manifest_bytes),
                (DIGEST_FILE.to_owned(), digest_bytes),
            ]),
        })
    })();
    fs::remove_dir_all(scratch_root)?;
    generated
}

fn require_candidate_readiness(
    bundle: &GeneratedCandidateBundle,
) -> Result<(), GateBCandidateError> {
    let manifest: FunctionalCandidateManifest =
        serde_json::from_slice(&bundle.files()[MANIFEST_FILE])?;
    if !manifest.semantic_conformance
        || manifest.limitation_count != 0
        || manifest.candidate_status != "CANDIDATE"
    {
        return Err(invariant(format!(
            "functional review bundle is not candidate-ready: {} disclosed semantic limitations",
            manifest.limitation_count
        )));
    }
    Ok(())
}

/// Execute the functional vertical and return only a semantically conformant successor candidate.
///
/// # Errors
///
/// Returns an error for every review-bundle generation failure and whenever any authored claim
/// remains an explicitly disclosed limitation. A blocked review bundle is never emitted as a
/// candidate.
pub fn generate_functional_candidate_bundle(
    repository_root: &Path,
    scratch_root: &Path,
) -> Result<GeneratedCandidateBundle, GateBCandidateError> {
    let bundle = generate_functional_review_bundle(repository_root, scratch_root)?;
    require_candidate_readiness(&bundle)?;
    Ok(bundle)
}

/// Verify the functional candidate's canonical members and detached digest chain.
///
/// # Errors
///
/// Returns an error for missing/extra/noncanonical members, candidate self-acceptance, corpus-index
/// advancement, or any member/detached digest mismatch.
pub fn verify_functional_candidate_bundle(
    candidate_root: &Path,
) -> Result<(), GateBCandidateError> {
    let observed = fs::read_dir(candidate_root)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if observed != MEMBERS.into_iter().map(str::to_owned).collect() {
        return Err(invariant("functional candidate member census differs"));
    }
    let manifest: FunctionalCandidateManifest =
        serde_json::from_slice(&fs::read(candidate_root.join(MANIFEST_FILE))?)?;
    let detached: FunctionalCandidateDigest =
        serde_json::from_slice(&fs::read(candidate_root.join(DIGEST_FILE))?)?;
    if manifest.candidate_id != FUNCTIONAL_CANDIDATE_ID
        || manifest.candidate_status != "CANDIDATE"
        || !manifest.semantic_conformance
        || manifest.limitation_count != 0
        || manifest.owner_acceptance.is_some()
        || manifest.corpus_index_advanced
        || manifest.members.len() != 3
    {
        return Err(invariant("functional candidate authority state differs"));
    }
    for member in &manifest.members {
        let bytes = fs::read(candidate_root.join(&member.path))?;
        if crate::integrity::framed_digest(&bytes) != member.digest {
            return Err(invariant(format!(
                "functional candidate member {} drifted",
                member.path
            )));
        }
    }
    if detached.candidate_id != FUNCTIONAL_CANDIDATE_ID
        || detached.manifest != MANIFEST_FILE
        || detached.digest != detached_digest(&manifest)?
    {
        return Err(invariant("functional candidate detached digest differs"));
    }
    for name in MEMBERS {
        let bytes = fs::read(candidate_root.join(name))?;
        let mut canonical = crate::contracts::jcs::canonicalize_slice(&bytes).map_err(invariant)?;
        canonical.push(b'\n');
        if bytes != canonical {
            return Err(invariant(format!(
                "functional candidate member {name} is not canonical JSON"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn generated_review() -> GeneratedCandidateBundle {
        let temporary = tempfile::tempdir().expect("temporary root");
        generate_functional_review_bundle(&root(), &temporary.path().join("scratch"))
            .expect("functional review bundle")
    }

    #[test]
    fn gate_b_human_review_bundle_contract() {
        let bundle = generated_review();
        let dossier: ReviewDossier =
            serde_json::from_slice(&bundle.files()[DOSSIER_FILE]).expect("dossier");
        let contract = crate::functional_golden::load_contract(&root()).expect("contract");
        assert_eq!(dossier.claims.len(), contract.claims.len());
        assert!(dossier.integrity_metadata_is_not_semantic_authority);
        for claim in &dossier.claims {
            assert!(!claim.normative_reference.is_empty());
            assert!(!claim.expected_logical_answer.is_null());
            assert!(!claim.decoded_actual.is_null());
            assert!(!claim.semantic_diff.is_empty());
            assert!(!claim.mutant_ids.is_empty());
            assert!(!claim.convergence.is_null());
            assert!(!claim.integrity.is_null());
            assert!(matches!(claim.status, ReviewStatus::Matched) || !claim.limitations.is_empty());
        }
    }

    #[test]
    fn gate_b_functional_candidate_is_expectation_independent() {
        let bundle = generated_review();
        let manifest: FunctionalCandidateManifest =
            serde_json::from_slice(&bundle.files()[MANIFEST_FILE]).expect("manifest");
        assert!(manifest.owner_acceptance.is_none());
        assert!(!manifest.corpus_index_advanced);
        assert!(!manifest.semantic_conformance);
        assert_eq!(manifest.limitation_count, 6);
        assert_eq!(manifest.candidate_status, "BLOCKED_REVIEW");
        assert!(require_candidate_readiness(&bundle).is_err());
        let temporary = tempfile::tempdir().expect("temporary root");
        let candidate_root = temporary.path().join("candidate");
        fs::create_dir(&candidate_root).expect("candidate root");
        for (name, bytes) in bundle.files() {
            fs::write(candidate_root.join(name), bytes).expect("candidate member");
        }
        assert!(verify_functional_candidate_bundle(&candidate_root).is_err());
        let index = fs::read(root().join("tests/golden/corpus-index.json")).expect("corpus index");
        assert!(!String::from_utf8_lossy(&index).contains(FUNCTIONAL_CANDIDATE_ID));
    }
}
