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
    // Mutate only this fixture's installed copies, regardless of the parent test command's paths.
    let child = Command::new(&stack.codefabric)
        .args(["supervisor", "serve", "--config"])
        .arg(&fixture.config_path)
        .env_remove("CODEFABRIC_PYREFLY_SIDECAR_BIN")
        .env_remove("CODEFABRIC_RUSTC_EXTRACTOR_BIN")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut supervisor = RunningSupervisor {
        child,
        discovery: fixture.supervisor_discovery(),
        codefabric: stack.codefabric.clone(),
    };
    supervisor.wait_ready();
    supervisor
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
