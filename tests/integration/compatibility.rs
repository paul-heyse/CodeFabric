use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use codefabric::compatibility;
use datafusion::prelude::SessionContext;
use serde_json::Value;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

#[tokio::test(flavor = "multi_thread")]
async fn stable_dependency_contract_is_executable() {
    assert_eq!(arrow::ARROW_VERSION, "59.2.0");
    assert_eq!(datafusion::DATAFUSION_VERSION, "55.0.0");
    let context: SessionContext = compatibility::session_with_provider().expect("provider");
    let batches = context
        .sql("SELECT id FROM compatibility ORDER BY id")
        .await
        .expect("plan")
        .collect()
        .await
        .expect("execute");
    assert_eq!(
        batches
            .iter()
            .map(arrow_array::RecordBatch::num_rows)
            .sum::<usize>(),
        2
    );

    compatibility::arrow_family_probe().expect("Arrow family kernels");
    compatibility::utility_probe().expect("utilities");
    assert_eq!(compatibility::git_hash_algorithm_count(), 2);
}

#[test]
fn wp02_behavioral_target_compile() {
    assert_eq!(arrow::ARROW_VERSION, "59.2.0");
    assert_eq!(datafusion::DATAFUSION_VERSION, "55.0.0");
    let lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"));
    assert!(lock.contains(
        "git+https://github.com/delta-io/delta-rs.git?rev=43a0cf10a313e5077c48637ad786a05359136bbb#43a0cf10a313e5077c48637ad786a05359136bbb"
    ));
    assert!(!lock.contains("9f9223197469897ef05ae4369eb4fd1390174e65"));
}

#[test]
fn wp02_structural_exact_graph() {
    let output = Command::new("bash")
        .arg(repository_root().join("scripts/stable_graph_check.sh"))
        .current_dir(repository_root())
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .expect("run exact resolved-graph contract");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn wp02_negative_old_family_graph() {
    let lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"));
    for predecessor in [
        "name = \"arrow\"\nversion = \"58.4.0\"",
        "name = \"datafusion\"\nversion = \"54.1.0\"",
        "9f9223197469897ef05ae4369eb4fd1390174e65",
    ] {
        assert!(
            !lock.contains(predecessor),
            "live predecessor {predecessor}"
        );
    }
}

#[test]
fn data_fabric_target_stack_release_contract() {
    wp02_behavioral_target_compile();
    let lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"));
    for exact in [
        "name = \"arrow\"\nversion = \"59.2.0\"",
        "name = \"datafusion\"\nversion = \"55.0.0\"",
        "name = \"parquet\"\nversion = \"59.2.0\"",
        "name = \"object_store\"\nversion = \"0.13.2\"",
    ] {
        assert!(lock.contains(exact), "missing target lock identity {exact}");
    }
    for predecessor in [
        "name = \"arrow\"\nversion = \"58.4.0\"",
        "name = \"datafusion\"\nversion = \"54.1.0\"",
        "9f9223197469897ef05ae4369eb4fd1390174e65",
    ] {
        assert!(!lock.contains(predecessor));
    }
}

#[test]
fn data_fabric_old_live_authority_zero_state() {
    let output = Command::new("bash")
        .arg(repository_root().join("scripts/data_fabric_old_authority_check.sh"))
        .current_dir(repository_root())
        .output()
        .expect("run predecessor authority check");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn data_fabric_current_reference_routing_contract() {
    let root = repository_root();
    let routes = [
        (
            ".claude/skills/datafusion-pyarrow-rust-ref/SKILL.md",
            "datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md",
        ),
        (
            ".claude/skills/deltalake-rust-ref/SKILL.md",
            "deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md",
        ),
        (
            "docs/spec_index/library-routing.md",
            "datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md",
        ),
        (
            "docs/spec_index/library-routing.md",
            "deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md",
        ),
    ];
    for (path, reference) in routes {
        let content = fs::read_to_string(root.join(path)).expect("read current reference route");
        assert!(
            content.contains(reference),
            "{path} does not route {reference}"
        );
    }
}

#[test]
fn data_fabric_gate_b_empty_differential() {
    let root = repository_root();
    let corpus = codefabric::golden_corpus::current_released_corpus_root(&root)
        .expect("resolve owner-accepted current golden corpus");
    let execution = codefabric::golden_corpus::execute_gate_b_artifacts(&corpus)
        .expect("execute all Gate-B fact contracts");
    assert_eq!(execution.artifact_digests.len(), 11);
    let rebuild: Value = serde_json::from_slice(
        &fs::read(corpus.join("expected/rebuild_comparison/gate-b.json"))
            .expect("read Gate-B rebuild contract"),
    )
    .expect("decode Gate-B rebuild contract");
    assert_eq!(rebuild["comparison"], "canonical-effective-state");
    assert_eq!(rebuild["physical_delta_layout_ignored"], true);
    assert_eq!(rebuild["operational_ids_ignored"], true);

    let identity: Value = serde_json::from_slice(
        &fs::read(root.join("contracts/generated/model/governance/toolchain-identity.json"))
            .expect("read G-07 toolchain identity"),
    )
    .expect("decode G-07 toolchain identity");
    let data_fabric = &identity["data_fabric"];
    assert_eq!(data_fabric["datafusion_version"], "55.0.0");
    assert_eq!(data_fabric["arrow_version"], "59.2.0");
    assert_eq!(data_fabric["parquet_version"], "59.2.0");
    assert_eq!(
        data_fabric["delta_rs_git_rev"],
        "43a0cf10a313e5077c48637ad786a05359136bbb"
    );
}

#[test]
fn wp06_behavioral_release_equivalence() {
    data_fabric_target_stack_release_contract();
    data_fabric_gate_b_empty_differential();
}

#[test]
fn wp06_structural_old_authority_zero_state() {
    data_fabric_old_live_authority_zero_state();
    data_fabric_current_reference_routing_contract();
}

#[test]
fn wp06_negative_fact_differential() {
    data_fabric_gate_b_empty_differential();
}
