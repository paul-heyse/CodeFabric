use super::block_queries::{block_rows, resource_bytes};
use super::*;

const PYTHON: &str = "def zulu(x: int) -> int:\n    return beta(alpha(x))\ndef alpha(x: int) -> int:\n    return x\ndef beta(x: int) -> int:\n    return x\n";
const RUST: &str = "pub fn zulu(x: u8) -> u8 { beta(alpha(x)) }\npub fn alpha(x: u8) -> u8 { x }\npub fn beta(x: u8) -> u8 { x }\n";

#[allow(
    clippy::too_many_lines,
    reason = "one public scenario compares all first-four forms and their stable ordering"
)]
fn ordered_queries(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
) -> Vec<Value> {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:ordering-{phase}"),
        "unused",
    );
    request["queries"] = json!([
        {"request":"find code entities","query_id":"default","looking_for":"Python function declarations"},
        {"request":"find code entities","query_id":"ascending","looking_for":"Python function declarations","return":{"order_by":["name"]}},
        {"request":"find code entities","query_id":"descending","looking_for":"Python function declarations","return":{"order_by":["name descending"]}},
        {"request":"find code entities","query_id":"rust","looking_for":"Rust function declarations","return":{"order_by":["name descending"]}},
        {"request":"find code entities","query_id":"position","looking_for":"Rust function declarations","return":{"order_by":["source position descending"]}},
        {"request":"find code entities","query_id":"within","looking_for":"Python function declarations","within":[{"source_location":{"source_file":"sample.py","start_byte":PYTHON.find("def alpha").unwrap(),"end_byte":PYTHON.len()}}],"return":{"order_by":["name descending"],"limit":{"maximum_results":1}}},
        {"request":"retrieve facts about code","query_id":"facts","about":[{"results_of":"descending","select":"entities"}],"facts":["declarations"],"return":{"order_by":["name ascending"]}},
        {"request":"retrieve facts about code","query_id":"rust-facts","about":[{"results_of":"rust","select":"entities"}],"facts":["declarations"],"return":{"order_by":["source position descending"]}},
        {"request":"follow code relationships","query_id":"calls","starting_from":["Python function `zulu`"],"relationship":"calls","direction":"outgoing","distance":"one step","return":{"order_by":["target name descending"]}},
        {"request":"follow code relationships","query_id":"rust-calls","starting_from":["Rust function `zulu`"],"relationship":"calls","direction":"outgoing","distance":"one step","return":{"order_by":["target name descending"]}},
        {"request":"retrieve source and syntax context","query_id":"source","about":[{"results_of":"ascending","select":"entities"}],"context":"exact source span","return":{"order_by":["name descending"],"maximum_source_bytes":256}},
        {"request":"retrieve source and syntax context","query_id":"rust-source","about":[{"results_of":"rust","select":"entities"}],"context":"exact source span","return":{"order_by":["source position descending"],"maximum_source_bytes":256}}
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
    let path = write_modern_client_scenario(fixture, &format!("ordering-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
    let pages = result["pages"].as_array().unwrap();
    assert!(pages.len() <= 128);
    let steps = pages.iter().enumerate().map(|(index,page)| json!({"id":format!("page{index}"),"operation":"read_resource","uri":page["uri"]})).collect::<Vec<_>>();
    let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
    let path = write_modern_client_scenario(fixture, &format!("ordering-pages-{phase}"), &scenario);
    let pages = modern_client_report(&run_modern_client(stack, &path));
    let binding = |query: &str| {
        manifest["canonical_semantic_response"]["queries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["query_id"] == query)
            .unwrap()
    };
    let rows = |query: &str| {
        let relation = &binding(query)["relation_id"];
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
    for (query, expected) in [
        ("default", ["zulu", "alpha", "beta"]),
        ("ascending", ["alpha", "beta", "zulu"]),
        ("descending", ["zulu", "beta", "alpha"]),
        ("facts", ["alpha", "beta", "zulu"]),
        ("source", ["zulu", "beta", "alpha"]),
        (
            "rust",
            [
                "ordering_queries::zulu",
                "ordering_queries::beta",
                "ordering_queries::alpha",
            ],
        ),
        (
            "position",
            [
                "ordering_queries::beta",
                "ordering_queries::alpha",
                "ordering_queries::zulu",
            ],
        ),
        (
            "rust-facts",
            [
                "ordering_queries::beta",
                "ordering_queries::alpha",
                "ordering_queries::zulu",
            ],
        ),
        (
            "rust-source",
            [
                "ordering_queries::beta",
                "ordering_queries::alpha",
                "ordering_queries::zulu",
            ],
        ),
    ] {
        let actual = rows(query);
        assert_eq!(
            actual
                .iter()
                .map(|row| row["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected,
            "{query}"
        );
        if query.ends_with("source") {
            for (row, name) in actual.iter().zip(expected) {
                let text = row["source_context"]["text"].as_str().unwrap();
                assert!(
                    text.contains(name.rsplit("::").next().unwrap()),
                    "{query}: {text}"
                );
            }
        }
    }
    for (query, expected) in [
        ("calls", ["sample.beta", "sample.alpha"]),
        (
            "rust-calls",
            ["ordering_queries::beta", "ordering_queries::alpha"],
        ),
    ] {
        assert_eq!(
            rows(query)
                .iter()
                .map(|row| row["target_name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(rows("within")[0]["name"], "beta");
    assert_eq!(rows("within").len(), 1);
    let processing = result["processing"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["query_id"] == "within")
        .unwrap();
    assert_eq!(processing["additional_rows"], true);
    assert_eq!(
        binding("descending")["return_directives"],
        json!([{"field":"return.order-by","ordinal":0,"value":"name descending"}])
    );
    let mut invalid_steps = Vec::new();
    for (id, keys) in [
        ("unknown", vec!["safest function"]),
        ("duplicate", vec!["name", "name descending"]),
    ] {
        let mut invalid = semantic_request(
            &fixture.workspace.public_id(),
            &format!("request:ordering-{id}-{phase}"),
            "Python function declarations",
        );
        invalid["queries"][0]["return"] = json!({"order_by":keys});
        invalid_steps.push(json!({"id":id,"operation":"call_tool","name":"query_code_graph","arguments":{"request":invalid,"delivery":"resource"}}));
    }
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!(invalid_steps),
    );
    let path =
        write_modern_client_scenario(fixture, &format!("ordering-invalid-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    for id in ["unknown", "duplicate"] {
        let rejected = modern_structured(modern_step(&report, id));
        assert_eq!(rejected["execution_state"], "SUCCEEDED");
        assert_eq!(rejected["query_results"][0]["execution_state"], "FAILED");
        assert_eq!(
            rejected["query_results"][0]["errors"][0]["code"],
            if id == "unknown" {
                "SEMANTIC_REFERENCE_UNAVAILABLE"
            } else {
                "INVALID_RETURN_DIRECTIVE"
            }
        );
        assert_eq!(rejected["total_rows"], 0);
        assert_eq!(rejected["total_bytes"], 0);
        assert!(rejected["pages"].as_array().unwrap().is_empty());
    }
    rows("default")
        .into_iter()
        .chain(rows("descending"))
        .chain(rows("rust"))
        .collect()
}

#[test]
fn pragmatic_semantic_ordering_precedes_limits_and_survives_prior_reuse_and_reopen() {
    let fixture = ProductionFixture::with_source(PYTHON.as_bytes());
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"ordering_queries\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"ordering_queries\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), RUST).unwrap();
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
    }
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    // This scenario measures query ordering after native publication. The observed mixed-language
    // preparation can exceed the adapter's per-operation deadline before any query is admitted.
    let selected = wait_for_semantic_activation_with_timeout(&fixture, Duration::from_secs(180));
    let expected = ordered_queries(&fixture, &stack, "initial");
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    assert_eq!(expected, ordered_queries(&fixture, &stack, "reopened"));
    supervisor.stop();
}
