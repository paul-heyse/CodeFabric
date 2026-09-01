use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

fn run_admin(discovery: &std::path::Path, arguments: &[String]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_codefabric"))
        .args(arguments)
        .arg("--discovery")
        .arg(discovery)
        .output()
        .expect("administrative CLI");
    assert!(
        output.status.success(),
        "admin CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("administrative response JSON")
}

#[test]
#[allow(clippy::too_many_lines)] // One process test proves the complete ordered admin CLI lifecycle.
fn wp12_wp14_cli_end_to_end() {
    let root = tempfile::tempdir().expect("temporary daemon root");
    let state = root.path().join("state");
    let runtime = root.path().join("runtime");
    let config_root = root.path().join("config");
    fs::create_dir(&config_root).expect("config root");
    fs::set_permissions(&config_root, fs::Permissions::from_mode(0o700))
        .expect("private config root");
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

    let mut daemon = Command::new(env!("CARGO_BIN_EXE_codefabricd"))
        .args(["serve", "--config"])
        .arg(&config_path)
        .spawn()
        .expect("spawn daemon");
    let discovery = runtime.join("daemon.json");
    let startup_deadline = Instant::now() + Duration::from_secs(5);
    while !discovery.is_file() && Instant::now() < startup_deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(discovery.is_file(), "daemon did not publish discovery");

    let status = Command::new(env!("CARGO_BIN_EXE_codefabric"))
        .args(["daemon", "status", "--discovery"])
        .arg(&discovery)
        .output()
        .expect("daemon status");
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    assert_eq!(status["daemon_liveness"], "LIVE");
    assert_eq!(status["workspace_readiness"], "NO_WORKSPACES_READY");

    let workspace_root = root.path().join("workspace");
    let moved_root = root.path().join("moved-workspace");
    let profile = root.path().join("profile.json");
    fs::create_dir(&workspace_root).expect("workspace root");
    fs::create_dir(&moved_root).expect("moved workspace root");
    fs::write(&profile, br#"{"profile":"default"}"#).expect("profile manifest");

    let added = run_admin(
        &discovery,
        &[
            "workspace".into(),
            "add".into(),
            workspace_root.display().to_string(),
        ],
    );
    let workspace_id = added["workspaces"][0]["workspace_id"]
        .as_str()
        .expect("public workspace ID")
        .to_owned();
    assert!(workspace_id.starts_with("workspace:"));
    assert_eq!(added["workspaces"][0]["status"], 20);

    let shown = run_admin(
        &discovery,
        &["workspace".into(), "show".into(), workspace_id.clone()],
    );
    assert_eq!(shown["workspaces"][0]["workspace_id"], workspace_id);
    let configured = run_admin(
        &discovery,
        &[
            "workspace".into(),
            "configure".into(),
            workspace_id.clone(),
            "--profile".into(),
            profile.display().to_string(),
        ],
    );
    assert_eq!(configured["workspaces"][0]["registration_revision"], 2);
    let relinked = run_admin(
        &discovery,
        &[
            "workspace".into(),
            "relink".into(),
            workspace_id.clone(),
            moved_root.display().to_string(),
            "--acknowledge-non-git".into(),
            "--inventory-matches".into(),
        ],
    );
    assert_eq!(relinked["workspaces"][0]["registration_revision"], 3);
    let enabled = run_admin(
        &discovery,
        &["workspace".into(), "enable".into(), workspace_id.clone()],
    );
    assert_eq!(enabled["workspaces"][0]["status"], 40);
    let reconciled = run_admin(
        &discovery,
        &["workspace".into(), "reconcile".into(), workspace_id.clone()],
    );
    assert_eq!(reconciled["workspaces"][0]["status"], 40);
    let disabled = run_admin(
        &discovery,
        &["workspace".into(), "disable".into(), workspace_id.clone()],
    );
    assert_eq!(disabled["workspaces"][0]["status"], 20);
    let removed = run_admin(
        &discovery,
        &[
            "workspace".into(),
            "remove".into(),
            workspace_id,
            "--retain-data".into(),
        ],
    );
    assert_eq!(removed["workspaces"][0]["status"], 90);
    assert_eq!(removed["workspace_readiness"], "NO_WORKSPACES_READY");
    let listed = run_admin(&discovery, &["workspace".into(), "list".into()]);
    assert_eq!(listed["workspaces"], serde_json::json!([]));

    assert!(
        Command::new(env!("CARGO_BIN_EXE_codefabric"))
            .args(["daemon", "stop", "--discovery"])
            .arg(&discovery)
            .status()
            .expect("daemon stop")
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
}
