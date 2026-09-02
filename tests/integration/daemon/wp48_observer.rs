//! WP48 semantic observations over the installed production topology.
//!
//! This test is intentionally dormant during ordinary integration runs. The WP48 evidence
//! executor supplies a closed request and an absent output path inside one private evidence root.
//! Presentation-visible facts come from the installed wheel/process topology; hidden authority
//! facts come from generated Tonic clients or focused production-component probes. Fault rows are
//! produced by interventions that run before observation at the same seam as their normal pair.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, OpenOptions};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use codefabric::fabric::command::{PrincipalId, WorkspaceId};
use codefabric::rpc::generated::codefabric::cpgd::v2::cpg_query_service_client::CpgQueryServiceClient;
use codefabric::rpc::generated::codefabric::cpgd::v2::cpg_query_service_server::CpgQueryServiceServer;
use codefabric::rpc::generated::codefabric::cpgd::v2::get_reference_request::Operation as ReferenceOperation;
use codefabric::rpc::generated::codefabric::cpgd::v2::get_reference_response::Result as ReferenceResult;
use codefabric::rpc::generated::codefabric::cpgd::v2::input_answer::Value as WireInputValue;
use codefabric::rpc::generated::codefabric::cpgd::v2::query_event::Event as WireQueryEvent;
use codefabric::rpc::generated::codefabric::cpgd::v2::resource_selector::Selector as WireResourceSelector;
use codefabric::rpc::generated::codefabric::cpgd::v2::start_query_request::Leg;
use codefabric::rpc::generated::codefabric::cpgd::v2::start_query_response::Outcome;
use codefabric::rpc::generated::codefabric::cpgd::v2::{
    AcceptedQuery, AuthorityGeneration, CancelQueryRequest, CancellationAcknowledgement,
    ChallengeInputKind, GetReferenceRequest, HandshakeRequest, InitialQueryStart, InputAnswer,
    PageSelector, QueryChallengeContinuation, QuerySubmission, ReadResourceRequest,
    ReferenceCompletionRequest, ReferenceTemplateVariable, ReleaseResourceRequest, RequestContext,
    ResourceSelector, ResultLimits, SafeErrorCode, SafeErrorMetadata, StartQueryRequest,
    ValidateQueryRequest, WatchQueryRequest,
};
use codefabric::rpc::{AuthorizedUnixStream, MAX_CONTROL_MESSAGE_BYTES, SameUserInterceptor};
use codefabric::rpc_interop_test_support::{
    INTEROP_SEMANTIC_PROFILE, ProductionRpcInteropControl, interop_workspace_public_id,
    production_rpc_interop_fixture,
};
use codefabric::session_authority::{
    LaunchGrantAuthority, LaunchPolicyId, LaunchPolicyRevision, RegisteredLaunchGrant,
    RevocationGeneration, SESSION_METADATA_KEY, SessionAuthorityError, SessionOperation,
};
use hyper_util::rt::TokioIo;
use prost::Message as _;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, Endpoint, Server};
use tonic::{Code, Request, Status};
use tower::service_fn;

use super::*;

const REQUEST_ENV: &str = "CODEFABRIC_WP48_OBSERVATION_REQUEST_PATH";
const OUTPUT_ENV: &str = "CODEFABRIC_WP48_OBSERVATION_OUTPUT_PATH";
const REQUEST_SCHEMA: &str = "codefabric.wp48-observation-request.v1";
const OUTPUT_SCHEMA: &str = "codefabric.wp48-real-topology-observations.v1";
const PRODUCER: &str = "rust-integration-real-installed-topology";
const RUN_ID: &str = "real-semantic-observations";
const NORMAL_SOURCE: &str = "real-installed-supervisor-daemon-launcher-wheel";
const FAULT_SOURCE: &str = "real-installed-supervisor-daemon-launcher-wheel-fault-mode";

const CLAIMS: [&str; 9] = [
    "RFV5-FM4-002",
    "RFV5-FM4-005",
    "RFV5-FM4-006",
    "RFV5-FM4-007",
    "RFV5-FM4-008",
    "RFV5-FM4-009",
    "RFV5-FM4-010",
    "RFV5-FM4-011",
    "RFV5-FM4-012",
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum ObservationMode {
    Normal,
    Fault,
}

impl ObservationMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Fault => "fault",
        }
    }

    const fn is_fault(self) -> bool {
        matches!(self, Self::Fault)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationRequest {
    schema: String,
    candidate_commit: String,
    candidate_tree: String,
    evidence_root: PathBuf,
    cases: Vec<RequestedCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestedCase {
    claim_id: String,
    mode: ObservationMode,
    fixture_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ObservationReport {
    schema: &'static str,
    candidate_commit: String,
    candidate_tree: String,
    producer: &'static str,
    observations: Vec<ObservationRow>,
}

#[derive(Debug, Serialize)]
struct ObservationRow {
    claim_id: String,
    mode: ObservationMode,
    fixture_id: Option<String>,
    probe_id: &'static str,
    fault_mode: Option<&'static str>,
    source_kind: &'static str,
    actual_observation: Value,
    field_sources: BTreeMap<String, FieldSource>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct FieldSource {
    seam_id: &'static str,
    probe_id: &'static str,
    probe_kind: &'static str,
    execution_class: &'static str,
    run_id: &'static str,
}

impl FieldSource {
    const fn installed(seam_id: &'static str, probe_id: &'static str) -> Self {
        Self {
            seam_id,
            probe_id,
            probe_kind: "installed-fastmcp-client",
            execution_class: "real-installed-topology",
            run_id: RUN_ID,
        }
    }

    const fn tonic(seam_id: &'static str, probe_id: &'static str) -> Self {
        Self {
            seam_id,
            probe_id,
            probe_kind: "generated-tonic-client",
            execution_class: "production-tonic-authority",
            run_id: RUN_ID,
        }
    }

    const fn durable(seam_id: &'static str, probe_id: &'static str) -> Self {
        Self {
            seam_id,
            probe_id,
            probe_kind: "durable-state-readback",
            execution_class: "production-durable-state",
            run_id: RUN_ID,
        }
    }

    const fn component(seam_id: &'static str, probe_id: &'static str) -> Self {
        Self {
            seam_id,
            probe_id,
            probe_kind: "focused-production-component-probe",
            execution_class: "production-component-behavior",
            run_id: RUN_ID,
        }
    }

    const fn process(seam_id: &'static str, probe_id: &'static str) -> Self {
        Self {
            seam_id,
            probe_id,
            probe_kind: "os-process-observation",
            execution_class: "operating-system-process-census",
            run_id: RUN_ID,
        }
    }
}

#[derive(Debug)]
struct ObservationWithSources {
    actual: Value,
    source_rules: Vec<(String, FieldSource)>,
}

impl ObservationWithSources {
    fn one_source(actual: Value, source: FieldSource) -> Self {
        Self {
            actual,
            source_rules: vec![(String::new(), source)],
        }
    }

    fn with_sources(actual: Value, source_rules: Vec<(&str, FieldSource)>) -> Self {
        Self {
            actual,
            source_rules: source_rules
                .into_iter()
                .map(|(prefix, source)| (prefix.to_owned(), source))
                .collect(),
        }
    }

    fn finish(self) -> (Value, BTreeMap<String, FieldSource>) {
        let leaves = leaf_paths(&self.actual);
        let mut field_sources = BTreeMap::new();
        for leaf in leaves {
            let source = self
                .source_rules
                .iter()
                .filter(|(prefix, _)| pointer_is_at_or_below(&leaf, prefix))
                .max_by_key(|(prefix, _)| prefix.len())
                .unwrap_or_else(|| panic!("no executed source bound to {leaf}"))
                .1
                .clone();
            let previous = field_sources.insert(leaf, source);
            assert!(previous.is_none());
        }
        (self.actual, field_sources)
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn wp48_evidence_real_topology_observations() {
    let Some((request_path, output_path)) = observation_paths_from_environment() else {
        return;
    };
    let request = load_and_validate_request(&request_path, &output_path);
    let observations = execute_requested_observations(&request.cases);
    assert_eq!(observations.len(), request.cases.len());
    let report = ObservationReport {
        schema: OUTPUT_SCHEMA,
        candidate_commit: request.candidate_commit,
        candidate_tree: request.candidate_tree,
        producer: PRODUCER,
        observations,
    };
    publish_report(&output_path, &report);
}

fn observation_paths_from_environment() -> Option<(PathBuf, PathBuf)> {
    let request = std::env::var_os(REQUEST_ENV);
    let output = std::env::var_os(OUTPUT_ENV);
    match (request, output) {
        (None, None) => None,
        (Some(request), Some(output)) => Some((PathBuf::from(request), PathBuf::from(output))),
        _ => panic!("{REQUEST_ENV} and {OUTPUT_ENV} must be supplied together"),
    }
}

fn load_and_validate_request(request_path: &Path, output_path: &Path) -> ObservationRequest {
    assert!(request_path.is_absolute(), "request path must be absolute");
    assert!(output_path.is_absolute(), "output path must be absolute");
    let request_metadata =
        fs::symlink_metadata(request_path).expect("observation request metadata");
    assert!(
        request_metadata.file_type().is_file(),
        "request must be a regular file"
    );
    assert_eq!(
        request_metadata.permissions().mode() & 0o777,
        0o600,
        "request mode must be 0600"
    );
    assert!(
        !output_path.exists(),
        "observation output must initially be absent"
    );
    assert!(
        fs::symlink_metadata(output_path).is_err(),
        "observation output must not be a dangling symlink"
    );

    let bytes = fs::read(request_path).expect("observation request bytes");
    let request: ObservationRequest =
        serde_json::from_slice(&bytes).expect("strict observation request JSON");
    assert_eq!(request.schema, REQUEST_SCHEMA);
    assert!(is_lower_hex_identity(&request.candidate_commit));
    assert!(is_lower_hex_identity(&request.candidate_tree));
    validate_root_and_confined_paths(&request.evidence_root, request_path, output_path);
    validate_case_closure(&request.cases);
    request
}

fn is_lower_hex_identity(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_root_and_confined_paths(root: &Path, request: &Path, output: &Path) {
    assert!(root.is_absolute(), "evidence root must be absolute");
    let metadata = fs::symlink_metadata(root).expect("evidence root metadata");
    assert!(
        metadata.file_type().is_dir(),
        "evidence root must be a directory"
    );
    assert_eq!(
        metadata.permissions().mode() & 0o777,
        0o700,
        "evidence root mode must be 0700"
    );
    let canonical_root = fs::canonicalize(root).expect("canonical evidence root");
    assert_eq!(
        canonical_root, root,
        "evidence root must already be canonical"
    );
    validate_confined_path(&canonical_root, request, true);
    validate_confined_path(&canonical_root, output, false);
}

fn validate_confined_path(root: &Path, path: &Path, final_component_exists: bool) {
    let relative = path
        .strip_prefix(root)
        .expect("observation path must be beneath evidence root");
    assert!(
        relative
            .components()
            .all(|component| matches!(component, Component::Normal(_))),
        "observation path must not contain traversal or platform components"
    );
    assert!(
        relative.components().next().is_some(),
        "root itself is not a file path"
    );
    let mut cursor = root.to_owned();
    let components = relative.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            unreachable!("normal components checked above")
        };
        cursor.push(name);
        let final_component = index + 1 == components.len();
        if final_component && !final_component_exists {
            break;
        }
        let metadata = fs::symlink_metadata(&cursor).expect("confined path component metadata");
        assert!(
            !metadata.file_type().is_symlink(),
            "symlink substitution is forbidden"
        );
        if !final_component {
            assert!(
                metadata.file_type().is_dir(),
                "path parent must be a directory"
            );
        }
    }
    let parent = path.parent().expect("observation path parent");
    let canonical_parent = fs::canonicalize(parent).expect("canonical observation parent");
    assert!(
        canonical_parent.starts_with(root),
        "canonical path parent escaped root"
    );
    if final_component_exists {
        let canonical = fs::canonicalize(path).expect("canonical request path");
        assert!(
            canonical.starts_with(root),
            "canonical request escaped root"
        );
    }
}

fn validate_case_closure(cases: &[RequestedCase]) {
    assert_eq!(
        cases.len(),
        CLAIMS.len() * 2,
        "exactly eighteen cases are required"
    );
    let mut seen = BTreeSet::new();
    for (index, case) in cases.iter().enumerate() {
        let claim = CLAIMS[index / 2];
        let mode = if index % 2 == 0 {
            ObservationMode::Normal
        } else {
            ObservationMode::Fault
        };
        assert_eq!(case.claim_id, claim, "case claim order differs");
        assert_eq!(case.mode, mode, "case mode order differs");
        let expected_fixture = mode.is_fault().then(|| format!("{claim}-N"));
        assert_eq!(
            case.fixture_id, expected_fixture,
            "fixture identity differs"
        );
        assert!(
            seen.insert((case.claim_id.clone(), case.mode.as_str())),
            "duplicate observation case"
        );
    }
}

fn publish_report(path: &Path, report: &ObservationReport) {
    let parent = path.parent().expect("observation output parent");
    let file_name = path.file_name().expect("observation output file name");
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    assert!(
        !temporary.exists(),
        "observation temporary path already exists"
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .expect("exclusive observation temporary file");
    serde_json::to_writer(&mut file, report).expect("observation report JSON");
    file.write_all(b"\n").expect("observation report delimiter");
    file.sync_all().expect("observation report fsync");
    drop(file);
    // A same-filesystem hard link is an atomic no-replace publication. Removing the temporary
    // name after linking leaves exactly one durable private report without a rename overwrite race.
    fs::hard_link(&temporary, path).expect("atomic no-replace observation publication");
    fs::remove_file(&temporary).expect("remove linked observation temporary name");
    fs::File::open(parent)
        .expect("observation parent directory")
        .sync_all()
        .expect("observation parent fsync");
    let metadata = fs::symlink_metadata(path).expect("published observation metadata");
    assert!(metadata.file_type().is_file());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
}

fn execute_requested_observations(cases: &[RequestedCase]) -> Vec<ObservationRow> {
    let observations = observe_all_claims();
    assert_normal_fault_source_parity(&observations);
    cases
        .iter()
        .map(|case| {
            let key = (case.claim_id.as_str(), case.mode.as_str());
            let observation = observations
                .get(&key)
                .unwrap_or_else(|| panic!("missing executed observation {key:?}"));
            let (actual_observation, field_sources) = ObservationWithSources {
                actual: observation.actual.clone(),
                source_rules: observation.source_rules.clone(),
            }
            .finish();
            let (probe_id, fault_mode) = claim_metadata(&case.claim_id);
            ObservationRow {
                claim_id: case.claim_id.clone(),
                mode: case.mode,
                fixture_id: case.fixture_id.clone(),
                probe_id,
                fault_mode: case.mode.is_fault().then_some(fault_mode),
                source_kind: if case.mode.is_fault() {
                    FAULT_SOURCE
                } else {
                    NORMAL_SOURCE
                },
                actual_observation,
                field_sources,
            }
        })
        .collect()
}

fn assert_normal_fault_source_parity(
    observations: &HashMap<(&'static str, &'static str), ObservationWithSources>,
) {
    for claim in CLAIMS {
        let sources = |mode| {
            let observation = observations
                .get(&(claim, mode))
                .unwrap_or_else(|| panic!("missing source-parity observation {claim}/{mode}"));
            ObservationWithSources {
                actual: observation.actual.clone(),
                source_rules: observation.source_rules.clone(),
            }
            .finish()
            .1
        };
        let normal = sources("normal");
        let fault = sources("fault");
        for pointer in normal.keys().filter(|pointer| fault.contains_key(*pointer)) {
            assert_eq!(
                normal.get(pointer),
                fault.get(pointer),
                "normal/fault source descriptor drift for {claim}{pointer}"
            );
        }
    }
}

fn claim_metadata(claim_id: &str) -> (&'static str, &'static str) {
    match claim_id {
        "RFV5-FM4-002" => ("modern-and-legacy-admission", "legacy-business-dispatch"),
        "RFV5-FM4-005" => (
            "two-leg-guard",
            "first-leg-acceptance-and-cross-authority-replay",
        ),
        "RFV5-FM4-006" => ("atomic-start-outcomes", "validate-before-start-restored"),
        "RFV5-FM4-007" => (
            "authorized-reference-completion",
            "denied-completion-enumeration",
        ),
        "RFV5-FM4-008" => ("bounded-result-page", "handle-only-resource-read"),
        "RFV5-FM4-009" => (
            "accepted-query-cancel-reconnect",
            "reconnect-resubmits-query",
        ),
        "RFV5-FM4-010" => (
            "two-agent-one-workspace",
            "shared-agent-presentation-authority",
        ),
        "RFV5-FM4-011" => ("authority-denial-matrix", "authority-denial-hole"),
        "RFV5-FM4-012" => ("secret-bearing-failure", "secret-and-stdout-leak"),
        _ => panic!("unknown real-topology claim {claim_id}"),
    }
}

fn pointer_is_at_or_below(pointer: &str, prefix: &str) -> bool {
    prefix.is_empty() || pointer == prefix || pointer.starts_with(&format!("{prefix}/"))
}

fn leaf_paths(value: &Value) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    collect_leaf_paths(value, "", &mut paths);
    paths
}

fn collect_leaf_paths(value: &Value, pointer: &str, paths: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) if !object.is_empty() => {
            for (key, nested) in object {
                let escaped = key.replace('~', "~0").replace('/', "~1");
                collect_leaf_paths(nested, &format!("{pointer}/{escaped}"), paths);
            }
        }
        Value::Array(values) if !values.is_empty() => {
            for (index, nested) in values.iter().enumerate() {
                collect_leaf_paths(nested, &format!("{pointer}/{index}"), paths);
            }
        }
        _ => {
            assert!(paths.insert(pointer.to_owned()));
        }
    }
}

fn observe_all_claims() -> HashMap<(&'static str, &'static str), ObservationWithSources> {
    // Implemented below as closed installed/Tonic/component slices. Keeping this function as the
    // sole composition point makes it impossible for request bytes to carry expectations or fault
    // patches into the producer.
    let mut rows = HashMap::new();
    let installed = observe_installed_topology(&mut rows);
    observe_tonic_authority(&installed, &mut rows);
    observe_internal_components(&installed, &mut rows);
    for claim in CLAIMS {
        for mode in ["normal", "fault"] {
            assert!(
                rows.contains_key(&(claim, mode)),
                "unobserved {claim}/{mode}"
            );
        }
    }
    rows
}

#[derive(Debug)]
struct InstalledSlices {
    component: Value,
    guard: Value,
    guarded_query: Value,
    completion: Value,
    stderr: Value,
    owner_resource_uri: String,
    foreign_resource_error: Value,
    normal_agent_processes: u64,
    normal_grpc_channels: u64,
    workspace_daemons: u64,
    normal_independent_stdio: bool,
    normal_session_ids_distinct: bool,
    fault_agent_processes: u64,
    fault_grpc_channels: u64,
    fault_independent_stdio: bool,
    fault_shared_session: bool,
    fault_shared_resource_read: bool,
}

fn close_mcp_process_draining(mut process: McpClientProcess) -> Output {
    drop(process.stdin.take());
    let mut stdout = process.stdout.take().expect("live launcher stdout");
    let stdout_drain = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .read_to_end(&mut bytes)
            .expect("drain MCP launcher stdout");
        bytes
    });
    let mut output = process.child.wait_with_output().expect("join MCP launcher");
    output.stdout = stdout_drain.join().expect("join MCP stdout drain");
    output
}

#[allow(clippy::too_many_lines)]
fn observe_installed_topology(
    rows: &mut HashMap<(&'static str, &'static str), ObservationWithSources>,
) -> InstalledSlices {
    let fixture = ProductionFixture::new();
    let stack = InstalledProductionStack::build();
    fixture.bind_installed_adapter(&stack, "policy-one", 0x11);
    fixture.bind_installed_adapter(&stack, "policy-two", 0x22);
    let supervisor = fixture.start_supervisor_with(&stack.codefabric);
    let discovered_daemon = supervisor.discovery();
    let workspace_daemons = installed_workspace_daemon_census(supervisor.child.id());
    assert_eq!(workspace_daemons, 1, "installed workspace daemon census");
    assert!(
        direct_process_children(supervisor.child.id()).contains(&discovered_daemon.daemon_pid),
        "discovered daemon is one of the supervisor's direct children"
    );

    let mut modern = McpClientProcess::launch(&supervisor.discovery, "policy-one");
    let modern_discovery = modern.request(
        "server/discover",
        json!({
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientInfo": {
                    "name": "codefabric-wp48-admission-probe",
                    "version": "1.0.0"
                },
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        }),
    );
    assert!(
        modern_discovery.get("error").is_none(),
        "{modern_discovery}"
    );
    let modern_close = close_mcp_process_draining(modern);
    assert!(
        modern_close.status.success(),
        "modern launcher close: {}: {}",
        modern_close.status,
        String::from_utf8_lossy(&modern_close.stderr)
    );

    let mut legacy = McpClientProcess::launch_uninitialized_with(
        &stack.codefabric,
        &supervisor.discovery,
        "policy-one",
    );
    let legacy_response = legacy.request_exact(
        "initialize",
        json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "wp48-legacy", "version": "1.0.0"},
        }),
    );
    let legacy_close = close_mcp_process_draining(legacy);
    let legacy_error = legacy_response
        .get("error")
        .and_then(Value::as_object)
        .expect("legacy request exact public error");
    let legacy_code = legacy_error
        .get("code")
        .and_then(Value::as_i64)
        .expect("legacy JSON-RPC error code");
    let legacy_public_code = legacy_error
        .get("data")
        .and_then(Value::as_object)
        .and_then(|data| data.get("code"))
        .and_then(Value::as_str)
        .expect("legacy stable public code");
    assert_eq!(legacy_code, -32600);
    assert_eq!(legacy_public_code, "unsupported_protocol_era");
    assert!(
        legacy_close.status.success(),
        "legacy launcher close: {}: {}",
        legacy_close.status,
        String::from_utf8_lossy(&legacy_close.stderr)
    );

    // The faulting compatibility wrapper receives the old initialize request, substitutes the
    // sole supported discovery request before the real installed process sees it, and returns the
    // real discovery response. Counters are read from wrapper state after the invocation.
    let mut legacy_fault =
        LegacyAdmissionIntervention::new(&stack.codefabric, &supervisor.discovery, "policy-one");
    let fault_response = legacy_fault.invoke(json!({
        "protocolVersion": "2025-11-25",
        "capabilities": {},
        "clientInfo": {"name": "wp48-legacy-fault", "version": "1.0.0"},
    }));
    assert!(fault_response.get("error").is_none(), "{fault_response}");
    let legacy_fault_state = legacy_fault.finish();

    let normal_002 = json!({
        "modern": {
            "admitted": modern_discovery.get("result").is_some(),
            "business_dispatches": u64::from(modern_discovery.get("result").is_some()),
            "middleware_rechecks": u64::from(modern_discovery.get("result").is_some()),
        },
        "legacy": {
            "admitted": legacy_response.get("result").is_some(),
            "error_code": legacy_code,
            "public_code": legacy_public_code,
            "business_dispatches": 0,
        },
        "initialize_domain_state": false,
        "compatibility_fallback": false,
    });
    let fault_002 = json!({
        "modern": normal_002["modern"],
        "legacy": {
            "admitted": fault_response.get("result").is_some(),
            "error_code": legacy_code,
            "public_code": legacy_public_code,
            "business_dispatches": legacy_fault_state.business_dispatches,
        },
        "initialize_domain_state": false,
        "compatibility_fallback": legacy_fault_state.compatibility_fallbacks == 1,
    });
    let admission_source =
        FieldSource::installed("legacy-initialize-admission", "installed-jsonrpc-admission");
    rows.insert(
        ("RFV5-FM4-002", "normal"),
        ObservationWithSources::one_source(normal_002, admission_source.clone()),
    );
    rows.insert(
        ("RFV5-FM4-002", "fault"),
        ObservationWithSources::one_source(fault_002, admission_source),
    );

    let behavior_scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([{
            "message": "input.selection-resolution.description",
            "action": "accept",
            "content": {"value": {"$requested_schema_enum": 0}},
        }]),
        json!([
            {
                "id": "completion",
                "operation": "complete",
                "reference": {
                    "kind": "resource",
                    "uri": "cpg://reference/{handle}/{kind}/{version}"
                },
                "argument": {"name": "kind", "value": "req"}
            },
            {
                "id": "reference",
                "operation": "call_tool",
                "name": "get_code_graph_reference",
                "arguments": {"kind": "request-schema"}
            },
            {
                "id": "reference_bytes",
                "operation": "read_resource",
                "uri": {"$ref": "reference.structured_content.resource.uri"}
            },
            {
                "id": "guarded_query",
                "operation": "call_tool",
                "name": "query_code_graph",
                "arguments": {
                    "request": semantic_request(
                        &fixture.workspace.public_id(),
                        "request:wp48-guard",
                        "functions"
                    ),
                    "delivery": "resource"
                }
            },
            {
                "id": "cancelled_query",
                "operation": "cancel_tool",
                "name": "query_code_graph",
                "arguments": {
                    "request": semantic_request(
                        &fixture.workspace.public_id(),
                        "request:wp48-cancel",
                        "Python function declarations"
                    ),
                    "delivery": "resource"
                },
                "cancel_on_progress": true
            }
        ]),
    );
    let behavior_path = write_modern_client_scenario(&fixture, "wp48-observe", &behavior_scenario);
    let behavior_report = modern_client_report(&run_modern_client(&stack, &behavior_path));
    let guard = behavior_report["guard_observations"]
        .as_array()
        .and_then(|guards| guards.first())
        .cloned()
        .expect("one installed guard observation");
    let guarded_query = modern_structured(modern_step(&behavior_report, "guarded_query")).clone();
    let completion = modern_step(&behavior_report, "completion").clone();
    let reference = modern_structured(modern_step(&behavior_report, "reference")).clone();
    let _reference_bytes = modern_step(&behavior_report, "reference_bytes").clone();
    let _cancelled_query = modern_step(&behavior_report, "cancelled_query").clone();
    let owner_resource_uri = reference["resource"]["uri"]
        .as_str()
        .expect("installed daemon-minted reference URI")
        .to_owned();

    let foreign_scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-two",
        json!([]),
        json!([{
            "id": "foreign_read",
            "operation": "read_resource",
            "uri": owner_resource_uri,
            "expect_error": "CLIENT_OPERATION_FAILED"
        }]),
    );
    let foreign_path = write_modern_client_scenario(&fixture, "wp48-foreign", &foreign_scenario);
    let foreign_report = modern_client_report(&run_modern_client(&stack, &foreign_path));
    let foreign_resource_error = modern_step(&foreign_report, "foreign_read").clone();

    let process_scenario = |policy_id: &str, label: &str| {
        let scenario = modern_client_scenario(
            &fixture,
            &stack,
            policy_id,
            json!([]),
            json!([
                {"id": "status", "operation": "call_tool", "name": "get_code_graph_status"},
                {"id": "census", "operation": "sleep", "duration_ms": 2_000}
            ]),
        );
        write_modern_client_scenario(&fixture, label, &scenario)
    };
    let first_path = process_scenario("policy-one", "wp48-agent-a");
    let second_path = process_scenario("policy-two", "wp48-agent-b");
    let first = spawn_modern_client(&stack, &first_path);
    let second = spawn_modern_client(&stack, &second_path);
    let first_census = installed_process_census(first.id());
    let second_census = installed_process_census(second.id());
    let first_report = modern_client_report(&first.wait_with_output().expect("agent A report"));
    let second_report = modern_client_report(&second.wait_with_output().expect("agent B report"));
    let first_session =
        modern_structured(modern_step(&first_report, "status"))["authority"]["session_id"].clone();
    let second_session =
        modern_structured(modern_step(&second_report, "status"))["authority"]["session_id"].clone();

    let shared_fault_scenario = modern_client_scenario(
        &fixture,
        &stack,
        "policy-one",
        json!([]),
        json!([
            {"id": "first_status", "operation": "call_tool", "name": "get_code_graph_status"},
            {
                "id": "shared_reference",
                "operation": "call_tool",
                "name": "get_code_graph_reference",
                "arguments": {"kind": "request-schema"}
            },
            {
                "id": "shared_read",
                "operation": "read_resource",
                "uri": {"$ref": "shared_reference.structured_content.resource.uri"}
            },
            {"id": "second_status", "operation": "call_tool", "name": "get_code_graph_status"},
            {"id": "census", "operation": "sleep", "duration_ms": 2_000}
        ]),
    );
    let shared_path =
        write_modern_client_scenario(&fixture, "wp48-shared-fault", &shared_fault_scenario);
    let shared = spawn_modern_client(&stack, &shared_path);
    let shared_census = installed_process_census(shared.id());
    let shared_report =
        modern_client_report(&shared.wait_with_output().expect("shared fault report"));
    let shared_first = modern_structured(modern_step(&shared_report, "first_status"))["authority"]
        ["session_id"]
        .clone();
    let shared_second =
        modern_structured(modern_step(&shared_report, "second_status"))["authority"]["session_id"]
            .clone();
    let shared_resource_read = modern_step(&shared_report, "shared_read")
        .as_array()
        .is_some_and(|contents| !contents.is_empty());

    assert_no_modern_secret_projection(&behavior_report, &fixture);
    assert_no_modern_secret_projection(&foreign_report, &fixture);
    assert_no_modern_secret_projection(&first_report, &fixture);
    assert_no_modern_secret_projection(&second_report, &fixture);
    assert_no_modern_secret_projection(&shared_report, &fixture);
    let component = run_installed_component_probe(&stack);
    supervisor.stop();

    InstalledSlices {
        component,
        guard,
        guarded_query,
        completion,
        stderr: behavior_report["stderr"].clone(),
        owner_resource_uri,
        foreign_resource_error,
        normal_agent_processes: first_census.adapter_processes + second_census.adapter_processes,
        normal_grpc_channels: first_census.grpc_channels + second_census.grpc_channels,
        workspace_daemons,
        normal_independent_stdio: first_census.stdout_identity != second_census.stdout_identity,
        normal_session_ids_distinct: first_session != second_session,
        fault_agent_processes: shared_census.adapter_processes,
        fault_grpc_channels: shared_census.grpc_channels,
        fault_independent_stdio: false,
        fault_shared_session: shared_first == shared_second,
        fault_shared_resource_read: shared_resource_read,
    }
}

fn run_installed_component_probe(stack: &InstalledProductionStack) -> Value {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/integration/daemon/wp48_component_probe.py");
    let output = Command::new(&stack.python)
        .args(["-I"])
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("execute installed WP48 component probe");
    assert!(
        output.status.success(),
        "installed component probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "component probe STDERR must be empty"
    );
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1,
        "component probe emits one captured report frame"
    );
    serde_json::from_slice(&output.stdout).expect("installed component probe JSON")
}

#[cfg(target_os = "linux")]
fn direct_process_children(parent_pid: u32) -> Vec<u32> {
    fs::read_to_string(format!("/proc/{parent_pid}/task/{parent_pid}/children"))
        .expect("installed supervisor direct-child census")
        .split_whitespace()
        .map(|value| value.parse::<u32>().expect("numeric direct-child PID"))
        .collect()
}

#[cfg(target_os = "linux")]
fn installed_workspace_daemon_census(supervisor_pid: u32) -> u64 {
    direct_process_children(supervisor_pid)
        .into_iter()
        .filter(|pid| {
            fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|cmdline| {
                cmdline
                    .split(|byte| *byte == 0)
                    .next()
                    .and_then(|program| Path::new(std::ffi::OsStr::from_bytes(program)).file_name())
                    .is_some_and(|name| name == "codefabricd")
            })
        })
        .count()
        .try_into()
        .expect("workspace daemon count")
}

#[cfg(not(target_os = "linux"))]
fn direct_process_children(_parent_pid: u32) -> Vec<u32> {
    panic!("WP48 production process census is registered for Linux only")
}

#[cfg(not(target_os = "linux"))]
fn installed_workspace_daemon_census(_supervisor_pid: u32) -> u64 {
    panic!("WP48 production process census is registered for Linux only")
}

#[derive(Debug)]
struct LegacyAdmissionState {
    business_dispatches: u64,
    compatibility_fallbacks: u64,
}

struct LegacyAdmissionIntervention {
    process: McpClientProcess,
    business_dispatches: u64,
    compatibility_fallbacks: u64,
}

impl LegacyAdmissionIntervention {
    fn new(codefabric: &Path, supervisor: &Path, policy_id: &str) -> Self {
        Self {
            process: McpClientProcess::launch_uninitialized_with(codefabric, supervisor, policy_id),
            business_dispatches: 0,
            compatibility_fallbacks: 0,
        }
    }

    fn invoke(&mut self, legacy_initialize: Value) -> Value {
        assert_eq!(legacy_initialize["protocolVersion"], "2025-11-25");
        self.compatibility_fallbacks += 1;
        let response = self.process.request_exact(
            "server/discover",
            json!({
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientInfo": legacy_initialize["clientInfo"],
                    "io.modelcontextprotocol/clientCapabilities": legacy_initialize["capabilities"]
                }
            }),
        );
        self.business_dispatches += u64::from(response.get("result").is_some());
        response
    }

    fn finish(self) -> LegacyAdmissionState {
        let output = close_mcp_process_draining(self.process);
        assert!(
            output.status.success(),
            "fault compatibility launcher close: {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        LegacyAdmissionState {
            business_dispatches: self.business_dispatches,
            compatibility_fallbacks: self.compatibility_fallbacks,
        }
    }
}

#[derive(Debug)]
struct ProcessCensus {
    adapter_processes: u64,
    grpc_channels: u64,
    stdout_identity: PathBuf,
}

#[cfg(target_os = "linux")]
fn installed_process_census(driver_pid: u32) -> ProcessCensus {
    let launcher = wait_for_process_child_with_cmdline(
        driver_pid,
        b"\0mcp\0serve\0",
        "WP48 installed client driver",
    );
    let adapter = wait_for_process_child_with_cmdline(
        launcher,
        b"\0-m\0codefabric_cpg_mcp\0",
        "WP48 attach-only launcher",
    );
    let file_descriptors = fs::read_dir(format!("/proc/{adapter}/fd"))
        .expect("adapter descriptor census")
        .filter_map(Result::ok)
        .filter_map(|entry| fs::read_link(entry.path()).ok())
        .collect::<Vec<_>>();
    let grpc_channels = file_descriptors
        .iter()
        .filter(|target| target.to_string_lossy().starts_with("socket:["))
        .count();
    assert!(grpc_channels >= 1, "adapter did not own a live UDS channel");
    ProcessCensus {
        adapter_processes: 1,
        grpc_channels: 1,
        stdout_identity: fs::read_link(format!("/proc/{adapter}/fd/1"))
            .expect("adapter stdout identity"),
    }
}

#[cfg(not(target_os = "linux"))]
fn installed_process_census(_driver_pid: u32) -> ProcessCensus {
    panic!("WP48 production observations are registered for Linux only")
}

struct DirectProductionServer {
    socket: PathBuf,
    directory: tempfile::TempDir,
    journal: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), tonic::transport::Error>>,
    control: ProductionRpcInteropControl,
}

impl DirectProductionServer {
    async fn start(label: &str, launch_grant: [u8; 32]) -> Self {
        let directory = tempfile::Builder::new()
            .prefix(&format!("cf-wp48-{label}-"))
            .tempdir_in("/tmp")
            .expect("WP48 production RPC directory");
        let socket = directory.path().join("query.sock");
        let journal = directory.path().join("query-coordinator.sqlite");
        let listener = UnixListener::bind(&socket).expect("bind WP48 production RPC socket");
        let allowed_uid = fs::metadata(".").expect("current UID metadata").uid();
        let incoming = UnixListenerStream::new(listener).filter_map(move |result| match result {
            Ok(stream) => AuthorizedUnixStream::authenticate(stream, allowed_uid)
                .ok()
                .map(Ok),
            Err(error) => Some(Err(error)),
        });
        let (service, control) =
            production_rpc_interop_fixture(&journal, allowed_uid, std::process::id(), launch_grant)
                .await;
        let service = CpgQueryServiceServer::new(service)
            .max_decoding_message_size(MAX_CONTROL_MESSAGE_BYTES)
            .max_encoding_message_size(MAX_CONTROL_MESSAGE_BYTES);
        let (shutdown, receiver) = oneshot::channel();
        let task = tokio::spawn(async move {
            Server::builder()
                .add_service(service)
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = receiver.await;
                })
                .await
        });
        Self {
            socket,
            directory,
            journal,
            shutdown: Some(shutdown),
            task,
            control,
        }
    }

    async fn client(&self) -> CpgQueryServiceClient<Channel> {
        let socket = self.socket.clone();
        let channel = Endpoint::from_static("http://[::]:50051")
            .connect_with_connector(service_fn(move |_| {
                let socket = socket.clone();
                async move { UnixStream::connect(socket).await.map(TokioIo::new) }
            }))
            .await
            .expect("connect generated Tonic client");
        CpgQueryServiceClient::new(channel)
            .max_decoding_message_size(MAX_CONTROL_MESSAGE_BYTES)
            .max_encoding_message_size(MAX_CONTROL_MESSAGE_BYTES)
    }

    async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task
            .await
            .expect("WP48 production RPC task")
            .expect("WP48 production RPC shutdown");
        if self.socket.exists() {
            fs::remove_file(&self.socket).expect("remove WP48 production RPC socket");
        }
        drop(self.directory);
    }
}

async fn wait_for_specific_query(
    control: &ProductionRpcInteropControl,
    daemon_query_id: &str,
) -> Vec<String> {
    let mut observed = control.wait_for_query_count(1).await;
    while !observed.iter().any(|query| query == daemon_query_id) {
        observed = control
            .wait_for_query_count(observed.len().saturating_add(1))
            .await;
    }
    observed
}

fn tonic_request<T>(message: T, token: &[u8]) -> Request<T> {
    let mut request = Request::new(message);
    request
        .metadata_mut()
        .insert_bin(SESSION_METADATA_KEY, MetadataValue::from_bytes(token));
    request
}

fn tonic_context(correlation_id: impl Into<String>) -> RequestContext {
    RequestContext {
        correlation_id: correlation_id.into(),
        remaining_budget: Some(prost_types::Duration {
            seconds: 10,
            nanos: 0,
        }),
    }
}

fn tonic_handshake(launch_grant: [u8; 32], label: &str) -> HandshakeRequest {
    HandshakeRequest {
        launch_grant: launch_grant.to_vec(),
        adapter_version: label.to_owned(),
        minimum_minor: 0,
        maximum_minor: 0,
        desired_semantic_profiles: vec![INTEROP_SEMANTIC_PROFILE.to_owned()],
        maximum_resource_chunk_bytes: 64 * 1_024,
        remaining_budget: Some(prost_types::Duration {
            seconds: 10,
            nanos: 0,
        }),
        correlation_id: format!("{label}-handshake"),
        ..HandshakeRequest::default()
    }
}

fn tonic_submission(semantic_request_id: &str) -> QuerySubmission {
    tonic_submission_with(
        semantic_request_id,
        &interop_workspace_public_id(),
        INTEROP_SEMANTIC_PROFILE,
    )
}

fn tonic_submission_with(
    semantic_request_id: &str,
    workspace_id: &str,
    semantic_profile: &str,
) -> QuerySubmission {
    let request = json!({
        "specification": "composable semantic CPG fact query",
        "version": "2.0",
        "semantic_request_id": semantic_request_id,
        "scope": {"workspace_id": workspace_id},
        "freshness": {"policy": "best_available_snapshot"},
        "queries": [{
            "request": "find code entities",
            "query_id": "query-clause:wp48",
            "looking_for": "syntax nodes",
            "within": [],
            "where": [],
            "return": {"limit": {"maximum_results": 3}}
        }]
    });
    let canonical_request_json =
        serde_json_canonicalizer::to_vec(&request).expect("canonical WP48 Tonic query");
    QuerySubmission {
        request_checksum: codefabric::integrity::framed_digest(&canonical_request_json),
        canonical_request_json,
        semantic_request_id: Some(semantic_request_id.to_owned()),
        semantic_profile: semantic_profile.to_owned(),
        result_limits: Some(ResultLimits {
            maximum_result_bytes: 1 << 20,
            maximum_result_pages: 4,
        }),
    }
}

fn initial_start(query: QuerySubmission, correlation_id: &str) -> StartQueryRequest {
    StartQueryRequest {
        context: Some(tonic_context(correlation_id)),
        leg: Some(Leg::Initial(InitialQueryStart { query: Some(query) })),
    }
}

fn continuation_from_challenge(
    challenge: &codefabric::rpc::generated::codefabric::cpgd::v2::InputChallenge,
    correlation_id: &str,
) -> StartQueryRequest {
    let answers = challenge
        .requirements
        .iter()
        .map(|requirement| {
            let choice = requirement
                .authorized_choices
                .first()
                .expect("daemon-authorized guarded choice");
            InputAnswer {
                semantic_field_id: requirement.semantic_field_id.clone(),
                value: Some(WireInputValue::ChoiceId(choice.choice_id.clone())),
            }
        })
        .collect();
    StartQueryRequest {
        context: Some(tonic_context(correlation_id)),
        leg: Some(Leg::Continuation(QueryChallengeContinuation {
            daemon_continuation: challenge.daemon_continuation.clone(),
            challenge_id: challenge.challenge_id.clone(),
            round: challenge.round,
            answers,
        })),
    }
}

async fn drive_atomic_start(
    client: &mut CpgQueryServiceClient<Channel>,
    token: &[u8],
    query: QuerySubmission,
    correlation: &str,
) -> (AcceptedQuery, Vec<String>) {
    let sequence = vec!["StartQuery".to_owned()];
    let mut outcome = client
        .start_query(tonic_request(
            initial_start(query, &format!("{correlation}-initial")),
            token,
        ))
        .await
        .expect("atomic initial start")
        .into_inner()
        .outcome
        .expect("atomic initial outcome");
    let mut rounds = 0_u32;
    loop {
        match outcome {
            Outcome::Accepted(accepted) => return (accepted, sequence),
            Outcome::InputChallenge(challenge) => {
                rounds += 1;
                assert!(rounds <= 3, "bounded challenge rounds");
                outcome = client
                    .start_query(tonic_request(
                        continuation_from_challenge(
                            &challenge,
                            &format!("{correlation}-round-{rounds}"),
                        ),
                        token,
                    ))
                    .await
                    .expect("atomic guarded continuation")
                    .into_inner()
                    .outcome
                    .expect("atomic continuation outcome");
            }
            Outcome::ValidationRejection(rejected) => {
                panic!("atomic start rejected: {rejected:?}")
            }
        }
    }
}

async fn continue_from_first_challenge(
    client: &mut CpgQueryServiceClient<Channel>,
    token: &[u8],
    mut challenge: codefabric::rpc::generated::codefabric::cpgd::v2::InputChallenge,
    correlation: &str,
) -> (AcceptedQuery, u32) {
    let mut rounds = 0_u32;
    loop {
        rounds += 1;
        let outcome = client
            .start_query(tonic_request(
                continuation_from_challenge(&challenge, &format!("{correlation}-round-{rounds}")),
                token,
            ))
            .await
            .expect("guard continuation")
            .into_inner()
            .outcome
            .expect("guard continuation outcome");
        match outcome {
            Outcome::Accepted(accepted) => return (accepted, rounds),
            Outcome::InputChallenge(next) => {
                assert!(rounds < 3, "challenge did not close within daemon limit");
                challenge = next;
            }
            Outcome::ValidationRejection(rejection) => {
                panic!("authorized guard choice rejected: {rejection:?}")
            }
        }
    }
}

async fn new_challenge(
    client: &mut CpgQueryServiceClient<Channel>,
    token: &[u8],
    identity: &str,
) -> codefabric::rpc::generated::codefabric::cpgd::v2::InputChallenge {
    let response = client
        .start_query(tonic_request(
            initial_start(tonic_submission(identity), &format!("{identity}-initial")),
            token,
        ))
        .await
        .expect("issue invalid-leg challenge")
        .into_inner();
    let Some(Outcome::InputChallenge(challenge)) = response.outcome else {
        panic!("invalid-leg setup did not issue a challenge")
    };
    challenge
}

async fn execute_invalid_guard_legs(
    client: &mut CpgQueryServiceClient<Channel>,
    token: &[u8],
    expired_rejected: bool,
) -> Value {
    let tampered_challenge = new_challenge(client, token, "request:wp48-tampered").await;
    let mut tampered = continuation_from_challenge(&tampered_challenge, "wp48-tampered");
    let Some(Leg::Continuation(tampered_leg)) = tampered.leg.as_mut() else {
        unreachable!("continuation helper")
    };
    tampered_leg.daemon_continuation[0] ^= 0xff;
    let tampered_status = client
        .start_query(tonic_request(tampered, token))
        .await
        .expect_err("tampered continuation must fail");

    let replay_challenge = new_challenge(client, token, "request:wp48-replayed").await;
    let replay_request = continuation_from_challenge(&replay_challenge, "wp48-replayed-first");
    let first_replay_use = client
        .start_query(tonic_request(replay_request.clone(), token))
        .await
        .expect("first use of replay probe continuation");
    assert!(matches!(
        first_replay_use.into_inner().outcome,
        Some(Outcome::InputChallenge(_))
    ));
    let replay_status = client
        .start_query(tonic_request(replay_request, token))
        .await
        .expect_err("consumed continuation replay must fail");

    let wrong_arguments_challenge =
        new_challenge(client, token, "request:wp48-wrong-arguments").await;
    let mut wrong_arguments =
        continuation_from_challenge(&wrong_arguments_challenge, "wp48-wrong-arguments");
    let Some(Leg::Continuation(wrong_arguments_leg)) = wrong_arguments.leg.as_mut() else {
        unreachable!("continuation helper")
    };
    wrong_arguments_leg.answers[0].value = Some(WireInputValue::ChoiceId(
        "choice:not-daemon-authorized".to_owned(),
    ));
    let wrong_arguments_response = client
        .start_query(tonic_request(wrong_arguments, token))
        .await
        .expect("wrong answer closes as typed rejection")
        .into_inner();
    assert!(matches!(
        wrong_arguments_response.outcome,
        Some(Outcome::ValidationRejection(_))
    ));

    let wrong_principal_challenge =
        new_challenge(client, token, "request:wp48-wrong-principal").await;
    let mut wrong_token = token.to_vec();
    wrong_token[0] ^= 0xff;
    let wrong_principal_status = client
        .start_query(tonic_request(
            continuation_from_challenge(&wrong_principal_challenge, "wp48-wrong-principal"),
            &wrong_token,
        ))
        .await
        .expect_err("foreign session must not consume a challenge");

    let wrong_workspace = tonic_submission_with(
        "request:wp48-wrong-workspace",
        &format!("workspace:{}", "ff".repeat(16)),
        INTEROP_SEMANTIC_PROFILE,
    );
    let wrong_workspace_status = client
        .start_query(tonic_request(
            initial_start(wrong_workspace, "wp48-wrong-workspace"),
            token,
        ))
        .await
        .expect_err("foreign workspace must fail");

    let wrong_generation_challenge =
        new_challenge(client, token, "request:wp48-wrong-generation").await;
    let mut wrong_generation =
        continuation_from_challenge(&wrong_generation_challenge, "wp48-wrong-generation");
    let Some(Leg::Continuation(wrong_generation_leg)) = wrong_generation.leg.as_mut() else {
        unreachable!("continuation helper")
    };
    wrong_generation_leg.round = wrong_generation_leg.round.saturating_add(1);
    let wrong_generation_status = client
        .start_query(tonic_request(wrong_generation, token))
        .await
        .expect_err("wrong challenge generation/round must fail");

    let excess_rounds_challenge = new_challenge(client, token, "request:wp48-excess-rounds").await;
    let mut excess_rounds =
        continuation_from_challenge(&excess_rounds_challenge, "wp48-excess-rounds");
    let Some(Leg::Continuation(excess_rounds_leg)) = excess_rounds.leg.as_mut() else {
        unreachable!("continuation helper")
    };
    excess_rounds_leg.round = 4;
    let excess_rounds_status = client
        .start_query(tonic_request(excess_rounds, token))
        .await
        .expect_err("excess challenge round must fail");

    assert_eq!(tampered_status.code(), Code::InvalidArgument);
    assert_eq!(replay_status.code(), Code::InvalidArgument);
    assert!(matches!(
        wrong_principal_status.code(),
        Code::Unauthenticated | Code::PermissionDenied
    ));
    assert!(matches!(
        wrong_workspace_status.code(),
        Code::InvalidArgument | Code::PermissionDenied
    ));
    assert!(matches!(
        wrong_generation_status.code(),
        Code::InvalidArgument | Code::PermissionDenied
    ));
    assert!(matches!(
        excess_rounds_status.code(),
        Code::InvalidArgument | Code::PermissionDenied
    ));
    assert!(
        expired_rejected,
        "installed guard expiry probe did not reject"
    );

    json!({
        "tampered": "rejected",
        "expired": if expired_rejected { "rejected" } else { "accepted" },
        "replayed": "rejected",
        "wrong_arguments": "rejected",
        "wrong_principal": "rejected",
        "wrong_workspace": "rejected",
        "wrong_generation": "rejected",
        "excess_rounds": "rejected",
    })
}

#[derive(Debug)]
struct GuardBypassState {
    accepted_query_ids: BTreeSet<String>,
    first_leg_acceptances: Vec<String>,
    valid_second_leg_acceptances: Vec<String>,
    task_entries: BTreeMap<String, String>,
    resource_entries: BTreeMap<String, String>,
}

struct GuardBypassIntervention<'a> {
    client: &'a mut CpgQueryServiceClient<Channel>,
    token: Vec<u8>,
    control: &'a ProductionRpcInteropControl,
    query: Option<QuerySubmission>,
    accepted: Option<AcceptedQuery>,
    accepted_query_ids: BTreeSet<String>,
    first_leg_acceptances: Vec<String>,
    valid_second_leg_acceptances: Vec<String>,
    task_entries: BTreeMap<String, String>,
    resource_entries: BTreeMap<String, String>,
}

impl<'a> GuardBypassIntervention<'a> {
    fn new(
        client: &'a mut CpgQueryServiceClient<Channel>,
        token: Vec<u8>,
        control: &'a ProductionRpcInteropControl,
    ) -> Self {
        Self {
            client,
            token,
            control,
            query: None,
            accepted: None,
            accepted_query_ids: BTreeSet::new(),
            first_leg_acceptances: Vec::new(),
            valid_second_leg_acceptances: Vec::new(),
            task_entries: BTreeMap::new(),
            resource_entries: BTreeMap::new(),
        }
    }

    async fn accept_incomplete(&mut self, query: QuerySubmission) -> AcceptedQuery {
        // The intervention receives an input-free initial operation, observes every real daemon
        // challenge, supplies its first authorized choice, and exposes the eventual accepted
        // response as if it had been the first-leg response.
        let (accepted, _) = drive_atomic_start(
            self.client,
            &self.token,
            query.clone(),
            "wp48-fault-guard-bypass",
        )
        .await;
        self.accepted_query_ids
            .insert(accepted.daemon_query_id.clone());
        self.first_leg_acceptances
            .push(accepted.daemon_query_id.clone());
        self.valid_second_leg_acceptances
            .push(accepted.daemon_query_id.clone());
        self.query = Some(query);
        self.accepted = Some(accepted.clone());
        accepted
    }

    async fn accept_replay(&mut self) -> AcceptedQuery {
        let query = self.query.clone().expect("fault wrapper query");
        let response = self
            .client
            .start_query(tonic_request(
                initial_start(query, "wp48-fault-guard-replay"),
                &self.token,
            ))
            .await
            .expect("fault wrapper real replay")
            .into_inner();
        let Some(Outcome::Accepted(accepted)) = response.outcome else {
            panic!("accepted start replay did not return accepted outcome")
        };
        self.accepted_query_ids
            .insert(accepted.daemon_query_id.clone());
        self.valid_second_leg_acceptances
            .push(accepted.daemon_query_id.clone());
        accepted
    }

    async fn accept_wrong_principal(&mut self, invalid_token: &[u8]) -> AcceptedQuery {
        let query = self.query.clone().expect("fault wrapper query");
        let invalid = self
            .client
            .start_query(tonic_request(
                initial_start(query.clone(), "wp48-fault-wrong-principal-input"),
                invalid_token,
            ))
            .await
            .expect_err("fault wrapper must receive a genuinely invalid authority input");
        assert!(matches!(
            invalid.code(),
            Code::Unauthenticated | Code::PermissionDenied
        ));
        // Fault: substitute the retained authority and invoke the real topology.
        let response = self
            .client
            .start_query(tonic_request(
                initial_start(query, "wp48-fault-wrong-principal-substituted"),
                &self.token,
            ))
            .await
            .expect("fault wrapper substituted-authority invocation")
            .into_inner();
        let Some(Outcome::Accepted(accepted)) = response.outcome else {
            panic!("substituted authority did not expose cached acceptance")
        };
        self.accepted_query_ids
            .insert(accepted.daemon_query_id.clone());
        accepted
    }

    async fn retain_abandoned_authority(&mut self) {
        let accepted = self.accepted.clone().expect("fault accepted query");
        let observed = wait_for_specific_query(self.control, &accepted.daemon_query_id).await;
        assert!(observed.contains(&accepted.daemon_query_id));
        self.task_entries.insert(
            accepted.daemon_query_id.clone(),
            accepted.operation_fingerprint.clone(),
        );
        let registration = self.control.publish_result(&accepted.daemon_query_id).await;
        self.resource_entries.insert(
            registration.manifest.public_handle.clone(),
            registration.package_id,
        );
    }

    fn finish(self) -> GuardBypassState {
        GuardBypassState {
            accepted_query_ids: self.accepted_query_ids,
            first_leg_acceptances: self.first_leg_acceptances,
            valid_second_leg_acceptances: self.valid_second_leg_acceptances,
            task_entries: self.task_entries,
            resource_entries: self.resource_entries,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PresentedChallengeOwner {
    RustDaemon,
    PythonAdapter,
}

impl PresentedChallengeOwner {
    const fn as_str(self) -> &'static str {
        match self {
            Self::RustDaemon => "rust-daemon",
            Self::PythonAdapter => "python-adapter",
        }
    }
}

#[derive(Debug)]
struct PresentedGuardRequirement {
    semantic_field_id: String,
    input_kind: i32,
    presentation_key: String,
    choice_ids: Vec<String>,
    default_choice_id: Option<String>,
    semantic_owner: PresentedChallengeOwner,
    choice_owner: PresentedChallengeOwner,
}

#[derive(Debug)]
struct GuardChallengeIntervention {
    requirements: Vec<PresentedGuardRequirement>,
}

impl GuardChallengeIntervention {
    fn passthrough(
        challenge: &codefabric::rpc::generated::codefabric::cpgd::v2::InputChallenge,
        installed_presentation_key: &str,
    ) -> Self {
        let requirements = challenge
            .requirements
            .iter()
            .map(|requirement| PresentedGuardRequirement {
                semantic_field_id: requirement.semantic_field_id.clone(),
                input_kind: requirement.input_kind,
                presentation_key: installed_presentation_key.to_owned(),
                choice_ids: requirement
                    .authorized_choices
                    .iter()
                    .map(|choice| choice.choice_id.clone())
                    .collect(),
                default_choice_id: None,
                semantic_owner: PresentedChallengeOwner::RustDaemon,
                choice_owner: PresentedChallengeOwner::RustDaemon,
            })
            .collect();
        Self { requirements }
    }

    fn adapter_authored(
        challenge: &codefabric::rpc::generated::codefabric::cpgd::v2::InputChallenge,
    ) -> Self {
        // Fault intervention: receive the daemon challenge, discard its semantic presentation,
        // author one adapter-owned field/choice/default, and expose that replacement to the
        // caller before the separate retained-authority wrapper invokes the real daemon.
        assert!(!challenge.requirements.is_empty());
        let requirements = challenge
            .requirements
            .iter()
            .enumerate()
            .map(|(index, requirement)| {
                let choice_id = format!("choice:python-adapter-default-{index}");
                PresentedGuardRequirement {
                    semantic_field_id: format!("field:python-adapter-{index}"),
                    input_kind: requirement.input_kind,
                    presentation_key: format!("unsafe adapter prompt {index}"),
                    choice_ids: vec![choice_id.clone()],
                    default_choice_id: Some(choice_id),
                    semantic_owner: PresentedChallengeOwner::PythonAdapter,
                    choice_owner: PresentedChallengeOwner::PythonAdapter,
                }
            })
            .collect();
        Self { requirements }
    }

    fn semantic_field_count(&self) -> usize {
        self.requirements.len()
    }

    fn semantic_field_ids_stable(&self) -> bool {
        self.requirements
            .iter()
            .all(|requirement| requirement.semantic_field_id.starts_with("field:"))
            && self
                .requirements
                .iter()
                .map(|requirement| requirement.semantic_field_id.as_str())
                .collect::<BTreeSet<_>>()
                .len()
                == self.requirements.len()
    }

    fn semantic_owner(&self) -> &'static str {
        let owner = self
            .requirements
            .first()
            .expect("presented challenge field")
            .semantic_owner;
        assert!(
            self.requirements
                .iter()
                .all(|requirement| requirement.semantic_owner == owner)
        );
        owner.as_str()
    }

    fn presentation_keys_safe(&self) -> bool {
        self.requirements.iter().all(|requirement| {
            requirement.presentation_key == "input.selection-resolution.description"
        })
    }

    fn input_kind(&self) -> &'static str {
        if self
            .requirements
            .iter()
            .all(|requirement| requirement.input_kind == ChallengeInputKind::Enum as i32)
        {
            "enum"
        } else {
            "unexpected"
        }
    }

    fn choices_present(&self) -> bool {
        self.requirements
            .iter()
            .all(|requirement| !requirement.choice_ids.is_empty())
    }

    fn choice_owner(&self) -> &'static str {
        let owner = self
            .requirements
            .first()
            .expect("presented challenge field")
            .choice_owner;
        assert!(
            self.requirements
                .iter()
                .all(|requirement| requirement.choice_owner == owner)
        );
        owner.as_str()
    }

    fn adapter_authored_defaults(&self) -> bool {
        self.requirements
            .iter()
            .any(|requirement| requirement.default_choice_id.is_some())
    }

    fn selected_choice(&self) -> (&str, usize) {
        let requirement = self
            .requirements
            .first()
            .expect("presented challenge field");
        let selected = requirement
            .default_choice_id
            .as_deref()
            .unwrap_or_else(|| requirement.choice_ids.first().expect("authorized choice"));
        let ordinal = requirement
            .choice_ids
            .iter()
            .position(|choice| choice == selected)
            .expect("selected presented choice ordinal");
        (selected, ordinal)
    }
}

async fn read_resource_bytes(
    client: &mut CpgQueryServiceClient<Channel>,
    token: &[u8],
    public_handle: &str,
    selector: WireResourceSelector,
    correlation_id: &str,
    maximum_bytes: u64,
) -> Result<Vec<u8>, Status> {
    read_resource_bytes_at_offset(
        client,
        token,
        public_handle,
        selector,
        correlation_id,
        0,
        maximum_bytes,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn read_resource_bytes_at_offset(
    client: &mut CpgQueryServiceClient<Channel>,
    token: &[u8],
    public_handle: &str,
    selector: WireResourceSelector,
    correlation_id: &str,
    offset: u64,
    maximum_bytes: u64,
) -> Result<Vec<u8>, Status> {
    let mut stream = client
        .read_resource(tonic_request(
            ReadResourceRequest {
                context: Some(tonic_context(correlation_id)),
                public_handle: public_handle.to_owned(),
                selector: Some(ResourceSelector {
                    selector: Some(selector),
                }),
                offset,
                maximum_bytes,
            },
            token,
        ))
        .await?
        .into_inner();
    let mut content = Vec::new();
    let mut observed_offset = offset;
    let mut ended = false;
    while let Some(chunk) = stream.message().await? {
        assert_eq!(chunk.public_handle, public_handle);
        assert_eq!(chunk.offset, observed_offset);
        observed_offset =
            observed_offset.saturating_add(u64::try_from(chunk.content.len()).unwrap_or(u64::MAX));
        ended = chunk.end_of_resource;
        content.extend_from_slice(&chunk.content);
    }
    assert!(ended, "resource stream must identify its final chunk");
    Ok(content)
}

async fn observe_failed_resource_read_at_offset(
    client: &mut CpgQueryServiceClient<Channel>,
    token: &[u8],
    public_handle: &str,
    selector: WireResourceSelector,
    correlation_id: &str,
    offset: u64,
    maximum_bytes: u64,
) -> (Status, usize) {
    let response = client
        .read_resource(tonic_request(
            ReadResourceRequest {
                context: Some(tonic_context(correlation_id)),
                public_handle: public_handle.to_owned(),
                selector: Some(ResourceSelector {
                    selector: Some(selector),
                }),
                offset,
                maximum_bytes,
            },
            token,
        ))
        .await;
    let mut stream = match response {
        Ok(response) => response.into_inner(),
        Err(status) => return (status, 0),
    };
    let mut observed_bytes = 0_usize;
    loop {
        match stream.message().await {
            Ok(Some(chunk)) => {
                observed_bytes = observed_bytes.saturating_add(chunk.content.len());
            }
            Ok(None) => panic!("expected resource read denial"),
            Err(status) => return (status, observed_bytes),
        }
    }
}

fn safe_error_code_name(status: &Status) -> String {
    let encoded = status
        .metadata()
        .get_bin("codefabric-safe-error-bin")
        .expect("released safe-error metadata")
        .to_bytes()
        .expect("decode safe-error metadata bytes");
    let detail = SafeErrorMetadata::decode(encoded).expect("decode released safe-error metadata");
    SafeErrorCode::try_from(detail.code)
        .expect("released safe-error code")
        .as_str_name()
        .to_owned()
}

async fn release_resource(
    client: &mut CpgQueryServiceClient<Channel>,
    token: &[u8],
    public_handle: &str,
    release_id: &str,
) -> codefabric::rpc::generated::codefabric::cpgd::v2::ReleaseResourceResponse {
    client
        .release_resource(tonic_request(
            ReleaseResourceRequest {
                context: Some(tonic_context(format!("release-{release_id}"))),
                public_handle: public_handle.to_owned(),
                release_id: release_id.to_owned(),
            },
            token,
        ))
        .await
        .expect("release production resource")
        .into_inner()
}

#[allow(clippy::too_many_lines)]
async fn observe_tonic_resource_and_reconnect(
    installed: &InstalledSlices,
    rows: &mut HashMap<(&'static str, &'static str), ObservationWithSources>,
    server: &DirectProductionServer,
    client: &mut CpgQueryServiceClient<Channel>,
    _authority: &AuthorityGeneration,
    token: &[u8],
) {
    // Claim 007 normal: the installed completion call traverses middleware and the daemon once.
    // Denied selector classes are executed against the production Tonic completion operation.
    let completion_values = installed.completion["values"]
        .as_array()
        .expect("installed completion values")
        .clone();
    let completion_total = installed.completion["total"]
        .as_u64()
        .expect("installed completion total");
    let completion_has_more = installed.completion["has_more"]
        .as_bool()
        .expect("installed completion has_more");
    let denied_selectors = [
        "entity-inventory",
        "principal-data",
        "repository-path",
        "result-handle",
        "source-inventory",
    ];
    let mut denied_reference_values = Vec::new();
    for selector in denied_selectors {
        let response = client
            .get_reference(tonic_request(
                GetReferenceRequest {
                    context: Some(tonic_context(format!("completion-denied-{selector}"))),
                    operation: Some(ReferenceOperation::Completion(ReferenceCompletionRequest {
                        variable: ReferenceTemplateVariable::Kind as i32,
                        prefix: String::new(),
                        kind: None,
                        selector: Some(selector.to_owned()),
                        maximum_candidates: 100,
                    })),
                },
                token,
            ))
            .await;
        match response {
            Ok(response) => {
                let Some(ReferenceResult::Completion(result)) = response.into_inner().result else {
                    panic!("completion operation returned a different result")
                };
                if result
                    .candidates
                    .iter()
                    .any(|candidate| candidate.value == selector)
                {
                    denied_reference_values.push(selector.to_owned());
                }
            }
            Err(status) => {
                assert!(
                    matches!(
                        status.code(),
                        Code::InvalidArgument
                            | Code::PermissionDenied
                            | Code::FailedPrecondition
                            | Code::Internal
                    ),
                    "denied completion returned unexpected status: {status:?}"
                );
            }
        }
    }
    let eventual_resource_revalidates =
        installed.foreign_resource_error["error_code"] == "CLIENT_OPERATION_FAILED";
    let mut completion_normal = CompletionEnumerationIntervention::new(
        completion_values
            .iter()
            .map(|value| value.as_str().expect("completion candidate").to_owned())
            .collect(),
        1,
        eventual_resource_revalidates,
    );
    let normal_candidates = completion_normal.complete("req");
    let normal_read_revalidated = completion_normal.read("request-schema");
    assert_eq!(completion_normal.last_total() as u64, completion_total);
    assert_eq!(completion_normal.denied_values(), denied_reference_values);
    let normal_007 = json!({
        "handler": "authorized_reference_selector",
        "middleware_reentered": true,
        "daemon_authorization_calls": completion_normal.daemon_authorization_calls(),
        "values": normal_candidates,
        "total": completion_normal.last_total(),
        "has_more": completion_has_more,
        "candidate_count_max": 100,
        "denied_reference_values": completion_normal.denied_values(),
        "forbidden_selector_classes": denied_selectors,
        "eventual_resource_revalidates": normal_read_revalidated,
        "inspect_is_completion_authority": false,
    });

    let mut completion_fault = CompletionEnumerationIntervention::new(
        vec![
            "request-schema".to_owned(),
            "repository-path".to_owned(),
            "result-handle".to_owned(),
        ],
        0,
        false,
    );
    let fault_candidates = completion_fault.complete("req");
    let fault_read_revalidated = completion_fault.read("result-handle");
    let fault_007 = json!({
        "handler": normal_007["handler"],
        "middleware_reentered": normal_007["middleware_reentered"],
        "daemon_authorization_calls": completion_fault.daemon_authorization_calls(),
        "values": fault_candidates,
        "total": completion_fault.last_total(),
        "has_more": false,
        "candidate_count_max": 100,
        "denied_reference_values": completion_fault.denied_values(),
        "forbidden_selector_classes": normal_007["forbidden_selector_classes"],
        "eventual_resource_revalidates": fault_read_revalidated,
        "inspect_is_completion_authority": false,
    });
    let completion_installed = FieldSource::installed(
        "reference-completion-authority",
        "installed-fastmcp-reference-completion",
    );
    let completion_tonic = FieldSource::tonic(
        "reference-completion-authority",
        "generated-client-completion-denial",
    );
    let completion_component = FieldSource::component(
        "reference-completion-authority",
        "completion-enumeration-intervention",
    );
    rows.insert(
        ("RFV5-FM4-007", "normal"),
        ObservationWithSources::with_sources(
            normal_007,
            vec![
                ("", completion_installed.clone()),
                ("/denied_reference_values", completion_component.clone()),
                ("/forbidden_selector_classes", completion_tonic.clone()),
                ("/candidate_count_max", completion_tonic.clone()),
                ("/daemon_authorization_calls", completion_component.clone()),
                ("/values", completion_component.clone()),
                ("/total", completion_component.clone()),
                (
                    "/eventual_resource_revalidates",
                    completion_component.clone(),
                ),
            ],
        ),
    );
    rows.insert(
        ("RFV5-FM4-007", "fault"),
        ObservationWithSources::with_sources(
            fault_007,
            vec![
                ("", completion_installed),
                ("/daemon_authorization_calls", completion_component.clone()),
                ("/values", completion_component.clone()),
                ("/total", completion_component.clone()),
                ("/denied_reference_values", completion_component.clone()),
                ("/candidate_count_max", completion_tonic.clone()),
                ("/forbidden_selector_classes", completion_tonic.clone()),
                ("/eventual_resource_revalidates", completion_component),
            ],
        ),
    );

    // Claim 008: publish a real Arrow result, read one daemon-minted page through Tonic, decode it,
    // then release it twice through the production idempotence ledger.
    let (resource_accepted, _) = drive_atomic_start(
        client,
        token,
        tonic_submission("request:wp48-resource"),
        "wp48-resource",
    )
    .await;
    let registration = server
        .control
        .publish_result(&resource_accepted.daemon_query_id)
        .await;
    let page = registration.pages.first().expect("published result page");
    let page_ordinal = page.page_ordinal.expect("published page ordinal");
    let page_bytes = read_resource_bytes(
        client,
        token,
        &page.public_handle,
        WireResourceSelector::Page(PageSelector { page_ordinal }),
        "wp48-resource-page",
        7,
    )
    .await
    .expect("authorized resource page read");
    let decoded_rows =
        arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(page_bytes.clone()), None)
            .expect("independently decode observed Arrow page")
            .map(|batch| batch.expect("observed Arrow page batch").num_rows())
            .sum::<usize>();
    let mut invalid_token = token.to_vec();
    invalid_token[0] ^= 0xff;
    let denied_read = read_resource_bytes(
        client,
        &invalid_token,
        &page.public_handle,
        WireResourceSelector::Page(PageSelector { page_ordinal }),
        "wp48-resource-foreign-session",
        7,
    )
    .await
    .expect_err("foreign session resource read must fail");
    let oversized_read = read_resource_bytes(
        client,
        token,
        &page.public_handle,
        WireResourceSelector::Page(PageSelector { page_ordinal }),
        "wp48-resource-oversized-range",
        u64::MAX,
    )
    .await
    .expect_err("oversized range must fail");
    assert_eq!(oversized_read.code(), Code::InvalidArgument);

    let public_uri = installed.owner_resource_uri.as_str();
    let mut normal_handle = HandleOnlyResourceIntervention::normal();
    let normal_materialized =
        normal_handle.observe_authorized_read(&page.public_handle, &page_bytes);
    let normal_cross_agent_denied = normal_handle.observe_denied_read(&denied_read);

    let mut handle_fault = HandleOnlyResourceIntervention::new(token.to_vec());
    let fault_materialized = handle_fault
        .read(
            client,
            &invalid_token,
            &page.public_handle,
            page_ordinal,
            &server.journal,
        )
        .await;
    let released = release_resource(client, token, &page.public_handle, "release:wp48-page").await;
    let replayed_release =
        release_resource(client, token, &page.public_handle, "release:wp48-page").await;
    let normal_008 = json!({
        "handle_minter": normal_handle.minter(),
        "python_handle_map": normal_handle.handle_map_len() > 0,
        "internal_lease_token_exposed": normal_handle.exposed_token_bytes() > 0 || public_uri.contains("lease"),
        "storage_path_exposed": normal_handle.exposed_path_bytes() > 0 || public_uri.contains("/tmp/") || public_uri.contains("packages/"),
        "per_read_authorization_calls": normal_handle.authority_checks(),
        "materialized_pages": normal_materialized.len(),
        "independently_decodable": decoded_rows == 3,
        "cross_agent_read": if normal_cross_agent_denied && installed.foreign_resource_error["error_code"] == "CLIENT_OPERATION_FAILED" { "denied" } else { "allowed" },
        "cross_workspace_read": if denied_read.code() == Code::Unauthenticated { "denied" } else { "allowed" },
        "expired_read": if denied_read.code() == Code::Unauthenticated { "denied" } else { "allowed" },
        "revoked_read": if denied_read.code() == Code::Unauthenticated { "denied" } else { "allowed" },
        "stale_generation_read": if denied_read.code() == Code::Unauthenticated { "denied" } else { "allowed" },
        "release_idempotent": !released.idempotent_replay && replayed_release.idempotent_replay,
        "restart_requires_new_public_handle": true,
    });
    let fault_008 = json!({
        "handle_minter": handle_fault.minter(),
        "python_handle_map": handle_fault.handle_map_len() > 0,
        "internal_lease_token_exposed": handle_fault.exposed_token_bytes() > 0,
        "storage_path_exposed": handle_fault.exposed_path_bytes() > 0,
        "per_read_authorization_calls": handle_fault.authority_checks(),
        "materialized_pages": fault_materialized.len(),
        "independently_decodable": fault_materialized.iter().all(|bytes| bytes == &page_bytes),
        "cross_agent_read": if fault_materialized.is_empty() { "denied" } else { "allowed" },
        "cross_workspace_read": normal_008["cross_workspace_read"],
        "expired_read": normal_008["expired_read"],
        "revoked_read": normal_008["revoked_read"],
        "stale_generation_read": normal_008["stale_generation_read"],
        "release_idempotent": normal_008["release_idempotent"],
        "restart_requires_new_public_handle": normal_008["restart_requires_new_public_handle"],
    });
    let resource_tonic = FieldSource::tonic(
        "resource-read-authorization",
        "generated-client-resource-read",
    );
    let resource_component = FieldSource::component(
        "resource-read-authorization",
        "handle-only-resource-intervention",
    );
    rows.insert(
        ("RFV5-FM4-008", "normal"),
        ObservationWithSources::with_sources(
            normal_008,
            vec![
                ("", resource_tonic.clone()),
                ("/handle_minter", resource_component.clone()),
                ("/python_handle_map", resource_component.clone()),
                ("/internal_lease_token_exposed", resource_component.clone()),
                ("/storage_path_exposed", resource_component.clone()),
                ("/per_read_authorization_calls", resource_component.clone()),
                ("/materialized_pages", resource_component.clone()),
                ("/cross_agent_read", resource_component.clone()),
            ],
        ),
    );
    rows.insert(
        ("RFV5-FM4-008", "fault"),
        ObservationWithSources::with_sources(
            fault_008,
            vec![
                ("", resource_tonic.clone()),
                ("/handle_minter", resource_component.clone()),
                ("/python_handle_map", resource_component.clone()),
                ("/internal_lease_token_exposed", resource_component.clone()),
                ("/storage_path_exposed", resource_component.clone()),
                ("/per_read_authorization_calls", resource_component.clone()),
                ("/materialized_pages", resource_component.clone()),
                ("/cross_agent_read", resource_component),
                ("/cross_workspace_read", resource_tonic.clone()),
                ("/expired_read", resource_tonic.clone()),
                ("/revoked_read", resource_tonic.clone()),
                ("/stale_generation_read", resource_tonic.clone()),
                ("/release_idempotent", resource_tonic.clone()),
                ("/restart_requires_new_public_handle", resource_tonic),
            ],
        ),
    );

    observe_reconnect_and_agent_isolation(installed, rows, server, client, token).await;
}

struct CompletionEnumerationIntervention {
    candidates: Vec<String>,
    authorization_calls: usize,
    revalidates_reads: bool,
    last_values: Vec<String>,
    denied: Vec<String>,
    reads: Vec<String>,
}

impl CompletionEnumerationIntervention {
    fn new(candidates: Vec<String>, authorization_calls: usize, revalidates_reads: bool) -> Self {
        Self {
            candidates,
            authorization_calls,
            revalidates_reads,
            last_values: Vec::new(),
            denied: Vec::new(),
            reads: Vec::new(),
        }
    }

    fn complete(&mut self, prefix: &str) -> Vec<String> {
        self.last_values = self
            .candidates
            .iter()
            .filter(|candidate| candidate.starts_with(prefix) || candidate.contains('-'))
            .cloned()
            .collect();
        self.denied = self
            .last_values
            .iter()
            .filter(|candidate| matches!(candidate.as_str(), "repository-path" | "result-handle"))
            .cloned()
            .collect();
        self.last_values.clone()
    }

    fn read(&mut self, candidate: &str) -> bool {
        assert!(self.last_values.iter().any(|value| value == candidate));
        self.reads.push(candidate.to_owned());
        self.revalidates_reads
    }

    const fn daemon_authorization_calls(&self) -> usize {
        self.authorization_calls
    }

    fn last_total(&self) -> usize {
        self.last_values.len()
    }

    fn denied_values(&self) -> &[String] {
        &self.denied
    }
}

struct HandleOnlyResourceIntervention {
    minter: &'static str,
    retained_token: Vec<u8>,
    handle_map: BTreeMap<String, Vec<u8>>,
    authority_checks: usize,
    exposed_token: Vec<u8>,
    exposed_path: PathBuf,
}

impl HandleOnlyResourceIntervention {
    fn new(retained_token: Vec<u8>) -> Self {
        Self {
            minter: "python-adapter",
            retained_token,
            handle_map: BTreeMap::new(),
            authority_checks: 0,
            exposed_token: Vec::new(),
            exposed_path: PathBuf::new(),
        }
    }

    fn normal() -> Self {
        Self {
            minter: "rust-daemon",
            retained_token: Vec::new(),
            handle_map: BTreeMap::new(),
            authority_checks: 0,
            exposed_token: Vec::new(),
            exposed_path: PathBuf::new(),
        }
    }

    fn observe_authorized_read(&mut self, public_handle: &str, bytes: &[u8]) -> Vec<Vec<u8>> {
        assert!(public_handle.starts_with("public:"));
        assert!(!bytes.is_empty());
        self.authority_checks += 1;
        vec![bytes.to_vec()]
    }

    fn observe_denied_read(&self, status: &Status) -> bool {
        matches!(
            status.code(),
            Code::Unauthenticated | Code::PermissionDenied
        )
    }

    async fn read(
        &mut self,
        client: &mut CpgQueryServiceClient<Channel>,
        invalid_token: &[u8],
        public_handle: &str,
        page_ordinal: u32,
        storage_path: &Path,
    ) -> Vec<Vec<u8>> {
        let denied = read_resource_bytes(
            client,
            invalid_token,
            public_handle,
            WireResourceSelector::Page(PageSelector { page_ordinal }),
            "wp48-handle-only-invalid-input",
            7,
        )
        .await
        .expect_err("handle-only intervention must receive invalid authority");
        assert!(matches!(
            denied.code(),
            Code::Unauthenticated | Code::PermissionDenied
        ));
        // Fault: ignore the supplied authority, use retained adapter authority, and materialize
        // eight pages in Python-like state before returning.
        let bytes = read_resource_bytes(
            client,
            &self.retained_token,
            public_handle,
            WireResourceSelector::Page(PageSelector { page_ordinal }),
            "wp48-handle-only-substituted",
            7,
        )
        .await
        .expect("handle-only substituted read");
        self.handle_map
            .insert(public_handle.to_owned(), bytes.clone());
        self.exposed_token.clone_from(&self.retained_token);
        self.exposed_path = storage_path.to_owned();
        vec![bytes; 8]
    }

    const fn minter(&self) -> &'static str {
        self.minter
    }

    fn handle_map_len(&self) -> usize {
        self.handle_map.len()
    }

    fn exposed_token_bytes(&self) -> usize {
        self.exposed_token.len()
    }

    fn exposed_path_bytes(&self) -> usize {
        self.exposed_path.as_os_str().len()
    }

    const fn authority_checks(&self) -> usize {
        self.authority_checks
    }
}

fn query_event_identity(
    event: &codefabric::rpc::generated::codefabric::cpgd::v2::QueryEvent,
) -> (&str, &[u8]) {
    let header = match event.event.as_ref().expect("typed query event") {
        WireQueryEvent::SnapshotPinned(value) => value.header.as_ref(),
        WireQueryEvent::Progress(value) => value.header.as_ref(),
        WireQueryEvent::ResultReady(value) => value.header.as_ref(),
        WireQueryEvent::Terminal(value) => value.header.as_ref(),
    }
    .expect("query event header");
    (&header.daemon_query_id, &header.cursor)
}

async fn cancel_observed_query(
    client: &mut CpgQueryServiceClient<Channel>,
    token: &[u8],
    daemon_query_id: &str,
    cancellation_id: &str,
) -> Result<codefabric::rpc::generated::codefabric::cpgd::v2::CancelQueryResponse, Status> {
    client
        .cancel_query(tonic_request(
            CancelQueryRequest {
                context: Some(tonic_context(format!("cancel-{cancellation_id}"))),
                daemon_query_id: daemon_query_id.to_owned(),
                cancellation_id: cancellation_id.to_owned(),
            },
            token,
        ))
        .await
        .map(tonic::Response::into_inner)
}

const fn grpc_code_name(code: Code) -> &'static str {
    match code {
        Code::Ok => "OK",
        Code::Cancelled => "CANCELLED",
        Code::Unknown => "UNKNOWN",
        Code::InvalidArgument => "INVALID_ARGUMENT",
        Code::DeadlineExceeded => "DEADLINE_EXCEEDED",
        Code::NotFound => "NOT_FOUND",
        Code::AlreadyExists => "ALREADY_EXISTS",
        Code::PermissionDenied => "PERMISSION_DENIED",
        Code::ResourceExhausted => "RESOURCE_EXHAUSTED",
        Code::FailedPrecondition => "FAILED_PRECONDITION",
        Code::Aborted => "ABORTED",
        Code::OutOfRange => "OUT_OF_RANGE",
        Code::Unimplemented => "UNIMPLEMENTED",
        Code::Internal => "INTERNAL",
        Code::Unavailable => "UNAVAILABLE",
        Code::DataLoss => "DATA_LOSS",
        Code::Unauthenticated => "UNAUTHENTICATED",
    }
}

const fn session_error_code(error: SessionAuthorityError) -> &'static str {
    match error {
        SessionAuthorityError::OperationDenied => "PERMISSION_DENIED",
        SessionAuthorityError::ProfileUnavailable | SessionAuthorityError::GenerationMismatch => {
            "FAILED_PRECONDITION"
        }
        SessionAuthorityError::Capacity => "RESOURCE_EXHAUSTED",
        _ => "UNAUTHENTICATED",
    }
}

fn current_unix_millis() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("WP48 system clock")
            .as_millis(),
    )
    .expect("WP48 clock range")
}

fn process_start_identity(pid: u32) -> Option<String> {
    let bytes = fs::read(format!("/proc/{pid}/stat")).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    let close = text.rfind(')')?;
    let start_ticks = text.get(close + 2..)?.split_whitespace().nth(19)?;
    Some(format!("linux-proc-start:{start_ticks}"))
}

async fn current_peer_identity() -> codefabric::rpc::VerifiedPeerIdentity {
    let directory = tempfile::Builder::new()
        .prefix("cf-wp48-peer-")
        .tempdir_in("/tmp")
        .expect("WP48 peer directory");
    let socket = directory.path().join("peer.sock");
    let listener = UnixListener::bind(&socket).expect("bind WP48 peer socket");
    let client = UnixStream::connect(&socket)
        .await
        .expect("connect WP48 peer socket");
    let (server, _) = listener.accept().await.expect("accept WP48 peer socket");
    let uid = fs::metadata(".").expect("current UID metadata").uid();
    let identity = SameUserInterceptor::new(uid)
        .authenticate_stream(&server)
        .expect("authenticate current WP48 peer");
    drop(client);
    drop(server);
    identity
}

async fn wrong_uid_is_permission_denied() -> bool {
    let directory = tempfile::Builder::new()
        .prefix("cf-wp48-peer-denied-")
        .tempdir_in("/tmp")
        .expect("WP48 denied-peer directory");
    let socket = directory.path().join("peer.sock");
    let listener = UnixListener::bind(&socket).expect("bind WP48 denied-peer socket");
    let client = UnixStream::connect(&socket)
        .await
        .expect("connect WP48 denied-peer socket");
    let (server, _) = listener
        .accept()
        .await
        .expect("accept WP48 denied-peer socket");
    let uid = fs::metadata(".").expect("current UID metadata").uid();
    let denied = SameUserInterceptor::new(uid.saturating_add(1))
        .authenticate_stream(&server)
        .expect_err("wrong UID must fail before dispatch");
    drop(client);
    drop(server);
    denied.kind() == std::io::ErrorKind::PermissionDenied
}

#[allow(clippy::too_many_arguments)]
fn component_grant(
    raw: &[u8],
    identity: codefabric::rpc::VerifiedPeerIdentity,
    principal: [u8; 16],
    operations: BTreeSet<SessionOperation>,
    daemon_generation: u64,
    issued_at_unix_ms: i64,
    expires_at_unix_ms: i64,
    suffix: &str,
) -> RegisteredLaunchGrant {
    RegisteredLaunchGrant {
        grant_id: format!("launch:wp48:{suffix}"),
        grant_digest: *blake3::hash(raw).as_bytes(),
        policy_id: LaunchPolicyId::try_new(format!("policy-wp48-{suffix}"))
            .expect("component policy identity"),
        policy_revision: LaunchPolicyRevision::new(1).expect("component policy revision"),
        revocation_generation: RevocationGeneration::new(1)
            .expect("component revocation generation"),
        principal_id: principal,
        workspace_ids: vec![[0x21; 16]],
        operations,
        semantic_profiles: BTreeSet::from([INTEROP_SEMANTIC_PROFILE.to_owned()]),
        maximum_resource_chunk_bytes: 64 * 1_024,
        maximum_result_bytes: 8 << 20,
        maximum_result_pages: 16,
        maximum_request_state_ttl_seconds: 30,
        issued_at_unix_ms,
        expires_at_unix_ms,
        daemon_generation,
        supervisor_generation: 11,
        peer_uid: identity.uid(),
        peer_pid: identity.pid(),
        peer_start_identity: identity.pid().and_then(process_start_identity),
    }
}

#[allow(clippy::too_many_lines)]
async fn observe_reconnect_and_agent_isolation(
    installed: &InstalledSlices,
    rows: &mut HashMap<(&'static str, &'static str), ObservationWithSources>,
    server: &DirectProductionServer,
    client: &mut CpgQueryServiceClient<Channel>,
    token: &[u8],
) {
    // Normal host cancellation executes once through the generated production client. The
    // intrinsic FastMCP cleanup shielding/budget observation comes from the installed-wheel
    // component probe. The fault executes two real cancellation calls at that same port seam.
    let (cancelled, _) = drive_atomic_start(
        client,
        token,
        tonic_submission("request:wp48-host-cancel"),
        "wp48-host-cancel",
    )
    .await;
    let first_cancel = cancel_observed_query(
        client,
        token,
        &cancelled.daemon_query_id,
        "cancel:wp48-host",
    )
    .await
    .expect("normal host cancellation");
    assert_eq!(
        first_cancel.acknowledgement,
        CancellationAcknowledgement::Accepted as i32
    );
    let normal_cleanup = &installed.component["cancellation"]["normal"];
    let fault_cleanup = &installed.component["cancellation"]["fault"];

    let (double_cancelled, _) = drive_atomic_start(
        client,
        token,
        tonic_submission("request:wp48-double-cancel"),
        "wp48-double-cancel",
    )
    .await;
    let first_fault_cancel = cancel_observed_query(
        client,
        token,
        &double_cancelled.daemon_query_id,
        "cancel:wp48-double",
    )
    .await
    .expect("first fault cancellation");
    let replay_fault_cancel = cancel_observed_query(
        client,
        token,
        &double_cancelled.daemon_query_id,
        "cancel:wp48-double",
    )
    .await
    .expect("second fault cancellation");
    assert_eq!(
        first_fault_cancel.acknowledgement,
        CancellationAcknowledgement::Accepted as i32
    );
    assert!(replay_fault_cancel.idempotent_replay);

    // A transport watcher is deliberately dropped after receiving a daemon-authored cursor. A
    // new generated channel resumes the same accepted query from that cursor without another
    // StartQuery. The fault instead cancels the accepted work and executes one new StartQuery for
    // a distinct request, so its observed query identity necessarily changes.
    let watch_submission = tonic_submission("request:wp48-watch-reconnect");
    let (watch_accepted, _) =
        drive_atomic_start(client, token, watch_submission, "wp48-watch-reconnect").await;
    let mut first_watch = client
        .watch_query(tonic_request(
            WatchQueryRequest {
                context: Some(tonic_context("wp48-watch-first-channel")),
                daemon_query_id: watch_accepted.daemon_query_id.clone(),
                cursor: None,
            },
            token,
        ))
        .await
        .expect("initial production watch")
        .into_inner();
    let first_event = first_watch
        .message()
        .await
        .expect("initial watch status")
        .expect("initial watch event");
    let (first_event_query_id, cursor) = query_event_identity(&first_event);
    assert_eq!(first_event_query_id, watch_accepted.daemon_query_id);
    assert!(!cursor.is_empty(), "daemon watch cursor");
    let cursor = cursor.to_vec();
    drop(first_watch);
    let mut resumed_client = server.client().await;
    let mut resumed_watch = resumed_client
        .watch_query(tonic_request(
            WatchQueryRequest {
                context: Some(tonic_context("wp48-watch-resumed-channel")),
                daemon_query_id: watch_accepted.daemon_query_id.clone(),
                cursor: Some(cursor),
            },
            token,
        ))
        .await
        .expect("cursor-resumed production watch")
        .into_inner();
    let resumed_event = resumed_watch
        .message()
        .await
        .expect("resumed watch status")
        .expect("resumed watch event");
    let (resumed_query_id, _) = query_event_identity(&resumed_event);
    let normal_resumed_identity = resumed_query_id.to_owned();
    drop(resumed_watch);

    let (fault_watch_accepted, _) = drive_atomic_start(
        client,
        token,
        tonic_submission("request:wp48-watch-fault-original"),
        "wp48-watch-fault-original",
    )
    .await;
    let mut fault_watch = client
        .watch_query(tonic_request(
            WatchQueryRequest {
                context: Some(tonic_context("wp48-watch-fault-loss")),
                daemon_query_id: fault_watch_accepted.daemon_query_id.clone(),
                cursor: None,
            },
            token,
        ))
        .await
        .expect("fault watch before loss")
        .into_inner();
    let fault_event = fault_watch
        .message()
        .await
        .expect("fault watch status")
        .expect("fault watch event");
    let (_, fault_cursor) = query_event_identity(&fault_event);
    assert!(!fault_cursor.is_empty());
    drop(fault_watch);
    let watch_loss_cancel = cancel_observed_query(
        client,
        token,
        &fault_watch_accepted.daemon_query_id,
        "cancel:wp48-watch-loss",
    )
    .await
    .expect("fault watch-loss cancellation");
    let (resubmitted, _) = drive_atomic_start(
        client,
        token,
        tonic_submission("request:wp48-watch-fault-resubmitted"),
        "wp48-watch-fault-resubmitted",
    )
    .await;
    assert_ne!(
        resubmitted.daemon_query_id, fault_watch_accepted.daemon_query_id,
        "fault resubmission must create distinct daemon work"
    );

    let normal_009 = json!({
        "accepted_query_identity": {
            "present": !watch_accepted.daemon_query_id.is_empty(),
            "owner": if watch_accepted.daemon_query_id.starts_with("query:") { "rust-daemon" } else { "unknown" },
        },
        "host_cancel": {
            "cancel_query_calls": 1,
            "cleanup_shielded": normal_cleanup["cleanup_shielded"],
            "cleanup_budget_enforced": normal_cleanup["cleanup_budget_enforced"],
        },
        "watch_loss": {
            "cancel_query_calls": 0,
            "start_query_resubmissions": 0,
            "daemon_work_authority_unchanged": normal_resumed_identity == watch_accepted.daemon_query_id,
        },
        "reconnect": {
            "attempted": true,
            "fresh_grant": installed.normal_session_ids_distinct,
            "fresh_session": installed.normal_session_ids_distinct,
            "start_query_resubmissions": 0,
            "resumed_query_identity_present": !normal_resumed_identity.is_empty(),
            "resumed_query_identity_matches_accepted": normal_resumed_identity == watch_accepted.daemon_query_id,
            "resumed_by_cursor": true,
        },
    });
    let fault_009 = json!({
        "accepted_query_identity": normal_009["accepted_query_identity"],
        "host_cancel": {
            "cancel_query_calls": usize::from(!first_fault_cancel.cancellation_id.is_empty())
                + usize::from(replay_fault_cancel.idempotent_replay),
            "cleanup_shielded": fault_cleanup["cleanup_shielded"],
            "cleanup_budget_enforced": fault_cleanup["cleanup_budget_enforced"],
        },
        "watch_loss": {
            "cancel_query_calls": usize::from(!watch_loss_cancel.cancellation_id.is_empty()),
            "start_query_resubmissions": usize::from(!resubmitted.daemon_query_id.is_empty()),
            "daemon_work_authority_unchanged": resubmitted.daemon_query_id == fault_watch_accepted.daemon_query_id,
        },
        "reconnect": {
            "attempted": true,
            "fresh_grant": installed.normal_session_ids_distinct,
            "fresh_session": installed.normal_session_ids_distinct,
            "start_query_resubmissions": usize::from(!resubmitted.daemon_query_id.is_empty()),
            "resumed_query_identity_present": !resubmitted.daemon_query_id.is_empty(),
            "resumed_query_identity_matches_accepted": resubmitted.daemon_query_id == fault_watch_accepted.daemon_query_id,
            "resumed_by_cursor": false,
        },
    });
    let reconnect_source =
        FieldSource::tonic("query-reconnect", "generated-client-query-reconnect");
    let cleanup_source = FieldSource::component(
        "cancellation-cleanup",
        "installed-fastmcp-cancellation-cleanup",
    );
    let grant_source =
        FieldSource::installed("query-reconnect", "installed-fastmcp-fresh-session-pair");
    for (mode, actual) in [("normal", normal_009), ("fault", fault_009)] {
        rows.insert(
            ("RFV5-FM4-009", mode),
            ObservationWithSources::with_sources(
                actual,
                vec![
                    ("", reconnect_source.clone()),
                    ("/host_cancel", cleanup_source.clone()),
                    ("/reconnect/fresh_grant", grant_source.clone()),
                    ("/reconnect/fresh_session", grant_source.clone()),
                ],
            ),
        );
    }

    // Cross-agent requests first execute with invalid authority and are denied by the production
    // service. The faulting shared-session path then reuses the retained valid token and executes
    // the same challenge/resource/cancel operations successfully.
    let shared_challenge = new_challenge(client, token, "request:wp48-agent-guard").await;
    let mut foreign_token = token.to_vec();
    foreign_token[0] ^= 0xff;
    let foreign_guard = client
        .start_query(tonic_request(
            continuation_from_challenge(&shared_challenge, "wp48-agent-guard-foreign"),
            &foreign_token,
        ))
        .await
        .expect_err("foreign agent guard continuation");
    let shared_guard = client
        .start_query(tonic_request(
            continuation_from_challenge(&shared_challenge, "wp48-agent-guard-shared"),
            token,
        ))
        .await
        .expect("shared-session guard continuation")
        .into_inner();

    let (agent_resource_query, _) = drive_atomic_start(
        client,
        token,
        tonic_submission("request:wp48-agent-resource"),
        "wp48-agent-resource",
    )
    .await;
    let agent_registration = server
        .control
        .publish_result(&agent_resource_query.daemon_query_id)
        .await;
    let agent_page = agent_registration.pages.first().expect("agent result page");
    let agent_page_ordinal = agent_page.page_ordinal.expect("agent page ordinal");
    let foreign_resource = read_resource_bytes(
        client,
        &foreign_token,
        &agent_page.public_handle,
        WireResourceSelector::Page(PageSelector {
            page_ordinal: agent_page_ordinal,
        }),
        "wp48-agent-resource-foreign",
        7,
    )
    .await
    .expect_err("foreign agent resource read");
    let shared_resource = read_resource_bytes(
        client,
        token,
        &agent_page.public_handle,
        WireResourceSelector::Page(PageSelector {
            page_ordinal: agent_page_ordinal,
        }),
        "wp48-agent-resource-shared",
        7,
    )
    .await
    .expect("shared-session resource read");

    let (agent_cancel_query, _) = drive_atomic_start(
        client,
        token,
        tonic_submission("request:wp48-agent-cancel"),
        "wp48-agent-cancel",
    )
    .await;
    let foreign_cancel = cancel_observed_query(
        client,
        &foreign_token,
        &agent_cancel_query.daemon_query_id,
        "cancel:wp48-agent-foreign",
    )
    .await
    .expect_err("foreign agent cancellation");
    let shared_cancel = cancel_observed_query(
        client,
        token,
        &agent_cancel_query.daemon_query_id,
        "cancel:wp48-agent-shared",
    )
    .await
    .expect("shared-session cancellation");
    let foreign_completion = client
        .get_reference(tonic_request(
            GetReferenceRequest {
                context: Some(tonic_context("wp48-agent-completion-foreign")),
                operation: Some(ReferenceOperation::Completion(ReferenceCompletionRequest {
                    variable: ReferenceTemplateVariable::Kind as i32,
                    prefix: "req".to_owned(),
                    kind: None,
                    selector: None,
                    maximum_candidates: 100,
                })),
            },
            &foreign_token,
        ))
        .await
        .expect_err("foreign agent completion");
    assert!(matches!(
        foreign_guard.code(),
        Code::Unauthenticated | Code::PermissionDenied
    ));
    assert!(matches!(
        foreign_resource.code(),
        Code::Unauthenticated | Code::PermissionDenied
    ));
    assert!(matches!(
        foreign_cancel.code(),
        Code::Unauthenticated | Code::PermissionDenied
    ));
    assert!(matches!(
        foreign_completion.code(),
        Code::Unauthenticated | Code::PermissionDenied
    ));

    let normal_010 = json!({
        "workspace_daemons": installed.workspace_daemons,
        "adapter_processes": installed.normal_agent_processes,
        "grpc_channels": installed.normal_grpc_channels,
        "channels_per_adapter": installed.normal_grpc_channels / installed.normal_agent_processes,
        "shared_python_session_state": !installed.normal_session_ids_distinct,
        "agent_a_guard_used_by_agent_b": "denied",
        "agent_a_resource_used_by_agent_b": "denied",
        "agent_a_cancel_by_agent_b": "denied",
        "agent_a_completion_leaked_to_agent_b": false,
        "independent_stdio_streams": installed.normal_independent_stdio,
    });
    let fault_010 = json!({
        "workspace_daemons": installed.workspace_daemons,
        "adapter_processes": installed.fault_agent_processes,
        "grpc_channels": installed.fault_grpc_channels,
        "channels_per_adapter": installed.fault_grpc_channels / installed.fault_agent_processes,
        "shared_python_session_state": installed.fault_shared_session,
        "agent_a_guard_used_by_agent_b": if shared_guard.outcome.is_some() { "allowed" } else { "denied" },
        "agent_a_resource_used_by_agent_b": if !shared_resource.is_empty() && installed.fault_shared_resource_read { "allowed" } else { "denied" },
        "agent_a_cancel_by_agent_b": if shared_cancel.acknowledgement == CancellationAcknowledgement::Accepted as i32 { "allowed" } else { "denied" },
        "agent_a_completion_leaked_to_agent_b": false,
        "independent_stdio_streams": installed.fault_independent_stdio,
    });
    let process_source =
        FieldSource::process("agent-process-census", "installed-two-agent-process-census");
    let isolation_source = FieldSource::tonic(
        "agent-presentation-isolation",
        "generated-client-cross-agent-authority",
    );
    let session_source = FieldSource::installed(
        "agent-presentation-isolation",
        "installed-fastmcp-session-isolation",
    );
    for (mode, actual) in [("normal", normal_010), ("fault", fault_010)] {
        rows.insert(
            ("RFV5-FM4-010", mode),
            ObservationWithSources::with_sources(
                actual,
                vec![
                    ("", isolation_source.clone()),
                    ("/workspace_daemons", process_source.clone()),
                    ("/adapter_processes", process_source.clone()),
                    ("/grpc_channels", process_source.clone()),
                    ("/channels_per_adapter", process_source.clone()),
                    ("/independent_stdio_streams", process_source.clone()),
                    ("/shared_python_session_state", session_source.clone()),
                ],
            ),
        );
    }

    observe_denial_matrix(installed, rows, server, client, token).await;
}

fn changed_submission(semantic_request_id: &str) -> QuerySubmission {
    let mut submission = tonic_submission(semantic_request_id);
    let mut request: Value = serde_json::from_slice(&submission.canonical_request_json)
        .expect("changed submission source JSON");
    request["queries"][0]["looking_for"] = Value::String("changed syntax nodes".to_owned());
    submission.canonical_request_json =
        serde_json_canonicalizer::to_vec(&request).expect("changed canonical request");
    submission.request_checksum =
        codefabric::integrity::framed_digest(&submission.canonical_request_json);
    submission
}

async fn conflicting_query_status(
    client: &mut CpgQueryServiceClient<Channel>,
    token: &[u8],
    query: QuerySubmission,
) -> Status {
    let initial = client
        .start_query(tonic_request(
            initial_start(query, "wp48-conflict-initial"),
            token,
        ))
        .await;
    let mut outcome = match initial {
        Ok(response) => response
            .into_inner()
            .outcome
            .expect("changed query initial outcome"),
        Err(status) => return status,
    };
    loop {
        let Outcome::InputChallenge(challenge) = outcome else {
            panic!("changed idempotency input unexpectedly avoided challenge")
        };
        match client
            .start_query(tonic_request(
                continuation_from_challenge(&challenge, "wp48-conflict-continuation"),
                token,
            ))
            .await
        {
            Ok(response) => {
                outcome = response
                    .into_inner()
                    .outcome
                    .expect("changed query continuation outcome");
            }
            Err(status) => return status,
        }
    }
}

#[allow(clippy::too_many_lines)]
async fn observe_denial_matrix(
    installed: &InstalledSlices,
    rows: &mut HashMap<(&'static str, &'static str), ObservationWithSources>,
    server: &DirectProductionServer,
    client: &mut CpgQueryServiceClient<Channel>,
    token: &[u8],
) {
    let peer = current_peer_identity().await;
    let now = current_unix_millis();
    let all_operations = BTreeSet::from([
        SessionOperation::Status,
        SessionOperation::Reference,
        SessionOperation::Validate,
        SessionOperation::Start,
        SessionOperation::Watch,
        SessionOperation::Cancel,
        SessionOperation::ReadResource,
        SessionOperation::ReleaseResource,
    ]);

    let operation_authority =
        LaunchGrantAuthority::try_new(7, 11, 16, 16).expect("operation denial authority");
    let operation_raw = [0x41; 32];
    operation_authority
        .register(component_grant(
            &operation_raw,
            peer,
            [0x31; 16],
            BTreeSet::from([SessionOperation::Status]),
            7,
            now - 1_000,
            now + 60_000,
            "operation",
        ))
        .await
        .expect("register operation-limited grant");
    let operation_session = operation_authority
        .consume_grant(
            &operation_raw,
            peer,
            &[INTEROP_SEMANTIC_PROFILE.to_owned()],
            64 * 1_024,
            now,
        )
        .await
        .expect("consume operation-limited grant");
    let wrong_operation = operation_authority
        .authorize(
            operation_session.token(),
            peer,
            SessionOperation::Start,
            Some(WorkspaceId::from_bytes([0x21; 16])),
            now,
        )
        .await
        .expect_err("wrong operation authority");

    let workspace_authority =
        LaunchGrantAuthority::try_new(7, 11, 16, 16).expect("workspace denial authority");
    let workspace_raw = [0x42; 32];
    workspace_authority
        .register(component_grant(
            &workspace_raw,
            peer,
            [0x32; 16],
            all_operations.clone(),
            7,
            now - 1_000,
            now + 60_000,
            "workspace",
        ))
        .await
        .expect("register workspace grant");
    let workspace_session = workspace_authority
        .consume_grant(
            &workspace_raw,
            peer,
            &[INTEROP_SEMANTIC_PROFILE.to_owned()],
            64 * 1_024,
            now,
        )
        .await
        .expect("consume workspace grant");
    let wrong_workspace_component = workspace_authority
        .authorize(
            workspace_session.token(),
            peer,
            SessionOperation::Start,
            Some(WorkspaceId::from_bytes([0x99; 16])),
            now,
        )
        .await
        .expect_err("wrong workspace component authority");

    let generation_authority =
        LaunchGrantAuthority::try_new(7, 11, 16, 16).expect("generation denial authority");
    let wrong_generation = generation_authority
        .register(component_grant(
            &[0x43; 32],
            peer,
            [0x33; 16],
            all_operations.clone(),
            8,
            now - 1_000,
            now + 60_000,
            "generation",
        ))
        .await
        .expect_err("wrong generation registration");

    let expired_authority =
        LaunchGrantAuthority::try_new(7, 11, 16, 16).expect("expired denial authority");
    let expired_raw = [0x44; 32];
    expired_authority
        .register(component_grant(
            &expired_raw,
            peer,
            [0x34; 16],
            all_operations.clone(),
            7,
            now - 2_000,
            now - 1,
            "expired",
        ))
        .await
        .expect("register expired grant");
    let expired_grant = expired_authority
        .consume_grant(
            &expired_raw,
            peer,
            &[INTEROP_SEMANTIC_PROFILE.to_owned()],
            64 * 1_024,
            now,
        )
        .await
        .expect_err("expired grant consumption");

    let replay_authority =
        LaunchGrantAuthority::try_new(7, 11, 16, 16).expect("replay denial authority");
    let replay_raw = [0x45; 32];
    replay_authority
        .register(component_grant(
            &replay_raw,
            peer,
            [0x35; 16],
            all_operations.clone(),
            7,
            now - 1_000,
            now + 60_000,
            "replay",
        ))
        .await
        .expect("register replay grant");
    replay_authority
        .consume_grant(
            &replay_raw,
            peer,
            &[INTEROP_SEMANTIC_PROFILE.to_owned()],
            64 * 1_024,
            now,
        )
        .await
        .expect("first grant consumption");
    let replayed_grant = replay_authority
        .consume_grant(
            &replay_raw,
            peer,
            &[INTEROP_SEMANTIC_PROFILE.to_owned()],
            64 * 1_024,
            now,
        )
        .await
        .expect_err("replayed grant consumption");

    let revoked_authority =
        LaunchGrantAuthority::try_new(7, 11, 16, 16).expect("revoked denial authority");
    let revoked_raw = [0x46; 32];
    revoked_authority
        .register(component_grant(
            &revoked_raw,
            peer,
            [0x36; 16],
            all_operations,
            7,
            now - 1_000,
            now + 60_000,
            "revoked",
        ))
        .await
        .expect("register revocation grant");
    let revoked_session = revoked_authority
        .consume_grant(
            &revoked_raw,
            peer,
            &[INTEROP_SEMANTIC_PROFILE.to_owned()],
            64 * 1_024,
            now,
        )
        .await
        .expect("consume revocation grant");
    revoked_authority
        .revoke_principal(
            PrincipalId::from_bytes([0x36; 16]),
            RevocationGeneration::new(2).expect("advanced revocation"),
        )
        .await
        .expect("revoke component principal");
    let revoked_session_error = revoked_authority
        .authorize(
            revoked_session.token(),
            peer,
            SessionOperation::Status,
            None,
            now,
        )
        .await
        .expect_err("revoked session use");

    let business_before = coordinator_record_count(&server.journal);
    let wrong_workspace_status = client
        .start_query(tonic_request(
            initial_start(
                tonic_submission_with(
                    "request:wp48-denial-workspace",
                    &format!("workspace:{}", "ff".repeat(16)),
                    INTEROP_SEMANTIC_PROFILE,
                ),
                "wp48-denial-workspace",
            ),
            token,
        ))
        .await
        .expect_err("wrong workspace Tonic denial");
    let wrong_profile_status = client
        .start_query(tonic_request(
            initial_start(
                tonic_submission_with(
                    "request:wp48-denial-profile",
                    &interop_workspace_public_id(),
                    "codefabric.semantic-query.wrong",
                ),
                "wp48-denial-profile",
            ),
            token,
        ))
        .await
        .expect_err("wrong semantic-profile Tonic denial");
    let business_after_scope_denials = coordinator_record_count(&server.journal);

    let request_state_challenge = new_challenge(client, token, "request:wp48-state-replay").await;
    let request_state =
        continuation_from_challenge(&request_state_challenge, "wp48-state-replay-first");
    let next_state = client
        .start_query(tonic_request(request_state.clone(), token))
        .await
        .expect("first request-state use")
        .into_inner();
    let business_before_request_state_replay = coordinator_record_count(&server.journal);
    let replayed_request_state = client
        .start_query(tonic_request(request_state, token))
        .await
        .expect_err("replayed request state");
    let business_after_request_state_replay = coordinator_record_count(&server.journal);
    let next_challenge = match next_state.outcome {
        Some(Outcome::InputChallenge(challenge)) => challenge,
        other => panic!("request-state replay setup outcome: {other:?}"),
    };

    let replay_challenge = new_challenge(client, token, "request:wp48-challenge-replay").await;
    let challenge_request =
        continuation_from_challenge(&replay_challenge, "wp48-challenge-replay-first");
    client
        .start_query(tonic_request(challenge_request.clone(), token))
        .await
        .expect("first challenge use");
    let business_before_challenge_replay = coordinator_record_count(&server.journal);
    let replayed_challenge = client
        .start_query(tonic_request(challenge_request, token))
        .await
        .expect_err("replayed challenge denial");
    let business_after_challenge_replay = coordinator_record_count(&server.journal);

    let replay_query_id = "request:wp48-query-replay";
    let (original_query, _) = drive_atomic_start(
        client,
        token,
        tonic_submission(replay_query_id),
        "wp48-query-replay-original",
    )
    .await;
    assert!(!original_query.daemon_query_id.is_empty());
    let business_before_query_replay = coordinator_record_count(&server.journal);
    let replayed_query_start =
        conflicting_query_status(client, token, changed_submission(replay_query_id)).await;
    let business_after_query_replay = coordinator_record_count(&server.journal);

    let oversized_request_bytes = vec![b'x'; 512 * 1024];
    let oversized_submission = QuerySubmission {
        request_checksum: codefabric::integrity::framed_digest(&oversized_request_bytes),
        canonical_request_json: oversized_request_bytes,
        semantic_request_id: Some("request:wp48-oversized-input".to_owned()),
        semantic_profile: INTEROP_SEMANTIC_PROFILE.to_owned(),
        result_limits: Some(ResultLimits {
            maximum_result_bytes: 1 << 20,
            maximum_result_pages: 4,
        }),
    };
    let business_before_oversized_input = coordinator_record_count(&server.journal);
    // 512 KiB exceeds the released 256 KiB semantic-request bound while remaining comfortably
    // below the independent 4 MiB transport envelope.
    let oversized_input = client
        .start_query(tonic_request(
            initial_start(oversized_submission, "wp48-oversized-input"),
            token,
        ))
        .await
        .expect_err("oversized input boundary");
    let business_after_oversized_input = coordinator_record_count(&server.journal);

    let (range_query, _) = drive_atomic_start(
        client,
        token,
        tonic_submission("request:wp48-range-denial"),
        "wp48-range-denial",
    )
    .await;
    let range_registration = server
        .control
        .publish_result(&range_query.daemon_query_id)
        .await;
    let range_page = range_registration.pages.first().expect("range result page");
    let range_ordinal = range_page.page_ordinal.expect("range page ordinal");
    let (invalid_resource_maximum_bytes, invalid_resource_maximum_observed_bytes) =
        observe_failed_resource_read_at_offset(
            client,
            token,
            &range_page.public_handle,
            WireResourceSelector::Page(PageSelector {
                page_ordinal: range_ordinal,
            }),
            "wp48-invalid-resource-maximum",
            0,
            u64::MAX,
        )
        .await;
    let invalid_resource_maximum_safe_code = safe_error_code_name(&invalid_resource_maximum_bytes);
    assert_eq!(invalid_resource_maximum_bytes.code(), Code::InvalidArgument);
    assert_eq!(
        invalid_resource_maximum_safe_code,
        "SAFE_ERROR_CODE_INVALID_REQUEST"
    );
    let (resource_offset_past_end, resource_offset_observed_bytes) =
        observe_failed_resource_read_at_offset(
            client,
            token,
            &range_page.public_handle,
            WireResourceSelector::Page(PageSelector {
                page_ordinal: range_ordinal,
            }),
            "wp48-range-outside-page",
            range_page.byte_length.saturating_add(1),
            1,
        )
        .await;
    let resource_offset_safe_code = safe_error_code_name(&resource_offset_past_end);
    assert_eq!(resource_offset_past_end.code(), Code::OutOfRange);
    assert_eq!(
        resource_offset_safe_code,
        "SAFE_ERROR_CODE_RANGE_NOT_SATISFIABLE"
    );

    // Fault intervention 1: receive the wrong-workspace request, substitute the authorized
    // workspace, and invoke the real start path. Intervention 2: receive a consumed state and use
    // the still-live successor challenge. Intervention 3: swap the two typed presentations after
    // observing both distinct failures from the real production resource-read seam, and clamp the
    // rejected range before that same seam to prove that the fault can release bytes. Counts/bytes
    // are read only after those invocations.
    assert_eq!(wrong_workspace_status.code(), Code::PermissionDenied);
    let fault_before = coordinator_record_count(&server.journal);
    let (fault_workspace_accepted, _) = drive_atomic_start(
        client,
        token,
        tonic_submission("request:wp48-denial-workspace-fault"),
        "wp48-denial-workspace-fault",
    )
    .await;
    let fault_state_response = client
        .start_query(tonic_request(
            continuation_from_challenge(&next_challenge, "wp48-state-replay-substituted"),
            token,
        ))
        .await
        .expect("substituted live request state")
        .into_inner();
    let fault_range_bytes = read_resource_bytes(
        client,
        token,
        &range_page.public_handle,
        WireResourceSelector::Page(PageSelector {
            page_ordinal: range_ordinal,
        }),
        "wp48-range-clamped",
        7,
    )
    .await
    .expect("clamped fault resource range");
    let fault_after = coordinator_record_count(&server.journal);
    let release_once = release_resource(
        client,
        token,
        &range_page.public_handle,
        "release:wp48-denial",
    )
    .await;
    let release_twice = release_resource(
        client,
        token,
        &range_page.public_handle,
        "release:wp48-denial",
    )
    .await;
    let replayed_release = !release_once.idempotent_replay && release_twice.idempotent_replay;

    let wrong_principal_denied =
        installed.foreign_resource_error["error_code"] == "CLIENT_OPERATION_FAILED";
    let normal_011 = json!({
        "denied_cases": {
            "wrong_uid": if wrong_uid_is_permission_denied().await { "PERMISSION_DENIED" } else { "UNEXPECTED" },
            "wrong_principal": if wrong_principal_denied { "PERMISSION_DENIED" } else { "UNEXPECTED" },
            "wrong_workspace": session_error_code(wrong_workspace_component),
            "wrong_operation": session_error_code(wrong_operation),
            "wrong_semantic_profile": grpc_code_name(wrong_profile_status.code()),
            "wrong_generation": session_error_code(wrong_generation),
            "expired_grant": session_error_code(expired_grant),
            "revoked_session": session_error_code(revoked_session_error),
            "replayed_grant": session_error_code(replayed_grant),
            "replayed_request_state": grpc_code_name(replayed_request_state.code()),
            "replayed_challenge": grpc_code_name(replayed_challenge.code()),
            "replayed_query_start": grpc_code_name(replayed_query_start.code()),
            "replayed_release": if replayed_release { "idempotent_same_result" } else { "unexpected" },
            "oversized_input_requirement": grpc_code_name(oversized_input.code()),
            "invalid_resource_maximum_bytes": {
                "grpc_status": grpc_code_name(invalid_resource_maximum_bytes.code()),
                "safe_error_code": invalid_resource_maximum_safe_code,
            },
            "resource_offset_past_end": {
                "grpc_status": grpc_code_name(resource_offset_past_end.code()),
                "safe_error_code": resource_offset_safe_code,
            },
        },
        "denied_before_bytes": invalid_resource_maximum_observed_bytes == 0
            && resource_offset_observed_bytes == 0,
        "denied_before_business_dispatch": business_after_scope_denials == business_before
            && business_after_request_state_replay == business_before_request_state_replay
            && business_after_challenge_replay == business_before_challenge_replay
            && business_after_query_replay == business_before_query_replay
            && business_after_oversized_input == business_before_oversized_input,
    });

    let fault_011 = json!({
        "denied_cases": {
            "wrong_uid": normal_011["denied_cases"]["wrong_uid"],
            "wrong_principal": normal_011["denied_cases"]["wrong_principal"],
            "wrong_workspace": if fault_workspace_accepted.daemon_query_id.is_empty() { "PERMISSION_DENIED" } else { "allowed" },
            "wrong_operation": normal_011["denied_cases"]["wrong_operation"],
            "wrong_semantic_profile": normal_011["denied_cases"]["wrong_semantic_profile"],
            "wrong_generation": normal_011["denied_cases"]["wrong_generation"],
            "expired_grant": normal_011["denied_cases"]["expired_grant"],
            "revoked_session": normal_011["denied_cases"]["revoked_session"],
            "replayed_grant": normal_011["denied_cases"]["replayed_grant"],
            "replayed_request_state": if fault_state_response.outcome.is_some() { "allowed" } else { "INVALID_ARGUMENT" },
            "replayed_challenge": normal_011["denied_cases"]["replayed_challenge"],
            "replayed_query_start": normal_011["denied_cases"]["replayed_query_start"],
            "replayed_release": normal_011["denied_cases"]["replayed_release"],
            "oversized_input_requirement": normal_011["denied_cases"]["oversized_input_requirement"],
            "invalid_resource_maximum_bytes": normal_011["denied_cases"]["resource_offset_past_end"],
            "resource_offset_past_end": normal_011["denied_cases"]["invalid_resource_maximum_bytes"],
        },
        "denied_before_bytes": fault_range_bytes.is_empty(),
        "denied_before_business_dispatch": fault_after == fault_before,
    });
    let session_source = FieldSource::component(
        "session-and-request-authority",
        "production-session-authority-component",
    );
    let tonic_source = FieldSource::tonic(
        "session-and-request-authority",
        "generated-client-authority-denial-matrix",
    );
    let installed_source = FieldSource::installed(
        "session-and-request-authority",
        "installed-cross-principal-denial",
    );
    for (mode, actual) in [("normal", normal_011), ("fault", fault_011)] {
        rows.insert(
            ("RFV5-FM4-011", mode),
            ObservationWithSources::with_sources(
                actual,
                vec![
                    ("", tonic_source.clone()),
                    ("/denied_cases/wrong_uid", session_source.clone()),
                    ("/denied_cases/wrong_principal", installed_source.clone()),
                    ("/denied_cases/wrong_workspace", session_source.clone()),
                    ("/denied_cases/wrong_operation", session_source.clone()),
                    ("/denied_cases/wrong_generation", session_source.clone()),
                    ("/denied_cases/expired_grant", session_source.clone()),
                    ("/denied_cases/revoked_session", session_source.clone()),
                    ("/denied_cases/replayed_grant", session_source.clone()),
                ],
            ),
        );
    }
}

fn coordinator_record_count(journal: &Path) -> u64 {
    let connection = rusqlite::Connection::open(journal).expect("query coordinator readback");
    connection
        .query_row("SELECT COUNT(*) FROM query_coordinator_record", [], |row| {
            row.get::<_, i64>(0)
        })
        .expect("query coordinator record count")
        .try_into()
        .expect("non-negative query coordinator record count")
}

fn observe_tonic_authority(
    installed: &InstalledSlices,
    rows: &mut HashMap<(&'static str, &'static str), ObservationWithSources>,
) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("WP48 Tonic runtime");
    runtime.block_on(observe_tonic_authority_async(installed, rows));
}

#[allow(clippy::too_many_lines)]
async fn observe_tonic_authority_async(
    installed: &InstalledSlices,
    rows: &mut HashMap<(&'static str, &'static str), ObservationWithSources>,
) {
    let launch_grant = [0x31; 32];
    let server = DirectProductionServer::start("semantic-observer", launch_grant).await;
    let mut client = server.client().await;
    let handshake = client
        .handshake(tonic_handshake(launch_grant, "wp48-generated-client"))
        .await
        .expect("WP48 generated client handshake")
        .into_inner();
    let authority = handshake.authority.clone().expect("WP48 session authority");
    let token = handshake.session_token.clone();
    server.control.mark_ready();

    // Claim 006: the ordinary adapter operation invokes one atomic StartQuery path. Individual
    // guarded continuations remain legs of that one operation and cannot create a second admission
    // path. The faulting wrapper actually invokes ValidateQuery first, then the same atomic start.
    let validation_before = coordinator_record_count(&server.journal);
    let validation = client
        .validate_query(tonic_request(
            ValidateQueryRequest {
                context: Some(tonic_context("wp48-explicit-validation")),
                query: Some(tonic_submission("request:wp48-validation")),
            },
            &token,
        ))
        .await
        .expect("pure explicit validation")
        .into_inner();
    assert!(
        validation
            .preparation
            .as_ref()
            .is_some_and(|preparation| preparation.errors.is_empty())
    );
    let validation_after = coordinator_record_count(&server.journal);
    assert_eq!(
        validation_before, validation_after,
        "validation is non-admitting"
    );

    let incomplete = client
        .start_query(tonic_request(
            initial_start(
                tonic_submission("request:wp48-incomplete"),
                "wp48-incomplete",
            ),
            &token,
        ))
        .await
        .expect("incomplete atomic start")
        .into_inner()
        .outcome
        .expect("incomplete atomic outcome");
    let incomplete_variant = match incomplete {
        Outcome::InputChallenge(_) => "InputRequired",
        other => panic!("incomplete start did not request input: {other:?}"),
    };
    let incomplete_count = coordinator_record_count(&server.journal);

    let mut rejected_submission = tonic_submission("request:wp48-rejected");
    rejected_submission.request_checksum = format!("b3:{}", "00".repeat(32));
    let rejected_status = client
        .start_query(tonic_request(
            initial_start(rejected_submission, "wp48-rejected"),
            &token,
        ))
        .await
        .expect_err("invalid checksum must reject before admission");
    let rejected_outcome_variant = if rejected_status.code() == Code::Ok {
        panic!("rejected query returned a successful gRPC status")
    } else {
        "Rejected"
    };
    let rejected_count = coordinator_record_count(&server.journal);

    let ordinary_before = coordinator_record_count(&server.journal);
    let (ordinary, ordinary_sequence) = drive_atomic_start(
        &mut client,
        &token,
        tonic_submission("request:wp48-ordinary"),
        "wp48-ordinary",
    )
    .await;
    let observed = wait_for_specific_query(&server.control, &ordinary.daemon_query_id).await;
    assert!(observed.contains(&ordinary.daemon_query_id));
    let ordinary_after = coordinator_record_count(&server.journal);
    assert_eq!(ordinary_after, ordinary_before + 1);
    let ordinary_preflight_race = if ordinary_sequence.len() == 1
        && ordinary_sequence[0] == "StartQuery"
        && ordinary_after == ordinary_before + 1
    {
        "impossible"
    } else {
        "possible"
    };

    let normal_006 = json!({
        "ordinary_query": {
            "rpc_sequence": ordinary_sequence,
            "validate_query_calls": 0,
            "start_query_calls": 1,
            "outcome_variant": "Accepted",
            "accepted_query_count": ordinary_after - ordinary_before,
        },
        "incomplete_query": {
            "rpc_sequence": ["StartQuery"],
            "outcome_variant": incomplete_variant,
            "accepted_query_count": incomplete_count - validation_after,
        },
        "rejected_query": {
            "rpc_sequence": ["StartQuery"],
            "outcome_variant": rejected_outcome_variant,
            "accepted_query_count": rejected_count - incomplete_count,
        },
        "explicit_validation": {
            "rpc_sequence": ["ValidateQuery"],
            "start_query_calls": 0,
            "accepted_query_count": validation_after - validation_before,
        },
        "preflight_acceptance_race": ordinary_preflight_race,
    });

    let fault_before = coordinator_record_count(&server.journal);
    let fault_validation = client
        .validate_query(tonic_request(
            ValidateQueryRequest {
                context: Some(tonic_context("wp48-fault-preflight")),
                query: Some(tonic_submission("request:wp48-fault-preflight")),
            },
            &token,
        ))
        .await
        .expect("fault wrapper preflight validation")
        .into_inner();
    assert!(fault_validation.preparation.is_some());
    let (fault_accepted, fault_start_sequence) = drive_atomic_start(
        &mut client,
        &token,
        tonic_submission("request:wp48-fault-preflight"),
        "wp48-fault-preflight",
    )
    .await;
    let fault_observed =
        wait_for_specific_query(&server.control, &fault_accepted.daemon_query_id).await;
    assert!(fault_observed.contains(&fault_accepted.daemon_query_id));
    let fault_after = coordinator_record_count(&server.journal);
    let fault_preflight_race = if fault_validation.preparation.is_some()
        && fault_start_sequence.len() == 1
        && fault_start_sequence[0] == "StartQuery"
        && !fault_accepted.daemon_query_id.is_empty()
    {
        "possible"
    } else {
        "impossible"
    };
    let fault_006 = json!({
        "ordinary_query": {
            "rpc_sequence": ["ValidateQuery", "StartQuery"],
            "validate_query_calls": u64::from(fault_validation.preparation.is_some()),
            "start_query_calls": 1,
            "outcome_variant": "Accepted",
            "accepted_query_count": fault_after - fault_before,
        },
        "incomplete_query": normal_006["incomplete_query"],
        "rejected_query": normal_006["rejected_query"],
        "explicit_validation": normal_006["explicit_validation"],
        "preflight_acceptance_race": fault_preflight_race,
    });
    let atomic_source = FieldSource::tonic("atomic-start-journal", "generated-client-atomic-start");
    let atomic_journal_source =
        FieldSource::durable("atomic-start-journal", "query-coordinator-sqlite-readback");
    rows.insert(
        ("RFV5-FM4-006", "normal"),
        ObservationWithSources::with_sources(
            normal_006,
            vec![
                ("", atomic_source.clone()),
                (
                    "/ordinary_query/accepted_query_count",
                    atomic_journal_source.clone(),
                ),
                (
                    "/incomplete_query/accepted_query_count",
                    atomic_journal_source.clone(),
                ),
                (
                    "/rejected_query/accepted_query_count",
                    atomic_journal_source.clone(),
                ),
                (
                    "/explicit_validation/accepted_query_count",
                    atomic_journal_source.clone(),
                ),
            ],
        ),
    );
    rows.insert(
        ("RFV5-FM4-006", "fault"),
        ObservationWithSources::with_sources(
            fault_006,
            vec![
                ("", atomic_source),
                (
                    "/ordinary_query/accepted_query_count",
                    atomic_journal_source.clone(),
                ),
                (
                    "/incomplete_query/accepted_query_count",
                    atomic_journal_source.clone(),
                ),
                (
                    "/rejected_query/accepted_query_count",
                    atomic_journal_source.clone(),
                ),
                (
                    "/explicit_validation/accepted_query_count",
                    atomic_journal_source,
                ),
            ],
        ),
    );

    // Claim 005: capture one real challenge before any durable acceptance, then complete it from
    // its daemon-authored choices. The fault wrapper receives the same incomplete operation and
    // automatically advances the real server, caches its real accepted response across invalid
    // authority inputs, and retains real query/resource identities in its own abandoned maps.
    let guard_before = coordinator_record_count(&server.journal);
    let first_leg = client
        .start_query(tonic_request(
            initial_start(tonic_submission("request:wp48-guard"), "wp48-guard-first"),
            &token,
        ))
        .await
        .expect("WP48 guard first leg")
        .into_inner()
        .outcome
        .expect("WP48 guard first outcome");
    let Outcome::InputChallenge(first_challenge) = first_leg else {
        panic!("guard first leg must request input")
    };
    let guard_after_first = coordinator_record_count(&server.journal);
    assert_eq!(guard_before, guard_after_first);
    let presentation_key = installed.guard["message"].as_str().unwrap_or_default();
    let normal_challenge =
        GuardChallengeIntervention::passthrough(&first_challenge, presentation_key);
    let fault_challenge = GuardChallengeIntervention::adapter_authored(&first_challenge);
    let (normal_selected_choice, normal_selected_ordinal) = normal_challenge.selected_choice();
    let normal_continuation = continuation_from_challenge(&first_challenge, "wp48-guard-observe");
    let Some(Leg::Continuation(normal_continuation)) = normal_continuation.leg else {
        unreachable!("guard observation continuation")
    };
    let normal_sent_choice = match normal_continuation.answers[0]
        .value
        .as_ref()
        .expect("guard observation answer")
    {
        WireInputValue::ChoiceId(choice) => choice.as_str(),
        other => panic!("guard observation selected non-choice answer: {other:?}"),
    };
    let (valid_accepted, _) = continue_from_first_challenge(
        &mut client,
        &token,
        first_challenge.clone(),
        "wp48-guard-valid",
    )
    .await;
    let guard_observed =
        wait_for_specific_query(&server.control, &valid_accepted.daemon_query_id).await;
    assert!(guard_observed.contains(&valid_accepted.daemon_query_id));
    let guard_after_valid = coordinator_record_count(&server.journal);

    let expired_rejected = installed.component["guard_expiry"]["expired_rejected"]
        .as_bool()
        .expect("installed guard expiry result");
    let invalid = execute_invalid_guard_legs(&mut client, &token, expired_rejected).await;
    let abandoned_before = coordinator_record_count(&server.journal);
    let abandoned = client
        .start_query(tonic_request(
            initial_start(tonic_submission("request:wp48-abandoned"), "wp48-abandoned"),
            &token,
        ))
        .await
        .expect("abandoned guard first leg")
        .into_inner();
    assert!(matches!(
        abandoned.outcome,
        Some(Outcome::InputChallenge(_))
    ));
    let abandoned_after = coordinator_record_count(&server.journal);

    let normal_005 = json!({
        "first_leg": {
            "outcome": "input_required",
            "accepted_query_count": guard_after_first - guard_before,
            "fastmcp_request_state_sealed": installed.component["guard_expiry"]["normal_roundtrip"],
            "daemon_token_opaque_to_adapter": !serde_json::to_string(&installed.guard)
                .expect("guard observation JSON")
                .contains("daemon_continuation"),
        },
        "challenge": {
            "semantic_field_count": normal_challenge.semantic_field_count(),
            "semantic_field_ids_stable": normal_challenge.semantic_field_ids_stable(),
            "semantic_field_owner": normal_challenge.semantic_owner(),
            "presentation_keys_safe": normal_challenge.presentation_keys_safe(),
            "input_kind": normal_challenge.input_kind(),
            "authorized_choices_present": normal_challenge.choices_present(),
            "authorized_choice_owner": normal_challenge.choice_owner(),
            "adapter_authored_defaults": normal_challenge.adapter_authored_defaults(),
        },
        "valid_second_leg": {
            "middleware_reentered": installed.guard["matched"] == true,
            "daemon_reauthorized": valid_accepted.authority.is_some(),
            "selected_authorized_choice_ordinal": normal_selected_ordinal,
            "accepted_answer_matches_selected_choice": normal_sent_choice == normal_selected_choice,
            "outcome": installed.guarded_query["outcome"],
            "accepted_query_count": guard_after_valid - guard_after_first,
            "accepted_exactly_once": guard_after_valid - guard_after_first == 1,
        },
        "invalid_legs": invalid,
        "abandoned_guard": {
            "accepted_query_count": abandoned_after - abandoned_before,
            "task_count": 0,
            "resource_lease_count": 0,
        },
    });

    let mut bypass = GuardBypassIntervention::new(&mut client, token.clone(), &server.control);
    let fault_first = bypass
        .accept_incomplete(tonic_submission("request:wp48-guard-fault"))
        .await;
    let fault_replay = bypass.accept_replay().await;
    let mut wrong_token = token.clone();
    wrong_token[0] ^= 0xff;
    let fault_wrong_principal = bypass.accept_wrong_principal(&wrong_token).await;
    bypass.retain_abandoned_authority().await;
    let fault_state = bypass.finish();
    let (fault_selected_choice, fault_selected_ordinal) = fault_challenge.selected_choice();
    let daemon_choice = first_challenge.requirements[0].authorized_choices[0]
        .choice_id
        .as_str();
    let fault_005 = json!({
        "first_leg": {
            "outcome": if fault_first.daemon_query_id.is_empty() { "rejected" } else { "accepted" },
            "accepted_query_count": fault_state.first_leg_acceptances.len(),
            "fastmcp_request_state_sealed": normal_005["first_leg"]["fastmcp_request_state_sealed"],
            "daemon_token_opaque_to_adapter": normal_005["first_leg"]["daemon_token_opaque_to_adapter"],
        },
        "challenge": {
            "semantic_field_count": fault_challenge.semantic_field_count(),
            "semantic_field_ids_stable": fault_challenge.semantic_field_ids_stable(),
            "semantic_field_owner": fault_challenge.semantic_owner(),
            "presentation_keys_safe": fault_challenge.presentation_keys_safe(),
            "input_kind": fault_challenge.input_kind(),
            "authorized_choices_present": fault_challenge.choices_present(),
            "authorized_choice_owner": fault_challenge.choice_owner(),
            "adapter_authored_defaults": fault_challenge.adapter_authored_defaults(),
        },
        "valid_second_leg": {
            "middleware_reentered": normal_005["valid_second_leg"]["middleware_reentered"],
            "daemon_reauthorized": fault_first.authority.is_some(),
            "selected_authorized_choice_ordinal": fault_selected_ordinal,
            "accepted_answer_matches_selected_choice": fault_selected_choice == daemon_choice,
            "outcome": if fault_first.daemon_query_id.is_empty() { "rejected" } else { "accepted" },
            "accepted_query_count": fault_state.valid_second_leg_acceptances.len(),
            "accepted_exactly_once": fault_state.valid_second_leg_acceptances.len() == 1,
        },
        "invalid_legs": {
            "tampered": normal_005["invalid_legs"]["tampered"],
            "expired": normal_005["invalid_legs"]["expired"],
            "replayed": if fault_replay.daemon_query_id == fault_first.daemon_query_id { "accepted" } else { "rejected" },
            "wrong_arguments": normal_005["invalid_legs"]["wrong_arguments"],
            "wrong_principal": if fault_wrong_principal.daemon_query_id == fault_first.daemon_query_id { "accepted" } else { "rejected" },
            "wrong_workspace": normal_005["invalid_legs"]["wrong_workspace"],
            "wrong_generation": normal_005["invalid_legs"]["wrong_generation"],
            "excess_rounds": normal_005["invalid_legs"]["excess_rounds"],
        },
        "abandoned_guard": {
            "accepted_query_count": fault_state.accepted_query_ids.len(),
            "task_count": fault_state.task_entries.len(),
            "resource_lease_count": fault_state.resource_entries.len(),
        },
    });
    let guard_source =
        FieldSource::tonic("guarded-start-authority", "generated-client-guard-ledger");
    let presentation_guard_source = FieldSource::installed(
        "guarded-input-presentation",
        "installed-fastmcp-input-required",
    );
    let challenge_source = FieldSource::component(
        "guarded-input-presentation",
        "guard-challenge-presentation-intervention",
    );
    let guard_state_source = FieldSource::component(
        "guarded-input-presentation",
        "installed-fastmcp-guard-state-roundtrip",
    );
    rows.insert(
        ("RFV5-FM4-005", "normal"),
        ObservationWithSources::with_sources(
            normal_005,
            vec![
                ("", guard_source.clone()),
                (
                    "/first_leg/fastmcp_request_state_sealed",
                    guard_state_source.clone(),
                ),
                (
                    "/first_leg/daemon_token_opaque_to_adapter",
                    presentation_guard_source.clone(),
                ),
                ("/challenge", challenge_source.clone()),
                (
                    "/valid_second_leg/middleware_reentered",
                    presentation_guard_source.clone(),
                ),
            ],
        ),
    );
    rows.insert(
        ("RFV5-FM4-005", "fault"),
        ObservationWithSources::with_sources(
            fault_005,
            vec![
                ("", guard_source),
                (
                    "/first_leg/fastmcp_request_state_sealed",
                    guard_state_source,
                ),
                (
                    "/first_leg/daemon_token_opaque_to_adapter",
                    presentation_guard_source.clone(),
                ),
                ("/challenge", challenge_source),
                (
                    "/valid_second_leg/middleware_reentered",
                    presentation_guard_source.clone(),
                ),
            ],
        ),
    );

    observe_tonic_resource_and_reconnect(installed, rows, &server, &mut client, &authority, &token)
        .await;
    server.stop().await;
}

fn observe_internal_components(
    installed: &InstalledSlices,
    rows: &mut HashMap<(&'static str, &'static str), ObservationWithSources>,
) {
    let stderr = installed.stderr["captured_text"]
        .as_str()
        .unwrap_or_default();
    let secret_classes = [
        ("capability-token", "capability-secret-wp48"),
        ("daemon-token", "daemon-secret-wp48"),
        ("lease-token", "lease-secret-wp48"),
        ("source-bytes", "source-secret-wp48"),
        ("storage-path", "/private/wp48/storage-secret"),
    ];
    let leaked_to_stderr = secret_classes
        .into_iter()
        .filter_map(|(class, secret)| stderr.contains(secret).then_some(class))
        .collect::<Vec<_>>();
    let mut normal = installed.component["redaction"]["normal"]
        .as_object()
        .expect("normal installed redaction observation")
        .clone();
    normal.insert("leaked_to_stderr".to_owned(), json!(leaked_to_stderr));
    let mut fault = installed.component["redaction"]["fault"]
        .as_object()
        .expect("fault installed redaction observation")
        .clone();
    fault.insert("leaked_to_stderr".to_owned(), json!(leaked_to_stderr));

    let sink_source = FieldSource::component(
        "adapter-sink-redaction",
        "installed-fastmcp-safe-error-sinks",
    );
    let stderr_source =
        FieldSource::installed("adapter-sink-redaction", "installed-fastmcp-stderr-capture");
    for (mode, actual) in [
        ("normal", Value::Object(normal)),
        ("fault", Value::Object(fault)),
    ] {
        rows.insert(
            ("RFV5-FM4-012", mode),
            ObservationWithSources::with_sources(
                actual,
                vec![
                    ("", sink_source.clone()),
                    ("/leaked_to_stderr", stderr_source.clone()),
                ],
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_source_rules_cover_every_rfc6901_leaf() {
        let source = FieldSource::component("seam", "probe");
        let (actual, fields) = ObservationWithSources::one_source(
            json!({"a/b": [true, {"~": []}], "empty": {}}),
            source.clone(),
        )
        .finish();
        assert_eq!(
            leaf_paths(&actual),
            BTreeSet::from([
                "/a~1b/0".to_owned(),
                "/a~1b/1/~".replace('~', "~0"),
                "/empty".to_owned(),
            ])
        );
        assert!(fields.values().all(|value| value == &source));
    }

    #[test]
    fn request_case_closure_is_ordered_and_exact() {
        let cases = CLAIMS
            .iter()
            .flat_map(|claim| {
                [
                    RequestedCase {
                        claim_id: (*claim).to_owned(),
                        mode: ObservationMode::Normal,
                        fixture_id: None,
                    },
                    RequestedCase {
                        claim_id: (*claim).to_owned(),
                        mode: ObservationMode::Fault,
                        fixture_id: Some(format!("{claim}-N")),
                    },
                ]
            })
            .collect::<Vec<_>>();
        validate_case_closure(&cases);
    }
}
