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

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "real mixed-language publication checks independent Rust structural identity and exact reopen"
)]
fn pragmatic_rust_canonical_structural_types_and_reopen() {
    let fixture = ProductionFixture::with_source(b"number: int = 1\n");
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"type_shapes\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"type_shapes\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), r#"
pub struct Wrap<T, const N: usize> { pub values: [T; N] }
pub static REF: &u16 = &0;
pub fn types(a: [u8; 2], b: [u8; 3], r: &'static u16, p: *mut u16,
    f: unsafe extern "C" fn(u8) -> u16,
    g: unsafe extern "C-unwind" fn(u8) -> u16,
    h: fn(u8) -> u16, j: extern "C" fn(u8) -> u16, w: Wrap<u8, 4>, higher: for<'a> fn(&'a u8)) -> (bool, char, f32, &'static str) {
    (true, 'x', 1.0, "value")
}
"#).unwrap();
    let supervisor = fixture.start_supervisor();
    let graph = canonical_diagnostic_rows(&fixture, "system.canonical_rust_type_graph");
    let types = canonical_diagnostic_rows(&fixture, "fact.code_type");
    assert!(
        !graph.is_empty(),
        "missing Rust types; target preparation={:?}; diagnostics={:?}",
        canonical_diagnostic_rows(&fixture, "system.rust_target_progress"),
        canonical_diagnostic_rows(&fixture, "fact.code_diagnostic")
    );
    assert!(types.iter().any(|row| row["language"] == "python"));
    let text = |value: &str| CbefValue::Utf8 {
        value: value.to_owned(),
        normalization: StringNormalization::None,
    };
    let workspace = bytes(&graph[0]["workspace_id"]).try_into().unwrap();
    let context = bytes(&graph[0]["context_id"]).try_into().unwrap();
    let mut interner = TypeInterner::default();
    let byte = interner
        .intern_type(
            workspace,
            context,
            &TypeTerm {
                constructor: TypeConstructor::Primitive,
                fields: vec![
                    CbefField {
                        tag: 1,
                        value: text("rust"),
                    },
                    CbefField {
                        tag: 2,
                        value: text("u8"),
                    },
                ],
            },
        )
        .unwrap();
    let mut arrays = Vec::new();
    for length in [2_u64, 3] {
        let array = interner
            .intern_type(
                workspace,
                context,
                &TypeTerm {
                    constructor: TypeConstructor::Array,
                    fields: vec![
                        CbefField {
                            tag: 1,
                            value: text("rust"),
                        },
                        CbefField {
                            tag: 2,
                            value: CbefValue::Id(byte.type_id),
                        },
                        CbefField {
                            tag: 3,
                            value: CbefValue::Unsigned(length.to_be_bytes().to_vec()),
                        },
                    ],
                },
            )
            .unwrap();
        let row = types
            .iter()
            .find(|row| row["language"] == "rust" && row["canonical_key"] == array.canonical_key)
            .expect("array length participates in the exact structural identity");
        assert_eq!(bytes(&row["type_id"]), array.type_id);
        arrays.push(array.type_id);
    }
    assert_ne!(arrays[0], arrays[1]);
    let pointers = types
        .iter()
        .filter(|row| {
            row["language"] == "rust"
                && row["type_kind_code"] == TypeConstructor::FunctionPointer.code()
        })
        .collect::<Vec<_>>();
    assert!(
        pointers.len() >= 4,
        "Rust/C/C-unwind and safe/unsafe signatures remain distinct: {pointers:?}"
    );
    assert!(
        graph
            .iter()
            .any(|row| row["unknown_reason"] == "rust_non_type_generic_arguments_unavailable")
    );
    assert!(
        graph
            .iter()
            .any(|row| row["unknown_reason"] == "rust_type_binder_normalization_unavailable")
    );
    assert!(graph.iter().any(|row| row["unknown_reason"] == "native_region_erased" && !row["type_id"].is_null()));
    let observations = canonical_diagnostic_rows(&fixture, "fact.code_type_observation");
    let components = canonical_diagnostic_rows(&fixture, "fact.code_type_component");
    let coverage = canonical_diagnostic_rows(&fixture, "system.entity_processing_scope");
    assert!(coverage.iter().any(|row| row["language"] == "rust"
        && row["context_id"] == graph[0]["context_id"]
        && row["family"] == "types"
        && row["processing_state"] == "partial"));
    let arguments = observations
        .iter()
        .filter(|row| row["language"] == "rust" && row["type_role"] == "mir-argument-type")
        .collect::<Vec<_>>();
    assert!(arguments.len() >= 10, "{arguments:?}");
    assert!(
        arguments
            .iter()
            .all(|row| row["type_occurrence_id"].is_null()
                && !row["owner_entity_id"].is_null()
                && row["provider_occurrence_kind"] == "mir-local"
                && !row["provider_type_key"].is_null())
    );
    for array in &arrays {
        assert!(
            arguments
                .iter()
                .any(|row| !row["type_id"].is_null() && bytes(&row["type_id"]) == *array)
        );
        assert!(components.iter().any(|row| row["language"] == "rust"
            && !row["owner_type_id"].is_null()
            && bytes(&row["owner_type_id"]) == *array
            && bytes(&row["referenced_type_id"]) == byte.type_id
            && row["component_role"] == "element"
            && row["component_ordinal"] == 0));
    }
    assert!(observations.iter().any(|row| row["language"] == "rust"
        && row["type_role"] == "compiler-item-type"
        && !row["type_occurrence_id"].is_null()));
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor();
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    for (relation, expected) in [
        ("system.canonical_rust_type_graph", graph),
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
