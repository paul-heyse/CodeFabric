use super::block_queries::{block_rows, resource_bytes};
use super::*;

const PYTHON: &str =
    "def alpha(x: int) -> int:\n    return x\ndef beta(x: int) -> int:\n    return alpha(x)\n";

#[allow(
    clippy::too_many_lines,
    reason = "one public scenario follows failed and successful branches through the sealed manifest and Arrow pages"
)]
fn branch_queries(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
) -> (Value, Vec<Value>) {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:branches-{phase}"),
        "unused",
    );
    // Request order intentionally differs from dependency order. Two independent branches
    // fail during return resolution/lowering; only their own dependents must be skipped.
    request["queries"] = json!([
        {"request":"retrieve facts about code","query_id":"blocked","about":[{"results_of":"unavailable","select":"entities"}],"facts":["declarations"]},
        {"request":"retrieve facts about code","query_id":"facts","about":[{"results_of":"good","select":"entities"}],"facts":["declarations"]},
        {"request":"find code entities","query_id":"unavailable","looking_for":"Python function declarations","return":{"order_by":["unsupported ordering"]}},
        {"request":"find code entities","query_id":"good","looking_for":"Python function declarations","return":{"order_by":["name descending"]}},
        {"request":"find code entities","query_id":"duplicate","looking_for":"Python function declarations","return":{"order_by":["kind","semantic kind"]}}
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
    let path = write_modern_client_scenario(fixture, &format!("branches-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
    let outcomes = &result["query_results"];
    let states = outcomes
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["query_id"].as_str().unwrap(),
                row["execution_state"].as_str().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        states,
        [
            ("blocked", "NOT_EXECUTED_DEPENDENCY"),
            ("facts", "COMPLETE"),
            ("unavailable", "FAILED"),
            ("good", "COMPLETE"),
            ("duplicate", "FAILED")
        ]
    );
    assert_eq!(outcomes[0]["errors"][0]["related_id"], "unavailable");
    assert_eq!(
        outcomes[2]["errors"][0]["code"],
        "SEMANTIC_REFERENCE_UNAVAILABLE"
    );
    assert_eq!(outcomes[4]["errors"][0]["code"], "INVALID_RETURN_DIRECTIVE");
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
    assert_eq!(
        manifest["canonical_semantic_response"]["query_results"],
        *outcomes
    );
    let bindings = manifest["canonical_semantic_response"]["queries"]
        .as_array()
        .unwrap();
    assert_eq!(bindings.len(), 2);
    assert!(
        bindings
            .iter()
            .all(|binding| ["good", "facts"].contains(&binding["query_id"].as_str().unwrap()))
    );
    assert_eq!(manifest["relations"].as_array().unwrap().len(), 2);
    let steps = result["pages"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(index, page)| {
            json!({
                "id":format!("page{index}"),"operation":"read_resource","uri":page["uri"]
            })
        })
        .collect::<Vec<_>>();
    let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
    let path = write_modern_client_scenario(fixture, &format!("branch-pages-{phase}"), &scenario);
    let pages = modern_client_report(&run_modern_client(stack, &path));
    let mut rows = Vec::new();
    for query in ["good", "facts"] {
        let binding = bindings
            .iter()
            .find(|binding| binding["query_id"] == query)
            .unwrap();
        let relation = manifest["relations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["relation_id"] == binding["relation_id"])
            .unwrap();
        let start = usize::try_from(relation["page_start"].as_u64().unwrap()).unwrap();
        let count = usize::try_from(relation["page_count"].as_u64().unwrap()).unwrap();
        let block = (start..start + count)
            .flat_map(|page| block_rows(&pages, page))
            .collect::<Vec<_>>();
        let names = block
            .iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(names, BTreeSet::from(["alpha", "beta"]));
        rows.extend(block);
    }
    (outcomes.clone(), rows)
}

#[test]
fn pragmatic_failed_query_branches_preserve_independent_results_and_exact_reopen() {
    let fixture = ProductionFixture::with_source(PYTHON.as_bytes());
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let expected = branch_queries(&fixture, &stack, "initial");
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    assert_eq!(expected, branch_queries(&fixture, &stack, "reopened"));
    supervisor.stop();
}
