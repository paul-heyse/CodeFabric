use codefabric::compatibility;
use datafusion::prelude::SessionContext;

#[tokio::test(flavor = "multi_thread")]
async fn stable_dependency_contract_is_executable() {
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
