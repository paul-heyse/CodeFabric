//! Deterministic WP09 schema-contract compiler.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::artifacts::ContractArtifactError;
use super::catalog::{CompiledCatalog, DerivationOutputKind};
use super::compiler::compile_artifact_for_generation;
use super::jcs::{canonicalize_value, checksum, decode_strict};
use super::schema_models::{
    LogicalType, PublicSchemaContract, PublicSchemaKind, SchemaContractIr, SqliteType,
};

pub(super) const SCHEMA_DERIVATION_ID: &str = "codefabric.derivation.schema-contracts";
const SCHEMA_IR_ARTIFACT_ID: &str = "codefabric.schema.contract-ir";
const GENERATOR_REVISION: &str = "codefabric-schema-contracts-v1";
const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";
const ID_PATTERN: &str = "^(workspace|repository|worktree|context|context-set|snapshot|publication):[0-9a-f]{32}$|^context:source$|^(entity|fact):[a-z0-9-]+:[0-9a-f]{32}$";
const DIGEST_PATTERN: &str = "^(b3|blake3):[0-9a-f]{64}$";

fn failure(path: &Path, message: impl Into<String>) -> ContractArtifactError {
    ContractArtifactError::Fixture {
        path: path.to_owned(),
        message: message.into(),
    }
}

#[allow(clippy::needless_pass_by_value)] // Schema nodes move into one parent exactly once.
fn strict_object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
        "required": required,
    })
}

fn string() -> Value {
    json!({"type": "string"})
}

fn id() -> Value {
    json!({"type": "string", "pattern": ID_PATTERN})
}

fn digest() -> Value {
    json!({"type": "string", "pattern": DIGEST_PATTERN})
}

fn nonnegative() -> Value {
    json!({"type": "integer", "minimum": 0})
}

fn string_array() -> Value {
    json!({"type": "array", "items": {"type": "string"}, "uniqueItems": true})
}

fn enum_string(values: &[&str]) -> Value {
    json!({"type": "string", "enum": values})
}

fn public_snapshot_schema() -> Value {
    strict_object(
        json!({
            "snapshot_id": id(), "workspace_id": id(), "repository_id": {"anyOf":[id(), {"type":"null"}]},
            "worktree_id": {"anyOf":[id(), {"type":"null"}]}, "source_generation": nonnegative(),
            "source_inventory_digest": digest(), "durable_base_publication": id(),
            "base_table_version_digest": digest(), "overlay_generation": nonnegative(), "overlay_checksum": digest(),
            "analysis_context_set_id": id(), "analysis_context_ids": {"type":"array","items":id(),"uniqueItems":true},
            "freshness_state": enum_string(&["CURRENT","POTENTIALLY_STALE","UNAVAILABLE"]),
            "source_trust_state": string(), "event_stream_health": string(), "git_acceleration_status": string(),
            "git_operation_summary": {"type":["object","null"]}, "pending_update_count": nonnegative(),
            "ontology_version": string(), "schema_bundle_version": string(), "provider_bundle_version": string(),
            "derivation_bundle_version": string(), "query_language_version": string(),
            "capability_summaries": {"type":"array","items":{"type":"object"}},
            "diagnostic_references": string_array()
        }),
        &[
            "snapshot_id",
            "workspace_id",
            "repository_id",
            "worktree_id",
            "source_generation",
            "source_inventory_digest",
            "durable_base_publication",
            "base_table_version_digest",
            "overlay_generation",
            "overlay_checksum",
            "analysis_context_set_id",
            "analysis_context_ids",
            "freshness_state",
            "source_trust_state",
            "event_stream_health",
            "git_acceleration_status",
            "git_operation_summary",
            "pending_update_count",
            "ontology_version",
            "schema_bundle_version",
            "provider_bundle_version",
            "derivation_bundle_version",
            "query_language_version",
            "capability_summaries",
            "diagnostic_references",
        ],
    )
}

fn analysis_context_schema() -> Value {
    strict_object(
        json!({
            "workspace_id": id(), "analysis_context_id": id(),
            "context_kind": enum_string(&["source","python","rust"]), "context_fingerprint": digest(),
            "provider_bundle_version": string(), "compiler_or_language_version": string(),
            "configuration_manifest_uri": {"type":["string","null"],"format":"uri-reference"}, "active":{"type":"boolean"}
        }),
        &[
            "workspace_id",
            "analysis_context_id",
            "context_kind",
            "context_fingerprint",
            "provider_bundle_version",
            "compiler_or_language_version",
            "configuration_manifest_uri",
            "active",
        ],
    )
}

fn source_context_schema() -> Value {
    let source_reference = strict_object(
        json!({
            "workspace_id": id(), "source_file_id": id(),
            "path": strict_object(json!({"display":string(),"bytes_base64":{"type":["string","null"],"contentEncoding":"base64"},"encoding_code":string(),"display_is_lossy":{"type":"boolean"}}), &["display","bytes_base64","encoding_code","display_is_lossy"]),
            "source_digest": digest(), "start_byte": nonnegative(), "end_byte": nonnegative()
        }),
        &[
            "workspace_id",
            "source_file_id",
            "path",
            "source_digest",
            "start_byte",
            "end_byte",
        ],
    );
    let text = strict_object(
        json!({"kind":{"const":"text"},"text":string(),"encoding":{"const":"UTF-8"},"newline_kind":string(),"lossless":{"const":true}}),
        &["kind", "text", "encoding", "newline_kind", "lossless"],
    );
    let bytes = strict_object(
        json!({"kind":{"const":"bytes"},"base64":{"type":"string","contentEncoding":"base64"},"encoding":string(),"newline_kind":string(),"text_unavailable_reason":string()}),
        &[
            "kind",
            "base64",
            "encoding",
            "newline_kind",
            "text_unavailable_reason",
        ],
    );
    strict_object(
        json!({"context_id":id(),"source_reference":source_reference,"context_kind":string(),"content":{"oneOf":[text,bytes]},"associated_entity_ids":{"type":"array","items":id()},"associated_fact_ids":{"type":"array","items":id()},"syntax_outline":{"type":["object","null"]}}),
        &[
            "context_id",
            "source_reference",
            "context_kind",
            "content",
            "associated_entity_ids",
            "associated_fact_ids",
            "syntax_outline",
        ],
    )
}

fn public_status_schema() -> Value {
    strict_object(
        json!({"ready":{"type":"boolean"},"workspace_id":id(),"agent_instance_id":string(),"snapshot":{"anyOf":[public_snapshot_schema(),{"type":"null"}]},"versions":{"type":"object","additionalProperties":{"type":"string"}},"supported_languages":string_array(),"supported_request_forms":string_array(),"capability_statuses":{"type":"array","items":{"type":"object"}},"freshness_state":enum_string(&["CURRENT","POTENTIALLY_STALE","UNAVAILABLE"]),"service_limits":{"type":"object"},"notices":string_array()}),
        &[
            "ready",
            "workspace_id",
            "agent_instance_id",
            "snapshot",
            "versions",
            "supported_languages",
            "supported_request_forms",
            "capability_statuses",
            "freshness_state",
            "service_limits",
            "notices",
        ],
    )
}

fn request_schema() -> Value {
    let forms = [
        "find code entities",
        "retrieve facts",
        "follow relationships",
        "find paths",
        "match a code fact pattern",
        "combine result sets",
        "summarize facts",
        "fetch source context",
    ];
    let variants = forms.iter().map(|form| strict_object(json!({"query_id":{"type":"string","minLength":1},"request":{"const":form},"label":{"type":["string","null"]},"input":{"type":["object","null"]},"where":{"type":["object","null"]},"limit":{"type":["object","null"]}}), &["query_id","request","label","input","where","limit"])).collect::<Vec<_>>();
    strict_object(
        json!({"specification":{"const":"composable semantic CPG fact query"},"version":{"const":"1.3"},"semantic_request_id":string(),"workspace_id":id(),"freshness_policy":enum_string(&["current_required","wait_for_current","best_available_snapshot"]),"queries":{"type":"array","minItems":1,"items":{"oneOf":variants}},"response_projection":{"type":["object","null"]},"cost_budget":{"type":["object","null"]}}),
        &[
            "specification",
            "version",
            "semantic_request_id",
            "workspace_id",
            "freshness_policy",
            "queries",
            "response_projection",
            "cost_budget",
        ],
    )
}

fn canonical_error_schema() -> Value {
    strict_object(
        json!({"code":string(),"layer":string(),"retryable":{"type":"boolean"},"safe_message":string(),"field":{"type":["string","null"]},"semantic_phrase":{"type":["string","null"]},"candidate_interpretations":string_array(),"failed_dependency_query_id":{"type":["string","null"]},"diagnostic_id":{"anyOf":[id(),{"type":"null"}]}}),
        &[
            "code",
            "layer",
            "retryable",
            "safe_message",
            "field",
            "semantic_phrase",
            "candidate_interpretations",
            "failed_dependency_query_id",
            "diagnostic_id",
        ],
    )
}

fn response_schema() -> Value {
    let dictionary = json!({"type":"object","propertyNames":{"pattern":"^(entity|fact|path|group|context):"},"additionalProperties":{"type":"object"}});
    let query_result = strict_object(
        json!({"query_id":string(),"request":string(),"execution_state":string(),"availability_state":string(),"completeness_state":string(),"freshness_state":string(),"limit_state":string(),"dependency_state":string(),"resolved_semantics":{"type":"object"},"entity_ids":{"type":"array","items":id()},"fact_ids":{"type":"array","items":id()},"path_ids":{"type":"array","items":id()},"group_ids":{"type":"array","items":id()},"source_context_ids":{"type":"array","items":id()},"coverage":{"type":"object"},"errors":{"type":"array","items":canonical_error_schema()},"notices":string_array()}),
        &[
            "query_id",
            "request",
            "execution_state",
            "availability_state",
            "completeness_state",
            "freshness_state",
            "limit_state",
            "dependency_state",
            "resolved_semantics",
            "entity_ids",
            "fact_ids",
            "path_ids",
            "group_ids",
            "source_context_ids",
            "coverage",
            "errors",
            "notices",
        ],
    );
    strict_object(
        json!({"specification":{"const":"composable semantic CPG fact query response"},"version":{"const":"1.3"},"semantic_request_id":string(),"execution_state":string(),"availability_state":string(),"completeness_state":string(),"freshness_state":string(),"limit_state":string(),"successful_query_count":nonnegative(),"failed_query_count":nonnegative(),"not_executed_dependency_count":nonnegative(),"snapshot":public_snapshot_schema(),"entities":dictionary,"facts":dictionary,"paths":dictionary,"groups":dictionary,"source_contexts":dictionary,"query_results":{"type":"array","items":query_result},"errors":{"type":"array","items":canonical_error_schema()}}),
        &[
            "specification",
            "version",
            "semantic_request_id",
            "execution_state",
            "availability_state",
            "completeness_state",
            "freshness_state",
            "limit_state",
            "successful_query_count",
            "failed_query_count",
            "not_executed_dependency_count",
            "snapshot",
            "entities",
            "facts",
            "paths",
            "groups",
            "source_contexts",
            "query_results",
            "errors",
        ],
    )
}

fn planspec_schema() -> Value {
    let node_kinds = [
        "find-entities",
        "retrieve-facts",
        "follow-relationships",
        "find-paths",
        "match-pattern",
        "combine-sets",
        "summarize-facts",
        "fetch-source-context",
    ];
    let node_variants = node_kinds.iter().map(|kind| strict_object(json!({"node_kind":{"const":kind},"query_id":string(),"label":{"type":["string","null"]},"inputs":{"type":"array","items":string()},"ontology_ids":string_array(),"conditions":{"type":"array","items":{"type":"object"}},"context_partition_policy":string(),"certainty_filters":string_array(),"coverage_requirement":{"type":"object"},"soft_limits":{"type":"object"},"requested_output_roles":string_array(),"canonical_ordering":{"type":"array","items":string()}}), &["node_kind","query_id","label","inputs","ontology_ids","conditions","context_partition_policy","certainty_filters","coverage_requirement","soft_limits","requested_output_roles","canonical_ordering"])).collect::<Vec<_>>();
    strict_object(
        json!({"plan_spec_version":{"const":"1.0"},"binding_state":enum_string(&["unbound","snapshot-bound"]),"bound_snapshot_id":{"anyOf":[id(),{"type":"null"}]},"semantic_request_id":string(),"workspace_id":id(),"snapshot_requirement":{"type":"object"},"context_selection":{"type":"object"},"authorization_scope":{"type":"object"},"source_boundary":{"type":"object"},"queries":{"type":"array","minItems":1,"items":{"oneOf":node_variants}},"deterministic_ordering":{"type":"array","items":string()},"response_projection":{"type":"object"},"cost_budget":{"type":"object"},"canonical_digest":digest()}),
        &[
            "plan_spec_version",
            "binding_state",
            "bound_snapshot_id",
            "semantic_request_id",
            "workspace_id",
            "snapshot_requirement",
            "context_selection",
            "authorization_scope",
            "source_boundary",
            "queries",
            "deterministic_ordering",
            "response_projection",
            "cost_budget",
            "canonical_digest",
        ],
    )
}

#[allow(clippy::too_many_lines)] // Keeping AC-G-19's one closed shape contiguous aids review.
fn serving_snapshot_schema() -> Value {
    let source = strict_object(
        json!({"source_generation":nonnegative(),"admitted_event_sequence":nonnegative(),"reconciled_event_sequence":nonnegative(),"inventory_digest":digest(),"authorization_fingerprint":digest(),"inclusion_policy_fingerprint":digest(),"path_profile_version":string(),"source_trust_state":enum_string(&["CURRENT","POTENTIALLY_STALE","UNAVAILABLE"]),"event_stream_health":enum_string(&["HEALTHY","RESCAN_REQUIRED","DEGRADED","UNAVAILABLE"]),"git_acceleration_status":string(),"git_state_fingerprint":{"anyOf":[digest(),{"type":"null"}]}}),
        &[
            "source_generation",
            "admitted_event_sequence",
            "reconciled_event_sequence",
            "inventory_digest",
            "authorization_fingerprint",
            "inclusion_policy_fingerprint",
            "path_profile_version",
            "source_trust_state",
            "event_stream_health",
            "git_acceleration_status",
            "git_state_fingerprint",
        ],
    );
    let context_record = strict_object(
        json!({"analysis_context_id":id(),"context_manifest_digest":digest(),"capability_partition_digest":digest()}),
        &[
            "analysis_context_id",
            "context_manifest_digest",
            "capability_partition_digest",
        ],
    );
    let contexts = strict_object(
        json!({"context_set_id":id(),"default_python_context_id":{"anyOf":[id(),{"type":"null"}]},"default_rust_context_id":{"anyOf":[id(),{"type":"null"}]},"records":{"type":"array","items":context_record}}),
        &[
            "context_set_id",
            "default_python_context_id",
            "default_rust_context_id",
            "records",
        ],
    );
    let base_table = strict_object(
        json!({"table_code":{"type":"integer"},"table_uri":{"type":"string","format":"uri-reference"},"delta_version":{"type":"integer"},"schema_digest":digest(),"row_count":nonnegative(),"primary_key_digest":digest(),"effective_content_digest":digest()}),
        &[
            "table_code",
            "table_uri",
            "delta_version",
            "schema_digest",
            "row_count",
            "primary_key_digest",
            "effective_content_digest",
        ],
    );
    let base_publication = strict_object(
        json!({"publication_id":id(),"tables":{"type":"array","items":base_table}}),
        &["publication_id", "tables"],
    );
    let overlay_table = strict_object(
        json!({"table_code":{"type":"integer"},"mutation_policy":string(),"replacement_row_count":nonnegative(),"owner_tombstone_count":nonnegative(),"key_tombstone_count":nonnegative(),"table_replacement":{"type":"boolean"},"row_digest":digest(),"tombstone_digest":digest()}),
        &[
            "table_code",
            "mutation_policy",
            "replacement_row_count",
            "owner_tombstone_count",
            "key_tombstone_count",
            "table_replacement",
            "row_digest",
            "tombstone_digest",
        ],
    );
    let overlay = strict_object(
        json!({"overlay_generation":nonnegative(),"overlay_digest":digest(),"total_memory_bytes":nonnegative(),"tables":{"type":"array","items":overlay_table}}),
        &[
            "overlay_generation",
            "overlay_digest",
            "total_memory_bytes",
            "tables",
        ],
    );
    let indexes = strict_object(
        json!({"capability_index_digest":digest(),"diagnostic_index_digest":digest(),"dependency_graph_digest":digest()}),
        &[
            "capability_index_digest",
            "diagnostic_index_digest",
            "dependency_graph_digest",
        ],
    );
    let bundles = strict_object(
        json!({"ontology_bundle_id":string(),"schema_bundle_id":string(),"provider_bundle_id":string(),"derivation_bundle_id":string(),"query_language_bundle_id":string(),"model_pack_bundle_id":string(),"toolchain_bundle_id":string()}),
        &[
            "ontology_bundle_id",
            "schema_bundle_id",
            "provider_bundle_id",
            "derivation_bundle_id",
            "query_language_bundle_id",
            "model_pack_bundle_id",
            "toolchain_bundle_id",
        ],
    );
    strict_object(
        json!({"manifest_version":{"const":"1.0"},"snapshot_id":id(),"workspace_id":id(),"repository_id":{"anyOf":[id(),{"type":"null"}]},"worktree_id":{"anyOf":[id(),{"type":"null"}]},"registration_revision":nonnegative(),"source":source,"contexts":contexts,"base_publication":base_publication,"overlay":overlay,"indexes":indexes,"bundles":bundles,"limits_profile_digest":digest(),"manifest_digest":digest()}),
        &[
            "manifest_version",
            "snapshot_id",
            "workspace_id",
            "repository_id",
            "worktree_id",
            "registration_revision",
            "source",
            "contexts",
            "base_publication",
            "overlay",
            "indexes",
            "bundles",
            "limits_profile_digest",
            "manifest_digest",
        ],
    )
}

fn public_body(kind: PublicSchemaKind) -> Value {
    match kind {
        PublicSchemaKind::AnalysisContext => analysis_context_schema(),
        PublicSchemaKind::ServingSnapshot => serving_snapshot_schema(),
        PublicSchemaKind::PublicSnapshotMetadata => public_snapshot_schema(),
        PublicSchemaKind::SourceContext => source_context_schema(),
        PublicSchemaKind::PublicStatus => public_status_schema(),
        PublicSchemaKind::CpgSemanticQueryRequest => request_schema(),
        PublicSchemaKind::CpgSemanticQueryResponse => response_schema(),
        PublicSchemaKind::PlanSpec => planspec_schema(),
    }
}

fn render_public_schema(
    contract: &PublicSchemaContract,
    source_canonical_digest: &str,
    source_digest: &str,
) -> Result<Vec<u8>, ContractArtifactError> {
    let mut schema = public_body(contract.schema_kind);
    let object = schema
        .as_object_mut()
        .expect("schema builders return objects");
    object.insert("$schema".into(), Value::String(DIALECT.into()));
    object.insert(
        "$id".into(),
        Value::String(format!(
            "https://codefabric.dev/{}",
            contract.path.display()
        )),
    );
    object.insert("title".into(), Value::String(contract.title.clone()));
    if contract.schema_kind == PublicSchemaKind::ServingSnapshot {
        object.insert(
            "x-codefabric-cbef-field-order".into(),
            json!([
                "manifest_version",
                "workspace_id",
                "repository_id",
                "worktree_id",
                "registration_revision",
                "source",
                "contexts",
                "base_publication",
                "overlay",
                "indexes",
                "bundles",
                "limits_profile_digest"
            ]),
        );
        object.insert(
            "x-codefabric-identity".into(),
            json!({
                "manifest_digest": "BLAKE3-256(codefabric-serving-snapshot-manifest-v1 || CBEF-v1(body))",
                "snapshot_id": "BLAKE3-128(CBEF-v1(SERVING_SNAPSHOT, manifest_digest))",
                "excluded_from_body": ["snapshot_id", "manifest_digest"]
            }),
        );
    }
    object.insert("x-codefabric-generated".into(), json!({"generator_revision":GENERATOR_REVISION,"source_artifact_id":SCHEMA_IR_ARTIFACT_ID,"source_canonical_digest":source_canonical_digest,"source_digest":source_digest}));
    object.insert("x-codefabric-artifact".into(), json!({"artifact_id":contract.artifact_id,"artifact_kind":"json-schema","version":"1.0","compatible_suite_major":1,"status":"released","canonical_digest":format!("b3:{}", "0".repeat(64)),"generator_revision":GENERATOR_REVISION}));
    let mut projection = schema.clone();
    projection["x-codefabric-artifact"]
        .as_object_mut()
        .expect("generated header is an object")
        .remove("canonical_digest");
    let digest = checksum(&canonicalize_value(&projection).map_err(|source| {
        ContractArtifactError::Canonical {
            path: contract.path.clone(),
            source,
        }
    })?);
    schema["x-codefabric-artifact"]["canonical_digest"] = Value::String(digest);
    let mut bytes = serde_json::to_vec_pretty(&schema)
        .map_err(|error| failure(&contract.path, error.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn rust_logical_type(logical: LogicalType) -> &'static str {
    match logical {
        LogicalType::Id16 => "Id16",
        LogicalType::Hash32 => "Hash32",
        LogicalType::Code16 => "Code16",
        LogicalType::Code32 => "Code32",
        LogicalType::Bucket16 => "Bucket16",
        LogicalType::Int16 => "Int16",
        LogicalType::Int32 => "Int32",
        LogicalType::Int64 => "Int64",
        LogicalType::Float64 => "Float64",
        LogicalType::Boolean => "Boolean",
        LogicalType::Utf8 => "Utf8",
        LogicalType::Binary => "Binary",
        LogicalType::TimestampUtc => "TimestampUtc",
        LogicalType::IdList => "IdList",
        LogicalType::StringMap => "StringMap",
    }
}

fn rust_strings(values: &[String]) -> String {
    format!(
        "&[{}]",
        values
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_rust(ir: &SchemaContractIr, source_digest: &str) -> Vec<u8> {
    let mut output = format!(
        "// @generated from {SCHEMA_IR_ARTIFACT_ID} {source_digest}; {GENERATOR_REVISION}; do not edit.\n\nconst GENERATED_TABLE_SPECS: &[GeneratedTableSpec] = &[\n"
    );
    for table in &ir.tables {
        writeln!(output, "    GeneratedTableSpec {{").unwrap();
        writeln!(
            output,
            "        table_code: {}, name: {:?}, family: {:?}, grain: {:?}, schema_version: {:?},",
            table.table_code, table.name, table.family, table.grain, table.schema_version
        )
        .unwrap();
        writeln!(output, "        columns: &[").unwrap();
        for column in &table.columns {
            writeln!(output, "            GeneratedColumn {{ name: {:?}, logical_type: LogicalType::{}, nullable: {}, semantic_type: {:?}, foreign_key: {:?}, hidden_operational: {} }},", column.name, rust_logical_type(column.logical_type), column.nullable, column.semantic_type.as_deref(), column.foreign_key.as_deref(), column.hidden_operational).unwrap();
        }
        writeln!(output, "        ],").unwrap();
        writeln!(
            output,
            "        primary_key: {}, partition_columns: {}, zorder_columns: {},",
            rust_strings(&table.primary_key),
            rust_strings(&table.partition_columns),
            rust_strings(&table.zorder_columns)
        )
        .unwrap();
        writeln!(output, "        durable_mutation: DurableMutationClass::{:?}, overlay_mutation: OverlayMutationPolicy::{:?}, materialization_role: MaterializationRole::{:?},", table.durable_mutation, table.overlay_mutation, table.materialization_role).unwrap();
        writeln!(
            output,
            "        dependencies: &{:?}, required_for_publication: {},",
            table.dependencies, table.required_for_publication
        )
        .unwrap();
        writeln!(output, "    }},").unwrap();
    }
    output.push_str("];\n");
    output.into_bytes()
}

fn format_rust(path: &Path, source: &[u8]) -> Result<Vec<u8>, ContractArtifactError> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| failure(path, format!("cannot start rustfmt: {error}")))?;
    child
        .stdin
        .take()
        .expect("piped rustfmt stdin is present")
        .write_all(source)
        .map_err(|error| failure(path, format!("cannot write rustfmt input: {error}")))?;
    let output = child
        .wait_with_output()
        .map_err(|error| failure(path, format!("cannot wait for rustfmt: {error}")))?;
    if !output.status.success() {
        return Err(failure(
            path,
            format!(
                "rustfmt rejected generated source: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }
    Ok(output.stdout)
}

fn render_ddl(ir: &SchemaContractIr, source_digest: &str) -> Vec<u8> {
    let mut output = format!(
        "-- @generated from {SCHEMA_IR_ARTIFACT_ID} {source_digest}; {GENERATOR_REVISION}; do not edit.\nPRAGMA foreign_keys = ON;\n\n"
    );
    for table in &ir.operational_tables {
        writeln!(output, "CREATE TABLE {} (", table.name).unwrap();
        let mut definitions = table
            .columns
            .iter()
            .map(|column| {
                format!(
                    "  {} {}{}",
                    column.name,
                    match column.sqlite_type {
                        SqliteType::Integer => "INTEGER",
                        SqliteType::Real => "REAL",
                        SqliteType::Text => "TEXT",
                        SqliteType::Blob => "BLOB",
                    },
                    if column.nullable { "" } else { " NOT NULL" }
                )
            })
            .collect::<Vec<_>>();
        definitions.push(format!("  PRIMARY KEY ({})", table.primary_key.join(", ")));
        definitions.extend(
            table
                .unique
                .iter()
                .map(|columns| format!("  UNIQUE ({})", columns.join(", "))),
        );
        writeln!(output, "{}\n) STRICT;\n", definitions.join(",\n")).unwrap();
    }
    output.push_str("CREATE VIEW workspace_update_state AS\nSELECT workspace_id, lifecycle_state_code, source_generation, event_watermark, newest_dirty_generation, durable_generation, reconcile_required, updated_at\nFROM worktree_state;\n");
    output.into_bytes()
}

fn render_manifest(
    ir: &SchemaContractIr,
    source_canonical_digest: &str,
    source_digest: &str,
) -> Result<Vec<u8>, ContractArtifactError> {
    canonicalize_value(&json!({"_generated":{"generator_revision":GENERATOR_REVISION,"profile":"codefabric-schema-contract-v1","source_artifact_id":SCHEMA_IR_ARTIFACT_ID,"source_canonical_digest":source_canonical_digest,"source_digest":source_digest},"owner_bucket_count":ir.owner_bucket_count,"tables":ir.tables,"operational_tables":ir.operational_tables,"public_schemas":ir.public_schemas})).map_err(|source| ContractArtifactError::Canonical { path: PathBuf::from("contracts/schema/arrow-delta/table-specs.json"), source })
}

pub(super) fn render_schema_outputs(
    repository_root: &Path,
    catalog: &CompiledCatalog,
) -> Result<BTreeMap<PathBuf, Vec<u8>>, ContractArtifactError> {
    let artifact = catalog.artifact(SCHEMA_IR_ARTIFACT_ID).ok_or_else(|| {
        failure(
            Path::new(SCHEMA_IR_ARTIFACT_ID),
            "schema Contract IR is not cataloged",
        )
    })?;
    let path = repository_root.join(&artifact.authority_path);
    let raw = fs::read(&path).map_err(|source| ContractArtifactError::Io {
        path: path.clone(),
        source,
    })?;
    let value = decode_strict(&raw).map_err(|source| ContractArtifactError::Canonical {
        path: path.clone(),
        source,
    })?;
    let ir: SchemaContractIr =
        serde_json::from_value(value).map_err(|error| failure(&path, error.to_string()))?;
    ir.validate().map_err(|message| failure(&path, message))?;
    let identity = compile_artifact_for_generation(repository_root, catalog, artifact)?;
    let mut rendered = BTreeMap::new();
    let manifest_path = catalog
        .outputs_of_kind(
            SCHEMA_DERIVATION_ID,
            DerivationOutputKind::TableSpecManifest,
        )
        .next()
        .ok_or_else(|| failure(&path, "TableSpec manifest output is absent"))?
        .0
        .to_owned();
    rendered.insert(
        manifest_path,
        render_manifest(&ir, &identity.canonical_digest, &identity.source_digest)?,
    );
    let rust_path = catalog
        .outputs_of_kind(
            SCHEMA_DERIVATION_ID,
            DerivationOutputKind::RustTableSpecBindings,
        )
        .next()
        .ok_or_else(|| failure(&path, "Rust TableSpec output is absent"))?
        .0
        .to_owned();
    rendered.insert(
        rust_path.clone(),
        format_rust(&rust_path, &render_rust(&ir, &identity.canonical_digest))?,
    );
    let ddl_path = catalog
        .outputs_of_kind(
            SCHEMA_DERIVATION_ID,
            DerivationOutputKind::OperationalStoreDdl,
        )
        .next()
        .ok_or_else(|| failure(&path, "operational DDL output is absent"))?
        .0
        .to_owned();
    rendered.insert(ddl_path, render_ddl(&ir, &identity.canonical_digest));
    let expected = catalog
        .outputs_of_kind(SCHEMA_DERIVATION_ID, DerivationOutputKind::PublicJsonSchema)
        .map(|(output, _)| output.to_owned())
        .collect::<BTreeSet<_>>();
    let declared = ir
        .public_schemas
        .iter()
        .map(|schema| schema.path.clone())
        .collect::<BTreeSet<_>>();
    if expected != declared {
        return Err(failure(&path, "public-schema IR/output census differs"));
    }
    for schema in &ir.public_schemas {
        rendered.insert(
            schema.path.clone(),
            render_public_schema(schema, &identity.canonical_digest, &identity.source_digest)?,
        );
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::catalog::ContractCatalog;

    const ROOT: &str = env!("CARGO_MANIFEST_DIR");

    #[test]
    fn wp09_operational_acceptance() {
        let catalog = ContractCatalog::load(Path::new(ROOT)).unwrap();
        let first = render_schema_outputs(Path::new(ROOT), &catalog).unwrap();
        let second = render_schema_outputs(Path::new(ROOT), &catalog).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.len(), 11);
        let ddl = std::str::from_utf8(
            first
                .get(Path::new("contracts/schema/operational-store.sql"))
                .unwrap(),
        )
        .unwrap();
        for table in [
            "common_repository_state",
            "worktree_state",
            "git_state_vector",
            "update_wave",
            "provider_run",
            "hot_overlay_manifest",
            "snapshot_lease",
            "result_artifact_lease",
            "serving_snapshot_manifest",
            "active_snapshot",
        ] {
            assert!(ddl.contains(&format!("CREATE TABLE {table}")));
        }
        assert!(ddl.contains("CREATE VIEW workspace_update_state"));
    }
}
