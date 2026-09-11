//! Real public queries against an incremental daemon and an independent clean state root.
use super::*;

mod provider_deployment;

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
    // This case tests retained pagination across a completed repair. Full semantic publication
    // is a setup prerequisite, independently of any individual public query's freshness deadline.
    let initial = wait_for_semantic_activation_with_timeout(&fixture, Duration::from_secs(600));
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
    wait_for_semantic_activation_after_generation(
        &fixture,
        Some(initial.row().pins.source_generation.get()),
        Duration::from_secs(600),
    );
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
            "syntax_provider_run_id",
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
    let mut supervisor = fixture.start_supervisor_with(&stack.codefabric);
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
        supervisor = assert_compiler_diagnostics_and_reopen(
            &fixture,
            &clean,
            &stack,
            phase,
            names,
            &incremental,
            supervisor,
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

fn rust_diagnostic_details(fixture: &ProductionFixture) -> BTreeMap<String, Vec<Value>> {
    ["child", "span", "suggestion", "edit"]
        .into_iter()
        .map(|kind| {
            let relation = format!("provider.rustc.diagnostic_{kind}.v1");
            let batches = fresh_activation_relation_batches(fixture, &relation);
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
                // These identify separate executions. Keep file identities, digests, complete message/
                // alternative/part relationships and native coordinates in the semantic comparison.
                for field in [
                    "provider_run_id",
                    "source_generation",
                    "compilation_unit_id",
                    "owner_id",
                ] {
                    fields.remove(field);
                }
            }
            rows.sort_by_cached_key(Value::to_string);
            (relation, rows)
        })
        .collect()
}

fn assert_compiler_diagnostics_and_reopen(
    fixture: &ProductionFixture,
    clean: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    names: &[&str],
    incremental: &[SemanticObservation],
    mut supervisor: RunningSupervisor,
) -> RunningSupervisor {
    let diagnostics = rust_diagnostic_messages(fixture);
    let details = rust_diagnostic_details(fixture);
    assert_eq!(
        details,
        rust_diagnostic_details(clean),
        "{phase}: native diagnostic details match independent clean publication"
    );
    assert_eq!(
        diagnostics,
        rust_diagnostic_messages(clean),
        "{phase}: exact live/clean compiler diagnostics"
    );
    if phase == "broken" {
        assert!(
            diagnostics
                .iter()
                .any(|(code, level, message)| code == "E0425"
                    && level == "error"
                    && message.contains("missing"))
        );
        let locations = &details["provider.rustc.diagnostic_span.v1"];
        assert!(
            locations
                .iter()
                .any(|row| row["location_state"] == "captured-source" && row["is_primary"] == true)
        );
        let activation = all_activation_control_rows(fixture);
        supervisor.stop();
        supervisor = fixture.start_supervisor_with(&stack.codefabric);
        assert_eq!(
            four_forms(fixture, stack, "broken-reopened", names),
            incremental
        );
        assert_eq!(rust_diagnostic_messages(fixture), diagnostics);
        assert_eq!(rust_diagnostic_details(fixture), details);
        assert_eq!(
            all_activation_control_rows(fixture),
            activation,
            "reopening a terminal failed compilation must reuse the exact persisted epoch"
        );
    } else {
        assert!(
            diagnostics.is_empty(),
            "{phase}: previous compiler failure must not remain current"
        );
    }
    supervisor
}

/// Independent expected calls accompany clean/live equality, including semantic unknowns.
fn python_context_calls(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    selected: Option<&str>,
    dependency_present: bool,
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
    let mut names = BTreeSet::from(["caller", "legacy", "current"]);
    if dependency_present {
        names.insert("imported");
    }
    assert_eq!(
        by_id.values().copied().collect::<BTreeSet<_>>(),
        names,
        "{phase}"
    );
    assert_eq!(entities.processing[0]["remaining_partitions"], 0, "{phase}");
    let caller = by_id.iter().find(|(_, name)| **name == "caller").unwrap().0;
    request["queries"] = json!([{
        "request": "follow code relationships", "query_id": "calls",
        "starting_from": [{"entity_id": caller}], "relationship": "calls", "direction": "outgoing",
        "distance": "one relationship step", "return": {"limit": {"maximum_results": 32}}
    }]);
    let calls = public_query(fixture, stack, &format!("{phase}-calls"), request);
    assert_eq!(
        calls.rows.len(),
        2,
        "{phase}: both call occurrences survive unresolved imports"
    );
    let targets = calls
        .rows
        .iter()
        .map(|row| {
            assert_eq!(row["public_source_entity_id"].as_str(), Some(*caller));
            row["public_target_entity_id"].as_str().map(|id| by_id[id])
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        targets,
        BTreeSet::from([
            selected,
            (dependency_present && selected.is_some()).then_some("imported")
        ]),
        "{phase}"
    );
    assert_eq!(
        calls.processing[0]["remaining_partitions"],
        if selected.is_none() {
            1 + u64::from(dependency_present)
        } else {
            u64::from(!dependency_present)
        },
        "{phase}: unknown import or unsupported context scope"
    );
    vec![entities, calls]
}

fn pyrefly_cache_observation(fixture: &ProductionFixture) -> Value {
    let costs: Value = serde_json::from_slice(
        &fs::read(
            fixture
                .fabric_workspace_root()
                .join("semantic-preparation-costs.json"),
        )
        .unwrap(),
    )
    .unwrap();
    costs["workspace_pyrefly_cache"].clone()
}

fn assert_pyrefly_cache_transition(
    phase: &str,
    cache: &Value,
    prior_cache: &Value,
    selected: Option<&str>,
) {
    if matches!(phase, "warm-source-edited" | "warm-source-restored") {
        assert_eq!(
            cache["process_starts"], prior_cache["process_starts"],
            "{phase}: same contained process"
        );
        assert!(
            cache["reused_runs"].as_u64().unwrap() > prior_cache["reused_runs"].as_u64().unwrap(),
            "{phase}: native checker reuse: {cache}"
        );
        assert_eq!(cache["retained_processes"], 1);
    } else if selected.is_some() {
        assert!(
            cache["process_starts"].as_u64().unwrap()
                > prior_cache["process_starts"].as_u64().unwrap(),
            "{phase}: configuration retires incompatible process: {cache}"
        );
        assert_eq!(cache["retained_processes"], 1);
    } else {
        assert_eq!(
            cache["retained_processes"], 0,
            "incomplete run retires native state"
        );
    }
}

#[test]
fn live_python_context_and_negative_imports_equal_independent_clean_queries() {
    let fixture = ProductionFixture::with_source(b"import sys\nfrom dependency import imported\ndef legacy():\n    return 1\ndef current():\n    return 2\nif sys.version_info >= (3, 14) and sys.platform == 'linux':\n    selected = current\nelse:\n    selected = legacy\ndef caller():\n    return selected() + imported()\n");
    let root = Path::new(&fixture.workspace.root_path_display);
    let original_source = fs::read_to_string(root.join("sample.py")).unwrap();
    fs::write(
        root.join("pyrefly.toml"),
        "python-version = '3.14'\npython-platform = 'linux'\n",
    )
    .unwrap();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let registration = fixture.root().join("registration.sqlite3");
    OperationalStore::open(&fixture.state.join("operational.sqlite3"))
        .unwrap()
        .backup_to(&registration)
        .unwrap();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let initial = python_context_calls(&fixture, &stack, "missing-import", Some("current"), false);
    let mut prior_cache = pyrefly_cache_observation(&fixture);
    assert_eq!(prior_cache["process_starts"], 1);
    assert_eq!(prior_cache["retained_processes"], 1);
    for (phase, version, platform, present, selected) in [
        ("warm-source-edited", "3.14", "linux", false, Some("legacy")),
        (
            "warm-source-restored",
            "3.14",
            "linux",
            false,
            Some("current"),
        ),
        ("import-created", "3.14", "linux", true, Some("current")),
        ("older-python", "3.12", "linux", true, Some("legacy")),
        ("other-platform", "3.14", "win32", true, Some("legacy")),
        ("unsupported-setting", "3.14", "linux", true, None),
        ("import-deleted", "3.14", "linux", false, Some("current")),
    ] {
        let source = if phase == "warm-source-edited" {
            original_source.replace("selected = current", "selected = legacy")
        } else {
            original_source.clone()
        };
        fs::write(root.join("sample.py"), source).unwrap();
        fs::write(
            root.join("pyrefly.toml"),
            format!(
                "python-version = '{version}'\npython-platform = '{platform}'\n{}",
                if selected.is_none() {
                    "unhandled-setting = true\n"
                } else {
                    ""
                }
            ),
        )
        .unwrap();
        if present {
            fs::write(
                root.join("dependency.py"),
                b"def imported():\n    return 4\n",
            )
            .unwrap();
        } else if root.join("dependency.py").exists() {
            fs::remove_file(root.join("dependency.py")).unwrap();
        }
        let live = python_context_calls(&fixture, &stack, phase, selected, present);
        let cache = pyrefly_cache_observation(&fixture);
        assert_pyrefly_cache_transition(phase, &cache, &prior_cache, selected);
        prior_cache = cache;
        let clean = clean_fixture(&fixture, &registration, &stack);
        let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
        let expected =
            python_context_calls(&clean, &stack, &format!("clean-{phase}"), selected, present);
        assert_eq!(
            live, expected,
            "{phase}: context/dependency replacement must equal clean semantics"
        );
        if !present && selected == Some("current") {
            assert_eq!(
                live, initial,
                "restoring the original inputs restores their exact semantics"
            );
        }
        clean_supervisor.stop();
    }
    supervisor.stop();
}

fn python_stub_observation(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    has_stub: bool,
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
    let source_root = codefabric::secure_path::SecureRoot::authorize(
        codefabric::secure_path::RootAuthorizationRecord::try_from(&fixture.workspace).unwrap(),
    )
    .unwrap();
    let file_id = |path: &str| {
        use std::fmt::Write as _;
        let path = codefabric::secure_path::PlatformPath::from_raw_relative_bytes(
            codefabric::identity::PlatformCode::Unix,
            path.as_bytes().to_vec(),
        )
        .unwrap();
        codefabric::identity::source_file_identity(&source_root.workspace_path(&path).unwrap())
            .unwrap()
            .id
            .iter()
            .fold(String::with_capacity(32), |mut text, byte| {
                write!(text, "{byte:02x}").unwrap();
                text
            })
    };
    let by_file = entities
        .rows
        .iter()
        .map(|row| (row["file_id"].as_str().unwrap(), row))
        .collect::<BTreeMap<_, _>>();
    let mut expected = vec![("sample.py", "caller"), ("ns/helper.py", "selected")];
    if has_stub {
        expected.push(("ns/helper.pyi", "selected"));
    }
    assert_eq!(
        entities.rows.len(),
        expected.len(),
        "{phase}: same-name source and stub declarations remain distinct"
    );
    for (path, name) in expected {
        assert_eq!(by_file[file_id(path).as_str()]["name"], name);
    }
    let caller = &by_file[file_id("sample.py").as_str()]["public_entity_id"];
    let target = &by_file[file_id(if has_stub {
        "ns/helper.pyi"
    } else {
        "ns/helper.py"
    })
    .as_str()]["public_entity_id"];
    if has_stub {
        assert_ne!(
            target,
            &by_file[file_id("ns/helper.py").as_str()]["public_entity_id"]
        );
    }
    assert_eq!(entities.processing[0]["remaining_partitions"], 0);
    request["queries"] = json!([{
        "request": "follow code relationships", "query_id": "calls",
        "starting_from": [{"entity_id": caller}], "relationship": "calls", "direction": "outgoing",
        "distance": "one relationship step", "return": {"limit": {"maximum_results": 32}}
    }]);
    let calls = public_query(fixture, stack, &format!("{phase}-calls"), request);
    assert_eq!(calls.rows.len(), 1, "{phase}: one captured call occurrence");
    assert_eq!(&calls.rows[0]["public_source_entity_id"], caller);
    assert_eq!(&calls.rows[0]["public_target_entity_id"], target);
    assert_eq!(
        calls.processing[0]["remaining_partitions"], 0,
        "{phase}: complete local import targets"
    );
    vec![entities, calls]
}

#[test]
fn live_python_namespace_stub_precedence_equals_independent_clean_queries() {
    const STUB: &[u8] = b"def selected() -> str: ...\n";
    let fixture = ProductionFixture::with_source(
        b"from ns.helper import selected\ndef caller():\n    return selected()\n",
    );
    let root = Path::new(&fixture.workspace.root_path_display);
    // No __init__.py: the effective roots must preserve PEP 420 namespace resolution.
    fs::create_dir(root.join("ns")).unwrap();
    fs::write(
        root.join("ns/helper.py"),
        b"def selected() -> int:\n    return 1\n",
    )
    .unwrap();
    fs::write(root.join("ns/helper.pyi"), STUB).unwrap();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let registration = fixture.root().join("registration.sqlite3");
    OperationalStore::open(&fixture.state.join("operational.sqlite3"))
        .unwrap()
        .backup_to(&registration)
        .unwrap();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let initial = python_stub_observation(&fixture, &stack, "stub-initial", true);
    for (phase, has_stub) in [("stub-removed", false), ("stub-restored", true)] {
        if has_stub {
            fs::write(root.join("ns/helper.pyi"), STUB).unwrap();
        } else {
            fs::remove_file(root.join("ns/helper.pyi")).unwrap();
        }
        let live = python_stub_observation(&fixture, &stack, phase, has_stub);
        let clean = clean_fixture(&fixture, &registration, &stack);
        let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
        let expected = python_stub_observation(&clean, &stack, &format!("clean-{phase}"), has_stub);
        assert_eq!(
            live, expected,
            "{phase}: complete local namespace/stub semantics"
        );
        if has_stub {
            assert_eq!(
                live, initial,
                "recreated stub restores canonical source and semantic identities"
            );
        }
        clean_supervisor.stop();
    }
    supervisor.stop();
}

fn ordered_python_import_observation(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    target: &str,
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
    let by_name = entities
        .rows
        .iter()
        .map(|row| {
            (
                row["name"].as_str().unwrap(),
                row["public_entity_id"].as_str().unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        by_name.keys().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from(["caller", "first_choice", "second_choice", "outside_choice"]),
        "{phase}: import roots must not remove source inventory"
    );
    assert_eq!(entities.rows.len(), 4);
    assert_eq!(entities.processing[0]["remaining_partitions"], 0);
    request["queries"] = json!([{
        "request": "follow code relationships", "query_id": "calls",
        "starting_from": [{"entity_id": by_name["caller"]}], "relationship": "calls", "direction": "outgoing",
        "distance": "one relationship step", "return": {"limit": {"maximum_results": 32}}
    }]);
    let calls = public_query(fixture, stack, &format!("{phase}-calls"), request);
    assert_eq!(calls.rows.len(), 1);
    assert_eq!(
        calls.rows[0]["public_target_entity_id"], by_name[target],
        "{phase}: only the configured root order selects import resolution"
    );
    assert_ne!(
        calls.rows[0]["public_target_entity_id"],
        by_name["outside_choice"]
    );
    assert_eq!(calls.processing[0]["remaining_partitions"], 0);
    vec![entities, calls]
}

#[test]
fn captured_python_site_packages_survive_public_queries_and_reopen() {
    let fixture = ProductionFixture::with_source(
        b"from external import py_leaf\ndef py_caller() -> int:\n    return py_leaf()\n",
    );
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir_all(root.join(".venv/lib/python3.14/site-packages/external")).unwrap();
    fs::write(
        root.join(".venv/lib/python3.14/site-packages/external/__init__.py"),
        b"def py_leaf() -> int:\n    return 42\n",
    )
    .unwrap();
    fs::write(
        root.join(".venv/lib/python3.14/site-packages/external/py.typed"),
        b"",
    )
    .unwrap();
    fs::write(
        root.join("pyrefly.toml"),
        "site-package-path=['.venv/lib/python3.14/site-packages']\n",
    )
    .unwrap();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    WorkspaceRegistry::new(
        &mut OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap(),
    )
    .set_source_disclosure(fixture.workspace.workspace_id, true)
    .unwrap();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let expected = ["py_caller", "py_leaf"];
    let observed = four_forms(&fixture, &stack, "site-packages-initial", &expected);
    let raw = fresh_activation_relation_batches(&fixture, "provider.pyrefly.call_target.v1");
    assert!(raw.iter().any(|batch| batch.num_rows() > 0));
    let costs: Value = serde_json::from_slice(
        &fs::read(
            fixture
                .fabric_workspace_root()
                .join("semantic-preparation-costs.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(costs["finished"], true);
    assert_eq!(costs["source_files"], 4);
    assert!(
        costs["phases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|phase| phase["phase"] == "pyrefly"
                && phase["elapsed_micros"].as_u64().unwrap() > 0)
    );
    eprintln!("Python external-root phase costs: {costs}");
    let selected = wait_for_semantic_activation(&fixture);
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(&fixture).table_versions()
    );
    assert_eq!(
        observed,
        four_forms(&fixture, &stack, "site-packages-reopen", &expected)
    );
    supervisor.stop();
}

#[test]
fn live_python_search_paths_preserve_all_sources_and_equal_independent_clean_queries() {
    let fixture = ProductionFixture::with_source(
        b"from helper import selected\ndef caller():\n    return selected()\n",
    );
    let root = Path::new(&fixture.workspace.root_path_display);
    for (directory, choice) in [("first", "first_choice"), ("second", "second_choice")] {
        fs::create_dir(root.join(directory)).unwrap();
        fs::write(
            root.join(directory).join("helper.py"),
            format!("def {choice}() -> int:\n    return 1\nselected = {choice}\n"),
        )
        .unwrap();
    }
    // Captured source outside the configured import roots must not shadow their selected module.
    fs::write(
        root.join("helper.py"),
        b"def outside_choice() -> int:\n    return 9\nselected = outside_choice\n",
    )
    .unwrap();
    let config = |first, second| format!("search-path = ['{first}', '{second}']\n");
    fs::write(root.join("pyrefly.toml"), config("first", "second")).unwrap();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let registration = fixture.root().join("registration.sqlite3");
    OperationalStore::open(&fixture.state.join("operational.sqlite3"))
        .unwrap()
        .backup_to(&registration)
        .unwrap();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let initial =
        ordered_python_import_observation(&fixture, &stack, "roots-first", "first_choice");
    for (phase, first, second, target) in [
        ("roots-second", "second", "first", "second_choice"),
        ("roots-restored", "first", "second", "first_choice"),
    ] {
        fs::write(root.join("pyrefly.toml"), config(first, second)).unwrap();
        let live = ordered_python_import_observation(&fixture, &stack, phase, target);
        let clean = clean_fixture(&fixture, &registration, &stack);
        let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
        let expected =
            ordered_python_import_observation(&clean, &stack, &format!("clean-{phase}"), target);
        assert_eq!(live, expected, "{phase}: exact live/clean context equality");
        if first == "first" {
            assert_eq!(live, initial, "restored roots restore original identities");
        }
        clean_supervisor.stop();
    }
    supervisor.stop();
}

fn python_path_observation(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    paths: &[&[u8]],
) -> Vec<SemanticObservation> {
    use std::fmt::Write as _;
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
    assert_eq!(
        entities.rows.len(),
        paths.len() * 2,
        "{phase}: every captured function"
    );
    assert_eq!(entities.processing[0]["remaining_partitions"], 0);
    let source_root = codefabric::secure_path::SecureRoot::authorize(
        codefabric::secure_path::RootAuthorizationRecord::try_from(&fixture.workspace).unwrap(),
    )
    .unwrap();
    let mut expected_calls = BTreeSet::new();
    for path in paths {
        let selected = codefabric::secure_path::PlatformPath::from_raw_relative_bytes(
            codefabric::identity::PlatformCode::Unix,
            path.to_vec(),
        )
        .unwrap();
        let file_id = codefabric::identity::source_file_identity(
            &source_root.workspace_path(&selected).unwrap(),
        )
        .unwrap()
        .id
        .iter()
        .fold(String::with_capacity(32), |mut text, byte| {
            write!(text, "{byte:02x}").unwrap();
            text
        });
        let functions = entities
            .rows
            .iter()
            .filter(|row| row["file_id"] == file_id)
            .map(|row| {
                (
                    row["name"].as_str().unwrap(),
                    row["public_entity_id"].as_str().unwrap(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            functions.keys().copied().collect::<BTreeSet<_>>(),
            BTreeSet::from(["caller", "leaf"])
        );
        expected_calls.insert((functions["caller"], functions["leaf"]));
    }
    let subjects = entities
        .rows
        .iter()
        .map(|row| json!({"entity_id": row["public_entity_id"]}))
        .collect::<Vec<_>>();
    request["queries"] = json!([{
        "request": "follow code relationships", "query_id": "calls", "starting_from": subjects,
        "relationship": "calls", "direction": "outgoing", "distance": "one relationship step",
        "return": {"limit": {"maximum_results": 32}}
    }]);
    let calls = public_query(fixture, stack, &format!("{phase}-calls"), request.clone());
    let actual_calls = calls
        .rows
        .iter()
        .map(|row| {
            (
                row["public_source_entity_id"].as_str().unwrap(),
                row["public_target_entity_id"].as_str().unwrap(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual_calls, expected_calls,
        "{phase}: same-name functions resolve in their exact source owner"
    );
    assert_eq!(calls.rows.len(), paths.len());
    assert_eq!(calls.processing[0]["remaining_partitions"], 0);
    request["queries"] = json!([{
        "request": "retrieve source and syntax context", "query_id": "source", "about": subjects,
        "context": "exact source span", "return": {"maximum_source_bytes": 1024, "limit": {"maximum_results": 32}}
    }]);
    let source = public_query(fixture, stack, &format!("{phase}-source"), request);
    assert_eq!(source.rows.len(), paths.len() * 2);
    for row in &source.rows {
        assert_eq!(row["source_context"]["text"], row["name"]);
        assert_eq!(row["source_context"]["complete"], true);
    }
    vec![entities, calls, source]
}

#[test]
fn live_python_raw_paths_and_root_initializer_keep_exact_source_identity() {
    use std::os::unix::ffi::OsStrExt as _;
    const SOURCE: &[u8] = b"def leaf():\n    return 1\ndef caller():\n    return leaf()\n";
    let paths: [&[u8]; 4] = [
        b"sample.py",
        b"dir-\xff/module%?# \\.py",
        "dir-�/module%?# \\.py".as_bytes(),
        b"__init__.py",
    ];
    let fixture = ProductionFixture::with_source(SOURCE);
    let root = Path::new(&fixture.workspace.root_path_display);
    for path in &paths[1..] {
        let path = root.join(std::ffi::OsStr::from_bytes(path));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, SOURCE).unwrap();
    }
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let registration = fixture.root().join("registration.sqlite3");
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
        store.backup_to(&registration).unwrap();
    }
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let initial = python_path_observation(&fixture, &stack, "paths-initial", &paths);
    for (phase, present) in [("paths-removed", false), ("paths-restored", true)] {
        let path = root.join(std::ffi::OsStr::from_bytes(paths[1]));
        if present {
            fs::write(path, SOURCE).unwrap();
        } else {
            fs::remove_file(path).unwrap();
        }
        let selected = paths
            .iter()
            .enumerate()
            .filter_map(|(index, path)| (present || index != 1).then_some(*path))
            .collect::<Vec<_>>();
        let live = python_path_observation(&fixture, &stack, phase, &selected);
        let clean = clean_fixture(&fixture, &registration, &stack);
        let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
        let expected =
            python_path_observation(&clean, &stack, &format!("clean-{phase}"), &selected);
        assert_eq!(
            live, expected,
            "{phase}: exact raw-path clean/live equality"
        );
        if present {
            assert_eq!(
                live, initial,
                "restoring raw path restores canonical identities"
            );
        }
        clean_supervisor.stop();
    }
    supervisor.stop();
}

fn function_source_expectations(edited: bool) -> (String, String, Vec<(String, String, String)>) {
    let inner = if edited {
        "return \"newé\""
    } else {
        "return \"é\""
    };
    let python_body = format!("def inner():\r\n        {inner}\r\n    return inner()");
    let python_definition = format!("def outer(value: str = \"é\"):\r\n    {python_body}");
    let python_other = "def other(): return 3";
    let rust_body = format!(
        "{{\n    // A comment with a closing brace }}\n    let text = \"{{ literal }}\";\n    if text.is_empty() {{ 0 }} else {{ {} }}\n}}",
        if edited { 6 } else { 4 }
    );
    let rust_definition = format!("pub fn rust_outer() -> u32 {rust_body}");
    let rust_other = "pub fn rust_other() -> u32 { 5 }";
    (
        format!("# source prefix\r\n{python_definition}\r\n\r\n{python_other}\r\n"),
        format!("{rust_definition}\n{rust_other}\n"),
        vec![
            ("outer".to_owned(), python_definition, python_body),
            (
                "inner".to_owned(),
                format!("def inner():\r\n        {inner}"),
                inner.to_owned(),
            ),
            (
                "other".to_owned(),
                python_other.to_owned(),
                "return 3".to_owned(),
            ),
            ("fixture::rust_outer".to_owned(), rust_definition, rust_body),
            (
                "fixture::rust_other".to_owned(),
                rust_other.to_owned(),
                "{ 5 }".to_owned(),
            ),
        ],
    )
}

fn function_source_observation(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    expected: &[(String, String, String)],
) -> Vec<SemanticObservation> {
    wait_for_function_sources(fixture);
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
    assert_eq!(entities.rows.len(), expected.len());
    let by_name = entities
        .rows
        .iter()
        .map(|row| {
            (
                row["name"].as_str().unwrap(),
                row["public_entity_id"].clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        by_name.keys().copied().collect::<BTreeSet<_>>(),
        expected.iter().map(|(name, _, _)| name.as_str()).collect()
    );
    let subjects = by_name
        .values()
        .map(|id| json!({"entity_id": id}))
        .collect::<Vec<_>>();
    let mut observed = Vec::new();
    for (index, kind) in ["function definition", "function body"]
        .into_iter()
        .enumerate()
    {
        request["queries"] = json!([{
            "request": "retrieve source and syntax context", "query_id": "source", "about": subjects,
            "context": kind, "return": {"maximum_source_bytes": 4096, "limit": {"maximum_results": 32}}
        }]);
        let result = public_query(
            fixture,
            stack,
            &format!("{phase}-source-{index}"),
            request.clone(),
        );
        assert_eq!(
            result.rows.len(),
            expected.len(),
            "{phase}: exact syntax owner for each function"
        );
        assert_eq!(
            result.processing[0]["remaining_partitions"], 0,
            "{phase}: function source scope"
        );
        for (name, definition, body) in expected {
            let row = result.rows.iter().find(|row| row["name"] == *name).unwrap();
            let text = if index == 0 { definition } else { body };
            assert_eq!(row["public_entity_id"], by_name[name.as_str()]);
            assert_eq!(row["context_kind"], kind);
            assert_eq!(
                row["source_context"]["text"], *text,
                "{phase}: {name}: {kind}"
            );
            assert_eq!(row["source_context"]["returned_bytes"], text.len());
            assert_eq!(row["source_context"]["omitted_bytes"], 0);
            assert_eq!(row["source_context"]["complete"], true);
            assert_eq!(row["source_context"]["start_byte"], row["start_byte"]);
            assert_eq!(row["source_context"]["end_byte"], row["end_byte"]);
            assert_eq!(
                row["source_mapping"],
                if name.starts_with("fixture::") {
                    "exact-function-header-start"
                } else {
                    "exact-function-name-child"
                }
            );
        }
        observed.push(result);
    }
    if phase == "function-initial" {
        request["queries"][0]["about"] = json!([{"entity_id": by_name["inner"]}]);
        request["queries"][0]["return"]["maximum_source_bytes"] = json!(9);
        let limited = public_query(fixture, stack, "function-body-split-unicode", request);
        let source = &limited.rows[0]["source_context"];
        assert!(source["text"].is_null());
        assert_eq!(source["bytes"], "72657475726e2022c3");
        assert_eq!(source["returned_bytes"], 9);
        assert!(source["end_utf8_column"].is_null());
        assert!(source["end_utf16_column"].is_null());
        assert_eq!(source["omitted_bytes"], 2);
        assert_eq!(source["complete"], false);
    }
    observed.push(function_outline_observation(
        fixture, stack, phase, expected, &subjects,
    ));
    observed.insert(0, entities);
    observed
}

fn wait_for_function_sources(fixture: &ProductionFixture) {
    wait_for_selected_sources(fixture, &["sample.py", "src/lib.rs"]);
}

fn wait_for_selected_sources(fixture: &ProductionFixture, paths: &[&str]) {
    // Expanded semantic families currently take longer to publish than one adapter call's
    // timeout. Wait for the exact edited inputs, not an older ready semantic activation.
    let root = Path::new(&fixture.workspace.root_path_display);
    let expected = paths
        .iter()
        .map(|path| (path.as_bytes().to_vec(), fs::read(root.join(path)).unwrap()))
        .collect::<Vec<_>>();
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        let selected = wait_for_semantic_activation_with_timeout(
            fixture,
            deadline.saturating_duration_since(Instant::now()),
        );
        let captured = selected_relation_batches(&selected, "source.exact_source_bytes")
            .into_iter()
            .flat_map(|batch| {
                let binary = |name| {
                    batch
                        .column_by_name(name)
                        .unwrap()
                        .as_any()
                        .downcast_ref::<arrow::array::BinaryArray>()
                        .unwrap()
                };
                binary("relative_path")
                    .iter()
                    .zip(binary("source_bytes").iter())
                    .map(|(path, bytes)| (path.unwrap().to_vec(), bytes.unwrap().to_vec()))
                    .collect::<BTreeMap<_, _>>()
            })
            .collect::<BTreeMap<_, _>>();
        if expected
            .iter()
            .all(|(path, bytes)| captured.get(path) == Some(bytes))
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "exact function inputs did not converge"
        );
        thread::sleep(Duration::from_millis(200));
    }
}

fn function_outline_observation(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    expected: &[(String, String, String)],
    subjects: &[Value],
) -> SemanticObservation {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "unused",
        "function declarations",
    );
    request["queries"] = json!([{"request":"retrieve source and syntax context", "query_id":"outline",
        "about":subjects, "context":"syntax outline", "return":{"limit":{"maximum_results":1024}}}]);
    let result = public_query(fixture, stack, &format!("{phase}-outline"), request);
    assert_eq!(result.processing[0]["remaining_partitions"], 0);
    for (name, definition, _) in expected {
        let nodes = result
            .rows
            .iter()
            .filter(|row| row["name"] == *name)
            .collect::<Vec<_>>();
        assert!(!nodes.is_empty(), "missing outline for {name}");
        let rust = name.starts_with("fixture::");
        let source = fs::read(
            Path::new(&fixture.workspace.root_path_display).join(if rust {
                "src/lib.rs"
            } else {
                "sample.py"
            }),
        )
        .unwrap();
        assert!(nodes.iter().any(|node| {
            node["syntax_raw_kind"]
                == if rust {
                    "function_item"
                } else {
                    "function_definition"
                }
                && node["syntax_start_byte"] == node["start_byte"]
                && node["syntax_end_byte"] == node["end_byte"]
        }));
        for node in nodes {
            let start = usize::try_from(node["start_byte"].as_u64().unwrap()).unwrap();
            let end = usize::try_from(node["end_byte"].as_u64().unwrap()).unwrap();
            assert_eq!(&source[start..end], definition.as_bytes());
            assert!(node["syntax_start_byte"].as_u64().unwrap() >= start as u64);
            assert!(node["syntax_end_byte"].as_u64().unwrap() <= end as u64);
            assert_eq!(
                node["syntax_context_id"],
                "ffffffffffffffffffffffffffffffff"
            );
            assert_ne!(node["context_id"], node["syntax_context_id"]);
            assert!(node.get("source_bytes").is_none());
            assert!(node.get("source_context").is_none());
        }
    }
    result
}

#[test]
fn live_mixed_function_definitions_and_bodies_equal_exact_clean_source() {
    let (python, rust, expected) = function_source_expectations(false);
    let fixture = ProductionFixture::with_source(python.as_bytes());
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), rust).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\ntest = false\ndoctest = false\n").unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let registration = fixture.root().join("registration.sqlite3");
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
        store.backup_to(&registration).unwrap();
    }
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let initial = function_source_observation(&fixture, &stack, "function-initial", &expected);
    for (phase, edited) in [("function-edited", true), ("function-restored", false)] {
        let (python, rust, expected) = function_source_expectations(edited);
        fs::write(root.join("sample.py"), python).unwrap();
        fs::write(root.join("src/lib.rs"), rust).unwrap();
        let live = function_source_observation(&fixture, &stack, phase, &expected);
        let clean = clean_fixture(&fixture, &registration, &stack);
        let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
        let expected =
            function_source_observation(&clean, &stack, &format!("clean-{phase}"), &expected);
        assert_eq!(
            live, expected,
            "{phase}: pinned function syntax agrees with independent clean state"
        );
        if !edited {
            assert_eq!(
                live, initial,
                "restoring source restores definitions and identities"
            );
        }
        clean_supervisor.stop();
    }
    supervisor.stop();
}

fn source_line_request(fixture: &ProductionFixture, subject: &Value, options: Value) -> Value {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "unused",
        "function declarations",
    );
    request["queries"] = json!([{
        "request": "retrieve source and syntax context", "query_id": "source",
        "about": [{"entity_id": subject}], "context": "surrounding lines"
    }]);
    request["queries"][0]["return"] = options;
    request
}

fn source_line_observation(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    original: &str,
) -> Vec<SemanticObservation> {
    let entities = public_query(
        fixture,
        stack,
        &format!("{phase}-find"),
        semantic_request(
            &fixture.workspace.public_id(),
            "unused",
            "Python function declarations",
        ),
    );
    let subject = |name: &str| {
        entities
            .rows
            .iter()
            .find(|row| row["name"] == name)
            .unwrap()["public_entity_id"]
            .clone()
    };
    let target = subject("target");
    let expected = "# 😀é\r\ndef target(value):\r\n    return value\r\n";
    let mut observations = Vec::new();
    for (name, id, options, text, start) in [
        (
            "window",
            target.clone(),
            json!({"source_lines_before": 1, "source_lines_after": 1}),
            expected,
            7,
        ),
        (
            "anchor",
            target.clone(),
            json!({"source_lines_before": 0}),
            "def target(value):\r\n",
            17,
        ),
        (
            "edges",
            subject("last"),
            json!({"source_lines_before": 4096, "source_lines_after": 4096}),
            original,
            0,
        ),
    ] {
        let result = public_query(
            fixture,
            stack,
            &format!("{phase}-{name}"),
            source_line_request(fixture, &id, options),
        );
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.processing[0]["remaining_partitions"], 0);
        let context = &result.rows[0]["source_context"];
        assert_eq!(context["text"], text);
        assert_eq!(context["start_byte"], start);
        assert_eq!(context["end_byte"], start + text.len());
        assert_eq!(context["requested_start_byte"], start);
        assert_eq!(context["requested_end_byte"], start + text.len());
        assert_eq!(context["returned_bytes"], text.len());
        assert_eq!(context["omitted_bytes"], 0);
        assert_eq!(context["complete"], true);
        if name != "edges" {
            assert_eq!(context["anchor_start_byte"], 21);
            assert_eq!(context["anchor_end_byte"], 27);
        }
        observations.push(result);
    }
    let split = public_query(
        fixture,
        stack,
        &format!("{phase}-split"),
        source_line_request(
            fixture,
            &target,
            json!({"source_lines_before": 1, "source_lines_after": 1, "maximum_source_bytes": 7}),
        ),
    );
    let context = &split.rows[0]["source_context"];
    assert!(context["text"].is_null());
    assert_eq!(context["bytes"], "2320f09f9880c3");
    assert_eq!(context["requested_start_byte"], 7);
    assert_eq!(context["requested_end_byte"], 7 + expected.len());
    assert_eq!(context["end_byte"], 14);
    assert!(context["end_utf8_column"].is_null());
    assert!(context["end_utf16_column"].is_null());
    assert_eq!(context["returned_bytes"], 7);
    assert_eq!(context["omitted_bytes"], expected.len() - 7);
    assert_eq!(context["complete"], false);
    observations.push(split);
    observations
}

fn assert_source_hard_limit(fixture: &ProductionFixture, stack: &InstalledProductionStack) {
    let entities = public_query(
        fixture,
        stack,
        "hard-limit-find",
        semantic_request(
            &fixture.workspace.public_id(),
            "unused",
            "Python function declarations",
        ),
    );
    let huge = entities
        .rows
        .iter()
        .find(|row| row["name"] == "huge")
        .unwrap();
    let mut request = source_line_request(fixture, &huge["public_entity_id"], json!({}));
    request["semantic_request_id"] = json!("request:source-hard-limit");
    request["queries"][0]["context"] = json!("function body");
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "query", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": request, "delivery": "resource"}}
        ]),
    );
    let path = write_modern_client_scenario(fixture, "source-hard-limit", &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "query"));
    assert_eq!(result["execution_state"], "FAILED", "{result}");
    assert_eq!(
        result["error"]["code"], "QUERY_HARD_LIMIT_EXCEEDED",
        "{result}"
    );
    assert_eq!(result["error"]["retryable"], false);
    assert!(result["pages"].as_array().unwrap().is_empty());
    request["queries"][0]["return"]["maximum_source_bytes"] = json!(128);
    let truncated = public_query(fixture, stack, "source-explicit-truncation", request);
    let context = &truncated.rows[0]["source_context"];
    assert_eq!(
        context["text"],
        format!("return '{}'", "x".repeat(1_100_000))[..128]
    );
    assert_eq!(context["returned_bytes"], 128);
    assert_eq!(context["omitted_bytes"], 1_100_009 - 128);
    assert_eq!(context["complete"], false);
}

#[test]
fn source_line_windows_and_hard_limits_survive_public_delivery_and_reopen() {
    let original = "# top\r\n# 😀é\r\ndef target(value):\r\n    return value\r\n# after\r\ndef last(): return 2";
    let fixture = ProductionFixture::with_source(original.as_bytes());
    fs::write(
        Path::new(&fixture.workspace.root_path_display).join("huge.py"),
        format!("def huge():\n    return '{}'\n", "x".repeat(1_100_000)),
    )
    .unwrap();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
    }
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let initial = source_line_observation(&fixture, &stack, "lines-initial", original);
    assert_source_hard_limit(&fixture, &stack);
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        initial,
        source_line_observation(&fixture, &stack, "lines-reopen", original)
    );
    supervisor.stop();
}

fn native_preparation_costs(fixture: &ProductionFixture) -> Value {
    serde_json::from_slice(
        &fs::read(
            fixture
                .fabric_workspace_root()
                .join("semantic-preparation-costs.json"),
        )
        .unwrap(),
    )
    .unwrap()
}

fn rust_toolchain_costs(fixture: &ProductionFixture) -> Value {
    native_preparation_costs(fixture)["workspace_rust_toolchain_cache"].clone()
}

fn assert_native_cpu_released_after_mixed_preparation(fixture: &ProductionFixture) {
    let costs = native_preparation_costs(fixture);
    let cpu = &costs["native_cpu"];
    assert!(cpu["capacity"].as_u64().unwrap() > 0);
    assert!(cpu["peak_allocated_slots"].as_u64().unwrap() <= cpu["capacity"].as_u64().unwrap());
    assert_eq!(
        cpu["allocated_slots"], 0,
        "completed native work releases shares while the checker remains resident"
    );
    assert!(
        cpu["admissions"].as_u64().unwrap() >= 2,
        "both native provider lanes were admitted"
    );
}

pub(super) fn assert_native_context_overlap(fixture: &ProductionFixture) {
    assert_native_cpu_released_after_mixed_preparation(fixture);
    let cpu = native_preparation_costs(fixture)["native_cpu"].clone();
    let capacity = cpu["capacity"].as_u64().unwrap();
    if capacity >= 2 {
        let width = (capacity / 2).clamp(1, 16);
        assert!(
            cpu["peak_allocated_slots"].as_u64().unwrap() >= 2 * width,
            "independent Cargo contexts actually held simultaneous bounded shares: {cpu}"
        );
    }
}

#[test]
fn mixed_raw_path_inventory_keeps_rust_calls_across_updates_and_clean_reopen() {
    use std::os::unix::ffi::OsStrExt as _;
    let fixture = ProductionFixture::with_source(
        b"def py_leaf():\n    return 1\ndef py_caller():\n    return py_leaf()\n",
    );
    let root = Path::new(&fixture.workspace.root_path_display);
    for raw in [
        b"dir-\xff/marker.py".as_slice(),
        "dir-�/marker.py".as_bytes(),
    ] {
        let path = root.join(std::ffi::OsStr::from_bytes(raw));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"marker = 1\n").unwrap();
    }
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("src/café.rs"),
        b"pub fn rust_leaf() -> u32 { 1 }\npub fn rust_caller() -> u32 { rust_leaf() }\n",
    )
    .unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\npath = \"src/café.rs\"\ntest = false\ndoctest = false\n").unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let registration = fixture.root().join("registration.sqlite3");
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
        store.backup_to(&registration).unwrap();
    }
    let expected = [
        "py_leaf",
        "py_caller",
        "fixture::rust_leaf",
        "fixture::rust_caller",
    ];
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    wait_for_selected_sources(&fixture, &["sample.py", "src/café.rs"]);
    four_forms(&fixture, &stack, "rust-raw-initial", &expected);
    assert_eq!(rust_toolchain_costs(&fixture)["captures"], 1);
    assert_native_cpu_released_after_mixed_preparation(&fixture);
    fs::write(
        root.join(std::ffi::OsStr::from_bytes(b"dir-\xff/marker.py")),
        b"marker = 2\n",
    )
    .unwrap();
    fs::write(
        root.join("src/café.rs"),
        b"pub fn rust_new() -> u32 { 2 }\npub fn rust_caller() -> u32 { rust_new() }\n",
    )
    .unwrap();
    let expected = [
        "py_leaf",
        "py_caller",
        "fixture::rust_new",
        "fixture::rust_caller",
    ];
    wait_for_selected_sources(&fixture, &["sample.py", "src/café.rs"]);
    let live = four_forms(&fixture, &stack, "rust-raw-edited", &expected);
    assert_native_cpu_released_after_mixed_preparation(&fixture);
    let reused = rust_toolchain_costs(&fixture);
    assert_eq!(
        reused["captures"], 1,
        "warm source edits retain the compatible immutable sysroot"
    );
    assert!(reused["reuses"].as_u64().unwrap() >= 1);
    assert_eq!(reused["retained_entries"], 1);
    assert!(reused["retained_bytes"].as_u64().unwrap() > 0);
    for row in &live[3].rows {
        if row["language"] == "rust" {
            assert_eq!(row["relative_path"], "7372632f636166c3a92e7273");
        }
    }
    let clean = clean_fixture(&fixture, &registration, &stack);
    let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
    wait_for_selected_sources(&clean, &["sample.py", "src/café.rs"]);
    assert_eq!(
        live,
        four_forms(&clean, &stack, "rust-raw-clean", &expected)
    );
    assert_native_cpu_released_after_mixed_preparation(&clean);
    assert_eq!(rust_toolchain_costs(&clean)["captures"], 1);
    assert_eq!(rust_toolchain_costs(&clean)["reuses"], 0);
    clean_supervisor.stop();
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        live,
        four_forms(&fixture, &stack, "rust-raw-reopened", &expected)
    );
    supervisor.stop();
}

pub(super) fn print_cargo_failure(fixture: &ProductionFixture) {
    if let Ok(outputs) = fs::read_dir(fixture.fabric_workspace_root().join("provider-output")) {
        for output in outputs.flatten() {
            for stage in ["rust-compilation-metadata", "rust-compilation-compiler"] {
                let path = output.path().join(stage).join("stderr.capture");
                if let Ok(file) = fs::File::open(&path) {
                    use std::io::Read as _;
                    let mut text = String::new();
                    file.take(16 * 1024).read_to_string(&mut text).unwrap();
                    eprintln!("{stage}: {text}");
                }
            }
        }
    }
}

fn cargo_build_script_observation(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    leaf: &str,
) -> Vec<SemanticObservation> {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "unused",
        "Rust function declarations",
    );
    request["scope"]["languages"] = json!(["rust"]);
    let entities = public_query(
        fixture,
        stack,
        &format!("{phase}-entities"),
        request.clone(),
    );
    if entities.rows.is_empty() {
        print_cargo_failure(fixture);
    }
    assert_eq!(
        entities
            .rows
            .iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["build_script_configure::main", "fixture::caller", leaf]),
        "processing: {}",
        entities.processing
    );
    assert_eq!(entities.processing[0]["remaining_partitions"], 0);
    let by_name = entities
        .rows
        .iter()
        .map(|row| (row["name"].as_str().unwrap(), &row["public_entity_id"]))
        .collect::<BTreeMap<_, _>>();
    request["queries"] = json!([{
        "request": "follow code relationships", "query_id": "calls",
        "starting_from": [{"entity_id": by_name["fixture::caller"]}],
        "relationship": "calls", "direction": "outgoing", "distance": "one relationship step",
        "return": {"limit": {"maximum_results": 32}}
    }]);
    let calls = public_query(fixture, stack, &format!("{phase}-calls"), request.clone());
    assert_eq!(calls.rows.len(), 1);
    assert_eq!(calls.rows[0]["public_target_entity_id"], *by_name[leaf]);
    assert_eq!(calls.processing[0]["requested_partitions"], 1);
    assert_eq!(calls.processing[0]["remaining_partitions"], 0);
    assert_eq!(calls.processing[0]["scope"], "selected_rust_call_owners");
    request["queries"][0]["starting_from"] = json!([{"entity_id": by_name[leaf]}]);
    let empty = public_query(
        fixture,
        stack,
        &format!("{phase}-empty-calls"),
        request.clone(),
    );
    assert!(empty.rows.is_empty());
    assert_eq!(empty.processing[0]["requested_partitions"], 1);
    assert_eq!(empty.processing[0]["remaining_partitions"], 0);
    request["queries"][0]["starting_from"] =
        json!([{"entity_id": by_name["build_script_configure::main"]}]);
    let build_calls = public_query(
        fixture,
        stack,
        &format!("{phase}-build-calls"),
        request.clone(),
    );
    assert!(!build_calls.rows.is_empty());
    assert_eq!(build_calls.processing[0]["remaining_partitions"], 1);
    assert_eq!(
        build_calls.processing[0]["remainder"][0]["entity_id"],
        *by_name["build_script_configure::main"]
    );
    request["queries"][0]["starting_from"] = json!([{"entity_id": by_name[leaf]}]);
    request["queries"][0]["direction"] = json!("incoming");
    let incoming = public_query(
        fixture,
        stack,
        &format!("{phase}-incoming-calls"),
        request.clone(),
    );
    assert_eq!(incoming.rows.len(), 1);
    assert_eq!(incoming.processing[0]["remaining_partitions"], 1);
    assert!(
        incoming.processing[0]["remainder"][0]["reason_code"]
            .as_str()
            .unwrap()
            .contains("unresolved_targets")
    );
    request["queries"] = json!([{
        "request": "retrieve source and syntax context", "query_id": "source",
        "about": [{"entity_id": by_name[leaf]}], "context": "function body"
    }]);
    let source = public_query(fixture, stack, &format!("{phase}-source"), request);
    assert_eq!(source.rows.len(), 1);
    assert_eq!(
        source.rows[0]["source_context"]["text"],
        if leaf == "fixture::selected" {
            "{ 1 }"
        } else {
            "{ 2 }"
        }
    );
    vec![entities, calls, source, empty, incoming, build_calls]
}

// Native Cargo discovery is independently checked here; it is not compiler/fact completeness.
fn cargo_build_unit_graph_observation(
    fixture: &ProductionFixture,
) -> (
    u64,
    serde_json::Value,
    Vec<codefabric::fabric::delta_exact::ExactDeltaPin>,
    Vec<u8>,
) {
    let selected = all_activation_control_rows(fixture)
        .into_iter()
        .max_by_key(|row| row.row().ordinal.get())
        .unwrap();
    let graphs = selected_relation_batches(&selected, "source.cargo_unit_graph");
    assert_eq!(graphs.iter().map(RecordBatch::num_rows).sum::<usize>(), 1);
    let graph: serde_json::Value = serde_json::from_slice(
        graphs
            .iter()
            .find(|batch| batch.num_rows() > 0)
            .unwrap()
            .column_by_name("native_graph_json")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow_array::BinaryArray>()
            .unwrap()
            .value(0),
    )
    .unwrap();
    assert_eq!(graph["version"], 1);
    let units = graph["units"].as_array().unwrap();
    assert_eq!(units.len(), 3);
    let library = units.iter().find(|unit| unit["mode"] == "check").unwrap();
    assert_eq!(library["target"]["src_path"], "/workspace/src/lib.rs");
    let build = units.iter().find(|unit| unit["mode"] == "build").unwrap();
    let execution = units
        .iter()
        .find(|unit| unit["mode"] == "run-custom-build")
        .unwrap();
    assert_eq!(
        build["target"]["src_path"],
        "/workspace/custom/configure.rs"
    );
    assert_ne!(build["profile"], execution["profile"]);
    let selection = selected_relation_batches(&selected, "source.cargo_unit_graph_selection");
    assert_eq!(
        selection.iter().map(RecordBatch::num_rows).sum::<usize>(),
        1
    );
    let run = selection
        .iter()
        .find(|batch| batch.num_rows() > 0)
        .unwrap()
        .column_by_name("provider_run_id")
        .unwrap()
        .as_any()
        .downcast_ref::<arrow_array::BinaryArray>()
        .unwrap()
        .value(0)
        .to_vec();
    let pins = [
        "source.cargo_unit_graph",
        "source.cargo_compilation_unit",
        "source.cargo_unit_dependency",
    ]
    .into_iter()
    .map(|relation| {
        assert!(
            selected_relation_batches(&selected, relation)
                .iter()
                .any(|batch| batch.num_rows() > 0)
        );
        selected
            .table_versions()
            .components()
            .find(|(id, _)| *id == relation)
            .unwrap()
            .1
            .clone()
    })
    .collect();
    (
        selected.row().pins.source_generation.get(),
        graph,
        pins,
        run,
    )
}

#[test]
fn custom_cargo_build_input_changes_context_and_matches_clean_public_results() {
    let fixture = ProductionFixture::with_source(b"marker = 1\n");
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir(root.join("src")).unwrap();
    fs::create_dir(root.join("custom")).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\nbuild = \"custom/configure.rs\"\n[lib]\ntest = false\ndoctest = false\n").unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), b"#[cfg(selected)]\npub fn selected() -> u32 { 1 }\n#[cfg(not(selected))]\npub fn alternate() -> u32 { 2 }\npub fn caller() -> u32 {\n    #[cfg(selected)] { selected() }\n    #[cfg(not(selected))] { alternate() }\n}\n").unwrap();
    let build_script = |selected: bool| {
        format!(
            "fn main() {{ println!(\"cargo::rustc-check-cfg=cfg(selected)\"); {} }}\n",
            if selected {
                "println!(\"cargo::rustc-cfg=selected\");"
            } else {
                ""
            }
        )
    };
    fs::write(root.join("custom/configure.rs"), build_script(true)).unwrap();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let registration = fixture.root().join("registration.sqlite3");
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
        store.backup_to(&registration).unwrap();
    }
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    wait_for_semantic_activation_with_timeout(&fixture, Duration::from_secs(300));
    let initial_graph = cargo_build_unit_graph_observation(&fixture);
    let initial = cargo_build_script_observation(
        &fixture,
        &stack,
        "cargo-build-initial",
        "fixture::selected",
    );
    fs::write(root.join("custom/configure.rs"), build_script(false)).unwrap();
    wait_for_semantic_activation_after_generation(
        &fixture,
        Some(initial_graph.0),
        Duration::from_secs(300),
    );
    let live_graph = cargo_build_unit_graph_observation(&fixture);
    assert_ne!(initial_graph.0, live_graph.0);
    assert_eq!(
        initial_graph.1, live_graph.1,
        "Cargo structure is unchanged by build-script bytes"
    );
    assert_eq!(
        initial_graph.2, live_graph.2,
        "nonempty graph/unit/dependency pins survive source publication"
    );
    assert_ne!(
        initial_graph.3, live_graph.3,
        "the current selection has its own provider run"
    );
    let live = cargo_build_script_observation(
        &fixture,
        &stack,
        "cargo-build-edited",
        "fixture::alternate",
    );
    assert_ne!(
        initial[0].rows[0]["context_id"], live[0].rows[0]["context_id"],
        "the changed captured build input selects another effective context"
    );
    let clean = clean_fixture(&fixture, &registration, &stack);
    let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
    wait_for_semantic_activation_with_timeout(&clean, Duration::from_secs(300));
    assert_eq!(live_graph.1, cargo_build_unit_graph_observation(&clean).1);
    assert_eq!(
        live,
        cargo_build_script_observation(&clean, &stack, "cargo-build-clean", "fixture::alternate")
    );
    clean_supervisor.stop();
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let reopened_graph = cargo_build_unit_graph_observation(&fixture);
    assert_eq!(live_graph, reopened_graph);
    assert_eq!(
        live,
        cargo_build_script_observation(
            &fixture,
            &stack,
            "cargo-build-reopened",
            "fixture::alternate"
        )
    );
    supervisor.stop();
}

fn cargo_target_observation(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    leaf: Option<&str>,
    missing: Option<&str>,
) -> Vec<SemanticObservation> {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "unused",
        "Rust function declarations",
    );
    request["scope"]["languages"] = json!(["rust"]);
    let entities = public_query(
        fixture,
        stack,
        &format!("{phase}-entities"),
        request.clone(),
    );
    let remaining = u64::from(missing.is_some());
    assert_eq!(
        entities.processing[0]["requested_partitions"],
        u64::from(leaf.is_some()) + remaining
    );
    assert_eq!(entities.processing[0]["remaining_partitions"], remaining);
    if let Some(missing) = missing {
        assert_eq!(
            entities.processing[0]["remainder"][0]["target_platform"],
            missing
        );
        assert_eq!(
            entities.processing[0]["remainder"][0]["state"],
            "unavailable"
        );
    }
    let Some(leaf) = leaf else {
        assert!(entities.rows.is_empty());
        return vec![entities];
    };
    if entities.rows.is_empty() {
        print_cargo_failure(fixture);
    }
    assert_eq!(
        entities
            .rows
            .iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["fixture::caller", leaf])
    );
    let by_name = entities
        .rows
        .iter()
        .map(|row| (row["name"].as_str().unwrap(), &row["public_entity_id"]))
        .collect::<BTreeMap<_, _>>();
    request["queries"] = json!([{
        "request": "follow code relationships", "query_id": "calls",
        "starting_from": [{"entity_id": by_name["fixture::caller"]}],
        "relationship": "calls", "direction": "outgoing", "distance": "one relationship step"
    }]);
    let calls = public_query(fixture, stack, &format!("{phase}-calls"), request.clone());
    assert_eq!(calls.rows.len(), 1);
    assert_eq!(calls.rows[0]["public_target_entity_id"], *by_name[leaf]);
    assert_eq!(calls.processing[0]["remaining_partitions"], 0);
    request["queries"] = json!([{
        "request": "retrieve source and syntax context", "query_id": "source",
        "about": [{"entity_id": by_name[leaf]}], "context": "function body"
    }]);
    let source = public_query(fixture, stack, &format!("{phase}-source"), request);
    assert_eq!(source.rows.len(), 1);
    assert_eq!(
        source.rows[0]["source_context"]["text"],
        if leaf == "fixture::selected" {
            "{ 1 }"
        } else {
            "{ 2 }"
        }
    );
    vec![entities, calls, source]
}

fn selected_compiler_host() -> String {
    let compiler_identity: Value = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/rustc-extractor/toolchain-identity.json"
    )))
    .unwrap();
    let version = std::process::Command::new("rustup")
        .args([
            "run",
            compiler_identity["toolchain"].as_str().unwrap(),
            "rustc",
            "-vV",
        ])
        .output()
        .unwrap();
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).unwrap();
    version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap()
        .to_owned()
}

fn write_cargo_platform_fixture(root: &Path) {
    fs::create_dir(root.join("src")).unwrap();
    fs::create_dir(root.join(".cargo")).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = 'fixture'\nversion = '0.1.0'\nedition = '2024'\n[lib]\ntest = false\ndoctest = false\n").unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = 'fixture'\nversion = '0.1.0'\n",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), b"#[cfg(selected)]\npub fn selected() -> u32 { 1 }\n#[cfg(not(selected))]\npub fn alternate() -> u32 { 2 }\npub fn caller() -> u32 {\n    #[cfg(selected)] { selected() }\n    #[cfg(not(selected))] { alternate() }\n}\n").unwrap();
}

#[test]
fn cargo_configured_platforms_and_flags_converge_with_clean_public_queries() {
    let fixture = ProductionFixture::with_source(b"marker = 1\n");
    let root = Path::new(&fixture.workspace.root_path_display);
    write_cargo_platform_fixture(root);
    let missing = "codefabric-missing-platform";
    fs::write(
        root.join(".cargo/config.toml"),
        format!("[build]\ntarget = '{missing}'\n"),
    )
    .unwrap();
    let host = selected_compiler_host();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let registration = fixture.root().join("registration.sqlite3");
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
        store.backup_to(&registration).unwrap();
    }
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    cargo_target_observation(
        &fixture,
        &stack,
        "cargo-platform-missing",
        None,
        Some(missing),
    );
    fs::write(
        root.join(".cargo/config.toml"),
        format!("[build]\ntarget = ['host-tuple', '{host}', '{missing}', '{missing}']\n"),
    )
    .unwrap();
    let partial = cargo_target_observation(
        &fixture,
        &stack,
        "cargo-platform-mixed",
        Some("fixture::alternate"),
        Some(missing),
    );
    fs::write(
        root.join(".cargo/config"),
        "[build]\ntarget = 'host-tuple'\nrustflags = ['--cfg', 'selected']\n",
    )
    .unwrap();
    let selected = cargo_target_observation(
        &fixture,
        &stack,
        "cargo-platform-selected",
        Some("fixture::selected"),
        None,
    );
    assert_ne!(
        partial[0].rows[0]["context_id"],
        selected[0].rows[0]["context_id"]
    );
    fs::write(
        root.join(".cargo/config.toml"),
        "[build]\ntarget = 'another-missing-platform'\nrustflags = ['--cfg', 'ignored_flag']\n",
    )
    .unwrap();
    let unchanged = cargo_target_observation(
        &fixture,
        &stack,
        "cargo-platform-ignored",
        Some("fixture::selected"),
        None,
    );
    assert_eq!(
        selected, unchanged,
        "unselected configuration must not change the effective context"
    );
    let clean = clean_fixture(&fixture, &registration, &stack);
    let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        unchanged,
        cargo_target_observation(
            &clean,
            &stack,
            "cargo-platform-clean",
            Some("fixture::selected"),
            None
        )
    );
    clean_supervisor.stop();
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        unchanged,
        cargo_target_observation(
            &fixture,
            &stack,
            "cargo-platform-reopened",
            Some("fixture::selected"),
            None
        )
    );
    supervisor.stop();
}

#[test]
fn cargo_library_linkage_kinds_survive_live_queries_and_clean_reopen() {
    let fixture = ProductionFixture::with_source(b"marker = 1\n");
    let root = Path::new(&fixture.workspace.root_path_display);
    write_cargo_platform_fixture(root);
    let manifest = |types: &str| {
        format!(
            "[package]\nname = 'fixture'\nversion = '0.1.0'\nedition = '2024'\n[lib]\ntest = false\ndoctest = false\ncrate-type = [{types}]\n"
        )
    };
    fs::write(root.join("Cargo.toml"), manifest("'cdylib'")).unwrap();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let registration = fixture.root().join("registration.sqlite3");
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
        store.backup_to(&registration).unwrap();
    }
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let mut observed = cargo_target_observation(
        &fixture,
        &stack,
        "cargo-linkage-cdylib",
        Some("fixture::alternate"),
        None,
    );
    for (phase, types) in [
        ("dylib", "'dylib'"),
        ("multiple", "'rlib', 'cdylib', 'staticlib'"),
    ] {
        fs::write(root.join("Cargo.toml"), manifest(types)).unwrap();
        let next = cargo_target_observation(
            &fixture,
            &stack,
            &format!("cargo-linkage-{phase}"),
            Some("fixture::alternate"),
            None,
        );
        assert_ne!(
            observed[0].rows[0]["context_id"],
            next[0].rows[0]["context_id"]
        );
        observed = next;
    }
    let clean = clean_fixture(&fixture, &registration, &stack);
    let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        observed,
        cargo_target_observation(
            &clean,
            &stack,
            "cargo-linkage-clean",
            Some("fixture::alternate"),
            None
        )
    );
    clean_supervisor.stop();
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        observed,
        cargo_target_observation(
            &fixture,
            &stack,
            "cargo-linkage-reopen",
            Some("fixture::alternate"),
            None
        )
    );
    supervisor.stop();
}

fn assert_cargo_selection_failures(processing: &Value, failures: &[Value]) {
    let mut actual_failures = processing["remainder"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            assert_eq!(row["state"], "unavailable");
            assert_eq!(row["path"], "member/Cargo.toml");
            row["rust_build"].clone()
        })
        .collect::<Vec<_>>();
    actual_failures.sort_by_cached_key(Value::to_string);
    let mut expected_failures = failures.to_vec();
    expected_failures.sort_by_cached_key(Value::to_string);
    assert_eq!(actual_failures, expected_failures);
}

fn cargo_build_selection_observation(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    leaves: &[&str],
    failures: &[Value],
) -> Vec<SemanticObservation> {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "unused",
        "Rust function declarations",
    );
    request["scope"]["languages"] = json!(["rust"]);
    let entities = public_query(
        fixture,
        stack,
        &format!("{phase}-entities"),
        request.clone(),
    );
    if entities.rows.is_empty() {
        print_cargo_failure(fixture);
    }
    assert_eq!(
        entities.processing[0]["requested_partitions"],
        (leaves.len() + failures.len()) as u64
    );
    assert_eq!(
        entities.processing[0]["remaining_partitions"],
        failures.len() as u64
    );
    let mut expected = leaves
        .iter()
        .copied()
        .chain(std::iter::repeat_n("fixture::caller", leaves.len()))
        .collect::<Vec<_>>();
    expected.sort_unstable();
    let mut actual = entities
        .rows
        .iter()
        .map(|row| row["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(actual, expected);
    assert_cargo_selection_failures(&entities.processing[0], failures);
    let mut observations = Vec::new();
    for (index, leaf) in entities
        .rows
        .iter()
        .filter(|row| row["name"] != "fixture::caller")
        .enumerate()
    {
        let callers = entities
            .rows
            .iter()
            .filter(|row| {
                row["name"] == "fixture::caller" && row["context_id"] == leaf["context_id"]
            })
            .collect::<Vec<_>>();
        assert_eq!(callers.len(), 1);
        request["queries"] = json!([{
            "request": "follow code relationships", "query_id": "calls",
            "starting_from": [{"entity_id": callers[0]["public_entity_id"]}],
            "relationship": "calls", "direction": "outgoing", "distance": "one relationship step"
        }]);
        let calls = public_query(
            fixture,
            stack,
            &format!("{phase}-calls-{index}"),
            request.clone(),
        );
        assert_eq!(calls.rows.len(), 1);
        assert_eq!(
            calls.rows[0]["public_target_entity_id"],
            leaf["public_entity_id"]
        );
        assert_eq!(calls.processing[0]["requested_partitions"], 1);
        assert_eq!(calls.processing[0]["remaining_partitions"], 0);
        request["queries"] = json!([{
            "request": "retrieve source and syntax context", "query_id": "source",
            "about": [{"entity_id": leaf["public_entity_id"]}], "context": "function body"
        }]);
        let source = public_query(
            fixture,
            stack,
            &format!("{phase}-source-{index}"),
            request.clone(),
        );
        assert_eq!(source.rows.len(), 1);
        assert_eq!(
            source.rows[0]["source_context"]["text"],
            if leaf["name"] == "fixture::selected" {
                "{ 1 }"
            } else {
                "{ 2 }"
            }
        );
        observations.extend([calls, source]);
    }
    observations.insert(0, entities);
    observations
}

fn write_cargo_build_selection_fixture(root: &Path) -> (String, String) {
    fs::create_dir_all(root.join("member/src")).unwrap();
    let workspace = "[workspace]\nmembers=['member']\nresolver='3'\n[profile.checking]\ninherits='dev'\ndebug-assertions=false\n".to_owned();
    let member = "[package]\nname='fixture'\nversion='0.1.0'\nedition='2024'\n[lib]\ntest=false\ndoctest=false\n[features]\ndefault=['selected']\nselected=[]\n".to_owned();
    fs::write(root.join("Cargo.toml"), &workspace).unwrap();
    fs::write(root.join("member/Cargo.toml"), &member).unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version=4\n[[package]]\nname='fixture'\nversion='0.1.0'\n",
    )
    .unwrap();
    fs::write(root.join("member/src/lib.rs"), "#[cfg(all(feature=\"selected\", debug_assertions))]\npub fn selected() -> u32 { 1 }\n#[cfg(not(all(feature=\"selected\", debug_assertions)))]\npub fn alternate() -> u32 { 2 }\npub fn caller() -> u32 {\n    #[cfg(all(feature=\"selected\", debug_assertions))] { selected() }\n    #[cfg(not(all(feature=\"selected\", debug_assertions)))] { alternate() }\n}\n").unwrap();
    (workspace, member)
}

#[test]
fn cargo_feature_and_profile_selections_keep_partial_contexts_and_equal_clean_queries() {
    let fixture = ProductionFixture::with_source(b"marker = 1\n");
    let root = Path::new(&fixture.workspace.root_path_display);
    let (workspace, member) = write_cargo_build_selection_fixture(root);
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let registration = fixture.root().join("registration.sqlite3");
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
        store.backup_to(&registration).unwrap();
    }
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    cargo_build_selection_observation(
        &fixture,
        &stack,
        "cargo-selection-default",
        &["fixture::selected"],
        &[],
    );
    fs::write(root.join("Cargo.toml"), format!("{workspace}\n[workspace.metadata.codefabric]\nrust_contexts=[{{profile='checking'}},{{features=['selected'],default_features=false}},{{features=['selected','selected'],default_features=false,platforms=['host-tuple']}},{{profile='missing-profile'}},{{features=['missing-feature']}},{{profile=3}}]\n")).unwrap();
    cargo_build_selection_observation(
        &fixture,
        &stack,
        "cargo-selection-mixed",
        &["fixture::selected", "fixture::alternate"],
        &[
            json!({"profile": "missing-profile", "features": [], "default_features": true}),
            json!({"profile": "dev", "features": ["missing-feature"], "default_features": true}),
            Value::Null,
        ],
    );
    fs::write(
        root.join("member/Cargo.toml"),
        format!(
            "{member}\n[package.metadata.codefabric]\nrust_contexts=[{{default_features=false}}]\n"
        ),
    )
    .unwrap();
    let final_live = cargo_build_selection_observation(
        &fixture,
        &stack,
        "cargo-selection-override",
        &["fixture::alternate"],
        &[],
    );
    let clean = clean_fixture(&fixture, &registration, &stack);
    let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        final_live,
        cargo_build_selection_observation(
            &clean,
            &stack,
            "cargo-selection-clean",
            &["fixture::alternate"],
            &[]
        )
    );
    clean_supervisor.stop();
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        final_live,
        cargo_build_selection_observation(
            &fixture,
            &stack,
            "cargo-selection-reopen",
            &["fixture::alternate"],
            &[]
        )
    );
    supervisor.stop();
}

fn encoded_sources(utf8: bool) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let python = if utf8 {
        "# coding: utf-8\r\n# é\r\nfrom helper import café\r\ndef caller():\r\n    return café()\r\n".as_bytes()
    } else {
        b"# coding: latin-1\r\n# \xe9\r\nfrom helper import caf\xe9\r\ndef caller():\r\n    return caf\xe9()\r\n"
    };
    let helper = "# coding: utf-8\r\n# é\r\ndef café() -> str:\r\n    return '😀é'\r\n";
    let rust = "// é\r\n\r\npub fn rust_leaf() -> u32 {\r\n    7\r\n}\r\n".as_bytes();
    let prefix = if utf8 {
        b"".as_slice()
    } else {
        b"\xef\xbb\xbf"
    };
    (
        python.to_vec(),
        [prefix, helper.as_bytes()].concat(),
        [prefix, rust].concat(),
    )
}

fn assert_encoded_text_columns(context: &serde_json::Value, name: &str, utf8: bool) {
    let (start_line, end_line, byte_column, utf8_column, utf16_column) = match name {
        "caller" => (4, 5, if utf8 { 18 } else { 17 }, 18, 17),
        "café" => (3, 4, 19, 19, 16),
        _ => (3, 5, 1, 1, 1),
    };
    assert_eq!(context["start_line"], start_line);
    assert_eq!(context["end_line"], end_line);
    assert_eq!(context["start_byte_column"], 0);
    assert_eq!(context["start_utf8_column"], 0);
    assert_eq!(context["start_utf16_column"], 0);
    assert_eq!(context["end_byte_column"], byte_column);
    assert_eq!(context["end_utf8_column"], utf8_column);
    assert_eq!(context["end_utf16_column"], utf16_column);
}

fn encoded_source_observation(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    phase: &str,
    utf8: bool,
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
    let by_name = entities
        .rows
        .iter()
        .map(|row| {
            (
                row["name"].as_str().unwrap(),
                row["public_entity_id"].clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        by_name.keys().copied().collect::<Vec<_>>(),
        ["café", "caller", "fixture::rust_leaf"]
    );
    assert_eq!(
        entities.processing[0]["remaining_partitions"], 0,
        "{phase}: encoded declarations"
    );
    request["queries"] = json!([{
        "request": "follow code relationships", "query_id": "calls", "starting_from": [{"entity_id": by_name["caller"]}],
        "relationship": "calls", "direction": "outgoing", "distance": "one relationship step", "return": {"limit": {"maximum_results": 32}}
    }]);
    let calls = public_query(fixture, stack, &format!("{phase}-calls"), request.clone());
    assert_eq!(calls.rows.len(), 1);
    assert_eq!(calls.rows[0]["public_target_entity_id"], by_name["café"]);
    assert_eq!(
        calls.processing[0]["remaining_partitions"], 0,
        "{phase}: encoded target definition"
    );
    let subjects = by_name
        .values()
        .map(|id| json!({"entity_id": id}))
        .collect::<Vec<_>>();
    request["queries"] = json!([{
        "request": "retrieve source and syntax context", "query_id": "source", "about": subjects,
        "context": "function definition", "return": {"maximum_source_bytes": 4096, "limit": {"maximum_results": 32}}
    }]);
    let source = public_query(fixture, stack, &format!("{phase}-source"), request);
    assert_eq!(source.rows.len(), 3, "{phase}: encoded function owners");
    assert_eq!(
        source.processing[0]["remaining_partitions"], 0,
        "{phase}: decoded syntax mapping"
    );
    let (python, helper, rust) = encoded_sources(utf8);
    for (name, bytes, expected) in [
        (
            "caller",
            python.as_slice(),
            if utf8 {
                "def caller():\r\n    return café()".as_bytes()
            } else {
                b"def caller():\r\n    return caf\xe9()"
            },
        ),
        (
            "café",
            helper.as_slice(),
            "def café() -> str:\r\n    return '😀é'".as_bytes(),
        ),
        (
            "fixture::rust_leaf",
            rust.as_slice(),
            b"pub fn rust_leaf() -> u32 {\r\n    7\r\n}".as_slice(),
        ),
    ] {
        let row = source.rows.iter().find(|row| row["name"] == name).unwrap();
        let start = bytes
            .windows(expected.len())
            .position(|window| window == expected)
            .unwrap();
        let context = &row["source_context"];
        assert_eq!(context["start_byte"], start);
        assert_eq!(context["end_byte"], start + expected.len());
        assert_eq!(context["returned_bytes"], expected.len());
        assert_eq!(context["complete"], true);
        assert_encoded_text_columns(context, name, utf8);
        if let Ok(text) = std::str::from_utf8(expected) {
            assert_eq!(context["text"], text);
        } else {
            assert!(context["text"].is_null());
            assert_eq!(
                context["bytes"],
                "6465662063616c6c657228293a0d0a2020202072657475726e20636166e92829"
            );
        }
    }
    vec![entities, calls, source]
}

#[test]
fn live_mixed_decoded_sources_equal_original_bytes_and_independent_clean_queries() {
    let (python, helper, rust) = encoded_sources(false);
    let fixture = ProductionFixture::with_source(&python);
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::write(root.join("helper.py"), helper).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), rust).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\ntest = false\ndoctest = false\n").unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let registration = fixture.root().join("registration.sqlite3");
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
        store.backup_to(&registration).unwrap();
    }
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let initial = encoded_source_observation(&fixture, &stack, "encoding-initial", false);
    for (phase, utf8) in [("encoding-utf8", true), ("encoding-restored", false)] {
        let (python, helper, rust) = encoded_sources(utf8);
        fs::write(root.join("sample.py"), python).unwrap();
        fs::write(root.join("helper.py"), helper).unwrap();
        fs::write(root.join("src/lib.rs"), rust).unwrap();
        let live = encoded_source_observation(&fixture, &stack, phase, utf8);
        let clean = clean_fixture(&fixture, &registration, &stack);
        let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
        let expected = encoded_source_observation(&clean, &stack, &format!("clean-{phase}"), utf8);
        assert_eq!(
            live, expected,
            "{phase}: decoded inputs match independent clean state"
        );
        if !utf8 {
            assert_eq!(
                live, initial,
                "restored encoding restores source and identity"
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
    let deadline = Instant::now() + Duration::from_secs(180);
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
fn explicit_poll_profile_publishes_nested_source_changes_and_reopens_exactly() {
    use arrow::array::StringArray;
    let fixture = ProductionFixture::with_source_and_activation_startup_fault(
        b"def original():\n    return 1\n",
        Some("hold_semantic_update_publication"),
    );
    install_separate_git_metadata(&fixture);
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::write(
        root.join(".gitmodules"),
        "[submodule \"vendor\"]\npath = target/vendor\n",
    )
    .unwrap();
    let git_attributes = fixture.state.join("observed-git/info/attributes");
    fs::write(&git_attributes, "*.py codefabric-input=initial\n").unwrap();
    fs::write(fixture.state.join("observed-git/info/exclude"), "*.py\n").unwrap();
    {
        use gix::bstr::ByteSlice as _;
        let mut index = gix::index::State::new(gix::hash::Kind::Sha1);
        for stage in [
            gix::index::entry::Stage::Ours,
            gix::index::entry::Stage::Theirs,
        ] {
            index.dangerously_push_entry(
                Default::default(),
                gix::hash::ObjectId::from_hex(&[b'1'; 40]).unwrap(),
                gix::index::entry::Flags::from_stage(stage),
                gix::index::entry::Mode::FILE,
                b"sample.py".as_bstr(),
            );
        }
        index.dangerously_push_entry(
            Default::default(),
            gix::hash::ObjectId::from_hex(&[b'4'; 40]).unwrap(),
            gix::index::entry::Flags::from_stage(gix::index::entry::Stage::Unconflicted),
            gix::index::entry::Mode::COMMIT,
            b"target/vendor".as_bstr(),
        );
        index.sort_entries();
        gix::index::File::from_state(index, fixture.state.join("observed-git/index"))
            .write(Default::default())
            .unwrap();
    }
    let configuration = fs::read_to_string(&fixture.config_path).unwrap().replace(
        "[static_config]",
        "[static_config]\nsource_watch_profile = \"poll\"",
    );
    write_private(&fixture.config_path, configuration.as_bytes());
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let query = |label: &str| {
        let request = semantic_request(
            &fixture.workspace.public_id(),
            "unused",
            "Python function declarations",
        );
        let (result, batches) = source_current_query(&fixture, &stack, label, request);
        let names = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column_by_name("name")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .iter()
                    .flatten()
                    .map(ToOwned::to_owned)
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(result["processing"][0]["remaining_partitions"], 0);
        (result, names)
    };
    let (initial, names) = query("poll-initial");
    assert_eq!(names, BTreeSet::from(["original".to_owned()]));
    let metadata_pin = |relation: &str| {
        let selected = all_activation_control_rows(&fixture)
            .into_iter()
            .max_by_key(|row| row.row().ordinal.get())
            .unwrap();
        let pin = selected
            .table_versions()
            .components()
            .find_map(|(id, pin)| (id == relation).then(|| pin.clone()))
            .unwrap();
        pin
    };
    let initial_stage_pin = metadata_pin("source.git_index_stage");
    let selected = all_activation_control_rows(&fixture)
        .into_iter()
        .max_by_key(|row| row.row().ordinal.get())
        .unwrap();
    let stages = selected_relation_batches(&selected, "source.git_index_stage");
    let observed_stages = stages
        .iter()
        .flat_map(|batch| {
            batch
                .column_by_name("stage")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::Int16Array>()
                .expect("CodeFabric's storage contract widens UInt8 to signed Int16")
                .values()
                .iter()
                .copied()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(observed_stages, BTreeSet::from([0, 2, 3]));
    assert_eq!(submodule_observation(&fixture), (false, false));
    fs::create_dir_all(root.join("target/vendor")).unwrap();
    gix::init(root.join("target/vendor")).unwrap();
    fs::create_dir_all(root.join("target/vendor/target/deep")).unwrap();
    gix::init(root.join("target/vendor/target/deep")).unwrap();
    fs::write(
        root.join("target/vendor/.gitmodules"),
        "[submodule \"deep\"]\npath = target/deep\n",
    )
    .unwrap();
    fs::write(
        root.join("target/vendor/target/deep/nested.py"),
        "def nested_polled():\n    return 3\n",
    )
    .unwrap();
    fs::write(
        root.join("target/vendor/added.py"),
        "def polled():\n    return 2\n",
    )
    .unwrap();
    // Read the durable selected snapshot without requesting a query census. This proves
    // background convergence and avoids using one query's deadline as a publication timer.
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let selected = all_activation_control_rows(&fixture)
            .into_iter()
            .max_by_key(|row| row.row().ordinal.get())
            .unwrap();
        let captured = selected_relation_batches(&selected, "source.exact_source_bytes");
        let added = [
            (
                b"target/vendor/added.py".as_slice(),
                b"def polled():\n    return 2\n".as_slice(),
            ),
            (
                b"target/vendor/target/deep/nested.py".as_slice(),
                b"def nested_polled():\n    return 3\n".as_slice(),
            ),
        ]
        .iter()
        .all(|(expected_path, expected_bytes)| {
            captured.iter().any(|batch| {
                let binary = |name| {
                    batch
                        .column_by_name(name)
                        .unwrap()
                        .as_any()
                        .downcast_ref::<arrow::array::BinaryArray>()
                        .unwrap()
                };
                binary("relative_path")
                    .iter()
                    .zip(binary("source_bytes").iter())
                    .any(|(path, bytes)| {
                        path == Some(*expected_path) && bytes == Some(*expected_bytes)
                    })
            })
        });
        if added {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "polling did not publish the nested source"
        );
        thread::sleep(Duration::from_millis(100));
    }
    let (updated, names) = query("poll-updated");
    assert_eq!(
        initial_stage_pin,
        metadata_pin("source.git_index_stage"),
        "unchanged nonempty index-stage rows retain their exact version"
    );
    let updated_path_pin = metadata_pin("source.git_path_context");
    assert_eq!(submodule_observation(&fixture), (true, true));
    assert_eq!(
        submodule_observation_for(&fixture, b"target/vendor/target/deep", b"deep"),
        (true, true)
    );
    let boundary_pin = metadata_pin("source.git_submodule_boundary");
    assert_eq!(
        names,
        BTreeSet::from([
            "original".to_owned(),
            "polled".to_owned(),
            "nested_polled".to_owned()
        ])
    );
    assert!(
        updated["source_generation"].as_u64().unwrap()
            > initial["source_generation"].as_u64().unwrap()
    );
    let selected_attribute = || {
        let selected = all_activation_control_rows(&fixture)
            .into_iter()
            .max_by_key(|row| row.row().ordinal.get())
            .unwrap();
        let batches = selected_relation_batches(&selected, "source.git_attribute");
        batches.iter().any(|batch| {
            let names = batch
                .column_by_name("name")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let values = batch
                .column_by_name("value")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::BinaryArray>()
                .unwrap();
            names.iter().zip(values.iter()).any(|(name, value)| {
                name == Some("codefabric-input") && value == Some(b"revised".as_slice())
            })
        })
    };
    // Metadata alone must reach the exact snapshot through background observation. Git ignore
    // rules never remove these explicitly admitted sources, and attributes never rewrite bytes.
    fs::write(&git_attributes, "*.py codefabric-input=revised\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    while !selected_attribute() {
        assert!(
            Instant::now() < deadline,
            "external Git attributes did not converge"
        );
        thread::sleep(Duration::from_millis(100));
    }
    let (metadata_updated, names) = query("poll-git-metadata-updated");
    assert_eq!(boundary_pin, metadata_pin("source.git_submodule_boundary"));
    assert_eq!(initial_stage_pin, metadata_pin("source.git_index_stage"));
    assert_eq!(
        updated_path_pin,
        metadata_pin("source.git_path_context"),
        "an attribute-only edit does not rewrite unchanged path classification"
    );
    assert_eq!(
        names,
        BTreeSet::from([
            "original".to_owned(),
            "polled".to_owned(),
            "nested_polled".to_owned()
        ])
    );
    assert!(
        metadata_updated["source_generation"].as_u64().unwrap()
            > updated["source_generation"].as_u64().unwrap()
    );
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let (reopened, names) = query("poll-reopened");
    assert_eq!(
        names,
        BTreeSet::from([
            "original".to_owned(),
            "polled".to_owned(),
            "nested_polled".to_owned()
        ])
    );
    assert_eq!(
        reopened["source_generation"],
        metadata_updated["source_generation"]
    );
    assert!(selected_attribute());
    assert_eq!(submodule_observation(&fixture), (true, true));
    assert_eq!(
        submodule_observation_for(&fixture, b"target/vendor/target/deep", b"deep"),
        (true, true)
    );
    assert_eq!(boundary_pin, metadata_pin("source.git_submodule_boundary"));
    assert_eq!(initial_stage_pin, metadata_pin("source.git_index_stage"));
    supervisor.stop();
}

fn submodule_observation(fixture: &ProductionFixture) -> (bool, bool) {
    submodule_observation_for(fixture, b"target/vendor", b"vendor")
}

fn submodule_observation_for(
    fixture: &ProductionFixture,
    path: &[u8],
    name: &[u8],
) -> (bool, bool) {
    let selected = all_activation_control_rows(fixture)
        .into_iter()
        .max_by_key(|row| row.row().ordinal.get())
        .unwrap();
    let batches = selected_relation_batches(&selected, "source.git_submodule_boundary");
    let mut found = Vec::new();
    for batch in &batches {
        let bytes = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<arrow_array::BinaryArray>()
                .unwrap()
        };
        let flag = |name, row| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<arrow_array::BooleanArray>()
                .unwrap()
                .value(row)
        };
        for (row, value) in bytes("relative_path").iter().enumerate() {
            if value == Some(path) {
                assert_eq!(bytes("name").value(row), name);
                found.push((
                    flag("captured_sources", row),
                    flag("repository_observed", row),
                ));
            }
        }
    }
    assert_eq!(
        found.len(),
        1,
        "one unambiguous selected submodule boundary for {path:?}"
    );
    found[0]
}

fn install_separate_git_metadata(fixture: &ProductionFixture) {
    // Real Git administration stays outside captured CPG inputs while the production
    // watcher resolves its metadata topology through the selected workspace's .git file.
    let root = Path::new(&fixture.workspace.root_path_display);
    drop(gix::init(root).unwrap());
    let metadata = fixture.state.join("observed-git");
    fs::rename(root.join(".git"), &metadata).unwrap();
    fs::write(
        root.join(".git"),
        format!("gitdir: {}\n", metadata.display()),
    )
    .unwrap();
}

fn selected_source_pins(
    fixture: &ProductionFixture,
) -> Vec<(String, codefabric::fabric::delta_exact::ExactDeltaPin)> {
    let selected = all_activation_control_rows(fixture)
        .into_iter()
        .max_by_key(|row| row.row().ordinal.get())
        .unwrap();
    ["source.exact_source_bytes", "source.code_line_index"]
        .into_iter()
        .map(|relation| {
            let (id, pin) = selected
                .table_versions()
                .components()
                .find(|(id, _)| *id == relation)
                .unwrap();
            (id.to_owned(), pin.clone())
        })
        .collect()
}

fn selected_comment_pin(
    fixture: &ProductionFixture,
    expected_rows: usize,
) -> codefabric::fabric::delta_exact::ExactDeltaPin {
    let selected = all_activation_control_rows(fixture)
        .into_iter()
        .max_by_key(|row| row.row().ordinal.get())
        .unwrap();
    let relation = "provider.ruff.comment";
    assert_eq!(
        selected_relation_batches(&selected, relation)
            .iter()
            .map(RecordBatch::num_rows)
            .sum::<usize>(),
        expected_rows
    );
    selected
        .table_versions()
        .components()
        .find(|(id, _)| *id == relation)
        .unwrap()
        .1
        .clone()
}

#[test]
fn shutdown_during_semantic_delta_publication_joins_workspace_owners() {
    exercise_semantic_publication_shutdown(PublicationShutdownProbe::Python);
}

#[test]
fn mixed_semantic_publication_shutdown_joins_workspace_owners() {
    exercise_semantic_publication_shutdown(PublicationShutdownProbe::Mixed);
}

#[test]
fn timed_out_client_during_publication_drains_and_reopens_exactly() {
    exercise_semantic_publication_shutdown(PublicationShutdownProbe::ClientTimeout);
}

#[derive(Clone, Copy)]
enum PublicationShutdownProbe {
    Python,
    Mixed,
    ClientTimeout,
}

fn exercise_semantic_publication_shutdown(probe: PublicationShutdownProbe) {
    let mixed = !matches!(probe, PublicationShutdownProbe::Python);
    let fixture = ProductionFixture::new();
    if mixed {
        let root = Path::new(&fixture.workspace.root_path_display);
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\ntest = false\ndoctest = false\n").unwrap();
        fs::write(
            root.join("Cargo.lock"),
            "version = 4\n[[package]]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "pub fn leaf() -> u32 { 1 }\npub fn caller() -> u32 { leaf() }\n",
        )
        .unwrap();
    }
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let source = all_activation_control_rows(&fixture)
        .into_iter()
        .max_by_key(|row| row.row().ordinal.get())
        .unwrap();
    let published_epoch = source.row().pins.epoch.as_bytes().iter().fold(
        String::with_capacity(32),
        |mut value, byte| {
            std::fmt::Write::write_fmt(&mut value, format_args!("{byte:02x}"))
                .expect("format epoch");
            value
        },
    );
    wait_for_unselected_delta_writes(&fixture, &published_epoch, 8);
    assert_eq!(
        all_activation_control_rows(&fixture).len(),
        1,
        "shutdown occurs before semantic activation"
    );
    if matches!(probe, PublicationShutdownProbe::ClientTimeout) {
        timeout_semantic_query(&fixture, &stack);
    }
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let reopened = wait_for_semantic_activation_with_timeout(&fixture, Duration::from_secs(180));
    assert_eq!(
        source.row().pins.source_generation,
        reopened.row().pins.source_generation
    );
    let request = semantic_request(
        &fixture.workspace.public_id(),
        "unused",
        "Python function declarations",
    );
    let result = public_query(&fixture, &stack, "cancelled-publication-reopened", request);
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0]["name"], "answer");
    if mixed {
        let request = semantic_request(
            &fixture.workspace.public_id(),
            "unused",
            "Rust function declarations",
        );
        let result = public_query(
            &fixture,
            &stack,
            "cancelled-publication-rust-reopened",
            request,
        );
        assert_eq!(
            result
                .rows
                .iter()
                .map(|row| row["name"].as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["fixture::caller", "fixture::leaf"])
        );
    }
    supervisor.stop();
}

fn wait_for_unselected_delta_writes(
    fixture: &ProductionFixture,
    published_epoch: &str,
    committed_tables: usize,
) {
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        let writing = fs::read_dir(fixture.fabric_workspace_root().join("epochs"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() != published_epoch)
            .any(|entry| {
                fs::read_dir(entry.path().join("relations")).is_ok_and(|tables| {
                    tables
                        .filter_map(Result::ok)
                        .filter(|table| {
                            table
                                .path()
                                .join("_delta_log/00000000000000000001.json")
                                .is_file()
                        })
                        .take(committed_tables)
                        .count()
                        == committed_tables
                })
            });
        if writing {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "semantic candidate did not start real Delta writes"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn timeout_semantic_query(fixture: &ProductionFixture, stack: &InstalledProductionStack) {
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "request:timeout-during-publication",
        "Python function declarations",
    );
    request["freshness"] = json!({"policy": "require_semantic_current", "deadline_ms": 120_000});
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "query", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": request, "delivery": "resource"}, "timeout_seconds": 0.5}
        ]),
    );
    let path = write_modern_client_scenario(fixture, "timeout-during-publication", &scenario);
    let output = run_modern_client(stack, &path);
    assert!(
        !output.status.success(),
        "client must time out before semantic completion"
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["failure_step"], "query", "{report}");
    assert_eq!(report["failure_class"], "MCPError", "{report}");
    assert!(
        report["failure_public_error"].is_null(),
        "client transport timeout, not a daemon public failure: {report}"
    );
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
    let (initial_source, initial_rows) =
        source_current_query(&fixture, &stack, "genesis-source", request.clone());
    assert_eq!(initial_source["processing"][0]["remaining_partitions"], 0);
    assert_eq!(
        initial_rows
            .iter()
            .map(RecordBatch::num_rows)
            .sum::<usize>(),
        1
    );
    let original = initial_rows
        .iter()
        .find(|batch| batch.num_rows() == 1)
        .unwrap()
        .column_by_name("name")
        .unwrap()
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert_eq!(original.value(0), "original");
    let initial_rust = semantic_request(
        &fixture.workspace.public_id(),
        "unused",
        "Rust function declarations",
    );
    let (pending_rust, rows) = source_current_query(&fixture, &stack, "genesis-rust", initial_rust);
    assert!(rows.iter().all(|batch| batch.num_rows() == 0));
    assert_eq!(pending_rust["epoch_id"], initial_source["epoch_id"]);
    assert_eq!(pending_rust["processing"][0]["requested_partitions"], 1);
    assert_eq!(
        pending_rust["processing"][0]["remainder"][0]["state"],
        "pending"
    );
    let first_semantic = pending_semantic_candidate(&fixture);
    let initial_source_pins = selected_source_pins(&fixture);
    let initial_empty_comment_pin = selected_comment_pin(&fixture, 0);
    fs::write(first_semantic.with_extension("resume"), b"resume").unwrap();
    let initial = public_query(&fixture, &stack, "staged-initial", request.clone());
    assert_eq!(initial.rows[0]["name"], "original");
    assert_eq!(
        initial_source_pins,
        selected_source_pins(&fixture),
        "semantic stage reuses exact captured source tables"
    );
    let costs: Value = serde_json::from_slice(
        &fs::read(
            fixture
                .fabric_workspace_root()
                .join("semantic-preparation-costs.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(costs["reused_relation_versions"].as_u64().unwrap() > 2);
    assert_eq!(initial_empty_comment_pin, selected_comment_pin(&fixture, 0));
    let syntax = &costs["workspace_syntax_cache"];
    for field in ["python_reuses", "rust_reuses", "ruff_parse_reuses"] {
        assert!(
            syntax[field].as_u64().unwrap() >= 1,
            "retained {field}: {syntax}"
        );
    }
    assert_eq!(syntax["retained_entries"], 2);

    fs::write(
        workspace.join("sample.py"),
        b"# admitted current comment\ndef obsolete():\n    return 2\ndef caller():\n    return obsolete()\n",
    )
    .unwrap();
    let obsolete = pending_semantic_candidate(&fixture);
    let obsolete_source_pins = selected_source_pins(&fixture);
    let populated_comment_pin = selected_comment_pin(&fixture, 1);
    assert_ne!(initial_empty_comment_pin, populated_comment_pin);
    assert!(
        initial_source_pins
            .iter()
            .zip(&obsolete_source_pins)
            .all(|((_, old), (_, new))| old != new),
        "changed generation does not relabel old produced rows"
    );
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
    let repaired_source_pins = selected_source_pins(&fixture);
    let repaired_empty_comment_pin = selected_comment_pin(&fixture, 0);
    assert_ne!(populated_comment_pin, repaired_empty_comment_pin);
    assert!(
        obsolete_source_pins
            .iter()
            .zip(&repaired_source_pins)
            .all(|((_, old), (_, new))| old != new)
    );
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
    assert_eq!(
        repaired_empty_comment_pin,
        selected_comment_pin(&fixture, 0)
    );
    assert_eq!(
        repaired_source_pins,
        selected_source_pins(&fixture),
        "obsolete prepared reuse cannot replace repaired selection"
    );

    // Stop with the source stage durably selected and no terminal semantic successor.
    fs::write(
        workspace.join("sample.py"),
        b"def after_restart():\n    return 4\n",
    )
    .unwrap();
    let before_restart = pending_semantic_candidate(&fixture);
    let (before, _) = source_current_query(&fixture, &stack, "before-restart", request.clone());
    let restart_source_pins = selected_source_pins(&fixture);
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
    assert_eq!(
        repaired_empty_comment_pin,
        selected_comment_pin(&fixture, 0)
    );
    assert_eq!(
        restart_source_pins,
        selected_source_pins(&fixture),
        "exact source identity survives pending-stage restart and semantic reuse"
    );
    supervisor.stop();
}
