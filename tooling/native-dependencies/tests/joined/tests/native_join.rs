#[tokio::test]
async fn original_batch_waits_for_hidden_workers_and_retains_real_receipts() {
    codefabric_native_joined_probe::joined_native_original_batch_survives_worker_and_policy_drop().await;
}
#[tokio::test]
async fn hidden_worker_denial_cannot_publish_prior_success() {
    codefabric_native_joined_probe::hidden_native_failure_rejects_earlier_success_after_join().await;
}
#[tokio::test]
async fn resource_owner_binds_exactly_one_native_operation() {
    codefabric_native_joined_probe::resource_owner_cannot_bind_two_native_operations().await;
}
