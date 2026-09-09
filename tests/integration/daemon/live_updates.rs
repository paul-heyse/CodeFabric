//! Real public queries against an incremental daemon and an independent clean state root.
use super::*;

#[test]
fn processing_remainder_pages_keep_exact_scope_across_reopen_and_updates() {
    let fixture = ProductionFixture::with_source(b"def start():\n    return 1\n");
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let root = Path::new(&fixture.workspace.root_path_display);
    for index in 0..130 {
        fs::write(
            root.join(format!("unknown_{index:03}.py")),
            format!("value = missing_{index:03}()\n"),
        )
        .unwrap();
    }
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let find = semantic_request(
        &fixture.workspace.public_id(),
        "unused",
        "Python function declarations",
    );
    let entities = public_query(&fixture, &stack, "remainder-initial", find.clone());
    assert_eq!(entities.rows.len(), 1);
    let start = entities
        .rows
        .iter()
        .find(|row| row["name"] == "start")
        .unwrap();
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "request:retained-remainder",
        "Python function declarations",
    );
    request["scope"]["languages"] = json!(["python"]);
    request["queries"] = json!([{
        "request": "follow code relationships", "query_id": "calls", "starting_from": [{"entity_id": start["public_entity_id"]}],
        "relationship": "calls", "direction": "incoming", "distance": "one relationship step",
        "return": {"limit": {"maximum_results": 32}}
    }]);
    let query_scenario = |name: &str| {
        eprintln!("processing continuation: {name}");
        let scenario = modern_client_scenario(
            &fixture,
            &stack,
            "policy-one",
            json!([]),
            json!([
                {"id": "query", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": request, "delivery": "resource"}},
                {"id": "facts", "operation": "read_resource", "uri": {"$ref": "query.structured_content.pages.0.uri"}},
                {"id": "manifest", "operation": "read_resource", "uri": {"$ref": "query.structured_content.manifest.uri"}}
            ]),
        );
        let path = write_modern_client_scenario(&fixture, name, &scenario);
        modern_client_report(&run_modern_client(&stack, &path))
    };
    let first_report = query_scenario("remainder-first");
    let first = modern_structured(modern_step(&first_report, "query"));
    assert_eq!(first["execution_state"], "SUCCEEDED", "{first}");
    assert_eq!(first["total_rows"], 0);
    let first_summary = &first["processing"][0];
    assert_eq!(first_summary["requested_partitions"], 131);
    assert_eq!(first_summary["remaining_partitions"], 130);
    assert_eq!(first_summary["remainder"].as_array().unwrap().len(), 64);
    assert_eq!(first_summary["next_offset"], 64);
    assert!(first_summary["remainder_handle"].is_string());
    let manifest_text = String::from_utf8(
        STANDARD
            .decode(
                modern_step(&first_report, "manifest")[0]["blob"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    assert!(
        !manifest_text.contains("table_root"),
        "private selected table leaked"
    );
    assert!(!manifest_text.contains(&fixture.state.to_string_lossy().to_string()));
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    for index in 0..130 {
        fs::write(
            root.join(format!("unknown_{index:03}.py")),
            format!("value = {index}\n"),
        )
        .unwrap();
    }
    let repaired = public_query(&fixture, &stack, "remainder-repaired", find);
    assert_eq!(repaired.rows.len(), 1);
    let scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "middle", "operation": "call_tool", "name": "get_code_graph_processing", "arguments": {"daemon_query_id": first["daemon_query_id"], "query_id": "calls", "offset": 64}},
            {"id": "wrong-block", "operation": "call_tool", "name": "get_code_graph_processing", "arguments": {"daemon_query_id": first["daemon_query_id"], "query_id": "another-block", "offset": 64}, "expect_error": "CLIENT_OPERATION_FAILED"},
            {"id": "outside", "operation": "call_tool", "name": "get_code_graph_processing", "arguments": {"daemon_query_id": first["daemon_query_id"], "query_id": "calls", "offset": 192}, "expect_error": "CLIENT_OPERATION_FAILED"},
            {"id": "last", "operation": "call_tool", "name": "get_code_graph_processing", "arguments": {"daemon_query_id": first["daemon_query_id"], "query_id": "calls", "offset": 128}},
            {"id": "released", "operation": "call_tool", "name": "get_code_graph_processing", "arguments": {"daemon_query_id": first["daemon_query_id"], "query_id": "calls", "offset": 64}, "expect_error": "CLIENT_OPERATION_FAILED"}
        ]),
    );
    let path = write_modern_client_scenario(&fixture, "remainder-pages", &scenario);
    let pages = modern_client_report(&run_modern_client(&stack, &path));
    for (step, code) in [
        ("wrong-block", "RESOURCE_NOT_FOUND"),
        ("outside", "RANGE_NOT_SATISFIABLE"),
        ("released", "RESOURCE_RELEASED"),
    ] {
        assert!(
            modern_step(&pages, step)["public_error"]
                .as_str()
                .unwrap()
                .contains(code),
            "unexpected failure for {step}: {}",
            modern_step(&pages, step)
        );
    }
    assert_ne!(
        modern_structured(modern_step(&pages, "middle"))["processing"]["remainder_handle"],
        first_summary["remainder_handle"]
    );
    let mut paths = first_summary["remainder"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    for (name, offset, count) in [("middle", 64, 64), ("last", 128, 2)] {
        let page = modern_structured(modern_step(&pages, name));
        assert_eq!(page["epoch_id"], first["epoch_id"]);
        let summary = &page["processing"];
        assert_eq!(summary["source_generation"], first["source_generation"]);
        assert_eq!(summary["remaining_partitions"], 130);
        assert_eq!(summary["remainder_offset"], offset);
        assert_eq!(summary["remainder"].as_array().unwrap().len(), count);
        paths.extend(
            summary["remainder"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["path"].as_str().unwrap().to_owned()),
        );
    }
    assert_eq!(
        paths,
        (0..130)
            .map(|index| format!("unknown_{index:03}.py"))
            .collect::<Vec<_>>()
    );
    assert!(modern_structured(modern_step(&pages, "last"))["processing"]["next_offset"].is_null());
    supervisor.stop();
}

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

fn pending_semantic_candidate(fixture: &ProductionFixture) -> PathBuf {
    eprintln!(
        "waiting for semantic publication in {}",
        fixture.state.display()
    );
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let ready = fs::read_dir(&fixture.state)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("semantic-update-")
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "ready")
            });
        if let Some(ready) = ready {
            return ready;
        }
        if Instant::now() >= deadline {
            let journal = rusqlite::Connection::open_with_flags(
                fixture
                    .fabric_workspace_root()
                    .join("fabric-commands.sqlite3"),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .unwrap();
            let mut query = journal.prepare("SELECT state_kind, json_extract(CAST(record_jcs AS TEXT), '$.state') FROM fabric_command_record LIMIT 3").unwrap();
            let states = query
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .unwrap()
                .map(Result::unwrap)
                .collect::<Vec<_>>();
            panic!("semantic candidate did not reach publication pause: {states:?}");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn source_current_query(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    mut request: Value,
) -> (Value, Vec<RecordBatch>) {
    eprintln!("source-current query: {phase}");
    request["semantic_request_id"] = json!(format!("request:source-stage-{phase}"));
    request["freshness"] = json!({"policy": "require_source_current", "deadline_ms": 30_000});
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
    let result = modern_structured(modern_step(&report, "query")).clone();
    assert_eq!(result["execution_state"], "SUCCEEDED", "{phase}: {result}");
    assert_eq!(result["freshness"], "CURRENT", "{phase}: {result}");
    let status = modern_structured(modern_step(&report, "status"));
    let observation = &status["source_observations"][0];
    assert_eq!(
        observation["source_freshness"], "CURRENT",
        "{phase}: {observation}"
    );
    assert_eq!(
        observation["semantic_pending"], true,
        "{phase}: {observation}"
    );
    assert_eq!(
        observation["runnable_pending"], true,
        "{phase}: {observation}"
    );
    let bytes = STANDARD
        .decode(modern_step(&report, "page")[0]["blob"].as_str().unwrap())
        .unwrap();
    let rows = arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    (result, rows)
}

#[test]
fn source_current_publication_fences_delayed_semantics_and_resumes_after_restart() {
    use arrow::array::StringArray;
    let fixture = ProductionFixture::with_source_and_activation_startup_fault(
        b"def original():\n    return 1\n",
        Some("hold_semantic_update_publication"),
    );
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
    fs::write(
        workspace.join("src/lib.rs"),
        b"pub fn rust_source() -> u32 { 7 }\n",
    )
    .unwrap();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let request = semantic_request(
        &fixture.workspace.public_id(),
        "unused",
        "Python function declarations",
    );
    let initial = public_query(&fixture, &stack, "staged-initial", request.clone());
    assert_eq!(initial.rows[0]["name"], "original");
    fs::write(
        workspace.join("sample.py"),
        b"def obsolete():\n    return 2\ndef caller():\n    return obsolete()\n",
    )
    .unwrap();
    let obsolete = pending_semantic_candidate(&fixture);
    let (source, rows) = source_current_query(&fixture, &stack, "pending-source", request.clone());
    let strings = |rows: &[RecordBatch], name: &str| {
        rows.iter()
            .flat_map(|batch| {
                batch
                    .column_by_name(name)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .iter()
                    .map(|value| value.unwrap().to_owned())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        strings(&rows, "name").into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from(["obsolete".to_owned(), "caller".to_owned()])
    );
    assert_eq!(source["processing"][0]["remaining_partitions"], 0);
    let mut calls = request.clone();
    calls["scope"]["languages"] = json!(["python"]);
    calls["queries"] = json!([{
        "request": "follow code relationships", "query_id": "calls", "starting_from": strings(&rows, "public_entity_id").iter().map(|id| json!({"entity_id": id})).collect::<Vec<_>>(),
        "relationship": "calls", "direction": "outgoing", "distance": "one relationship step", "return": {"limit": {"maximum_results": 32}}
    }]);
    let (pending, _) = source_current_query(&fixture, &stack, "pending-calls", calls);
    assert_eq!(pending["source_generation"], source["source_generation"]);
    assert_eq!(
        pending["processing"][0]["remaining_partitions"], 1,
        "{pending}"
    );
    assert_eq!(pending["processing"][0]["remainder"][0]["state"], "pending");
    assert_eq!(
        pending["processing"][0]["remainder"][0]["reason_code"],
        "semantic_work_pending"
    );
    let rust = semantic_request(
        &fixture.workspace.public_id(),
        "unused",
        "Rust function declarations",
    );
    let (rust_pending, rows) = source_current_query(&fixture, &stack, "pending-rust-target", rust);
    assert!(rows.iter().all(|batch| batch.num_rows() == 0));
    let rust_scope = &rust_pending["processing"][0];
    assert_eq!(rust_scope["requested_partitions"], 1);
    assert_eq!(rust_scope["remaining_partitions"], 1);
    assert_eq!(rust_scope["remainder"][0]["scope_kind"], "cargo_target");
    assert_eq!(rust_scope["remainder"][0]["target"], "fixture");
    assert_eq!(rust_scope["remainder"][0]["state"], "pending");
    let mut strict = request.clone();
    strict["semantic_request_id"] = json!("request:staged-deadline");
    strict["freshness"] = json!({"policy": "require_semantic_current", "deadline_ms": 250});
    let scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "deadline", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": strict}, "expect_error": "CLIENT_OPERATION_FAILED"}
        ]),
    );
    let path = write_modern_client_scenario(&fixture, "semantic-deadline", &scenario);
    let report = modern_client_report(&run_modern_client(&stack, &path));
    assert!(
        modern_step(&report, "deadline")["public_error"]
            .as_str()
            .unwrap()
            .contains("FRESHNESS_DEADLINE"),
        "{report}"
    );

    // Complete an older provider, hold its publication, then replace the source. It must
    // be discarded even though it produced a valid output before the newer edit.
    fs::write(
        workspace.join("sample.py"),
        b"def repaired():\n    return 3\n",
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while obsolete.exists() {
        assert!(
            Instant::now() < deadline,
            "obsolete provider publication was not cancelled"
        );
        thread::sleep(Duration::from_millis(20));
    }
    let repaired = pending_semantic_candidate(&fixture);
    let (next_source, rows) =
        source_current_query(&fixture, &stack, "repaired-source", request.clone());
    assert_eq!(strings(&rows, "name"), ["repaired"]);
    assert!(
        next_source["source_generation"].as_u64().unwrap()
            > source["source_generation"].as_u64().unwrap()
    );
    fs::write(repaired.with_extension("resume"), b"resume").unwrap();
    let final_result = public_query(&fixture, &stack, "repaired-semantic", request.clone());
    assert_eq!(final_result.rows[0]["name"], "repaired");

    // Stop with the source stage durably selected and no terminal semantic successor.
    fs::write(
        workspace.join("sample.py"),
        b"def after_restart():\n    return 4\n",
    )
    .unwrap();
    let before_restart = pending_semantic_candidate(&fixture);
    let (before, _) = source_current_query(&fixture, &stack, "before-restart", request.clone());
    supervisor.stop();
    assert!(
        !before_restart.exists(),
        "shutdown joins the paused publication owner"
    );
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let resumed = pending_semantic_candidate(&fixture);
    assert_eq!(
        resumed, before_restart,
        "reopen resumes the same source generation"
    );
    let (after, rows) = source_current_query(&fixture, &stack, "after-restart", request.clone());
    assert!(before["epoch_id"].as_str().is_some());
    assert_eq!(before["epoch_id"], after["epoch_id"]);
    assert_eq!(before["source_generation"], after["source_generation"]);
    assert_eq!(strings(&rows, "name"), ["after_restart"]);
    fs::write(resumed.with_extension("resume"), b"resume").unwrap();
    let final_result = public_query(&fixture, &stack, "resumed-semantic", request);
    assert_eq!(final_result.rows[0]["name"], "after_restart");
    supervisor.stop();
}
