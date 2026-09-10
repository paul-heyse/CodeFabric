//! The plan's source-authored mixed corpus reaches all four forms through installed clients.

use super::*;

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pragmatic_cpg");

fn leaf(name: &str) -> &str {
    name.rsplit(['.', ':']).next().unwrap()
}

fn fixture() -> ProductionFixture {
    let source = Path::new(CORPUS).join("workspace");
    let fixture = ProductionFixture::with_source(&fs::read(source.join("sample.py")).unwrap());
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir(root.join("src")).unwrap();
    for path in ["Cargo.toml", "helpers.py", "src/lib.rs"] {
        fs::copy(source.join(path), root.join(path)).unwrap();
    }
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"pragmatic-cpg-fixture\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
    WorkspaceRegistry::new(&mut store)
        .set_source_disclosure(fixture.workspace.workspace_id, true)
        .unwrap();
    fixture
}

fn request(fixture: &ProductionFixture, phase: &str) -> Value {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:first-release-{phase}"),
        "function declarations",
    );
    request["queries"][0]["query_id"] = json!("functions");
    let subjects = json!([{"results_of":"functions","select":"entities"}]);
    request["queries"].as_array_mut().unwrap().extend([
        json!({"request":"retrieve facts about code","query_id":"declarations","about":subjects,"facts":["declarations"]}),
        json!({"request":"retrieve facts about code","query_id":"signatures","about":subjects,"facts":["parameter and return type observations"]}),
        json!({"request":"retrieve facts about code","query_id":"imports","about":subjects,"facts":["imports in declaring files"]}),
        json!({"request":"follow code relationships","query_id":"calls","starting_from":subjects,"relationship":"calls","direction":"outgoing","distance":"one step"}),
        json!({"request":"retrieve source and syntax context","query_id":"source","about":subjects,"context":"function definition","return":{"maximum_source_bytes":4096}}),
    ]);
    for query in request["queries"].as_array_mut().unwrap() {
        query["return"]["limit"] = json!({"maximum_results":128});
    }
    request
}

fn observe(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
) -> (BTreeMap<String, Vec<Value>>, Value) {
    let (report, mut rows) = super::block_queries::resource_query(
        fixture,
        stack,
        &format!("first-release-{phase}"),
        &request(fixture, phase),
    );
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["freshness"], "CURRENT");
    let expectations: Value =
        serde_json::from_slice(&fs::read(Path::new(CORPUS).join("expectations.json")).unwrap())
            .unwrap();
    assert_declarations_and_calls(&rows, &expectations);
    assert_types(&rows, &expectations);
    assert_source(fixture, &rows, &expectations);
    let processing = result["processing"].as_array().unwrap();
    let status = |id| processing.iter().find(|s| s["query_id"] == id).unwrap();
    assert_eq!(status("functions")["remaining_partitions"], 0);
    assert!(
        status("calls")["remaining_partitions"].as_u64().unwrap() > 0,
        "authored indirect targets remain unknown"
    );
    for summary in processing {
        // Absence means exhaustion was not observed, rather than an implicit false.
        // Source-authored assertions above establish the required facts independently.
        assert_ne!(summary["additional_rows"], true, "{summary}");
    }
    for row in rows.get_mut("source").unwrap() {
        // This delivery handle includes the serving snapshot and disclosure authorization.
        let handle = row["source_context"]
            .as_object_mut()
            .unwrap()
            .remove("source_context_id")
            .unwrap();
        codefabric::identity::decode_public_id(
            codefabric::identity::IdentityDomain::QuerySourceContext,
            None,
            handle.as_str().unwrap(),
        )
        .unwrap();
    }
    (rows, result["processing"].clone())
}

fn assert_declarations_and_calls(rows: &BTreeMap<String, Vec<Value>>, expected: &Value) {
    for language in ["python", "rust"] {
        let entities = rows["functions"]
            .iter()
            .filter(|r| r["language"] == language)
            .collect::<Vec<_>>();
        let names = entities
            .iter()
            .map(|r| leaf(r["name"].as_str().unwrap()))
            .collect::<BTreeSet<_>>();
        let expected_names = expected[language]["declarations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| leaf(name.as_str().unwrap()))
            .collect::<BTreeSet<_>>();
        assert_eq!(names, expected_names, "{language} declaration census");
        for entity in &entities {
            assert!(
                rows["declarations"]
                    .iter()
                    .any(|row| row["public_entity_id"] == entity["public_entity_id"])
            );
        }
        let by_id = entities
            .iter()
            .map(|r| {
                (
                    r["public_entity_id"].as_str().unwrap(),
                    leaf(r["name"].as_str().unwrap()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let calls = rows["calls"]
            .iter()
            .filter(|r| r["language"] == language)
            .collect::<Vec<_>>();
        let pairs = calls
            .iter()
            .filter_map(|r| {
                r["public_target_entity_id"].as_str().map(|target| {
                    (
                        by_id[r["public_source_entity_id"].as_str().unwrap()],
                        by_id[target],
                    )
                })
            })
            .collect::<BTreeSet<_>>();
        let expected_pairs = expected[language]["resolved_calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    leaf(r["caller"].as_str().unwrap()),
                    leaf(r["callee"].as_str().unwrap()),
                )
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(pairs, expected_pairs, "{language} compiler-resolved calls");
        for unresolved in expected[language]["unresolved"].as_array().unwrap() {
            let caller = leaf(unresolved["caller"].as_str().unwrap());
            assert!(
                calls
                    .iter()
                    .any(
                        |r| by_id[r["public_source_entity_id"].as_str().unwrap()] == caller
                            && r["public_target_entity_id"].is_null()
                            && r["unknown_reason"].is_string()
                    ),
                "{language}: {caller}"
            );
        }
    }
    for expected in expected["python"]["imports"].as_array().unwrap() {
        assert!(rows["imports"].iter().any(|r| r["language"] == "python"
            && r["module_name"] == expected["module"]
            && r["resolution"] == "resolved"));
    }
}

fn primitive(owner: &Value, language: &str, name: &str) -> String {
    use codefabric::identity::{
        CbefField, CbefValue, StringNormalization, TypeConstructor, TypeInterner, TypeTerm,
    };
    let text = |value: &str| CbefValue::Utf8 {
        value: value.into(),
        normalization: StringNormalization::None,
    };
    let decode = |field: &str| {
        let value = owner[field].as_str().unwrap();
        let bytes = (0..value.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        <[u8; 16]>::try_from(bytes).unwrap()
    };
    let scalar = if language == "python" {
        CbefValue::TaggedUnion {
            variant: 1,
            value: Box::new(text(name)),
        }
    } else {
        text(name)
    };
    let identity = TypeInterner::default()
        .intern_type(
            decode("workspace_id"),
            decode("context_id"),
            &TypeTerm {
                constructor: TypeConstructor::Primitive,
                fields: vec![
                    CbefField {
                        tag: 1,
                        value: text(language),
                    },
                    CbefField {
                        tag: 2,
                        value: scalar,
                    },
                ],
            },
        )
        .unwrap();
    super::block_queries::hex_bytes(&identity.type_id)
}

fn assert_types(rows: &BTreeMap<String, Vec<Value>>, expected: &Value) {
    for language in ["python", "rust"] {
        for expected in expected[language]["types"].as_array().unwrap() {
            let owner = rows["declarations"]
                .iter()
                .find(|r| {
                    r["language"] == language
                        && leaf(r["name"].as_str().unwrap())
                            == leaf(expected["entity"].as_str().unwrap())
                })
                .unwrap();
            for (role, scalar) in [("parameter", "type"), ("return", "return")] {
                let type_id = primitive(owner, language, expected[scalar].as_str().unwrap());
                let observations = rows["signatures"]
                    .iter()
                    .filter(|r| {
                        r["owner_entity_id"] == owner["entity_id"] && r["type_role"] == role
                    })
                    .collect::<Vec<_>>();
                assert_eq!(observations.len(), 1, "{language} {role}: {observations:?}");
                assert_eq!(observations[0]["type_id"], type_id);
                if role == "parameter" && language == "python" {
                    assert_eq!(observations[0]["parameter_name"], expected["parameter"]);
                }
            }
        }
    }
}

fn assert_source(
    fixture: &ProductionFixture,
    rows: &BTreeMap<String, Vec<Value>>,
    expected: &Value,
) {
    assert_eq!(rows["source"].len(), rows["functions"].len());
    for row in &rows["source"] {
        let encoded = row["relative_path"].as_str().unwrap();
        let path = (0..encoded.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&encoded[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        let path = String::from_utf8(path).unwrap();
        let bytes = fs::read(Path::new(&fixture.workspace.root_path_display).join(&path)).unwrap();
        let context = &row["source_context"];
        let start = usize::try_from(context["start_byte"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(context["end_byte"].as_u64().unwrap()).unwrap();
        assert_eq!(
            context["text"],
            std::str::from_utf8(&bytes[start..end]).unwrap()
        );
        assert_eq!(context["complete"], true);
        let language = row["language"].as_str().unwrap();
        assert_eq!(row["context_kind"], "function definition");
        let name = leaf(row["name"].as_str().unwrap());
        let prefix = if language == "python" {
            format!("def {name}(")
        } else {
            format!("pub fn {name}(")
        };
        assert!(context["text"].as_str().unwrap().starts_with(&prefix));
        for expected in expected[language]["source"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| {
                e["path"] == path
                    && e["name"].as_str().unwrap() == leaf(row["name"].as_str().unwrap())
            })
        {
            assert_eq!(context["start_line"], expected["line_1_based"]);
        }
    }
    for language in ["python", "rust"] {
        for expected in expected[language]["source"].as_array().unwrap() {
            assert!(
                rows["source"].iter().any(|row| row["language"] == language
                    && leaf(row["name"].as_str().unwrap()) == expected["name"].as_str().unwrap()
                    && row["source_context"]["start_line"] == expected["line_1_based"]),
                "each authored source expectation has a delivered definition: {expected}"
            );
        }
    }
}

#[test]
fn pragmatic_plan_corpus_first_four_forms_and_independent_expectations_survive_reopen() {
    let fixture = fixture();
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let selected = wait_for_semantic_activation_with_timeout(&fixture, Duration::from_secs(180));
    let initial = observe(&fixture, &stack, "initial");
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    assert_eq!(initial, observe(&fixture, &stack, "reopened"));
    supervisor.stop();
}
