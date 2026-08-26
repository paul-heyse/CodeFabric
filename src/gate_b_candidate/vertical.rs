//! Actual-output Gate B execution used only to assemble an unreleased review candidate.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arrow::array::{Array as _, Int16Array};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

use super::*;
use crate::core_facts::CoreFactEngine;
use crate::daemon::{
    AdminCommand, DaemonConfig, ReloadableConfig, StaticConfig, administer,
    serve_with_query_backend, wait_for_discovery,
};
use crate::fabric::{
    PublicationPins, PublicationRequest, ServingQuerySession, ServingRuntimeConfig,
    SnapshotOverlayProviderFactory as _, bootstrap_workspace,
};
use crate::lifecycle::CanonicalState;
use crate::pyrefly_service::{PyreflyModuleInput, PyreflyRunRequest, analyze_pyrefly_uds};
use crate::query_service::WorkspaceQueryBackend;
use crate::registries::{Completeness, CpgdFeatureMask, OwnerCapabilityState, SnapshotLeaseKind};
use crate::rpc::generated::codefabric::cpgd::v1::cpg_query_service_client::CpgQueryServiceClient;
use crate::rpc::generated::codefabric::cpgd::v1::query_event::Event;
use crate::rpc::generated::codefabric::cpgd::v1::{
    CredentialProof, DeliveryPreference, HandshakeRequest, HostCapabilityProfile,
    PayloadCompression, QueryEventHeader, ReadResultRequest, StartQueryRequest, StreamQueryRequest,
    VersionRange, WorkspaceClaim, WorkspaceReadiness,
};
use crate::rustc_service::{
    AcceptedRustcCompilation, RustcObservationService, RustcProtocolPolicy, RustcRunAdmission,
    serve_rustc_uds,
};
use crate::snapshot::{
    ServingSnapshotManifestBody, SnapshotBasePublication, SnapshotBundles, SnapshotContextRecord,
    SnapshotContexts, SnapshotIndexes, SnapshotOverlay, SnapshotSource,
};
use crate::snapshot_runtime::{ServingSnapshotRuntime, SnapshotLeaseManager};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct VerticalExecution {
    pub execution_id: String,
    pub workspace_id: String,
    pub analysis_context_id: String,
    pub source_generation: u64,
    pub publication_id: String,
    pub snapshot_id: String,
    pub provider_run_ids: BTreeMap<String, String>,
    pub planes: BTreeMap<String, Value>,
    pub execution_digest: String,
}

struct ChildGuard(Child);
static SHORT_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct DirectoryGuard(PathBuf);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn now_millis() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(i64::MAX)
}

fn digest(bytes: &[u8]) -> String {
    crate::integrity::framed_digest(bytes)
}

async fn wait_for_socket(path: &Path) -> Result<(), GateBCandidateError> {
    for _ in 0..500 {
        if path.exists() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Err(invariant(format!(
        "provider socket did not appear at {}",
        path.display()
    )))
}

fn provider_binary(repository_root: &Path, relative: &str) -> Result<PathBuf, GateBCandidateError> {
    let path = repository_root.join(relative);
    if !path.is_file() {
        return Err(invariant(format!(
            "required Gate B provider binary is absent at {}; run the provider gate first",
            path.display()
        )));
    }
    Ok(path)
}

pub(crate) async fn run_pyrefly(
    repository_root: &Path,
    state_root: &Path,
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    source_generation: u64,
    source_path: &Path,
    invalid_source_path: &Path,
) -> Result<crate::pyrefly_service::AcceptedPyreflyRun, GateBCandidateError> {
    let binary = provider_binary(repository_root, "target/debug/codefabric-pyrefly-sidecar")?;
    let socket = state_root.join("pyrefly.sock");
    let child = Command::new(binary)
        .arg("--serve")
        .env(
            "CODEFABRIC_PYREFLY_ENDPOINT",
            format!("unix://{}", socket.display()),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let _guard = ChildGuard(child);
    wait_for_socket(&socket).await?;
    let workspace_id_text =
        encode_public_id(IdentityDomain::Workspace, None, workspace_id).map_err(invariant)?;
    let context_id_text =
        encode_public_id(IdentityDomain::AnalysisContext, None, analysis_context_id)
            .map_err(invariant)?;
    let context_manifest = b"{\"language\":\"python\",\"profile\":\"PYTHON_SEMANTIC_V1\"}".to_vec();
    let valid_source = fs::read(source_path)?;
    let invalid_source = fs::read(invalid_source_path)?;
    let source_manifest_digest = digest(
        [
            (valid_source.len() as u64).to_be_bytes().as_slice(),
            valid_source.as_slice(),
            (invalid_source.len() as u64).to_be_bytes().as_slice(),
            invalid_source.as_slice(),
        ]
        .concat()
        .as_slice(),
    );
    let request = PyreflyRunRequest {
        provider_run_id: "run:gate-b-pyrefly".to_owned(),
        workspace_id: workspace_id_text,
        analysis_context_id: context_id_text,
        canonical_workspace_id: workspace_id,
        canonical_analysis_context_id: analysis_context_id,
        source_generation,
        context_manifest,
        source_snapshot_lease_id: "lease:gate-b-pyrefly".to_owned(),
        source_manifest_digest,
        modules: vec![
            PyreflyModuleInput {
                module_id: "module:gate-b-python".to_owned(),
                module_name: "golden_pkg.core".to_owned(),
                file_id: "file:gate-b-python".to_owned(),
                source_path: source_path.to_path_buf(),
            },
            PyreflyModuleInput {
                module_id: "module:gate-b-python-invalid".to_owned(),
                module_name: "malformed.broken".to_owned(),
                file_id: "file:gate-b-python-invalid".to_owned(),
                source_path: invalid_source_path.to_path_buf(),
            },
        ],
        requested_capability_codes: vec![90, 110],
        deadline_unix_ms: now_millis() + 120_000,
        output_schema_bundle_digest: digest(include_bytes!(
            "../../contracts/schema/schema-contract-ir.json"
        )),
    };
    analyze_pyrefly_uds(&socket, &request)
        .await
        .map_err(invariant)
}

fn extractor_environment(
    command: &mut Command,
    socket: &Path,
    workspace_id: &str,
    context_id: &str,
    source_generation: u64,
) {
    let fixed_digest = digest(b"gate-b-rustc-fixture");
    command
        .env(
            "CODEFABRIC_EXTRACTOR_ENDPOINT",
            format!("unix://{}", socket.display()),
        )
        .env("CODEFABRIC_PROVIDER_RUN_ID", "run:gate-b-rustc")
        .env("CODEFABRIC_WORKSPACE_ID", workspace_id)
        .env("CODEFABRIC_ANALYSIS_CONTEXT_ID", context_id)
        .env(
            "CODEFABRIC_SOURCE_GENERATION",
            source_generation.to_string(),
        )
        .env("CODEFABRIC_CONTEXT_MANIFEST_DIGEST", &fixed_digest)
        .env(
            "CODEFABRIC_PROVIDER_RESOURCE_PROFILE_ID",
            "compiler-semantic-standard",
        )
        .env("CODEFABRIC_SOURCE_SNAPSHOT_MANIFEST_DIGEST", &fixed_digest)
        .env("CODEFABRIC_CARGO_METADATA_DIGEST", &fixed_digest)
        .env("CODEFABRIC_CARGO_LOCK_DIGEST", &fixed_digest)
        .env("CODEFABRIC_CARGO_CONFIG_DIGEST", &fixed_digest);
}

#[allow(clippy::too_many_lines)] // One compiler transaction keeps invocation, stream, and terminal validation ordered.
pub(crate) async fn run_rustc(
    repository_root: &Path,
    state_root: &Path,
    workspace_root: &Path,
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    source_generation: u64,
) -> Result<AcceptedRustcCompilation, GateBCandidateError> {
    provider_binary(
        repository_root,
        "target/extractor/debug/codefabric-rustc-extractor",
    )?;
    let identity_bytes = fs::read(repository_root.join("rustc-extractor/toolchain-identity.json"))?;
    let identity: Value = serde_json::from_slice(&identity_bytes)?;
    let workspace_id_text =
        encode_public_id(IdentityDomain::Workspace, None, workspace_id).map_err(invariant)?;
    let context_id_text =
        encode_public_id(IdentityDomain::AnalysisContext, None, analysis_context_id)
            .map_err(invariant)?;
    let fixed_digest = digest(b"gate-b-rustc-fixture");
    let policy = RustcProtocolPolicy {
        daemon_build: "codefabricd-gate-b".to_owned(),
        output_schema_bundle_digest: digest(include_bytes!(
            "../../contracts/schema/schema-contract-ir.json"
        )),
        sandbox_profile_digest: fixed_digest.clone(),
        extractor_build: identity["extractor"]
            .as_str()
            .ok_or_else(|| invariant("extractor identity is absent"))?
            .to_owned(),
        rustc_version: identity["rustc_release"]
            .as_str()
            .ok_or_else(|| invariant("rustc release is absent"))?
            .to_owned(),
        rustc_commit: identity["rustc_commit_hash"]
            .as_str()
            .ok_or_else(|| invariant("rustc commit is absent"))?
            .to_owned(),
        toolchain_identity_digest: digest(&identity_bytes),
        supported_feature_bits: 0,
        provider_deadline_unix_ms: now_millis() + 120_000,
    };
    let admission = RustcRunAdmission {
        provider_run_id: "run:gate-b-rustc".to_owned(),
        workspace_id: workspace_id_text.clone(),
        analysis_context_id: context_id_text.clone(),
        canonical_workspace_id: workspace_id,
        canonical_analysis_context_id: analysis_context_id,
        source_generation,
        context_manifest_digest: fixed_digest.clone(),
        source_snapshot_manifest_digest: fixed_digest,
        resource_profile_id: "compiler-semantic-standard".to_owned(),
    };
    let (service, mut accepted) =
        RustcObservationService::new(policy, admission).map_err(invariant)?;
    let socket = state_root.join("rustc.sock");
    let allowed_uid = fs::metadata(state_root)?.uid();
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let server_socket = socket.clone();
    let server = tokio::spawn(async move {
        serve_rustc_uds(&server_socket, allowed_uid, service, async {
            let _ = shutdown_receiver.await;
        })
        .await
    });
    wait_for_socket(&socket).await?;
    let wrapper = repository_root.join("scripts/run_rustc_extractor.sh");
    let lowercase_source = workspace_root.join("rust/src/lib.rs");
    let source = if lowercase_source.is_file() {
        lowercase_source
    } else {
        workspace_root.join("rust/src/Lib.rs")
    };
    if !source.is_file() {
        return Err(invariant(
            "Gate B rustc fixture has no current Rust library source",
        ));
    }
    let output_directory = state_root.join("rust-output");
    fs::create_dir(&output_directory)?;
    let status = tokio::task::spawn_blocking(move || {
        let rustc = Command::new("rustup")
            .args(["which", "--toolchain", "nightly-2026-08-18", "rustc"])
            .output()?;
        if !rustc.status.success() {
            return Err(std::io::Error::other(format!(
                "rustup could not resolve the pinned rustc: {}",
                String::from_utf8_lossy(&rustc.stderr)
            )));
        }
        let rustc = PathBuf::from(String::from_utf8_lossy(&rustc.stdout).trim());
        let sysroot_library = rustc
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| std::io::Error::other("pinned rustc has no sysroot parent"))?
            .join("lib");
        let mut command = Command::new(wrapper);
        command
            .arg(rustc)
            .arg(source)
            .args([
                "--crate-name=codefabric_gate_b_rust",
                "--crate-type=lib",
                "--edition=2024",
                "--emit=metadata",
            ])
            .arg(format!("--out-dir={}", output_directory.display()))
            .env("DYLD_LIBRARY_PATH", &sysroot_library)
            .env("LD_LIBRARY_PATH", &sysroot_library)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        extractor_environment(
            &mut command,
            &socket,
            &workspace_id_text,
            &context_id_text,
            source_generation,
        );
        command.output()
    })
    .await
    .map_err(invariant)??;
    if !status.status.success() {
        let _ = shutdown_sender.send(());
        let _ = server.await;
        return Err(invariant(format!(
            "real rustc extractor compilation failed: {}",
            String::from_utf8_lossy(&status.stderr)
        )));
    }
    let compilation = tokio::time::timeout(Duration::from_secs(30), accepted.recv())
        .await
        .map_err(|_| invariant("rustc accepted stream timed out"))?
        .ok_or_else(|| invariant("rustc service closed without an accepted compilation"))?;
    let _ = shutdown_sender.send(());
    server.await.map_err(invariant)?.map_err(invariant)?;
    Ok(compilation)
}

fn snapshot_body(
    workspace_id: [u8; 16],
    source_generation: u64,
    inventory_digest: [u8; 32],
) -> Result<ServingSnapshotManifestBody, GateBCandidateError> {
    let context_set = crate::identity::context_set_identity(workspace_id, &[SOURCE_CONTEXT_ID])
        .map_err(invariant)?;
    Ok(ServingSnapshotManifestBody {
        manifest_version: "1.0".to_owned(),
        workspace_id: encode_public_id(IdentityDomain::Workspace, None, workspace_id)
            .map_err(invariant)?,
        repository_id: None,
        worktree_id: None,
        registration_revision: 1,
        source: SnapshotSource {
            source_generation,
            admitted_event_sequence: source_generation,
            reconciled_event_sequence: source_generation,
            inventory_digest: digest(&inventory_digest),
            authorization_fingerprint: digest(b"gate-b-authorization"),
            inclusion_policy_fingerprint: digest(b"gate-b-inclusion"),
            path_profile_version: "1".to_owned(),
            source_trust_state: "CURRENT".to_owned(),
            event_stream_health: "HEALTHY".to_owned(),
            git_acceleration_status: "UNAVAILABLE_FALLBACK_ACTIVE".to_owned(),
            git_state_fingerprint: None,
        },
        contexts: SnapshotContexts {
            context_set_id: encode_public_id(IdentityDomain::ContextSet, None, context_set.id)
                .map_err(invariant)?,
            default_python_context_id: None,
            default_rust_context_id: None,
            records: vec![SnapshotContextRecord {
                analysis_context_id: encode_public_id(
                    IdentityDomain::AnalysisContext,
                    None,
                    SOURCE_CONTEXT_ID,
                )
                .map_err(invariant)?,
                context_manifest_digest: digest(b"gate-b-context"),
                capability_partition_digest: digest(b"gate-b-capabilities"),
            }],
        },
        base_publication: SnapshotBasePublication {
            publication_id: String::new(),
            tables: Vec::new(),
        },
        overlay: SnapshotOverlay {
            overlay_generation: 0,
            overlay_digest: digest(&[0; 32]),
            total_memory_bytes: 0,
            tables: Vec::new(),
        },
        indexes: SnapshotIndexes {
            capability_index_digest: digest(b"gate-b-capability-index"),
            diagnostic_index_digest: digest(b"gate-b-diagnostic-index"),
            dependency_graph_digest: digest(b"gate-b-dependency-graph"),
        },
        bundles: SnapshotBundles {
            ontology_bundle_id: "ontology:1.3".to_owned(),
            schema_bundle_id: "schema:1.3".to_owned(),
            provider_bundle_id: "provider:1.3".to_owned(),
            derivation_bundle_id: "derivation:1.3".to_owned(),
            query_language_bundle_id: "query:1.3".to_owned(),
            model_pack_bundle_id: "model-pack:1.3".to_owned(),
            toolchain_bundle_id: "toolchain:1.3".to_owned(),
        },
        limits_profile_digest: digest(b"gate-b-limits"),
        source_blob_digests: Vec::new(),
    })
}

fn plane_digest(value: &Value) -> Result<String, GateBCandidateError> {
    Ok(digest(&canonical_value(value)?))
}

fn actual_derived_digest(
    canonicals: &[crate::fact_ingest::CanonicalIngestOutput],
) -> Result<([u8; 32], u64), GateBCandidateError> {
    let mut inputs = Vec::new();
    let mut row_count = 0_u64;
    for canonical in canonicals {
        let Some(relations) = canonical.batches.get(&110) else {
            continue;
        };
        let column_index = relations
            .batch()
            .schema()
            .index_of("derivation_code")
            .map_err(invariant)?;
        let derivations = relations
            .batch()
            .column(column_index)
            .as_any()
            .downcast_ref::<Int16Array>()
            .ok_or_else(|| invariant("relation derivation_code is not Int16"))?;
        let derived_rows = (0..derivations.len())
            .filter(|&index| !derivations.is_null(index))
            .count();
        if derived_rows > 0 {
            row_count = row_count
                .checked_add(u64::try_from(derived_rows).map_err(invariant)?)
                .ok_or_else(|| invariant("derived row count overflow"))?;
            inputs.extend_from_slice(&batch_checksum(relations.batch()).map_err(invariant)?);
        }
    }
    if row_count == 0 {
        return Err(invariant(
            "production reconciliation produced no registered derived relation rows",
        ));
    }
    inputs.extend_from_slice(&row_count.to_be_bytes());
    Ok((digest_bytes(&inputs), row_count))
}

fn eight_form_request(workspace_id: &str, request_id: &str) -> String {
    format!(
        r#"{{"specification":"composable semantic CPG fact query","version":"1.3","semantic_request_id":"{request_id}","workspace_id":"{workspace_id}","freshness_policy":"best_available_snapshot","queries":[{{"query_id":"entities","request":"find code entities","label":null,"looking_for":"syntax nodes","return":{{"limit":{{"maximum_results":10}}}}}},{{"query_id":"properties","request":"retrieve facts about code","label":null,"about":[{{"results_of":"entities","select":"entities"}}],"facts":["callable contracts"],"return":{{"limit":{{"maximum_results":10}}}}}},{{"query_id":"relations","request":"follow code relationships","label":null,"starting_from":[{{"results_of":"entities","select":"entities"}}],"relationship":"call targets","direction":"outgoing","distance":"transitive","return":{{"limit":{{"maximum_results":10}}}}}},{{"query_id":"paths","request":"find connecting fact paths","label":null,"starting_from":[{{"results_of":"entities","select":"entities"}}],"ending_at":["syntax nodes"],"through":["control flow"],"path_policy":"one shortest witness path","direction":"outgoing","maximum_length":4,"return":{{"limit":{{"maximum_results":10}}}}}},{{"query_id":"patterns","request":"match a code fact pattern","label":null,"bindings":[{{"name":"source","looking_for":"syntax nodes","within":{{"results_of":"entities","select":"entities"}}}}],"relationships":[],"return":{{"limit":{{"maximum_results":10}}}}}},{{"query_id":"combined","request":"combine result sets","label":null,"inputs":[{{"results_of":"properties","select":"facts"}},{{"results_of":"relations","select":"facts"}}],"combination":"union by fact identity","identity":"fact identity","preserve_origin":"all origins","return":{{"limit":{{"maximum_results":10}}}}}},{{"query_id":"summary","request":"summarize objective facts","label":null,"input":[{{"results_of":"combined","select":"groups"}}],"summaries":["graph metrics"],"return":{{"limit":{{"maximum_results":10}}}}}},{{"query_id":"context","request":"retrieve source and syntax context","label":null,"for":[{{"results_of":"paths","select":"paths"}}],"context":["source location","exact span"],"text_handling":"omit text","return":{{"limit":{{"maximum_results":10}}}}}}],"response_projection":{{"canonical_semantic_identity":true,"coverage":true}},"cost_budget":{{"maximum_rows":2048}}}}"#
    )
    .replace("\"maximum_results\":10", "\"maximum_results\":256")
}

fn daemon_config(root: &Path, repository_root: &Path) -> Result<DaemonConfig, GateBCandidateError> {
    for path in [
        root.join("state"),
        root.join("runtime"),
        root.join("config"),
    ] {
        fs::create_dir_all(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    }
    let token = root.join("config/query.capability");
    fs::write(&token, b"gate-b-query-capability-token")?;
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600))?;
    Ok(DaemonConfig {
        static_config: StaticConfig {
            state_root: root.join("state"),
            runtime_root: root.join("runtime"),
            config_root: root.join("config"),
            socket_endpoint: root.join("runtime/admin.sock"),
            query_socket_endpoint: root.join("runtime/query.sock"),
            query_capability_token_file: PathBuf::from("query.capability"),
            operational_database: PathBuf::from("operational.sqlite3"),
            bundle_index: repository_root.join(
                "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json",
            ),
            toolchain_identity: repository_root.join("contracts/toolchain/toolchain-identity.json"),
            sandbox_policy: "required-for-untrusted".to_owned(),
            hard_limit_profile: "daemon-default-v1".to_owned(),
            supported_platform_profile: "local-workstation-v1".to_owned(),
        },
        reloadable: ReloadableConfig {
            log_level: "info".to_owned(),
            telemetry_sampling: 0.0,
            soft_query_quota: 4,
            maintenance_schedule: "daily-idle".to_owned(),
        },
    })
}

async fn query_client(
    socket: PathBuf,
) -> Result<CpgQueryServiceClient<Channel>, GateBCandidateError> {
    let channel = Endpoint::try_from("http://[::]:50051")
        .map_err(invariant)?
        .connect_with_connector(service_fn(move |_| {
            let socket = socket.clone();
            async move { UnixStream::connect(socket).await.map(TokioIo::new) }
        }))
        .await
        .map_err(invariant)?;
    Ok(CpgQueryServiceClient::new(channel))
}

#[derive(Debug)]
struct QueryPlanes {
    canonical_tables: Value,
    queries: Value,
    rpc: Value,
    mcp: Value,
    diagnostics: Value,
}

fn canonical_state_plane(state: &CanonicalState) -> Value {
    let tables = state
        .tables
        .iter()
        .map(|(name, table)| {
            let rows = table
                .row_multiplicities
                .iter()
                .map(|(row, multiplicity)| {
                    json!({
                        "canonical_row_hex": lower_hex(row),
                        "multiplicity": multiplicity,
                    })
                })
                .collect::<Vec<_>>();
            let governed_rows = table
                .governed_rows
                .iter()
                .map(|(key, row)| {
                    json!({
                        "governed_key_hex": lower_hex(key),
                        "canonical_row_hex": lower_hex(row),
                    })
                })
                .collect::<Vec<_>>();
            (
                name.clone(),
                json!({
                    "canonical_schema_digest": digest(&table.canonical_schema),
                    "row_count": table.row_count,
                    "rows": rows,
                    "governed_rows": governed_rows,
                }),
            )
        })
        .collect::<BTreeMap<_, _>>();
    json!({
        "comparison_contract": "comparison-ignore-registry+generated-projections",
        "state_digest": lower_hex(&state.digest()),
        "tables": tables,
    })
}

fn observe_event_header(
    header: Option<&QueryEventHeader>,
    daemon_query_id: &str,
    expected_sequence: u64,
) -> Result<(), GateBCandidateError> {
    let header = header.ok_or_else(|| invariant("Gate B query event lacks its header"))?;
    if header.daemon_query_id != daemon_query_id
        || header.sequence != expected_sequence
        || header.event_checksum
            != digest(format!("{daemon_query_id}:{expected_sequence}").as_bytes())
    {
        return Err(invariant(
            "Gate B query event correlation, sequence, or checksum differs",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // One vertical keeps daemon, stream, artifact, and MCP correlation visibly coherent.
async fn run_query_vertical(
    repository_root: &Path,
    vertical_root: &Path,
    workspace_root: &Path,
    candidate: Arc<crate::snapshot_runtime::ServingSnapshotCandidate>,
    durable_pointer_generation: u64,
) -> Result<QueryPlanes, GateBCandidateError> {
    let query_state_root = vertical_root.join("query-state");
    fs::create_dir(&query_state_root)?;
    let mut store = OperationalStore::open(&query_state_root.join("operational.sqlite"))?;
    let record = {
        let mut registry = WorkspaceRegistry::new(&mut store);
        let registered = registry.add_directory_fixture(workspace_root, [0x7b; 16])?;
        registry.enable(registered.workspace_id)?
    };
    if candidate.manifest().raw_workspace_id().map_err(invariant)? != record.workspace_id {
        return Err(invariant(
            "serving activation workspace identity differs from provider execution",
        ));
    }
    let runtime = ServingSnapshotRuntime::default();
    runtime
        .activate(
            &mut store,
            Arc::clone(&candidate),
            None,
            0,
            durable_pointer_generation,
            10_000,
            None,
        )
        .map_err(invariant)?;
    let mut source_images = SourceImageStore::open(
        &vertical_root.join("clean/source-blobs"),
        SourceCapturePolicy::default(),
    )?;
    let lease = SnapshotLeaseManager::new([0x7e; 16])
        .acquire(
            &mut store,
            &mut source_images,
            Arc::clone(&candidate),
            SnapshotLeaseKind::Query,
            Some(b"gate-b-query-agent"),
            10_001,
            Duration::from_secs(600),
            None,
        )
        .map_err(invariant)?;
    let session = Arc::new(
        ServingQuerySession::from_lease(
            lease,
            &store.reader_factory(),
            ServingRuntimeConfig::new(
                64 * 1024 * 1024,
                128 * 1024 * 1024,
                vertical_root.join("query-spill"),
                2,
            )
            .map_err(invariant)?,
        )
        .map_err(invariant)?,
    );
    let canonical_tables = canonical_state_plane(
        &CanonicalState::from_serving_session(&session)
            .await
            .map_err(invariant)?,
    );
    let backend = Arc::new(WorkspaceQueryBackend::default());
    backend.install(session).await.map_err(invariant)?;
    let workspace_id = candidate.manifest().body.workspace_id.clone();
    let daemon_root = std::env::temp_dir().join(format!(
        "cfgb-{}-{}",
        std::process::id(),
        SHORT_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&daemon_root)?;
    fs::set_permissions(&daemon_root, fs::Permissions::from_mode(0o700))?;
    let _daemon_directory = DirectoryGuard(daemon_root.clone());
    let config = daemon_config(&daemon_root, repository_root)?;
    let discovery = config.static_config.runtime_root.join("daemon.json");
    let query_socket = config.static_config.query_socket_endpoint.clone();
    let claim = WorkspaceClaim {
        workspace_id: workspace_id.clone(),
        repository_id: None,
        worktree_id: None,
        workspace_kind: "non-git-root".to_owned(),
        readiness: WorkspaceReadiness::Ready as i32,
        permission_claims: vec!["query".to_owned()],
    };
    let daemon = tokio::spawn(serve_with_query_backend(config, backend, vec![claim], None));
    if let Err(error) = wait_for_discovery(&discovery, Duration::from_secs(20)).await {
        if daemon.is_finished() {
            let exit = daemon.await.map_err(invariant)?;
            return Err(invariant(format!(
                "Gate B daemon exited before discovery: {exit:?}"
            )));
        }
        return Err(invariant(error));
    }

    let execution = async {
        let mut client = query_client(query_socket.clone()).await?;
        let mut host = HostCapabilityProfile {
            delivery_modes: vec![
                DeliveryPreference::Inline as i32,
                DeliveryPreference::Resource as i32,
                DeliveryPreference::Auto as i32,
            ],
            compression_algorithms: vec![PayloadCompression::Identity as i32],
            supports_resource_links: true,
            supports_trace_context: true,
            maximum_frame_bytes: 1_048_576,
            profile_digest: String::new(),
        };
        host.profile_digest = crate::query_service::host_capability_profile_digest(&host)
            .map_err(invariant)?;
        let handshake = client
            .handshake(HandshakeRequest {
                rpc_versions: Some(VersionRange {
                    minimum: "1.0".to_owned(),
                    maximum: "1.0".to_owned(),
                }),
                semantic_query_versions: Some(VersionRange {
                    minimum: "1.3".to_owned(),
                    maximum: "1.3".to_owned(),
                }),
                required_feature_bits: CpgdFeatureMask::REQUIRED.bits(),
                optional_feature_bits: CpgdFeatureMask::SUPPORTED
                    .missing_from(CpgdFeatureMask::REQUIRED)
                    .bits(),
                desired_workspace_ids: vec![workspace_id.clone()],
                host_capabilities: Some(host.clone()),
                credential_proof: Some(CredentialProof {
                    credential_id: "gate-b-credential".to_owned(),
                    capability_token: b"gate-b-query-capability-token".to_vec(),
                }),
                agent_instance_id: "gate-b-rpc-agent".to_owned(),
                ..HandshakeRequest::default()
            })
            .await
            .map_err(invariant)?
            .into_inner();
        if handshake.authorized_workspaces.len() != 1 {
            return Err(invariant("Gate B daemon handshake did not authorize the workspace"));
        }
        let request_text = eight_form_request(&workspace_id, "gate-b-rpc-eight-form");
        let canonical_request = canonicalize_slice(request_text.as_bytes()).map_err(invariant)?;
        let started = client
            .start_query(StartQueryRequest {
                agent_instance_id: "gate-b-rpc-agent".to_owned(),
                workspace_id: workspace_id.clone(),
                semantic_query_version: "1.3".to_owned(),
                canonical_request_json: canonical_request.clone(),
                request_checksum: digest(&canonical_request),
                delivery_preference: DeliveryPreference::Resource as i32,
                deadline_unix_ms: now_millis() + 120_000,
                idempotency_key: "gate-b-rpc-eight-form".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                host_capability_profile_digest: host.profile_digest,
                mcp_call_id: "mcp-call:gate-b-rpc".to_owned(),
                ..StartQueryRequest::default()
            })
            .await
            .map_err(invariant)?
            .into_inner();
        let daemon_query_id = started.daemon_query_id.clone();
        let mut stream = client
            .stream_query(StreamQueryRequest {
                daemon_query_id: started.daemon_query_id,
                resume_token: started.resume_token,
                after_sequence: 0,
            })
            .await
            .map_err(invariant)?
            .into_inner();
        let mut event_kinds = Vec::new();
        let mut event_count = 0_u64;
        let mut artifact = None;
        let mut terminal_succeeded = false;
        let mut terminal_error = None;
        while let Some(event) = stream.message().await.map_err(invariant)? {
            match event.event {
                Some(Event::SnapshotPinned(value)) => {
                    event_count = event_count.saturating_add(1);
                    observe_event_header(value.header.as_ref(), &daemon_query_id, event_count)?;
                    event_kinds.push("snapshot_pinned");
                }
                Some(Event::Progress(value)) => {
                    event_count = event_count.saturating_add(1);
                    observe_event_header(value.header.as_ref(), &daemon_query_id, event_count)?;
                    event_kinds.push("progress");
                }
                Some(Event::ResponseChunk(value)) => {
                    event_count = event_count.saturating_add(1);
                    observe_event_header(value.header.as_ref(), &daemon_query_id, event_count)?;
                    event_kinds.push("response_chunk");
                }
                Some(Event::ArtifactReady(value)) => {
                    event_count = event_count.saturating_add(1);
                    observe_event_header(value.header.as_ref(), &daemon_query_id, event_count)?;
                    event_kinds.push("artifact_ready");
                    artifact = Some(value);
                }
                Some(Event::Terminal(value)) => {
                    event_count = event_count.saturating_add(1);
                    observe_event_header(value.header.as_ref(), &daemon_query_id, event_count)?;
                    event_kinds.push("terminal");
                    terminal_succeeded = value.execution_state
                        == crate::rpc::generated::codefabric::cpgd::v1::QueryExecutionState::Succeeded
                            as i32;
                    terminal_error = value.canonical_error_record_json.map(|bytes| {
                        String::from_utf8_lossy(&bytes).into_owned()
                    });
                }
                None => return Err(invariant("Gate B query stream contained an empty event")),
            }
        }
        if !terminal_succeeded {
            return Err(invariant(format!(
                "Gate B UDS query did not succeed: {}",
                terminal_error.as_deref().unwrap_or("terminal error record absent")
            )));
        }
        let artifact = artifact.ok_or_else(|| invariant("Gate B query emitted no artifact"))?;
        let artifact_id = artifact.artifact_id.clone();
        let mut chunks = client
            .read_result(ReadResultRequest {
                artifact_id: artifact.artifact_id,
                offset: 0,
                maximum_bytes: None,
                lease_token: artifact.lease_token,
                accepted_compression: PayloadCompression::Identity as i32,
            })
            .await
            .map_err(invariant)?
            .into_inner();
        let mut response_bytes = Vec::new();
        while let Some(chunk) = chunks.message().await.map_err(invariant)? {
            response_bytes.extend_from_slice(&chunk.payload);
            if chunk.final_chunk {
                break;
            }
        }
        let response: Value = serde_json::from_slice(&response_bytes)?;
        if response["successful_query_count"] != 8 {
            return Err(invariant("Gate B daemon did not execute all eight query forms"));
        }

        let adapter_request = vertical_root.join("gate-b-mcp-request.json");
        fs::write(
            &adapter_request,
            eight_form_request(&workspace_id, "gate-b-mcp-eight-form"),
        )?;
        let probe = repository_root.join("tooling/gate_b_adapter_probe.py");
        let python = repository_root.join("codefabric-cpg-mcp/.venv/bin/python");
        let command_root = repository_root.to_path_buf();
        let output = tokio::task::spawn_blocking(move || {
            Command::new(python)
                .arg(probe)
                .arg(adapter_request)
                .current_dir(command_root)
                .env(
                    "CODEFABRIC_CPG_DAEMON_TARGET",
                    format!("unix://{}", query_socket.display()),
                )
                .env("CODEFABRIC_WORKSPACE_ID", &workspace_id)
                .env("CODEFABRIC_AGENT_INSTANCE_ID", "gate-b-stdio-agent")
                .env(
                    "CODEFABRIC_CPG_CAPABILITY_TOKEN",
                    "gate-b-query-capability-token",
                )
                .stdin(Stdio::null())
                .output()
        })
        .await
        .map_err(invariant)??;
        if !output.status.success() {
            return Err(invariant(format!(
                "locked FastMCP STDIO probe failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let mut mcp: Value = serde_json::from_slice(&output.stdout)?;
        if mcp["transport"] != "stdio"
            || mcp["structured_content"]["delivery"]["response"]["successful_query_count"] != 8
        {
            return Err(invariant("locked FastMCP STDIO response is incomplete"));
        }
        let mcp_call_id = mcp["structured_content"]["mcp_call_id"]
            .as_str()
            .ok_or_else(|| invariant("locked FastMCP response lacks an MCP correlation id"))?;
        if !mcp_call_id.starts_with("execution:") {
            return Err(invariant("FastMCP correlation id is not the daemon execution id"));
        }
        mcp["structured_content"]
            .as_object_mut()
            .ok_or_else(|| invariant("locked FastMCP structured content is not an object"))?
            .remove("mcp_call_id");
        mcp.as_object_mut()
            .ok_or_else(|| invariant("locked FastMCP probe output is not an object"))?
            .insert("mcp_call_id_correlated".to_owned(), Value::Bool(true));
        let artifact_root = daemon_root.join("state/query-results");
        let plan_artifact_count = fs::read_dir(artifact_root.join("query-plan-artifacts"))?
            .filter_map(Result::ok)
            .count();
        if plan_artifact_count == 0 {
            return Err(invariant("Gate B query persisted no plan-artifact bundle"));
        }
        Ok(QueryPlanes {
            canonical_tables,
            queries: json!({
                "form_count": 8,
                "successful_query_count": response["successful_query_count"],
                "response_digest": digest(&response_bytes),
                "response_bytes_hex": lower_hex(&response_bytes),
                "snapshot_id": response["snapshot"]["snapshot_id"],
            }),
            rpc: json!({
                "transport": "unix-domain-socket",
                "daemon_query_id_correlated": true,
                "artifact_id": artifact_id,
                "event_kinds": event_kinds,
                "event_count": event_count,
                "event_checksums_valid": true,
                "mcp_call_id": "mcp-call:gate-b-rpc",
            }),
            mcp,
            diagnostics: json!({
                "artifact_persisted": true,
                "plan_artifact_count": plan_artifact_count,
                "terminal_state": "SUCCEEDED",
            }),
        })
    }
    .await;
    let stop = administer(&discovery, AdminCommand::Stop).await;
    let daemon_exit = daemon.await.map_err(invariant)?;
    stop.map_err(invariant)?;
    daemon_exit.map_err(invariant)?;
    execution
}

#[allow(clippy::too_many_lines)] // One Gate B execution keeps all eleven correlated planes in one auditable transaction.
pub(super) fn execute(
    repository_root: &Path,
    corpus_root: &Path,
    scratch_root: &Path,
) -> Result<VerticalExecution, GateBCandidateError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(invariant)?;
    runtime.block_on(async move {
        let vertical_root = scratch_root.join("gate-b-vertical");
        fs::create_dir(&vertical_root)?;
        let workspace_root = vertical_root.join("workspace");
        copy_directory(&corpus_root.join("workspace"), &workspace_root)?;
        let incremental_root = vertical_root.join("incremental");
        fs::create_dir(&incremental_root)?;
        let mut incremental_store =
            OperationalStore::open(&incremental_root.join("operational.sqlite"))?;
        let record = WorkspaceRegistry::new(&mut incremental_store)
            .add_directory_fixture(&workspace_root, [0x7b; 16])?;
        let lifecycle = candidate_lifecycle_config(false);
        let mut incremental = build_engine(
            record.workspace_id,
            &workspace_root,
            &incremental_root,
            0,
            0,
            lifecycle,
            engine_config(),
        )?;
        incremental
            .rebuild_from_zero(&mut incremental_store)
            .map_err(invariant)?
            .ok_or_else(|| invariant("Gate B initial rebuild did not publish"))?;
        write_workspace(
            &workspace_root,
            "python/golden_pkg/core.py",
            b"def normalized_total(values: list[int]) -> int:\n    return sum(values) + 7\n",
        )?;
        let hot = incremental
            .process_batch(
                &mut incremental_store,
                batch(
                    vec![hint(
                        "python/golden_pkg/core.py",
                        WatchHintKind::CreateOrModify,
                    )],
                    false,
                ),
                &BTreeMap::new(),
            )
            .map_err(invariant)?
            .ok_or_else(|| invariant("Gate B hot edit did not publish"))?;

        let clean_root = vertical_root.join("clean");
        fs::create_dir(&clean_root)?;
        let mut store = OperationalStore::open(&clean_root.join("operational.sqlite"))?;
        let clean_record = WorkspaceRegistry::new(&mut store)
            .add_directory_fixture(&workspace_root, [0x7b; 16])?;
        if clean_record.workspace_id != record.workspace_id {
            return Err(invariant("incremental and clean workspace identities differ"));
        }
        let mut clean = build_engine(
            clean_record.workspace_id,
            &workspace_root,
            &clean_root,
            0,
            0,
            lifecycle,
            engine_config(),
        )?;
        let rebuilt = clean
            .rebuild_from_zero(&mut store)
            .map_err(invariant)?
            .ok_or_else(|| invariant("Gate B current-byte rebuild did not publish"))?;
        let source_generation = rebuilt.wave.source_generation;
        let pyrefly = run_pyrefly(
            repository_root,
            &vertical_root,
            clean_record.workspace_id,
            SOURCE_CONTEXT_ID,
            source_generation,
            &workspace_root.join("python/golden_pkg/core.py"),
            &workspace_root.join("malformed/broken.py"),
        )
        .await?;
        let rustc = run_rustc(
            repository_root,
            &vertical_root,
            &workspace_root,
            clean_record.workspace_id,
            SOURCE_CONTEXT_ID,
            source_generation,
        )
        .await?;
        let core = CoreFactEngine::default();
        let mut canonicals = rebuilt
            .fast_outputs
            .into_iter()
            .map(|output| output.canonical)
            .collect::<Vec<_>>();
        canonicals.extend(core.reconcile_pyrefly_run(&pyrefly).map_err(invariant)?);
        canonicals.extend(core.reconcile_rustc_compilation(&rustc).map_err(invariant)?);
        let (derived_fact_digest, derived_row_count) = actual_derived_digest(&canonicals)?;
        let mut canonical_rows = BTreeMap::<i16, usize>::new();
        let mut explicit_unknown_rows = 0_usize;
        for canonical in &canonicals {
            for (&table_code, validated) in &canonical.batches {
                *canonical_rows.entry(table_code).or_default() += validated.num_rows();
                if table_code == 9 {
                    let states = validated
                        .batch()
                        .column_by_name("owner_capability_state_code")
                        .and_then(|array| array.as_any().downcast_ref::<Int16Array>())
                        .ok_or_else(|| invariant("capability state column is absent"))?;
                    let completeness = validated
                        .batch()
                        .column_by_name("completeness_state_code")
                        .and_then(|array| array.as_any().downcast_ref::<Int16Array>())
                        .ok_or_else(|| invariant("capability completeness column is absent"))?;
                    explicit_unknown_rows = explicit_unknown_rows.saturating_add(
                        (0..validated.num_rows())
                            .filter(|&row| {
                                states.value(row) != OwnerCapabilityState::Current as i16
                                    || completeness.value(row) != Completeness::Complete as i16
                            })
                            .count(),
                    );
                }
            }
        }
        let publication_id = [0x7c; 16];
        let contexts = vec![SOURCE_CONTEXT_ID];
        let mut fabric = bootstrap_workspace(&vertical_root.join("delta"), &clean_record)
            .await
            .map_err(invariant)?;
        let request = PublicationRequest {
            operation_id: [0x7d; 16],
            pins: PublicationPins {
                publication_id,
                workspace_id: clean_record.workspace_id,
                repository_id: None,
                worktree_id: None,
                source_generation: i64::try_from(source_generation)
                    .map_err(|_| invariant("Gate B generation exceeds i64"))?,
                source_inventory_digest: clean.current_inventory_digest(),
                analysis_context_set_id: crate::identity::context_set_identity(
                    clean_record.workspace_id,
                    &contexts,
                )
                .map_err(invariant)?
                .id,
                analysis_context_ids: contexts,
                git_state_fingerprint: None,
                inclusion_policy_fingerprint: [0x31; 32],
                base_fact_digest: rebuilt.overlay.checksum(),
                derived_fact_digest: Some(derived_fact_digest),
                ontology_version: "1.3".to_owned(),
                schema_bundle_version: "1.3".to_owned(),
                provider_bundle_version: "1.3".to_owned(),
                derivation_bundle_version: "1.3".to_owned(),
                toolchain_bundle_version: "1.3".to_owned(),
            },
            expected_pointer: None,
            expected_publication_table_version: fabric.table(5).unwrap().version(),
            expected_manifest_table_version: fabric.table(6).unwrap().version(),
            expected_pointer_table_version: fabric.table(7).unwrap().version(),
            started_at_micros: 1_000,
            completed_at_micros: 2_000,
        };
        let publication = core
            .publish_canonical_set(&mut fabric, &mut store, &request, canonicals)
            .await
            .map_err(invariant)?;
        let candidate = Arc::new(core
            .freeze_publication(
                &publication,
                snapshot_body(
                    clean_record.workspace_id,
                    source_generation,
                    clean.current_inventory_digest(),
                )?,
                &[],
            )
            .await
            .map_err(invariant)?);
        let source_inventory = rebuilt
            .wave
            .items
            .iter()
            .filter_map(|item| item.captured.as_deref())
            .map(|source| {
                json!({
                    "path": source.path.display_string,
                    "file_id": lower_hex(&source.file_id),
                    "digest": digest(&source.digest),
                    "byte_length": source.byte_length,
                })
            })
            .collect::<Vec<_>>();
        let provider_observations = json!({
            "tree_sitter_and_ruff_owner_count": canonical_rows.get(&8).copied().unwrap_or(0),
            "pyrefly": {
                "provider_run_id": pyrefly.provider_run_id,
                "module_ids": pyrefly.modules.iter().map(|module| &module.module_id).collect::<Vec<_>>(),
                "module_names": pyrefly.modules.iter().map(|module| &module.module_name).collect::<Vec<_>>(),
                "module_count": pyrefly.modules.len(),
                "terminal_digest_verified": true,
            },
            "rustc_mir": {
                "provider_run_id": rustc.admission.provider_run_id,
                "owner_count": rustc.owners.len(),
                "terminal_digest_verified": true,
            },
        });
        let publication_plane = json!({
            "publication_id": candidate.manifest().body.base_publication.publication_id,
            "pointer_generation": publication.pointer.pointer_generation,
            "tables": publication.tables.iter().map(|(code, table)| (code.to_string(), json!({
                "delta_version": table.delta_version,
                "row_count": table.row_count,
                "schema_fingerprint": lower_hex(&table.schema_fingerprint),
                "checksum": digest(&table.table_checksum),
                "validated": table.validated,
            }))).collect::<BTreeMap<_,_>>(),
        });
        let snapshot_plane = json!({
            "snapshot_id": candidate.manifest().snapshot_id,
            "publication_id": candidate.manifest().body.base_publication.publication_id,
            "source_generation": candidate.manifest().body.source.source_generation,
            "source_trust_state": candidate.manifest().body.source.source_trust_state,
            "manifest_digest": candidate.manifest().manifest_digest,
        });
        let identities = json!({
            "workspace_id": candidate.manifest().body.workspace_id,
            "analysis_context_id": candidate.manifest().body.contexts.records[0].analysis_context_id,
            "hot_wave_id": lower_hex(&hot.wave.wave_id),
            "clean_wave_id": lower_hex(&rebuilt.wave.wave_id),
            "owner_identity_is_application_owned": true,
        });
        let rebuild_plane = json!({
            "incremental_inventory_digest": digest(&incremental.current_inventory_digest()),
            "clean_inventory_digest": digest(&clean.current_inventory_digest()),
            "inventory_equal": incremental.current_inventory_digest() == clean.current_inventory_digest(),
            "independent_operational_roots": incremental_root != clean_root,
            "independent_delta_roots": incremental_root.join("delta") != vertical_root.join("delta"),
        });
        let query_planes = run_query_vertical(
            repository_root,
            &vertical_root,
            &workspace_root,
            Arc::clone(&candidate),
            u64::try_from(publication.pointer.pointer_generation).map_err(invariant)?,
        )
        .await?;
        let canonical_tables = json!({
            "rows": canonical_rows,
            "governed_effective_state": query_planes.canonical_tables,
            "contains_python_semantics": true,
            "contains_rust_mir": true,
            "contains_relation": publication.tables.get(&110).is_some_and(|table| table.row_count > 0),
            "contains_property": publication.tables.get(&120).is_some_and(|table| table.row_count > 0),
            "contains_derived": derived_row_count > 0,
            "contains_unknown": explicit_unknown_rows > 0,
            "explicit_unknown_row_count": explicit_unknown_rows,
            "derived_row_count": derived_row_count,
            "derived_fact_digest": digest(&derived_fact_digest),
        });
        let planes = BTreeMap::from([
            ("source_inventory".to_owned(), json!(source_inventory)),
            ("identities".to_owned(), identities),
            ("provider_observations".to_owned(), provider_observations),
            ("canonical_tables".to_owned(), canonical_tables),
            ("publications".to_owned(), publication_plane),
            ("serving_snapshots".to_owned(), snapshot_plane),
            ("queries".to_owned(), query_planes.queries),
            ("rpc".to_owned(), query_planes.rpc),
            ("mcp".to_owned(), query_planes.mcp),
            ("diagnostics".to_owned(), query_planes.diagnostics),
            ("rebuild_comparison".to_owned(), rebuild_plane),
        ]);
        let execution_material = planes
            .iter()
            .map(|(name, value)| Ok((name.clone(), plane_digest(value)?)))
            .collect::<Result<BTreeMap<_, _>, GateBCandidateError>>()?;
        let execution_digest = digest(&canonical_bytes(&execution_material)?);
        Ok(VerticalExecution {
            execution_id: "gate-b-vertical-v3".to_owned(),
            workspace_id: candidate.manifest().body.workspace_id.clone(),
            analysis_context_id: candidate.manifest().body.contexts.records[0]
                .analysis_context_id
                .clone(),
            source_generation,
            publication_id: candidate.manifest().body.base_publication.publication_id.clone(),
            snapshot_id: candidate.manifest().snapshot_id.clone(),
            provider_run_ids: BTreeMap::from([
                ("pyrefly-python".to_owned(), "run:gate-b-pyrefly".to_owned()),
                ("rustc-mir".to_owned(), "run:gate-b-rustc".to_owned()),
            ]),
            planes,
            execution_digest,
        })
    })
}

fn digest_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}
