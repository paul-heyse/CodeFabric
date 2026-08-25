//! Short-lived `RUSTC_WORKSPACE_WRAPPER` client for one Cargo compilation unit.

use std::env;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt as _;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arrow_array::builder::{ListBuilder, StringBuilder};
use arrow_array::{ArrayRef, BooleanArray, RecordBatch, StringArray, UInt64Array};
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{DataType, Field, Schema};
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
    ExtractorHello, OwnerBegin, OwnerEnd, OwnerObservationChunk, PackageTargetIdentity,
    RejectionRuleErrorCode,
};
use crate::protocol::generated::observation_schema::{
    PROVIDER_OBSERVATION_SCHEMAS, ProviderObservationLogicalType, ProviderObservationSchema,
};
use crate::protocol::generated::registries::{CAPABILITY_CODES, CAPABILITY_IDS};
use crate::rustc_link::OwnedMirItem;

include!("generated/digest_frames.rs");

const MAX_CHUNK_BYTES: usize = 16 * 1024 * 1024;

fn rust_mir_capability_code() -> Result<u32, String> {
    CAPABILITY_IDS
        .iter()
        .zip(CAPABILITY_CODES)
        .find_map(|(candidate, code)| (*candidate == "RUST_MIR").then_some(u32::from(*code)))
        .ok_or_else(|| "generated RUST_MIR capability allocation is absent".to_owned())
}

fn rust_mir_observation_contract() -> Result<&'static ProviderObservationSchema, String> {
    PROVIDER_OBSERVATION_SCHEMAS
        .iter()
        .find(|schema| schema.provider_id == "rustc-mir")
        .ok_or_else(|| "generated rustc MIR observation schema is absent".to_owned())
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
                && std::path::Path::new(argument)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
        })
        .map(PathBuf::from)
}

fn normalized_invocation_digest(real_rustc: &OsStr, arguments: &[OsString]) -> String {
    let fields = std::iter::once(real_rustc.as_bytes().to_vec()).chain(
        arguments
            .iter()
            .map(|argument| argument.as_bytes().to_vec()),
    );
    digest_frames(b"codefabric.rustc.invocation.v1\0", fields)
}

fn short_identity(domain: &[u8], fields: &[&str]) -> String {
    let digest = digest_frames(domain, fields.iter().map(|field| field.as_bytes().to_vec()));
    digest[3..35].to_owned()
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

fn mir_schema() -> Result<Arc<Schema>, String> {
    let contract = rust_mir_observation_contract()?;
    let fields = contract
        .fields
        .iter()
        .map(|field| {
            let data_type = match field.logical_type {
                ProviderObservationLogicalType::Utf8 => DataType::Utf8,
                ProviderObservationLogicalType::Boolean => DataType::Boolean,
                ProviderObservationLogicalType::UInt64 => DataType::UInt64,
                ProviderObservationLogicalType::Utf8List => {
                    DataType::List(Arc::new(Field::new_list_field(DataType::Utf8, false)))
                }
            };
            Field::new(field.name, data_type, field.nullable)
        })
        .collect::<Vec<_>>();
    Ok(Arc::new(Schema::new(fields)))
}

fn string_list(values: &[String]) -> ArrayRef {
    let mut builder = ListBuilder::new(StringBuilder::new())
        .with_field(Field::new_list_field(DataType::Utf8, false));
    for value in values {
        builder.values().append_value(value);
    }
    builder.append(true);
    Arc::new(builder.finish())
}

fn encode_mir_item(item: &OwnedMirItem) -> Result<Vec<u8>, String> {
    let schema = mir_schema()?;
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec![item.name.as_str()])),
        Arc::new(StringArray::from(vec![item.item_kind.as_str()])),
        Arc::new(StringArray::from(vec![item.type_description.as_str()])),
        Arc::new(BooleanArray::from(vec![item.requires_monomorphization])),
        Arc::new(UInt64Array::from(vec![item.basic_block_count as u64])),
        Arc::new(UInt64Array::from(vec![item.local_count as u64])),
        string_list(&item.statement_kinds),
        string_list(&item.terminator_kinds),
        Arc::new(UInt64Array::from(vec![item.successor_count as u64])),
    ];
    let batch = RecordBatch::try_new(schema.clone(), columns)
        .map_err(|error| format!("failed to build MIR Arrow batch: {error}"))?;
    let mut bytes = Vec::new();
    {
        let mut writer = StreamWriter::try_new(&mut bytes, &schema)
            .map_err(|error| format!("failed to create MIR IPC writer: {error}"))?;
        writer
            .write(&batch)
            .and_then(|()| writer.finish())
            .map_err(|error| format!("failed to encode MIR Arrow IPC: {error}"))?;
    }
    if bytes.len() > MAX_CHUNK_BYTES {
        return Err("MIR Arrow IPC chunk exceeds the protocol credit limit".to_owned());
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

fn overall_stream_digest(events: &[ExtractionEvent]) -> String {
    let fields = events
        .iter()
        .map(|event| {
            let mut normalized = event.clone();
            if let Some(Event::CompilationEnd(end)) = normalized.event.as_mut() {
                end.overall_stream_digest.clear();
            }
            normalized.encode_to_vec()
        })
        .collect::<Vec<_>>();
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
    let invocation_digest = normalized_invocation_digest(real_rustc, arguments);
    let arguments = arguments
        .iter()
        .map(|argument| {
            argument
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| "rustc analysis arguments must be UTF-8".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source = source_path(&arguments)
        .ok_or_else(|| "analysis invocation has no Rust source input".to_owned())?;
    let source_bytes = std::fs::read(&source)
        .map_err(|error| format!("failed to read compiler source input: {error}"))?;
    let rust_mir_capability_code = rust_mir_capability_code()?;
    let rust_mir_observation = rust_mir_observation_contract()?;
    let rust_mir_observation_family_code = u32::from(rust_mir_observation.observation_family_code);
    let target = target_identity(&arguments, &invocation_digest);
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
            .max_encoding_message_size(MAX_CHUNK_BYTES + 1024 * 1024);
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
            || !valid_digest(&acknowledgement.output_schema_bundle_digest)
            || !valid_digest(&acknowledgement.sandbox_profile_digest)
            || acknowledgement.accepted_resource_profile_id != environment.resource_profile_id
            || acknowledgement.maximum_outstanding_chunks == 0
            || acknowledgement.maximum_outstanding_chunks > 4
            || acknowledgement.maximum_unacknowledged_bytes == 0
            || acknowledgement.maximum_unacknowledged_bytes > MAX_CHUNK_BYTES as u64
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
                && accepted.accepted_generation == environment.source_generation => {}
        Command::Cancel(_) => cancelled.store(true, Ordering::Release),
        _ => return Err("daemon did not accept the compilation begin".to_owned()),
    }

    let mut rustc_arguments = Vec::with_capacity(arguments.len() + 1);
    rustc_arguments.push(real_rustc.to_string_lossy().into_owned());
    rustc_arguments.extend(arguments);
    let extracted = crate::rustc_link::extract_owned(&rustc_arguments);
    let compiler_exit_status = i32::from(extracted.is_err());
    let items = extracted.unwrap_or_default();
    let mut sequence = 1_u64;
    let mut closed_owners = Vec::new();
    if !cancelled.load(Ordering::Acquire) && compiler_exit_status == 0 {
        for item in &items {
            let owner_id = format!(
                "owner:{}",
                short_identity(
                    b"codefabric.rustc.owner.v1\0",
                    &[&compilation_unit_id, &item.name]
                )
            );
            let owner_begin = OwnerBegin {
                provider_run_id: environment.provider_run_id.clone(),
                compilation_unit_id: compilation_unit_id.clone(),
                sequence,
                owner: Some(CompilerOwnerKey {
                    owner_id: owner_id.clone(),
                    owner_kind: "MIR_BODY".to_owned(),
                    file_id: format!("file:{}", &b3(&source_bytes)[3..35]),
                    source_start: 0,
                    source_end: u32::try_from(source_bytes.len()).unwrap_or(u32::MAX),
                }),
                expected_observation_family_codes: vec![rust_mir_observation_family_code],
            };
            send_event(
                &event_sender,
                Event::OwnerBegin(owner_begin.clone()),
                &mut events,
            )?;
            sequence += 1;
            let arrow_ipc = encode_mir_item(item)?;
            let chunk = OwnerObservationChunk {
                provider_run_id: environment.provider_run_id.clone(),
                compilation_unit_id: compilation_unit_id.clone(),
                sequence,
                owner_id: owner_id.clone(),
                observation_family_code: rust_mir_observation_family_code,
                chunk_digest: b3(&arrow_ipc),
                arrow_ipc,
                payload_reference: None,
                schema_digest: rust_mir_observation.schema_digest.to_owned(),
                row_count: 1,
            };
            send_event(
                &event_sender,
                Event::OwnerObservationChunk(chunk.clone()),
                &mut events,
            )?;
            match receive_command(&monitor_receiver, deadline)? {
                Command::ChunkAccepted(accepted) if accepted.sequence == sequence => {}
                Command::ChunkRejected(rejected) if rejected.sequence == sequence => {
                    return Err(format!(
                        "daemon rejected MIR chunk: {}",
                        rejected.error_code
                    ));
                }
                Command::Cancel(_) => {
                    cancelled.store(true, Ordering::Release);
                    break;
                }
                _ => return Err("daemon returned an invalid chunk acknowledgement".to_owned()),
            }
            sequence += 1;
            let owner_end = OwnerEnd {
                provider_run_id: environment.provider_run_id.clone(),
                compilation_unit_id: compilation_unit_id.clone(),
                sequence,
                owner_id,
                family_counts: [(rust_mir_observation_family_code, 1)]
                    .into_iter()
                    .collect(),
                owner_content_digest: owner_content_digest(&owner_begin, &[chunk]),
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
                "COMPILER_BODY_CENSUS_COMPLETE".to_owned()
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
) -> Result<i32, String> {
    let Some(environment) = WrapperEnvironment::resolve()? else {
        return passthrough(real_rustc, arguments);
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create extractor runtime: {error}"))?;
    run_protocol(&runtime, environment, real_rustc, arguments, identity_bytes)
}

#[cfg(test)]
mod tests {
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

    use super::*;

    type MockCommandStream =
        Pin<Box<dyn Stream<Item = Result<ExtractorCommand, Status>> + Send + 'static>>;

    #[derive(Clone, Debug)]
    struct MockDaemon {
        events: Arc<StdMutex<Vec<ExtractionEvent>>>,
        deadline_unix_ms: i64,
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
                    output_schema_bundle_digest: b3(b"schema"),
                    sandbox_profile_digest: b3(b"sandbox"),
                    maximum_outstanding_chunks: 4,
                    maximum_unacknowledged_bytes: MAX_CHUNK_BYTES as u64,
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
                                            granted_credit_bytes: MAX_CHUNK_BYTES as u64,
                                        },
                                    )),
                                }))
                                .await;
                        }
                        Some(Event::OwnerObservationChunk(chunk)) => {
                            let _ = sender
                                .send(Ok(ExtractorCommand {
                                    command: Some(Command::ChunkAccepted(
                                        crate::protocol::generated::codefabric::provider::v1::ChunkAccepted {
                                            sequence: chunk.sequence,
                                            next_credit_bytes: MAX_CHUNK_BYTES as u64,
                                            next_credit_chunks: 4,
                                        },
                                    )),
                                }))
                                .await;
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
        let item = OwnedMirItem {
            name: "fixture::answer".to_owned(),
            item_kind: "function".to_owned(),
            type_description: "fn() -> u8".to_owned(),
            requires_monomorphization: false,
            basic_block_count: 1,
            local_count: 1,
            statement_kinds: vec!["assign".to_owned()],
            terminator_kinds: vec!["return".to_owned()],
            successor_count: 0,
        };
        let first = encode_mir_item(&item).unwrap();
        let second = encode_mir_item(&item).unwrap();
        assert_eq!(first, second);
        assert!(first.starts_with(b"ARROW1") || first.starts_with(&[0xff; 4]));
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
            environment,
            OsStr::new("rustc"),
            &arguments,
            include_bytes!("../toolchain-identity.json"),
        )
        .unwrap();
        assert_eq!(exit, 0);

        for _ in 0..100 {
            if matches!(
                events
                    .lock()
                    .unwrap()
                    .last()
                    .and_then(|event| event.event.as_ref()),
                Some(Event::CompilationEnd(_))
            ) {
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
        let chunk = observed
            .iter()
            .find_map(|event| match event.event.as_ref() {
                Some(Event::OwnerObservationChunk(chunk)) => Some(chunk),
                _ => None,
            })
            .unwrap();
        let mut reader = StreamReader::try_new(Cursor::new(&chunk.arrow_ipc), None).unwrap();
        let batch = reader.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 1);
        assert_eq!(batch.num_columns(), 9);

        let _ = shutdown_sender.send(());
        server.join().unwrap();
    }
}
