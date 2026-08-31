//! Short-lived `RUSTC_WORKSPACE_WRAPPER` client for one Cargo compilation unit.

use std::collections::HashMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arrow_array::builder::{BooleanBuilder, FixedSizeBinaryBuilder, StringBuilder, UInt64Builder};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_ipc::MetadataVersion;
use arrow_ipc::writer::{IpcWriteOptions, StreamWriter};
use arrow_schema::DataType;
use hyper_util::rt::TokioIo;
use prost::Message;
use tokio::net::UnixStream;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Endpoint;
use tower::service_fn;

use crate::protocol::generated::codefabric::provider::v1::{CapabilityOutcome, ProviderRunState};
use crate::protocol::generated::codefabric::rustc::v1::extraction_event::Event;
use crate::protocol::generated::codefabric::rustc::v1::extractor_command::Command;
use crate::protocol::generated::codefabric::rustc::v1::rustc_extractor_client::RustcExtractorClient;
use crate::protocol::generated::codefabric::rustc::v1::{
    CompilationBegin, CompilationEnd, CompilerOwnerKey, DiagnosticSummary, ExtractionEvent,
    ExtractorHello, OwnerBegin, OwnerEnd, OwnerObservationChunk, OwnerRelationIpcFrame,
    PackageTargetIdentity, RejectionRuleErrorCode,
};
use crate::protocol::generated::registries::{CAPABILITY_CODES, CAPABILITY_IDS};
use crate::relation_ipc_contract::relation_wire_identity;
use crate::relation_ipc_proto::{
    RelationCoverage, decode_flow_control_ack, encode_relation_frames,
};
use crate::rustc_link::{OwnedCell, OwnedRow, OwnedRustcOwner, OwnedRustcRelation};
use crate::rustc_relation_schema::{RustcRelation, schema_bundle_digest};

include!("generated/digest_frames.rs");

const MAX_RELATION_IPC_BYTES: usize = 16 * 1024 * 1024;
const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

/// Typed failure at the compiler-wrapper process boundary.
#[derive(Debug)]
pub(crate) struct WrapperError {
    phase: &'static str,
    detail: String,
}

impl WrapperError {
    fn protocol(detail: impl Into<String>) -> Self {
        Self {
            phase: "execution",
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for WrapperError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "INTERNAL:{}:{}", self.phase, self.detail)
    }
}

impl std::error::Error for WrapperError {}

impl From<String> for WrapperError {
    fn from(detail: String) -> Self {
        Self::protocol(detail)
    }
}

fn rust_mir_capability_code() -> Result<u32, String> {
    CAPABILITY_IDS
        .iter()
        .zip(CAPABILITY_CODES)
        .find_map(|(candidate, code)| (*candidate == "RUST_MIR").then_some(u32::from(*code)))
        .ok_or_else(|| "generated RUST_MIR capability allocation is absent".to_owned())
}

#[derive(Clone, Debug)]
struct WrapperEnvironment {
    endpoint: PathBuf,
    provider_run_id: String,
    workspace_id: String,
    analysis_context_id: String,
    source_generation: u64,
    context_manifest_digest: String,
    resource_profile_id: String,
    source_snapshot_manifest_digest: String,
    cargo_metadata_digest: String,
    cargo_lock_digest: String,
    cargo_config_digest: String,
}

#[derive(Debug)]
enum MonitorMessage {
    Command(Command),
    Transport(String),
    Closed,
}

#[derive(Debug, serde::Deserialize)]
struct ToolchainIdentity {
    extractor: String,
    rustc_release: String,
    rustc_commit_hash: String,
}

fn b3(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 67
        && value.starts_with("b3:")
        && value[3..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_sandbox_profile_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .or_else(|| value.strip_prefix("b3:"))
        .is_some_and(|payload| {
            payload.len() == 64
                && payload
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

fn environment_value(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("required wrapper environment is absent: {name}"))
}

impl WrapperEnvironment {
    fn resolve() -> Result<Option<Self>, String> {
        let Some(endpoint) = env::var_os("CODEFABRIC_EXTRACTOR_ENDPOINT") else {
            return Ok(None);
        };
        let endpoint = endpoint
            .to_str()
            .ok_or_else(|| "extractor endpoint is not UTF-8".to_owned())?
            .strip_prefix("unix://")
            .ok_or_else(|| "extractor endpoint must use unix://".to_owned())?;
        if endpoint.is_empty() {
            return Err("extractor endpoint path is empty".to_owned());
        }
        let source_generation = environment_value("CODEFABRIC_SOURCE_GENERATION")?
            .parse::<u64>()
            .map_err(|_| "source generation is not an unsigned integer".to_owned())?;
        let resolved = Self {
            endpoint: PathBuf::from(endpoint),
            provider_run_id: environment_value("CODEFABRIC_PROVIDER_RUN_ID")?,
            workspace_id: environment_value("CODEFABRIC_WORKSPACE_ID")?,
            analysis_context_id: environment_value("CODEFABRIC_ANALYSIS_CONTEXT_ID")?,
            source_generation,
            context_manifest_digest: environment_value("CODEFABRIC_CONTEXT_MANIFEST_DIGEST")?,
            resource_profile_id: environment_value("CODEFABRIC_PROVIDER_RESOURCE_PROFILE_ID")?,
            source_snapshot_manifest_digest: environment_value(
                "CODEFABRIC_SOURCE_SNAPSHOT_MANIFEST_DIGEST",
            )?,
            cargo_metadata_digest: environment_value("CODEFABRIC_CARGO_METADATA_DIGEST")?,
            cargo_lock_digest: environment_value("CODEFABRIC_CARGO_LOCK_DIGEST")?,
            cargo_config_digest: environment_value("CODEFABRIC_CARGO_CONFIG_DIGEST")?,
        };
        for digest in [
            &resolved.context_manifest_digest,
            &resolved.source_snapshot_manifest_digest,
            &resolved.cargo_metadata_digest,
            &resolved.cargo_lock_digest,
            &resolved.cargo_config_digest,
        ] {
            if !valid_digest(digest) {
                return Err("wrapper environment contains a malformed digest".to_owned());
            }
        }
        Ok(Some(resolved))
    }
}

fn passthrough(real_rustc: &OsStr, arguments: &[OsString]) -> Result<i32, String> {
    let status = ProcessCommand::new(real_rustc)
        .args(arguments)
        .status()
        .map_err(|error| format!("failed to execute real rustc: {error}"))?;
    Ok(status.code().unwrap_or(1))
}

fn argument_value(arguments: &[String], name: &str) -> Option<String> {
    let equals = format!("{name}=");
    arguments.iter().enumerate().find_map(|(index, argument)| {
        argument
            .strip_prefix(&equals)
            .map(str::to_owned)
            .or_else(|| {
                (argument == name)
                    .then(|| arguments.get(index + 1).cloned())
                    .flatten()
            })
    })
}

fn source_path(arguments: &[String]) -> Option<PathBuf> {
    arguments
        .iter()
        .find(|argument| {
            !argument.starts_with('-')
                && (std::path::Path::new(argument)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
                    || std::path::Path::new(argument).is_file())
        })
        .map(PathBuf::from)
}

fn normalized_invocation_digest(
    real_rustc: &OsStr,
    arguments: &[OsString],
    source: &Path,
    source_bytes: &[u8],
) -> String {
    let mut normalized = Vec::with_capacity(arguments.len() + 1);
    normalized.push(
        Path::new(real_rustc)
            .file_name()
            .unwrap_or(real_rustc)
            .as_bytes()
            .to_vec(),
    );
    let mut skip_next = false;
    for argument in arguments {
        if skip_next {
            skip_next = false;
            continue;
        }
        let bytes = argument.as_bytes();
        if bytes == b"--out-dir" || bytes == b"-o" {
            skip_next = true;
            continue;
        }
        if bytes.starts_with(b"--out-dir=") {
            continue;
        }
        if Path::new(argument) == source {
            normalized.push(format!("source-content:{}", b3(source_bytes)).into_bytes());
            continue;
        }
        normalized.push(bytes.to_vec());
    }
    let fields = normalized;
    digest_frames(b"codefabric.rustc.invocation.v1\0", fields)
}

fn short_identity(domain: &[u8], fields: &[&str]) -> String {
    let digest = digest_frames(domain, fields.iter().map(|field| field.as_bytes().to_vec()));
    digest[3..35].to_owned()
}

fn fixed16_hex(value: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(32);
    for byte in value {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn target_identity(arguments: &[String], invocation_digest: &str) -> PackageTargetIdentity {
    let crate_name =
        argument_value(arguments, "--crate-name").unwrap_or_else(|| "unknown_crate".to_owned());
    let crate_type = argument_value(arguments, "--crate-type").unwrap_or_else(|| "lib".to_owned());
    let package_name = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| crate_name.clone());
    let package_id = format!(
        "pkg:{}",
        short_identity(
            b"codefabric.rustc.package.v1\0",
            &[&package_name, invocation_digest]
        )
    );
    PackageTargetIdentity {
        package_id,
        package_name,
        target_name: crate_name.clone(),
        target_kind: crate_type.clone(),
        crate_name,
        crate_type,
        crate_disambiguator: invocation_digest[3..19].to_owned(),
    }
}

struct RelationContext<'a> {
    provider_run_id: &'a str,
    compilation_unit_id: &'a str,
    owner_id: &'a str,
    source_generation: u64,
    source_file_id: &'a str,
    source_content_digest: [u8; 32],
}

fn relation_cell(
    row: &OwnedRow,
    owner: &OwnedRustcOwner,
    context: &RelationContext<'_>,
    field: &str,
) -> Option<OwnedCell> {
    match field {
        "provider_run_id" => Some(OwnedCell::Utf8(context.provider_run_id.to_owned())),
        "compilation_unit_id" => Some(OwnedCell::Utf8(context.compilation_unit_id.to_owned())),
        "owner_id" => Some(OwnedCell::Utf8(context.owner_id.to_owned())),
        "source_generation" => Some(OwnedCell::UInt64(context.source_generation)),
        "source_file_id" => Some(OwnedCell::Utf8(context.source_file_id.to_owned())),
        "source_content_digest" => Some(OwnedCell::Fixed32(context.source_content_digest)),
        "stable_crate_id" => owner
            .compiler_key
            .map(|key| OwnedCell::UInt64(key.stable_crate_id)),
        "def_path_hash" => owner
            .compiler_key
            .map(|key| OwnedCell::Fixed16(key.def_path_hash)),
        _ => row.0.get(field).cloned(),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed scalar set is checked against each Arrow physical type in one encoder"
)]
fn encode_relation(
    owner: &OwnedRustcOwner,
    relation: &OwnedRustcRelation,
    context: &RelationContext<'_>,
) -> Result<Vec<u8>, String> {
    if RustcRelation::from_family_code(relation.relation.family_code()) != Some(relation.relation) {
        return Err("rustc relation is outside the shared family registry".to_owned());
    }
    let schema = relation.relation.schema();
    let mut columns = Vec::<ArrayRef>::with_capacity(schema.fields().len());
    for field in schema.fields() {
        let values = relation
            .rows
            .iter()
            .map(|row| relation_cell(row, owner, context, field.name()))
            .collect::<Vec<_>>();
        let array: ArrayRef = match field.data_type() {
            DataType::Utf8 => {
                let mut builder = StringBuilder::with_capacity(values.len(), values.len() * 16);
                for value in values {
                    match value {
                        Some(OwnedCell::Utf8(value)) => builder.append_value(value),
                        None if field.is_nullable() => builder.append_null(),
                        _ => {
                            return Err(format!(
                                "{} field {} differs from utf8/nullability contract",
                                relation.relation.relation_id(),
                                field.name()
                            ));
                        }
                    }
                }
                Arc::new(builder.finish())
            }
            DataType::UInt64 => {
                let mut builder = UInt64Builder::with_capacity(values.len());
                for value in values {
                    match value {
                        Some(OwnedCell::UInt64(value)) => builder.append_value(value),
                        None if field.is_nullable() => builder.append_null(),
                        _ => {
                            return Err(format!(
                                "{} field {} differs from uint64/nullability contract",
                                relation.relation.relation_id(),
                                field.name()
                            ));
                        }
                    }
                }
                Arc::new(builder.finish())
            }
            DataType::Boolean => {
                let mut builder = BooleanBuilder::with_capacity(values.len());
                for value in values {
                    match value {
                        Some(OwnedCell::Boolean(value)) => builder.append_value(value),
                        None if field.is_nullable() => builder.append_null(),
                        _ => {
                            return Err(format!(
                                "{} field {} differs from boolean/nullability contract",
                                relation.relation.relation_id(),
                                field.name()
                            ));
                        }
                    }
                }
                Arc::new(builder.finish())
            }
            DataType::FixedSizeBinary(width @ (16 | 32)) => {
                let mut builder = FixedSizeBinaryBuilder::with_capacity(values.len(), *width);
                for value in values {
                    match value {
                        Some(OwnedCell::Fixed16(value)) if *width == 16 => builder
                            .append_value(value)
                            .map_err(|error| error.to_string())?,
                        Some(OwnedCell::Fixed32(value)) if *width == 32 => builder
                            .append_value(value)
                            .map_err(|error| error.to_string())?,
                        None if field.is_nullable() => builder.append_null(),
                        _ => {
                            return Err(format!(
                                "{} field {} differs from fixed-binary/nullability contract",
                                relation.relation.relation_id(),
                                field.name()
                            ));
                        }
                    }
                }
                Arc::new(builder.finish())
            }
            _ => {
                return Err(format!(
                    "{} uses unsupported Arrow field {}",
                    relation.relation.relation_id(),
                    field.name()
                ));
            }
        };
        columns.push(array);
    }
    let batch = RecordBatch::try_new(schema.clone(), columns)
        .map_err(|error| format!("failed to build rustc relation batch: {error}"))?;
    let mut bytes = Vec::new();
    {
        let options = IpcWriteOptions::try_new(64, false, MetadataVersion::V5)
            .map_err(|error| format!("failed to configure rustc Arrow IPC V5 writer: {error}"))?;
        let mut writer = StreamWriter::try_new_with_options(&mut bytes, &schema, options)
            .map_err(|error| format!("failed to create rustc IPC writer: {error}"))?;
        writer
            .write(&batch)
            .and_then(|()| writer.finish())
            .map_err(|error| format!("failed to encode rustc Arrow IPC: {error}"))?;
    }
    if bytes.len() > MAX_RELATION_IPC_BYTES {
        return Err("rustc Arrow IPC chunk exceeds the protocol credit limit".to_owned());
    }
    Ok(bytes)
}

fn owner_content_digest(begin: &OwnerBegin, chunks: &[OwnerObservationChunk]) -> String {
    let fields =
        std::iter::once(begin.encode_to_vec()).chain(chunks.iter().map(Message::encode_to_vec));
    digest_frames(b"codefabric.rustc.owner-content.v1\0", fields)
}

fn closed_owner_set_digest(owners: &[(OwnerBegin, OwnerEnd)]) -> String {
    let mut fields = owners
        .iter()
        .map(|(begin, end)| {
            let mut bytes = begin
                .owner
                .as_ref()
                .map_or_else(Vec::new, |owner| owner.owner_id.as_bytes().to_vec());
            bytes.extend_from_slice(end.owner_content_digest.as_bytes());
            bytes
        })
        .collect::<Vec<_>>();
    fields.sort();
    digest_frames(b"codefabric.rustc.closed-owner-set.v1\0", fields)
}

fn canonical_event_bytes(event: &ExtractionEvent) -> Vec<u8> {
    let mut normalized = event.clone();
    let mut family_counts = Vec::new();
    match normalized.event.as_mut() {
        Some(Event::OwnerEnd(end)) => {
            family_counts = end
                .family_counts
                .iter()
                .map(|(family, count)| (*family, *count))
                .collect::<Vec<_>>();
            family_counts.sort_unstable();
            end.family_counts.clear();
        }
        Some(Event::CompilationEnd(end)) => end.overall_stream_digest.clear(),
        _ => {}
    }
    let fields = std::iter::once(normalized.encode_to_vec()).chain(family_counts.into_iter().map(
        |(family, count)| {
            let mut bytes = family.to_be_bytes().to_vec();
            bytes.extend_from_slice(&count.to_be_bytes());
            bytes
        },
    ));
    digest_frames(b"codefabric.rustc.canonical-event.v1\0", fields).into_bytes()
}

fn overall_stream_digest(events: &[ExtractionEvent]) -> String {
    let fields = events.iter().map(canonical_event_bytes).collect::<Vec<_>>();
    digest_frames(b"codefabric.rustc.observation-stream.v1\0", fields)
}

fn deadline_timeout(deadline_unix_ms: i64) -> Duration {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        });
    Duration::from_millis(u64::try_from(deadline_unix_ms.saturating_sub(now)).unwrap_or(0))
}

fn receive_command(
    commands: &Receiver<MonitorMessage>,
    deadline_unix_ms: i64,
) -> Result<Command, String> {
    match commands.recv_timeout(deadline_timeout(deadline_unix_ms)) {
        Ok(MonitorMessage::Command(command)) => Ok(command),
        Ok(MonitorMessage::Transport(error)) => Err(error),
        Ok(MonitorMessage::Closed) => Err("daemon closed the compiler command stream".to_owned()),
        Err(RecvTimeoutError::Timeout) => Err("compiler provider deadline elapsed".to_owned()),
        Err(RecvTimeoutError::Disconnected) => {
            Err("compiler command monitor disconnected".to_owned())
        }
    }
}

fn receive_terminal_close(
    commands: &Receiver<MonitorMessage>,
    deadline_unix_ms: i64,
) -> Result<(), String> {
    match commands.recv_timeout(deadline_timeout(deadline_unix_ms)) {
        Ok(MonitorMessage::Closed) => Ok(()),
        Ok(MonitorMessage::Transport(error)) => Err(error),
        Ok(MonitorMessage::Command(_)) => {
            Err("daemon sent a command after compiler terminal event".to_owned())
        }
        Err(RecvTimeoutError::Timeout) => {
            Err("daemon did not close the accepted compiler stream before deadline".to_owned())
        }
        Err(RecvTimeoutError::Disconnected) => {
            Err("compiler command monitor disconnected before terminal close".to_owned())
        }
    }
}

fn send_event(
    sender: &mpsc::Sender<ExtractionEvent>,
    event: Event,
    events: &mut Vec<ExtractionEvent>,
) -> Result<(), String> {
    let event = ExtractionEvent { event: Some(event) };
    sender
        .blocking_send(event.clone())
        .map_err(|_| "daemon event stream is closed".to_owned())?;
    events.push(event);
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn run_protocol(
    runtime: &Runtime,
    environment: WrapperEnvironment,
    real_rustc: &OsStr,
    arguments: &[OsString],
    identity_bytes: &[u8],
) -> Result<i32, String> {
    let identity: ToolchainIdentity = serde_json::from_slice(identity_bytes)
        .map_err(|error| format!("toolchain identity is invalid: {error}"))?;
    let argument_strings = arguments
        .iter()
        .map(|argument| {
            argument
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| "rustc analysis arguments must be UTF-8".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source = source_path(&argument_strings)
        .ok_or_else(|| "analysis invocation has no Rust source input".to_owned())?;
    let source_bytes = std::fs::read(&source)
        .map_err(|error| format!("failed to read compiler source input: {error}"))?;
    let invocation_digest =
        normalized_invocation_digest(real_rustc, arguments, &source, &source_bytes);
    let rust_mir_capability_code = rust_mir_capability_code()?;
    let target = target_identity(&argument_strings, &invocation_digest);
    let compilation_unit_id = format!(
        "unit:{}",
        short_identity(
            b"codefabric.rustc.compilation-unit.v1\0",
            &[
                &environment.workspace_id,
                &environment.analysis_context_id,
                &target.package_id,
                &target.target_name,
                &target.crate_type,
                &invocation_digest,
            ]
        )
    );
    let begin = CompilationBegin {
        provider_run_id: environment.provider_run_id.clone(),
        compilation_unit_id: compilation_unit_id.clone(),
        workspace_id: environment.workspace_id.clone(),
        analysis_context_id: environment.analysis_context_id.clone(),
        source_generation: environment.source_generation,
        target: Some(target),
        rustc_version: identity.rustc_release.clone(),
        rustc_commit: identity.rustc_commit_hash.clone(),
        normalized_rustc_invocation_digest: invocation_digest,
        cargo_metadata_digest: environment.cargo_metadata_digest.clone(),
        cargo_lock_digest: environment.cargo_lock_digest.clone(),
        cargo_config_digest: environment.cargo_config_digest.clone(),
        build_script_output_digests: Vec::new(),
        proc_macro_output_digests: Vec::new(),
        source_snapshot_manifest_digest: environment.source_snapshot_manifest_digest.clone(),
        requested_capability_codes: vec![rust_mir_capability_code],
        context_manifest_digest: environment.context_manifest_digest.clone(),
        resource_profile_id: environment.resource_profile_id.clone(),
        toolchain_identity_digest: b3(identity_bytes),
    };

    let socket = environment.endpoint.clone();
    let (event_sender, event_receiver) = mpsc::channel(4);
    let (monitor_sender, monitor_receiver) = std::sync::mpsc::channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_monitor = Arc::clone(&cancelled);
    let deadline = runtime.block_on(async move {
        let channel = Endpoint::from_static("http://[::]:50051")
            .connect_with_connector(service_fn(move |_| {
                let socket = socket.clone();
                async move { UnixStream::connect(socket).await.map(TokioIo::new) }
            }))
            .await
            .map_err(|error| format!("failed to connect extractor endpoint: {error}"))?;
        let mut client = RustcExtractorClient::new(channel)
            .max_decoding_message_size(4 * 1024 * 1024)
            .max_encoding_message_size(MAX_FRAME_BYTES);
        let hello = ExtractorHello {
            protocol_major: 1,
            protocol_minor: 0,
            required_feature_bits: 0,
            optional_feature_bits: 0,
            extractor_build: identity.extractor.clone(),
            rustc_version: identity.rustc_release.clone(),
            rustc_commit: identity.rustc_commit_hash.clone(),
            toolchain_identity_digest: b3(identity_bytes),
            resource_profile_id: environment.resource_profile_id.clone(),
        };
        let acknowledgement = client
            .handshake(hello)
            .await
            .map_err(|error| format!("extractor handshake failed: {error}"))?
            .into_inner();
        if acknowledgement.protocol_major != 1
            || acknowledgement.protocol_minor != 0
            || acknowledgement.output_schema_bundle_digest != schema_bundle_digest()
            || !valid_sandbox_profile_digest(&acknowledgement.sandbox_profile_digest)
            || acknowledgement.accepted_resource_profile_id != environment.resource_profile_id
            || acknowledgement.maximum_outstanding_chunks != 4
            || acknowledgement.maximum_unacknowledged_bytes != MAX_RELATION_IPC_BYTES as u64
        {
            return Err(
                "daemon handshake acknowledgement is outside the accepted profile".to_owned(),
            );
        }
        let mut commands = client
            .observe(ReceiverStream::new(event_receiver))
            .await
            .map_err(|error| format!("failed to open compiler observation stream: {error}"))?
            .into_inner();
        tokio::spawn(async move {
            loop {
                match commands.message().await {
                    Ok(Some(command)) => {
                        let Some(command) = command.command else {
                            let _ = monitor_sender.send(MonitorMessage::Transport(
                                "daemon sent an empty extractor command".to_owned(),
                            ));
                            break;
                        };
                        if matches!(command, Command::Cancel(_)) {
                            cancelled_monitor.store(true, Ordering::Release);
                        }
                        if monitor_sender
                            .send(MonitorMessage::Command(command))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(None) => {
                        let _ = monitor_sender.send(MonitorMessage::Closed);
                        break;
                    }
                    Err(error) => {
                        let _ = monitor_sender.send(MonitorMessage::Transport(format!(
                            "compiler command stream failed: {error}"
                        )));
                        break;
                    }
                }
            }
        });
        Ok::<i64, String>(acknowledgement.provider_deadline_unix_ms)
    })?;

    let mut events = Vec::new();
    send_event(
        &event_sender,
        Event::CompilationBegin(begin.clone()),
        &mut events,
    )?;
    match receive_command(&monitor_receiver, deadline)? {
        Command::CompilationAccepted(accepted)
            if accepted.provider_run_id == environment.provider_run_id
                && accepted.compilation_unit_id == compilation_unit_id
                && accepted.accepted_generation == environment.source_generation
                && accepted.granted_chunk_credits == 4
                && accepted.granted_credit_bytes == MAX_RELATION_IPC_BYTES as u64 => {}
        Command::Cancel(_) => cancelled.store(true, Ordering::Release),
        _ => return Err("daemon did not accept the compilation begin".to_owned()),
    }

    let mut rustc_arguments = Vec::with_capacity(argument_strings.len() + 1);
    rustc_arguments.push(real_rustc.to_string_lossy().into_owned());
    rustc_arguments.extend(argument_strings);
    let extracted = crate::rustc_link::extract_owned(&rustc_arguments);
    let compiler_exit_status = i32::from(extracted.is_err());
    let owners = extracted.map_or_else(|_| Vec::new(), |extraction| extraction.owners);
    let mut sequence = 1_u64;
    let mut closed_owners = Vec::new();
    let source_file_id = format!("file:{}", &b3(&source_bytes)[3..35]);
    let source_content_digest = *blake3::hash(&source_bytes).as_bytes();
    if !cancelled.load(Ordering::Acquire) && compiler_exit_status == 0 {
        for owner in &owners {
            let stable_identity = owner.compiler_key.map_or_else(
                || owner.qualified_name.clone(),
                |key| {
                    format!(
                        "{:016x}:{}",
                        key.stable_crate_id,
                        fixed16_hex(key.def_path_hash)
                    )
                },
            );
            let owner_id = format!(
                "owner:{}",
                short_identity(
                    b"codefabric.rustc.owner.v1\0",
                    &[&compilation_unit_id, &stable_identity]
                )
            );
            let expected_observation_family_codes = owner
                .relations
                .iter()
                .map(|relation| relation.relation.family_code())
                .collect::<Vec<_>>();
            let owner_begin = OwnerBegin {
                provider_run_id: environment.provider_run_id.clone(),
                compilation_unit_id: compilation_unit_id.clone(),
                sequence,
                owner: Some(CompilerOwnerKey {
                    owner_id: owner_id.clone(),
                    owner_kind: owner.owner_kind.clone(),
                    file_id: source_file_id.clone(),
                    source_start: 0,
                    source_end: u32::try_from(source_bytes.len()).unwrap_or(u32::MAX),
                }),
                expected_observation_family_codes,
            };
            send_event(
                &event_sender,
                Event::OwnerBegin(owner_begin.clone()),
                &mut events,
            )?;
            sequence += 1;
            let context = RelationContext {
                provider_run_id: &environment.provider_run_id,
                compilation_unit_id: &compilation_unit_id,
                owner_id: &owner_id,
                source_generation: environment.source_generation,
                source_file_id: &source_file_id,
                source_content_digest,
            };
            let mut chunks = Vec::with_capacity(owner.relations.len());
            let mut family_counts = HashMap::new();
            for relation in &owner.relations {
                let arrow_ipc = encode_relation(owner, relation, &context)?;
                let row_count = u64::try_from(relation.rows.len()).unwrap_or(u64::MAX);
                let logical_sequence = sequence;
                let chunk = OwnerObservationChunk {
                    provider_run_id: environment.provider_run_id.clone(),
                    compilation_unit_id: compilation_unit_id.clone(),
                    sequence: logical_sequence,
                    owner_id: owner_id.clone(),
                    observation_family_code: relation.relation.family_code(),
                    chunk_digest: b3(&arrow_ipc),
                    arrow_ipc: arrow_ipc.clone(),
                    payload_reference: None,
                    schema_digest: relation.relation.schema_digest(),
                    row_count,
                };
                let relation_identity = relation_wire_identity(
                    relation.relation.relation_id(),
                    &relation.relation.schema_digest(),
                    &environment.provider_run_id,
                    &owner_id,
                    &environment.source_snapshot_manifest_digest,
                    &environment.context_manifest_digest,
                )?;
                let frames = encode_relation_frames(
                    relation_identity,
                    &arrow_ipc,
                    1,
                    row_count,
                    &RelationCoverage::complete(1),
                )?;
                let mut next_ack_sequence = 0_u64;
                for frame in frames {
                    let payload = match frame.frame.as_ref() {
                        Some(
                            crate::relation_ipc_proto_types::relation_ipc_frame::Frame::Payload(
                                payload,
                            ),
                        ) => Some((
                            payload
                                .header
                                .as_ref()
                                .map_or(u64::MAX, |header| header.sequence),
                            u64::try_from(payload.arrow_ipc_fragment.len()).unwrap_or(u64::MAX),
                        )),
                        _ => None,
                    };
                    send_event(
                        &event_sender,
                        Event::OwnerRelationIpcFrame(OwnerRelationIpcFrame {
                            provider_run_id: environment.provider_run_id.clone(),
                            compilation_unit_id: compilation_unit_id.clone(),
                            sequence,
                            owner_id: owner_id.clone(),
                            observation_family_code: relation.relation.family_code(),
                            frame: Some(frame),
                        }),
                        &mut events,
                    )?;
                    sequence = sequence
                        .checked_add(1)
                        .ok_or_else(|| "compiler event sequence space is exhausted".to_owned())?;
                    if let Some((payload_sequence, payload_bytes)) = payload {
                        match receive_command(&monitor_receiver, deadline)? {
                            Command::RelationIpcAck(frame) => {
                                let acknowledgement = decode_flow_control_ack(&frame)?;
                                if acknowledgement.header.identity != relation_identity
                                    || acknowledgement.header.sequence != next_ack_sequence
                                {
                                    return Err(
                                        "daemon returned a mismatched relation acknowledgement"
                                            .to_owned(),
                                    );
                                }
                                if acknowledgement.cancelled {
                                    cancelled.store(true, Ordering::Release);
                                    break;
                                }
                                if acknowledgement.acknowledged_sequence != Some(payload_sequence)
                                    || acknowledgement.released_bytes != payload_bytes
                                {
                                    return Err(
                                        "daemon returned a mismatched relation acknowledgement"
                                            .to_owned(),
                                    );
                                }
                                next_ack_sequence =
                                    next_ack_sequence.checked_add(1).ok_or_else(|| {
                                        "relation acknowledgement sequence space is exhausted"
                                            .to_owned()
                                    })?;
                            }
                            Command::Cancel(_) => {
                                cancelled.store(true, Ordering::Release);
                                break;
                            }
                            Command::ChunkAccepted(_) | Command::ChunkRejected(_) => {
                                return Err("daemon returned legacy whole-relation chunk control"
                                    .to_owned());
                            }
                            _ => {
                                return Err(
                                    "daemon returned an invalid rustc relation acknowledgement"
                                        .to_owned(),
                                );
                            }
                        }
                    }
                }
                if cancelled.load(Ordering::Acquire) {
                    break;
                }
                family_counts.insert(relation.relation.family_code(), row_count);
                chunks.push(chunk);
            }
            if cancelled.load(Ordering::Acquire) {
                break;
            }
            let owner_end = OwnerEnd {
                provider_run_id: environment.provider_run_id.clone(),
                compilation_unit_id: compilation_unit_id.clone(),
                sequence,
                owner_id,
                family_counts,
                owner_content_digest: owner_content_digest(&owner_begin, &chunks),
            };
            send_event(
                &event_sender,
                Event::OwnerEnd(owner_end.clone()),
                &mut events,
            )?;
            closed_owners.push((owner_begin, owner_end));
            sequence += 1;
        }
    }

    let was_cancelled = cancelled.load(Ordering::Acquire);
    let terminal_state = if was_cancelled {
        ProviderRunState::Cancelled
    } else if compiler_exit_status == 0 {
        ProviderRunState::Succeeded
    } else {
        ProviderRunState::Failed
    };
    let mut end = CompilationEnd {
        provider_run_id: environment.provider_run_id,
        compilation_unit_id,
        sequence,
        compiler_exit_status,
        closed_owner_set_digest: closed_owner_set_digest(&closed_owners),
        capability_outcomes: vec![CapabilityOutcome {
            capability_code: rust_mir_capability_code,
            owner_capability_state_code: if compiler_exit_status == 0 && !was_cancelled {
                10
            } else {
                60
            },
            completeness_state_code: if compiler_exit_status == 0 && !was_cancelled {
                10
            } else {
                40
            },
            reason_code: if was_cancelled {
                "CANCELLED".to_owned()
            } else if compiler_exit_status == 0 {
                "COMPILER_RELATION_CENSUS_COMPLETE".to_owned()
            } else {
                "UNAVAILABLE_COMPILE".to_owned()
            },
        }],
        diagnostic_summary: Some(DiagnosticSummary {
            error_count: u32::from(compiler_exit_status != 0),
            warning_count: 0,
            diagnostics_digest: b3(b""),
        }),
        overall_stream_digest: String::new(),
        terminal_state: terminal_state as i32,
        rejection_error: (terminal_state == ProviderRunState::Failed)
            .then_some(RejectionRuleErrorCode::CompilerFailed as i32),
    };
    let mut digest_events = events.clone();
    digest_events.push(ExtractionEvent {
        event: Some(Event::CompilationEnd(end.clone())),
    });
    end.overall_stream_digest = overall_stream_digest(&digest_events);
    send_event(&event_sender, Event::CompilationEnd(end), &mut events)?;
    drop(event_sender);
    receive_terminal_close(&monitor_receiver, deadline)?;
    Ok(if was_cancelled {
        130
    } else {
        compiler_exit_status
    })
}

/// Run one compiler wrapper invocation, passing through probes when no provider endpoint exists.
pub(crate) fn run(
    real_rustc: &OsStr,
    arguments: &[OsString],
    identity_bytes: &[u8],
) -> Result<i32, WrapperError> {
    let Some(environment) = WrapperEnvironment::resolve()? else {
        return passthrough(real_rustc, arguments).map_err(WrapperError::from);
    };
    if !arguments.iter().any(|argument| {
        !argument.as_bytes().starts_with(b"-")
            && (Path::new(argument)
                .extension()
                .is_some_and(|extension| extension == "rs")
                || Path::new(argument).is_file())
    }) {
        return passthrough(real_rustc, arguments).map_err(WrapperError::from);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create extractor runtime: {error}"))?;
    run_protocol(&runtime, environment, real_rustc, arguments, identity_bytes)
        .map_err(WrapperError::from)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Cursor;
    use std::pin::Pin;
    use std::sync::Mutex as StdMutex;

    use arrow_ipc::reader::StreamReader;
    use tokio::sync::oneshot;
    use tokio_stream::Stream;
    use tokio_stream::wrappers::{ReceiverStream, UnixListenerStream};
    use tonic::transport::Server;
    use tonic::{Request, Response, Status};

    use crate::protocol::generated::codefabric::rustc::v1::ExtractorCommand;
    use crate::protocol::generated::codefabric::rustc::v1::rustc_extractor_server::{
        RustcExtractor, RustcExtractorServer,
    };
    use crate::rustc_relation_schema::RustcRelation;

    use super::*;

    #[test]
    fn normalized_invocation_excludes_operational_output_paths() {
        let first = vec![
            OsString::from("source.rs"),
            OsString::from("--crate-name=fixture"),
            OsString::from("--out-dir=/tmp/first"),
        ];
        let second = vec![
            OsString::from("source.rs"),
            OsString::from("--crate-name=fixture"),
            OsString::from("--out-dir"),
            OsString::from("/tmp/second"),
        ];
        assert_eq!(
            normalized_invocation_digest(
                OsStr::new("/first/toolchain/rustc"),
                &first,
                Path::new("source.rs"),
                b"source",
            ),
            normalized_invocation_digest(
                OsStr::new("/second/toolchain/rustc"),
                &second,
                Path::new("source.rs"),
                b"source",
            )
        );
    }

    type MockCommandStream =
        Pin<Box<dyn Stream<Item = Result<ExtractorCommand, Status>> + Send + 'static>>;

    #[derive(Clone, Debug)]
    struct MockDaemon {
        events: Arc<StdMutex<Vec<ExtractionEvent>>>,
        deadline_unix_ms: i64,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct DecodedRelation {
        family_code: u32,
        schema_fingerprint: [u8; 32],
        row_count: u64,
        arrow_ipc: Vec<u8>,
    }

    struct OpenRelation {
        decoded: DecodedRelation,
        identity: crate::relation_ipc_contract::RelationWireIdentity,
        next_sequence: u64,
        saw_ipc_end: bool,
        saw_coverage: bool,
    }

    fn decode_relation_events(events: &[ExtractionEvent]) -> Vec<DecodedRelation> {
        use crate::relation_ipc_proto_types::RelationIpcTerminalStatus;
        use crate::relation_ipc_proto_types::relation_ipc_frame::Frame;

        let mut relations = Vec::new();
        let mut open: Option<OpenRelation> = None;
        for event in events {
            let Some(Event::OwnerRelationIpcFrame(event)) = event.event.as_ref() else {
                continue;
            };
            let frame = event.frame.as_ref().expect("relation frame is present");
            match frame
                .frame
                .as_ref()
                .expect("relation frame variant is present")
            {
                Frame::Open(frame) => {
                    assert!(open.is_none());
                    crate::relation_ipc_proto::validate_open_profile(frame).unwrap();
                    let header =
                        crate::relation_ipc_proto::parse_header(frame.header.as_ref()).unwrap();
                    assert_eq!(header.sequence, 0);
                    open = Some(OpenRelation {
                        decoded: DecodedRelation {
                            family_code: event.observation_family_code,
                            schema_fingerprint: header.identity.schema_fingerprint,
                            row_count: 0,
                            arrow_ipc: Vec::new(),
                        },
                        identity: header.identity,
                        next_sequence: 1,
                        saw_ipc_end: false,
                        saw_coverage: false,
                    });
                }
                Frame::Payload(frame) => {
                    let header =
                        crate::relation_ipc_proto::parse_header(frame.header.as_ref()).unwrap();
                    let relation = open.as_mut().expect("payload follows open");
                    assert_eq!(event.observation_family_code, relation.decoded.family_code);
                    assert_eq!(header.identity, relation.identity);
                    assert_eq!(header.sequence, relation.next_sequence);
                    relation.next_sequence += 1;
                    relation
                        .decoded
                        .arrow_ipc
                        .extend_from_slice(&frame.arrow_ipc_fragment);
                }
                Frame::IpcEnd(frame) => {
                    let header =
                        crate::relation_ipc_proto::parse_header(frame.header.as_ref()).unwrap();
                    let relation = open.as_mut().expect("IPC end follows open");
                    assert_eq!(header.identity, relation.identity);
                    assert_eq!(header.sequence, relation.next_sequence);
                    relation.next_sequence += 1;
                    assert_eq!(frame.declared_batches, 1);
                    assert_eq!(
                        frame.declared_ipc_bytes,
                        u64::try_from(relation.decoded.arrow_ipc.len()).unwrap()
                    );
                    relation.decoded.row_count = frame.declared_rows;
                    relation.saw_ipc_end = true;
                }
                Frame::CoverageTrailer(frame) => {
                    let header =
                        crate::relation_ipc_proto::parse_header(frame.header.as_ref()).unwrap();
                    let relation = open.as_mut().expect("coverage follows open");
                    assert_eq!(header.identity, relation.identity);
                    assert_eq!(header.sequence, relation.next_sequence);
                    relation.next_sequence += 1;
                    assert!(relation.saw_ipc_end);
                    assert_eq!(frame.status, RelationIpcTerminalStatus::Complete as i32);
                    assert_eq!(frame.requested_units, 1);
                    assert_eq!(frame.completed_units, 1);
                    assert!(frame.remainders.is_empty());
                    relation.saw_coverage = true;
                }
                Frame::Terminal(frame) => {
                    let header =
                        crate::relation_ipc_proto::parse_header(frame.header.as_ref()).unwrap();
                    let relation = open.as_ref().expect("terminal follows open");
                    assert_eq!(header.identity, relation.identity);
                    assert_eq!(header.sequence, relation.next_sequence);
                    assert!(relation.saw_coverage);
                    assert_eq!(frame.status, RelationIpcTerminalStatus::Complete as i32);
                    relations.push(open.take().unwrap().decoded);
                }
                Frame::FlowControlAck(_) => {
                    panic!("extractor emitted a receiver-direction acknowledgement")
                }
            }
        }
        assert!(open.is_none());
        relations
    }

    #[tonic::async_trait]
    impl RustcExtractor for MockDaemon {
        async fn handshake(
            &self,
            request: Request<ExtractorHello>,
        ) -> Result<
            Response<crate::protocol::generated::codefabric::rustc::v1::ExtractorHelloAck>,
            Status,
        > {
            let hello = request.into_inner();
            if hello.protocol_major != 1 || !valid_digest(&hello.toolchain_identity_digest) {
                return Err(Status::failed_precondition("unexpected extractor hello"));
            }
            Ok(Response::new(
                crate::protocol::generated::codefabric::rustc::v1::ExtractorHelloAck {
                    protocol_major: 1,
                    protocol_minor: 0,
                    negotiated_feature_bits: 0,
                    daemon_build: "codefabricd-test".to_owned(),
                    output_schema_bundle_digest: schema_bundle_digest(),
                    sandbox_profile_digest: b3(b"sandbox"),
                    maximum_outstanding_chunks: 4,
                    maximum_unacknowledged_bytes: MAX_RELATION_IPC_BYTES as u64,
                    accepted_resource_profile_id: hello.resource_profile_id,
                    provider_deadline_unix_ms: self.deadline_unix_ms,
                },
            ))
        }

        type ObserveStream = MockCommandStream;

        async fn observe(
            &self,
            request: Request<tonic::Streaming<ExtractionEvent>>,
        ) -> Result<Response<Self::ObserveStream>, Status> {
            let mut input = request.into_inner();
            let events = Arc::clone(&self.events);
            let (sender, receiver) = mpsc::channel(8);
            tokio::spawn(async move {
                let mut next_ack_sequence = BTreeMap::<Vec<u8>, u64>::new();
                while let Ok(Some(event)) = input.message().await {
                    events.lock().unwrap().push(event.clone());
                    match event.event {
                        Some(Event::CompilationBegin(begin)) => {
                            let _ = sender
                                .send(Ok(ExtractorCommand {
                                    command: Some(Command::CompilationAccepted(
                                        crate::protocol::generated::codefabric::rustc::v1::CompilationAccepted {
                                            provider_run_id: begin.provider_run_id,
                                            compilation_unit_id: begin.compilation_unit_id,
                                            accepted_generation: begin.source_generation,
                                            granted_chunk_credits: 4,
                                            granted_credit_bytes: MAX_RELATION_IPC_BYTES as u64,
                                        },
                                    )),
                                }))
                                .await;
                        }
                        Some(Event::OwnerRelationIpcFrame(event)) => {
                            if let Some(frame) = event.frame
                                && let Some(
                                    crate::relation_ipc_proto_types::relation_ipc_frame::Frame::Payload(
                                        payload,
                                    ),
                                ) = frame.frame
                            {
                                let parsed = crate::relation_ipc_proto::parse_header(
                                    payload.header.as_ref(),
                                )
                                .unwrap();
                                let acknowledgement_sequence = next_ack_sequence
                                    .entry(parsed.identity.stream_id.to_vec())
                                    .or_default();
                                let acknowledgement =
                                    crate::relation_ipc_proto::flow_control_ack_frame(
                                        parsed.identity,
                                        *acknowledgement_sequence,
                                        Some(parsed.sequence),
                                        u64::try_from(payload.arrow_ipc_fragment.len())
                                            .unwrap_or(u64::MAX),
                                        false,
                                    )
                                    .unwrap();
                                *acknowledgement_sequence += 1;
                                let _ = sender
                                    .send(Ok(ExtractorCommand {
                                        command: Some(Command::RelationIpcAck(acknowledgement)),
                                    }))
                                    .await;
                            }
                        }
                        Some(Event::CompilationEnd(_)) => break,
                        _ => {}
                    }
                }
            });
            Ok(Response::new(Box::pin(ReceiverStream::new(receiver))))
        }
    }

    #[test]
    fn wp35_structural_acceptance() {
        let owner = OwnedRustcOwner {
            qualified_name: "fixture".to_owned(),
            owner_kind: "COMPILATION".to_owned(),
            compiler_key: None,
            relations: Vec::new(),
        };
        let relation = OwnedRustcRelation {
            relation: RustcRelation::Compilation,
            rows: vec![OwnedRow(BTreeMap::from([
                ("crate_name", OwnedCell::Utf8("fixture".to_owned())),
                ("is_local_crate", OwnedCell::Boolean(true)),
                ("local_item_count", OwnedCell::UInt64(3)),
                ("body_owner_count", OwnedCell::UInt64(1)),
                (
                    "rustc_release",
                    OwnedCell::Utf8(crate::rustc_relation_schema::RUSTC_PUBLIC_RELEASE.to_owned()),
                ),
                (
                    "rustc_toolchain",
                    OwnedCell::Utf8(crate::rustc_relation_schema::RUSTC_TOOLCHAIN.to_owned()),
                ),
                (
                    "stable_identity_authority",
                    OwnedCell::Utf8("StableCrateId+DefPathHash".to_owned()),
                ),
                (
                    "source_hygiene_authority",
                    OwnedCell::Utf8("rustc Span".to_owned()),
                ),
            ]))],
        };
        let context = RelationContext {
            provider_run_id: "run:test",
            compilation_unit_id: "unit:test",
            owner_id: "owner:test",
            source_generation: 7,
            source_file_id: "file:test",
            source_content_digest: [9; 32],
        };
        let first = encode_relation(&owner, &relation, &context).unwrap();
        let second = encode_relation(&owner, &relation, &context).unwrap();
        assert_eq!(first, second);
        let mut reader = StreamReader::try_new(Cursor::new(&first), None).unwrap();
        assert_eq!(reader.schema(), RustcRelation::Compilation.schema());
        assert_eq!(reader.next().unwrap().unwrap().num_rows(), 1);
        assert!(reader.next().is_none());
    }

    #[test]
    fn zero_fact_relation_is_a_schema_carrying_arrow_batch() {
        let owner = OwnedRustcOwner {
            qualified_name: "fixture::caller".to_owned(),
            owner_kind: "MIR_BODY".to_owned(),
            compiler_key: Some(crate::rustc_link::OwnedCompilerKey {
                stable_crate_id: 11,
                def_path_hash: [12; 16],
            }),
            relations: Vec::new(),
        };
        let relation = OwnedRustcRelation {
            relation: RustcRelation::Call,
            rows: Vec::new(),
        };
        let context = RelationContext {
            provider_run_id: "run:test",
            compilation_unit_id: "unit:test",
            owner_id: "owner:test",
            source_generation: 7,
            source_file_id: "file:test",
            source_content_digest: [9; 32],
        };

        let bytes = encode_relation(&owner, &relation, &context).unwrap();
        let mut reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
        assert_eq!(reader.schema(), RustcRelation::Call.schema());
        assert_eq!(reader.next().unwrap().unwrap().num_rows(), 0);
        assert!(reader.next().is_none());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn wp35_operational_acceptance() {
        let temporary = tempfile::tempdir().unwrap();
        let socket = temporary.path().join("rustc.sock");
        let events = Arc::new(StdMutex::new(Vec::new()));
        let deadline_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
            })
            + 60_000;
        let daemon = MockDaemon {
            events: Arc::clone(&events),
            deadline_unix_ms,
        };
        let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        let server_socket = socket.clone();
        let server = std::thread::spawn(move || {
            let runtime = Runtime::new().unwrap();
            runtime.block_on(async move {
                let listener = tokio::net::UnixListener::bind(&server_socket).unwrap();
                ready_sender.send(()).unwrap();
                Server::builder()
                    .add_service(RustcExtractorServer::new(daemon))
                    .serve_with_incoming_shutdown(UnixListenerStream::new(listener), async {
                        let _ = shutdown_receiver.await;
                    })
                    .await
                    .unwrap();
            });
        });
        ready_receiver.recv().unwrap();

        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/golden/codefabric-golden-v1/workspace/rust/src/lib.rs");
        let output = temporary.path().join("output");
        std::fs::create_dir_all(&output).unwrap();
        let sysroot = ProcessCommand::new("rustup")
            .args(["run", "nightly-2026-08-18", "rustc", "--print", "sysroot"])
            .output()
            .unwrap();
        assert!(sysroot.status.success());
        let sysroot = String::from_utf8(sysroot.stdout).unwrap();
        let arguments = vec![
            source.into_os_string(),
            OsString::from("--crate-name=codefabric_wrapper_probe"),
            OsString::from("--crate-type=lib"),
            OsString::from("--edition=2024"),
            OsString::from("--emit=metadata"),
            OsString::from(format!("--out-dir={}", output.display())),
            OsString::from(format!("--sysroot={}", sysroot.trim())),
        ];
        let environment = WrapperEnvironment {
            endpoint: socket,
            provider_run_id: "run:wrapper-probe".to_owned(),
            workspace_id: "workspace:golden".to_owned(),
            analysis_context_id: "context:rust".to_owned(),
            source_generation: 1,
            context_manifest_digest: b3(b"context"),
            resource_profile_id: "compiler-semantic-standard".to_owned(),
            source_snapshot_manifest_digest: b3(b"source-snapshot"),
            cargo_metadata_digest: b3(b"cargo-metadata"),
            cargo_lock_digest: b3(b"cargo-lock"),
            cargo_config_digest: b3(b"cargo-config"),
        };
        let runtime = Runtime::new().unwrap();
        let exit = run_protocol(
            &runtime,
            environment.clone(),
            OsStr::new("rustc"),
            &arguments,
            include_bytes!("../toolchain-identity.json"),
        )
        .unwrap();
        assert_eq!(exit, 0);
        let replay_exit = run_protocol(
            &runtime,
            environment,
            OsStr::new("rustc"),
            &arguments,
            include_bytes!("../toolchain-identity.json"),
        )
        .unwrap();
        assert_eq!(replay_exit, 0);

        for _ in 0..100 {
            if events
                .lock()
                .unwrap()
                .iter()
                .filter(|event| matches!(event.event, Some(Event::CompilationEnd(_))))
                .count()
                == 2
                && matches!(
                    events
                        .lock()
                        .unwrap()
                        .last()
                        .and_then(|event| event.event.as_ref()),
                    Some(Event::CompilationEnd(_))
                )
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let observed = events.lock().unwrap().clone();
        assert!(matches!(
            observed.first().and_then(|event| event.event.as_ref()),
            Some(Event::CompilationBegin(_))
        ));
        assert!(matches!(
            observed.last().and_then(|event| event.event.as_ref()),
            Some(Event::CompilationEnd(_))
        ));
        assert!(
            !observed
                .iter()
                .any(|event| matches!(event.event, Some(Event::OwnerObservationChunk(_))))
        );
        let first_end = observed
            .iter()
            .position(|event| matches!(event.event, Some(Event::CompilationEnd(_))))
            .unwrap();
        let first_run = decode_relation_events(&observed[..=first_end]);
        let second_run = decode_relation_events(&observed[first_end + 1..]);
        assert!(first_run.len() > 2);
        assert_eq!(first_run.len(), second_run.len());
        for (first, second) in first_run.iter().zip(&second_run) {
            assert_eq!(first.family_code, second.family_code);
            assert_eq!(first.schema_fingerprint, second.schema_fingerprint);
            assert_eq!(first.row_count, second.row_count);
            assert_eq!(first.arrow_ipc, second.arrow_ipc);

            let relation = RustcRelation::from_family_code(first.family_code).unwrap();
            let schema_digest = relation.schema_digest();
            let expected_schema_fingerprint = relation_wire_identity(
                relation.relation_id(),
                &schema_digest,
                "test-run",
                "test-scope",
                &b3(b"test-source"),
                &b3(b"test-context"),
            )
            .unwrap()
            .schema_fingerprint;
            assert_eq!(first.schema_fingerprint, expected_schema_fingerprint);
            let mut reader = StreamReader::try_new(Cursor::new(&first.arrow_ipc), None).unwrap();
            assert_eq!(reader.schema(), relation.schema());
            let batch = reader.next().unwrap().unwrap();
            assert_eq!(batch.num_rows() as u64, first.row_count);
            assert!(reader.next().is_none());
        }
        let families = first_run
            .iter()
            .map(|relation| relation.family_code)
            .collect::<BTreeSet<_>>();
        for required in [
            RustcRelation::Compilation,
            RustcRelation::PublicItem,
            RustcRelation::Type,
            RustcRelation::MirBody,
            RustcRelation::MirBlock,
            RustcRelation::MirLocal,
            RustcRelation::MirStatement,
            RustcRelation::MirTerminator,
            RustcRelation::CfgEdge,
            RustcRelation::Coverage,
            RustcRelation::Remainder,
        ] {
            assert!(families.contains(&required.family_code()));
        }

        let _ = shutdown_sender.send(());
        server.join().unwrap();
    }
}
