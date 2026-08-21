use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn wp12_cli_end_to_end() {
    let root = tempfile::tempdir().expect("temporary daemon root");
    let state = root.path().join("state");
    let runtime = root.path().join("runtime");
    let config_root = root.path().join("config");
    fs::create_dir(&config_root).expect("config root");
    fs::set_permissions(&config_root, fs::Permissions::from_mode(0o700))
        .expect("private config root");
    let config_path = config_root.join("codefabric.toml");
    let config = format!(
        r#"
[static_config]
state_root = {state:?}
runtime_root = {runtime:?}
config_root = {config_root:?}
socket_endpoint = {socket:?}
operational_database = "operational.sqlite3"
bundle_index = "contracts/generated/artifact-index.json"
toolchain_identity = "contracts/toolchain/toolchain-identity.json"
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
