use super::super::normalize;
use super::{Component, Node};
use std::collections::{BTreeMap, BTreeSet};

fn node(index: u64, kind: &'static str, components: &[(&'static str, u64)]) -> Node<'static> {
    Node {
        index,
        kind,
        primitive: None,
        definition: None,
        generic_argument_count: 0,
        array_length: None,
        mutability: None,
        region: None,
        bound_variable_count: 0,
        function_abi: None,
        function_abi_unwind: None,
        function_unsafe: None,
        function_variadic: None,
        components: components
            .iter()
            .enumerate()
            .map(|(index, (role, target))| Component {
                role,
                target: Some(*target),
                ordinal: index as u64 + 1,
            })
            .collect(),
    }
}

#[test]
fn rust_type_identity_preserves_shape_precision_and_is_independent_of_graph_order() {
    let mut nodes = vec![node(0, "Uint", &[]), node(1, "Uint", &[])];
    nodes[0].primitive = Some("u8");
    nodes[1].primitive = Some("u16");
    for (index, length) in [(2, Some(2)), (3, Some(3)), (12, None)] {
        let mut value = node(index, "Array", &[("element", 0)]);
        value.array_length = length;
        nodes.push(value);
    }
    for (index, abi, unwind, unsafe_fn, binders) in [
        (4, "c", Some(false), true, 0),
        (5, "c", Some(true), true, 0),
        (6, "c", Some(false), false, 0),
        (7, "rust", None, false, 0),
        (10, "rust", None, false, 1),
        (13, "custom", None, false, 0),
    ] {
        let mut value = node(
            index,
            "FnPtr",
            &[("function-input", 0), ("function-output", 1)],
        );
        value.function_abi = Some(abi);
        value.function_abi_unwind = unwind;
        value.function_unsafe = Some(unsafe_fn);
        value.function_variadic = Some(false);
        value.bound_variable_count = binders;
        nodes.push(value);
    }
    for (index, region) in [(8, "static"), (9, "erased")] {
        let mut value = node(index, "Ref", &[("element", 1)]);
        value.region = Some(region);
        value.mutability = Some("not-mutable");
        nodes.push(value);
    }
    let mut generic = node(11, "Adt", &[("generic-type-argument", 0)]);
    generic.definition = Some([88; 16]);
    generic.generic_argument_count = 2;
    nodes.push(generic);
    nodes.extend([
        node(14, "Tuple", &[("tuple-element", 9)]),
        node(15, "Tuple", &[("tuple-element", 12)]),
        node(16, "Tuple", &[("tuple-element", 16)]),
    ]);
    let normalized = normalize::normalize([6; 16], [9; 16], &nodes).unwrap();
    let normalized = normalized
        .into_iter()
        .map(|node| (node.index, node))
        .collect::<BTreeMap<_, _>>();
    let id = |index| normalized[&index].identity.as_ref().unwrap().type_id;
    assert_ne!(id(0), id(1));
    assert_ne!(id(2), id(3));
    assert_ne!(id(8), id(9));
    assert_eq!(
        [4, 5, 6, 7]
            .map(id)
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    assert_eq!(normalized[&9].unknown_reason, Some("native_region_erased"));
    assert_eq!(
        normalized[&14].unknown_reason,
        Some("type_component_unknown")
    );
    for index in [10, 11, 12, 13, 15, 16] {
        assert!(normalized[&index].identity.is_none());
        assert!(normalized[&index].unknown_reason.is_some());
    }
    nodes.reverse();
    for actual in normalize::normalize([6; 16], [9; 16], &nodes).unwrap() {
        assert_eq!(actual.identity, normalized[&actual.index].identity);
        assert_eq!(
            actual.unknown_reason,
            normalized[&actual.index].unknown_reason
        );
    }
}
