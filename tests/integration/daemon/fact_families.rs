use super::*;

mod callables;
mod members;

const FAMILIES: &[(&str, &str, &str, Option<&str>)] = &[
    ("modules", "module metadata", "modules", Some("entity_id")),
    (
        "import",
        "imports in declaring files",
        "imports",
        Some("file_id"),
    ),
    (
        "semantic_reference",
        "semantic references to entities",
        "semantic-references",
        Some("target_entity_id"),
    ),
    (
        "type_observation",
        "type observations in declaring files",
        "types",
        Some("file_id"),
    ),
    (
        "type",
        "structural types in analysis contexts",
        "types",
        None,
    ),
    (
        "type_component",
        "type components in declaring files",
        "types",
        Some("file_id"),
    ),
    (
        "diagnostic",
        "diagnostic messages in analysis contexts",
        "diagnostic-messages",
        None,
    ),
    (
        "diagnostic_child",
        "diagnostic children in analysis contexts",
        "diagnostic-children",
        None,
    ),
    (
        "diagnostic_span",
        "diagnostic locations in analysis contexts",
        "diagnostic-locations",
        None,
    ),
    (
        "diagnostic_suggestion",
        "diagnostic suggestions in analysis contexts",
        "diagnostic-suggestions",
        None,
    ),
    (
        "diagnostic_edit",
        "diagnostic edits in analysis contexts",
        "diagnostic-suggestions",
        None,
    ),
    (
        "callable_type",
        "parameter and return type observations",
        "types",
        Some("owner_entity_id"),
    ),
];

fn page_rows(report: &Value, step: &str) -> Vec<Value> {
    let bytes = STANDARD
        .decode(modern_step(report, step)[0]["blob"].as_str().unwrap())
        .unwrap();
    let batches = arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None)
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    let mut writer = arrow::json::WriterBuilder::new()
        .with_explicit_nulls(true)
        .build::<_, arrow::json::writer::JsonArray>(Vec::new());
    writer
        .write_batches(&batches.iter().collect::<Vec<_>>())
        .unwrap();
    writer.finish().unwrap();
    let mut rows: Vec<Value> = serde_json::from_slice(&writer.into_inner()).unwrap();
    rows.sort_by_cached_key(Value::to_string);
    rows
}

// Compare every selected typed family and its public coverage using one installed client.
#[allow(clippy::too_many_lines)]
fn public_families(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    label: &str,
) -> Vec<Vec<Value>> {
    let mut entities = canonical_diagnostic_rows(fixture, "fact.code_declaration");
    entities.extend(canonical_diagnostic_rows(fixture, "fact.code_module"));
    let ids = |field: &str| {
        entities
            .iter()
            .filter_map(|e| e[field].as_str())
            .map(ToOwned::to_owned)
            .collect::<BTreeSet<_>>()
    };
    let entity_ids = ids("entity_id");
    let file_ids = ids("file_id");
    let contexts = ids("context_id");
    let mut subjects = ids("public_entity_id")
        .into_iter()
        .map(|entity| json!({"entity_id": entity}))
        .collect::<Vec<_>>();
    subjects.push(subjects[0].clone());
    let mut steps = vec![];
    let mut expected = vec![];
    for (ordinal, (suffix, meaning, _, key)) in FAMILIES.iter().enumerate() {
        let relation = if *suffix == "modules" {
            "fact.code_module".to_owned()
        } else {
            format!("fact.code_{suffix}")
        };
        expected.push(
            canonical_diagnostic_rows(fixture, &relation)
                .into_iter()
                .filter(|row| {
                    contexts.contains(row["context_id"].as_str().unwrap())
                        && match key {
                            Some("file_id") => row["file_id"]
                                .as_str()
                                .is_some_and(|id| file_ids.contains(id)),
                            Some(key) => {
                                row[key].as_str().is_some_and(|id| entity_ids.contains(id))
                            }
                            None => true,
                        }
                })
                .collect::<Vec<_>>(),
        );
        assert!(
            expected[ordinal].len() < 512,
            "fixture fits its explicit row limit"
        );
        let mut request = semantic_request(
            &fixture.workspace.public_id(),
            &format!("request:families-{label}-{ordinal}"),
            "unused",
        );
        request["queries"] = json!([{"request":"retrieve facts about code", "query_id":"facts", "about":subjects, "facts":[meaning], "return":{"limit":{"maximum_results":512}}}]);
        if ordinal == 1 {
            request["queries"][0]["facts"] = json!(["show the imported evidence"]);
        }
        steps.push(json!({"id":format!("q{ordinal}"), "operation":"call_tool", "name":"query_code_graph", "arguments":{"request":request,"delivery":"resource"}}));
        steps.push(json!({"id":format!("p{ordinal}"), "operation":"read_resource", "uri":{"$ref":format!("q{ordinal}.structured_content.pages.0.uri")}}));
    }
    let request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:modules-{label}"),
        "Python modules",
    );
    steps.push(json!({"id":"modules", "operation":"call_tool", "name":"query_code_graph", "arguments":{"request":request,"delivery":"resource"}}));
    steps.push(json!({"id":"module_page", "operation":"read_resource", "uri":{"$ref":"modules.structured_content.pages.0.uri"}}));
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([{"message":"input.selection-resolution.description","action":"accept","content":{"value":{"$requested_schema_presentation":"Imports in declaring files"}}}]),
        Value::Array(steps),
    );
    let path = write_modern_client_scenario(fixture, &format!("families-{label}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    for (ordinal, (_, _, coverage, _)) in FAMILIES.iter().enumerate() {
        let result = modern_structured(modern_step(&report, &format!("q{ordinal}")));
        assert_eq!(
            result["execution_state"], "SUCCEEDED",
            "{ordinal}: {report}"
        );
        assert_eq!(result["processing"][0]["family"], *coverage);
        assert_eq!(result["total_rows"], expected[ordinal].len());
        assert_eq!(result["pages"].as_array().unwrap().len(), 1);
        assert_eq!(
            page_rows(&report, &format!("p{ordinal}")),
            expected[ordinal],
            "{ordinal}"
        );
        if matches!(
            *coverage,
            "diagnostic-children" | "diagnostic-locations" | "diagnostic-suggestions"
        ) {
            assert!(
                result["processing"][0]["remaining_partitions"]
                    .as_u64()
                    .unwrap()
                    > 0,
                "Python structured detail absence remains unknown"
            );
        }
    }
    let modules = modern_structured(modern_step(&report, "modules"));
    assert_eq!(modules["execution_state"], "SUCCEEDED");
    assert_eq!(modules["processing"][0]["family"], "modules");
    let names = page_rows(&report, "module_page")
        .into_iter()
        .map(|row| row["name"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(names, BTreeSet::from(["sample".to_owned(), "b".to_owned()]));
    expected
}

#[test]
fn pragmatic_canonical_fact_families_through_installed_clients_and_reopen() {
    let fixture = ProductionFixture::with_source(b"from b import target as alias\ndef subject(value: int) -> int:\n    return alias(value)\nbroken: str = 1\n");
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::write(
        root.join("b.py"),
        b"def target(value: int) -> int:\n    return value\n",
    )
    .unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"public_families\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"public_families\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), "mod inner;\nuse inner::target as alias;\npub fn subject(value: u8) -> u8 { alias(value) }\n").unwrap();
    fs::write(
        root.join("src/inner.rs"),
        "pub fn target(value: u8) -> u8 { value }\n",
    )
    .unwrap();
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let selected = wait_for_semantic_activation_with_timeout(&fixture, Duration::from_secs(180));
    let first = public_families(&fixture, &stack, "initial");
    callables::assert_facts(&fixture, first.last().unwrap());
    let scoped = callables::scoped(&fixture, &stack, "initial");
    assert!(first[1].iter().any(|row| row["language"] == "python"
        && row["alias_name"] == "alias"
        && row["resolution"] == "resolved"));
    assert!(first[1].iter().any(|row| row["language"] == "rust"
        && row["alias_name"] == "alias"
        && row["resolution"] == "resolved"));
    assert!(
        first[2]
            .iter()
            .any(|row| row["language"] == "rust" && row["name"] == "alias")
    );
    assert!(
        first[3]
            .iter()
            .any(|row| row["language"] == "python" && row["type_role"] == "checker-observed")
    );
    assert!(
        first[4]
            .iter()
            .any(|row| row["language"] == "rust" && !row["canonical_key"].is_null())
    );
    assert!(first[6].iter().any(|row| row["language"] == "python"
        && row["message"].as_str().is_some_and(|s| s.contains("str"))));
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    assert_eq!(first, public_families(&fixture, &stack, "reopened"));
    assert_eq!(scoped, callables::scoped(&fixture, &stack, "reopened"));
    supervisor.stop();
}
