use codefabric::compatibility;
use datafusion::prelude::SessionContext;

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
