use super::block_queries::{block_rows, resource_bytes};
use super::*;

#[allow(
    clippy::too_many_lines,
    reason = "one public scenario checks block-local results and their resolved interpretations"
)]
fn query_literals(fixture: &ProductionFixture, stack: &InstalledProductionStack, phase: &str) {
    let predicate =
        |property, operator, value| json!({"property":property,"operator":operator,"value":value});
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:literals-{phase}"),
        "unused",
    );
    let queries = json!([
        {"request":"find code entities","query_id":"python","looking_for":"the Python function named `safe_to_refactor`"},
        {"request":"find code entities","query_id":"rust","looking_for":"the Rust function named `safe_to_refactor`"},
        {"request":"find code entities","query_id":"ambiguous","looking_for":"the Rust function named `target`"},
        {"request":"find code entities","query_id":"qualified","looking_for":"the Rust function `literal_queries::inner::target`"},
        {"request":"find code entities","query_id":"predicate","looking_for":"Python function declarations","where":[predicate("name","equals","target"),predicate("language","equals","python")]},
        {"request":"find code entities","query_id":"empty","looking_for":"Python function `absent`"},
        {"request":"find code entities","query_id":"contradiction","looking_for":"Python function `target`","where":[predicate("name","does not equal","target")]},
        {"request":"retrieve facts about code","query_id":"facts","about":[{"results_of":"ambiguous","select":"entities"}],"facts":["declarations"],"where":[predicate("qualified name","equals","literal_queries::inner::target")]},
        {"request":"follow code relationships","query_id":"calls","starting_from":[{"results_of":"rust","select":"entities"}],"relationship":"calls","direction":"outgoing","distance":"one step","where":[predicate("resolution","equals","resolved_declaration")]},
        {"request":"retrieve source and syntax context","query_id":"source","about":[{"results_of":"python","select":"entities"}],"context":"exact source span","where":[predicate("name","equals","safe_to_refactor")],"return":{"maximum_source_bytes":128}},
        {"request":"retrieve facts about code","query_id":"named_python","about":[{"semantic_reference":"Python function `safe_to_refactor`"}],"facts":["declarations"]},
        {"request":"retrieve facts about code","query_id":"named_rust","about":["Rust function `target`"],"facts":["declarations"]},
        {"request":"retrieve facts about code","query_id":"named_empty","about":["Python function `absent`"],"facts":["declarations"]},
        {"request":"retrieve facts about code","query_id":"named_mixed","about":["Rust function `literal_queries::inner::target`","Rust function `literal_queries::inner::target`",{"results_of":"python","select":"entities"}],"facts":["declarations"]},
        {"request":"follow code relationships","query_id":"named_calls","starting_from":["Rust function `safe_to_refactor`"],"relationship":"calls","direction":"outgoing","distance":"one step","where":[predicate("resolution","equals","resolved_declaration")]},
        {"request":"follow code relationships","query_id":"named_incoming","starting_from":["Rust function `literal_queries::inner::target`"],"relationship":"calls","direction":"incoming","distance":"one step"},
        {"request":"retrieve source and syntax context","query_id":"named_source","about":["Python function `safe_to_refactor`"],"context":"exact source span","return":{"maximum_source_bytes":128}},
        {"request":"retrieve source and syntax context","query_id":"named_rust_source","about":["Rust function `literal_queries::inner::target`"],"context":"exact source span","return":{"maximum_source_bytes":128}}
    ]);
    let count = queries.as_array().unwrap().len();
    request["queries"] = queries;
    let mut steps = vec![
        json!({"id":"query","operation":"call_tool","name":"query_code_graph","arguments":{"request":request,"delivery":"resource"}}),
        json!({"id":"manifest","operation":"read_resource","uri":{"$ref":"query.structured_content.manifest.uri"}}),
    ];
    for page in 0..count {
        steps.push(json!({"id":format!("page{page}"),"operation":"read_resource","uri":{"$ref":format!("query.structured_content.pages.{page}.uri")}}));
    }
    let mut unavailable = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:qualified-unavailable-{phase}"),
        "Python function declarations",
    );
    unavailable["queries"][0]["where"] = json!([predicate("qualified name", "equals", "target")]);
    steps.push(json!({"id":"unavailable","operation":"call_tool","name":"query_code_graph","arguments":{"request":unavailable,"delivery":"resource"}}));
    let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
    let path = write_modern_client_scenario(fixture, &format!("literals-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED");
    let unavailable = modern_structured(modern_step(&report, "unavailable"));
    assert_eq!(unavailable["execution_state"], "FAILED");
    assert_eq!(unavailable["error"]["code"], "VALIDATION_REJECTED");
    assert_eq!(unavailable["error"]["retryable"], false);
    assert!(unavailable["pages"].as_array().unwrap().is_empty());
    assert!(
        report["guard_observations"].as_array().unwrap().is_empty(),
        "explicit literals need no phrase guessing"
    );
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
    let binding = |query| {
        manifest["canonical_semantic_response"]["queries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["query_id"] == query)
            .unwrap()
    };
    assert_eq!(
        binding("python")["resolved_semantics"]["looking_for"],
        json!(["python:function"])
    );
    assert_eq!(
        binding("python")["resolved_semantics"]["where"],
        json!([predicate("name", "equals", "safe_to_refactor")])
    );
    assert_eq!(
        binding("qualified")["resolved_semantics"]["where"],
        json!([predicate(
            "qualified name",
            "equals",
            "literal_queries::inner::target"
        )])
    );
    let rows = |query| {
        let relation = &manifest["canonical_semantic_response"]["queries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["query_id"] == query)
            .unwrap()["relation_id"];
        let entry = manifest["relations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["relation_id"] == *relation)
            .unwrap();
        assert_eq!(entry["page_count"], 1);
        block_rows(
            &report,
            usize::try_from(entry["page_start"].as_u64().unwrap()).unwrap(),
        )
    };
    assert_eq!(rows("python").len(), 1);
    assert_eq!(rows("python")[0]["name"], "safe_to_refactor");
    assert_eq!(
        rows("rust").len(),
        1,
        "underscores are literal in a qualified-name suffix"
    );
    assert_eq!(
        rows("rust")[0]["qualified_name"],
        "literal_queries::safe_to_refactor"
    );
    let ambiguous = rows("ambiguous");
    assert_eq!(
        ambiguous.len(),
        2,
        "all exact unqualified-name candidates survive"
    );
    assert_eq!(
        ambiguous
            .iter()
            .map(|r| r["qualified_name"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "literal_queries::inner::target",
            "literal_queries::other::target"
        ])
    );
    assert_eq!(rows("qualified").len(), 1);
    assert_eq!(
        rows("qualified")[0]["qualified_name"],
        "literal_queries::inner::target"
    );
    assert_eq!(rows("predicate").len(), 1);
    assert_eq!(rows("predicate")[0]["name"], "target");
    assert!(rows("empty").is_empty());
    assert!(rows("contradiction").is_empty());
    assert_eq!(rows("facts").len(), 1);
    assert_eq!(
        rows("facts")[0]["qualified_name"],
        "literal_queries::inner::target"
    );
    assert_eq!(rows("calls").len(), 1);
    assert_eq!(
        rows("calls")[0]["public_target_entity_id"],
        rows("qualified")[0]["public_entity_id"]
    );
    assert_eq!(rows("source").len(), 1);
    assert_eq!(
        rows("source")[0]["source_context"]["text"],
        "safe_to_refactor"
    );
    assert_eq!(rows("named_python").len(), 1);
    assert_eq!(
        rows("named_python")[0]["public_entity_id"],
        rows("python")[0]["public_entity_id"]
    );
    assert_eq!(rows("named_rust").len(), 2);
    assert!(rows("named_empty").is_empty());
    assert_eq!(
        rows("named_mixed").len(),
        2,
        "named and prior subjects are a set"
    );
    assert_eq!(rows("named_calls"), rows("calls"));
    assert_eq!(rows("named_incoming").len(), 1);
    assert_eq!(rows("named_source"), rows("source"));
    assert_eq!(rows("named_rust_source").len(), 1);
    assert_eq!(
        rows("named_rust_source")[0]["source_context"]["text"],
        "pub fn target() -> u8"
    );
    assert_eq!(
        binding("named_calls")["resolved_semantics"]["named_subject"][0]["selector"],
        "rust:function"
    );
}

#[test]
fn pragmatic_literal_identifiers_and_property_filters_survive_public_reopen() {
    let fixture = ProductionFixture::with_source(b"def target(value: int) -> int:\n    return value\ndef safe_to_refactor(value: int) -> int:\n    return target(value)\n");
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"literal_queries\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"literal_queries\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), "pub mod inner { pub fn target() -> u8 { 1 } }\npub mod other { pub fn target() -> u8 { 2 } }\npub fn safe_to_refactor() -> u8 { inner::target() }\npub fn safeXtoXrefactor() -> u8 { 3 }\n").unwrap();
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
    }
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    query_literals(&fixture, &stack, "initial");
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    query_literals(&fixture, &stack, "reopened");
    supervisor.stop();
}
