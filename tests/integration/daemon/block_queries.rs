use super::*;

fn resource_bytes(report: &Value, step: &str) -> Vec<u8> {
    let item = &modern_step(report, step)[0];
    if let Some(blob) = item["blob"].as_str() {
        STANDARD.decode(blob).unwrap()
    } else {
        item["text"].as_str().unwrap().as_bytes().to_vec()
    }
}

fn block_rows(report: &Value, page: usize) -> Vec<Value> {
    let bytes = resource_bytes(report, &format!("page{page}"));
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
    serde_json::from_slice(&writer.into_inner()).unwrap()
}

fn repeated_blocks(fixture: &ProductionFixture, stack: &InstalledProductionStack, phase: &str) {
    let declarations = canonical_diagnostic_rows(fixture, "fact.code_declaration");
    let subject = |name: &str| {
        declarations
            .iter()
            .find(|row| row["name"] == name && row["entity_kind"] == "function")
            .unwrap()["public_entity_id"]
            .clone()
    };
    let mut queries = vec![
        json!({"request":"find code entities", "query_id":"functions", "looking_for":"Python function declarations"}),
        json!({"request":"find code entities", "query_id":"classes", "looking_for":"Python class declarations"}),
    ];
    for name in ["first", "second"] {
        queries.extend([
            json!({"request":"retrieve facts about code", "query_id":format!("facts-{name}"), "about":[{"entity_id":subject(name)}], "facts":["declarations"]}),
            json!({"request":"follow code relationships", "query_id":format!("calls-{name}"), "starting_from":[{"entity_id":subject(name)}], "relationship":"calls", "direction":"outgoing", "distance":"one step"}),
            json!({"request":"retrieve source and syntax context", "query_id":format!("source-{name}"), "about":[{"entity_id":subject(name)}], "context":"exact source span", "return":{"maximum_source_bytes":128}}),
        ]);
    }
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:blocks-{phase}"),
        "unused",
    );
    request["queries"] = Value::Array(queries);
    let mut steps = vec![
        json!({"id":"query", "operation":"call_tool", "name":"query_code_graph", "arguments":{"request":request,"delivery":"resource"}}),
        json!({"id":"manifest", "operation":"read_resource", "uri":{"$ref":"query.structured_content.manifest.uri"}}),
    ];
    for page in 0..8 {
        steps.push(json!({"id":format!("page{page}"),"operation":"read_resource","uri":{"$ref":format!("query.structured_content.pages.{page}.uri")}}));
    }
    let scenario =
        modern_client_scenario(fixture, stack, "policy-one", json!([]), Value::Array(steps));
    let path = write_modern_client_scenario(fixture, &format!("blocks-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{report}");
    assert_eq!(result["processing"].as_array().unwrap().len(), 8);
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
    let bindings = manifest["canonical_semantic_response"]["queries"]
        .as_array()
        .unwrap();
    assert_eq!(bindings.len(), 8);
    let unique = bindings
        .iter()
        .map(|b| b["relation_id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        unique.len(),
        8,
        "repeated forms own distinct output relations"
    );
    let rows = |query: &str| {
        let relation = &bindings.iter().find(|b| b["query_id"] == query).unwrap()["relation_id"];
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
    let names = |query: &str| {
        rows(query)
            .iter()
            .map(|row| row["name"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(
        names("functions"),
        BTreeSet::from(["first".to_owned(), "second".to_owned(), "helper".to_owned()])
    );
    assert_eq!(names("classes"), BTreeSet::from(["Box".to_owned()]));
    for name in ["first", "second"] {
        assert_eq!(
            names(&format!("facts-{name}")),
            BTreeSet::from([name.to_owned()])
        );
        let source = rows(&format!("source-{name}"));
        assert_eq!(source.len(), 1);
        assert_eq!(source[0]["source_context"]["text"], name);
    }
    let first = rows("calls-first");
    assert_eq!(first.len(), 1);
    assert_eq!(first[0]["public_target_entity_id"], subject("helper"));
    assert!(
        rows("calls-second").is_empty(),
        "one block's subjects cannot enter another's request relation"
    );
}

#[test]
fn pragmatic_repeated_first_four_blocks_keep_independent_results_and_reopen() {
    let fixture = ProductionFixture::with_source(b"def helper(value: int) -> int:\n    return value\ndef first(value: int) -> int:\n    return helper(value)\ndef second() -> int:\n    return 2\nclass Box:\n    pass\n");
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
    }
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    repeated_blocks(&fixture, &stack, "initial");
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    repeated_blocks(&fixture, &stack, "reopened");
    supervisor.stop();
}

fn prior_blocks(fixture: &ProductionFixture, stack: &InstalledProductionStack, phase: &str) {
    let module = canonical_diagnostic_rows(fixture, "fact.code_module")[0]["public_entity_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let prior = |query: &str| json!({"results_of":query,"select":"entities"});
    let queries = json!([
        {"request":"find code entities","query_id":"functions","looking_for":"Python function declarations","return":{"limit":{"maximum_results":1}}},
        {"request":"find code entities","query_id":"classes","looking_for":"Python class declarations"},
        {"request":"find code entities","query_id":module,"looking_for":"Rust function declarations"},
        {"request":"retrieve facts about code","query_id":"facts","about":[prior("functions")],"facts":["declarations"]},
        {"request":"retrieve facts about code","query_id":"combined","about":[prior("functions"),prior("classes"),prior("functions")],"facts":["declarations"]},
        {"request":"follow code relationships","query_id":"calls","starting_from":[prior("functions")],"relationship":"calls","direction":"outgoing","distance":"one step"},
        {"request":"retrieve source and syntax context","query_id":"source","about":[prior("functions")],"context":"exact source span","return":{"maximum_source_bytes":128}},
        {"request":"retrieve facts about code","query_id":"empty","about":[prior(&module)],"facts":["declarations"]},
        {"request":"retrieve facts about code","query_id":"module-empty","about":[prior(&module)],"facts":["module metadata"]}
    ]);
    let count = queries.as_array().unwrap().len();
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:prior-{phase}"),
        "unused",
    );
    request["queries"] = queries;
    let mut steps = vec![
        json!({"id":"query","operation":"call_tool","name":"query_code_graph","arguments":{"request":request,"delivery":"resource"}}),
        json!({"id":"manifest","operation":"read_resource","uri":{"$ref":"query.structured_content.manifest.uri"}}),
    ];
    for page in 0..count {
        steps.push(json!({"id":format!("page{page}"),"operation":"read_resource","uri":{"$ref":format!("query.structured_content.pages.{page}.uri")}}));
    }
    let scenario =
        modern_client_scenario(fixture, stack, "policy-one", json!([]), Value::Array(steps));
    let path = write_modern_client_scenario(fixture, &format!("prior-{phase}"), &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED");
    let processing = result["processing"].as_array().unwrap();
    assert_eq!(
        processing
            .iter()
            .find(|p| p["query_id"] == "functions")
            .unwrap()["additional_rows"],
        true
    );
    let manifest: Value = serde_json::from_slice(&resource_bytes(&report, "manifest")).unwrap();
    let canonical = &manifest["canonical_semantic_response"];
    assert_eq!(canonical["query_dependencies"].as_array().unwrap().len(), 8);
    let mut outputs = BTreeMap::new();
    for binding in canonical["queries"].as_array().unwrap() {
        let relation = manifest["relations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["relation_id"] == binding["relation_id"])
            .unwrap();
        assert_eq!(relation["page_count"], 1);
        outputs.insert(
            binding["query_id"].as_str().unwrap(),
            block_rows(
                &report,
                usize::try_from(relation["page_start"].as_u64().unwrap()).unwrap(),
            ),
        );
    }
    let names = |query: &str| {
        outputs[query]
            .iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(names("functions"), BTreeSet::from(["first"]));
    assert_eq!(
        names("facts"),
        BTreeSet::from(["first"]),
        "consumer must not rerun the unlimited producer"
    );
    assert_eq!(
        names("combined"),
        BTreeSet::from(["Box", "first"]),
        "fan-in is a subject union without duplicates"
    );
    assert_eq!(outputs["source"][0]["source_context"]["text"], "first");
    assert_eq!(outputs["calls"].len(), 1);
    let helper = canonical_diagnostic_rows(fixture, "fact.code_declaration")
        .into_iter()
        .find(|row| row["name"] == "helper")
        .unwrap();
    assert_eq!(
        outputs["calls"][0]["public_target_entity_id"],
        helper["public_entity_id"]
    );
    assert!(outputs["empty"].is_empty());
    assert!(
        outputs["module-empty"].is_empty(),
        "a producer block ID cannot become an explicit entity subject"
    );
}

#[test]
fn pragmatic_prior_entity_results_fan_out_and_union_through_installed_clients_and_reopen() {
    let fixture = ProductionFixture::with_source(b"def helper(value: int) -> int:\n    return value\ndef first(value: int) -> int:\n    return helper(value)\ndef second() -> int:\n    return 2\nclass Box:\n    pass\n");
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
    }
    let stack = InstalledProductionStack::build();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    prior_blocks(&fixture, &stack, "initial");
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    prior_blocks(&fixture, &stack, "reopened");
    supervisor.stop();
}
