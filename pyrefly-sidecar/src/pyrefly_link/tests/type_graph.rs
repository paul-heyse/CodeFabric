use super::*;
use arrow_array::{Array as _, BinaryArray};

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one native run checks member evidence, wire anchors, known empty scopes and bounded retention"
)]
fn native_class_members_preserve_anchors_types_flags_empty_scopes_and_bounds() {
    let root = claim_001_temp_root("native-members");
    std::fs::create_dir_all(&root).unwrap();
    let source = b"from typing import ClassVar, Final\nclass Empty:\n    pass\nclass Descriptor:\n    def __get__(self, obj: object, owner: object) -> int:\n        return 1\n    def __set__(self, obj: object, value: int) -> None:\n        pass\n    def __delete__(self, obj: object) -> None:\n        pass\nclass Box:\n    descriptor = Descriptor()\n    label: Final[str] = 'box'\n    counter: ClassVar[int] = 0\n    plain: int\n    def __init__(self):\n        self.instance = 1\n    @property\n    def value(self) -> int:\n        return self.instance\n    class Nested:\n        item: bytes\n";
    let mut context = SemanticContext::test_only_fixture(&root, "native-members").unwrap();
    let result = context
        .analyze_modules(
            &inventory_run(1),
            &complete([inventory_module(&root, "main", source)]),
        )
        .unwrap();
    let relation = result.modules[0]
        .relations
        .iter()
        .find(|relation| relation.relation == PyreflyRelation::MemberObservation)
        .unwrap();
    let batch = StreamReader::try_new(Cursor::new(&relation.arrow_ipc), None)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let strings = |name| {
        batch
            .column_by_name(name)
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap()
    };
    let numbers = |name| {
        batch
            .column_by_name(name)
            .unwrap()
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap()
    };
    let booleans = |name| {
        batch
            .column_by_name(name)
            .unwrap()
            .as_any()
            .downcast_ref::<BooleanArray>()
            .unwrap()
    };
    let class_rows = (0..batch.num_rows())
        .filter(|row| strings("record_kind").value(*row) == "class")
        .map(|row| (strings("class_name").value(row), row))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        class_rows.keys().copied().collect::<BTreeSet<_>>(),
        ["Box", "Descriptor", "Empty", "Nested"].into()
    );
    let empty = class_rows["Empty"];
    assert_eq!(numbers("class_expected_members").value(empty), 0);
    assert!(booleans("class_census_complete").value(empty));
    assert!(strings("member_name").is_null(empty));
    assert!(booleans("is_final").is_null(empty));
    for row in 0..batch.num_rows() {
        assert_eq!(
            &source[usize::try_from(numbers("class_start_byte").value(row)).unwrap()
                ..usize::try_from(numbers("class_end_byte").value(row)).unwrap()],
            strings("class_name").value(row).as_bytes()
        );
        if strings("member_name").is_valid(row) && strings("member_name").value(row) == "label" {
            assert!(booleans("annotation_present").value(row));
            assert!(booleans("is_final").value(row));
            assert!(numbers("declared_local_type_index").is_valid(row));
            assert!(numbers("computed_local_type_index").is_valid(row));
        }
    }
    let loaded = &context.loaded["module:main"];
    let query = |bound| {
        context
            .query
            .get_type_facts_in_file(
                ModuleName::from_str("main"),
                ModulePath::filesystem(loaded.provider_path.clone()),
                bound,
            )
            .unwrap()
            .structural
    };
    let graph = query(MAX_RELATION_ROWS);
    assert!(graph.member_census_complete);
    let scopes = graph
        .classes
        .iter()
        .map(|scope| (scope.name.as_str(), scope))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        scopes.keys().copied().collect::<BTreeSet<_>>(),
        ["Empty", "Box", "Descriptor", "Nested"].into()
    );
    assert_eq!(scopes["Empty"].expected_members, Some(0));
    assert!(scopes["Empty"].complete && scopes["Empty"].members.is_empty());
    let members = scopes["Box"]
        .members
        .iter()
        .map(|member| (member.name.as_str(), member))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(members["descriptor"].native_kind, Some("descriptor"));
    assert_eq!(members["descriptor"].descriptor_has_set, Some(true));
    assert_eq!(members["descriptor"].descriptor_has_delete, Some(true));
    assert_eq!(members["label"].is_final, Some(true));
    assert_eq!(members["counter"].is_class_var, Some(true));
    assert_eq!(members["value"].native_kind, Some("property"));
    assert_eq!(members["value"].is_property, Some(true));
    assert_eq!(members["instance"].definition_kind, "defined-in-method");
    for (name, intrinsic) in [("label", "str"), ("counter", "int"), ("plain", "int")] {
        let member = members[name];
        let declared = member.declared_type_index.unwrap();
        let computed = member.computed_type_index.unwrap();
        assert_eq!(graph.nodes[declared].intrinsic, Some(intrinsic));
        assert_eq!(graph.nodes[computed].intrinsic, Some(intrinsic));
        assert!(member.unknown_reason.is_none());
    }
    for scope in &graph.classes {
        let anchor = &scope.definition;
        assert_eq!(
            &source[anchor.start_byte as usize..anchor.end_byte as usize],
            scope.name.as_bytes()
        );
        assert_eq!(scope.expected_members, Some(scope.members.len()));
        assert!(scope.complete);
    }
    let bounded = query(1);
    assert!(!bounded.member_census_complete);
    let mut coverage = vec![];
    super::super::members::coverage(&mut coverage, Some(&bounded));
    assert!(coverage[0].unknown);
    assert_eq!(coverage[0].completeness, "partial");
    super::super::members::coverage(&mut coverage, None);
    assert_eq!(coverage[1].completeness, "unknown");
    assert!(
        bounded
            .classes
            .iter()
            .map(|scope| 1 + scope.members.len())
            .sum::<usize>()
            <= 1
    );
    drop(context);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_member_precision_follows_nested_error_types_without_widening_other_classes() {
    let root = claim_001_temp_root("native-member-precision");
    std::fs::create_dir_all(&root).unwrap();
    let source = b"class Known:\n    field: int\nclass Broken:\n    field: Missing\n    nested: tuple[int, Missing]\n    def method(self, value: Missing) -> int:\n        return 1\n";
    let mut context = SemanticContext::test_only_fixture(&root, "native-member-precision").unwrap();
    context
        .analyze_modules(
            &inventory_run(1),
            &complete([inventory_module(&root, "main", source)]),
        )
        .unwrap();
    let loaded = &context.loaded["module:main"];
    let graph = context
        .query
        .get_type_facts_in_file(
            ModuleName::from_str("main"),
            ModulePath::filesystem(loaded.provider_path.clone()),
            MAX_RELATION_ROWS,
        )
        .unwrap()
        .structural;
    let known = graph
        .classes
        .iter()
        .find(|class| class.name == "Known")
        .unwrap();
    let broken = graph
        .classes
        .iter()
        .find(|class| class.name == "Broken")
        .unwrap();
    assert!(
        known.complete && broken.complete,
        "both associated-name censuses are complete"
    );
    assert!(
        known
            .members
            .iter()
            .all(|member| member.unknown_reason.is_none())
    );
    for name in ["field", "nested", "method"] {
        let member = broken
            .members
            .iter()
            .find(|member| member.name == name)
            .unwrap();
        assert!(
            member.computed_type_index.is_some(),
            "the native type graph retains error-bearing structures"
        );
        assert_eq!(
            member.unknown_reason,
            Some("native_member_computed_type_incomplete"),
            "{name}: {member:?}"
        );
    }
    drop(context);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_type_graph_includes_unreferenced_nested_declarations() {
    let root = claim_001_temp_root("native-declaration-types");
    std::fs::create_dir_all(&root).unwrap();
    let source = b"class Unused:\n    def method(self, value: int) -> str:\n        return ''\ndef outer(value: bytes) -> bool:\n    def inner(text: str) -> int:\n        return 1\n    return True\n";
    let mut context =
        SemanticContext::test_only_fixture(&root, "native-declaration-types").unwrap();
    let result = context
        .analyze_modules(
            &inventory_run(1),
            &complete([inventory_module(&root, "main", source)]),
        )
        .unwrap();
    let relation = result.modules[0]
        .relations
        .iter()
        .find(|relation| relation.relation == PyreflyRelation::TypeNode)
        .unwrap();
    let batches = StreamReader::try_new(Cursor::new(&relation.arrow_ipc), None).unwrap();
    let mut functions = BTreeSet::new();
    let mut classes = BTreeSet::new();
    for batch in batches {
        let batch = batch.unwrap();
        let kinds = batch
            .column_by_name("type_kind")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let starts = batch
            .column_by_name("definition_start_byte")
            .unwrap()
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap();
        let ends = batch
            .column_by_name("definition_end_byte")
            .unwrap()
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap();
        for row in 0..batch.num_rows() {
            if starts.is_null(row) {
                continue;
            }
            let name = &source[usize::try_from(starts.value(row)).unwrap()
                ..usize::try_from(ends.value(row)).unwrap()];
            match kinds.value(row) {
                "function" => {
                    functions.insert(name.to_vec());
                }
                "class-object" => {
                    classes.insert(name.to_vec());
                }
                _ => {}
            }
        }
    }
    assert_eq!(
        functions,
        [b"method".to_vec(), b"outer".to_vec(), b"inner".to_vec()].into()
    );
    assert!(classes.contains(b"Unused".as_slice()));
    drop(context);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one checker run independently checks native structural distinctions and a bounded repeat"
)]
fn native_type_graph_preserves_discriminants_literals_parameters_and_bounds() {
    let root = claim_001_temp_root("native-type-graph");
    std::fs::create_dir_all(&root).unwrap();
    let source = b"from typing import Any, Callable, Literal\nclass Box:\n    pass\ndef shape(value: int, /, enabled: bool = False, *, label: str) -> bytes:\n    return b'value'\nfirst: tuple[int, str]\nvariadic: tuple[int, ...]\nmaybe: int | None\nchoice: str | int\ndynamic: Any\nfn: Callable[[int, str], bool]\nobj: Box = Box()\nliterals: Literal['1', 1, True, b'1']\nselected = shape\nunknown = missing\n";
    let mut context = SemanticContext::test_only_fixture(&root, "native-type-graph").unwrap();
    let result = context
        .analyze_modules(
            &inventory_run(1),
            &complete([inventory_module(&root, "main", source)]),
        )
        .unwrap();
    let relation = |kind| {
        result.modules[0]
            .relations
            .iter()
            .find(|relation| relation.relation == kind)
            .unwrap()
    };
    let batches = |kind| {
        StreamReader::try_new(Cursor::new(&relation(kind).arrow_ipc), None)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    let nodes = batches(PyreflyRelation::TypeNode);
    let mut kinds = BTreeSet::new();
    let mut tuple_styles = BTreeSet::new();
    let mut literal_kinds = BTreeSet::new();
    let mut box_anchor = false;
    for batch in nodes {
        let text = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
        };
        let number = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
        };
        for row in 0..batch.num_rows() {
            let kind = text("type_kind").value(row);
            kinds.insert(kind.to_owned());
            if kind == "tuple" {
                tuple_styles.insert(text("style").value(row).to_owned());
            }
            if kind == "literal" {
                let kind = text("literal_kind").value(row);
                literal_kinds.insert(kind.to_owned());
                if kind == "bytes" {
                    let bytes = batch
                        .column_by_name("literal_bytes")
                        .unwrap()
                        .as_any()
                        .downcast_ref::<BinaryArray>()
                        .unwrap();
                    assert!(matches!(bytes.value(row), b"1" | b"value"));
                    assert!(text("literal_text").is_null(row));
                }
            }
            if text("name").value(row) == "main.Box" {
                assert_eq!(text("definition_file_id").value(row), "file:main");
                assert_eq!(number("definition_start_byte").value(row), 48);
                assert_eq!(number("definition_end_byte").value(row), 51);
                assert_eq!(
                    text("definition_mapping").value(row),
                    "exact_checker_definition"
                );
                box_anchor = true;
            }
        }
    }
    for kind in [
        "any",
        "error",
        "tuple",
        "union",
        "none",
        "nominal",
        "class-object",
        "callable",
        "function",
        "literal",
    ] {
        assert!(kinds.contains(kind), "missing {kind}: {kinds:?}");
    }
    assert!(box_anchor);
    assert_eq!(
        tuple_styles,
        BTreeSet::from(["concrete".to_owned(), "unbounded".to_owned()])
    );
    assert_eq!(
        literal_kinds,
        BTreeSet::from([
            "string".to_owned(),
            "integer".to_owned(),
            "boolean".to_owned(),
            "bytes".to_owned()
        ])
    );
    let edges = batches(PyreflyRelation::TypeEdge);
    let mut parameters = BTreeSet::new();
    for batch in edges {
        let text = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
        };
        let required = batch
            .column_by_name("parameter_required")
            .unwrap()
            .as_any()
            .downcast_ref::<BooleanArray>()
            .unwrap();
        for row in 0..batch.num_rows() {
            if text("component_role").value(row) == "parameter"
                && !text("parameter_name").is_null(row)
            {
                parameters.insert((
                    text("parameter_name").value(row).to_owned(),
                    text("parameter_kind").value(row).to_owned(),
                    required.value(row),
                ));
            }
        }
    }
    for (name, kind, required) in [
        ("value", "positional-only", true),
        ("enabled", "positional-or-keyword", false),
        ("label", "keyword-only", true),
    ] {
        assert!(
            parameters.contains(&(name.to_owned(), kind.to_owned(), required)),
            "{parameters:?}"
        );
    }
    let loaded = &context.loaded["module:main"];
    let bounded = context
        .query
        .get_type_facts_in_file(
            ModuleName::from_str("main"),
            ModulePath::filesystem(loaded.provider_path.clone()),
            1,
        )
        .unwrap();
    assert!(!bounded.structural.complete);
    assert_eq!(bounded.structural.nodes.len(), 1);
    assert!(
        bounded
            .structural
            .types
            .iter()
            .any(|occurrence| occurrence.type_index.is_none())
    );
    assert!(
        bounded
            .structural
            .nodes
            .iter()
            .flat_map(|node| &node.components)
            .all(|edge| edge.target.is_none_or(|index| index < 1))
    );
    assert_eq!(
        bounded.presentation.types.len(),
        bounded.structural.types.len(),
        "both outputs share the same occurrence census"
    );
    drop(context);
    std::fs::remove_dir_all(root).unwrap();
}
