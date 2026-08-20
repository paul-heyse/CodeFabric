use codefabric::{normalize_workspace_id, version};

#[test]
fn version_is_exposed_publicly() {
    assert!(!version().is_empty());
}

#[test]
fn workspace_id_round_trips_through_the_public_api() {
    let normalized = normalize_workspace_id("  codefabric  ").expect("non-blank input");
    assert_eq!(normalized, "codefabric");
}
