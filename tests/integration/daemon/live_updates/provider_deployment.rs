//! A real provider redeployment must converge without touching source or cache authority.
use super::*;
use arrow::array::{BinaryArray, BooleanArray};

struct InputState {
    generation: u64,
    inventory: Vec<u8>,
    deployment: Vec<u8>,
    pending: bool,
}

fn input_state(selected: &PersistedActivationControlRow) -> InputState {
    let batches = selected_relation_batches(selected, "source.input_inventory_state");
    let batch = batches.iter().find(|batch| batch.num_rows() == 1).unwrap();
    let digest = |name| {
        batch
            .column_by_name(name)
            .unwrap()
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap()
            .value(0)
            .to_vec()
    };
    InputState {
        // This independent reader sees Delta storage types, before contract restoration.
        generation: selected.row().pins.source_generation.get(),
        inventory: digest("inventory_digest"),
        deployment: digest("provider_deployment_digest"),
        pending: batch
            .column_by_name("semantic_pending")
            .unwrap()
            .as_any()
            .downcast_ref::<BooleanArray>()
            .unwrap()
            .value(0),
    }
}

fn selected_inputs(fixture: &ProductionFixture) -> InputState {
    input_state(
        &all_activation_control_rows(fixture)
            .into_iter()
            .max_by_key(|row| row.row().ordinal.get())
            .unwrap(),
    )
}

fn start_private_providers(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
) -> RunningSupervisor {
    start_private_deployment(fixture, stack, None)
}

fn start_private_deployment(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    rustup_home: Option<&Path>,
) -> RunningSupervisor {
    // Mutate only this fixture's installed copies, regardless of the parent test command's paths.
    let mut command = Command::new(&stack.codefabric);
    command
        .args(["supervisor", "serve", "--config"])
        .arg(&fixture.config_path)
        .env_remove("CODEFABRIC_PYREFLY_SIDECAR_BIN")
        .env_remove("CODEFABRIC_RUSTC_EXTRACTOR_BIN")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    if let Some(root) = rustup_home {
        command.env("RUSTUP_HOME", root);
    }
    let child = command.spawn().unwrap();
    let mut supervisor = RunningSupervisor {
        child,
        discovery: fixture.supervisor_discovery(),
        codefabric: stack.codefabric.clone(),
        preparation_root: fixture.fabric_workspace_root(),
    };
    supervisor.wait_ready();
    supervisor
}

fn copy_toolchain_tree(source: &Path, destination: &Path) {
    // Copy bytes rather than linking shared installed files: even link-count changes would alter
    // the real deployment's ctime. Mutations below affect only this fixture's toolchain tree.
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if fs::metadata(entry.path()).unwrap().is_dir() {
            copy_toolchain_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn assert_rust_context_replaced(before: &[SemanticObservation], after: &[SemanticObservation]) {
    let old = &before[0];
    let new = &after[0];
    assert_eq!(old.schema, new.schema);
    assert_eq!(old.processing, new.processing);
    assert_eq!(old.rows.len(), new.rows.len());
    for left in &old.rows {
        let right = new
            .rows
            .iter()
            .find(|row| row["name"] == left["name"])
            .unwrap();
        if left["language"] == "rust" {
            assert_ne!(
                left["context_id"], right["context_id"],
                "changed captured sysroot selects a new context"
            );
            for key in [
                "entity_kind",
                "name",
                "language",
                "file_id",
                "qualified_name",
                "relative_path",
                "start_byte",
                "end_byte",
            ] {
                assert_eq!(left[key], right[key], "unchanged declaration {key}");
            }
        } else {
            assert_eq!(left, right, "unaffected Python context and declaration");
        }
    }
    // four_forms separately asserts exact names, current-ID call endpoints, complete coverage and
    // independently expected source spans for each state; context-dependent IDs may change.
}

#[test]
fn selected_sysroot_changes_publish_without_source_edits_and_survive_restart() {
    let fixture = ProductionFixture::with_source_and_activation_startup_fault(
        b"def py_leaf():\n    return 1\ndef py_caller():\n    return py_leaf()\n",
        Some("hold_semantic_update_publication"),
    );
    let root = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("src/lib.rs"),
        b"pub fn rust_leaf() -> u32 { 1 }\npub fn rust_caller() -> u32 { rust_leaf() }\n",
    )
    .unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = 'fixture'\nversion = '0.1.0'\nedition = '2024'\n[lib]\ntest = false\ndoctest = false\n").unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = 'fixture'\nversion = '0.1.0'\n",
    )
    .unwrap();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    WorkspaceRegistry::new(
        &mut OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap(),
    )
    .set_source_disclosure(fixture.workspace.workspace_id, true)
    .unwrap();
    let identity: Value = serde_json::from_slice(include_bytes!(
        "../../../../rustc-extractor/toolchain-identity.json"
    ))
    .unwrap();
    let selected = Command::new("rustup")
        .args([
            "which",
            "--toolchain",
            identity["toolchain"].as_str().unwrap(),
            "rustc",
        ])
        .output()
        .unwrap();
    assert!(selected.status.success());
    let installed = PathBuf::from(String::from_utf8(selected.stdout).unwrap().trim());
    let installed = installed.parent().unwrap().parent().unwrap();
    let rustup_home = fixture.root().join("private-rustup");
    let sysroot = rustup_home
        .join("toolchains")
        .join(installed.file_name().unwrap());
    fs::create_dir_all(sysroot.join("bin")).unwrap();
    for tool in ["cargo", "rustc"] {
        fs::copy(
            installed.join("bin").join(tool),
            sysroot.join("bin").join(tool),
        )
        .unwrap();
    }
    copy_toolchain_tree(&installed.join("lib"), &sysroot.join("lib"));
    let names = [
        "py_leaf",
        "py_caller",
        "fixture::rust_leaf",
        "fixture::rust_caller",
    ];
    let supervisor = start_private_deployment(&fixture, &stack, Some(&rustup_home));
    let ready = pending_after(&fixture, 0);
    fs::write(ready.with_extension("resume"), b"resume").unwrap();
    wait_for_semantic_activation(&fixture);
    let expected = four_forms(&fixture, &stack, "sysroot-initial", &names);
    let first = selected_inputs(&fixture);
    let marker = sysroot.join("lib/codefabric-fixture-input");
    fs::write(&marker, b"first tool input").unwrap();
    // No source edit or public query initiates this replacement. A new sysroot member alone
    // must invalidate the selected baseline while the old immutable capture remains owned.
    let ready = pending_after(&fixture, first.generation);
    let second = selected_inputs(&fixture);
    assert_eq!(first.inventory, second.inventory);
    assert_ne!(first.deployment, second.deployment);
    assert!(second.pending);
    fs::write(ready.with_extension("resume"), b"resume").unwrap();
    wait_for_semantic_activation(&fixture);
    let live = four_forms(&fixture, &stack, "sysroot-live", &names);
    assert_rust_context_replaced(&expected, &live);
    assert_eq!(rust_toolchain_costs(&fixture)["captures"], 2);
    supervisor.stop();

    // Restart has no retained toolchain cache. The selected Delta input witness must still find
    // same-length, mtime-preserving changes and prevent stale current semantic answers.
    let modified = fs::metadata(&marker).unwrap().modified().unwrap();
    fs::write(&marker, b"other tool input").unwrap();
    fs::File::open(&marker)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    let supervisor = start_private_deployment(&fixture, &stack, Some(&rustup_home));
    let ready = pending_after(&fixture, second.generation);
    let third = selected_inputs(&fixture);
    assert_eq!(first.inventory, third.inventory);
    assert_ne!(second.deployment, third.deployment);
    fs::write(ready.with_extension("resume"), b"resume").unwrap();
    wait_for_semantic_activation(&fixture);
    let reopened = four_forms(&fixture, &stack, "sysroot-replaced-reopen", &names);
    assert_rust_context_replaced(&live, &reopened);
    supervisor.stop();

    let supervisor = start_private_deployment(&fixture, &stack, Some(&rustup_home));
    assert_eq!(
        reopened,
        four_forms(&fixture, &stack, "sysroot-unchanged-reopen", &names)
    );
    assert_eq!(selected_inputs(&fixture).generation, third.generation);
    supervisor.stop();
}

fn replace_provider(stack: &InstalledProductionStack) {
    let path = stack
        .codefabric
        .parent()
        .unwrap()
        .join("codefabric-pyrefly-sidecar");
    let replacement = path.with_extension("replacement");
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    fs::copy(&path, &replacement).unwrap();
    fs::File::open(&replacement)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    fs::rename(replacement, path).unwrap();
}

fn pending_after(fixture: &ProductionFixture, generation: u64) -> PathBuf {
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        for entry in fs::read_dir(&fixture.state).unwrap().flatten() {
            let path = entry.path();
            if path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("semantic-update-")
                && path.extension().is_some_and(|value| value == "ready")
                && fs::read(&path)
                    .ok()
                    .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
                    .is_some_and(|bytes| u64::from_be_bytes(bytes) > generation)
            {
                return path;
            }
        }
        assert!(
            Instant::now() < deadline,
            "provider redeployment did not create a successor"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn selected_provider_redeployment_fences_delayed_facts_and_reconciles_reopen() {
    let source = b"def py_leaf():\n    return 1\ndef py_caller():\n    return py_leaf()\n";
    let fixture = ProductionFixture::with_source_and_activation_startup_fault(
        source,
        Some("hold_semantic_update_publication"),
    );
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    WorkspaceRegistry::new(
        &mut OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap(),
    )
    .set_source_disclosure(fixture.workspace.workspace_id, true)
    .unwrap();
    let supervisor = start_private_providers(&fixture, &stack);
    let initial_ready = pending_after(&fixture, 0);
    fs::write(initial_ready.with_extension("resume"), b"resume").unwrap();
    wait_for_semantic_activation(&fixture);
    let names = ["py_leaf", "py_caller"];
    let initial = four_forms(&fixture, &stack, "deployment-initial", &names);
    let first = selected_inputs(&fixture);

    replace_provider(&stack);
    // No query, source write or cache eviction requests this background successor.
    let obsolete_ready = pending_after(&fixture, first.generation);
    let obsolete = selected_inputs(&fixture);
    assert!(obsolete.pending);
    assert_eq!(obsolete.inventory, first.inventory);
    assert_ne!(obsolete.deployment, first.deployment);
    replace_provider(&stack);
    fs::write(obsolete_ready.with_extension("resume"), b"resume").unwrap();
    let repaired_ready = pending_after(&fixture, obsolete.generation);
    let repaired = selected_inputs(&fixture);
    assert!(repaired.pending);
    assert_eq!(repaired.inventory, first.inventory);
    assert_ne!(repaired.deployment, obsolete.deployment);
    assert!(
        all_activation_control_rows(&fixture).iter().all(|row| {
            let state = input_state(row);
            state.generation != obsolete.generation || state.pending
        }),
        "an obsolete deployment's completed provider result must not activate"
    );
    fs::write(repaired_ready.with_extension("resume"), b"resume").unwrap();
    wait_for_semantic_activation(&fixture);
    assert_eq!(
        initial,
        four_forms(&fixture, &stack, "deployment-repaired", &names)
    );
    supervisor.stop();

    replace_provider(&stack);
    let supervisor = start_private_providers(&fixture, &stack);
    let reopened_ready = pending_after(&fixture, repaired.generation);
    let reopened = selected_inputs(&fixture);
    assert_eq!(reopened.inventory, first.inventory);
    assert_ne!(reopened.deployment, repaired.deployment);
    fs::write(reopened_ready.with_extension("resume"), b"resume").unwrap();
    wait_for_semantic_activation(&fixture);
    assert_eq!(
        initial,
        four_forms(&fixture, &stack, "deployment-reopened", &names)
    );
    supervisor.stop();

    let supervisor = start_private_providers(&fixture, &stack);
    assert_eq!(
        initial,
        four_forms(&fixture, &stack, "deployment-unchanged", &names)
    );
    assert_eq!(selected_inputs(&fixture).generation, reopened.generation);
    assert_eq!(
        fs::read(Path::new(&fixture.workspace.root_path_display).join("sample.py")).unwrap(),
        source
    );
    supervisor.stop();
}
