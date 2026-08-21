use std::io::Write;

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
    assert_eq!(compatibility::delta_application_transaction_version(), 1);
}

#[test]
fn descriptor_relative_open_reads_the_selected_file() {
    let directory = std::env::temp_dir().join(format!(
        "codefabric-wp01-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::create_dir_all(&directory).expect("temporary directory");
    let path = directory.join("probe.txt");
    let mut created = std::fs::File::create(&path).expect("create probe");
    created.write_all(b"codefabric").expect("write probe");
    drop(created);

    let root = std::fs::File::open(&directory).expect("open directory");
    let opened = compatibility::descriptor_relative_open(&root, std::path::Path::new("probe.txt"))
        .expect("descriptor-relative open");
    assert_eq!(opened.metadata().expect("metadata").len(), 10);

    std::fs::remove_file(path).expect("remove probe");
    std::fs::remove_dir(directory).expect("remove temporary directory");
}
