use super::block_queries::{block_rows, resource_bytes};
use super::*;

const PYTHON: &str = "# café\r\ndef helper(x: int) -> int:\r\n    return x\r\ndef target(x: int) -> int:\r\n    return helper(x)\r\n";
const RUST: &str = "// café\r\npub fn target() -> u8 { 1 }\r\n";

fn location(file: &str, line: u64, column: u64, meaning: &str) -> Value {
    json!({"source_location":{"source_file":file,"start_line":line,"start_column":column,"semantic_location":meaning}})
}

#[allow(
    clippy::too_many_lines,
    reason = "the public scenario independently checks both coordinate bases and all matching subject paths"
)]
fn located_queries(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
) -> Vec<Value> {
    let function = location("sample.py", 4, 4, "Python function declaration");
    let identifier = location("sample.py", 5, 11, "Python syntax node `identifier`");
    let call_start = PYTHON.rfind("helper(x)").unwrap();
    let call = json!({"source_location":{"source_file":"sample.py","start_byte":call_start,"semantic_location":"Python syntax node `call`"}});
    let rust = location("orphan.rs", 2, 7, "Rust syntax node `function_item`");
    let empty = json!({"source_location":{"source_file":"empty.rs","start_byte":0,"semantic_location":"Rust syntax node `source_file`"}});
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:locations-{phase}"),
        "unused",
    );
    request["queries"] = json!([
        {"request":"retrieve facts about code","query_id":"declaration","about":[function.clone()],"facts":["declarations"]},
        {"request":"follow code relationships","query_id":"calls","starting_from":[function.clone()],"relationship":"calls","direction":"outgoing","distance":"one step"},
        {"request":"retrieve source and syntax context","query_id":"function-source","about":[function],"context":"exact source span","return":{"maximum_source_bytes":256}},
        {"request":"retrieve facts about code","query_id":"identifier","about":[identifier.clone()],"facts":["syntax node properties"]},
        {"request":"retrieve source and syntax context","query_id":"identifier-source","about":[identifier],"context":"exact source span","return":{"maximum_source_bytes":256}},
        {"request":"follow code relationships","query_id":"parent","starting_from":[call],"relationship":"syntax parents","direction":"outgoing","distance":"one step"},
        {"request":"retrieve facts about code","query_id":"rust","about":[rust.clone()],"facts":["syntax node properties"]},
        {"request":"retrieve source and syntax context","query_id":"rust-source","about":[rust],"context":"exact source span","return":{"maximum_source_bytes":256}},
        {"request":"retrieve facts about code","query_id":"empty","about":[empty.clone()],"facts":["syntax node properties"]},
        {"request":"retrieve source and syntax context","query_id":"empty-source","about":[empty],"context":"exact source span","return":{"maximum_source_bytes":256}},
        {"request":"retrieve facts about code","query_id":"range","about":[{"source_location":{"source_file":"sample.py","start_byte":call_start,"end_byte":call_start+9,"semantic_location":"Python syntax node `identifier`"}}],"facts":["syntax node properties"]},
        {"request":"find code entities","query_id":"within","looking_for":"Python syntax nodes","within":[{"source_location":{"source_file":"sample.py","start_byte":call_start,"end_byte":call_start+9,"semantic_location":"Python syntax node `identifier`"}}],"return":{"limit":{"maximum_results":1}}}
    ]);
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!([
            {"id":"query","operation":"call_tool","name":"query_code_graph","arguments":{"request":request,"delivery":"resource"}},
            {"id":"manifest","operation":"read_resource","uri":{"$ref":"query.structured_content.manifest.uri"}}
        ]),
    );
    let path = write_modern_client_scenario(fixture, &format!("locations-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
    let pages = result["pages"].as_array().unwrap();
    assert!(pages.len() <= 128);
    let steps = pages.iter().enumerate().map(|(index,page)| json!({"id":format!("page{index}"),"operation":"read_resource","uri":page["uri"]})).collect::<Vec<_>>();
    let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
    let path = write_modern_client_scenario(fixture, &format!("location-pages-{phase}"), &scenario);
    let pages = modern_client_report(&run_modern_client(stack, &path));
    let rows = |query: &str| {
        let relation = &manifest["canonical_semantic_response"]["queries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["query_id"] == query)
            .unwrap()["relation_id"];
        let entry = manifest["relations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["relation_id"] == *relation)
            .unwrap();
        let start = usize::try_from(entry["page_start"].as_u64().unwrap()).unwrap();
        let count = usize::try_from(entry["page_count"].as_u64().unwrap()).unwrap();
        (start..start + count)
            .flat_map(|page| block_rows(&pages, page))
            .collect::<Vec<_>>()
    };
    let declarations = rows("declaration");
    assert_eq!(declarations.len(), 1);
    assert_eq!(declarations[0]["name"], "target");
    let calls = rows("calls");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["target_name"], "sample.helper");
    for (query, expected) in [
        ("function-source", "target"),
        ("identifier-source", "helper"),
        ("rust-source", "pub fn target() -> u8 { 1 }"),
        ("empty-source", ""),
    ] {
        let source = rows(query);
        assert_eq!(source.len(), 1, "{query}: {source:?}");
        assert_eq!(source[0]["source_context"]["text"], expected);
    }
    let identifier = rows("identifier");
    assert_eq!(identifier.len(), 1);
    assert_eq!(identifier[0]["start_byte"], call_start);
    assert_eq!(identifier[0]["end_byte"], call_start + 6);
    let parent = rows("parent");
    assert_eq!(parent.len(), 1);
    assert_eq!(parent[0]["raw_kind"], "call");
    assert_eq!(parent[0]["target_entity_kind"], "syntax-node");
    let rust = rows("rust");
    assert_eq!(rust.len(), 1);
    assert_eq!(rust[0]["raw_kind"], "function_item");
    let empty = rows("empty");
    assert_eq!(empty.len(), 1);
    assert_eq!(empty[0]["start_byte"], 0);
    assert_eq!(empty[0]["end_byte"], 0);
    let within = rows("within");
    assert_eq!(within.len(), 1);
    let processing = result["processing"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["query_id"] == "within")
        .unwrap();
    assert_eq!(processing["requested_partitions"], 1);
    assert_eq!(processing["additional_rows"], true);
    let range = rows("range");
    assert_eq!(
        range.len(),
        2,
        "both helper and x are inside the requested range"
    );
    declarations
        .into_iter()
        .chain(identifier)
        .chain(rust)
        .chain(empty)
        .collect()
}

#[test]
fn pragmatic_source_locations_resolve_captured_bytes_lines_and_zero_width_nodes() {
    let fixture = ProductionFixture::with_source(PYTHON.as_bytes());
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::write(root.join("orphan.rs"), RUST).unwrap();
    fs::write(root.join("empty.rs"), b"").unwrap();
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
    }
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let expected = located_queries(&fixture, &stack, "initial");
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    assert_eq!(expected, located_queries(&fixture, &stack, "reopened"));
    supervisor.stop();
}

#[test]
fn pragmatic_source_location_facts_do_not_require_source_disclosure() {
    let fixture = ProductionFixture::with_source(PYTHON.as_bytes());
    assert_eq!(
        fixture.workspace.allowed_source_disclosure_rules,
        ["metadata"]
    );
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "request:private-source-location",
        "unused",
    );
    request["queries"] = json!([{
        "request":"retrieve facts about code", "query_id":"function",
        "about":[location("sample.py",4,4,"Python function declaration")],"facts":["declarations"]
    }]);
    let scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([]),
        json!([
            {"id":"query","operation":"call_tool","name":"query_code_graph","arguments":{"request":request,"delivery":"resource"}}
        ]),
    );
    let path = write_modern_client_scenario(&fixture, "private-source-location", &scenario);
    let report = modern_client_report(&run_modern_client(&stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
    assert_eq!(result["total_rows"], 1);
    supervisor.stop();
}
