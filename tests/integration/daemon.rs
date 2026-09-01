use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use codefabric::fabric::command::{LeaseId, WorkspaceId};
use codefabric::fabric::writer_generation_sqlite::SqliteWriterGenerationStore;
use codefabric::fabric::writer_lease::WorkspaceWriterLease;
use codefabric::operational_store::OperationalStore;
use codefabric::workspace_registry::{WorkspaceRegistry, WorkspaceSourceRegistration};

#[test]
#[allow(clippy::too_many_lines)] // One process test proves production startup and joined restart.
fn wp29_production_binary_bootstraps_without_legacy_admin_or_false_ready() {
    let root = tempfile::tempdir().expect("temporary daemon root");
    let state = root.path().join("state");
    let runtime = root.path().join("runtime");
    let config_root = root.path().join("config");
    fs::create_dir(&state).expect("state root");
    fs::set_permissions(&state, fs::Permissions::from_mode(0o700)).expect("private state root");
    fs::create_dir(&runtime).expect("runtime root");
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).expect("private runtime root");
    fs::create_dir(&config_root).expect("config root");
    fs::set_permissions(&config_root, fs::Permissions::from_mode(0o700))
        .expect("private config root");
    let workspace_root = root.path().join("workspace");
    fs::create_dir(&workspace_root).expect("workspace root");
    fs::set_permissions(&workspace_root, fs::Permissions::from_mode(0o700))
        .expect("private workspace root");
    let mut operational =
        OperationalStore::open(&state.join("operational.sqlite3")).expect("operational store");
    let registered = WorkspaceRegistry::new(&mut operational)
        .add(&workspace_root, WorkspaceSourceRegistration::Directory)
        .expect("explicit operational workspace");
    let second_workspace_root = root.path().join("workspace-second");
    fs::create_dir(&second_workspace_root).expect("second workspace root");
    fs::set_permissions(&second_workspace_root, fs::Permissions::from_mode(0o700))
        .expect("private second workspace root");
    let second_registered = WorkspaceRegistry::new(&mut operational)
        .add(
            &second_workspace_root,
            WorkspaceSourceRegistration::Directory,
        )
        .expect("second explicit operational workspace");
    drop(operational);

    let config_path = config_root.join("codefabric.toml");
    let capability_path = config_root.join("query.capability");
    fs::write(&capability_path, b"integration-query-capability-token")
        .expect("query capability token");
    fs::set_permissions(&capability_path, fs::Permissions::from_mode(0o600))
        .expect("private query capability token");
    let config = format!(
        r#"
[static_config]
state_root = {state:?}
runtime_root = {runtime:?}
config_root = {config_root:?}
socket_endpoint = {socket:?}
query_socket_endpoint = {query_socket:?}
query_capability_token_file = "query.capability"
operational_database = "operational.sqlite3"
sandbox_policy = "required-for-untrusted"
hard_limit_profile = "daemon-default-v1"
supported_platform_profile = "local-workstation-v1"

[reloadable]
log_level = "info"
telemetry_sampling = 0.1
soft_query_quota = 4
maintenance_schedule = "daily-idle"
"#,
        state = state.display().to_string(),
        runtime = runtime.display().to_string(),
        config_root = config_root.display().to_string(),
        socket = runtime.join("admin.sock").display().to_string(),
        query_socket = runtime.join("query.sock").display().to_string(),
    );
    fs::write(&config_path, config).expect("configuration");
    fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600))
        .expect("private config file");

    assert!(
        Command::new(env!("CARGO_BIN_EXE_codefabricd"))
            .args(["check-config", "--config"])
            .arg(&config_path)
            .status()
            .expect("check-config command")
            .success()
    );

    let discovery = runtime.join("daemon.json");
    let admin_socket = runtime.join("admin.sock");
    fs::write(&admin_socket, b"foreign-admin-owner").expect("occupied admin endpoint");
    fs::set_permissions(&admin_socket, fs::Permissions::from_mode(0o600))
        .expect("private occupied endpoint");
    let mut rejected_start = Command::new(env!("CARGO_BIN_EXE_codefabricd"))
        .args(["serve", "--config"])
        .arg(&config_path)
        .spawn()
        .expect("spawn rejected daemon start");
    let rejected_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = rejected_start.try_wait().expect("rejected daemon wait") {
            assert!(!status.success());
            break;
        }
        if Instant::now() >= rejected_deadline {
            rejected_start
                .kill()
                .expect("kill timed-out rejected daemon");
            panic!("daemon did not fail closed after occupied admin bind");
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::read(&admin_socket).expect("foreign endpoint retained"),
        b"foreign-admin-owner"
    );
    assert!(!discovery.exists());
    fs::remove_file(&admin_socket).expect("remove controlled occupied endpoint");

    let writer_root = state.join("writer-authority");
    fs::create_dir_all(&writer_root).expect("writer authority root");
    fs::set_permissions(&writer_root, fs::Permissions::from_mode(0o700))
        .expect("private writer authority root");
    let generations = SqliteWriterGenerationStore::open(&state.join("writer-generations.sqlite"))
        .expect("writer generation store");
    let mut ordered_records = [registered.clone(), second_registered.clone()];
    ordered_records.sort_by_key(|record| record.workspace_id);
    let held_later_writer = WorkspaceWriterLease::acquire(
        &writer_root,
        WorkspaceId::from_bytes(ordered_records[1].workspace_id),
        LeaseId::from_bytes([9; 16]),
        &generations,
    )
    .expect("hold later workspace writer");
    let mut partial_start = Command::new(env!("CARGO_BIN_EXE_codefabricd"))
        .args(["serve", "--config"])
        .arg(&config_path)
        .spawn()
        .expect("spawn partial writer daemon start");
    let partial_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = partial_start.try_wait().expect("partial daemon wait") {
            assert!(!status.success());
            break;
        }
        if Instant::now() >= partial_deadline {
            partial_start.kill().expect("kill timed-out partial daemon");
            panic!("daemon did not fail closed after partial writer acquisition");
        }
        thread::sleep(Duration::from_millis(10));
    }
    held_later_writer
        .release()
        .expect("release controlled writer");
    generations.close().expect("close generation store");
    assert!(!discovery.exists());
    assert!(!admin_socket.exists());

    for shutdown_command in ["drain", "stop"] {
        let mut daemon = Command::new(env!("CARGO_BIN_EXE_codefabricd"))
            .args(["serve", "--config"])
            .arg(&config_path)
            .spawn()
            .expect("spawn daemon");
        let startup_deadline = Instant::now() + Duration::from_secs(5);
        while !discovery.is_file() && Instant::now() < startup_deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(discovery.is_file(), "daemon did not publish discovery");
        assert!(
            !runtime.join("query.sock").exists(),
            "an unconstructed semantic query service must not bind"
        );

        let discovery_value: serde_json::Value =
            serde_json::from_slice(&fs::read(&discovery).expect("read discovery"))
                .expect("discovery JSON");
        assert_eq!(
            discovery_value["public_bundle_versions"]["codefabric.authoritative-suite"],
            "codefabric-relational-data-fabric@2.2.0"
        );
        assert_eq!(discovery_value["basic_readiness"], false);

        let status = Command::new(env!("CARGO_BIN_EXE_codefabric"))
            .args(["daemon", "status", "--discovery"])
            .arg(&discovery)
            .output()
            .expect("daemon status");
        assert!(status.status.success());
        let status: serde_json::Value =
            serde_json::from_slice(&status.stdout).expect("status JSON");
        assert_eq!(status["daemon_liveness"], "LIVE");
        assert_eq!(
            status["workspace_readiness"],
            "ENDPOINTS_BOUND_BOOTSTRAPPING"
        );
        assert_eq!(status["error_code"], "SEMANTIC_AUTHORITY_BOOTSTRAPPING");
        let workspace_ids = status["workspaces"]
            .as_array()
            .expect("workspace array")
            .iter()
            .map(|record| record["workspace_id"].as_str().expect("workspace id"))
            .collect::<Vec<_>>();
        let registered_id = registered.public_id();
        let second_registered_id = second_registered.public_id();
        assert!(workspace_ids.contains(&registered_id.as_str()));
        assert!(workspace_ids.contains(&second_registered_id.as_str()));

        let rejected = Command::new(env!("CARGO_BIN_EXE_codefabric"))
            .args(["workspace", "list", "--discovery"])
            .arg(&discovery)
            .output()
            .expect("rejected legacy workspace command");
        assert!(!rejected.status.success());
        assert_eq!(
            String::from_utf8_lossy(&rejected.stderr).trim(),
            "COMMAND_RECOVERY_REQUIRED"
        );

        assert!(
            Command::new(env!("CARGO_BIN_EXE_codefabric"))
                .args(["daemon", shutdown_command, "--discovery"])
                .arg(&discovery)
                .status()
                .expect("daemon shutdown")
                .success()
        );
        let shutdown_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = daemon.try_wait().expect("daemon wait") {
                assert!(status.success());
                break;
            }
            if Instant::now() >= shutdown_deadline {
                daemon.kill().expect("kill timed-out daemon");
                panic!("daemon did not complete joined shutdown");
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!discovery.exists());
        assert!(!runtime.join("admin.sock").exists());
    }
}
