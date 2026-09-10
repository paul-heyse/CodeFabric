use super::block_queries::{block_rows, resource_bytes};
use super::*;

const PYTHON: &str = "# café\r\ndef target(x: int) -> int:\r\n    return x + 1\r\n";
const RUST: &str = "// café\r\npub fn target() -> u8 { 1 }\r\npub fn broken() { let = ; }\r\n";

#[allow(
    clippy::too_many_lines,
    reason = "one public DAG checks node census, properties, exact source and structural edges together"
)]
fn query_syntax(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
) -> Vec<Value> {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:syntax-{phase}"),
        "unused",
    );
    request["scope"]["analysis_contexts"] = json!({"mode":"source"});
    request["scope"]["representations"] = json!(["syntax"]);
    request["scope"]["source_boundaries"] =
        json!([{"kind":"path","root":"sample.py"},{"kind":"path","root":"orphan.rs"}]);
    let mut queries = Vec::new();
    for (language, function, root) in [
        ("Python", "function_definition", "module"),
        ("Rust", "function_item", "source_file"),
    ] {
        let prior = json!({"results_of":format!("find-{language}"),"select":"entities"});
        queries.extend([
            json!({"request":"find code entities","query_id":format!("find-{language}"),"looking_for":format!("{language} syntax nodes")}),
            json!({"request":"retrieve facts about code","query_id":format!("facts-{language}"),"about":[prior.clone()],"facts":["syntax node properties"]}),
            json!({"request":"follow code relationships","query_id":format!("parents-{language}"),"starting_from":[prior],"relationship":"syntax parents","direction":"outgoing","distance":"one step"}),
            json!({"request":"retrieve source and syntax context","query_id":format!("source-{language}"),"about":[format!("{language} syntax node `{function}`")],"context":"exact source span","return":{"maximum_source_bytes":512}}),
            json!({"request":"retrieve source and syntax context","query_id":format!("outline-{language}"),"about":[format!("{language} syntax node `{root}`"),format!("{language} syntax node `{root}`")],"context":"syntax outline"}),
            json!({"request":"retrieve source and syntax context","query_id":format!("outline-limited-{language}"),"about":[format!("{language} syntax node `{root}`")],"context":"syntax outline","return":{"limit":{"maximum_results":3,"when_exceeded":"truncate"}}}),
            json!({"request":"follow code relationships","query_id":format!("children-{language}"),"starting_from":[format!("{language} syntax node `{root}`")],"relationship":"syntax parents","direction":"incoming","distance":"one step"}),
        ]);
    }
    request["queries"] = json!(queries);
    let steps = vec![
        json!({"id":"query","operation":"call_tool","name":"query_code_graph","arguments":{"request":request,"delivery":"resource"}}),
        json!({"id":"manifest","operation":"read_resource","uri":{"$ref":"query.structured_content.manifest.uri"}}),
    ];
    let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
    let path = write_modern_client_scenario(fixture, &format!("syntax-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
    for outcome in result["query_results"].as_array().unwrap() {
        assert_ne!(outcome["execution_state"], "FAILED", "{outcome}");
    }
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
    let pages = result["pages"].as_array().unwrap();
    assert!(pages.len() <= 128);
    let steps = pages.iter().enumerate().map(|(index, page)| json!({"id":format!("page{index}"), "operation":"read_resource", "uri":page["uri"]})).collect::<Vec<_>>();
    let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
    let path = write_modern_client_scenario(fixture, &format!("syntax-pages-{phase}"), &scenario);
    let page_report = modern_client_report(&run_modern_client(stack, &path));
    let rows = |query: &str| {
        let relation = &manifest["canonical_semantic_response"]["queries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["query_id"] == query)
            .unwrap_or_else(|| panic!("missing binding for {query}: {}", result["query_results"]))
            ["relation_id"];
        let entry = manifest["relations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["relation_id"] == *relation)
            .unwrap();
        let start = usize::try_from(entry["page_start"].as_u64().unwrap()).unwrap();
        let count = usize::try_from(entry["page_count"].as_u64().unwrap()).unwrap();
        (start..start + count)
            .flat_map(|page| block_rows(&page_report, page))
            .collect::<Vec<_>>()
    };
    let mut exact = Vec::new();
    for (language, text, root_kind) in [("Python", PYTHON, "module"), ("Rust", RUST, "source_file")]
    {
        let found = rows(&format!("find-{language}"));
        let facts = rows(&format!("facts-{language}"));
        assert!(facts.len() > 15);
        assert_eq!(found.len(), facts.len());
        let roots = facts
            .iter()
            .filter(|row| row["parent_entity_id"].is_null())
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0]["raw_kind"], root_kind);
        assert_eq!(roots[0]["start_byte"], 0);
        assert_eq!(roots[0]["end_byte"], text.len());
        assert!(facts.iter().any(|row| row["named"] == false));
        assert!(facts.iter().any(|row| row["extra"] == true));
        assert!(facts.iter().any(|row| row["raw_kind"] == "identifier"));
        if language == "Rust" {
            assert!(
                facts
                    .iter()
                    .any(|row| row["error"] == true || row["missing"] == true)
            );
        }
        for row in &facts {
            assert_eq!(row["context_id"], "ffffffffffffffffffffffffffffffff");
            assert!(row["start_byte"].as_u64().unwrap() <= row["end_byte"].as_u64().unwrap());
            assert!(row["end_byte"].as_u64().unwrap() <= text.len() as u64);
            assert!(
                row["public_entity_id"]
                    .as_str()
                    .unwrap()
                    .starts_with("entity:syntax-node:")
            );
        }
        let parents = rows(&format!("parents-{language}"));
        assert_eq!(parents.len() + 1, facts.len());
        for edge in &parents {
            assert_eq!(edge["target_entity_kind"], "syntax-node");
            let parent = facts
                .iter()
                .find(|row| row["entity_id"] == edge["parent_entity_id"])
                .unwrap();
            assert_eq!(edge["public_target_entity_id"], parent["public_entity_id"]);
        }
        let children = rows(&format!("children-{language}"));
        assert_eq!(
            children.len(),
            facts
                .iter()
                .filter(|row| row["parent_entity_id"] == roots[0]["entity_id"])
                .count()
        );
        let source = rows(&format!("source-{language}"));
        assert!(source.iter().any(|row| {
            row["source_context"]["text"]
                .as_str()
                .unwrap()
                .contains("target")
        }));
        for row in source {
            let start = usize::try_from(row["start_byte"].as_u64().unwrap()).unwrap();
            let end = usize::try_from(row["end_byte"].as_u64().unwrap()).unwrap();
            assert_eq!(row["source_context"]["text"], &text[start..end]);
        }
        let processing = result["processing"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["query_id"] == format!("find-{language}"))
            .unwrap();
        assert_eq!(processing["requested_partitions"], 1);
        assert_eq!(processing["completed_partitions"], 1);
        assert_eq!(processing["remaining_partitions"], 0);
        let outline = rows(&format!("outline-{language}"));
        assert_eq!(outline.len(), facts.len());
        assert_eq!(rows(&format!("outline-limited-{language}")), outline[..3]);
        let limited = result["processing"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["query_id"] == format!("outline-limited-{language}"))
            .unwrap();
        assert_eq!(limited["maximum_rows"], 3);
        assert_eq!(limited["additional_rows"], true);
        assert_eq!(outline[0]["syntax_raw_kind"], root_kind);
        assert_eq!(outline[0]["syntax_start_byte"], 0);
        assert_eq!(outline[0]["syntax_end_byte"], text.len());
        for node in &outline {
            assert_eq!(node["context_kind"], "syntax outline");
            assert!(node.get("source_bytes").is_none());
            assert!(node.get("source_context").is_none());
            let fact = facts
                .iter()
                .find(|fact| fact["public_entity_id"] == node["syntax_node_id"])
                .unwrap();
            assert_eq!(node["syntax_raw_kind"], fact["raw_kind"]);
            assert_eq!(node["syntax_start_byte"], fact["start_byte"]);
            assert_eq!(node["syntax_end_byte"], fact["end_byte"]);
            if fact["parent_entity_id"].is_null() {
                assert!(node["syntax_parent_id"].is_null());
            } else {
                let parent = facts
                    .iter()
                    .find(|parent| parent["entity_id"] == fact["parent_entity_id"])
                    .unwrap();
                assert_eq!(node["syntax_parent_id"], parent["public_entity_id"]);
            }
        }
        exact.extend(outline);
        exact.extend(facts);
    }
    exact
}

#[test]
fn pragmatic_syntax_nodes_properties_parents_and_source_survive_public_reopen() {
    let fixture = ProductionFixture::with_source(PYTHON.as_bytes());
    let root = Path::new(&fixture.workspace.root_path_display);
    // No Cargo manifest: syntax does not depend on a valid compiler unit.
    fs::write(root.join("orphan.rs"), RUST).unwrap();
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
    }
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let original = query_syntax(&fixture, &stack, "initial");
    outline_disclosure(&fixture, &stack);
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    assert_eq!(original, query_syntax(&fixture, &stack, "reopened"));
    supervisor.stop();
}

fn outline_disclosure(fixture: &ProductionFixture, stack: &InstalledProductionStack) {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "request:outline-policy",
        "unused",
    );
    request["scope"]["analysis_contexts"] = json!({"mode":"source"});
    request["scope"]["representations"] = json!(["syntax"]);
    request["queries"] = json!([{"request":"retrieve source and syntax context", "query_id":"outline",
        "about":["Python syntax node `module`"], "context":"syntax outline"}]);
    let mut denied = request.clone();
    denied["semantic_request_id"] = json!("request:outline-denied");
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!([
            {"id":"source","operation":"call_tool","name":"query_code_graph","arguments":{"request":request,"delivery":"resource"}},
            {"id":"before","operation":"read_resource","uri":{"$ref":"source.structured_content.manifest.uri"}},
            {"id":"pause","operation":"barrier","name":"outline-policy"},
            {"id":"after","operation":"read_resource","uri":{"$ref":"source.structured_content.pages.0.uri"},"expect_error":"CLIENT_OPERATION_FAILED"},
            {"id":"denied","operation":"call_tool","name":"query_code_graph","arguments":{"request":denied,"delivery":"resource"}}
        ]),
    );
    let path = write_modern_client_scenario(fixture, "outline-policy", &scenario);
    let mut client = spawn_modern_client(stack, &path);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !path.parent().unwrap().join("outline-policy.ready").exists() {
        assert!(
            client.try_wait().unwrap().is_none(),
            "outline client exited before barrier"
        );
        assert!(
            Instant::now() < deadline,
            "outline client did not reach policy barrier"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let policy = |allow| {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, allow)
            .unwrap();
    };
    policy(false);
    fs::write(
        path.parent().unwrap().join("outline-policy.resume"),
        b"resume\n",
    )
    .unwrap();
    let report = modern_client_report(&client.wait_with_output().unwrap());
    assert_eq!(
        modern_step(&report, "after")["public_error"],
        "PERMISSION_DENIED:NOT_AUTHORIZED"
    );
    let denied = modern_structured(modern_step(&report, "denied"));
    assert_eq!(denied["query_results"][0]["execution_state"], "FAILED");
    assert_eq!(
        denied["query_results"][0]["errors"][0]["code"],
        "SOURCE_ACCESS_DENIED"
    );
    policy(true);
}
