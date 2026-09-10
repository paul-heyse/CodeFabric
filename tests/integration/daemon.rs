use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use arrow::record_batch::RecordBatch;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use codefabric::fabric::activation_control_delta::{
    ActivationControlRowCodec, PersistedActivationControlRow,
};
use codefabric::fabric::command::ExpectedHead;
use codefabric::operational_store::OperationalStore;
use codefabric::session_authority::{
    LaunchPolicyId, LaunchPolicyRevision, RevocationGeneration, SessionOperation,
};
use codefabric::supervisor::{AgentLaunchPolicy, SupervisorDiscovery};
use codefabric::workspace_registry::{
    WorkspaceRecord, WorkspaceRegistry, WorkspaceSourceRegistration,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde_json::{Value, json};

const PROCESS_DEADLINE: Duration = Duration::from_secs(180);
const DEFAULT_PRODUCTION_SOURCE: &[u8] = b"def answer(value: int) -> int:\n    return value + 1\n";

#[cfg(target_os = "linux")]
mod semantic_references;
mod types;

struct InstalledProductionStack {
    _root: tempfile::TempDir,
    codefabric: PathBuf,
    python: PathBuf,
}

impl InstalledProductionStack {
    fn build() -> Self {
        let root = tempfile::tempdir().expect("temporary installed-wheel root");
        let distribution_root = private_directory(&root.path().join("dist"));
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let build = Command::new("uv")
            .args(["build", "--project"])
            .arg(repository.join("codefabric-cpg-mcp"))
            .args(["--wheel", "--out-dir"])
            .arg(&distribution_root)
            .output()
            .expect("build adapter wheel");
        assert!(
            build.status.success(),
            "adapter wheel build failed: {}",
            String::from_utf8_lossy(&build.stderr)
        );
        let wheels = fs::read_dir(&distribution_root)
            .expect("adapter wheel distribution root")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "whl"))
            .collect::<Vec<_>>();
        assert_eq!(wheels.len(), 1, "one freshly built adapter wheel");

        let venv = root.path().join("venv");
        let created = Command::new("uv")
            .args(["venv", "--python", "3.14"])
            .arg(&venv)
            .output()
            .expect("create isolated adapter venv");
        assert!(
            created.status.success(),
            "adapter venv creation failed: {}",
            String::from_utf8_lossy(&created.stderr)
        );
        let python = venv.join("bin/python");
        let installed = Command::new("uv")
            .args(["pip", "install", "--python"])
            .arg(&python)
            .arg(&wheels[0])
            .output()
            .expect("install adapter wheel");
        assert!(
            installed.status.success(),
            "adapter wheel installation failed: {}",
            String::from_utf8_lossy(&installed.stderr)
        );

        let provenance = Command::new(&python)
            .args([
                "-I",
                "-c",
                r"import json, pathlib, sys
from importlib.metadata import distribution, version
import codefabric_cpg_mcp
root = pathlib.Path(sys.argv[1]).resolve()
module = pathlib.Path(codefabric_cpg_mcp.__file__).resolve()
assert module.is_relative_to(root)
assert version('codefabric-cpg-mcp') == '0.1.0'
origin = distribution('codefabric-cpg-mcp').read_text('direct_url.json')
if origin:
    document = json.loads(origin)
    assert not document.get('dir_info', {}).get('editable', False)
print(module)",
            ])
            .arg(&venv)
            .output()
            .expect("inspect installed adapter provenance");
        assert!(
            provenance.status.success(),
            "installed-wheel provenance failed: {}",
            String::from_utf8_lossy(&provenance.stderr)
        );
        assert!(!provenance.stdout.is_empty());

        let executable_root = private_directory(&root.path().join("bin"));
        let codefabric = executable_root.join("codefabric");
        install_executable(Path::new(env!("CARGO_BIN_EXE_codefabric")), &codefabric);
        install_executable(
            Path::new(env!("CARGO_BIN_EXE_codefabricd")),
            &executable_root.join("codefabricd"),
        );
        let sidecar = std::env::var_os("CODEFABRIC_PYREFLY_SIDECAR_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| repository.join("target/debug/codefabric-pyrefly-sidecar"));
        assert!(
            sidecar.is_file(),
            "build the Pyrefly sidecar before installed production tests"
        );
        install_executable(
            &sidecar,
            &executable_root.join("codefabric-pyrefly-sidecar"),
        );

        let extractor = std::env::var_os("CODEFABRIC_RUSTC_EXTRACTOR_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                repository.join("target/extractor/debug/codefabric-rustc-extractor")
            });
        if extractor.is_file() {
            install_executable(
                &extractor,
                &executable_root.join("codefabric-rustc-extractor"),
            );
        }

        Self {
            _root: root,
            codefabric,
            python,
        }
    }
}

struct ProductionFixture {
    _root: tempfile::TempDir,
    state: PathBuf,
    runtime: PathBuf,
    config_root: PathBuf,
    config_path: PathBuf,
    workspace: WorkspaceRecord,
}

impl ProductionFixture {
    fn new() -> Self {
        Self::with_source_and_activation_startup_fault(DEFAULT_PRODUCTION_SOURCE, None)
    }

    fn with_activation_startup_fault(fault: Option<&str>) -> Self {
        Self::with_source_and_activation_startup_fault(DEFAULT_PRODUCTION_SOURCE, fault)
    }

    fn with_source(source: &[u8]) -> Self {
        Self::with_source_and_activation_startup_fault(source, None)
    }

    fn with_source_and_activation_startup_fault(source: &[u8], fault: Option<&str>) -> Self {
        let root = tempfile::tempdir().expect("temporary production root");
        let state = private_directory(&root.path().join("state"));
        let runtime = private_directory(&root.path().join("runtime"));
        let config_root = private_directory(&root.path().join("config"));
        let workspace_root = private_directory(&root.path().join("workspace"));
        fs::write(workspace_root.join("sample.py"), source).expect("production Python source");

        let mut operational =
            OperationalStore::open(&state.join("operational.sqlite3")).expect("operational store");
        let workspace = WorkspaceRegistry::new(&mut operational)
            .add(&workspace_root, WorkspaceSourceRegistration::Directory)
            .expect("one explicit operational workspace");
        drop(operational);

        let config_path = config_root.join("codefabric.toml");
        let activation_fault = fault.map_or_else(String::new, |fault| {
            format!("activation_startup_assurance_fault = {fault:?}\n")
        });
        let config = format!(
            r#"
[static_config]
state_root = {state:?}
runtime_root = {runtime:?}
config_root = {config_root:?}
query_socket_endpoint = {query_socket:?}
operational_database = "operational.sqlite3"
sandbox_policy = "required-for-untrusted"
hard_limit_profile = "daemon-default-v1"
supported_platform_profile = "local-workstation-v1"
{activation_fault}

[reloadable]
log_level = "info"
telemetry_sampling = 0.1
soft_query_quota = 4
maintenance_schedule = "daily-idle"
"#,
            state = state.display().to_string(),
            runtime = runtime.display().to_string(),
            config_root = config_root.display().to_string(),
            query_socket = runtime.join("query.sock").display().to_string(),
            activation_fault = activation_fault,
        );
        write_private(&config_path, config.as_bytes());

        let operations = BTreeSet::from([
            SessionOperation::Status,
            SessionOperation::Reference,
            SessionOperation::Validate,
            SessionOperation::Start,
            SessionOperation::Watch,
            SessionOperation::Cancel,
            SessionOperation::ReadResource,
            SessionOperation::ReleaseResource,
        ]);
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let adapter_program = repository.join("codefabric-cpg-mcp/.venv/bin/python");
        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("test clock")
                .as_millis(),
        )
        .expect("test clock range");
        let policy = AgentLaunchPolicy {
            format: "codefabric.agent-launch-policy.v1".to_owned(),
            policy_id: LaunchPolicyId::try_new("policy-one").expect("policy id"),
            policy_revision: LaunchPolicyRevision::new(1).expect("policy revision"),
            revocation_generation: RevocationGeneration::new(1).expect("revocation generation"),
            issued_at_unix_ms: now - 1_000,
            not_before_unix_ms: now - 1_000,
            expires_at_unix_ms: now + 3_600_000,
            adapter_program: adapter_program.clone(),
            adapter_arguments: vec!["-m".to_owned(), "codefabric_cpg_mcp".to_owned()],
            adapter_distribution: "codefabric-cpg-mcp".to_owned(),
            adapter_distribution_version: "0.1.0".to_owned(),
            adapter_executable_digest: format!(
                "b3:{}",
                blake3::hash(&fs::read(&adapter_program).expect("adapter executable bytes"))
                    .to_hex()
            ),
            principal_id: format!("owner:{}", "11".repeat(16)),
            workspace_ids: vec![workspace.public_id()],
            operations,
            semantic_profiles: BTreeSet::from(["codefabric.semantic-query.v2".to_owned()]),
            maximum_resource_chunk_bytes: 1024 * 1024,
            maximum_result_bytes: 64 * 1024 * 1024,
            maximum_result_pages: 1_024,
            maximum_request_state_ttl_seconds: 30,
            maximum_session_seconds: 600,
            maximum_concurrent_launches: 4,
        };
        let policy_root = private_directory(&config_root.join("agent-launch-policies"));
        write_private(
            &policy_root.join("policy-one.json"),
            &serde_json::to_vec(&policy).expect("launch policy JSON"),
        );

        Self {
            _root: root,
            state,
            runtime,
            config_root,
            config_path,
            workspace,
        }
    }

    fn supervisor_discovery(&self) -> PathBuf {
        self.runtime.join("supervisor.json")
    }

    fn root(&self) -> &Path {
        self._root.path()
    }

    fn fabric_workspace_root(&self) -> PathBuf {
        self.state.join("fabric").join(
            self.workspace
                .workspace_id
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
        )
    }

    fn policy_path(&self) -> PathBuf {
        self.config_root
            .join("agent-launch-policies/policy-one.json")
    }

    fn launch_policy(&self) -> AgentLaunchPolicy {
        serde_json::from_slice(&fs::read(self.policy_path()).expect("launch policy bytes"))
            .expect("strict launch policy JSON")
    }

    fn write_launch_policy(&self, policy: &AgentLaunchPolicy) {
        write_private(
            &self.policy_path(),
            &serde_json::to_vec(policy).expect("launch policy JSON"),
        );
    }

    fn bind_installed_adapter(
        &self,
        stack: &InstalledProductionStack,
        policy_id: &str,
        principal_suffix: u8,
    ) -> AgentLaunchPolicy {
        let mut policy = self.launch_policy();
        policy.policy_id = LaunchPolicyId::try_new(policy_id).expect("named policy id");
        policy.adapter_program.clone_from(&stack.python);
        policy.adapter_arguments = vec![
            "-I".to_owned(),
            "-m".to_owned(),
            "codefabric_cpg_mcp".to_owned(),
        ];
        policy.adapter_executable_digest = format!(
            "b3:{}",
            blake3::hash(&fs::read(&stack.python).expect("installed adapter executable bytes"))
                .to_hex()
        );
        policy.principal_id = format!("owner:{}", format!("{principal_suffix:02x}").repeat(16));
        let policy_path = self
            .config_root
            .join("agent-launch-policies")
            .join(format!("{policy_id}.json"));
        write_private(
            &policy_path,
            &serde_json::to_vec(&policy).expect("installed adapter policy JSON"),
        );
        policy
    }

    fn start_supervisor(&self) -> RunningSupervisor {
        self.start_supervisor_with(Path::new(env!("CARGO_BIN_EXE_codefabric")))
    }

    fn start_supervisor_with(&self, codefabric: &Path) -> RunningSupervisor {
        let child = Command::new(codefabric)
            .args(["supervisor", "serve", "--config"])
            .arg(&self.config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn production supervisor");
        let mut running = RunningSupervisor {
            child,
            discovery: self.supervisor_discovery(),
            codefabric: codefabric.to_owned(),
        };
        running.wait_ready();
        running
    }
}

struct RunningSupervisor {
    child: Child,
    discovery: PathBuf,
    codefabric: PathBuf,
}

impl RunningSupervisor {
    fn wait_ready(&mut self) {
        let deadline = Instant::now() + PROCESS_DEADLINE;
        loop {
            if self.discovery.is_file() {
                let status = self.status();
                assert_eq!(status["accepted"], true);
                assert_eq!(status["code"], "READY");
                return;
            }
            if let Some(status) = self.child.try_wait().expect("supervisor wait") {
                panic!("supervisor exited before readiness: {status}");
            }
            assert!(
                Instant::now() < deadline,
                "supervisor readiness deadline exceeded"
            );
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn discovery(&self) -> SupervisorDiscovery {
        serde_json::from_slice(&fs::read(&self.discovery).expect("supervisor discovery"))
            .expect("strict supervisor discovery")
    }

    fn wait_for_daemon_generation(&mut self, minimum: u64) -> SupervisorDiscovery {
        let deadline = Instant::now() + PROCESS_DEADLINE;
        loop {
            if let Some(status) = self.child.try_wait().expect("supervisor wait") {
                panic!("supervisor exited during daemon restart: {status}");
            }
            if let Ok(bytes) = fs::read(&self.discovery)
                && let Ok(discovery) = serde_json::from_slice::<SupervisorDiscovery>(&bytes)
                && discovery.daemon_generation >= minimum
            {
                return discovery;
            }
            assert!(
                Instant::now() < deadline,
                "daemon generation did not advance before the restart deadline"
            );
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn status(&self) -> Value {
        let output = Command::new(&self.codefabric)
            .args(["supervisor", "status", "--discovery"])
            .arg(&self.discovery)
            .output()
            .expect("supervisor status");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("supervisor status JSON")
    }

    fn stop(mut self) {
        let pid = i32::try_from(self.child.id())
            .ok()
            .and_then(rustix::process::Pid::from_raw)
            .expect("live supervisor PID");
        rustix::process::kill_process(pid, rustix::process::Signal::TERM)
            .expect("signal supervisor shutdown");
        let deadline = Instant::now() + PROCESS_DEADLINE;
        loop {
            if let Some(status) = self.child.try_wait().expect("supervisor join") {
                assert!(status.success(), "supervisor joined with {status}");
                break;
            }
            assert!(
                Instant::now() < deadline,
                "supervisor joined-shutdown deadline exceeded"
            );
            thread::sleep(Duration::from_millis(25));
        }
        assert!(!self.discovery.exists());
    }
}

impl Drop for RunningSupervisor {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_some() {
            return;
        }
        if let Some(pid) = i32::try_from(self.child.id())
            .ok()
            .and_then(rustix::process::Pid::from_raw)
        {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct McpClientProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    next_id: u64,
}

impl McpClientProcess {
    fn launch_uninitialized(supervisor: &Path, policy_id: &str) -> Self {
        Self::launch_uninitialized_with(
            Path::new(env!("CARGO_BIN_EXE_codefabric")),
            supervisor,
            policy_id,
        )
    }

    fn launch_uninitialized_with(codefabric: &Path, supervisor: &Path, policy_id: &str) -> Self {
        let mut child = Command::new(codefabric)
            .args(["mcp", "serve", "--supervisor"])
            .arg(supervisor)
            .args(["--policy-id", policy_id])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn attach-only MCP launcher");
        let stdin = child.stdin.take().expect("launcher stdin");
        let stdout = BufReader::new(child.stdout.take().expect("launcher stdout"));
        Self {
            child,
            stdin: Some(stdin),
            stdout: Some(stdout),
            next_id: 1,
        }
    }

    fn launch(supervisor: &Path, policy_id: &str) -> Self {
        let mut process = Self::launch_uninitialized(supervisor, policy_id);
        let discovered = process.request(
            "server/discover",
            json!({
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientInfo": {
                        "name": "codefabric-wp44-probe",
                        "version": "1.0.0"
                    },
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }),
        );
        assert!(
            discovered.get("error").is_none(),
            "discovery failed: {discovered}"
        );
        process
    }

    fn request_exact(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let stdin = self.stdin.as_mut().expect("live launcher stdin");
        serde_json::to_writer(&mut *stdin, &request).expect("MCP request JSON");
        stdin.write_all(b"\n").expect("MCP request delimiter");
        stdin.flush().expect("MCP request flush");
        let stdout = self.stdout.as_mut().expect("live launcher stdout");
        loop {
            let mut line = String::new();
            let count = stdout.read_line(&mut line).expect("MCP response line");
            if count == 0 {
                let status = self.child.wait().expect("join failed MCP launcher");
                let mut diagnostic = String::new();
                if let Some(mut stderr) = self.child.stderr.take() {
                    stderr
                        .read_to_string(&mut diagnostic)
                        .expect("read MCP launcher diagnostic");
                }
                panic!(
                    "MCP process closed stdout before responding to {method}: {status}: {diagnostic}"
                );
            }
            let response: Value = serde_json::from_str(&line).expect("MCP response JSON");
            if response.get("id") == Some(&json!(id)) {
                return response;
            }
        }
    }

    fn request(&mut self, method: &str, mut params: Value) -> Value {
        params
            .as_object_mut()
            .expect("MCP request params object")
            .entry("_meta")
            .or_insert_with(|| {
                json!({
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientInfo": {
                        "name": "codefabric-wp44-probe",
                        "version": "1.0.0"
                    },
                    "io.modelcontextprotocol/clientCapabilities": {}
                })
            });
        self.request_exact(method, params)
    }

    fn close(mut self) -> Output {
        drop(self.stdin.take());
        drop(self.stdout.take());
        self.child.wait_with_output().expect("join MCP launcher")
    }

    #[cfg(target_os = "linux")]
    fn direct_adapter_pid(&self) -> u32 {
        let launcher_pid = self.child.id();
        let children =
            fs::read_to_string(format!("/proc/{launcher_pid}/task/{launcher_pid}/children"))
                .expect("launcher child process census");
        let children = children
            .split_whitespace()
            .map(|value| value.parse::<u32>().expect("numeric adapter PID"))
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 1, "exactly one installed adapter child");
        children[0]
    }

    #[cfg(target_os = "linux")]
    fn kill_abruptly(mut self) -> u32 {
        let adapter_pid = self.direct_adapter_pid();
        self.child.kill().expect("kill MCP launcher");
        self.child.wait().expect("join killed MCP launcher");
        drop(self.stdin.take());
        drop(self.stdout.take());
        adapter_pid
    }
}

fn launch_with_closed_input(supervisor: &Path, policy_id: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codefabric"))
        .args(["mcp", "serve", "--supervisor"])
        .arg(supervisor)
        .args(["--policy-id", policy_id])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("run attach-only launcher with terminal input")
}

fn private_directory(path: &Path) -> PathBuf {
    fs::create_dir(path).expect("private directory");
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("private permissions");
    path.to_owned()
}

fn write_private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("private file");
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("private file mode");
}

fn install_executable(source: &Path, destination: &Path) {
    fs::copy(source, destination).expect("install production executable");
    fs::set_permissions(destination, fs::Permissions::from_mode(0o700))
        .expect("installed executable mode");
    assert_eq!(
        blake3::hash(&fs::read(source).expect("source executable bytes")),
        blake3::hash(&fs::read(destination).expect("installed executable bytes")),
        "installed executable differs from the Cargo-built production artifact"
    );
}

fn modern_client_scenario(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    policy_id: &str,
    guard_responses: Value,
    steps: Value,
) -> Value {
    json!({
        "format": "codefabric.fastmcp4-modern-client-scenario.v1",
        "server": {
            "command": stack.codefabric,
            "args": [
                "mcp",
                "serve",
                "--supervisor",
                fixture.supervisor_discovery(),
                "--policy-id",
                policy_id,
            ],
            "cwd": fixture.root(),
        },
        "timeout_seconds": 180.0,
        "input_required_max_rounds": 3,
        "stderr_limit_bytes": 131_072,
        "report_limit_bytes": 16_777_216,
        "guard_responses": guard_responses,
        "require_all_guard_responses": true,
        "steps": steps,
    })
}

fn write_modern_client_scenario(
    fixture: &ProductionFixture,
    label: &str,
    scenario: &Value,
) -> PathBuf {
    let path = fixture.root().join(format!("modern-client-{label}.json"));
    write_private(
        &path,
        &serde_json::to_vec(scenario).expect("modern client scenario JSON"),
    );
    path
}

fn spawn_modern_client(stack: &InstalledProductionStack, scenario: &Path) -> Child {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Command::new(repository.join("scripts/run_fastmcp4_modern_client.sh"))
        .arg(&stack.python)
        .arg(scenario)
        .current_dir(scenario.parent().expect("scenario parent"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn installed-wheel modern FastMCP client")
}

fn run_modern_client(stack: &InstalledProductionStack, scenario: &Path) -> Output {
    spawn_modern_client(stack, scenario)
        .wait_with_output()
        .expect("join installed-wheel modern FastMCP client")
}

#[cfg(target_os = "linux")]
fn wait_for_process_child_with_cmdline(parent_pid: u32, expected: &[u8], label: &str) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(10);
    let children_path = format!("/proc/{parent_pid}/task/{parent_pid}/children");
    loop {
        if let Ok(children) = fs::read_to_string(&children_path) {
            for child in children
                .split_whitespace()
                .map(|value| value.parse::<u32>().expect("numeric child PID"))
            {
                if fs::read(format!("/proc/{child}/cmdline")).is_ok_and(|cmdline| {
                    cmdline
                        .windows(expected.len())
                        .any(|window| window == expected)
                }) {
                    return child;
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "{label} did not expose its expected direct child"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn modern_client_report(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "modern client failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "driver STDERR must remain empty");
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("closed modern client report");
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1,
        "driver STDOUT is exactly one JSON report frame"
    );
    assert_eq!(
        report["format"],
        "codefabric.fastmcp4-modern-client-report.v1"
    );
    assert_eq!(report["status"], "ok", "{report}");
    assert!(report["error_code"].is_null(), "{report}");
    assert_eq!(report["protocol_mode"], "2026-07-28");
    assert_eq!(report["stack"]["codefabric-cpg-mcp"], "0.1.0");
    assert_eq!(report["stack"]["fastmcp"], "4.0.0");
    assert_eq!(report["stack"]["mcp"], "2.1.1");
    assert_eq!(report["stack"]["pydantic"], "2.13.4");
    assert_eq!(report["stderr"]["truncated"], false);
    assert_eq!(report["stderr"]["utf8_valid"], true);
    report
}

fn modern_step<'a>(report: &'a Value, identifier: &str) -> &'a Value {
    &report["steps"]
        .as_array()
        .expect("modern client steps")
        .iter()
        .find(|step| step["id"] == identifier)
        .unwrap_or_else(|| panic!("missing modern client step {identifier}: {report}"))["result"]
}

fn modern_structured(result: &Value) -> &Value {
    result
        .get("structured_content")
        .or_else(|| result.get("structuredContent"))
        .expect("modern FastMCP structured result")
}

fn assert_no_modern_secret_projection(report: &Value, fixture: &ProductionFixture) {
    let encoded = serde_json::to_string(report).expect("modern client report JSON");
    let runtime_root = fixture.root().display().to_string();
    for forbidden in [
        "launch_grant_hex",
        "session_token",
        "daemon_continuation",
        "request_state",
        "def answer(value: int)",
        runtime_root.as_str(),
    ] {
        assert!(
            !encoded.contains(forbidden),
            "modern client report projected forbidden material {forbidden}"
        );
    }
}

fn semantic_request(workspace_id: &str, request_id: &str, looking_for: &str) -> Value {
    json!({
        "specification": "composable semantic CPG fact query",
        "version": "2.0",
        "semantic_request_id": request_id,
        "scope": {"workspace_id": workspace_id},
        "freshness": {"policy": "require_semantic_current", "deadline_ms": 120_000},
        "queries": [{
            "request": "find code entities",
            "query_id": "q1",
            "looking_for": looking_for,
            "within": [],
            "where": [],
            "return": {"limit": {"maximum_results": 32}}
        }]
    })
}

fn activation_control_versions(fixture: &ProductionFixture) -> Vec<PathBuf> {
    let log = fixture
        .fabric_workspace_root()
        .join("activation-control/_delta_log");
    let mut versions = fs::read_dir(log)
        .expect("activation-control Delta log")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    versions.sort();
    versions
}

fn decoded_activation_control_rows(
    fixture: &ProductionFixture,
) -> Vec<PersistedActivationControlRow> {
    activation_control_rows_at(
        fixture,
        &fixture
            .fabric_workspace_root()
            .join("activation-control/_delta_log/00000000000000000001.json"),
    )
}

fn all_activation_control_rows(fixture: &ProductionFixture) -> Vec<PersistedActivationControlRow> {
    activation_control_versions(fixture)
        .into_iter()
        .skip(1)
        .flat_map(|commit| activation_control_rows_at(fixture, &commit))
        .collect()
}

fn activation_control_rows_at(
    fixture: &ProductionFixture,
    commit: &Path,
) -> Vec<PersistedActivationControlRow> {
    let table_root = fixture.fabric_workspace_root().join("activation-control");
    let add_paths = BufReader::new(fs::File::open(commit).expect("activation commit log"))
        .lines()
        .map(|line| {
            serde_json::from_str::<Value>(&line.expect("activation commit log line"))
                .expect("activation commit action")
        })
        .filter_map(|action| {
            action
                .get("add")
                .and_then(|add| add.get("path"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        add_paths.len(),
        1,
        "one fresh activation must append one exact control data file"
    );
    let codec = ActivationControlRowCodec::try_new().expect("activation row codec");
    let reader = ParquetRecordBatchReaderBuilder::try_new(
        fs::File::open(table_root.join(&add_paths[0])).expect("activation control data file"),
    )
    .expect("activation control Parquet reader")
    .build()
    .expect("activation control batch reader");
    reader
        .flat_map(|batch| {
            let batch = batch.expect("activation control Parquet batch");
            let batch = RecordBatch::try_new(
                Arc::clone(codec.contract().storage_schema()),
                batch.columns().to_vec(),
            )
            .expect("reattach released activation storage schema metadata");
            codec
                .decode_storage(&batch)
                .expect("exact activation control storage decode")
        })
        .collect()
}

#[derive(Debug)]
struct InstalledVerticalObservation {
    query_rows: u64,
    epoch_id: String,
    source_authority: [u8; 32],
    provider_set: [u8; 32],
    application_release: [u8; 32],
    provider_release: [u8; 32],
    proof_receipt: [u8; 32],
    relation_ids: BTreeSet<String>,
    reference_content: Value,
}

fn installed_vertical_observation(
    stack: &InstalledProductionStack,
    source: &[u8],
    label: &str,
) -> InstalledVerticalObservation {
    let fixture = ProductionFixture::with_source(source);
    fixture.bind_installed_adapter(stack, "policy-one", 0x11);
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let request = semantic_request(
        &fixture.workspace.public_id(),
        &format!("request:wp63-{label}"),
        "Python function declarations",
    );
    let scenario = modern_client_scenario(
        &fixture,
        stack,
        "policy-one",
        json!([]),
        json!([
            {
                "id": "reference",
                "operation": "call_tool",
                "name": "get_code_graph_reference",
                "arguments": {"kind": "request-schema"},
            },
            {
                "id": "reference_bytes",
                "operation": "read_resource",
                "uri": {"$ref": "reference.structured_content.resource.uri"},
            },
            {
                "id": "query",
                "operation": "call_tool",
                "name": "query_code_graph",
                "arguments": {"request": request, "delivery": "resource"},
            },
            {"id": "status", "operation": "call_tool", "name": "get_code_graph_status"},
            {
                "id": "manifest",
                "operation": "read_resource",
                "uri": {"$ref": "query.structured_content.manifest.uri"},
            },
            {
                "id": "page_zero",
                "operation": "read_resource",
                "uri": {"$ref": "query.structured_content.pages.0.uri"},
            },
        ]),
    );
    let scenario = write_modern_client_scenario(&fixture, label, &scenario);
    let report = modern_client_report(&run_modern_client(stack, &scenario));
    let status = modern_structured(modern_step(&report, "status"));
    let query = modern_structured(modern_step(&report, "query"));
    assert_eq!(
        status["public_status"]["semantic_release"],
        "codefabric-relational-data-fabric@2.3.0"
    );
    assert_eq!(query["execution_state"], "SUCCEEDED", "{query}");
    assert!(
        modern_step(&report, "manifest")
            .as_array()
            .is_some_and(|content| !content.is_empty())
    );
    assert!(
        modern_step(&report, "page_zero")
            .as_array()
            .is_some_and(|content| !content.is_empty())
    );

    let persisted = all_activation_control_rows(&fixture);
    assert_eq!(persisted.len(), 2, "source genesis and semantic successor");
    let selected = persisted
        .iter()
        .max_by_key(|row| row.row().ordinal.get())
        .unwrap();
    let row = selected.row();
    let epoch_id = row
        .pins
        .epoch
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(status["active_epoch_id"], format!("epoch:{epoch_id}"));
    assert_eq!(query["epoch_id"], format!("snapshot:{epoch_id}"));
    assert_eq!(
        row.predecessor_epoch,
        ExpectedHead::Epoch(persisted[0].row().pins.epoch)
    );
    assert_eq!(
        row.pins.table_versions,
        selected.table_versions().reference()
    );

    let relation_ids = selected
        .table_versions()
        .components()
        .map(|(relation_id, pin)| {
            assert_eq!(pin.version(), 1, "fresh relation {relation_id} version");
            relation_id.to_owned()
        })
        .collect::<BTreeSet<_>>();
    for required in [
        "provider.tree_sitter.coverage",
        "provider.ruff.coverage",
        "runtime.accepted_fact_family",
        "runtime.unsupported_remainder",
    ] {
        assert!(
            relation_ids.contains(required),
            "installed candidate omitted coverage/explicit-remainder relation {required}: {relation_ids:?}"
        );
    }
    #[cfg(target_os = "linux")]
    for required in [
        "provider.pyrefly.module_context.v1",
        "provider.pyrefly.located_type.v1",
    ] {
        assert!(
            relation_ids.contains(required),
            "installed candidate omitted Pyrefly semantics: {required}"
        );
    }
    let proof_root = fixture
        .fabric_workspace_root()
        .join("epochs")
        .join(&epoch_id)
        .join("proof");
    assert!(
        !proof_root.exists(),
        "ordinary activation must not produce generalized proof histories"
    );
    assert_ne!(
        row.pins.proof_receipt.as_bytes(),
        &[0; 32],
        "activation retains its candidate record identity"
    );
    assert_no_modern_secret_projection(&report, &fixture);
    let reference_blob = modern_step(&report, "reference_bytes")[0]["blob"]
        .as_str()
        .expect("request-schema resource blob");
    let reference_document: Value = serde_json::from_slice(
        &STANDARD
            .decode(reference_blob)
            .expect("request-schema resource base64"),
    )
    .expect("request-schema resource JSON");
    let observation = InstalledVerticalObservation {
        query_rows: query["total_rows"].as_u64().expect("query row count"),
        epoch_id,
        source_authority: *row.pins.source_authority.as_bytes(),
        provider_set: *row.pins.provider_set.as_bytes(),
        application_release: *row.pins.application_release.as_bytes(),
        provider_release: *row.pins.provider_release.as_bytes(),
        proof_receipt: *row.pins.proof_receipt.as_bytes(),
        relation_ids,
        reference_content: reference_document["projection"].clone(),
    };
    supervisor.stop();
    observation
}

fn installed_query_report(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    label: &str,
    stale_resource_uri: Option<&str>,
) -> Value {
    let mut steps = Vec::new();
    if let Some(uri) = stale_resource_uri {
        steps.push(json!({
            "id": "stale_resource",
            "operation": "read_resource",
            "uri": uri,
            "expect_error": "CLIENT_OPERATION_FAILED",
        }));
    }
    steps.push(json!({
        "id": "query",
        "operation": "call_tool",
        "name": "query_code_graph",
        "arguments": {
            "request": semantic_request(
                &fixture.workspace.public_id(),
                &format!("request:wp63-restart-{label}"),
                "Python function declarations",
            ),
            "delivery": "resource",
        },
    }));
    steps.push(json!({
        "id": "manifest",
        "operation": "read_resource",
        "uri": {"$ref": "query.structured_content.manifest.uri"},
    }));
    steps.push(json!({
        "id": "status",
        "operation": "call_tool",
        "name": "get_code_graph_status",
    }));
    let scenario =
        modern_client_scenario(fixture, stack, "policy-one", json!([]), Value::Array(steps));
    let scenario = write_modern_client_scenario(fixture, label, &scenario);
    modern_client_report(&run_modern_client(stack, &scenario))
}

#[test]
fn wp44_int_thin_binaries_delegate_to_strict_library_settings() {
    let fixture = ProductionFixture::new();
    let accepted = Command::new(env!("CARGO_BIN_EXE_codefabric"))
        .args(["supervisor", "check-config", "--config"])
        .arg(&fixture.config_path)
        .output()
        .expect("library-owned config check");
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );

    let ad_hoc_workspace = Command::new(env!("CARGO_BIN_EXE_codefabric"))
        .args(["supervisor", "serve", "--config"])
        .arg(&fixture.config_path)
        .args(["--workspace", &fixture.workspace.public_id()])
        .output()
        .expect("ad hoc workspace rejection");
    assert!(!ad_hoc_workspace.status.success());
    assert!(String::from_utf8_lossy(&ad_hoc_workspace.stderr).contains("closed command grammar"));

    let daemon_bypass = Command::new(env!("CARGO_BIN_EXE_codefabricd"))
        .args(["check-config", "--config"])
        .arg(&fixture.config_path)
        .output()
        .expect("daemon settings bypass rejection");
    assert!(!daemon_bypass.status.success());
    assert!(String::from_utf8_lossy(&daemon_bypass.stderr).contains("closed command grammar"));
}

#[test]
fn wp44_beh_real_supervisor_ready_requires_durable_fresh_activation() {
    // Hold the background successor so exact genesis assertions do not race semantics.
    let fixture =
        ProductionFixture::with_activation_startup_fault(Some("hold_semantic_update_publication"));
    let supervisor = fixture.start_supervisor();
    let discovery = supervisor.discovery();
    assert_eq!(discovery.daemon_generation, 1);
    assert_eq!(supervisor.status()["code"], "READY");
    let versions = activation_control_versions(&fixture);
    assert_eq!(
        versions
            .iter()
            .map(|path| path.file_name().expect("Delta log file name"))
            .collect::<Vec<_>>(),
        [
            std::ffi::OsStr::new("00000000000000000000.json"),
            std::ffi::OsStr::new("00000000000000000001.json"),
        ],
        "fresh readiness requires exactly the provisioned history and one activation append"
    );
    let persisted = decoded_activation_control_rows(&fixture);
    assert_eq!(persisted.len(), 1, "one exact activation row");
    let persisted = &persisted[0];
    let row = persisted.row();
    assert_eq!(row.workspace_id.as_bytes(), &fixture.workspace.workspace_id);
    assert!(matches!(row.predecessor_epoch, ExpectedHead::Empty));
    assert!(row.predecessor_event_id.is_none());
    assert_eq!(row.ordinal.get(), 1);
    assert_eq!(persisted.control_predecessor().table().version(), 0);
    assert_eq!(persisted.control_commit_version(), 1);
    assert_eq!(
        row.pins.table_versions,
        persisted.table_versions().reference()
    );
    assert!(
        persisted.table_versions().len() > 2,
        "fresh activation must retain a complete reversible relation vector"
    );

    let command_state = fixture
        .fabric_workspace_root()
        .join("activation-commands.sqlite3");
    assert!(
        fs::metadata(&command_state)
            .expect("durable activation command state")
            .len()
            > 0
    );
    let connection = rusqlite::Connection::open(&command_state).expect("activation state inspect");
    let request: Vec<u8> = connection
        .query_row(
            "SELECT request_jcs FROM activation_command_request LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("activation request row");
    let request: Value = serde_json::from_slice(&request).expect("activation request JSON");
    assert_eq!(
        request["command"]["identity"]["operation_id"],
        json!(row.operation_id.as_bytes())
    );
    assert_eq!(request["event_id"], json!(row.event_id.as_bytes()));
    assert_eq!(
        request["command"]["writer_fence"]["generation"],
        row.execution_fence.generation.get()
    );
    assert_eq!(
        request["command"]["writer_fence"]["lease_id"],
        json!(row.execution_fence.lease_id.as_bytes())
    );
    assert_eq!(request["pins"]["epoch"], json!(row.pins.epoch.as_bytes()));
    assert_eq!(
        request["pins"]["proof_receipt"],
        json!(row.pins.proof_receipt.as_bytes())
    );
    assert_eq!(
        request["pins"]["table_versions"],
        json!(row.pins.table_versions.as_bytes())
    );
    assert_eq!(
        request["transaction"],
        json!(row.commit.transaction.as_bytes())
    );
    assert_eq!(
        request["operation_selection"],
        json!(row.commit.operation_selection.as_bytes())
    );
    assert_eq!(request["control_version"], 0);
    assert_eq!(
        request["control_root"],
        persisted
            .control_predecessor()
            .table()
            .canonical_root()
            .as_str()
    );
    let exact_vector = Value::Array(
        persisted
            .table_versions()
            .components()
            .map(|(relation_id, pin)| {
                json!({
                    "relation_id": relation_id,
                    "canonical_root": pin.canonical_root().as_str(),
                    "version": pin.version(),
                })
            })
            .collect(),
    );
    assert_eq!(request["table_version_vector"], exact_vector);
    let reconciliation_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM activation_command_reconciliation",
            [],
            |record| record.get(0),
        )
        .expect("activation reconciliation count");
    assert_eq!(
        reconciliation_count, 0,
        "the exact direct readback path must not fabricate a reconciliation ticket"
    );

    let journal = rusqlite::Connection::open(
        fixture
            .fabric_workspace_root()
            .join("fabric-commands.sqlite3"),
    )
    .expect("fabric command journal");
    let (state_kind, terminal, record): (String, i64, Vec<u8>) = journal
        .query_row(
            "SELECT state_kind, is_terminal, record_jcs FROM fabric_command_record",
            [],
            |record| Ok((record.get(0)?, record.get(1)?, record.get(2)?)),
        )
        .expect("terminal activation command");
    assert_eq!(state_kind, "SUCCEEDED");
    assert_eq!(terminal, 1);
    let record: Value = serde_json::from_slice(&record).expect("terminal command JSON");
    assert_eq!(
        record["command"]["identity"]["operation_id"],
        json!(row.operation_id.as_bytes())
    );
    assert_eq!(
        record["state"]["Succeeded"]["transaction"],
        json!(row.commit.transaction.as_bytes())
    );
    assert_eq!(record["state"]["Succeeded"]["confirmation"], "Direct");
    assert_eq!(
        record["state"]["Succeeded"]["result"]["EpochActivated"]["epoch"],
        json!(row.pins.epoch.as_bytes())
    );
    assert_eq!(
        record["state"]["Succeeded"]["result"]["EpochActivated"]["selection"],
        json!(row.commit.operation_selection.as_bytes())
    );

    let writer_authority =
        rusqlite::Connection::open(fixture.state.join("writer-generations.sqlite"))
            .expect("writer authority state");
    let (generation, lease_id): (Vec<u8>, Vec<u8>) = writer_authority
        .query_row(
            "SELECT current_generation, current_lease_id
             FROM writer_generation_state WHERE workspace_id = ?1",
            [fixture.workspace.workspace_id.as_slice()],
            |record| Ok((record.get(0)?, record.get(1)?)),
        )
        .expect("exact active writer fence");
    assert_eq!(
        generation,
        row.execution_fence.generation.get().to_be_bytes()
    );
    assert_eq!(lease_id, row.execution_fence.lease_id.as_bytes());

    supervisor.stop();
    assert!(!discovery.query_socket.exists());
}

#[test]
fn wp44_beh_real_project_venv_launch_preserves_distribution_authority() {
    let fixture = ProductionFixture::new();
    let supervisor = fixture.start_supervisor();
    let mut client = McpClientProcess::launch(&supervisor.discovery, "policy-one");
    let tools = client.request("tools/list", json!({}));
    let names = tools["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("installed FastMCP tool list: {tools}"))
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<BTreeSet<_>>();
    assert!(names.contains("get_code_graph_status"));
    let output = client.close();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    supervisor.stop();
}

#[test]
fn wp44_ops_real_durable_append_acknowledgement_loss_reconciles_exact_readback() {
    let fixture = ProductionFixture::with_activation_startup_fault(Some(
        "durable_append_acknowledgement_lost_before_readback",
    ));
    let supervisor = fixture.start_supervisor();
    assert_eq!(supervisor.status()["code"], "READY");

    let versions = activation_control_versions(&fixture);
    assert_eq!(
        versions
            .iter()
            .map(|path| path.file_name().expect("Delta log file name"))
            .collect::<Vec<_>>(),
        [
            std::ffi::OsStr::new("00000000000000000000.json"),
            std::ffi::OsStr::new("00000000000000000001.json"),
        ],
        "unknown acknowledgement must reconcile the original append without retrying it"
    );
    let persisted = decoded_activation_control_rows(&fixture);
    assert_eq!(persisted.len(), 1, "one durable activation row");

    let activation_state = rusqlite::Connection::open(
        fixture
            .fabric_workspace_root()
            .join("activation-commands.sqlite3"),
    )
    .expect("activation reconciliation state");
    let reconciliation_count: i64 = activation_state
        .query_row(
            "SELECT COUNT(*) FROM activation_command_reconciliation",
            [],
            |record| record.get(0),
        )
        .expect("activation reconciliation count");
    assert_eq!(
        reconciliation_count, 1,
        "the injected unknown outcome must produce one exact durable reconciliation record"
    );

    let journal = rusqlite::Connection::open(
        fixture
            .fabric_workspace_root()
            .join("fabric-commands.sqlite3"),
    )
    .expect("fabric command journal");
    let (state_kind, terminal, record): (String, i64, Vec<u8>) = journal
        .query_row(
            "SELECT state_kind, is_terminal, record_jcs FROM fabric_command_record",
            [],
            |record| Ok((record.get(0)?, record.get(1)?, record.get(2)?)),
        )
        .expect("reconciled activation command");
    assert_eq!(state_kind, "SUCCEEDED");
    assert_eq!(terminal, 1);
    let record: Value = serde_json::from_slice(&record).expect("terminal command JSON");
    assert!(
        record["state"]["Succeeded"]["confirmation"]
            .get("Reconciled")
            .is_some(),
        "terminal success must be authorized by exact reconciliation: {record}"
    );

    supervisor.stop();
}

#[cfg(target_os = "linux")]
#[test]
fn wp44_ops_real_pre_ready_child_failure_keeps_admission_closed_and_cleans_endpoints() {
    let fixture =
        ProductionFixture::with_activation_startup_fault(Some("exit_before_ready_acknowledgement"));
    let mut supervisor = Command::new(env!("CARGO_BIN_EXE_codefabric"))
        .args(["supervisor", "serve", "--config"])
        .arg(&fixture.config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn production supervisor with pre-ready child fault");
    let supervisor_pid = supervisor.id();
    let children_path = format!("/proc/{supervisor_pid}/task/{supervisor_pid}/children");
    let deadline = Instant::now() + PROCESS_DEADLINE;
    let mut observed_daemon_pid = None;
    let status = loop {
        if observed_daemon_pid.is_none()
            && let Ok(children) = fs::read_to_string(&children_path)
        {
            observed_daemon_pid = children
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<u32>().ok());
        }
        if let Some(status) = supervisor.try_wait().expect("supervisor join") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "pre-ready child failure did not join within its process deadline"
        );
        thread::sleep(Duration::from_millis(10));
    };
    assert!(!status.success(), "pre-ready fault must fail startup");
    let daemon_pid = observed_daemon_pid.expect("real codefabricd child was observed");
    let daemon_deadline = Instant::now() + Duration::from_secs(5);
    while Path::new(&format!("/proc/{daemon_pid}")).exists() {
        assert!(
            Instant::now() < daemon_deadline,
            "pre-ready codefabricd child remained unjoined"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let diagnostic = supervisor
        .stderr
        .take()
        .map(|mut stderr| {
            let mut value = String::new();
            stderr.read_to_string(&mut value).unwrap();
            value
        })
        .unwrap_or_default();
    assert!(
        diagnostic.contains("before the authenticated ready acknowledgement")
            || diagnostic.contains("failed daemon hello"),
        "typed pre-ready failure diagnostic: {diagnostic}"
    );
    assert!(!fixture.supervisor_discovery().exists());
    assert!(!fixture.runtime.join("supervisor.sock").exists());
    assert!(!fixture.runtime.join("query.sock").exists());
}

#[test]
fn wp44_ops_real_launch_capacity_recovers_pending_failure_and_abrupt_exit() {
    let fixture = ProductionFixture::new();
    let mut policy = fixture.launch_policy();
    let installed_program = policy.adapter_program.clone();
    let installed_digest = policy.adapter_executable_digest.clone();
    policy.maximum_concurrent_launches = 1;
    policy.adapter_program = fixture.config_root.join("missing-adapter");
    fixture.write_launch_policy(&policy);

    let supervisor = fixture.start_supervisor();
    let failed_spawn = launch_with_closed_input(&supervisor.discovery, "policy-one");
    assert!(!failed_spawn.status.success());
    assert!(
        String::from_utf8_lossy(&failed_spawn.stderr).contains("adapter"),
        "missing adapter failure must remain diagnostic: {}",
        String::from_utf8_lossy(&failed_spawn.stderr)
    );

    policy.adapter_program = installed_program;
    policy.adapter_executable_digest = installed_digest;
    fixture.write_launch_policy(&policy);
    let active = McpClientProcess::launch(&supervisor.discovery, "policy-one");

    let capacity = launch_with_closed_input(&supervisor.discovery, "policy-one");
    assert!(!capacity.status.success());
    assert!(
        String::from_utf8_lossy(&capacity.stderr).contains("LAUNCH_CAPACITY_EXCEEDED"),
        "a live max=1 launch must retain its active slot: {}",
        String::from_utf8_lossy(&capacity.stderr)
    );

    let abandoned_adapter_pid = active.kill_abruptly();
    let deadline = Instant::now() + PROCESS_DEADLINE;
    loop {
        let recovered = launch_with_closed_input(&supervisor.discovery, "policy-one");
        if recovered.status.success() {
            assert!(
                !Path::new(&format!("/proc/{abandoned_adapter_pid}")).exists(),
                "capacity was reissued before the abandoned adapter PID was gone"
            );
            break;
        }
        assert!(
            String::from_utf8_lossy(&recovered.stderr).contains("LAUNCH_CAPACITY_EXCEEDED"),
            "abrupt-exit recovery failed for an unexpected reason: {}",
            String::from_utf8_lossy(&recovered.stderr)
        );
        assert!(
            Instant::now() < deadline,
            "abrupt launcher exit stranded the max=1 active slot"
        );
        thread::sleep(Duration::from_millis(100));
    }

    supervisor.stop();
}

#[test]
fn wp44_ops_real_supervisor_restarts_daemon_and_joins_owned_endpoints() {
    let fixture = ProductionFixture::new();
    let mut supervisor = fixture.start_supervisor();
    let initial = supervisor.discovery();
    let supervisor_socket = fs::symlink_metadata(&initial.supervisor_socket)
        .expect("supervisor socket")
        .ino();
    let daemon_pid = i32::try_from(initial.daemon_pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .expect("daemon PID");
    rustix::process::kill_process(daemon_pid, rustix::process::Signal::KILL)
        .expect("inject daemon exit");
    let restarted = supervisor.wait_for_daemon_generation(initial.daemon_generation + 1);
    assert_ne!(restarted.daemon_pid, initial.daemon_pid);
    assert_eq!(
        restarted.supervisor_generation,
        initial.supervisor_generation
    );
    assert_eq!(
        fs::symlink_metadata(&restarted.supervisor_socket)
            .expect("same owned supervisor rendezvous")
            .ino(),
        supervisor_socket
    );
    assert!(restarted.query_socket.exists());

    supervisor.stop();
    assert!(!fixture.supervisor_discovery().exists());
    assert!(!restarted.supervisor_socket.exists());
    assert!(!restarted.query_socket.exists());

    let replacement = fixture.start_supervisor();
    let replacement_discovery = replacement.discovery();
    assert_ne!(
        replacement_discovery.supervisor_generation,
        initial.supervisor_generation
    );
    replacement.stop();
    assert!(!replacement_discovery.supervisor_socket.exists());
    assert!(!replacement_discovery.query_socket.exists());
}

#[test]
fn wp44_ops_real_signal_orders_drain_before_shutdown_and_joins_owned_endpoints() {
    let fixture = ProductionFixture::new();
    let supervisor = fixture.start_supervisor();
    let discovery = supervisor.discovery();
    assert_eq!(supervisor.status()["code"], "READY");

    supervisor.stop();

    assert!(!fixture.supervisor_discovery().exists());
    assert!(!discovery.supervisor_socket.exists());
    assert!(!discovery.query_socket.exists());
    #[cfg(target_os = "linux")]
    assert!(
        !Path::new(&format!("/proc/{}", discovery.daemon_pid)).exists(),
        "drained daemon remained live after joined supervisor shutdown"
    );
}

#[test]
fn wp47_int_real_installed_wheel_modern_contract_observation() {
    let fixture = ProductionFixture::new();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([]),
        json!([{"id": "discovery", "operation": "discover"}]),
    );
    let scenario = write_modern_client_scenario(&fixture, "contract", &scenario);
    let output = run_modern_client(&stack, &scenario);
    let report = modern_client_report(&output);
    let discovery = modern_step(&report, "discovery");

    assert_eq!(discovery["protocol_version"], "2026-07-28");
    assert_eq!(discovery["supported_versions"], json!(["2026-07-28"]));
    let tools = discovery["tools"]
        .as_array()
        .expect("installed modern tool catalog");
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "get_code_graph_reference",
            "get_code_graph_status",
            "query_code_graph",
            "validate_code_graph_query",
        ])
    );
    let query = tools
        .iter()
        .find(|tool| tool["name"] == "query_code_graph")
        .expect("query tool");
    assert_eq!(
        query["input_schema"]["properties"]
            .as_object()
            .expect("query input properties")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["delivery", "request"])
    );
    assert_eq!(discovery["resources"], json!([]));
    assert_eq!(
        discovery["resource_templates"]
            .as_array()
            .expect("resource templates")
            .iter()
            .map(|template| template["uri_template"].as_str().expect("URI template"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "cpg://reference/{handle}/{kind}/{version}",
            "cpg://result/{handle}/{selector}/{page_ordinal}",
        ])
    );
    assert_eq!(discovery["prompts"], json!([]));
    assert_eq!(
        discovery["capabilities"]["extensions"],
        json!({"io.modelcontextprotocol/ui": {}}),
        "{}",
        discovery["capabilities"]
    );
    assert!(discovery["capabilities"].get("tasks").is_none());
    assert!(discovery["capabilities"]["completions"].is_object());
    assert_no_modern_secret_projection(&report, &fixture);
    supervisor.stop();
}

#[test]
#[allow(clippy::too_many_lines)]
fn wp47_beh_real_installed_wheel_guard_query_resource_and_completion() {
    let fixture = ProductionFixture::new();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let exact_request = semantic_request(
        &fixture.workspace.public_id(),
        "request:wp47-validation",
        "Python function declarations",
    );
    let guarded_request = semantic_request(
        &fixture.workspace.public_id(),
        "request:wp47-guarded",
        "functions",
    );
    let scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([{
            "message": "input.selection-resolution.description",
            "action": "accept",
            "content": {"value": {"$requested_schema_presentation": "Python function declarations"}},
        }]),
        json!([
            {"id": "discovery", "operation": "discover"},
            {"id": "status", "operation": "call_tool", "name": "get_code_graph_status"},
            {
                "id": "validation",
                "operation": "call_tool",
                "name": "validate_code_graph_query",
                "arguments": {"request": exact_request},
            },
            {
                "id": "completion",
                "operation": "complete",
                "reference": {
                    "kind": "resource",
                    "uri": "cpg://reference/{handle}/{kind}/{version}",
                },
                "argument": {"name": "kind", "value": "req"},
            },
            {
                "id": "reference",
                "operation": "call_tool",
                "name": "get_code_graph_reference",
                "arguments": {"kind": "request-schema"},
            },
            {
                "id": "reference_bytes",
                "operation": "read_resource",
                "uri": {"$ref": "reference.structured_content.resource.uri"},
            },
            {
                "id": "query",
                "operation": "call_tool",
                "name": "query_code_graph",
                "arguments": {"request": guarded_request, "delivery": "resource"},
            },
            {
                "id": "manifest",
                "operation": "read_resource",
                "uri": {"$ref": "query.structured_content.manifest.uri"},
            },
            {
                "id": "page_zero",
                "operation": "read_resource",
                "uri": {"$ref": "query.structured_content.pages.0.uri"},
            },
        ]),
    );
    let scenario = write_modern_client_scenario(&fixture, "behavior", &scenario);
    let report = modern_client_report(&run_modern_client(&stack, &scenario));

    assert_eq!(
        modern_structured(modern_step(&report, "status"))["lifecycle"],
        "READY"
    );
    assert_eq!(
        modern_structured(modern_step(&report, "validation"))["valid"],
        true
    );
    assert_eq!(
        modern_step(&report, "completion")["values"],
        json!(["request-schema"])
    );
    assert_eq!(modern_step(&report, "completion")["total"], 1);
    assert_eq!(modern_step(&report, "completion")["has_more"], false);
    let reference = modern_structured(modern_step(&report, "reference"));
    assert_eq!(reference["resource"]["kind"], "reference");
    assert!(
        modern_step(&report, "reference_bytes")
            .as_array()
            .is_some_and(|content| !content.is_empty())
    );
    let query = modern_structured(modern_step(&report, "query"));
    assert_eq!(query["outcome"], "accepted", "{query}");
    assert_eq!(query["execution_state"], "SUCCEEDED", "{query}");
    assert!(query["total_rows"].as_u64().is_some_and(|rows| rows > 0));
    assert!(query["total_pages"].as_u64().is_some_and(|pages| pages > 0));
    assert!(
        modern_step(&report, "manifest")
            .as_array()
            .is_some_and(|content| !content.is_empty())
    );
    assert!(
        modern_step(&report, "page_zero")
            .as_array()
            .is_some_and(|content| !content.is_empty())
    );
    assert_eq!(
        report["guard_observations"].as_array().map(Vec::len),
        Some(1)
    );
    let guard = &report["guard_observations"][0];
    assert_eq!(guard["matched"], true);
    assert_eq!(guard["action"], "accept");
    let choices = &guard["requested_schema"]["properties"]["value"];
    let labels = choices["x-codefabric-choice-presentations"]
        .as_object()
        .unwrap();
    for expected in [
        "Python function declarations",
        "Python class declarations",
        "Rust static declarations",
    ] {
        let id = labels
            .iter()
            .find(|(_, label)| label.as_str() == Some(expected))
            .unwrap()
            .0;
        assert!(choices["enum"].as_array().unwrap().contains(&json!(id)));
    }
    assert_no_modern_secret_projection(&report, &fixture);
    supervisor.stop();
}

#[test]
fn wp47_neg_real_agent_scope_legacy_framing_and_secret_denial() {
    let fixture = ProductionFixture::new();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    fixture.bind_installed_adapter(&stack, "policy-two", 0x22);
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);

    let owner_scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([]),
        json!([
            {
                "id": "reference",
                "operation": "call_tool",
                "name": "get_code_graph_reference",
                "arguments": {"kind": "request-schema"},
            },
            {"id": "bridge_probe_window", "operation": "sleep", "duration_ms": 5_000},
        ]),
    );
    let owner_scenario = write_modern_client_scenario(&fixture, "owner", &owner_scenario);
    let owner = spawn_modern_client(&stack, &owner_scenario);
    let owner_report = modern_client_report(&owner.wait_with_output().expect("join owner agent"));
    let foreign_uri = modern_structured(modern_step(&owner_report, "reference"))["resource"]["uri"]
        .as_str()
        .expect("owner reference URI")
        .to_owned();

    let foreign_scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-two",
        json!([]),
        json!([{
            "id": "foreign_read",
            "operation": "read_resource",
            "uri": foreign_uri,
            "expect_error": "CLIENT_OPERATION_FAILED",
        }]),
    );
    let foreign_scenario = write_modern_client_scenario(&fixture, "foreign", &foreign_scenario);
    let foreign_report = modern_client_report(&run_modern_client(&stack, &foreign_scenario));
    assert_eq!(
        modern_step(&foreign_report, "foreign_read")["error_code"],
        "CLIENT_OPERATION_FAILED"
    );

    let mut legacy = McpClientProcess::launch_uninitialized_with(
        &stack.codefabric,
        &supervisor.discovery,
        "policy-one",
    );
    let rejected = legacy.request_exact(
        "initialize",
        json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "wp47-legacy-negative", "version": "1.0.0"},
        }),
    );
    assert!(
        rejected.get("error").is_some(),
        "legacy era was accepted: {rejected}"
    );
    let legacy_output = legacy.close();
    let legacy_bytes = [legacy_output.stdout, legacy_output.stderr].concat();
    let legacy_text = String::from_utf8_lossy(&legacy_bytes);
    assert!(!legacy_text.contains("launch_grant_hex"));
    assert!(!legacy_text.contains("session_token"));
    assert!(!legacy_text.contains(fixture.root().to_string_lossy().as_ref()));

    assert_no_modern_secret_projection(&owner_report, &fixture);
    assert_no_modern_secret_projection(&foreign_report, &fixture);
    supervisor.stop();
}

#[test]
#[allow(clippy::too_many_lines)]
fn wp47_ops_real_progress_cancel_restart_reconnect_and_two_agent_isolation() {
    let fixture = ProductionFixture::new();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    fixture.bind_installed_adapter(&stack, "policy-two", 0x22);
    let mut supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let initial = supervisor.discovery();

    let cancellation_scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([]),
        json!([{
            "id": "cancelled_query",
            "operation": "cancel_tool",
            "name": "query_code_graph",
            "arguments": {
                "request": semantic_request(
                    &fixture.workspace.public_id(),
                    "request:wp47-cancelled",
                    "Python function declarations",
                ),
                "delivery": "resource",
            },
            "cancel_on_progress": true,
        }]),
    );
    let cancellation_scenario =
        write_modern_client_scenario(&fixture, "cancellation", &cancellation_scenario);
    let cancellation_report =
        modern_client_report(&run_modern_client(&stack, &cancellation_scenario));
    assert_eq!(
        modern_step(&cancellation_report, "cancelled_query"),
        &json!({"cancelled": true, "trigger": "progress"})
    );
    let coordinator = rusqlite::Connection::open(fixture.state.join("query-coordinator.sqlite3"))
        .expect("query coordinator journal");
    let records = coordinator
        .prepare("SELECT record_bytes FROM query_coordinator_record")
        .expect("query coordinator records")
        .query_map([], |row| row.get::<_, Vec<u8>>(0))
        .expect("query coordinator rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("query coordinator record bytes");
    assert!(records.iter().any(|record| {
        serde_json::from_slice::<Value>(record).is_ok_and(|record| {
            record["events"].as_array().is_some_and(|events| {
                events.iter().any(|event| {
                    event["payload"]["kind"] == "terminal"
                        && event["payload"]["payload"]["state"] == "cancelled"
                })
            })
        })
    }));

    let daemon_pid = i32::try_from(initial.daemon_pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .expect("initial daemon PID");
    rustix::process::kill_process(daemon_pid, rustix::process::Signal::KILL)
        .expect("inject daemon restart");
    let restarted = supervisor.wait_for_daemon_generation(initial.daemon_generation + 1);

    let first_request = semantic_request(
        &fixture.workspace.public_id(),
        "request:wp47-agent-one",
        "Python function declarations",
    );
    let second_request = semantic_request(
        &fixture.workspace.public_id(),
        "request:wp47-agent-two",
        "function declarations",
    );
    let first_scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "status", "operation": "call_tool", "name": "get_code_graph_status"},
            {"id": "overlap", "operation": "sleep", "duration_ms": 250},
            {
                "id": "query",
                "operation": "call_tool",
                "name": "query_code_graph",
                "arguments": {"request": first_request, "delivery": "resource"},
            },
        ]),
    );
    let second_scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-two",
        json!([]),
        json!([
            {"id": "status", "operation": "call_tool", "name": "get_code_graph_status"},
            {"id": "overlap", "operation": "sleep", "duration_ms": 250},
            {
                "id": "query",
                "operation": "call_tool",
                "name": "query_code_graph",
                "arguments": {"request": second_request, "delivery": "resource"},
            },
        ]),
    );
    let first_path = write_modern_client_scenario(&fixture, "agent-one", &first_scenario);
    let second_path = write_modern_client_scenario(&fixture, "agent-two", &second_scenario);
    let first = spawn_modern_client(&stack, &first_path);
    let second = spawn_modern_client(&stack, &second_path);
    let first_report = modern_client_report(&first.wait_with_output().expect("join first agent"));
    let second_report =
        modern_client_report(&second.wait_with_output().expect("join second agent"));
    let first_status = modern_structured(modern_step(&first_report, "status"));
    let second_status = modern_structured(modern_step(&second_report, "status"));
    assert_eq!(
        first_status["authority"]["daemon_generation"],
        restarted.daemon_generation
    );
    assert_eq!(
        first_status["authority"]["daemon_generation"],
        second_status["authority"]["daemon_generation"]
    );
    assert_ne!(
        first_status["authority"]["session_id"],
        second_status["authority"]["session_id"]
    );
    let first_query = modern_structured(modern_step(&first_report, "query"));
    let second_query = modern_structured(modern_step(&second_report, "query"));
    assert_eq!(first_query["execution_state"], "SUCCEEDED", "{first_query}");
    assert_eq!(
        second_query["execution_state"], "SUCCEEDED",
        "{second_query}"
    );
    assert_ne!(
        first_query["daemon_query_id"],
        second_query["daemon_query_id"]
    );
    assert_eq!(first_query["epoch_id"], second_query["epoch_id"]);
    assert_no_modern_secret_projection(&cancellation_report, &fixture);
    assert_no_modern_secret_projection(&first_report, &fixture);
    assert_no_modern_secret_projection(&second_report, &fixture);
    supervisor.stop();
}

#[test]
#[cfg(target_os = "linux")]
fn pragmatic_python_semantics_publish_real_call_targets() {
    // Python 3.14 is the effective context. Both functions exist syntactically;
    // only the checker-selected branch supplies the call's semantic target.
    let fixture = ProductionFixture::with_source(b"import sys\ndef legacy() -> str:\n    return 'old'\ndef current() -> int:\n    return 1\nif sys.version_info >= (3, 14):\n    selected = current\nelse:\n    selected = legacy\nanswer = selected()\ndef caller() -> int:\n    return selected() + selected()\ndef indirect(f):\n    return f()\n");
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let mut targets = BTreeSet::new();
    for batch in fresh_activation_relation_batches(&fixture, "provider.pyrefly.call_target.v1") {
        let values = batch
            .column_by_name("qualified_target")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .unwrap();
        targets.extend(values.iter().flatten().map(ToOwned::to_owned));
    }
    assert_eq!(targets, BTreeSet::from(["sample.current".to_owned()]));
    let entities = canonical_entity_names(&fixture);
    assert!(
        entities.contains(&("python".to_owned(), "current".to_owned())),
        "{entities:?}"
    );
    assert!(
        entities.contains(&("python".to_owned(), "legacy".to_owned())),
        "{entities:?}"
    );
    assert_canonical_python_calls(&fixture);
    let mut processing = std::collections::BTreeMap::new();
    for batch in fresh_activation_relation_batches(&fixture, "system.entity_processing_scope") {
        let text = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .unwrap()
        };
        for row in 0..batch.num_rows() {
            processing.insert(
                text("family").value(row).to_owned(),
                (
                    text("processing_state").value(row).to_owned(),
                    text("reason").value(row).to_owned(),
                ),
            );
        }
    }
    assert_eq!(
        processing["function-declarations"],
        ("complete".to_owned(), String::new())
    );
    assert_eq!(processing["call-targets"].0, "partial");
    assert!(
        processing["call-targets"].1.contains("unresolved_targets"),
        "{processing:?}"
    );
    assert!(
        processing["call-targets"].1.contains("caller_entities"),
        "{processing:?}"
    );
    assert_public_call_queries(&fixture, &stack, "python", "caller", "current", "fresh");
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_public_call_queries(&fixture, &stack, "python", "caller", "current", "reopen");
    supervisor.stop();
}

fn assert_canonical_python_calls(fixture: &ProductionFixture) {
    use arrow::array::{Array as _, BinaryArray, StringArray};
    use std::collections::BTreeMap;
    let mut entities = BTreeMap::new();
    for batch in fresh_activation_relation_batches(fixture, "fact.code_entity") {
        let names = batch
            .column_by_name("name")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let ids = batch
            .column_by_name("entity_id")
            .unwrap()
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        for row in 0..batch.num_rows() {
            entities.insert(names.value(row).to_owned(), ids.value(row).to_vec());
        }
    }
    let mut seen = BTreeSet::new();
    let mut calls_from_caller = 0;
    let mut unknown = 0;
    let mut module = 0;
    for batch in fresh_activation_relation_batches(fixture, "fact.code_call_site") {
        let ids = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap()
        };
        let text = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
        };
        for row in 0..batch.num_rows() {
            assert!(!ids("call_site_id").is_null(row));
            assert!(seen.insert(ids("call_site_id").value(row).to_vec()));
            let caller = text("caller_name").value(row);
            if caller.ends_with("indirect") {
                assert_eq!(ids("caller_entity_id").value(row), entities["indirect"]);
                assert!(ids("target_entity_id").is_null(row));
                assert_eq!(text("resolution").value(row), "unknown");
                assert!(!text("unknown_reason").is_null(row));
                unknown += 1;
            } else {
                assert_eq!(ids("target_entity_id").value(row), entities["current"]);
                assert_eq!(text("resolution").value(row), "resolved_declaration");
                if caller.ends_with("caller") {
                    assert_eq!(ids("caller_entity_id").value(row), entities["caller"]);
                    calls_from_caller += 1;
                } else {
                    assert!(ids("caller_entity_id").is_null(row));
                    assert_eq!(
                        text("unknown_reason").value(row),
                        "caller_entity_unavailable"
                    );
                    module += 1;
                }
            }
        }
    }
    assert_eq!(
        (seen.len(), calls_from_caller, unknown, module),
        (4, 2, 1, 1)
    );
}

#[test]
#[cfg(target_os = "linux")]
fn pragmatic_live_python_edits_converge_without_restart() {
    use arrow::array::StringArray;
    let fixture = ProductionFixture::new();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let query = |phase: &str, policy: &str, expected: &[&str]| -> u64 {
        eprintln!("live update phase: {phase}");
        let mut request = semantic_request(
            &fixture.workspace.public_id(),
            &format!("request:live-{phase}"),
            "Python function declarations",
        );
        request["freshness"] = json!({"policy": policy, "deadline_ms": 60_000});
        let scenario = modern_client_scenario(
            &fixture,
            &stack,
            "policy-one",
            json!([]),
            json!([
                {"id": "query", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": request, "delivery": "resource"}},
                {"id": "page", "operation": "read_resource", "uri": {"$ref": "query.structured_content.pages.0.uri"}},
                {"id": "status", "operation": "call_tool", "name": "get_code_graph_status"}
            ]),
        );
        let path = write_modern_client_scenario(&fixture, phase, &scenario);
        let report = modern_client_report(&run_modern_client(&stack, &path));
        let result = modern_structured(modern_step(&report, "query"));
        assert_eq!(result["execution_state"], "SUCCEEDED", "{phase}: {result}");
        assert_eq!(result["freshness"], "CURRENT", "{phase}: {result}");
        assert!(result["analysis_context_set_id"].as_str().is_some());
        assert_eq!(
            result["processing"][0]["remaining_partitions"], 0,
            "{phase}: {result}"
        );
        let bytes = STANDARD
            .decode(modern_step(&report, "page")[0]["blob"].as_str().unwrap())
            .unwrap();
        let mut names = BTreeSet::new();
        for batch in
            arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None).unwrap()
        {
            let batch = batch.unwrap();
            let column = batch
                .column_by_name("name")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            names.extend(column.iter().flatten().map(ToOwned::to_owned));
        }
        assert_eq!(
            names,
            expected.iter().map(|name| (*name).to_owned()).collect(),
            "{phase}"
        );
        let status = modern_structured(modern_step(&report, "status"));
        let observation = &status["source_observations"][0];
        assert_eq!(observation["workspace_id"], fixture.workspace.public_id());
        assert_eq!(
            observation["selected_source_generation"],
            result["source_generation"]
        );
        assert_eq!(observation["watch_healthy"], true);
        result["source_generation"].as_u64().unwrap()
    };
    let initial = query("initial", "require_current_for_targets", &["answer"]);
    let workspace = Path::new(&fixture.workspace.root_path_display);
    fs::write(
        workspace.join("sample.py"),
        b"def replacement():\n    return 2\n",
    )
    .unwrap();
    let replaced = query("replaced", "await_latest", &["replacement"]);
    assert!(replaced > initial);
    fs::write(
        workspace.join("extra.py"),
        b"def additional():\n    return 3\n",
    )
    .unwrap();
    let added = query(
        "added",
        "require_source_current",
        &["additional", "replacement"],
    );
    assert!(added > replaced);
    fs::remove_file(workspace.join("sample.py")).unwrap();
    let removed = query("removed", "require_semantic_current", &["additional"]);
    assert!(removed > added);
    fs::write(
        workspace.join("replacement.tmp"),
        b"def atomic_save():\n    return 4\n",
    )
    .unwrap();
    fs::rename(
        workspace.join("replacement.tmp"),
        workspace.join("extra.py"),
    )
    .unwrap();
    let atomic = query("atomic", "require_current_for_targets", &["atomic_save"]);
    assert!(atomic > removed);
    fs::remove_file(workspace.join("extra.py")).unwrap();
    let empty = query("empty", "require_current_for_targets", &[]);
    assert!(empty > atomic);
    fs::write(
        workspace.join("renamed.py"),
        b"def atomic_save():\n    return 4\n",
    )
    .unwrap();
    let recreated = query("recreated", "require_semantic_current", &["atomic_save"]);
    assert!(recreated > empty);
    assert_eq!(
        query("unchanged", "await_latest", &["atomic_save"]),
        recreated,
        "unchanged census must not republish"
    );
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_eq!(
        query("reopen", "require_source_current", &["atomic_save"]),
        recreated
    );
    supervisor.stop();
}

#[test]
#[cfg(target_os = "linux")]
fn pragmatic_public_source_context_is_exact_and_separately_authorized() {
    use arrow::array::{Array, BinaryArray, BooleanArray, StringArray, StructArray, UInt64Array};
    let fixture = ProductionFixture::with_source(
        "# π\r\ndef café(value):\r\n    return value\r\n".as_bytes(),
    );
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let target = fresh_activation_relation_batches(&fixture, "fact.code_declaration")
        .into_iter()
        .find_map(|batch| {
            let names = batch
                .column_by_name("name")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let ids = batch
                .column_by_name("public_entity_id")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            (0..batch.num_rows())
                .find(|row| names.value(*row) == "café")
                .map(|row| ids.value(row).to_owned())
        })
        .expect("real Unicode declaration");
    let mut request = semantic_request(&fixture.workspace.public_id(), "request:source", "unused");
    request["queries"] = json!([{
        "request": "retrieve source and syntax context", "query_id": "source",
        "about": [{"entity_id": target}], "context": "exact source span",
        "return": {"maximum_source_bytes": 32, "limit": {"maximum_results": 32}}
    }]);
    let run = |phase: &str, allowed: bool| {
        let mut steps = Vec::new();
        if !allowed {
            let metadata = semantic_request(
                &fixture.workspace.public_id(),
                &format!("request:{phase}-metadata"),
                "Python function declarations",
            );
            steps.push(json!({"id": "metadata", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": metadata, "delivery": "resource"}}));
        }
        for (id, bound) in [("full", 32), ("limited", 4)] {
            let mut selected = request.clone();
            selected["semantic_request_id"] = json!(format!("request:source-{phase}-{id}"));
            selected["queries"][0]["return"]["maximum_source_bytes"] = json!(bound);
            steps.push(json!({"id": id, "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": selected, "delivery": "resource"}}));
            if !allowed {
                steps.last_mut().unwrap()["expect_error"] = json!("CLIENT_OPERATION_FAILED");
            }
            if allowed {
                steps.push(json!({"id": format!("{id}_page"), "operation": "read_resource", "uri": {"$ref": format!("{id}.structured_content.pages.0.uri")}}));
            }
        }
        let scenario =
            modern_client_scenario(&fixture, &stack, "policy-one", json!([]), json!(steps));
        let path = write_modern_client_scenario(&fixture, phase, &scenario);
        let report = modern_client_report(&run_modern_client(&stack, &path));
        if !allowed {
            assert_eq!(
                modern_structured(modern_step(&report, "metadata"))["execution_state"],
                "SUCCEEDED"
            );
        }
        for id in ["full", "limited"] {
            if !allowed {
                assert_eq!(
                    modern_step(&report, id)["error_code"],
                    "CLIENT_OPERATION_FAILED"
                );
                assert_eq!(
                    modern_step(&report, id)["public_error"],
                    "PERMISSION_DENIED:NOT_AUTHORIZED"
                );
                continue;
            }
            let result = modern_structured(modern_step(&report, id));
            assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
            assert_eq!(result["processing"][0]["family"], "source-context");
            let bytes = STANDARD
                .decode(
                    modern_step(&report, &format!("{id}_page"))[0]["blob"]
                        .as_str()
                        .unwrap(),
                )
                .unwrap();
            let batches =
                arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None)
                    .unwrap()
                    .map(Result::unwrap)
                    .collect::<Vec<_>>();
            assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 1);
            let batch = batches.iter().find(|batch| batch.num_rows() > 0).unwrap();
            assert!(
                batch.column_by_name("source_bytes").is_none(),
                "private whole-file bytes leaked"
            );
            let context = batch
                .column_by_name("source_context")
                .unwrap()
                .as_any()
                .downcast_ref::<StructArray>()
                .unwrap();
            let number = |name| {
                context
                    .column_by_name(name)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap()
                    .value(0)
            };
            let delivered_column = if id == "full" { 9 } else { 8 };
            assert_eq!(
                (number("start_byte"), number("end_byte")),
                (10, 6 + delivered_column)
            );
            assert_eq!((number("start_line"), number("start_byte_column")), (2, 4));
            assert_eq!(
                (number("end_line"), number("end_byte_column")),
                (2, delivered_column)
            );
            let text = context
                .column_by_name("text")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let raw = context
                .column_by_name("bytes")
                .unwrap()
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap();
            let complete = context
                .column_by_name("complete")
                .unwrap()
                .as_any()
                .downcast_ref::<BooleanArray>()
                .unwrap()
                .value(0);
            if id == "full" {
                assert_eq!(text.value(0), "café");
                assert!(raw.is_null(0));
                assert_eq!((number("returned_bytes"), number("omitted_bytes")), (5, 0));
                assert!(complete);
            } else {
                assert!(text.is_null(0));
                assert_eq!(raw.value(0), b"caf\xc3");
                assert_eq!((number("returned_bytes"), number("omitted_bytes")), (4, 1));
                assert!(!complete);
            }
        }
    };
    run("source-denied", false);
    let policy = |allow| {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, allow)
            .unwrap();
    };
    policy(true);
    run("source-allowed", true);
    // Keep the same live client/session across revocation of an already published page.
    let mut retained_request = request.clone();
    retained_request["semantic_request_id"] = json!("request:source-retained-revocation");
    let scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "source", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": retained_request, "delivery": "resource"}},
        {"id": "before", "operation": "read_resource", "uri": {"$ref": "source.structured_content.manifest.uri"}},
            {"id": "revoke", "operation": "barrier", "name": "source-policy"},
            {"id": "after", "operation": "read_resource", "uri": {"$ref": "source.structured_content.pages.0.uri"}, "expect_error": "CLIENT_OPERATION_FAILED"}
        ]),
    );
    let path = write_modern_client_scenario(&fixture, "source-retained", &scenario);
    let mut client = spawn_modern_client(&stack, &path);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !path.parent().unwrap().join("source-policy.ready").exists() {
        assert!(
            client.try_wait().unwrap().is_none(),
            "source client exited before revocation barrier"
        );
        assert!(
            Instant::now() < deadline,
            "source client did not reach revocation barrier"
        );
        thread::sleep(Duration::from_millis(10));
    }
    policy(false);
    fs::write(
        path.parent().unwrap().join("source-policy.resume"),
        b"resume\n",
    )
    .unwrap();
    let report = modern_client_report(&client.wait_with_output().unwrap());
    assert_eq!(
        modern_step(&report, "after")["error_code"],
        "CLIENT_OPERATION_FAILED"
    );
    assert_eq!(
        modern_step(&report, "after")["public_error"],
        "PERMISSION_DENIED:NOT_AUTHORIZED"
    );
    policy(true);
    supervisor.stop();
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    run("source-reopen", true);

    // An unread page remains bound to the old exact source after a successor converges.
    let mut pinned_request = request.clone();
    pinned_request["semantic_request_id"] = json!("request:source-pinned-edit");
    let scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "source", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": pinned_request, "delivery": "resource"}},
            {"id": "edit", "operation": "barrier", "name": "source-edit"},
            {"id": "page", "operation": "read_resource", "uri": {"$ref": "source.structured_content.pages.0.uri"}}
        ]),
    );
    let path = write_modern_client_scenario(&fixture, "source-pinned-edit", &scenario);
    let mut client = spawn_modern_client(&stack, &path);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !path.parent().unwrap().join("source-edit.ready").exists() {
        assert!(
            client.try_wait().unwrap().is_none(),
            "source client exited before edit barrier"
        );
        assert!(
            Instant::now() < deadline,
            "source client did not reach edit barrier"
        );
        thread::sleep(Duration::from_millis(10));
    }
    fs::write(
        fixture._root.path().join("workspace/sample.py"),
        b"def changed():\n    pass\n",
    )
    .unwrap();
    let mut current = semantic_request(
        &fixture.workspace.public_id(),
        "request:source-after-edit",
        "Python function declarations",
    );
    current["freshness"] = json!({"policy": "await_latest", "deadline_ms": 60_000});
    let current_scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "query", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": current, "delivery": "resource"}}
        ]),
    );
    let current_path =
        write_modern_client_scenario(&fixture, "source-after-edit", &current_scenario);
    let current_report = modern_client_report(&run_modern_client(&stack, &current_path));
    let current_result = modern_structured(modern_step(&current_report, "query"));
    assert_eq!(
        current_result["execution_state"], "SUCCEEDED",
        "{current_result}"
    );
    fs::write(
        path.parent().unwrap().join("source-edit.resume"),
        b"resume\n",
    )
    .unwrap();
    let report = modern_client_report(&client.wait_with_output().unwrap());
    let selected = modern_structured(modern_step(&report, "source"));
    assert!(
        selected["source_generation"].as_u64().unwrap()
            < current_result["source_generation"].as_u64().unwrap()
    );
    let bytes = STANDARD
        .decode(modern_step(&report, "page")[0]["blob"].as_str().unwrap())
        .unwrap();
    let batches = arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None)
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    let batch = batches.iter().find(|batch| batch.num_rows() > 0).unwrap();
    let context = batch
        .column_by_name("source_context")
        .unwrap()
        .as_any()
        .downcast_ref::<StructArray>()
        .unwrap();
    assert_eq!(
        context
            .column_by_name("text")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap()
            .value(0),
        "café"
    );
    policy(false);
    run("source-revoked", false);
    supervisor.stop();
}

#[test]
#[cfg(target_os = "linux")]
fn pragmatic_python_public_lexical_references() {
    const SOURCE: &str =
        "def leaf(value):\n    return value\ndef caller():\n    return leaf(1) + leaf(2)\n";
    let stack = InstalledProductionStack::build();
    for incomplete in [false, true] {
        let source = if incomplete {
            format!("{SOURCE}def unresolved():\n    return missing\n")
        } else {
            SOURCE.to_owned()
        };
        let fixture = ProductionFixture::with_source(source.as_bytes());
        fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
        let supervisor = fixture.start_supervisor_with(&stack.codefabric);
        assert_public_lexical_references(&fixture, &stack, &source, incomplete, "fresh");
        supervisor.stop();
        if !incomplete {
            let supervisor = fixture.start_supervisor_with(&stack.codefabric);
            assert_public_lexical_references(&fixture, &stack, &source, incomplete, "reopen");
            supervisor.stop();
        }
    }
}

fn assert_public_lexical_references(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    source: &str,
    incomplete: bool,
    phase: &str,
) {
    use arrow::array::{Array, StringArray, UInt64Array};
    let target = fresh_activation_relation_batches(fixture, "fact.code_declaration")
        .into_iter()
        .find_map(|batch| {
            let names = batch
                .column_by_name("name")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let ids = batch
                .column_by_name("public_entity_id")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            (0..batch.num_rows())
                .find(|row| names.value(*row) == "leaf")
                .map(|row| ids.value(row).to_owned())
        })
        .unwrap();
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "request:references",
        "unused",
    );
    request["scope"]["languages"] = json!(["python"]);
    request["queries"] = json!([{
        "request": "follow code relationships", "query_id": "references",
        "starting_from": [{"entity_id": target}, {"entity_id": target}],
        "relationship": "lexical references", "direction": "incoming", "distance": "one step",
        "return": {"limit": {"maximum_results": 32}}
    }]);
    let mut steps = Vec::new();
    for id in ["incoming", "limited", "empty"] {
        let mut selected = request.clone();
        selected["semantic_request_id"] = json!(format!("request:references-{phase}-{id}"));
        if id == "limited" {
            selected["queries"][0]["return"]["limit"]["maximum_results"] = json!(1);
        }
        if id == "empty" {
            selected["queries"][0]["starting_from"] =
                json!([{"entity_id": "entity:function:01010101010101010101010101010101"}]);
        }
        steps.push(json!({"id": id, "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": selected, "delivery": "resource"}}));
        steps.push(json!({"id": format!("{id}_page"), "operation": "read_resource", "uri": {"$ref": format!("{id}.structured_content.pages.0.uri")}}));
    }
    let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
    let path = write_modern_client_scenario(fixture, "canonical-references", &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let batches = |report: &Value, id: &str| {
        let bytes = STANDARD
            .decode(modern_step(report, id)[0]["blob"].as_str().unwrap())
            .unwrap();
        arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None)
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>()
    };
    let mut reference_ids = BTreeSet::new();
    let mut positions = BTreeSet::new();
    for id in ["incoming", "limited", "empty"] {
        let result = modern_structured(modern_step(&report, id));
        assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
        assert_eq!(result["processing"][0]["family"], "lexical-references");
        assert_eq!(
            result["processing"][0]["remaining_partitions"],
            u64::from(incomplete)
        );
        assert_eq!(result["processing"][0]["additional_rows"], id == "limited");
        if incomplete {
            assert_eq!(
                result["processing"][0]["remainder"][0]["reason_code"],
                "lexical_reference_targets_unknown"
            );
        }
        let batches = batches(&report, &format!("{id}_page"));
        assert_eq!(
            batches.iter().map(RecordBatch::num_rows).sum::<usize>(),
            match id {
                "incoming" => 3,
                "limited" => 1,
                _ => 0,
            }
        );
        if id != "incoming" {
            continue;
        }
        for batch in batches {
            let text = |name| {
                batch
                    .column_by_name(name)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
            };
            let starts = batch
                .column_by_name("start_byte")
                .unwrap()
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap();
            for row in 0..batch.num_rows() {
                assert_eq!(text("relationship_kind").value(row), "lexical-reference");
                assert_eq!(text("reference_name").value(row), "leaf");
                assert_eq!(
                    text("reference_kind").value(row),
                    if starts.value(row) == 4 {
                        "write"
                    } else {
                        "call-reference"
                    }
                );
                assert_eq!(text("resolution_scope").value(row), "lexical");
                assert_eq!(text("public_target_entity_id").value(row), target);
                assert!(text("public_call_site_id").is_null(row));
                assert!(batch.column_by_name("argument_count").unwrap().is_null(row));
                assert_eq!(
                    text("public_source_entity_id").value(row),
                    text("public_occurrence_id").value(row)
                );
                assert!(reference_ids.insert(text("public_occurrence_id").value(row).to_owned()));
                positions.insert(starts.value(row) as usize);
            }
        }
    }
    assert_eq!(
        positions,
        source
            .match_indices("leaf(")
            .map(|(index, _)| index)
            .collect()
    );
    request["semantic_request_id"] = json!(format!("request:references-{phase}-outgoing"));
    request["queries"][0]["starting_from"] = json!(
        reference_ids
            .iter()
            .map(|id| json!({"entity_id": id}))
            .collect::<Vec<_>>()
    );
    request["queries"][0]["direction"] = json!("outgoing");
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "outgoing", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": request, "delivery": "resource"}},
            {"id": "page", "operation": "read_resource", "uri": {"$ref": "outgoing.structured_content.pages.0.uri"}}
        ]),
    );
    let path = write_modern_client_scenario(fixture, "reference-endpoints", &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "outgoing"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
    let returned = batches(&report, "page");
    assert_eq!(returned.iter().map(RecordBatch::num_rows).sum::<usize>(), 3);
    for batch in returned {
        let ids = batch
            .column_by_name("public_target_entity_id")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert!(ids.iter().all(|id| id == Some(target.as_str())));
    }
}

#[test]
#[cfg(target_os = "linux")]
fn pragmatic_python_public_declaration_kinds() {
    let fixture = ProductionFixture::with_source(
        b"import sys\ntype Alias[T] = list[T]\nmarker = 1\nclass Box:\n    def read(self, value: int) -> int:\n        return value\n",
    );
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    assert_public_declaration_kinds(
        &fixture,
        &stack,
        "python",
        &[
            ("Python class declarations", "class", &["Box"]),
            (
                "Python parameter declarations",
                "parameter",
                &["self", "value"],
            ),
            ("Python binding declarations", "binding", &["marker"]),
            ("Python import declarations", "import", &["sys"]),
            ("Python type-alias declarations", "type-alias", &["Alias"]),
            (
                "Python type-parameter declarations",
                "type-parameter",
                &["T"],
            ),
        ],
    );
    supervisor.stop();
}

#[test]
#[cfg(target_os = "linux")]
fn pragmatic_python_implicit_calls_qualify_call_coverage() {
    let fixture = ProductionFixture::with_source(
        b"class Box:\n    @property\n    def value(self) -> int:\n        return 1\ndef read(box: Box) -> int:\n    return box.value\n",
    );
    let supervisor = fixture.start_supervisor();
    let scopes = fresh_activation_relation_batches(&fixture, "system.entity_processing_scope");
    let mut observed = false;
    for batch in scopes {
        let text = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .unwrap()
        };
        for row in 0..batch.num_rows() {
            if text("family").value(row) == "call-targets" {
                observed = true;
                assert_eq!(text("processing_state").value(row), "partial");
                assert!(
                    text("reason")
                        .value(row)
                        .contains("implicit_calls_not_normalized"),
                    "{:?}",
                    text("reason")
                );
            } else if text("family").value(row) == "function-declarations" {
                assert_eq!(text("processing_state").value(row), "complete");
            }
        }
    }
    assert!(observed);
    supervisor.stop();
}

#[test]
#[cfg(target_os = "linux")]
fn pragmatic_python_chunked_inventory_publishes_cross_module_semantics() {
    use arrow::array::{Array as _, BinaryArray, Decimal128Array, StringArray};

    let fixture =
        ProductionFixture::with_source(b"from extra_69 import chosen\nanswer = chosen()\n");
    let workspace = Path::new(&fixture.workspace.root_path_display);
    for index in 1..70 {
        fs::write(
            workspace.join(format!("extra_{index}.py")),
            "def chosen() -> int:\n    return 42\n",
        )
        .unwrap();
    }
    let supervisor = fixture.start_supervisor();
    let target_file = fresh_activation_relation_batches(&fixture, "source.code_file")
        .iter()
        .find_map(|batch| {
            let paths = batch
                .column_by_name("relative_path")
                .unwrap()
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap();
            let files = batch
                .column_by_name("file_id")
                .unwrap()
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap();
            (0..batch.num_rows())
                .find(|row| paths.value(*row) == b"extra_69.py")
                .map(|row| files.value(row).to_vec())
        })
        .unwrap();
    let target_file = codefabric::identity::encode_public_id(
        codefabric::identity::IdentityDomain::SourceFile,
        None,
        target_file.try_into().unwrap(),
    )
    .unwrap();
    let mut targets = BTreeSet::new();
    for batch in fresh_activation_relation_batches(&fixture, "provider.pyrefly.call_target.v1") {
        let values = batch
            .column_by_name("qualified_target")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        targets.extend(values.iter().flatten().map(ToOwned::to_owned));
        let files = batch
            .column_by_name("target_file_id")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let starts = batch
            .column_by_name("target_start_byte")
            .unwrap()
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .unwrap();
        let ends = batch
            .column_by_name("target_end_byte")
            .unwrap()
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .unwrap();
        for row in 0..batch.num_rows() {
            assert_eq!(files.value(row), target_file);
            assert_eq!((starts.value(row), ends.value(row)), (4, 10));
        }
    }
    assert_eq!(targets, BTreeSet::from(["extra_69.chosen".to_owned()]));
    let mut declarations = BTreeSet::new();
    let mut expected_target = None;
    for batch in fresh_activation_relation_batches(&fixture, "fact.code_declaration") {
        let names = batch
            .column_by_name("name")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let kinds = batch
            .column_by_name("entity_kind")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let entities = batch
            .column_by_name("entity_id")
            .unwrap()
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        let files = batch
            .column_by_name("file_id")
            .unwrap()
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        for row in 0..batch.num_rows() {
            if names.value(row) == "chosen" && kinds.value(row) == "function" {
                declarations.insert(entities.value(row).to_vec());
                let file = codefabric::identity::encode_public_id(
                    codefabric::identity::IdentityDomain::SourceFile,
                    None,
                    files.value(row).try_into().unwrap(),
                )
                .unwrap();
                if file == target_file {
                    expected_target = Some(entities.value(row).to_vec());
                }
            }
        }
    }
    // Each file defines a different function even though every spelling is the same.
    assert_eq!(declarations.len(), 69);
    let expected_target = expected_target.unwrap();
    let mut calls = 0;
    for batch in fresh_activation_relation_batches(&fixture, "fact.code_call_site") {
        let targets = batch
            .column_by_name("target_entity_id")
            .unwrap()
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        let sites = batch
            .column_by_name("call_site_id")
            .unwrap()
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        let starts = batch
            .column_by_name("start_byte")
            .unwrap()
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .unwrap();
        let ends = batch
            .column_by_name("end_byte")
            .unwrap()
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .unwrap();
        for row in 0..batch.num_rows() {
            assert_eq!(targets.value(row), expected_target);
            assert!(!sites.is_null(row));
            assert_eq!((starts.value(row), ends.value(row)), (37, 45));
            calls += 1;
        }
    }
    assert_eq!(calls, 1);
    supervisor.stop();
}

#[test]
#[cfg(target_os = "linux")]
fn pragmatic_rust_semantics_publish_real_call_targets() {
    rust_semantics_publication(None, false);
}

#[test]
#[cfg(target_os = "linux")]
fn pragmatic_rust_semantics_publish_captured_path_dependency() {
    rust_semantics_publication(Some(RustFixtureDependency::Path), false);
}

#[test]
#[cfg(target_os = "linux")]
fn pragmatic_rust_semantics_publish_locked_directory_dependency_and_reopen() {
    rust_semantics_publication(Some(RustFixtureDependency::Directory), false);
}

#[test]
#[cfg(target_os = "linux")]
fn pragmatic_rust_target_failure_retains_other_targets() {
    rust_semantics_publication(None, true);
}

#[cfg(target_os = "linux")]
#[test]
fn pragmatic_all_rust_targets_failed_retains_diagnostics_and_source() {
    let fixture = ProductionFixture::new();
    let workspace = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname='fixture'\nversion='0.1.0'\nedition='2024'\n",
    )
    .unwrap();
    fs::write(
        workspace.join("Cargo.lock"),
        "version=4\n[[package]]\nname='fixture'\nversion='0.1.0'\n",
    )
    .unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "pub fn unresolved() { missing_function(); }\n",
    )
    .unwrap();
    fs::write(workspace.join("diagnostic.py"), "value: int = 'wrong'\n").unwrap();
    let supervisor = fixture.start_supervisor();
    let names = canonical_entity_names(&fixture);
    assert!(names.iter().all(|(language, _)| language == "python"));
    assert_structured_rust_failure_diagnostics(&fixture);
    assert!(
        fresh_activation_relation_batches(&fixture, "source.exact_source_bytes")
            .iter()
            .any(|batch| {
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
                    .any(|(path, source)| {
                        path == Some(b"src/lib.rs".as_slice())
                            && source
                                == Some(b"pub fn unresolved() { missing_function(); }\n".as_slice())
                    })
            }),
        "compilation failure must preserve the exact captured Rust source"
    );
    assert!(
        fresh_activation_relation_batches(&fixture, "provider.tree_sitter_rust.cst_node")
            .iter()
            .any(|batch| batch
                .column_by_name("raw_kind")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .unwrap()
                .iter()
                .any(|kind| kind == Some("function_item"))),
        "Rust syntax remains available without MIR"
    );
    let batches = fresh_activation_relation_batches(&fixture, "system.rust_target_progress");
    assert!(batches.iter().all(|batch| {
        batch
            .column_by_name("processing_state")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .unwrap()
            .iter()
            .all(|state| state == Some("unavailable"))
    }));
    assert_canonical_diagnostics_and_reopen(&fixture, supervisor);
}

#[cfg(target_os = "linux")]
fn assert_canonical_diagnostics_and_reopen(
    fixture: &ProductionFixture,
    supervisor: RunningSupervisor,
) {
    let messages = canonical_diagnostic_rows(fixture, "fact.code_diagnostic");
    assert!(messages.iter().any(|row| row["language"] == "python"
        && row["structured_fields_available"] == false
        && row["code"].is_null()
        && row["severity"].is_null()
        && row["authority"] == "pyrefly-rendered-diagnostic"));
    let spans = canonical_diagnostic_rows(fixture, "fact.code_diagnostic_span");
    assert!(
        spans
            .iter()
            .any(|row| row["location_state"] == "captured-source"
                && row["is_primary"] == true
                && row["start_byte"] == 22
                && row["end_byte"] == 38)
    );
    let coverage = canonical_diagnostic_rows(fixture, "system.entity_processing_scope");
    assert!(
        coverage.iter().any(|row| row["language"] == "rust"
            && row["family"] == "diagnostic-messages"
            && row["processing_state"] == "failed"
            && row["reason"] == "provider_failed"),
        "closed positive diagnostic rows do not certify the entire failed target"
    );
    assert!(coverage.iter().any(|row| row["language"] == "python"
        && row["family"] == "diagnostic-locations"
        && row["processing_state"] == "unsupported"));
    let selected = wait_for_semantic_activation(fixture);
    let details = ["child", "span", "suggestion", "edit"].map(|kind| {
        let relation = format!("fact.code_diagnostic_{kind}");
        let rows = canonical_diagnostic_rows(fixture, &relation);
        (relation, rows)
    });
    supervisor.stop();
    let supervisor = fixture.start_supervisor();
    assert_eq!(
        selected.table_versions(),
        wait_for_semantic_activation(fixture).table_versions()
    );
    assert_eq!(
        messages,
        canonical_diagnostic_rows(fixture, "fact.code_diagnostic")
    );
    for (relation, rows) in details {
        assert_eq!(rows, canonical_diagnostic_rows(fixture, &relation));
    }
    supervisor.stop();
}

#[cfg(target_os = "linux")]
fn canonical_diagnostic_rows(
    fixture: &ProductionFixture,
    relation: &str,
) -> Vec<serde_json::Value> {
    let batches = fresh_activation_relation_batches(fixture, relation);
    let mut writer = arrow::json::WriterBuilder::new()
        .with_explicit_nulls(true)
        .build::<_, arrow::json::writer::JsonArray>(Vec::new());
    writer
        .write_batches(&batches.iter().collect::<Vec<_>>())
        .unwrap();
    writer.finish().unwrap();
    let mut rows: Vec<serde_json::Value> = serde_json::from_slice(&writer.into_inner()).unwrap();
    rows.sort_by_cached_key(serde_json::Value::to_string);
    rows
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
enum RustFixtureDependency {
    Path,
    Directory,
}

#[cfg(target_os = "linux")]
fn rust_semantics_publication(dependency: Option<RustFixtureDependency>, with_failure: bool) {
    let with_dependency = dependency.is_some();
    let fixture = if with_failure {
        ProductionFixture::with_source(b"def answer(value: int) -> int:\n    return value + 1\n\ndef zebra() -> int:\n    return 2\n")
    } else {
        ProductionFixture::new()
    };
    let stack = with_failure.then(InstalledProductionStack::build);
    if let Some(stack) = &stack {
        fixture.bind_installed_adapter(stack, "policy-one", 0x11);
    }
    let workspace = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir(workspace.join("src")).unwrap();
    fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        workspace.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        workspace.join("src/lib.rs"),
        "mod other; pub fn caller() -> u32 { other::target(4) }\n",
    )
    .unwrap();
    fs::write(
        workspace.join("src/other.rs"),
        "pub fn target(v: u32) -> u32 { v + 1 }\npub fn indirect(f: fn(u32) -> u32) -> u32 { f(4) }\npub fn two_calls() -> u32 { target(4) + target(5) }\nmacro_rules! forward { ($value:expr) => { target($value) } }\npub fn through_macro() -> u32 { forward!(4) }\n",
    )
    .unwrap();
    if with_dependency {
        fs::create_dir_all(workspace.join("helper/src")).unwrap();
        fs::write(
            workspace.join("helper/Cargo.toml"),
            "[package]\nname = \"helper\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            workspace.join("helper/src/lib.rs"),
            "pub fn increment(value: u32) -> u32 { value + 1 }\n",
        )
        .unwrap();
        fs::write(
            workspace.join("src/other.rs"),
            "pub fn target(v: u32) -> u32 { helper::increment(v) }\n",
        )
        .unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(workspace.join("Cargo.toml"))
            .unwrap()
            .write_all(b"[dependencies]\nhelper = { path = \"helper\" }\n")
            .unwrap();
        fs::write(workspace.join("Cargo.lock"), "version = 4\n[[package]]\nname = \"fixture\"\nversion = \"0.1.0\"\ndependencies = [\"helper\"]\n[[package]]\nname = \"helper\"\nversion = \"0.1.0\"\n").unwrap();
    }
    if matches!(dependency, Some(RustFixtureDependency::Directory)) {
        fs::create_dir(workspace.join("third_party")).unwrap();
        fs::rename(
            workspace.join("helper"),
            workspace.join("third_party/helper"),
        )
        .unwrap();
        fs::create_dir(workspace.join(".cargo")).unwrap();
        fs::write(workspace.join(".cargo/config.toml"), "[source.crates-io]\nreplace-with='captured'\n[source.captured]\ndirectory='third_party'\n").unwrap();
        fs::write(workspace.join("Cargo.toml"), "[package]\nname='fixture'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nhelper='=0.1.0'\n").unwrap();
        // A deliberately local directory-source fixture. Cargo checks the package checksum
        // against the locked source identity; production also pins every captured file byte.
        let checksum = "a".repeat(64);
        fs::write(workspace.join("Cargo.lock"), format!("version = 4\n[[package]]\nname='fixture'\nversion='0.1.0'\ndependencies=['helper']\n[[package]]\nname='helper'\nversion='0.1.0'\nsource='registry+https://github.com/rust-lang/crates.io-index'\nchecksum='{checksum}'\n")).unwrap();
        fs::write(
            workspace.join("third_party/helper/.cargo-checksum.json"),
            serde_json::to_vec(&json!({"package": checksum, "files": {}})).unwrap(),
        )
        .unwrap();
        fs::create_dir(workspace.join("third_party/helper/tests")).unwrap();
        fs::write(
            workspace.join("third_party/helper/tests/not_selected.rs"),
            "compile_error!(\"dependency tests are not selected\");\n",
        )
        .unwrap();
    }
    if with_failure {
        fs::OpenOptions::new()
            .append(true)
            .open(workspace.join("src/other.rs"))
            .unwrap()
            .write_all(b"pub const ANSWER: u32 = 42;\npub static TOTAL: u32 = 7;\n")
            .unwrap();
        fs::write(workspace.join("scratch.rs"), "// π\r\nfn unfinished( {").unwrap();
        fs::create_dir_all(workspace.join("src/bin")).unwrap();
        fs::write(
            workspace.join("src/bin/broken.rs"),
            "fn main() { missing_function(); }\n",
        )
        .unwrap();
        fs::write(
            workspace.join("src/bin/working.rs"),
            "fn main() { let _ = fixture::caller(); }\n",
        )
        .unwrap();
    }
    let supervisor = stack.as_ref().map_or_else(
        || fixture.start_supervisor(),
        |stack| fixture.start_supervisor_with(&stack.codefabric),
    );
    let entities = canonical_entity_names(&fixture);
    if with_dependency && !entities.iter().any(|(language, _)| language == "rust") {
        live_updates::print_cargo_failure(&fixture);
        eprintln!(
            "Rust progress: {:?}",
            fresh_activation_relation_batches(&fixture, "system.rust_target_progress")
        );
    }
    assert!(
        entities.contains(&("python".to_owned(), "answer".to_owned())),
        "{entities:?}"
    );
    assert!(
        entities
            .iter()
            .any(|(language, name)| language == "rust" && name.ends_with("caller")),
        "{entities:?}"
    );
    assert_canonical_rust_declaration(&fixture);
    assert_canonical_python_reference(&fixture);
    assert_canonical_rust_calls(&fixture, !with_dependency);
    let mut targets = BTreeSet::new();
    for batch in fresh_activation_relation_batches(&fixture, "provider.rustc.call.v1") {
        let values = batch
            .column_by_name("declared_target")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .unwrap();
        targets.extend(values.iter().flatten().map(ToOwned::to_owned));
    }
    assert!(
        targets
            .iter()
            .any(|target| target.ends_with("other::target")),
        "{targets:?}"
    );
    if with_dependency {
        assert!(
            targets
                .iter()
                .any(|target| target.ends_with("helper::increment")),
            "{targets:?}"
        );
    }
    if with_failure {
        let mut states = std::collections::BTreeMap::new();
        for batch in fresh_activation_relation_batches(&fixture, "system.rust_target_progress") {
            let names = batch
                .column_by_name("target_name")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .unwrap();
            let values = batch
                .column_by_name("processing_state")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .unwrap();
            for row in 0..batch.num_rows() {
                states.insert(names.value(row).to_owned(), values.value(row).to_owned());
            }
        }
        assert_eq!(
            states.get("broken").map(String::as_str),
            Some("unavailable")
        );
        assert_eq!(states.get("working").map(String::as_str), Some("processed"));
        assert_eq!(states.get("fixture").map(String::as_str), Some("processed"));
        assert_structured_rust_failure_diagnostics(&fixture);
        assert_rust_syntax_survives_compilation_failure(&fixture);
        assert_mixed_public_entity_queries(&fixture, stack.as_ref().unwrap());
        assert_public_declaration_kinds(
            &fixture,
            stack.as_ref().unwrap(),
            "rust",
            &[
                ("Rust constant declarations", "constant", &["ANSWER"]),
                ("Rust static declarations", "static", &["TOTAL"]),
            ],
        );
        assert_public_call_queries(
            &fixture,
            stack.as_ref().unwrap(),
            "rust",
            "two_calls",
            "target",
            "fresh",
        );
    }
    if matches!(dependency, Some(RustFixtureDependency::Directory)) {
        let progress = fresh_activation_relation_batches(&fixture, "system.rust_target_progress");
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
        assert!(
            costs["phases"]
                .as_array()
                .unwrap()
                .iter()
                .any(|phase| phase["phase"] == "cargo-rustc"
                    && phase["elapsed_micros"].as_u64().unwrap() > 0)
        );
        eprintln!("Rust directory-source phase costs: {costs}");
        let names = progress
            .iter()
            .flat_map(|batch| {
                batch
                    .column_by_name("target_name")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<arrow::array::StringArray>()
                    .unwrap()
                    .iter()
                    .flatten()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(names, BTreeSet::from(["fixture"]));
        assert!(
            entities
                .iter()
                .any(|(language, name)| language == "rust" && name.ends_with("helper::increment")),
            "{entities:?}"
        );
        let selected = wait_for_semantic_activation(&fixture);
        supervisor.stop();
        let supervisor = fixture.start_supervisor();
        let reopened = wait_for_semantic_activation(&fixture);
        assert_eq!(selected.table_versions(), reopened.table_versions());
        assert_eq!(entities, canonical_entity_names(&fixture));
        supervisor.stop();
    } else {
        supervisor.stop();
    }
}

#[cfg(target_os = "linux")]
fn assert_structured_rust_failure_diagnostics(fixture: &ProductionFixture) {
    let canonical = canonical_rust_diagnostic_messages(fixture);
    assert_eq!(canonical, rust_diagnostic_messages(fixture));
    assert!(
        canonical
            .iter()
            .any(|(code, severity, message)| code == "E0425"
                && severity == "error"
                && message.contains("missing_function")),
        "the broken target must retain its structured compiler diagnostic"
    );
}

#[cfg(target_os = "linux")]
fn canonical_rust_diagnostic_messages(
    fixture: &ProductionFixture,
) -> Vec<(String, String, String)> {
    let mut diagnostics = Vec::new();
    for batch in fresh_activation_relation_batches(fixture, "fact.code_diagnostic") {
        let strings = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .unwrap()
        };
        for row in 0..batch.num_rows() {
            if strings("language").value(row) == "rust" {
                diagnostics.push((
                    strings("code").value(row).to_owned(),
                    strings("severity").value(row).to_owned(),
                    strings("message").value(row).to_owned(),
                ));
                assert_eq!(
                    strings("authority").value(row),
                    "rustc-structured-diagnostic"
                );
                assert!(!batch.column_by_name("diagnostic_id").unwrap().is_null(row));
            }
        }
    }
    diagnostics.sort();
    diagnostics
}

#[cfg(target_os = "linux")]
fn rust_diagnostic_messages(fixture: &ProductionFixture) -> Vec<(String, String, String)> {
    let mut diagnostics = Vec::new();
    for batch in fresh_activation_relation_batches(fixture, "provider.rustc.diagnostic.v1") {
        let strings = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .unwrap()
        };
        for row in 0..batch.num_rows() {
            diagnostics.push((
                strings("reason_code").value(row).to_owned(),
                strings("severity").value(row).to_owned(),
                strings("message").value(row).to_owned(),
            ));
        }
    }
    diagnostics.sort();
    diagnostics
}

#[test]
#[cfg(target_os = "linux")]
fn pragmatic_rust_virtual_workspace_inherits_package_settings() {
    let fixture = ProductionFixture::new();
    let workspace = Path::new(&fixture.workspace.root_path_display);
    fs::create_dir_all(workspace.join("app/src")).unwrap();
    fs::create_dir_all(workspace.join("helper/src")).unwrap();
    for (path, bytes) in [
        (
            "Cargo.toml",
            "[workspace]\nmembers = ['app', 'helper']\nresolver = '3'\n[workspace.package]\nversion = '0.1.0'\nedition = '2024'\n",
        ),
        (
            "Cargo.lock",
            "version = 4\n[[package]]\nname = 'fixture'\nversion = '0.1.0'\ndependencies = ['helper']\n[[package]]\nname = 'helper'\nversion = '0.1.0'\n",
        ),
        (
            "app/Cargo.toml",
            "[package]\nname = 'fixture'\nversion.workspace = true\nedition.workspace = true\n[dependencies]\nhelper = { path = '../helper' }\n",
        ),
        (
            "app/src/lib.rs",
            "pub fn caller() -> u32 { helper::increment(1) }\n",
        ),
        (
            "helper/Cargo.toml",
            "[package]\nname = 'helper'\nversion.workspace = true\nedition.workspace = true\n",
        ),
        (
            "helper/src/lib.rs",
            "pub fn increment(value: u32) -> u32 { value + 1 }\n",
        ),
    ] {
        fs::write(workspace.join(path), bytes).unwrap();
    }
    let supervisor = fixture.start_supervisor();
    let calls = fresh_activation_relation_batches(&fixture, "provider.rustc.call.v1");
    assert!(calls.iter().any(|batch| {
        batch
            .column_by_name("declared_target")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .unwrap()
            .iter()
            .flatten()
            .any(|target| target.ends_with("helper::increment"))
    }));
    let progress = fresh_activation_relation_batches(&fixture, "system.rust_target_progress");
    assert_eq!(progress.iter().map(RecordBatch::num_rows).sum::<usize>(), 2);
    assert!(progress.iter().all(|batch| {
        batch
            .column_by_name("processing_state")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .unwrap()
            .iter()
            .all(|value| value == Some("processed"))
    }));
    supervisor.stop();
}

fn assert_rust_syntax_survives_compilation_failure(fixture: &ProductionFixture) {
    use arrow::array::{BinaryArray, BooleanArray, Decimal128Array, StringArray};
    let broken = blake3::hash(b"fn main() { missing_function(); }\n");
    let malformed = blake3::hash("// π\r\nfn unfinished( {".as_bytes());
    let mut located_identifier = false;
    let mut retained_error = false;
    for batch in fresh_activation_relation_batches(fixture, "provider.tree_sitter_rust.cst_node") {
        assert!(batch.column_by_name("python_target_major").is_none());
        let strings = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
        };
        let binary = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap()
        };
        let numbers = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<Decimal128Array>()
                .unwrap()
        };
        let errors = batch
            .column_by_name("error")
            .unwrap()
            .as_any()
            .downcast_ref::<BooleanArray>()
            .unwrap();
        for row in 0..batch.num_rows() {
            assert_eq!(strings("provider_id").value(row), "tree-sitter-rust");
            assert_eq!(
                binary("analysis_context_id").value(row),
                codefabric::identity::SOURCE_CONTEXT_ID
            );
            if binary("content_digest").value(row) == broken.as_bytes()
                && strings("raw_kind").value(row) == "identifier"
                && numbers("start_byte").value(row) == 12
                && numbers("end_byte").value(row) == 28
            {
                located_identifier = true;
            }
            retained_error |=
                binary("content_digest").value(row) == malformed.as_bytes() && errors.value(row);
        }
    }
    assert!(
        located_identifier,
        "compiler-failing file retains the exact identifier occurrence"
    );
    assert!(
        retained_error,
        "malformed UTF-8/CRLF source retains syntax recovery observations"
    );
}

fn fresh_activation_relation_batches(
    fixture: &ProductionFixture,
    relation: &str,
) -> Vec<RecordBatch> {
    let selected = wait_for_semantic_activation(fixture);
    selected_relation_batches(&selected, relation)
}

fn wait_for_semantic_activation(fixture: &ProductionFixture) -> PersistedActivationControlRow {
    use arrow::array::{Array as _, BooleanArray};
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let selected = all_activation_control_rows(fixture)
            .into_iter()
            .max_by_key(|row| row.row().ordinal.get())
            .expect("durable activation");
        let state = selected_relation_batches(&selected, "source.input_inventory_state");
        let rows = state.iter().map(RecordBatch::num_rows).sum::<usize>();
        assert_eq!(rows, 1, "one exact source inventory state");
        let pending = state
            .iter()
            .find(|batch| batch.num_rows() == 1)
            .unwrap()
            .column_by_name("semantic_pending")
            .is_some_and(|column| {
                let values = column.as_any().downcast_ref::<BooleanArray>().unwrap();
                assert!(!values.is_null(0));
                values.value(0)
            });
        if !pending {
            return selected;
        }
        assert!(
            Instant::now() < deadline,
            "semantic activation did not converge"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

fn selected_relation_batches(
    selected: &PersistedActivationControlRow,
    relation: &str,
) -> Vec<RecordBatch> {
    let (_, pin) = selected
        .table_versions()
        .components()
        .find(|(id, _)| *id == relation)
        .unwrap_or_else(|| panic!("missing activated relation {relation}"));
    assert_eq!(
        pin.version(),
        1,
        "fresh fixture relation has one data commit"
    );
    let root = pin.canonical_root().to_file_path().unwrap();
    let log = fs::File::open(root.join("_delta_log/00000000000000000001.json")).unwrap();
    let mut batches = Vec::new();
    for action in BufReader::new(log).lines() {
        let action: Value = serde_json::from_str(&action.unwrap()).unwrap();
        if let Some(path) = action
            .get("add")
            .and_then(|add| add.get("path"))
            .and_then(Value::as_str)
        {
            let reader =
                ParquetRecordBatchReaderBuilder::try_new(fs::File::open(root.join(path)).unwrap())
                    .unwrap()
                    .build()
                    .unwrap();
            batches.extend(reader.map(Result::unwrap));
        }
    }
    batches
}

fn assert_mixed_public_entity_queries(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
) {
    let mut python = semantic_request(
        &fixture.workspace.public_id(),
        "request:canonical-python",
        "function declarations",
    );
    python["scope"]["languages"] = json!(["python"]);
    python["queries"][0]["return"]["limit"]["maximum_results"] = json!(1);
    let rust = semantic_request(
        &fixture.workspace.public_id(),
        "request:canonical-rust",
        "Rust function declarations",
    );
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "python", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": python, "delivery": "resource"}},
            {"id": "python_manifest", "operation": "read_resource", "uri": {"$ref": "python.structured_content.manifest.uri"}},
            {"id": "python_page", "operation": "read_resource", "uri": {"$ref": "python.structured_content.pages.0.uri"}},
            {"id": "rust", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": rust, "delivery": "resource"}},
            {"id": "rust_manifest", "operation": "read_resource", "uri": {"$ref": "rust.structured_content.manifest.uri"}},
            {"id": "rust_page", "operation": "read_resource", "uri": {"$ref": "rust.structured_content.pages.0.uri"}},
        ]),
    );
    let path = write_modern_client_scenario(fixture, "canonical-mixed", &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let bytes = |step: &str| {
        STANDARD
            .decode(modern_step(&report, step)[0]["blob"].as_str().unwrap())
            .unwrap()
    };
    let python_manifest: Value = serde_json::from_slice(&bytes("python_manifest")).unwrap();
    let rust_manifest: Value = serde_json::from_slice(&bytes("rust_manifest")).unwrap();
    assert_eq!(
        modern_structured(modern_step(&report, "python"))["execution_state"],
        "SUCCEEDED",
        "{report}"
    );
    assert_eq!(
        modern_structured(modern_step(&report, "rust"))["execution_state"],
        "SUCCEEDED",
        "{report}"
    );
    let python_scope = &python_manifest["processing"][0]["processing"];
    let rust_scope = &rust_manifest["processing"][0]["processing"];
    let public_python = &modern_structured(modern_step(&report, "python"))["processing"][0];
    let public_rust = &modern_structured(modern_step(&report, "rust"))["processing"][0];
    assert_eq!(public_python["remaining_partitions"], 0);
    assert_eq!(public_python["maximum_rows"], 1);
    assert_eq!(public_python["additional_rows"], true);
    assert_eq!(public_rust["remaining_partitions"], 1);
    assert_eq!(public_rust["remainder"][0]["target"], "broken");
    assert_eq!(public_rust["remainder"][0]["state"], "unavailable");
    assert_eq!(public_rust["remainder"][0]["target_kind"], "binary");
    assert_eq!(
        public_rust["source_generation"],
        rust_scope["source_generation"]
    );
    assert_eq!(public_rust["additional_rows"], false);
    assert_eq!(python_scope["requested_partitions"], 1, "{python_manifest}");
    assert_eq!(python_scope["remaining_partitions"], 0, "{python_manifest}");
    assert_eq!(
        python_manifest["relations"][0]["coverage_state"], "complete",
        "{python_manifest}"
    );
    assert_eq!(rust_scope["requested_partitions"], 3, "{rust_manifest}");
    assert_eq!(rust_scope["remaining_partitions"], 1, "{rust_manifest}");
    assert_eq!(
        rust_scope["remainder"][0]["target"], "broken",
        "{rust_manifest}"
    );
    assert_eq!(
        rust_manifest["relations"][0]["coverage_state"], "partial",
        "{rust_manifest}"
    );
    let names = |step: &str, expected_language: &str| {
        let reader =
            arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes(step)), None)
                .unwrap();
        let mut names = BTreeSet::new();
        for batch in reader {
            let batch = batch.unwrap();
            let languages = batch
                .column_by_name("language")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .unwrap();
            let column = batch
                .column_by_name("name")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .unwrap();
            assert!(
                languages
                    .iter()
                    .all(|value| value == Some(expected_language))
            );
            names.extend(column.iter().flatten().map(ToOwned::to_owned));
        }
        names
    };
    assert_eq!(
        names("python_page", "python"),
        BTreeSet::from(["answer".to_owned()])
    );
    assert!(
        names("rust_page", "rust")
            .iter()
            .any(|name| name.ends_with("caller"))
    );
    assert_mixed_public_declaration_facts(fixture, stack);
}

fn assert_public_declaration_kinds(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    language: &str,
    cases: &[(&str, &str, &[&str])],
) {
    use arrow::array::StringArray;
    let mut steps = Vec::new();
    for (ordinal, (phrase, _, _)) in cases.iter().enumerate() {
        let request = semantic_request(
            &fixture.workspace.public_id(),
            &format!("request:declaration-kind-{ordinal}"),
            phrase,
        );
        steps.push(json!({"id": format!("kind{ordinal}"), "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": request, "delivery": "resource"}}));
        steps.push(json!({"id": format!("page{ordinal}"), "operation": "read_resource", "uri": {"$ref": format!("kind{ordinal}.structured_content.pages.0.uri")}}));
    }
    let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
    let path = write_modern_client_scenario(fixture, "canonical-declaration-kinds", &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let mut subjects = BTreeSet::new();
    for (ordinal, (_, kind, expected)) in cases.iter().enumerate() {
        let response = modern_structured(modern_step(&report, &format!("kind{ordinal}")));
        assert_eq!(response["execution_state"], "SUCCEEDED", "{response}");
        assert_eq!(response["processing"][0]["family"], "declarations");
        assert_eq!(response["processing"][0]["languages"], json!([language]));
        assert_eq!(response["processing"][0]["additional_rows"], false);
        assert_eq!(
            response["processing"][0]["remaining_partitions"],
            if language == "python" { 0 } else { 1 }
        );
        let bytes = STANDARD
            .decode(
                modern_step(&report, &format!("page{ordinal}"))[0]["blob"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
        let reader =
            arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None).unwrap();
        let mut names = BTreeSet::new();
        for batch in reader {
            let batch = batch.unwrap();
            let text = |name| {
                batch
                    .column_by_name(name)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
            };
            for row in 0..batch.num_rows() {
                assert_eq!(text("entity_kind").value(row), *kind);
                assert_eq!(text("language").value(row), language);
                let public_id = text("public_entity_id").value(row);
                subjects.insert(public_id.to_owned());
                assert!(
                    codefabric::identity::decode_public_id(
                        codefabric::identity::IdentityDomain::Entity,
                        Some(kind),
                        public_id,
                    )
                    .is_ok(),
                    "invalid reusable public entity ID: {public_id}"
                );
                names.insert(
                    text("name")
                        .value(row)
                        .rsplit("::")
                        .next()
                        .unwrap()
                        .to_owned(),
                );
            }
        }
        assert_eq!(
            names,
            expected.iter().map(|name| (*name).to_owned()).collect(),
            "{kind}"
        );
    }
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "request:found-declaration-facts",
        "unused",
    );
    request["queries"] = json!([{
        "request": "retrieve facts about code", "query_id": "facts",
        "about": subjects.iter().map(|id| json!({"entity_id": id})).collect::<Vec<_>>(),
        "facts": ["declaration locations and provenance"],
        "return": {"limit": {"maximum_results": 64}}
    }]);
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "facts", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": request, "delivery": "resource"}},
            {"id": "page", "operation": "read_resource", "uri": {"$ref": "facts.structured_content.pages.0.uri"}}
        ]),
    );
    let path = write_modern_client_scenario(fixture, "found-declaration-facts", &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    assert_eq!(
        modern_structured(modern_step(&report, "facts"))["execution_state"],
        "SUCCEEDED",
        "{report}"
    );
    let bytes = STANDARD
        .decode(modern_step(&report, "page")[0]["blob"].as_str().unwrap())
        .unwrap();
    let mut found = BTreeSet::new();
    for batch in
        arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None).unwrap()
    {
        let batch = batch.unwrap();
        let ids = batch
            .column_by_name("public_entity_id")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        found.extend(ids.iter().flatten().map(ToOwned::to_owned));
    }
    assert_eq!(
        found, subjects,
        "public find results must be reusable as fact subjects"
    );
}

fn assert_mixed_public_declaration_facts(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
) {
    use arrow::array::{Array, StringArray, UInt64Array};
    let mut subjects = BTreeSet::new();
    for batch in fresh_activation_relation_batches(fixture, "fact.code_declaration") {
        let names = batch
            .column_by_name("name")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let ids = batch
            .column_by_name("public_entity_id")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        for row in 0..batch.num_rows() {
            if (names.value(row) == "answer" || names.value(row).ends_with("caller"))
                && !ids.is_null(row)
            {
                subjects.insert(ids.value(row).to_owned());
            }
        }
    }
    assert!(subjects.len() >= 2);
    let mut about = subjects
        .iter()
        .map(|id| json!({"entity_id": id}))
        .collect::<Vec<_>>();
    about.push(about[0].clone()); // A repeated subject must not duplicate its facts.
    let mut request = semantic_request(
        &fixture.workspace.public_id(),
        "request:declaration-facts",
        "unused",
    );
    request["queries"] = json!([{
        "request": "retrieve facts about code", "query_id": "facts",
        "about": about, "facts": ["declaration locations and provenance"],
        "return": {"limit": {"maximum_results": 32}}
    }]);
    let mut empty = request.clone();
    empty["semantic_request_id"] = json!("request:declaration-empty");
    empty["scope"]["languages"] = json!(["python"]);
    empty["queries"][0]["about"] =
        json!([{"entity_id": "entity:function:01010101010101010101010101010101"}]);
    let mut unresolved = request.clone();
    unresolved["semantic_request_id"] = json!("request:declaration-unresolved");
    unresolved["queries"][0]["about"] = json!(["answer"]);
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "facts", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": request, "delivery": "resource"}},
            {"id": "page", "operation": "read_resource", "uri": {"$ref": "facts.structured_content.pages.0.uri"}},
            {"id": "empty", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": empty, "delivery": "resource"}},
            {"id": "empty_page", "operation": "read_resource", "uri": {"$ref": "empty.structured_content.pages.0.uri"}},
            {"id": "unresolved", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": unresolved, "delivery": "resource"}}
        ]),
    );
    let path = write_modern_client_scenario(fixture, "canonical-facts", &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    let result = modern_structured(modern_step(&report, "facts"));
    assert_eq!(result["execution_state"], "SUCCEEDED", "{report}");
    assert_eq!(result["processing"][0]["family"], "declarations");
    assert_eq!(result["processing"][0]["remaining_partitions"], 1);
    assert_eq!(result["processing"][0]["additional_rows"], false);
    let batches = |step: &str| {
        let bytes = STANDARD
            .decode(modern_step(&report, step)[0]["blob"].as_str().unwrap())
            .unwrap();
        arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None)
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>()
    };
    let mut seen = BTreeSet::new();
    let mut languages = BTreeSet::new();
    for batch in batches("page") {
        let string = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
        };
        let number = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
        };
        for row in 0..batch.num_rows() {
            assert!(seen.insert(string("public_entity_id").value(row).to_owned()));
            let language = string("language").value(row);
            languages.insert(language.to_owned());
            assert_eq!(string("identity_state").value(row), "canonical");
            assert_eq!(
                string("provider").value(row),
                if language == "python" {
                    "ruff"
                } else {
                    "rustc"
                }
            );
            assert_eq!(
                (
                    number("start_byte").value(row),
                    number("end_byte").value(row)
                ),
                if language == "python" {
                    (4, 10)
                } else {
                    (11, 33)
                }
            );
            assert_eq!(
                batch.column_by_name("content_digest").unwrap().null_count(),
                0
            );
        }
    }
    assert_eq!(seen, subjects);
    assert_eq!(
        languages,
        BTreeSet::from(["python".to_owned(), "rust".to_owned()])
    );
    assert_eq!(
        batches("empty_page")
            .iter()
            .map(RecordBatch::num_rows)
            .sum::<usize>(),
        0
    );
    let empty = modern_structured(modern_step(&report, "empty"));
    assert_eq!(empty["processing"][0]["remaining_partitions"], 0);
    assert_eq!(empty["processing"][0]["additional_rows"], false);
    let unresolved = modern_structured(modern_step(&report, "unresolved"));
    assert_eq!(unresolved["outcome"], "validation_rejection");
    assert_eq!(unresolved["error"]["code"], "VALIDATION_REJECTED");
    assert_eq!(
        unresolved["issues"][0]["presentation_key"],
        "query.validation.semantic_reference_unavailable"
    );
    {
        let mut store = OperationalStore::open(&fixture.state.join("operational.sqlite3")).unwrap();
        WorkspaceRegistry::new(&mut store)
            .set_source_disclosure(fixture.workspace.workspace_id, true)
            .unwrap();
    }
    let mut source = semantic_request(
        &fixture.workspace.public_id(),
        "request:mixed-source",
        "unused",
    );
    source["queries"] = json!([{
        "request": "retrieve source and syntax context", "query_id": "source",
        "about": subjects.iter().map(|id| json!({"entity_id": id})).collect::<Vec<_>>(),
        "context": "exact source span", "return": {"maximum_source_bytes": 1024, "limit": {"maximum_results": 32}}
    }]);
    let mut empty_source = source.clone();
    empty_source["semantic_request_id"] = json!("request:mixed-source-empty");
    empty_source["queries"][0]["about"] =
        json!([{"entity_id": "entity:function:01010101010101010101010101010101"}]);
    let scenario = modern_client_scenario(
        fixture,
        stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "source", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": source, "delivery": "resource"}},
            {"id": "page", "operation": "read_resource", "uri": {"$ref": "source.structured_content.pages.0.uri"}},
            {"id": "empty", "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": empty_source, "delivery": "resource"}},
            {"id": "empty_page", "operation": "read_resource", "uri": {"$ref": "empty.structured_content.pages.0.uri"}}
        ]),
    );
    let path = write_modern_client_scenario(fixture, "mixed-source", &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    assert_eq!(
        modern_structured(modern_step(&report, "source"))["processing"][0]["remaining_partitions"],
        1
    );
    let mut contexts = BTreeMap::new();
    let mut source_subjects = BTreeSet::new();
    for id in ["page", "empty_page"] {
        let bytes = STANDARD
            .decode(modern_step(&report, id)[0]["blob"].as_str().unwrap())
            .unwrap();
        for batch in
            arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None).unwrap()
        {
            let batch = batch.unwrap();
            if id == "empty_page" {
                assert_eq!(batch.num_rows(), 0);
                continue;
            }
            assert!(batch.column_by_name("source_bytes").is_none());
            let languages = batch
                .column_by_name("language")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let context = batch
                .column_by_name("source_context")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow::array::StructArray>()
                .unwrap();
            let text = context
                .column_by_name("text")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let ids = batch
                .column_by_name("public_entity_id")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            for row in 0..batch.num_rows() {
                assert!(source_subjects.insert(ids.value(row).to_owned()));
                contexts
                    .entry(languages.value(row).to_owned())
                    .or_insert_with(BTreeSet::new)
                    .insert(text.value(row).to_owned());
            }
        }
    }
    assert_eq!(source_subjects, subjects);
    assert_eq!(
        contexts,
        BTreeMap::from([
            ("python".to_owned(), BTreeSet::from(["answer".to_owned()])),
            (
                "rust".to_owned(),
                BTreeSet::from(["pub fn caller() -> u32".to_owned()])
            ),
        ])
    );
}

/// Select compiler contexts by their actual target processing state.
fn rust_target_contexts(fixture: &ProductionFixture, state: &str) -> BTreeSet<Vec<u8>> {
    use arrow::array::{Array as _, BinaryArray, StringArray};
    let mut contexts = BTreeSet::new();
    for batch in fresh_activation_relation_batches(fixture, "system.rust_target_progress") {
        let states = batch
            .column_by_name("processing_state")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let ids = batch
            .column_by_name("context_id")
            .unwrap()
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        for row in 0..batch.num_rows() {
            if states.value(row) == state && !ids.is_null(row) {
                contexts.insert(ids.value(row).to_vec());
            }
        }
    }
    contexts
}

/// Actual provider calls through the installed modern client, including exact restart.
fn assert_public_call_queries(
    fixture: &ProductionFixture,
    stack: &InstalledProductionStack,
    language: &str,
    caller_name: &str,
    target_name: &str,
    phase: &str,
) {
    use arrow::array::{Array, BinaryArray, StringArray, UInt64Array};
    let mut subjects = std::collections::BTreeMap::new();
    for batch in fresh_activation_relation_batches(fixture, "fact.code_declaration") {
        let text = |name| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
        };
        for row in 0..batch.num_rows() {
            if text("language").value(row) == language && !text("public_entity_id").is_null(row) {
                let name = text("name").value(row).rsplit("::").next().unwrap();
                let contexts = batch
                    .column_by_name("context_id")
                    .unwrap()
                    .as_any()
                    .downcast_ref::<BinaryArray>()
                    .unwrap();
                subjects.insert(
                    (contexts.value(row).to_vec(), name.to_owned()),
                    text("public_entity_id").value(row).to_owned(),
                );
            }
        }
    }
    // Multiple Cargo targets intentionally have distinct semantic identities. A failed parent
    // can retain positive library facts, but cannot satisfy this complete-target expectation.
    let processed = (language == "rust").then(|| rust_target_contexts(fixture, "processed"));
    let context_bytes = subjects
        .keys()
        .find(|(context, name)| {
            name == caller_name
                && processed
                    .as_ref()
                    .is_none_or(|contexts| contexts.contains(context))
        })
        .unwrap()
        .0
        .clone();
    let caller = &subjects[&(context_bytes.clone(), caller_name.to_owned())];
    let target = &subjects[&(context_bytes.clone(), target_name.to_owned())];
    let indirect = &subjects[&(context_bytes.clone(), "indirect".to_owned())];
    let mut request = semantic_request(&fixture.workspace.public_id(), "request:calls", "unused");
    request["scope"]["languages"] = json!([language]);
    request["queries"] = json!([{
        "request": "follow code relationships", "query_id": "calls",
        "starting_from": [{"entity_id": caller}, {"entity_id": caller}],
        "relationship": "calls", "direction": "outgoing", "distance": "one relationship step",
        "return": {"limit": {"maximum_results": 64}}
    }]);
    let mut requests = vec![("outgoing", request.clone())];
    let failed_subjects = if language == "rust" {
        let unavailable = rust_target_contexts(fixture, "unavailable");
        let failed_context = &subjects
            .keys()
            .find(|(context, name)| name == caller_name && unavailable.contains(context))
            .expect("the failed binary retains its successfully compiled library unit")
            .0;
        let failed_caller = &subjects[&(failed_context.clone(), caller_name.to_owned())];
        let failed_target = &subjects[&(failed_context.clone(), target_name.to_owned())];
        let mut selected = request.clone();
        selected["queries"][0]["starting_from"] = json!([{"entity_id": failed_caller}]);
        requests.push(("failed_parent", selected));
        Some((failed_caller, failed_target))
    } else {
        None
    };
    let mut default = request.clone();
    default["queries"][0]
        .as_object_mut()
        .unwrap()
        .remove("direction");
    default["queries"][0]["distance"] = json!("one step");
    requests.push(("default", default));
    let mut incoming = request.clone();
    incoming["queries"][0]["starting_from"] = json!([{"entity_id": target}]);
    incoming["queries"][0]["direction"] = json!("incoming");
    requests.push(("incoming", incoming));
    let mut dynamic = request.clone();
    dynamic["queries"][0]["starting_from"] = json!([{"entity_id": indirect}]);
    requests.push(("dynamic", dynamic));
    let mut empty = request.clone();
    empty["queries"][0]["starting_from"] =
        json!([{"entity_id": "entity:function:01010101010101010101010101010101"}]);
    requests.push(("empty", empty));
    let mut limited = request.clone();
    limited["queries"][0]["return"]["limit"]["maximum_results"] = json!(1);
    requests.push(("limited", limited));
    let mut other_language = request.clone();
    other_language["scope"]["languages"] = json!([if language == "python" {
        "rust"
    } else {
        "python"
    }]);
    requests.push(("other_language", other_language));
    let mut context = request.clone();
    let context_id = codefabric::identity::encode_public_id(
        codefabric::identity::IdentityDomain::AnalysisContext,
        None,
        context_bytes.try_into().unwrap(),
    )
    .unwrap();
    context["scope"]["analysis_contexts"] =
        json!({"mode": "explicit", "context_ids": [context_id]});
    requests.push(("context", context));
    if language == "rust" {
        let mut references = request.clone();
        references["queries"][0]["relationship"] = json!("lexical references");
        references["queries"][0]["direction"] = json!("incoming");
        requests.push(("unsupported_references", references));
    }
    let mut steps = Vec::new();
    for (id, mut selected) in requests {
        selected["semantic_request_id"] = json!(format!("request:calls-{phase}-{id}"));
        steps.push(json!({"id": id, "operation": "call_tool", "name": "query_code_graph", "arguments": {"request": selected, "delivery": "resource"}}));
        steps.push(json!({"id": format!("{id}_page"), "operation": "read_resource", "uri": {"$ref": format!("{id}.structured_content.pages.0.uri")}}));
    }
    let mut unsupported = request;
    unsupported["semantic_request_id"] =
        json!(format!("request:calls-{phase}-unsupported-distance"));
    unsupported["queries"][0]["distance"] = json!("two steps");
    steps.push(json!({"id": "unsupported", "operation": "call_tool", "name": "validate_code_graph_query", "arguments": {"request": unsupported}}));
    let scenario = modern_client_scenario(fixture, stack, "policy-one", json!([]), json!(steps));
    let path = write_modern_client_scenario(fixture, "canonical-calls", &scenario);
    let report = modern_client_report(&run_modern_client(stack, &path));
    if language == "rust" {
        let result = modern_structured(modern_step(&report, "unsupported_references"));
        assert_eq!(result["execution_state"], "SUCCEEDED", "{result}");
        assert_eq!(result["total_rows"], 0);
        assert_eq!(result["processing"][0]["family"], "lexical-references");
        assert_eq!(result["processing"][0]["completed_partitions"], 0);
        assert!(
            result["processing"][0]["remaining_partitions"]
                .as_u64()
                .unwrap()
                > 0
        );
        for row in result["processing"][0]["remainder"].as_array().unwrap() {
            assert_eq!(row["state"], "unsupported");
            assert_eq!(
                row["reason_code"],
                "rust_lexical_references_unimplemented"
            );
        }
    }
    let rows = |id: &str| {
        let result = modern_structured(modern_step(&report, id));
        assert_eq!(result["execution_state"], "SUCCEEDED", "{id}: {report}");
        assert_eq!(
            result["processing"][0]["family"], "call-targets",
            "{id}: {report}"
        );
        let bytes = STANDARD
            .decode(
                modern_step(&report, &format!("{id}_page"))[0]["blob"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
        let reader =
            arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None).unwrap();
        let mut rows = Vec::new();
        for batch in reader {
            let batch = batch.unwrap();
            let text = |name| {
                batch
                    .column_by_name(name)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
            };
            for row in 0..batch.num_rows() {
                assert_eq!(text("language").value(row), language);
                let value =
                    |name| (!text(name).is_null(row)).then(|| text(name).value(row).to_owned());
                let number = |name| {
                    let values = batch
                        .column_by_name(name)
                        .unwrap()
                        .as_any()
                        .downcast_ref::<UInt64Array>()
                        .unwrap();
                    (!values.is_null(row)).then(|| values.value(row))
                };
                rows.push((
                    value("public_caller_entity_id"),
                    value("public_target_entity_id"),
                    value("public_call_site_id"),
                    number("start_byte"),
                    number("end_byte"),
                    value("unknown_reason"),
                ));
            }
        }
        rows
    };
    let outgoing = rows("outgoing");
    assert_eq!(
        outgoing.len(),
        2,
        "repeated subjects must not duplicate call occurrences: {outgoing:?}"
    );
    assert_eq!(outgoing, rows("default"));
    assert_eq!(outgoing, rows("context"));
    assert_ne!(outgoing[0].2, outgoing[1].2);
    for row in &outgoing {
        assert_eq!(row.0.as_ref(), Some(caller));
        assert_eq!(row.1.as_ref(), Some(target));
        assert!(row.2.as_ref().unwrap().starts_with("entity:call-site:"));
        assert!(row.3.unwrap() < row.4.unwrap());
        assert!(row.5.is_none());
    }
    let incoming = rows("incoming");
    assert_eq!(
        incoming
            .iter()
            .filter(|row| row.0.as_ref() == Some(caller))
            .count(),
        2
    );
    assert!(incoming.iter().all(|row| row.1.as_ref() == Some(target)));
    let dynamic = rows("dynamic");
    assert_eq!(dynamic.len(), 1);
    assert_eq!(dynamic[0].0.as_ref(), Some(indirect));
    assert!(dynamic[0].1.is_none());
    assert!(dynamic[0].5.is_some());
    assert!(rows("empty").is_empty());
    assert!(rows("other_language").is_empty());
    assert_eq!(rows("limited"), outgoing[..1]);
    assert_eq!(
        modern_structured(modern_step(&report, "limited"))["processing"][0]["additional_rows"],
        true
    );
    let coverage = modern_structured(modern_step(&report, "outgoing"));
    if let Some((failed_caller, failed_target)) = failed_subjects {
        let retained = rows("failed_parent");
        assert_eq!(retained.len(), 2);
        assert!(
            retained
                .iter()
                .all(|row| row.0.as_ref() == Some(failed_caller)
                    && row.1.as_ref() == Some(failed_target))
        );
        let failed = modern_structured(modern_step(&report, "failed_parent"));
        assert_eq!(failed["processing"][0]["remaining_partitions"], 1);
        assert_eq!(
            failed["processing"][0]["remainder"][0]["state"],
            "unavailable"
        );
        assert_eq!(
            failed["processing"][0]["remainder"][0]["entity_id"],
            *failed_caller
        );
    }
    if language == "rust" {
        assert_eq!(coverage["processing"][0]["requested_partitions"], 1);
        assert_eq!(coverage["processing"][0]["remaining_partitions"], 0);
        let dynamic = modern_structured(modern_step(&report, "dynamic"));
        assert_eq!(dynamic["processing"][0]["remaining_partitions"], 1);
        assert_eq!(
            dynamic["processing"][0]["remainder"][0]["entity_id"],
            *indirect
        );
        assert!(modern_structured(modern_step(&report, "incoming"))["processing"][0]["remaining_partitions"].as_u64().unwrap() > 0);
    } else {
        assert!(
            coverage["processing"][0]["remaining_partitions"]
                .as_u64()
                .unwrap()
                > 0
        );
    }
    assert_eq!(coverage["processing"][0]["additional_rows"], false);
    assert_ne!(
        modern_structured(modern_step(&report, "unsupported"))["execution_state"],
        "SUCCEEDED"
    );
}

fn assert_canonical_rust_calls(fixture: &ProductionFixture, expect_indirect: bool) {
    use arrow::array::{Array, BinaryArray, Decimal128Array, StringArray};
    let mut entities = std::collections::BTreeMap::new();
    for batch in fresh_activation_relation_batches(fixture, "fact.code_entity") {
        let names = batch
            .column_by_name("name")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let ids = batch
            .column_by_name("entity_id")
            .unwrap()
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        let contexts = batch
            .column_by_name("context_id")
            .unwrap()
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        for row in 0..batch.num_rows() {
            entities.insert(
                (contexts.value(row).to_vec(), names.value(row).to_owned()),
                ids.value(row).to_vec(),
            );
        }
    }
    let mut direct = false;
    let mut indirect = false;
    let mut macro_unmapped = false;
    let mut dependency_target = false;
    let mut repeated = std::collections::BTreeMap::<Vec<u8>, Vec<(Vec<u8>, Vec<u8>)>>::new();
    for batch in fresh_activation_relation_batches(fixture, "fact.code_call_site") {
        let text = |name: &str| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
        };
        let binary = |name: &str| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap()
        };
        let number = |name: &str| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<Decimal128Array>()
                .unwrap()
        };
        for row in 0..batch.num_rows() {
            if text("caller_name").value(row).ends_with("caller") {
                let target_name = text("target_name").value(row);
                assert!(target_name.ends_with("other::target"));
                let context = binary("context_id").value(row).to_vec();
                assert_eq!(
                    binary("target_entity_id").value(row),
                    entities[&(context.clone(), target_name.to_owned())]
                );
                assert_eq!(
                    binary("caller_entity_id").value(row),
                    entities[&(context, text("caller_name").value(row).to_owned())]
                );
                assert!(!binary("call_site_id").is_null(row));
                assert_ne!(
                    binary("call_site_id").value(row),
                    binary("caller_entity_id").value(row)
                );
                assert_ne!(
                    binary("call_site_id").value(row),
                    binary("target_entity_id").value(row)
                );
                assert_eq!(
                    (
                        number("start_byte").value(row),
                        number("end_byte").value(row)
                    ),
                    (36, 52)
                );
                assert_eq!(text("source_mapping").value(row), "exact_call_expression");
                assert_eq!(text("dispatch_kind").value(row), "direct");
                assert_eq!(text("resolution").value(row), "resolved_declaration");
                assert!(text("unknown_reason").is_null(row));
                assert_eq!(number("argument_count").value(row), 1);
                direct = true;
            }
            if text("caller_name").value(row).ends_with("indirect") {
                assert!(!binary("call_site_id").is_null(row));
                assert!(binary("target_entity_id").is_null(row));
                assert_eq!(text("dispatch_kind").value(row), "function-pointer");
                assert_eq!(
                    text("unknown_reason").value(row),
                    "indirect_target_unresolved"
                );
                indirect = true;
            }
            if text("caller_name").value(row).ends_with("two_calls") {
                assert!(!binary("call_site_id").is_null(row));
                assert!(!binary("target_entity_id").is_null(row));
                repeated
                    .entry(binary("context_id").value(row).to_vec())
                    .or_default()
                    .push((
                        binary("call_site_id").value(row).to_vec(),
                        binary("target_entity_id").value(row).to_vec(),
                    ));
            }
            if text("caller_name").value(row).ends_with("through_macro") {
                assert!(
                    binary("call_site_id").is_null(row),
                    "macro expansion must not be assigned a guessed syntax occurrence"
                );
                assert!(
                    !binary("target_entity_id").is_null(row),
                    "known target semantics survive missing source mapping"
                );
                assert_eq!(
                    text("unknown_reason").value(row),
                    "source_call_site_unmapped"
                );
                macro_unmapped = true;
            }
            if text("caller_name").value(row).ends_with("other::target")
                && text("raw_declared_target")
                    .value(row)
                    .ends_with("helper::increment")
            {
                assert!(!binary("call_site_id").is_null(row));
                assert!(!binary("target_entity_id").is_null(row));
                let key = (
                    binary("context_id").value(row).to_vec(),
                    text("target_name").value(row).to_owned(),
                );
                assert_eq!(binary("target_entity_id").value(row), entities[&key]);
                assert!(text("unknown_reason").is_null(row));
                dependency_target = true;
            }
        }
    }
    assert!(
        direct,
        "actual direct call normalized to canonical caller and target"
    );
    assert_eq!(indirect, expect_indirect);
    assert_eq!(macro_unmapped, expect_indirect);
    assert_eq!(dependency_target, !expect_indirect);
    assert_eq!(!repeated.is_empty(), expect_indirect);
    for calls in repeated.values() {
        assert_eq!(calls.len(), 2, "two actual calls in this context");
        assert_ne!(
            calls[0].0, calls[1].0,
            "distinct call sites survive a shared callee"
        );
        assert_eq!(calls[0].1, calls[1].1);
    }
}

fn assert_canonical_python_reference(fixture: &ProductionFixture) {
    use arrow::array::{BinaryArray, Decimal128Array, StringArray};
    let declarations = fresh_activation_relation_batches(fixture, "fact.code_declaration");
    let target = declarations
        .iter()
        .find_map(|batch| {
            let names = batch
                .column_by_name("name")?
                .as_any()
                .downcast_ref::<StringArray>()?;
            let kinds = batch
                .column_by_name("entity_kind")?
                .as_any()
                .downcast_ref::<StringArray>()?;
            let entities = batch
                .column_by_name("entity_id")?
                .as_any()
                .downcast_ref::<BinaryArray>()?;
            (0..batch.num_rows())
                .find(|&row| names.value(row) == "value" && kinds.value(row) == "parameter")
                .map(|row| entities.value(row).to_vec())
        })
        .expect("actual source parameter declaration");
    let mut found = 0;
    for batch in fresh_activation_relation_batches(fixture, "fact.code_reference") {
        let names = batch
            .column_by_name("name")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let targets = batch
            .column_by_name("target_entity_id")
            .unwrap()
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        let ids = batch
            .column_by_name("reference_id")
            .unwrap()
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        let resolution = batch
            .column_by_name("resolution")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let kinds = batch
            .column_by_name("reference_kind")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let starts = batch
            .column_by_name("start_byte")
            .unwrap()
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .unwrap();
        let ends = batch
            .column_by_name("end_byte")
            .unwrap()
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .unwrap();
        for row in 0..batch.num_rows() {
            if names.value(row) == "value" {
                assert_eq!(targets.value(row), target);
                assert_ne!(ids.value(row), target);
                assert_eq!(resolution.value(row), "resolved");
                let expected = match kinds.value(row) {
                    "read" => (42, 47),
                    "write" => (11, 16),
                    kind => panic!("unexpected parameter reference kind {kind}"),
                };
                assert_eq!((starts.value(row), ends.value(row)), expected);
                found += 1;
            }
        }
    }
    assert_eq!(
        found, 2,
        "the parameter definition writes its binding and return value + 1 reads it"
    );
}

fn canonical_entity_names(fixture: &ProductionFixture) -> BTreeSet<(String, String)> {
    use arrow::array::StringArray;
    let mut names = BTreeSet::new();
    for batch in fresh_activation_relation_batches(fixture, "fact.code_entity") {
        let language = batch
            .column_by_name("language")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let name = batch
            .column_by_name("name")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        for row in 0..batch.num_rows() {
            names.insert((language.value(row).to_owned(), name.value(row).to_owned()));
        }
    }
    names
}

fn assert_canonical_rust_declaration(fixture: &ProductionFixture) {
    use arrow::array::{BinaryArray, Decimal128Array, StringArray};
    let mut found = 0;
    for batch in fresh_activation_relation_batches(fixture, "fact.code_declaration") {
        let text = |name: &str| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
        };
        let bytes = |name: &str| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap()
        };
        let number = |name: &str| {
            batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<Decimal128Array>()
                .unwrap()
        };
        for row in 0..batch.num_rows() {
            if text("language").value(row) != "rust" || !text("name").value(row).ends_with("caller")
            {
                continue;
            }
            found += 1;
            assert_eq!(text("entity_kind").value(row), "function");
            assert_eq!(text("identity_state").value(row), "canonical");
            assert_eq!(text("provider").value(row), "rustc");
            assert_eq!(number("start_byte").value(row), 11);
            // rustc_public's item span is the declaration header, separate from MIR body span.
            assert_eq!(number("end_byte").value(row), 33);
            assert_eq!(
                bytes("content_digest").value(row),
                blake3::hash(b"mod other; pub fn caller() -> u32 { other::target(4) }\n")
                    .as_bytes()
            );
            assert_eq!(bytes("entity_id").value(row).len(), 16);
            assert_eq!(bytes("declaration_id").value(row).len(), 16);
            assert_ne!(
                bytes("entity_id").value(row),
                bytes("declaration_id").value(row)
            );
        }
    }
    assert!(found > 0, "expected source-owned caller declaration");
}

#[test]
fn wp63_beh_real_source_to_installed_fastmcp_is_causal_and_epoch_coherent() {
    const BASELINE: &[u8] = b"def answer(value: int) -> int:\n    return value + 1\n";
    const MUTATED: &[u8] = b"def answer(value: int) -> int:\n    return value + 1\n\ndef doubled(value: int) -> int:\n    return value * 2\n";

    let stack = InstalledProductionStack::build();
    let baseline_started = Instant::now();
    let baseline = installed_vertical_observation(&stack, BASELINE, "causal-baseline");
    let baseline_millis = baseline_started.elapsed().as_secs_f64() * 1_000.0;
    let mutated_started = Instant::now();
    let mutated = installed_vertical_observation(&stack, MUTATED, "causal-mutated");
    let mutated_millis = mutated_started.elapsed().as_secs_f64() * 1_000.0;

    assert_eq!(baseline.query_rows, 1, "pre-registered baseline clause");
    assert_eq!(mutated.query_rows, 2, "pre-registered mutation clause");
    assert_eq!(mutated.query_rows, baseline.query_rows + 1);
    assert_ne!(baseline.epoch_id, mutated.epoch_id);
    assert_ne!(baseline.source_authority, mutated.source_authority);
    assert_ne!(baseline.provider_set, mutated.provider_set);
    assert_ne!(baseline.proof_receipt, mutated.proof_receipt);
    assert_eq!(baseline.application_release, mutated.application_release);
    assert_eq!(baseline.provider_release, mutated.provider_release);
    assert_eq!(baseline.relation_ids, mutated.relation_ids);
    assert_eq!(
        baseline.reference_content, mutated.reference_content,
        "source mutation changed unrelated request-schema presentation"
    );
    if std::env::var_os("CODEFABRIC_WP65_MEASURE").is_some() {
        println!(
            "CODEFABRIC_WP65_OBSERVATION={}",
            json!({
                "workload_id": "installed_fastmcp_vertical",
                "baseline_millis": baseline_millis,
                "mutated_millis": mutated_millis,
                "baseline_rows": baseline.query_rows,
                "mutated_rows": mutated.query_rows,
                "semantic_change_observed": baseline.query_rows != mutated.query_rows,
                "joined": true,
            })
        );
    }
}

#[test]
fn wp63_ops_installed_restart_reconstructs_only_exact_activation_authority() {
    let fixture = ProductionFixture::new();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    let mut supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let initial_discovery = supervisor.discovery();
    let first = installed_query_report(&fixture, &stack, "restart-before", None);
    let initial_activation = all_activation_control_rows(&fixture);
    let first_status = modern_structured(modern_step(&first, "status"));
    let first_query = modern_structured(modern_step(&first, "query"));
    let first_handle = first_query["manifest"]["uri"]
        .as_str()
        .expect("first manifest URI")
        .to_owned();
    assert_eq!(
        first_status["active_epoch_id"]
            .as_str()
            .and_then(|value| value.strip_prefix("epoch:")),
        first_query["epoch_id"]
            .as_str()
            .and_then(|value| value.strip_prefix("snapshot:"))
    );

    let daemon_pid = i32::try_from(initial_discovery.daemon_pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .expect("initial daemon PID");
    rustix::process::kill_process(daemon_pid, rustix::process::Signal::KILL)
        .expect("inject daemon loss");
    let restarted = supervisor.wait_for_daemon_generation(initial_discovery.daemon_generation + 1);
    let second = installed_query_report(
        &fixture,
        &stack,
        "restart-reconstructed-daemon",
        Some(&first_handle),
    );
    let second_status = modern_structured(modern_step(&second, "status"));
    let second_query = modern_structured(modern_step(&second, "query"));
    assert_eq!(
        modern_step(&second, "stale_resource")["error_code"],
        "CLIENT_OPERATION_FAILED",
        "process-local resource handle survived daemon replacement"
    );
    assert_eq!(
        first_status["active_epoch_id"],
        second_status["active_epoch_id"]
    );
    assert_eq!(first_query["epoch_id"], second_query["epoch_id"]);
    assert_eq!(first_query["total_rows"], second_query["total_rows"]);
    assert_ne!(
        first_status["authority"]["session_id"],
        second_status["authority"]["session_id"]
    );
    assert_eq!(
        second_status["authority"]["daemon_generation"],
        restarted.daemon_generation
    );
    assert_eq!(all_activation_control_rows(&fixture), initial_activation);
    assert_eq!(activation_control_versions(&fixture).len(), 3);

    supervisor.stop();
    let replacement = fixture.start_supervisor_with(&stack.codefabric);
    let replacement_discovery = replacement.discovery();
    assert_ne!(
        replacement_discovery.supervisor_generation,
        initial_discovery.supervisor_generation
    );
    let third = installed_query_report(&fixture, &stack, "restart-reconstructed-all", None);
    let third_status = modern_structured(modern_step(&third, "status"));
    let third_query = modern_structured(modern_step(&third, "query"));
    assert_eq!(
        first_status["active_epoch_id"],
        third_status["active_epoch_id"]
    );
    assert_eq!(first_query["epoch_id"], third_query["epoch_id"]);
    assert_eq!(first_query["total_rows"], third_query["total_rows"]);
    assert_eq!(all_activation_control_rows(&fixture), initial_activation);
    assert_eq!(activation_control_versions(&fixture).len(), 3);
    assert_no_modern_secret_projection(&first, &fixture);
    assert_no_modern_secret_projection(&second, &fixture);
    assert_no_modern_secret_projection(&third, &fixture);
    replacement.stop();
}

#[test]
fn wp37_neg_codefabricd_rejects_direct_start_without_supervisor_control() {
    let fixture = ProductionFixture::new();
    let output = Command::new(env!("CARGO_BIN_EXE_codefabricd"))
        .args(["serve", "--config"])
        .arg(&fixture.config_path)
        .stdin(Stdio::null())
        .output()
        .expect("direct daemon rejection");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("control"));
    assert!(!fixture.supervisor_discovery().exists());
    assert!(!fixture.runtime.join("daemon.json").exists());
    assert!(!fixture.runtime.join("admin.sock").exists());
}

#[cfg(target_os = "linux")]
mod live_updates;
