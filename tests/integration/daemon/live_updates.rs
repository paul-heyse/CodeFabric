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

#[test]
fn live_python_context_and_negative_imports_equal_independent_clean_queries() {
    let fixture = ProductionFixture::with_source(b"import sys\nfrom dependency import imported\ndef legacy():\n    return 1\ndef current():\n    return 2\nif sys.version_info >= (3, 14) and sys.platform == 'linux':\n    selected = current\nelse:\n    selected = legacy\ndef caller():\n    return selected() + imported()\n");
    let root = Path::new(&fixture.workspace.root_path_display);
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
    for (phase, version, platform, present, selected) in [
        ("import-created", "3.14", "linux", true, Some("current")),
        ("older-python", "3.12", "linux", true, Some("legacy")),
        ("other-platform", "3.14", "win32", true, Some("legacy")),
        ("unsupported-setting", "3.14", "linux", true, None),
        ("import-deleted", "3.14", "linux", false, Some("current")),
    ] {
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
        } else {
            fs::remove_file(root.join("dependency.py")).unwrap();
        }
        let live = python_context_calls(&fixture, &stack, phase, selected, present);
        let clean = clean_fixture(&fixture, &registration, &stack);
        let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
        let expected =
            python_context_calls(&clean, &stack, &format!("clean-{phase}"), selected, present);
        assert_eq!(
            live, expected,
            "{phase}: context/dependency replacement must equal clean semantics"
        );
        if !present {
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
    observed.insert(0, entities);
    observed
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
    four_forms(&fixture, &stack, "rust-raw-initial", &expected);
    fs::write(
        root.join(std::ffi::OsStr::from_bytes(b"dir-\xff/marker.py")),
        b"marker = 2\n",
    )
    .unwrap();
    let live = four_forms(&fixture, &stack, "rust-raw-edited", &expected);
    for row in &live[3].rows {
        if row["language"] == "rust" {
            assert_eq!(row["relative_path"], "7372632f636166c3a92e7273");
        }
    }
    let clean = clean_fixture(&fixture, &registration, &stack);
    let clean_supervisor = clean.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        live,
        four_forms(&clean, &stack, "rust-raw-clean", &expected)
    );
    clean_supervisor.stop();
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        live,
        four_forms(&fixture, &stack, "rust-raw-reopened", &expected)
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
