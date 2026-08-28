use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use codefabric::fabric::{
    EmptySnapshotOverlay, PublicationOutcome, PublicationPins, PublicationRequest,
    SnapshotProviderCatalog, WorkspaceFabric, bootstrap_workspace,
};
use codefabric::governed_session::GovernedSession;
use codefabric::identity::{
    IdentityDomain, SOURCE_CONTEXT_ID, context_set_identity, encode_public_id,
};
use codefabric::ontology_candidate::{CandidateClosureReport, CandidateClosureRunner};
use codefabric::ontology_gate::GateResourceEnvelope;
use codefabric::ontology_program::{
    InstalledOntologyProgramPackage, OntologyPackagingProfile, OntologyProgramPackage,
    build_ontology_program_package, install_ontology_program_package,
    verify_installed_ontology_program_package,
};
use codefabric::operational_store::{
    ActiveOntologyAuthority, OntologyOwnerDecision, OperationalStore,
};
use codefabric::registries::SnapshotLeaseKind;
use codefabric::snapshot::{
    ResultAuthorityPin, ServingSnapshotManifestBody, SnapshotBasePublication, SnapshotBundles,
    SnapshotContextRecord, SnapshotContexts, SnapshotIndexes, SnapshotOverlay, SnapshotSource,
};
use codefabric::snapshot_runtime::{
    ServingSnapshotCandidate, ServingSnapshotRuntime, SnapshotLeaseManager,
};
use codefabric::source_image::{SourceCapturePolicy, SourceImageStore};
use codefabric::workspace_registry::{WorkspaceRegistry, WorkspaceSourceRegistration};
use datafusion::prelude::SessionConfig;
use serde_json::Value;
use tempfile::TempDir;

#[derive(Clone, Debug)]
struct IntegrationEvidence {
    exact_delta_readback: bool,
    single_cas_winner: bool,
    invalid_predecessor_left_no_request: bool,
    restart_retry_was_idempotent: bool,
    old_new_leases_survived_restart: bool,
    post_cutover_publication_reused_authority: bool,
    forward_rollback_generation: i64,
    package_readback: bool,
}

struct DaemonProcess {
    child: Child,
    discovery: std::path::PathBuf,
}

impl DaemonProcess {
    fn stop(mut self) {
        let output = Command::new(env!("CARGO_BIN_EXE_codefabric"))
            .args(["daemon", "stop", "--discovery"])
            .arg(&self.discovery)
            .output()
            .expect("stop daemon through admin transport");
        assert!(
            output.status.success(),
            "daemon stop failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if self.child.try_wait().expect("poll daemon").is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "daemon did not stop");
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn digest(byte: u8) -> String {
    codefabric::integrity::framed_digest(&[byte; 32])
}

fn write_daemon_config(root: &std::path::Path) -> std::path::PathBuf {
    let state = root.join("state");
    let runtime = root.join("runtime");
    let config_root = root.join("config");
    for path in [&state, &runtime, &config_root] {
        fs::create_dir_all(path).expect("daemon directory");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .expect("private daemon directory");
    }
    let capability = config_root.join("query.capability");
    fs::write(&capability, b"ontology-cutover-capability").expect("capability token");
    fs::set_permissions(&capability, fs::Permissions::from_mode(0o600))
        .expect("private capability token");
    let config_path = config_root.join("codefabric.toml");
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
bundle_index = "contracts/generated/artifact-index.json"
toolchain_identity = "contracts/toolchain/toolchain-identity.json"
sandbox_policy = "required-for-untrusted"
hard_limit_profile = "daemon-default-v1"
supported_platform_profile = "local-workstation-v1"

[reloadable]
log_level = "info"
telemetry_sampling = 0.0
soft_query_quota = 2
maintenance_schedule = "daily-idle"
"#,
        state = state.display().to_string(),
        runtime = runtime.display().to_string(),
        config_root = config_root.display().to_string(),
        socket = runtime.join("admin.sock").display().to_string(),
        query_socket = runtime.join("query.sock").display().to_string(),
    );
    fs::write(&config_path, config).expect("daemon config");
    fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600))
        .expect("private daemon config");
    config_path
}

fn start_daemon(config: &std::path::Path, discovery: &std::path::Path) -> DaemonProcess {
    let child = Command::new(env!("CARGO_BIN_EXE_codefabricd"))
        .args(["serve", "--config"])
        .arg(config)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ontology cutover daemon");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !discovery.is_file() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(discovery.is_file(), "daemon did not publish discovery");
    DaemonProcess {
        child,
        discovery: discovery.to_owned(),
    }
}

fn activation_command(
    discovery: &std::path::Path,
    workspace_id: &str,
    candidate_identity: &str,
    decision_identity: &str,
    request_key: &str,
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codefabric"));
    command
        .args([
            "workspace",
            "activate-candidate",
            workspace_id,
            candidate_identity,
            decision_identity,
            request_key,
            "--discovery",
        ])
        .arg(discovery)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn response(output: Output) -> Value {
    assert!(
        !output.stdout.is_empty(),
        "admin command produced no response: status={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("admin response JSON")
}

fn authority_pin(authority: &ActiveOntologyAuthority) -> ResultAuthorityPin {
    ResultAuthorityPin {
        result_authority_identity: authority.result_authority_identity.clone(),
        program_identity: authority.program_identity.clone(),
        function_catalog_identity: authority.function_catalog_identity.clone(),
        policy_identity: authority.policy_identity.clone(),
        query_form_identity: authority.query_form_identity.clone(),
        checksum_version: authority.checksum_version.clone(),
        exact_table_set_identity: authority.exact_table_set_identity.clone(),
    }
}

async fn publication_request(
    fabric: &WorkspaceFabric,
    workspace_id: [u8; 16],
    publication_byte: u8,
    source_generation: i64,
) -> PublicationRequest {
    let analysis_context_ids = vec![SOURCE_CONTEXT_ID];
    let analysis_context_set_id = context_set_identity(workspace_id, &analysis_context_ids)
        .expect("context set")
        .id;
    PublicationRequest {
        operation_id: [publication_byte.wrapping_add(0x40); 16],
        pins: PublicationPins {
            publication_id: [publication_byte; 16],
            workspace_id,
            repository_id: None,
            worktree_id: None,
            source_generation,
            source_inventory_digest: [publication_byte; 32],
            analysis_context_set_id,
            analysis_context_ids,
            git_state_fingerprint: None,
            inclusion_policy_fingerprint: [0x31; 32],
            base_fact_digest: [publication_byte.wrapping_add(1); 32],
            derived_fact_digest: None,
            ontology_version: "1.3".into(),
            schema_bundle_version: "1.0.0".into(),
            provider_bundle_version: "1.0.0".into(),
            derivation_bundle_version: "1.0.0".into(),
            toolchain_bundle_version: "1.0.0".into(),
        },
        expected_pointer: fabric
            .current_publication()
            .await
            .expect("publication pointer"),
        expected_publication_table_version: fabric.table(5).expect("publication table").version(),
        expected_manifest_table_version: fabric.table(6).expect("manifest table").version(),
        expected_pointer_table_version: fabric.table(7).expect("pointer table").version(),
        started_at_micros: source_generation * 1_000,
        completed_at_micros: source_generation * 1_000 + 500,
    }
}

fn snapshot_body(
    workspace_id: [u8; 16],
    registration_revision: u64,
    publication: &PublicationOutcome,
    authority: Option<ResultAuthorityPin>,
) -> ServingSnapshotManifestBody {
    let source_generation =
        u64::try_from(publication.scope.source_generation).expect("nonnegative source generation");
    ServingSnapshotManifestBody {
        manifest_version: if authority.is_some() { "2.0" } else { "1.0" }.into(),
        workspace_id: encode_public_id(IdentityDomain::Workspace, None, workspace_id)
            .expect("workspace public ID"),
        repository_id: None,
        worktree_id: None,
        registration_revision,
        source: SnapshotSource {
            source_generation,
            admitted_event_sequence: source_generation,
            reconciled_event_sequence: source_generation,
            inventory_digest: digest(1),
            authorization_fingerprint: digest(2),
            inclusion_policy_fingerprint: digest(3),
            path_profile_version: "1".into(),
            source_trust_state: "CURRENT".into(),
            event_stream_health: "HEALTHY".into(),
            git_acceleration_status: "UNAVAILABLE_FALLBACK_ACTIVE".into(),
            git_state_fingerprint: None,
        },
        contexts: SnapshotContexts {
            context_set_id: encode_public_id(
                IdentityDomain::ContextSet,
                None,
                publication.scope.analysis_context_set_id,
            )
            .expect("context-set public ID"),
            default_python_context_id: None,
            default_rust_context_id: None,
            records: vec![SnapshotContextRecord {
                analysis_context_id: encode_public_id(
                    IdentityDomain::AnalysisContext,
                    None,
                    SOURCE_CONTEXT_ID,
                )
                .expect("analysis context public ID"),
                context_manifest_digest: digest(4),
                capability_partition_digest: digest(5),
            }],
        },
        base_publication: SnapshotBasePublication {
            publication_id: String::new(),
            tables: Vec::new(),
        },
        overlay: SnapshotOverlay {
            overlay_generation: 0,
            overlay_digest: digest(0),
            total_memory_bytes: 0,
            tables: Vec::new(),
        },
        indexes: SnapshotIndexes {
            capability_index_digest: digest(6),
            diagnostic_index_digest: digest(7),
            dependency_graph_digest: digest(8),
        },
        bundles: SnapshotBundles {
            ontology_bundle_id: "ontology:1.3".into(),
            schema_bundle_id: "schema:1.0.0".into(),
            provider_bundle_id: "provider:1.0.0".into(),
            derivation_bundle_id: "derivation:1.0.0".into(),
            query_language_bundle_id: "query:1.0.0".into(),
            model_pack_bundle_id: "model:1.0.0".into(),
            toolchain_bundle_id: "toolchain:1.0.0".into(),
            sandbox_profile_digests: std::collections::BTreeMap::new(),
        },
        result_authority: authority,
        limits_profile_digest: digest(9),
        source_blob_digests: Vec::new(),
    }
}

async fn serving_candidate(
    workspace_id: [u8; 16],
    registration_revision: u64,
    publication: &PublicationOutcome,
    authority: Option<ResultAuthorityPin>,
) -> Arc<ServingSnapshotCandidate> {
    let catalog = Arc::new(
        SnapshotProviderCatalog::build(publication, &EmptySnapshotOverlay)
            .await
            .expect("freeze exact real Delta catalog"),
    );
    Arc::new(
        ServingSnapshotCandidate::build(
            snapshot_body(workspace_id, registration_revision, publication, authority),
            catalog,
            &[],
        )
        .expect("build serving candidate"),
    )
}

async fn proved_candidate(
    package: OntologyProgramPackage,
    publication: PublicationOutcome,
    predecessor: Option<String>,
) -> (CandidateClosureReport, bool) {
    let runner = CandidateClosureRunner::new_for_epoch(
        package,
        publication,
        GovernedSession::new(SessionConfig::new(), "policy.ontology.v1")
            .expect("governed candidate session"),
        predecessor,
        1_000_000,
    )
    .expect("candidate closure runner");
    let catalog = runner
        .open_frozen_catalog()
        .await
        .expect("read exact real Delta candidate catalog");
    let exact_delta_readback = catalog.provider_records().len() > 10
        && catalog
            .provider_records()
            .all(|record| record.manifest.table_uri.starts_with("file://"));
    let report = runner
        .execute(&GateResourceEnvelope::default())
        .await
        .expect("prove sealed candidate");
    (report, exact_delta_readback)
}

fn decision(report: &CandidateClosureReport, accepted_at: i64) -> OntologyOwnerDecision {
    OntologyOwnerDecision::new(
        report.candidate_identity(),
        "owner:ontology-cutover",
        report.policy_identity(),
        accepted_at,
    )
    .expect("owner decision")
}

async fn run_scenario() -> IntegrationEvidence {
    let root = TempDir::new().expect("ontology cutover root");
    let config = write_daemon_config(root.path());
    let state_root = root.path().join("state");
    let database_path = state_root.join("operational.sqlite3");
    let discovery = root.path().join("runtime/daemon.json");
    let workspace_root = root.path().join("workspace");
    fs::create_dir(&workspace_root).expect("workspace root");

    let mut store = OperationalStore::open(&database_path).expect("operational store");
    let workspace = {
        let mut registry = WorkspaceRegistry::new(&mut store);
        let registered = registry
            .add(&workspace_root, WorkspaceSourceRegistration::Directory)
            .expect("register workspace");
        registry
            .enable(registered.workspace_id)
            .expect("enable workspace for first snapshot")
    };
    let workspace_text = encode_public_id(IdentityDomain::Workspace, None, workspace.workspace_id)
        .expect("workspace public ID");
    let mut fabric = bootstrap_workspace(&state_root, &workspace)
        .await
        .expect("bootstrap real Delta fabric");
    let initial_request = publication_request(&fabric, workspace.workspace_id, 0x51, 1).await;
    let initial_publication = fabric
        .publish(&mut store, &initial_request, &[])
        .await
        .expect("publish initial exact Delta state");

    let package = build_ontology_program_package(&OntologyPackagingProfile::default())
        .expect("ontology program package");
    let installation: InstalledOntologyProgramPackage =
        install_ontology_program_package(&state_root, &package)
            .expect("install durable ontology package");
    verify_installed_ontology_program_package(&installation, &package)
        .expect("package artifact readback");

    let serving_runtime = ServingSnapshotRuntime::default();
    let legacy = serving_candidate(
        workspace.workspace_id,
        workspace.registration_revision,
        &initial_publication,
        None,
    )
    .await;
    serving_runtime
        .activate(
            &mut store,
            Arc::clone(&legacy),
            None,
            0,
            u64::try_from(initial_publication.pointer.pointer_generation)
                .expect("publication generation"),
            10,
            None,
        )
        .expect("activate legacy serving snapshot");
    let mut source_images = SourceImageStore::open(
        &state_root.join("source-images"),
        SourceCapturePolicy::default(),
    )
    .expect("source-image store");
    let lease_manager = SnapshotLeaseManager::new([0x61; 16]);
    let old_lease = lease_manager
        .acquire(
            &mut store,
            &mut source_images,
            Arc::clone(&legacy),
            SnapshotLeaseKind::ResourceRead,
            None,
            11,
            Duration::from_secs(10_000),
            None,
        )
        .expect("legacy lease");
    assert!(old_lease.result_authority().is_none());

    let (target_report, exact_delta_readback) =
        proved_candidate(package.clone(), initial_publication.clone(), None).await;
    let target_decision = decision(&target_report, 20);
    store
        .persist_proved_ontology_candidate(&target_report, 19)
        .expect("persist target candidate");
    store
        .record_ontology_owner_decision(&target_decision)
        .expect("persist target decision");
    drop(fabric);
    drop(store);

    let daemon = start_daemon(&config, &discovery);
    let invalid = response(
        activation_command(
            &discovery,
            &workspace_text,
            target_report.candidate_identity(),
            &format!("b3:{}", "f".repeat(64)),
            "cutover-invalid-decision",
        )
        .output()
        .expect("invalid predecessor command"),
    );
    assert_eq!(invalid["accepted"], false);

    let first = activation_command(
        &discovery,
        &workspace_text,
        target_report.candidate_identity(),
        target_decision.identity(),
        "cutover-race-a",
    )
    .spawn()
    .expect("spawn first CAS contender");
    let second = activation_command(
        &discovery,
        &workspace_text,
        target_report.candidate_identity(),
        target_decision.identity(),
        "cutover-race-b",
    )
    .spawn()
    .expect("spawn second CAS contender");
    let first_response = response(first.wait_with_output().expect("first CAS output"));
    let second_response = response(second.wait_with_output().expect("second CAS output"));
    let first_won = first_response["accepted"] == true;
    let second_won = second_response["accepted"] == true;
    assert_ne!(first_won, second_won, "exactly one CAS contender must win");
    let winning_request = if first_won {
        "cutover-race-a"
    } else {
        "cutover-race-b"
    };
    daemon.stop();

    let store = OperationalStore::open(&database_path).expect("inspect target activation");
    let (request_count, acceptance_count) = store
        .reader_factory()
        .open()
        .expect("operational reader")
        .with_connection(|connection| {
            Ok::<_, rusqlite::Error>((
                connection.query_row(
                    "SELECT COUNT(*) FROM ontology_activation_request",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                connection.query_row("SELECT COUNT(*) FROM ontology_acceptance", [], |row| {
                    row.get::<_, i64>(0)
                })?,
            ))
        })
        .expect("activation counts");
    assert_eq!((request_count, acceptance_count), (1, 1));
    drop(store);

    let restarted = start_daemon(&config, &discovery);
    let replay = response(
        activation_command(
            &discovery,
            &workspace_text,
            target_report.candidate_identity(),
            target_decision.identity(),
            winning_request,
        )
        .output()
        .expect("restart activation replay"),
    );
    assert_eq!(replay["accepted"], true);
    restarted.stop();

    let mut store = OperationalStore::open(&database_path).expect("reopen after target cutover");
    store
        .validate_ontology_activation_recovery()
        .expect("recover target authority");
    let target_authority = store
        .active_ontology_authority(workspace.workspace_id)
        .expect("target authority read")
        .expect("target authority active");
    let target = serving_candidate(
        workspace.workspace_id,
        workspace.registration_revision,
        &initial_publication,
        Some(authority_pin(&target_authority)),
    )
    .await;
    serving_runtime
        .activate(
            &mut store,
            Arc::clone(&target),
            Some(
                legacy
                    .manifest()
                    .raw_snapshot_id()
                    .expect("legacy snapshot ID"),
            ),
            1,
            u64::try_from(initial_publication.pointer.pointer_generation)
                .expect("publication generation"),
            30,
            None,
        )
        .expect("activate target serving snapshot");
    let target_lease = lease_manager
        .acquire(
            &mut store,
            &mut source_images,
            Arc::clone(&target),
            SnapshotLeaseKind::ResourceRead,
            None,
            31,
            Duration::from_secs(10_000),
            None,
        )
        .expect("target authority lease");
    assert_eq!(
        target_lease
            .result_authority()
            .expect("target result authority")
            .result_authority_identity,
        target_authority.result_authority_identity
    );

    let mut fabric = bootstrap_workspace(&state_root, &workspace)
        .await
        .expect("reopen real Delta fabric");
    let post_request = publication_request(&fabric, workspace.workspace_id, 0x52, 2).await;
    let post_publication = fabric
        .publish(&mut store, &post_request, &[])
        .await
        .expect("ordinary post-cutover publication");
    let unchanged_authority = store
        .active_ontology_authority(workspace.workspace_id)
        .expect("post-publication authority")
        .expect("post-publication active authority");
    assert_eq!(unchanged_authority, target_authority);
    let continuity = serving_candidate(
        workspace.workspace_id,
        workspace.registration_revision,
        &post_publication,
        Some(authority_pin(&target_authority)),
    )
    .await;
    serving_runtime
        .activate(
            &mut store,
            Arc::clone(&continuity),
            Some(
                target
                    .manifest()
                    .raw_snapshot_id()
                    .expect("target snapshot ID"),
            ),
            2,
            u64::try_from(post_publication.pointer.pointer_generation)
                .expect("post publication generation"),
            40,
            None,
        )
        .expect("activate post-publication snapshot with unchanged authority");

    let (rollback_report, _) = proved_candidate(
        package.clone(),
        post_publication.clone(),
        Some(target_authority.epoch_identity.clone()),
    )
    .await;
    let rollback_decision = decision(&rollback_report, 50);
    store
        .persist_proved_ontology_candidate(&rollback_report, 49)
        .expect("persist forward rollback candidate");
    store
        .record_ontology_owner_decision(&rollback_decision)
        .expect("persist forward rollback decision");
    drop(fabric);
    drop(store);

    let rollback_daemon = start_daemon(&config, &discovery);
    let rollback_response = response(
        activation_command(
            &discovery,
            &workspace_text,
            rollback_report.candidate_identity(),
            rollback_decision.identity(),
            "cutover-forward-rollback",
        )
        .output()
        .expect("forward rollback command"),
    );
    assert_eq!(rollback_response["accepted"], true);
    rollback_daemon.stop();

    let mut store = OperationalStore::open(&database_path).expect("final restart recovery");
    store
        .validate_ontology_activation_recovery()
        .expect("recover forward rollback");
    let rollback_authority = store
        .active_ontology_authority(workspace.workspace_id)
        .expect("rollback authority")
        .expect("rollback authority active");
    assert_eq!(
        continuity
            .manifest()
            .body
            .result_authority
            .as_ref()
            .expect("continuity result authority")
            .result_authority_identity,
        rollback_authority.result_authority_identity,
        "a forward ontology epoch with unchanged semantics reuses the exact serving manifest"
    );
    let rollback_lease = lease_manager
        .acquire(
            &mut store,
            &mut source_images,
            Arc::clone(&continuity),
            SnapshotLeaseKind::ResourceRead,
            None,
            61,
            Duration::from_secs(10_000),
            None,
        )
        .expect("rollback lease");
    assert_eq!(
        rollback_lease
            .result_authority()
            .expect("rollback authority pin")
            .result_authority_identity,
        target_authority.result_authority_identity
    );
    assert_ne!(
        rollback_lease.record().ontology_epoch_identity,
        target_lease.record().ontology_epoch_identity
    );
    let active_generation = store
        .reader_factory()
        .open()
        .expect("final reader")
        .with_connection(|connection| {
            connection.query_row(
                "SELECT pointer_generation FROM ontology_active_pointer WHERE workspace_id=?1",
                [workspace.workspace_id.as_slice()],
                |row| row.get::<_, i64>(0),
            )
        })
        .expect("rollback pointer generation");
    drop(store);

    let reopened = OperationalStore::open(&database_path).expect("lease restart readback");
    let leases = SnapshotLeaseManager::list(&reopened, workspace.workspace_id)
        .expect("old/new lease restart reconstruction");
    let legacy_count = leases
        .iter()
        .filter(|lease| lease.result_authority.is_none())
        .count();
    let versioned_authorities = leases
        .iter()
        .filter_map(|lease| lease.result_authority.as_ref())
        .map(|authority| authority.result_authority_identity.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let ontology_epochs = leases
        .iter()
        .filter_map(|lease| lease.ontology_epoch_identity.as_deref())
        .collect::<std::collections::BTreeSet<_>>();
    let old_new_leases_survived_restart = legacy_count == 1
        && versioned_authorities.contains(target_authority.result_authority_identity.as_str())
        && ontology_epochs.contains(target_authority.epoch_identity.as_str())
        && ontology_epochs.contains(rollback_authority.epoch_identity.as_str());
    verify_installed_ontology_program_package(&installation, &package)
        .expect("retained package survives cutover and rollback");

    IntegrationEvidence {
        exact_delta_readback,
        single_cas_winner: first_won ^ second_won,
        invalid_predecessor_left_no_request: request_count == 1 && acceptance_count == 1,
        restart_retry_was_idempotent: replay["accepted"] == true,
        old_new_leases_survived_restart,
        post_cutover_publication_reused_authority: unchanged_authority == target_authority,
        forward_rollback_generation: active_generation,
        package_readback: installation.manifest_path().is_file(),
    }
}

fn evidence() -> &'static IntegrationEvidence {
    static EVIDENCE: OnceLock<IntegrationEvidence> = OnceLock::new();
    EVIDENCE.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("ontology integration runtime")
            .block_on(run_scenario())
    })
}

#[test]
fn ontology_datafabric_end_to_end_cutover() {
    let evidence = evidence();
    assert!(evidence.exact_delta_readback);
    assert!(evidence.package_readback);
    assert!(evidence.single_cas_winner);
}

#[test]
fn ontology_datafabric_predecessor_failure_atomicity() {
    assert!(evidence().invalid_predecessor_left_no_request);
}

#[test]
fn ontology_datafabric_old_new_lease_restart() {
    let evidence = evidence();
    assert!(evidence.restart_retry_was_idempotent);
    assert!(evidence.old_new_leases_survived_restart);
    assert_eq!(evidence.forward_rollback_generation, 2);
}

#[test]
fn ontology_datafabric_post_cutover_fact_publication() {
    assert!(evidence().post_cutover_publication_reused_authority);
}
