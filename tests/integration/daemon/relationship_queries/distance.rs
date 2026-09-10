use super::super::block_queries::{block_rows, resource_bytes};
use super::super::*;

const PYTHON: &[u8] = b"def leaf() -> None:\n    pass\ndef left() -> None:\n    leaf()\n    leaf()\ndef right() -> None:\n    leaf()\ndef start() -> None:\n    left()\n    right()\ndef cycle_a() -> None:\n    cycle_b()\ndef cycle_b() -> None:\n    cycle_a()\ndef boundary_start() -> None:\n    from bridge import bridge_middle\n    bridge_middle()\ndef boundary_end() -> None:\n    leaf()\n";
const RUST: &str = "pub fn leaf() {}\npub fn left() { leaf(); leaf(); }\npub fn right() { leaf(); }\npub fn start() { left(); right(); }\npub fn cycle_a() { cycle_b(); }\npub fn cycle_b() { cycle_a(); }\nmod bridge;\npub fn boundary_start() { bridge::bridge_middle(); }\npub fn boundary_end() { leaf(); }\n";

fn short_name(value: &str) -> &str {
    value
        .rsplit("::")
        .next()
        .unwrap()
        .rsplit('.')
        .next()
        .unwrap()
}

fn queries(fixture: &ProductionFixture, language: &str) -> Vec<Value> {
    let declarations = canonical_diagnostic_rows(fixture, "fact.code_declaration");
    let id = |name: &str| {
        declarations
            .iter()
            .find(|row| {
                row["language"] == language
                    && row["name"].as_str().unwrap().rsplit("::").next() == Some(name)
            })
            .unwrap()["public_entity_id"]
            .clone()
    };
    let label = if language == "python" {
        "Python"
    } else {
        "Rust"
    };
    let mut queries = vec![
        json!({"request":"find code entities","query_id":"seed","looking_for":format!("{label} function `start`")}),
        json!({"request":"find code entities","query_id":"absent","looking_for":format!("{label} function `absent`")}),
    ];
    for (query, subject, direction, distance) in [
        ("one", "start", "outgoing", "one step"),
        ("boundary", "boundary_start", "outgoing", "three steps"),
        ("two", "start", "outgoing", "two relationship steps"),
        ("three", "start", "outgoing", "3 steps"),
        (
            "through",
            "start",
            "outgoing",
            "up to five relationship steps",
        ),
        ("incoming", "leaf", "incoming", "two steps"),
        ("cycle", "cycle_a", "outgoing", "three steps"),
        ("cycle-through", "cycle_a", "outgoing", "up to 8 steps"),
    ] {
        queries.push(json!({"request":"follow code relationships","query_id":query,"starting_from":[{"entity_id":id(subject)},{"entity_id":id(subject)}],"relationship":"calls","direction":direction,"distance":distance}));
    }
    for (query, producer) in [("prior", "seed"), ("empty", "absent")] {
        queries.push(json!({"request":"follow code relationships","query_id":query,"starting_from":[{"results_of":producer,"select":"entities"},{"results_of":producer,"select":"entities"}],"relationship":"calls","distance":"up to two steps"}));
    }
    let calls = canonical_diagnostic_rows(fixture, "fact.code_call_site");
    let left = calls
        .iter()
        .find(|row| {
            row["language"] == language
                && row["target_name"]
                    .as_str()
                    .is_some_and(|name| short_name(name) == "left")
        })
        .unwrap()["target_name"]
        .clone();
    queries.push(json!({"request":"follow code relationships","query_id":"filtered","starting_from":[format!("{label} function `start`")],"relationship":"calls","distance":"up to two relationship steps","where":[{"property":"target name","operator":"does not equal","value":left}]}));
    queries.extend(stopping_queries(label));
    queries.push(json!({"request":"follow code relationships","query_id":"stop-unavailable","starting_from":[format!("{label} function `start`")],"relationship":"calls","distance":"up to two steps","stop_when":["do not traverse bodies of unindexed external entities"]}));
    queries.push(json!({"request":"retrieve facts about code","query_id":"stop-dependent","about":[{"results_of":"stop-unavailable","select":"facts"}],"facts":["declarations"]}));
    for query in &mut queries {
        if query["request"] == "follow code relationships" {
            query.as_object_mut().unwrap().entry("where").or_insert_with(|| json!([]))
                .as_array_mut().unwrap().push(json!({"property":"resolution","operator":"equals","value":"resolved_declaration"}));
        }
    }
    queries
}

fn stopping_queries(language: &str) -> Vec<Value> {
    [
        ("stop-through", "start", "outgoing", "up to five steps", vec!["left", "left"]),
        ("stop-exact", "start", "outgoing", "two steps", vec!["left"]),
        ("stop-start", "start", "outgoing", "one step", vec!["start"]),
        ("stop-incoming", "leaf", "incoming", "two steps", vec!["left"]),
        ("stop-both", "start", "outgoing", "up to three steps", vec!["left", "right"]),
        ("stop-cycle", "cycle_a", "outgoing", "up to eight steps", vec!["cycle_b"]),
        ("stop-absent", "start", "outgoing", "up to five steps", vec!["absent"]),
    ].into_iter().map(|(query, start, direction, distance, stops)| {
        json!({"request":"follow code relationships", "query_id":query,
            "starting_from":[format!("{language} function `{start}`")],
            "relationship":"calls", "direction":direction, "distance":distance,
            "stop_when":stops.into_iter().map(|name| format!("{language} function `{name}`")).collect::<Vec<_>>()})
    }).collect()
}

fn run(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    language: &str,
    phase: &str,
    bounded: bool,
) -> BTreeMap<String, Vec<Value>> {
    let phase = format!("{phase}-{bounded}");
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:walk-{language}-{phase}"),
        "unused",
    );
    let mut queries = queries(fixture, language);
    if bounded {
        queries.retain(|query| query["query_id"] == "boundary");
        request["scope"]["source_boundaries"] = json!([{"kind":"path", "root":if language == "python" { "sample.py" } else { "src/lib.rs" }}]);
    }
    request["queries"] = json!(queries);
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
    let path =
        write_modern_client_scenario(fixture, &format!("walk-{language}-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
    if !bounded {
        let outcomes = result["query_results"].as_array().unwrap();
        let outcome = |id: &str| outcomes.iter().find(|row| row["query_id"] == id).unwrap();
        assert_eq!(outcome("stop-unavailable")["execution_state"], "FAILED");
        assert_eq!(
            outcome("stop-unavailable")["errors"][0]["code"],
            "SEMANTIC_REFERENCE_UNAVAILABLE"
        );
        assert_eq!(
            outcome("stop-unavailable")["errors"][0]["subject_id"],
            "stop_when"
        );
        assert_eq!(
            outcome("stop-dependent")["execution_state"],
            "NOT_EXECUTED_DEPENDENCY"
        );
    }
    for status in result["processing"].as_array().unwrap() {
        if status["query_id"] != "one" && status["query_id"] != "stop-start" {
            assert_ne!(
                status["scope"], "selected_rust_call_owners",
                "multi-step coverage must include later owners"
            );
        }
    }
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
    let steps = result["pages"].as_array().unwrap().iter().enumerate().map(|(index, page)| json!({"id":format!("page{index}"),"operation":"read_resource","uri":page["uri"]})).collect::<Vec<_>>();
    let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
    let path = write_modern_client_scenario(
        fixture,
        &format!("walk-pages-{language}-{phase}"),
        &scenario,
    );
    let pages = modern_client_report(&run_modern_client(stack, &path));
    let rows = manifest["canonical_semantic_response"]["queries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|query| {
            let relation = manifest["relations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["relation_id"] == query["relation_id"])
                .unwrap();
            let start = usize::try_from(relation["page_start"].as_u64().unwrap()).unwrap();
            let count = usize::try_from(relation["page_count"].as_u64().unwrap()).unwrap();
            let mut rows = (start..start + count)
                .flat_map(|page| block_rows(&pages, page))
                .collect::<Vec<_>>();
            rows.sort_by_cached_key(Value::to_string);
            (query["query_id"].as_str().unwrap().to_owned(), rows)
        })
        .collect::<BTreeMap<_, _>>();
    if bounded {
        assert!(
            rows["boundary"].is_empty(),
            "walk must not cross an excluded source file"
        );
    } else {
        assert_walks(&rows);
    }
    rows
}

fn assert_walks(rows: &BTreeMap<String, Vec<Value>>) {
    let pairs = |query: &str| {
        let mut pairs = rows[query]
            .iter()
            .map(|row| {
                let name = |key: &str| short_name(row[key].as_str().unwrap()).to_owned();
                (name("caller_name"), name("target_name"))
            })
            .collect::<Vec<_>>();
        pairs.sort();
        pairs
    };
    let expected = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        pairs("one"),
        expected(&[("start", "left"), ("start", "right")])
    );
    assert_eq!(
        pairs("two"),
        expected(&[("left", "leaf"), ("left", "leaf"), ("right", "leaf")])
    );
    assert_eq!(pairs("boundary"), expected(&[("boundary_end", "leaf")]));
    assert!(rows["three"].is_empty());
    assert!(rows["empty"].is_empty());
    assert_eq!(
        pairs("incoming"),
        expected(&[
            ("bridge_middle", "boundary_end"),
            ("start", "left"),
            ("start", "right")
        ])
    );
    assert_eq!(
        pairs("through"),
        expected(&[
            ("left", "leaf"),
            ("left", "leaf"),
            ("right", "leaf"),
            ("start", "left"),
            ("start", "right")
        ])
    );
    assert_eq!(rows["through"], rows["prior"]);
    assert_eq!(pairs("cycle"), expected(&[("cycle_a", "cycle_b")]));
    assert_eq!(
        pairs("cycle-through"),
        expected(&[("cycle_a", "cycle_b"), ("cycle_b", "cycle_a")])
    );
    assert_eq!(
        pairs("filtered"),
        expected(&[("right", "leaf"), ("start", "right")])
    );
    assert_eq!(
        pairs("stop-through"),
        expected(&[("right", "leaf"), ("start", "left"), ("start", "right")])
    );
    assert_eq!(pairs("stop-exact"), expected(&[("right", "leaf")]));
    assert!(rows["stop-start"].is_empty());
    assert_eq!(
        pairs("stop-incoming"),
        expected(&[("bridge_middle", "boundary_end"), ("start", "right")])
    );
    assert_eq!(pairs("stop-both"), pairs("one"));
    assert_eq!(pairs("stop-cycle"), expected(&[("cycle_a", "cycle_b")]));
    assert_eq!(rows["stop-absent"], rows["through"]);
    let sites = rows["two"]
        .iter()
        .map(|row| row["public_call_site_id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        sites.len(),
        3,
        "distinct call sites survive shared endpoints"
    );
}

#[test]
fn bounded_call_walks_preserve_witnesses_filters_cycles_priors_and_exact_reopen() {
    let fixture = ProductionFixture::with_source(PYTHON);
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"walks\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"walks\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), RUST).unwrap();
    fs::write(
        root.join("src/bridge.rs"),
        "pub fn bridge_middle() { crate::boundary_end(); }\n",
    )
    .unwrap();
    fs::write(
        root.join("bridge.py"),
        "from sample import boundary_end\ndef bridge_middle() -> None:\n    boundary_end()\n",
    )
    .unwrap();
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let selected = wait_for_semantic_activation_with_timeout(&fixture, Duration::from_secs(180));
    let mut first = Vec::new();
    for language in ["python", "rust"] {
        for bounded in [false, true] {
            first.push((
                language,
                bounded,
                run(&fixture, &stack, language, "initial", bounded),
            ));
        }
    }
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    for (language, bounded, expected) in first {
        assert_eq!(
            expected,
            run(&fixture, &stack, language, "reopened", bounded)
        );
    }
    supervisor.stop();
}
