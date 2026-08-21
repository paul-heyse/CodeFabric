use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use codefabric::contracts::artifacts::{
    ContractArtifactError, VerificationProfile, generate, identity, verify,
    verify_broken_trace_edge_fixture, verify_checksum_fixture, verify_jcs_corpus,
};
use codefabric::contracts::catalog::{BundleKind, ContractCatalog, SemanticProjectionSource};
use codefabric::contracts::compiler::{ContractCompileError, compile_artifact};
use codefabric::contracts::index::{ARTIFACT_INDEX_BYTES, artifact_index, artifact_index_digest};
use codefabric::contracts::jcs::validate_checksum;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn copy_file(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination.parent().expect("copied file has a parent"))
        .expect("create isolated parent");
    std::fs::copy(source, destination).expect("copy isolated input");
}

fn compile_isolated(
    repository_root: &Path,
    artifact_id: &str,
) -> Result<codefabric::contracts::compiler::CompiledArtifact, ContractCompileError> {
    let catalog = ContractCatalog::load(repository_root).expect("isolated catalog");
    let descriptor = catalog.artifact(artifact_id).expect("isolated descriptor");
    compile_artifact(repository_root, &catalog, descriptor)
}

fn isolated_contract_root() -> tempfile::TempDir {
    let source_root = Path::new(ROOT);
    let isolated = tempfile::tempdir().expect("isolated contract root");
    let catalog = ContractCatalog::load(source_root).expect("source catalog");
    for artifact in catalog.artifacts() {
        copy_file(
            &source_root.join(&artifact.authority_path),
            &isolated.path().join(&artifact.authority_path),
        );
    }
    for artifact in catalog.artifacts() {
        if let SemanticProjectionSource::DerivationOutput { output } =
            &artifact.semantic_projection_source
        {
            copy_file(
                &source_root.join(&output.path),
                &isolated.path().join(&output.path),
            );
        }
    }
    let fixture_manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(source_root.join("contracts/manifests/fixture-oracles.json"))
            .expect("fixture manifest"),
    )
    .expect("fixture manifest JSON");
    for record in fixture_manifest["records"]
        .as_array()
        .expect("fixture records")
    {
        let relative = Path::new(record["path"].as_str().expect("fixture path"));
        copy_file(&source_root.join(relative), &isolated.path().join(relative));
    }
    for relative in [
        "Cargo.lock",
        "codefabric-cpg-mcp/uv.lock",
        "rustc-extractor/toolchain-identity.json",
        "pyrefly-sidecar/toolchain-identity.json",
    ] {
        copy_file(&source_root.join(relative), &isolated.path().join(relative));
    }
    std::fs::create_dir_all(isolated.path().join("contracts/schema/arrow-delta"))
        .expect("isolated required schema directory");
    generate(isolated.path()).expect("generate isolated outputs");
    verify(isolated.path(), VerificationProfile::Full).expect("isolated baseline verifies");
    isolated
}

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
fn seeded_no_sort_mutant_is_killed_by_the_normative_jcs_kat() {
    let corpus: serde_json::Value = serde_json::from_slice(
        &std::fs::read(Path::new(ROOT).join("contracts/fixtures/jcs/vectors.json"))
            .expect("JCS KAT corpus"),
    )
    .expect("JCS KAT JSON");
    let vector = corpus["positive"]
        .as_array()
        .expect("positive KATs")
        .iter()
        .find(|vector| vector["id"] == "member-order")
        .expect("member-order KAT");
    let seeded_no_sort_mutant = vector["input_json"].as_str().expect("KAT input");

    assert_ne!(seeded_no_sort_mutant, vector["canonical_utf8"]);
}

#[test]
fn packaged_index_has_the_exact_source_census_and_bytes() {
    let catalog = ContractCatalog::load(Path::new(ROOT)).expect("compiled contract catalog");
    let index = artifact_index().expect("typed packaged artifact index");
    assert_eq!(index.artifacts.len(), catalog.artifacts().count());
    assert_eq!(
        ARTIFACT_INDEX_BYTES,
        std::fs::read(
            Path::new(ROOT)
                .join("codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/artifact-index.json")
        )
        .expect("canonical package resource")
    );
    validate_checksum(artifact_index_digest()).expect("index checksum frame");
    assert!(
        index
            .artifacts
            .iter()
            .all(|artifact| validate_checksum(&artifact.canonical_digest).is_ok())
    );
}

#[test]
fn full_and_released_profiles_accept_the_zero_warning_catalog() {
    let catalog = ContractCatalog::load(Path::new(ROOT)).expect("compiled contract catalog");
    let artifact_count = catalog.artifacts().count();
    let report = verify(Path::new(ROOT), VerificationProfile::Full).expect("full verification");
    assert_eq!(report.artifact_count, artifact_count);
    assert_eq!(report.warning_count, 0);
    assert_eq!(
        verify(Path::new(ROOT), VerificationProfile::Released)
            .expect("released verification")
            .warning_count,
        0
    );
}

#[test]
fn wp11_behavioral_acceptance() {
    let report = verify(Path::new(ROOT), VerificationProfile::Released)
        .expect("all Wave-1 contracts are released and internally consistent");
    assert_eq!(report.warning_count, 0);
}

#[test]
fn wp11_structural_acceptance() {
    let root = Path::new(ROOT);
    let catalog = ContractCatalog::load(root).expect("compiled catalog");
    for kind in BundleKind::ALL {
        let bundle_id = format!("codefabric.bundles.{}-bundle", kind.artifact_slug());
        let descriptor = catalog.artifact(&bundle_id).expect("bundle descriptor");
        let bundle: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join(&descriptor.authority_path)).expect("bundle authority"),
        )
        .expect("bundle JSON");
        let actual = bundle["artifacts"]
            .as_array()
            .expect("bundle members")
            .iter()
            .map(|member| {
                member["artifact_id"]
                    .as_str()
                    .expect("member ID")
                    .to_owned()
            })
            .collect::<BTreeSet<_>>();
        let expected = catalog
            .artifacts()
            .filter(|artifact| artifact.bundle_membership.contains(&kind))
            .map(|artifact| artifact.artifact_id.clone())
            .collect::<BTreeSet<_>>();
        assert!(!actual.is_empty(), "{bundle_id} must be populated");
        assert_eq!(actual, expected, "{bundle_id} membership drifted");
    }
}

#[test]
fn wp11_negative_zero_state() {
    verify_broken_trace_edge_fixture(Path::new(ROOT))
        .expect("the committed unallocated CF-ID edge remains rejected");
}

#[test]
fn wp11_operational_acceptance() {
    let isolated = isolated_contract_root();
    let index_path = isolated
        .path()
        .join("codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/artifact-index.json");
    let before = std::fs::read(&index_path).expect("first generated artifact index");
    generate(isolated.path()).expect("second isolated generation");
    let after = std::fs::read(index_path).expect("second generated artifact index");
    assert_eq!(before, after);
    verify(isolated.path(), VerificationProfile::Released)
        .expect("second generation remains release-valid");
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
fn verifier_rejects_semantic_embedded_and_bundle_digest_mutations() {
    let isolated = isolated_contract_root();
    let schema_path = isolated
        .path()
        .join("contracts/schema/public-status.schema.json");
    let original_schema = std::fs::read(&schema_path).expect("schema source");
    let mut schema: serde_json::Value =
        serde_json::from_slice(&original_schema).expect("schema JSON");
    schema
        .as_object_mut()
        .expect("schema object")
        .insert("description".to_owned(), "semantic mutation".into());
    std::fs::write(
        &schema_path,
        serde_json::to_vec_pretty(&schema).expect("mutated schema"),
    )
    .expect("write semantic mutation");
    assert!(matches!(
        compile_isolated(isolated.path(), "codefabric.schema.public-status.schema"),
        Err(ContractCompileError::Digest { .. })
    ));

    std::fs::write(&schema_path, &original_schema).expect("restore schema");
    let mut schema: serde_json::Value =
        serde_json::from_slice(&original_schema).expect("schema JSON");
    schema["x-codefabric-artifact"]["canonical_digest"] =
        "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into();
    std::fs::write(
        &schema_path,
        serde_json::to_vec_pretty(&schema).expect("mutated schema digest"),
    )
    .expect("write digest mutation");
    assert!(matches!(
        compile_isolated(isolated.path(), "codefabric.schema.public-status.schema"),
        Err(ContractCompileError::Digest { claimed, .. })
            if claimed == "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    ));

    std::fs::write(&schema_path, &original_schema).expect("restore schema");
    let bundle_path = isolated.path().join("contracts/bundles/schema-bundle.json");
    let mut bundle: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&bundle_path).expect("bundle source"))
            .expect("bundle JSON");
    bundle["bundle_digest"] =
        "b3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into();
    std::fs::write(
        &bundle_path,
        serde_json::to_vec_pretty(&bundle).expect("mutated bundle digest"),
    )
    .expect("write bundle mutation");
    assert!(matches!(
        compile_isolated(isolated.path(), "codefabric.bundles.schema-bundle"),
        Err(ContractCompileError::Digest { claimed, .. })
            if claimed == "b3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    ));
}

#[test]
fn bundle_ingress_rejects_incomplete_unknown_mistyped_and_duplicate_records() {
    let isolated = isolated_contract_root();
    let bundle_path = isolated.path().join("contracts/bundles/schema-bundle.json");
    let baseline: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&bundle_path).expect("typed bundle source"))
            .expect("typed bundle JSON");
    let member = serde_json::json!({
        "artifact_id": "codefabric.test.member",
        "version": "1.0",
        "canonical_digest": "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "required": true,
        "feature_bits": []
    });

    let cases = [
        (
            {
                let mut value = baseline.clone();
                value["unknown_root"] = true.into();
                value
            },
            "typed-record",
            "$.unknown_root",
        ),
        (
            {
                let mut value = baseline.clone();
                value.as_object_mut().unwrap().remove("created_by");
                value
            },
            "typed-record",
            "$",
        ),
        (
            {
                let mut value = baseline.clone();
                value["artifacts"] = serde_json::json!([member.clone()]);
                value["artifacts"][0]["unknown_member"] = true.into();
                value
            },
            "typed-record",
            "$.artifacts[0].unknown_member",
        ),
        (
            {
                let mut value = baseline.clone();
                value["artifacts"] = serde_json::json!([member.clone()]);
                value["artifacts"][0]["required"] = "yes".into();
                value
            },
            "typed-record",
            "$.artifacts[0].required",
        ),
        (
            {
                let mut value = baseline.clone();
                value["artifacts"] = serde_json::json!([member.clone(), member]);
                value
            },
            "duplicate-bundle-member",
            "$.artifacts",
        ),
    ];

    for (mutated, expected_class, expected_data_path) in cases {
        std::fs::write(
            &bundle_path,
            serde_json::to_vec_pretty(&mutated).expect("mutated typed bundle"),
        )
        .expect("write typed bundle mutation");
        let error = verify(isolated.path(), VerificationProfile::Full)
            .expect_err("invalid typed bundle must fail verification");
        assert!(
            matches!(
                error,
                ContractArtifactError::Compile(ContractCompileError::Parse {
                    class,
                    ref data_path,
                    ..
                }) if class == expected_class && data_path == expected_data_path
            ),
            "unexpected error for {expected_class} at {expected_data_path}: {error}"
        );
    }
}

#[test]
fn source_only_json_whitespace_changes_only_the_source_identity() {
    let isolated = isolated_contract_root();
    let catalog = ContractCatalog::load(isolated.path()).expect("isolated catalog");
    let descriptor = catalog
        .artifact("codefabric.schema.public-status.schema")
        .expect("schema descriptor");
    let before = compile_artifact(isolated.path(), &catalog, descriptor).expect("baseline compile");
    let path = isolated.path().join(&descriptor.authority_path);
    let mut source = std::fs::read(&path).expect("schema source");
    source.push(b'\n');
    std::fs::write(&path, source).expect("write source-only mutation");
    let after = compile_artifact(isolated.path(), &catalog, descriptor).expect("mutated compile");

    assert_eq!(before.canonical_digest, after.canonical_digest);
    assert_ne!(before.source_digest, after.source_digest);
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
