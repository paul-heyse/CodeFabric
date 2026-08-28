//! Actual-output Gate B execution used only to assemble an unreleased review candidate.

use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arrow::array::{
    Array as _, BinaryArray, FixedSizeBinaryArray, Int16Array, Int32Array, ListArray, StringArray,
};
use arrow::ipc::reader::StreamReader;
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
use crate::provider_runtime::fixture::{CompatibilityProviderRuntimeDispatch, ProviderSourceBlob};
use crate::query_service::WorkspaceQueryBackend;
use crate::registries::{
    Completeness, CpgdFeatureMask, Language, OwnerCapabilityState, SnapshotLeaseKind,
};
use crate::rpc::generated::codefabric::cpgd::v1::cpg_query_service_client::CpgQueryServiceClient;
use crate::rpc::generated::codefabric::cpgd::v1::query_event::Event;
use crate::rpc::generated::codefabric::cpgd::v1::{
    CredentialProof, DeliveryPreference, HandshakeRequest, HostCapabilityProfile,
    PayloadCompression, QueryEventHeader, ReadResultRequest, StartQueryRequest, StreamQueryRequest,
    VersionRange, WorkspaceClaim, WorkspaceReadiness,
};
use crate::rustc_service::AcceptedRustcCompilation;
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

/// Named interventions applied at the input side of a producing or public seam.
///
/// These are deliberately part of the real Gate B vertical instead of mutations of the final
/// evidence JSON. Each variant changes an input consumed by the named subsystem, so a killed
/// intervention demonstrates causal sensitivity rather than comparator sensitivity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CausalIntervention {
    PyreflySourceAdmission,
    ReconciliationAuthority,
    DeltaPublication,
    SnapshotActivation,
    ArtifactPersistence,
    ArtifactReadback,
    FastMcpAdaptation,
}

fn intervention_is(intervention: Option<CausalIntervention>, expected: CausalIntervention) -> bool {
    intervention == Some(expected)
}

static SHORT_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct DirectoryGuard(PathBuf);

struct DirectoryPermissionGuard {
    path: PathBuf,
    original: fs::Permissions,
}

impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl DirectoryPermissionGuard {
    fn deny_writes(path: &Path) -> Result<Self, GateBCandidateError> {
        let metadata = fs::metadata(path)?;
        let original = metadata.permissions();
        let mut denied = original.clone();
        denied.set_mode(0o500);
        fs::set_permissions(path, denied)?;
        Ok(Self {
            path: path.to_path_buf(),
            original,
        })
    }
}

impl Drop for DirectoryPermissionGuard {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, self.original.clone());
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

fn normalize_review_numbers(value: &mut Value) {
    const MAXIMUM_INTEROPERABLE_INTEGER: u64 = 9_007_199_254_740_991;
    match value {
        Value::Array(values) => {
            for value in values {
                normalize_review_numbers(value);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                normalize_review_numbers(value);
            }
        }
        Value::Number(number)
            if number
                .as_u64()
                .is_some_and(|value| value > MAXIMUM_INTEROPERABLE_INTEGER)
                || number.as_i64().is_some_and(|value| {
                    value < -i64::try_from(MAXIMUM_INTEROPERABLE_INTEGER).unwrap_or(i64::MAX)
                }) =>
        {
            *value = Value::String(number.to_string());
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn decoded_pyrefly_observations(
    run: &crate::pyrefly_service::AcceptedPyreflyRun,
) -> Result<Vec<Value>, GateBCandidateError> {
    run.modules
        .iter()
        .map(|module| {
            let decode = |column: &str| -> Result<Value, GateBCandidateError> {
                let bytes = module
                    .batch
                    .column_by_name(column)
                    .and_then(|array| array.as_any().downcast_ref::<BinaryArray>())
                    .map(|array| array.value(0))
                    .ok_or_else(|| invariant(format!("Pyrefly {column} is absent")))?;
                let mut value: Value =
                    serde_json::from_slice(bytes).map_err(GateBCandidateError::from)?;
                normalize_review_numbers(&mut value);
                Ok(value)
            };
            Ok(json!({
                "module_name": module.module_name,
                "type_table": decode("type_table_json")?,
                "callees": decode("callees_json")?,
                "diagnostics": decode("diagnostics_json")?,
            }))
        })
        .collect()
}

fn string_list(batch: &arrow::record_batch::RecordBatch, column: &str) -> Vec<String> {
    batch
        .column_by_name(column)
        .and_then(|array| array.as_any().downcast_ref::<ListArray>())
        .map(|array| array.value(0))
        .and_then(|values| {
            values
                .as_any()
                .downcast_ref::<StringArray>()
                .map(|strings| {
                    strings
                        .iter()
                        .flatten()
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
        })
        .unwrap_or_default()
}

fn decoded_rustc_observations(
    compilation: &AcceptedRustcCompilation,
) -> Result<Vec<Value>, GateBCandidateError> {
    let mut observations = Vec::new();
    for owner in &compilation.owners {
        for chunk in &owner.chunks {
            let batches =
                StreamReader::try_new(Cursor::new(&chunk.arrow_ipc), None).map_err(invariant)?;
            for batch in batches {
                let batch = batch.map_err(invariant)?;
                let text = |column: &str| {
                    batch
                        .column_by_name(column)
                        .and_then(|array| array.as_any().downcast_ref::<StringArray>())
                        .map(|array| array.value(0).to_owned())
                };
                observations.push(json!({
                    "name": text("name"),
                    "item_kind": text("item_kind"),
                    "type_description": text("type_description"),
                    "statement_kinds": string_list(&batch, "statement_kinds"),
                    "terminator_kinds": string_list(&batch, "terminator_kinds"),
                }));
            }
        }
    }
    observations.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
    Ok(observations)
}

#[allow(clippy::too_many_lines)] // A single decoded projection keeps entity joins visible to reviewers.
fn decoded_canonical_semantics(
    canonicals: &[crate::fact_ingest::CanonicalIngestOutput],
) -> Result<Value, GateBCandidateError> {
    let mut names = BTreeMap::<Vec<u8>, String>::new();
    let mut entities = Vec::new();
    for canonical in canonicals {
        let Some(validated) = canonical.batches.get(&100) else {
            continue;
        };
        let batch = validated.batch();
        let ids = batch
            .column_by_name("entity_id")
            .and_then(|array| array.as_any().downcast_ref::<FixedSizeBinaryArray>())
            .ok_or_else(|| invariant("canonical entity_id is absent"))?;
        let entity_names = batch
            .column_by_name("name")
            .and_then(|array| array.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| invariant("canonical entity name is absent"))?;
        let qualified_names = batch
            .column_by_name("qualified_name")
            .and_then(|array| array.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| invariant("canonical entity qualified_name is absent"))?;
        let kinds = batch
            .column_by_name("entity_kind_code")
            .and_then(|array| array.as_any().downcast_ref::<Int32Array>())
            .ok_or_else(|| invariant("canonical entity kind is absent"))?;
        let languages = batch
            .column_by_name("language")
            .and_then(|array| array.as_any().downcast_ref::<Int16Array>())
            .ok_or_else(|| invariant("canonical entity language is absent"))?;
        let file_ids = batch
            .column_by_name("file_id")
            .and_then(|array| array.as_any().downcast_ref::<FixedSizeBinaryArray>())
            .ok_or_else(|| invariant("canonical entity file_id is absent"))?;
        for row in 0..batch.num_rows() {
            let short_name =
                (!entity_names.is_null(row)).then(|| entity_names.value(row).to_owned());
            let qualified_name =
                (!qualified_names.is_null(row)).then(|| qualified_names.value(row).to_owned());
            let name = qualified_name.clone().or_else(|| short_name.clone());
            if let Some(name) = &name {
                names.insert(ids.value(row).to_vec(), name.clone());
            }
            entities.push(json!({
                "entity_id": format!("entity:{}", lower_hex(ids.value(row))),
                "name": name,
                "short_name": short_name,
                "qualified_name": qualified_name,
                "file_id": (!file_ids.is_null(row))
                    .then(|| format!("file:{}", lower_hex(file_ids.value(row)))),
                "entity_kind_code": kinds.value(row),
                "language_code": languages.value(row),
            }));
        }
    }
    entities.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));

    let mut relations = Vec::new();
    let mut properties = Vec::new();
    let mut capabilities = Vec::new();
    for canonical in canonicals {
        if let Some(validated) = canonical.batches.get(&110) {
            let batch = validated.batch();
            let sources = batch
                .column_by_name("source_id")
                .and_then(|array| array.as_any().downcast_ref::<FixedSizeBinaryArray>())
                .ok_or_else(|| invariant("canonical relation source is absent"))?;
            let targets = batch
                .column_by_name("target_id")
                .and_then(|array| array.as_any().downcast_ref::<FixedSizeBinaryArray>())
                .ok_or_else(|| invariant("canonical relation target is absent"))?;
            let kinds = batch
                .column_by_name("relation_kind_code")
                .and_then(|array| array.as_any().downcast_ref::<Int32Array>())
                .ok_or_else(|| invariant("canonical relation kind is absent"))?;
            let certainty = batch
                .column_by_name("certainty_code")
                .and_then(|array| array.as_any().downcast_ref::<Int16Array>())
                .ok_or_else(|| invariant("canonical relation certainty is absent"))?;
            let fact_ids = batch
                .column_by_name("fact_id")
                .and_then(|array| array.as_any().downcast_ref::<FixedSizeBinaryArray>())
                .ok_or_else(|| invariant("canonical relation fact_id is absent"))?;
            for row in 0..batch.num_rows() {
                relations.push(json!({
                    "fact_id": format!("fact:relation:{}", lower_hex(fact_ids.value(row))),
                    "source_id": format!("entity:{}", lower_hex(sources.value(row))),
                    "target_id": format!("entity:{}", lower_hex(targets.value(row))),
                    "source_name": names.get(sources.value(row)),
                    "target_name": names.get(targets.value(row)),
                    "relation_kind_code": kinds.value(row),
                    "certainty_code": certainty.value(row),
                }));
            }
        }
        if let Some(validated) = canonical.batches.get(&120) {
            let batch = validated.batch();
            let subjects = batch
                .column_by_name("subject_entity_id")
                .and_then(|array| array.as_any().downcast_ref::<FixedSizeBinaryArray>())
                .ok_or_else(|| invariant("canonical property subject is absent"))?;
            let kinds = batch
                .column_by_name("property_kind_code")
                .and_then(|array| array.as_any().downcast_ref::<Int32Array>())
                .ok_or_else(|| invariant("canonical property kind is absent"))?;
            let values = batch
                .column_by_name("value_text")
                .and_then(|array| array.as_any().downcast_ref::<StringArray>())
                .ok_or_else(|| invariant("canonical property text is absent"))?;
            let fact_ids = batch
                .column_by_name("fact_id")
                .and_then(|array| array.as_any().downcast_ref::<FixedSizeBinaryArray>())
                .ok_or_else(|| invariant("canonical property fact_id is absent"))?;
            for row in 0..batch.num_rows() {
                properties.push(json!({
                    "fact_id": format!("fact:property:{}", lower_hex(fact_ids.value(row))),
                    "subject_id": format!("entity:{}", lower_hex(subjects.value(row))),
                    "subject_name": names.get(subjects.value(row)),
                    "property_kind_code": kinds.value(row),
                    "value_text": (!values.is_null(row)).then(|| values.value(row)),
                }));
            }
        }
        if let Some(validated) = canonical.batches.get(&9) {
            let batch = validated.batch();
            let codes = batch
                .column_by_name("capability_code")
                .and_then(|array| array.as_any().downcast_ref::<Int16Array>())
                .ok_or_else(|| invariant("canonical capability code is absent"))?;
            let states = batch
                .column_by_name("owner_capability_state_code")
                .and_then(|array| array.as_any().downcast_ref::<Int16Array>())
                .ok_or_else(|| invariant("canonical capability state is absent"))?;
            let completeness = batch
                .column_by_name("completeness_state_code")
                .and_then(|array| array.as_any().downcast_ref::<Int16Array>())
                .ok_or_else(|| invariant("canonical completeness is absent"))?;
            for row in 0..batch.num_rows() {
                capabilities.push(json!({
                    "capability_code": codes.value(row),
                    "state_code": states.value(row),
                    "completeness_code": completeness.value(row),
                }));
            }
        }
    }
    Ok(json!({
        "entities": entities,
        "relations": relations,
        "properties": properties,
        "capabilities": capabilities,
    }))
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
            sandbox_profile_digests: BTreeMap::from([
                ("pyrefly-python".into(), digest(b"gate-b-pyrefly-sandbox")),
                ("rustc-mir".into(), digest(b"gate-b-rustc-sandbox")),
            ]),
        },
        result_authority: None,
        limits_profile_digest: digest(b"gate-b-limits"),
        source_blob_digests: Vec::new(),
    })
}

fn plane_digest(value: &Value) -> Result<String, GateBCandidateError> {
    Ok(digest(&canonical_bytes(value)?))
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

#[derive(Debug)]
struct FunctionalQueryTargets {
    rust_callables: Vec<String>,
    ffi_call_site: String,
    ffi_target: String,
    context_source_file: String,
}

fn functional_query_targets(
    decoded: &Value,
) -> Result<FunctionalQueryTargets, GateBCandidateError> {
    let mut rust_callables = decoded["entities"]
        .as_array()
        .ok_or_else(|| invariant("decoded canonical entities are absent"))?
        .iter()
        .filter(|entity| entity["language_code"] == Language::Rust as i16)
        .filter(|entity| {
            entity["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("codefabric_gate_b_rust::"))
        })
        .filter_map(|entity| entity["entity_id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    rust_callables.sort();
    if rust_callables.len() != 3 {
        return Err(invariant(format!(
            "functional query expected three rustc callables, observed {}",
            rust_callables.len()
        )));
    }
    let relations = decoded["relations"]
        .as_array()
        .ok_or_else(|| invariant("decoded canonical relations are absent"))?;
    let relation = relations
        .iter()
        .find(|relation| {
            relation["source_name"]
                .as_str()
                .is_some_and(|name| name.starts_with("golden_pkg.core:"))
                && relation["target_name"] == "golden_pkg.core.scale"
        })
        .ok_or_else(|| {
            let named = relations
                .iter()
                .filter_map(|relation| {
                    Some(format!(
                        "{} -> {}",
                        relation["source_name"].as_str()?,
                        relation["target_name"].as_str()?
                    ))
                })
                .collect::<Vec<_>>();
            invariant(format!(
                "functional Python call-target relation is absent; named relations: {named:?}"
            ))
        })?;
    let call_site_id = relation["source_id"]
        .as_str()
        .ok_or_else(|| invariant("functional Python call-site identity is absent"))?;
    let call_site_file = decoded["entities"]
        .as_array()
        .and_then(|entities| {
            entities
                .iter()
                .find(|entity| entity["entity_id"] == call_site_id)
        })
        .and_then(|entity| entity["file_id"].as_str())
        .ok_or_else(|| invariant("functional Python call-site file identity is absent"))?;
    let source_file_kind = crate::registries::entity_kind("SOURCE_FILE")
        .ok_or_else(|| invariant("SOURCE_FILE registry allocation is absent"))?
        .code;
    let context_source_file = decoded["entities"]
        .as_array()
        .and_then(|entities| {
            entities.iter().find(|entity| {
                entity["entity_kind_code"] == source_file_kind
                    && entity["file_id"] == call_site_file
            })
        })
        .and_then(|entity| entity["entity_id"].as_str())
        .ok_or_else(|| invariant("functional source-file entity is absent"))?
        .to_owned();
    Ok(FunctionalQueryTargets {
        rust_callables,
        ffi_call_site: call_site_id.to_owned(),
        ffi_target: relation["target_id"]
            .as_str()
            .ok_or_else(|| invariant("functional Python call-target identity is absent"))?
            .to_owned(),
        context_source_file,
    })
}

fn eight_form_request(
    workspace_id: &str,
    request_id: &str,
    targets: &FunctionalQueryTargets,
) -> Result<String, GateBCandidateError> {
    let limit = json!({"limit": {"maximum_results": 256}});
    let rust_references = targets
        .rust_callables
        .iter()
        .map(|entity_id| json!({"entity_id": entity_id}))
        .collect::<Vec<_>>();
    let request = json!({
        "specification": "composable semantic CPG fact query",
        "version": "1.3",
        "semantic_request_id": request_id,
        "workspace_id": workspace_id,
        "freshness_policy": "best_available_snapshot",
        "queries": [
            {"query_id":"q.entities","request":"find code entities","label":null,
             "looking_for":"source files","within":[{"entity_id":targets.context_source_file}],
             "return":limit},
            {"query_id":"q.facts","request":"retrieve facts about code","label":null,
             "about":rust_references,
             "facts":["callable contracts"],"return":limit},
            {"query_id":"q.relationships","request":"follow code relationships","label":null,
             "starting_from":[{"entity_id":targets.ffi_call_site}],"relationship":"call targets",
             "direction":"outgoing","distance":"direct","return":limit},
            {"query_id":"q.paths","request":"find connecting fact paths","label":null,
             "starting_from":[{"entity_id":targets.ffi_call_site}],
             "ending_at":[{"entity_id":targets.ffi_target}],"through":["control flow"],
             "path_policy":"one shortest witness path","direction":"outgoing",
             "maximum_length":1,"return":limit},
            {"query_id":"q.pattern","request":"match a code fact pattern","label":null,
             "bindings":[
             {"name":"source","looking_for":"syntax nodes",
             "within":{"entity_id":targets.ffi_call_site}},
             {"name":"target","looking_for":"Python async generators",
             "within":{"entity_id":targets.ffi_target}}],
             "relationships":[{"from":"source","to":"target",
             "relationship":"call targets","direction":"outgoing","distance":"direct"}],
             "return":limit},
            {"query_id":"q.combine","request":"combine result sets","label":null,
             "inputs":[{"results_of":"q.facts","select":"facts"},
             {"results_of":"q.relationships","select":"facts"}],
             "combination":"union by fact identity","identity":"fact identity",
             "preserve_origin":"all origins","return":limit},
            {"query_id":"q.summary","request":"summarize objective facts","label":null,
             "input":[{"results_of":"q.combine","select":"groups"}],
             "summaries":["graph metrics"],"return":limit},
            {"query_id":"q.context","request":"retrieve source and syntax context","label":null,
             "for":[{"results_of":"q.entities","select":"entities"}],
             "context":["source location","exact span"],"text_handling":"omit text",
             "return":limit}
        ],
        "response_projection":{"canonical_semantic_identity":true,"coverage":true},
        "cost_budget":{"maximum_rows":2048}
    });
    serde_json::to_string(&request).map_err(GateBCandidateError::from)
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
    decoded_canonical: &Value,
    intervention: Option<CausalIntervention>,
) -> Result<QueryPlanes, GateBCandidateError> {
    let functional_targets = functional_query_targets(decoded_canonical)?;
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
        .commit_fact_snapshot(
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
            Some(&[0x7d; 16]),
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

    let result_root = daemon_root.join("state/query-results");
    let _artifact_permission_fault =
        if intervention_is(intervention, CausalIntervention::ArtifactPersistence) {
            Some(DirectoryPermissionGuard::deny_writes(&result_root)?)
        } else {
            None
        };

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
        let request_text = eight_form_request(
            &workspace_id,
            "gate-b-rpc-eight-form",
            &functional_targets,
        )?;
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
        if intervention_is(intervention, CausalIntervention::ArtifactReadback) {
            let first = response_bytes
                .first_mut()
                .ok_or_else(|| invariant("Gate B artifact readback was empty"))?;
            *first ^= 0xff;
        }
        let response: Value = serde_json::from_slice(&response_bytes)?;
        if response["successful_query_count"] != 8 {
            return Err(invariant("Gate B daemon did not execute all eight query forms"));
        }

        let adapter_request = vertical_root.join("gate-b-mcp-request.json");
        let adapter_request_bytes = if intervention_is(
            intervention,
            CausalIntervention::FastMcpAdaptation,
        ) {
            b"{}".to_vec()
        } else {
            eight_form_request(
                &workspace_id,
                "gate-b-rpc-eight-form",
                &functional_targets,
            )?
            .into_bytes()
        };
        fs::write(&adapter_request, adapter_request_bytes)?;
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
        if mcp["structured_content"]["delivery"]["response"] != response {
            return Err(invariant(
                "FastMCP decoded response differs from UDS artifact readback",
            ));
        }
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
                "decoded_response": response.clone(),
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

#[cfg(test)]
pub(super) fn execute_authored_workspace(
    repository_root: &Path,
    corpus_root: &Path,
    scratch_root: &Path,
) -> Result<VerticalExecution, GateBCandidateError> {
    execute_with_hot_edit(repository_root, corpus_root, scratch_root, false, None)
}

pub(super) fn execute_functional_candidate(
    repository_root: &Path,
    corpus_root: &Path,
    scratch_root: &Path,
) -> Result<VerticalExecution, GateBCandidateError> {
    execute_with_hot_edit(repository_root, corpus_root, scratch_root, false, None)
}

#[cfg(test)]
pub(super) fn execute_with_intervention(
    repository_root: &Path,
    corpus_root: &Path,
    scratch_root: &Path,
    intervention: CausalIntervention,
) -> Result<VerticalExecution, GateBCandidateError> {
    execute_with_hot_edit(
        repository_root,
        corpus_root,
        scratch_root,
        false,
        Some(intervention),
    )
    .map_err(|error| {
        invariant(format!(
            "causal intervention {intervention:?} was detected at its producing/public seam: {error}"
        ))
    })
}

#[allow(clippy::too_many_lines)] // One Gate B execution keeps all eleven correlated planes in one auditable transaction.
fn execute_with_hot_edit(
    repository_root: &Path,
    corpus_root: &Path,
    scratch_root: &Path,
    apply_hot_edit: bool,
    intervention: Option<CausalIntervention>,
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
        let initial = incremental
            .rebuild_from_zero(&mut incremental_store)
            .map_err(invariant)?
            .ok_or_else(|| invariant("Gate B initial rebuild did not publish"))?;
        let hot = if apply_hot_edit {
            let hot_edit: &[u8] = if corpus_root.join("functional-expectations.json").is_file() {
                b"def scale(value: int) -> int:  # @anchor py.scale\n    return value * 2\n\n\ndef pipeline(value: int) -> int:  # @anchor py.pipeline\n    return scale(value) + 7  # @anchor py.call.scale\n\n\nclass Counter:  # @anchor py.counter\n    def increment(self, value: int) -> int:  # @anchor py.counter.increment\n        return value + 1\n"
            } else {
                b"def normalized_total(values: list[int]) -> int:\n    return sum(values) + 7\n"
            };
            write_workspace(&workspace_root, "python/golden_pkg/core.py", hot_edit)?;
            incremental
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
                .ok_or_else(|| invariant("Gate B hot edit did not publish"))?
        } else {
            initial
        };

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
        let captured_blob = |display_path: &str| -> Result<ProviderSourceBlob, GateBCandidateError> {
            let image = rebuilt
                .wave
                .items
                .iter()
                .filter_map(|item| item.captured.as_deref())
                .find(|image| image.path.display_string == display_path)
                .ok_or_else(|| invariant(format!("captured source image is absent: {display_path}")))?;
            Ok(ProviderSourceBlob {
                path: clean_root.join("source-blobs").join(&image.blob.relative_name),
                content_digest: format!("b3:{}", lower_hex(&image.digest)),
                file_id: image.file_id,
                image: image.clone(),
            })
        };
        let mut python_blob = captured_blob("python/golden_pkg/core.py")?;
        let ffi_blob = captured_blob("ffi/boundary.py")?;
        let invalid_python_blob = captured_blob("malformed/broken.py")?;
        let rust_blob = captured_blob("rust/src/lib.rs")?;
        if intervention_is(
            intervention,
            CausalIntervention::PyreflySourceAdmission,
        ) {
            python_blob.content_digest = format!("b3:{}", "0".repeat(64));
        }
        let provider_dispatch = CompatibilityProviderRuntimeDispatch::new(
            repository_root,
            &vertical_root,
            clean_record.workspace_id,
            SOURCE_CONTEXT_ID,
            source_generation,
        );
        let mut pyrefly = provider_dispatch
            .pyrefly(
            &python_blob,
            &ffi_blob,
            &invalid_python_blob,
        )
        .await
        .map_err(invariant)?;
        let rustc = provider_dispatch.rustc(&rust_blob).await.map_err(invariant)?;
        let core = CoreFactEngine::default();
        if intervention_is(
            intervention,
            CausalIntervention::ReconciliationAuthority,
        ) {
            pyrefly
                .modules
                .retain(|module| module.module_name != "golden_pkg.core");
        }
        let mut canonicals = rebuilt
            .fast_outputs
            .into_iter()
            .map(|output| output.canonical)
            .collect::<Vec<_>>();
        canonicals.extend(core.reconcile_pyrefly_run(&pyrefly).map_err(invariant)?);
        canonicals.extend(core.reconcile_rustc_compilation(&rustc).map_err(invariant)?);
        let decoded_canonical = decoded_canonical_semantics(&canonicals)?;
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
            expected_publication_table_version: if intervention_is(
                intervention,
                CausalIntervention::DeltaPublication,
            ) {
                fabric
                    .table(5)
                    .unwrap()
                    .version()
                    .map(|version| version.saturating_add(1))
            } else {
                fabric.table(5).unwrap().version()
            },
            expected_manifest_table_version: fabric.table(6).unwrap().version(),
            expected_pointer_table_version: fabric.table(7).unwrap().version(),
            started_at_micros: 1_000,
            completed_at_micros: 2_000,
        };
        let publication = core
            .publish_canonical_set(&mut fabric, &mut store, &request, canonicals)
            .await
            .map_err(invariant)?;
        let mut candidate_body = snapshot_body(
            clean_record.workspace_id,
            source_generation,
            clean.current_inventory_digest(),
        )?;
        if intervention_is(intervention, CausalIntervention::SnapshotActivation) {
            "STALE".clone_into(&mut candidate_body.source.source_trust_state);
        }
        let candidate = Arc::new(core
            .freeze_publication(
                &publication,
                candidate_body,
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
        let decoded_pyrefly = decoded_pyrefly_observations(&pyrefly)?;
        let decoded_rustc = decoded_rustc_observations(&rustc)?;
        let provider_observations = json!({
            "tree_sitter_and_ruff_owner_count": canonical_rows.get(&8).copied().unwrap_or(0),
            "pyrefly": {
                "provider_run_id": pyrefly.provider_run_id,
                "module_ids": pyrefly.modules.iter().map(|module| &module.module_id).collect::<Vec<_>>(),
                "module_names": pyrefly.modules.iter().map(|module| &module.module_name).collect::<Vec<_>>(),
                "module_count": pyrefly.modules.len(),
                "terminal_digest_verified": true,
                "decoded_semantics": decoded_pyrefly,
            },
            "rustc_mir": {
                "provider_run_id": rustc.admission.provider_run_id,
                "owner_count": rustc.owners.len(),
                "terminal_digest_verified": true,
                "decoded_semantics": decoded_rustc,
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
            &decoded_canonical,
            intervention,
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
            "decoded_semantics": decoded_canonical,
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
