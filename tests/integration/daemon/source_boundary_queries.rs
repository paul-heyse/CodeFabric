use super::block_queries::{block_rows, resource_bytes};
use super::*;

#[allow(
    clippy::too_many_lines,
    reason = "one installed-client request checks all first-four scopes and their processing projections"
)]
fn query_boundaries(fixture: &ProductionFixture, stack: &InstalledProductionStack, phase: &str) {
    let meanings = [
        ("Python", "picked", "helper"),
        (
            "Rust",
            "boundary_queries::picked",
            "boundary_queries::helper",
        ),
    ];
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:boundary-{phase}"),
        "unused",
    );
    request["scope"]["source_boundaries"] =
        json!([{"kind":"path","root":"selected"},{"kind":"path","root":"src"}]);
    let mut queries = Vec::new();
    for (language, name, _) in meanings {
        queries.extend([
            json!({"request":"find code entities","query_id":format!("find-{language}"),"looking_for":format!("{language} function declarations"),"return":{"limit":{"maximum_results":1}}}),
            json!({"request":"retrieve facts about code","query_id":format!("facts-{language}"),"about":[format!("{language} function `{name}`")],"facts":["declarations"]}),
            json!({"request":"follow code relationships","query_id":format!("calls-{language}"),"starting_from":[format!("{language} function `{name}`")],"relationship":"calls","direction":"outgoing","distance":"one step"}),
            json!({"request":"retrieve source and syntax context","query_id":format!("source-{language}"),"about":[format!("{language} function `{name}`")],"context":"exact source span","return":{"maximum_source_bytes":128}})
        ]);
    }
    request["queries"] = json!(queries);
    let mut empty = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:boundary-empty-{phase}"),
        "Python function declarations",
    );
    empty["scope"]["languages"] = json!(["python"]);
    empty["scope"]["source_boundaries"] = json!([{"kind":"path","root":"missing"}]);
    let mut invalid = empty.clone();
    invalid["semantic_request_id"] = json!(format!("request:boundary-invalid-{phase}"));
    invalid["scope"]["source_boundaries"] = json!([{"kind":"path","root":"../selected"}]);
    let mut steps = vec![
        json!({"id":"query","operation":"call_tool","name":"query_code_graph","arguments":{"request":request,"delivery":"resource"}}),
        json!({"id":"manifest","operation":"read_resource","uri":{"$ref":"query.structured_content.manifest.uri"}}),
    ];
    for page in 0..8 {
        steps.push(json!({"id":format!("page{page}"),"operation":"read_resource","uri":{"$ref":format!("query.structured_content.pages.{page}.uri")}}));
    }
    steps.extend([
            json!({"id":"empty","operation":"call_tool","name":"query_code_graph","arguments":{"request":empty,"delivery":"resource"}}),
            json!({"id":"invalid","operation":"call_tool","name":"query_code_graph","arguments":{"request":invalid,"delivery":"resource"}}),
        ]);
    let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
    let path = write_modern_client_scenario(fixture, &format!("boundaries-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED");
    let invalid = modern_structured(modern_step(&report, "invalid"));
    assert_eq!(invalid["execution_state"], "FAILED");
    assert_eq!(invalid["error"]["code"], "VALIDATION_REJECTED");
    let empty = modern_structured(modern_step(&report, "empty"));
    assert_eq!(empty["execution_state"], "SUCCEEDED");
    assert_eq!(empty["total_rows"], 0);
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
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
            .find(|r| r["relation_id"] == *relation)
            .unwrap();
        assert_eq!(entry["page_count"], 1);
        block_rows(
            &report,
            usize::try_from(entry["page_start"].as_u64().unwrap()).unwrap(),
        )
    };
    assert_eq!(
        manifest["canonical_semantic_response"]["resolved_scope"]["source_boundaries"],
        json!([{"kind":"path","root":"selected"},{"kind":"path","root":"src"}])
    );
    for (language, name, target) in meanings {
        let found = rows(&format!("find-{language}"));
        assert_eq!(found.len(), 1);
        assert!([name, target].contains(&found[0]["name"].as_str().unwrap()));
        let facts = rows(&format!("facts-{language}"));
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0]["name"], name);
        let calls = rows(&format!("calls-{language}"));
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0]["target_name"],
            if language == "Python" {
                "selected.included.helper"
            } else {
                target
            }
        );
        let source = rows(&format!("source-{language}"));
        assert_eq!(source.len(), 1);
        assert_eq!(source[0]["name"], name);
        assert_eq!(
            source[0]["source_context"]["text"],
            if language == "Python" {
                "picked"
            } else {
                "pub fn picked() -> u8"
            }
        );
        let processing = result["processing"].as_array().unwrap();
        let find = processing
            .iter()
            .find(|p| p["query_id"] == format!("find-{language}"))
            .unwrap();
        assert_eq!(find["additional_rows"], true);
        if language == "Python" {
            assert_eq!(find["requested_partitions"], 1);
            assert_eq!(find["remaining_partitions"], 0);
        }
    }
}

#[test]
fn pragmatic_source_boundaries_narrow_first_four_forms_and_survive_reopen() {
    let fixture = ProductionFixture::with_source(b"def outside():\n    return 0\n");
    let root = Path::new(&fixture.workspace.root_path_display);
    for directory in ["selected", "selected_other", "src"] {
        fs::create_dir(root.join(directory)).unwrap();
    }
    fs::write(root.join("selected/included.py"), "def helper(value: int) -> int:\n    return value\ndef picked(value: int) -> int:\n    return helper(value)\n").unwrap();
    fs::write(
        root.join("selected_other/excluded.py"),
        "from missing_package import unknown\ndef distractor():\n    return unknown()\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"boundary_queries\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"boundary_queries\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub fn helper() -> u8 { 1 }\npub fn picked() -> u8 { helper() }\n",
    )
    .unwrap();
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
    }
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    query_boundaries(&fixture, &stack, "initial");
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    query_boundaries(&fixture, &stack, "reopened");
    supervisor.stop();
}
