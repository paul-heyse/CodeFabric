use std::path::Path;
use std::process::Command;

use codefabric::contracts::artifacts::{
    ContractArtifactError, VerificationProfile, identity, verify, verify_checksum_fixture,
    verify_jcs_corpus,
};
use codefabric::contracts::generated::{CONTRACT_ARTIFACT_INDEX_DIGEST, CONTRACT_ARTIFACTS};
use codefabric::contracts::jcs::validate_checksum;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

#[test]
fn shared_jcs_corpus_passes_the_rust_boundary() {
    verify_jcs_corpus(
        Path::new(ROOT)
            .join("contracts/fixtures/jcs/vectors.json")
            .as_path(),
    )
    .expect("shared JCS corpus");
}

#[test]
fn generated_index_has_the_exact_source_census() {
    assert_eq!(CONTRACT_ARTIFACTS.len(), 50);
    validate_checksum(CONTRACT_ARTIFACT_INDEX_DIGEST).expect("index checksum frame");
    assert!(
        CONTRACT_ARTIFACTS
            .iter()
            .all(|artifact| validate_checksum(artifact.canonical_digest).is_ok())
    );
}

#[test]
fn full_profile_accepts_drafts_and_released_profile_rejects_them() {
    let report = verify(Path::new(ROOT), VerificationProfile::Full).expect("full verification");
    assert_eq!(report.artifact_count, 50);
    assert_eq!(report.warning_count, 50);

    let error = verify(Path::new(ROOT), VerificationProfile::Released).unwrap_err();
    assert!(matches!(error, ContractArtifactError::ReleasedWarnings(50)));
}

#[test]
fn committed_checksum_fixtures_are_proven_negative() {
    for fixture in ["perturbed-artifact.json", "drifted-digest.json"] {
        let path = Path::new(ROOT)
            .join("contracts/fixtures/negative")
            .join(fixture);
        assert!(
            verify_checksum_fixture(&path).is_err(),
            "{fixture} was unexpectedly valid"
        );
    }
}

#[test]
fn administrative_binary_identity_is_exact() {
    let output = Command::new(env!("CARGO_BIN_EXE_codefabric-contracts"))
        .arg("--identity")
        .output()
        .expect("run contract identity");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let actual: serde_json::Value = serde_json::from_slice(&output.stdout).expect("identity JSON");
    let expected = serde_json::to_value(identity()).expect("identity value");
    assert_eq!(actual, expected);
}
