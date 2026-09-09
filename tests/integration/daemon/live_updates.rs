//! Real public queries against an incremental daemon and an independent clean state root.
use super::*;

fn clean_fixture(
    original: &ProductionFixture,
    registration: &Path,
    stack: &InstalledProductionStack,
) -> ProductionFixture {
    let mut clean = ProductionFixture::new();
    // Copy only the pre-start operational registration, never a fabric epoch/provider cache.
    // Both daemons read the same authorized source root and preserve its identity namespace.
    let database = clean.state.join("operational.sqlite3");
    fs::remove_file(&database).unwrap();
    OperationalStore::open(registration)
        .unwrap()
        .backup_to(&database)
        .unwrap();
    clean.workspace = original.workspace.clone();
    let mut policy = clean.launch_policy();
    policy.workspace_ids = vec![original.workspace.public_id()];
    clean.write_launch_policy(&policy);
    clean.bind_installed_adapter(stack, "policy-one", 0x11);
    clean
}

#[derive(Debug, PartialEq)]
struct SemanticObservation {
    schema: Vec<(String, String)>,
    rows: Vec<Value>,
    processing: Value,
}

fn public_query(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    mut request: Value,
) -> SemanticObservation {
    eprintln!("mixed clean comparison: {phase}");
    request["semantic_request_id"] = json!(format!("request:clean-compare-{phase}"));
    request["freshness"] = json!({"policy": "await_latest", "deadline_ms": 120_000});
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "query", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": request, "delivery": "resource"}},
            {"id": "page", "operation": "read_resource", "uri": {"$ref": "query.structured_content.pages.0.uri"}},
            {"id": "status", "operation": "call_tool", "name": "get_code_graph_status"}
        ]),
    );
    let path = write_modern_client_scenario(fixture, phase, &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{phase}: {result}");
    assert_eq!(result["freshness"], "CURRENT", "{phase}: {result}");
    assert_eq!(
        result["total_pages"], 1,
        "fixture must fit one bounded page"
    );
    let status = modern_structured(modern_step(&report, "status"));
    let source = &status["source_observations"][0];
    assert_eq!(source["runnable_pending"], false, "{phase}: {source}");
    assert_eq!(
        source["selected_source_generation"],
        result["source_generation"]
    );
    let bytes = STANDARD
        .decode(modern_step(&report, "page")[0]["blob"].as_str().unwrap())
        .unwrap();
    let reader =
        arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None).unwrap();
    let schema = reader
        .schema()
        .fields()
        .iter()
        .map(|field| (field.name().clone(), field.data_type().to_string()))
        .collect();
    let batches = reader.map(Result::unwrap).collect::<Vec<_>>();
    let mut writer = arrow::json::WriterBuilder::new()
        .with_explicit_nulls(true)
        .build::<_, arrow::json::writer::JsonArray>(Vec::new());
    writer
        .write_batches(&batches.iter().collect::<Vec<_>>())
        .unwrap();
    writer.finish().unwrap();
    let mut rows: Vec<Value> = serde_json::from_slice(&writer.into_inner()).unwrap();
    for row in &mut rows {
        let fields = row.as_object_mut().unwrap();
        // These identify independently executed observations, not semantic entities/occurrences.
        // Canonical IDs, file/context IDs, digests, positions, kinds and all relationships remain.
        for name in [
            "source_generation",
            "provider_run_id",
            "provider_observation_id",
        ] {
            fields.remove(name);
        }
        if let Some(context) = fields.get_mut("source_context") {
            // A source-context handle explicitly includes snapshot/generation/disclosure identity.
            // Preserve its entire delivered byte/range payload and compare its canonical subject.
            assert!(context["source_context_id"].as_str().is_some());
            context.as_object_mut().unwrap().remove("source_context_id");
        }
    }
    let mut processing = result["processing"].clone();
    for summary in processing.as_array_mut().unwrap() {
        summary.as_object_mut().unwrap().remove("source_generation");
    }
    SemanticObservation {
        schema,
        rows,
        processing,
    }
}

fn four_forms(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    expected_names: &[&str],
) -> Vec<SemanticObservation> {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "unused",
        "function declarations",
    );
    let entities = public_query(
        fixture,
        stack,
        &format!("{phase}-entities"),
        request.clone(),
    );
    let names = entities
        .rows
        .iter()
        .map(|row| row["name"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(names, expected_names.iter().copied().collect(), "{phase}");
    assert_eq!(
        entities.processing[0]["remaining_partitions"],
        if phase.contains("broken") { 1 } else { 0 },
        "{phase}: terminal coverage"
    );
    let subjects = entities
        .rows
        .iter()
        .map(|row| json!({"entity_id": row["public_entity_id"]}))
        .collect::<Vec<_>>();
    request["queries"] = json!([{
        "request": "retrieve facts about code", "query_id": "facts", "about": subjects,
        "facts": ["declarations"], "return": {"limit": {"maximum_results": 32}}
    }]);
    let declarations = public_query(
        fixture,
        stack,
        &format!("{phase}-declarations"),
        request.clone(),
    );
    request["queries"] = json!([{
        "request": "follow code relationships", "query_id": "calls", "starting_from": subjects,
        "relationship": "calls", "direction": "outgoing", "distance": "one relationship step",
        "return": {"limit": {"maximum_results": 32}}
    }]);
    let calls = public_query(fixture, stack, &format!("{phase}-calls"), request.clone());
    let by_id = entities
        .rows
        .iter()
        .map(|row| {
            (
                row["public_entity_id"].as_str().unwrap(),
                row["name"].as_str().unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let pairs = calls
        .rows
        .iter()
        .map(|row| {
            (
                by_id[row["public_source_entity_id"].as_str().unwrap()],
                by_id[row["public_target_entity_id"].as_str().unwrap()],
            )
        })
        .collect::<BTreeSet<_>>();
    let python_target = if names.contains("py_new") {
        "py_new"
    } else {
        "py_leaf"
    };
    let rust_target = if names.contains("fixture::rust_new") {
        "fixture::rust_new"
    } else {
        "fixture::rust_leaf"
    };
    let mut expected_pairs = BTreeSet::from([("py_caller", python_target)]);
    if names.contains("fixture::rust_caller") {
        expected_pairs.insert(("fixture::rust_caller", rust_target));
    }
    assert_eq!(pairs, expected_pairs);
    assert_eq!(
        calls.rows.len(),
        expected_pairs.len(),
        "one direct call per available caller"
    );
    request["queries"] = json!([{
        "request": "retrieve source and syntax context", "query_id": "source", "about": subjects,
        "context": "exact source span", "return": {"maximum_source_bytes": 1024, "limit": {"maximum_results": 32}}
    }]);
    let source = public_query(fixture, stack, &format!("{phase}-source"), request);
    assert_eq!(source.rows.len(), entities.rows.len());
    for row in &source.rows {
        let name = row["name"].as_str().unwrap();
        let expected = if row["language"] == "rust" {
            format!("pub fn {}() -> u32", name.rsplit("::").next().unwrap())
        } else {
            name.to_owned()
        };
        assert_eq!(row["source_context"]["text"], expected);
        assert_eq!(row["source_context"]["complete"], true);
    }
    vec![entities, declarations, calls, source]
}

#[test]
fn mixed_live_updates_equal_independent_clean_public_queries() {
    const PYTHON_INITIAL: &[u8] =
        b"def py_leaf():\n    return 1\ndef py_caller():\n    return py_leaf()\n";
    const RUST_INITIAL: &[u8] =
        b"pub fn rust_leaf() -> u32 { 1 }\npub fn rust_caller() -> u32 { rust_leaf() }\n";
    const PYTHON_EDITED: &[u8] =
        b"def py_new():\n    return 2\ndef py_caller():\n    return py_new()\n";
    const RUST_EDITED: &[u8] =
        b"pub fn rust_new() -> u32 { 2 }\npub fn rust_caller() -> u32 { rust_new() }\n";
    let fixture = ProductionFixture::with_source(PYTHON_INITIAL);
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let workspace = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir(workspace.join("src")).unwrap();
    fs::write(workspace.join("Cargo.toml"), "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\ntest = false\ndoctest = false\n").unwrap();
    fs::write(
        workspace.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(workspace.join("src/lib.rs"), RUST_INITIAL).unwrap();
    let registration = fixture.root().join("registration.sqlite3");
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
        store.backup_to(&registration).unwrap();
    }
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let initial = four_forms(
        &fixture,
        &stack,
        "initial",
        &[
            "py_leaf",
            "py_caller",
            "fixture::rust_leaf",
            "fixture::rust_caller",
        ],
    );
    let scenarios: [(&str, &[u8], &[u8], &[&str]); 3] = [
        (
            "edited",
            PYTHON_EDITED,
            RUST_EDITED,
            &[
                "py_new",
                "py_caller",
                "fixture::rust_new",
                "fixture::rust_caller",
            ],
        ),
        (
            "broken",
            PYTHON_EDITED,
            b"pub fn rust_caller() -> u32 { missing() }\n",
            &["py_new", "py_caller"],
        ),
        (
            "restored",
            PYTHON_INITIAL,
            RUST_INITIAL,
            &[
                "py_leaf",
                "py_caller",
                "fixture::rust_leaf",
                "fixture::rust_caller",
            ],
        ),
    ];
    for (phase, python, rust, names) in scenarios {
        // A discarded intermediate Python edit exercises burst coalescing before the final pair.
        fs::write(workspace.join("sample.py"), b"def transient():\n    pass\n").unwrap();
        fs::write(workspace.join("sample.py"), python).unwrap();
        fs::write(workspace.join("src/lib.rs"), rust).unwrap();
        let incremental = four_forms(&fixture, &stack, phase, names);
        let clean = clean_fixture(&fixture, &registration, &stack);
        let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
        let expected = four_forms(&clean, &stack, &format!("clean-{phase}"), names);
        assert_eq!(
            incremental, expected,
            "{phase}: exact public semantic comparison"
        );
        if phase == "restored" {
            assert_eq!(
                incremental, initial,
                "restoring bytes must restore canonical semantics"
            );
        }
        clean_supervisor.stop();
    }
    supervisor.stop();
}
