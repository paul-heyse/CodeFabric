#[test]
fn pipeline_is_stable() {
    assert_eq!(codefabric_functional_golden_rust::pipeline(3), 7);
}
