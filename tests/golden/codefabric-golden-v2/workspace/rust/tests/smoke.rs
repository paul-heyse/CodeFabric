#[test]
fn normalized_total_is_stable() {
    assert_eq!(codefabric_golden_rust::normalized_total(vec![3, 1, 2]), 6);
}
