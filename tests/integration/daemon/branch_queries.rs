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
    // fail during return resolution/lowering; later subject/property/source checks also isolate
    // their own branches. Source disclosure remains disabled for this entire fixture.
    request["queries"] = json!([
        {"request":"retrieve facts about code","query_id":"blocked","about":[{"results_of":"unavailable","select":"entities"}],"facts":["declarations"]},
        {"request":"retrieve facts about code","query_id":"facts","about":[{"results_of":"good","select":"entities"}],"facts":["declarations"]},
        {"request":"find code entities","query_id":"unavailable","looking_for":"Python function declarations","return":{"order_by":["unsupported ordering"]}},
        {"request":"find code entities","query_id":"good","looking_for":"Python function declarations","return":{"order_by":["name descending"]}},
        {"request":"find code entities","query_id":"duplicate","looking_for":"Python function declarations","return":{"order_by":["kind","semantic kind"]}},
        {"request":"retrieve facts about code","query_id":"property-dependent","about":[{"results_of":"property","select":"entities"}],"facts":["declarations"]},
        {"request":"retrieve facts about code","query_id":"subject","about":["the unattested target"],"facts":["declarations"]},
        {"request":"retrieve source and syntax context","query_id":"denied","about":[{"results_of":"good","select":"entities"}],"context":"exact source span"},
        {"request":"find code entities","query_id":"property","looking_for":"Python function declarations","where":[{"property":"qualified name","operator":"equals","value":"alpha"}]}
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
            ("duplicate", "FAILED"),
            ("property-dependent", "NOT_EXECUTED_DEPENDENCY"),
            ("subject", "FAILED"),
            ("denied", "FAILED"),
            ("property", "FAILED")
        ]
    );
    assert_eq!(outcomes[0]["errors"][0]["related_id"], "unavailable");
    assert_eq!(
        outcomes[2]["errors"][0]["code"],
        "SEMANTIC_REFERENCE_UNAVAILABLE"
    );
    assert_eq!(outcomes[4]["errors"][0]["code"], "INVALID_RETURN_DIRECTIVE");
    assert_eq!(outcomes[5]["errors"][0]["related_id"], "property");
    assert_eq!(
        outcomes[6]["errors"][0]["code"],
        "SEMANTIC_REFERENCE_UNAVAILABLE"
    );
    assert_eq!(outcomes[7]["errors"][0]["code"], "SOURCE_ACCESS_DENIED");
    assert_eq!(
        outcomes[8]["errors"][0]["code"],
        "SEMANTIC_REFERENCE_UNAVAILABLE"
    );
    assert!(
        result["processing"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| { ["good", "facts"].contains(&row["query_id"].as_str().unwrap()) })
    );
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
    let failed = outcomes_only_queries(&fixture, &stack, "initial");
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    assert_eq!(expected, branch_queries(&fixture, &stack, "reopened"));
    assert_eq!(failed, outcomes_only_queries(&fixture, &stack, "reopened"));
    supervisor.stop();
}

fn outcomes_only_queries(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
) -> Value {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:all-failed-{phase}"),
        "unused",
    );
    request["queries"] = json!([
        {"request":"retrieve facts about code","query_id":"blocked","about":[{"results_of":"failed","select":"entities"}],"facts":["declarations"]},
        {"request":"find code entities","query_id":"failed","looking_for":"Python function declarations","return":{"order_by":["unknown key"]}},
        {"request":"retrieve source and syntax context","query_id":"denied","about":["the Python function `alpha`"],"context":"exact source span"},
        {"request":"retrieve facts about code","query_id":"subject","about":["the unattested target"],"facts":["declarations"]}
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
    let path = write_modern_client_scenario(fixture, &format!("outcomes-only-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
    assert_eq!(
        result["query_results"][0]["execution_state"],
        "NOT_EXECUTED_DEPENDENCY"
    );
    assert_eq!(result["query_results"][1]["execution_state"], "FAILED");
    assert_eq!(
        result["query_results"][2]["errors"][0]["code"],
        "SOURCE_ACCESS_DENIED"
    );
    assert_eq!(
        result["query_results"][3]["errors"][0]["code"],
        "SEMANTIC_REFERENCE_UNAVAILABLE"
    );
    for key in ["total_rows", "total_pages", "total_bytes"] {
        assert_eq!(result[key], 0);
    }
    assert_eq!(result["pages"], json!([]));
    assert_eq!(result["processing"], json!([]));
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
    assert_eq!(manifest["relations"], json!([]));
    assert_eq!(manifest["pages"], json!([]));
    assert_eq!(
        manifest["canonical_semantic_response"]["queries"],
        json!([])
    );
    assert_eq!(
        manifest["canonical_semantic_response"]["query_results"],
        result["query_results"]
    );
    result["query_results"].clone()
}
