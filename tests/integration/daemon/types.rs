use super::*;
use codefabric::identity::{
    CbefField, CbefValue, StringNormalization, TypeConstructor, TypeInterner, TypeTerm,
};

fn bytes(value: &serde_json::Value) -> Vec<u8> {
    value
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one native publication checks independent structural keys, observation provenance and exact reopen"
)]
fn pragmatic_python_canonical_structural_types_and_reopen() {
    let fixture = ProductionFixture::with_source(
        b"from typing import Any, Callable, Literal\nclass Box:\n    pass\ndef shape(value: int, /, enabled: bool = False, *, label: str) -> bytes:\n    return b'value'\nfirst: tuple[int, str]\nvariadic: tuple[int, ...]\nmaybe: int | None\nchoice: str | int\ndynamic: Any\nfn: Callable[[int, str], bool]\nobj: Box = Box()\nliterals: Literal['1', 1, True, b'1']\nselected = shape\nunknown = missing\n",
    );
    let supervisor = fixture.start_supervisor();
    let graph = canonical_diagnostic_rows(&fixture, "system.canonical_python_type_graph");
    let types = canonical_diagnostic_rows(&fixture, "fact.code_type");
    let observations = canonical_diagnostic_rows(&fixture, "fact.code_type_observation");
    let components = canonical_diagnostic_rows(&fixture, "fact.code_type_component");
    assert!(!graph.is_empty());
    assert!(!observations.is_empty());
    let text = |value: &str| CbefValue::Utf8 {
        value: value.to_owned(),
        normalization: StringNormalization::None,
    };
    let primitive = TypeTerm {
        constructor: TypeConstructor::Primitive,
        fields: vec![
            CbefField {
                tag: 1,
                value: text("python"),
            },
            CbefField {
                tag: 2,
                value: CbefValue::TaggedUnion {
                    variant: 1,
                    value: Box::new(text("int")),
                },
            },
        ],
    };
    let workspace = bytes(&graph[0]["workspace_id"]).try_into().unwrap();
    let context = bytes(&graph[0]["context_id"]).try_into().unwrap();
    let expected = TypeInterner::default()
        .intern_type(workspace, context, &primitive)
        .unwrap();
    let int = types
        .iter()
        .find(|row| row["canonical_key"] == expected.canonical_key)
        .expect("native int has the specified structural key");
    assert_eq!(bytes(&int["type_id"]), expected.type_id);
    for kind in [
        TypeConstructor::Primitive,
        TypeConstructor::AnyDynamic,
        TypeConstructor::Error,
        TypeConstructor::NullNone,
        TypeConstructor::Nominal,
        TypeConstructor::ClassObject,
        TypeConstructor::Tuple,
        TypeConstructor::Union,
        TypeConstructor::Callable,
        TypeConstructor::Literal,
    ] {
        assert!(
            types.iter().any(|row| row["type_kind_code"] == kind.code()),
            "missing {kind:?}: {types:?}"
        );
    }
    assert!(
        observations
            .iter()
            .all(|row| row["type_role"] == "checker-observed"
                && !row["type_occurrence_id"].is_null())
    );
    assert!(
        observations.iter().any(|row| row["unknown_reason"]
            .as_str()
            .is_some_and(|reason| !reason.is_empty())),
        "checker errors cannot assert complete types"
    );
    assert!(
        components
            .iter()
            .any(|row| row["component_role"] == "parameter"
                && row["parameter_kind"] == "positional-only")
    );
    assert!(
        components
            .iter()
            .any(|row| row["component_role"] == "parameter"
                && row["parameter_name"] == "enabled"
                && row["parameter_required"] == false)
    );
    assert!(
        components
            .iter()
            .any(|row| row["component_role"] == "parameter"
                && row["parameter_name"] == "label"
                && row["parameter_required"] == true)
    );
    let coverage = canonical_diagnostic_rows(&fixture, "system.entity_processing_scope");
    assert!(
        coverage
            .iter()
            .any(|row| row["family"] == "types" && row["processing_state"] == "partial")
    );
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor();
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    for (relation, expected) in [
        ("system.canonical_python_type_graph", graph),
        ("fact.code_type", types),
        ("fact.code_type_observation", observations),
        ("fact.code_type_component", components),
        ("system.entity_processing_scope", coverage),
    ] {
        assert_eq!(
            canonical_diagnostic_rows(&fixture, relation),
            expected,
            "{relation}"
        );
    }
    supervisor.stop();
}
