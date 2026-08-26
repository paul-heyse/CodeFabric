//! Daemon-hosted validation and flow control for the compiler-wrapper protocol.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::future::Future;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::{Stream, stream};
use prost::Message;
use tokio::sync::{Mutex, mpsc};
use tonic::service::InterceptorLayer;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::registries::RustcFeatureMask;
use crate::registries::{PROVIDER_ENTRIES, PROVIDER_RESOURCE_PROFILES};
use crate::rpc::generated::codefabric::provider::v1::{
    CancelAcknowledgement, CancelAcknowledgementState, ChunkAccepted, ChunkRejected,
    ProviderRunState,
};
use crate::rpc::generated::codefabric::rustc::v1::extraction_event::Event;
use crate::rpc::generated::codefabric::rustc::v1::extractor_command::Command;
use crate::rpc::generated::codefabric::rustc::v1::rustc_extractor_server::RustcExtractor;
use crate::rpc::generated::codefabric::rustc::v1::rustc_extractor_server::RustcExtractorServer;
use crate::rpc::generated::codefabric::rustc::v1::{
    CancelCompilationRequest, CompilationBegin, CompilationEnd, ExtractionEvent, ExtractorCommand,
    ExtractorHello, ExtractorHelloAck, OwnerBegin, OwnerEnd, OwnerObservationChunk,
    RejectionRuleErrorCode,
};
use crate::rpc::{AuthorizedUnixStream, SameUserInterceptor, negotiate_feature_bits};

include!("generated/digest_frames.rs");

/// AC-G-31 maximum number of chunks a wrapper may have in flight.
pub const MAX_OUTSTANDING_CHUNKS: u32 = 4;
/// AC-G-31 maximum unacknowledged payload bytes per compilation.
pub const MAX_UNACKNOWLEDGED_BYTES: u64 = 16 * 1024 * 1024;
const MAX_DECODED_MESSAGE_BYTES: usize = 17 * 1024 * 1024;

type CommandStream = Pin<Box<dyn Stream<Item = Result<ExtractorCommand, Status>> + Send>>;

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}

fn digest(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 67
        && value.starts_with("b3:")
        && value[3..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1_024
        && !value
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
}

/// Digest an inline Arrow IPC payload with the canonical `b3:` framing.
#[must_use]
pub fn arrow_chunk_digest(bytes: &[u8]) -> String {
    digest(bytes)
}

/// Compute the governed owner-content digest from a begin record and ordered chunks.
#[must_use]
pub fn owner_content_digest(begin: &OwnerBegin, chunks: &[OwnerObservationChunk]) -> String {
    let mut fields = vec![begin.encode_to_vec()];
    fields.extend(chunks.iter().map(Message::encode_to_vec));
    digest_frames(b"codefabric.rustc.owner-content.v1\0", fields)
}

/// Compute the closed-owner-set digest independently of owner arrival order.
#[must_use]
pub fn closed_owner_set_digest(owners: &[AcceptedRustcOwner]) -> String {
    let mut fields = owners
        .iter()
        .map(|owner| {
            let mut bytes = owner
                .begin
                .owner
                .as_ref()
                .map_or_else(Vec::new, |key| key.owner_id.as_bytes().to_vec());
            bytes.extend_from_slice(owner.end.owner_content_digest.as_bytes());
            bytes
        })
        .collect::<Vec<_>>();
    fields.sort();
    digest_frames(b"codefabric.rustc.closed-owner-set.v1\0", fields)
}

/// Compute the stream digest with the terminal digest field cleared.
#[must_use]
pub fn overall_stream_digest(events: &[ExtractionEvent]) -> String {
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

/// Immutable per-run admission resolved before Cargo is launched.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcRunAdmission {
    pub provider_run_id: String,
    pub workspace_id: String,
    pub analysis_context_id: String,
    pub canonical_workspace_id: [u8; 16],
    pub canonical_analysis_context_id: [u8; 16],
    pub source_generation: u64,
    pub context_manifest_digest: String,
    pub source_snapshot_manifest_digest: String,
    pub resource_profile_id: String,
}

/// Exact protocol/toolchain identities supplied by the daemon endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcProtocolPolicy {
    pub daemon_build: String,
    pub output_schema_bundle_digest: String,
    pub sandbox_profile_digest: String,
    pub extractor_build: String,
    pub rustc_version: String,
    pub rustc_commit: String,
    pub toolchain_identity_digest: String,
    pub supported_feature_bits: u64,
    pub provider_deadline_unix_ms: i64,
}

impl RustcProtocolPolicy {
    fn validate(&self) -> Result<(), Status> {
        if !valid_identifier(&self.daemon_build)
            || !valid_identifier(&self.extractor_build)
            || !valid_identifier(&self.rustc_version)
            || !valid_identifier(&self.rustc_commit)
            || !valid_digest(&self.output_schema_bundle_digest)
            || !valid_digest(&self.sandbox_profile_digest)
            || !valid_digest(&self.toolchain_identity_digest)
            || self.provider_deadline_unix_ms <= now_millis()
        {
            return Err(Status::invalid_argument(
                "rustc protocol policy is incomplete or expired",
            ));
        }
        Ok(())
    }
}

/// One completely verified compiler owner and its Arrow observation chunks.
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptedRustcOwner {
    pub begin: OwnerBegin,
    pub chunks: Vec<OwnerObservationChunk>,
    pub end: OwnerEnd,
}

/// One compiler stream admitted for canonical reconciliation.
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptedRustcCompilation {
    pub admission: RustcRunAdmission,
    pub begin: CompilationBegin,
    pub owners: Vec<AcceptedRustcOwner>,
    pub end: CompilationEnd,
}

#[derive(Clone, Debug)]
struct ActiveRun {
    compilation_unit_id: String,
    commands: mpsc::Sender<Result<ExtractorCommand, Status>>,
    cancelled: bool,
    terminal_state: Option<ProviderRunState>,
}

#[derive(Debug)]
struct OpenOwner {
    begin: OwnerBegin,
    expected_families: BTreeSet<u32>,
    observed_counts: BTreeMap<u32, u64>,
    chunks: Vec<OwnerObservationChunk>,
}

#[derive(Debug)]
struct RunValidator {
    admission: RustcRunAdmission,
    policy: RustcProtocolPolicy,
    begin: CompilationBegin,
    events: Vec<ExtractionEvent>,
    owners: Vec<AcceptedRustcOwner>,
    owner_ids: BTreeSet<String>,
    open_owner: Option<OpenOwner>,
    next_sequence: u64,
}

impl RunValidator {
    fn new(
        admission: RustcRunAdmission,
        policy: RustcProtocolPolicy,
        begin: CompilationBegin,
        first_event: ExtractionEvent,
    ) -> Result<Self, Status> {
        validate_begin(&admission, &policy, &begin)?;
        Ok(Self {
            admission,
            policy,
            begin,
            events: vec![first_event],
            owners: Vec::new(),
            owner_ids: BTreeSet::new(),
            open_owner: None,
            next_sequence: 1,
        })
    }

    fn accept_owner_begin(
        &mut self,
        begin: OwnerBegin,
        event: ExtractionEvent,
    ) -> Result<(), Status> {
        self.validate_header(
            &begin.provider_run_id,
            &begin.compilation_unit_id,
            begin.sequence,
        )?;
        if self.open_owner.is_some() {
            return Err(Status::failed_precondition(
                "previous compiler owner is not closed",
            ));
        }
        let owner = begin
            .owner
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("compiler owner key is missing"))?;
        if !valid_identifier(&owner.owner_id)
            || !valid_identifier(&owner.owner_kind)
            || !valid_identifier(&owner.file_id)
            || owner.source_start > owner.source_end
            || begin.expected_observation_family_codes.is_empty()
        {
            return Err(Status::invalid_argument(
                "compiler owner identity is invalid",
            ));
        }
        let expected_families = begin
            .expected_observation_family_codes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if expected_families.len() != begin.expected_observation_family_codes.len()
            || expected_families.contains(&0)
            || !self.owner_ids.insert(owner.owner_id.clone())
        {
            return Err(Status::already_exists(
                "compiler owner or observation family is duplicated",
            ));
        }
        self.next_sequence += 1;
        self.events.push(event);
        self.open_owner = Some(OpenOwner {
            begin,
            expected_families,
            observed_counts: BTreeMap::new(),
            chunks: Vec::new(),
        });
        Ok(())
    }

    fn accept_chunk(
        &mut self,
        chunk: OwnerObservationChunk,
        event: ExtractionEvent,
    ) -> Result<ExtractorCommand, Status> {
        self.validate_header(
            &chunk.provider_run_id,
            &chunk.compilation_unit_id,
            chunk.sequence,
        )?;
        let owner = self
            .open_owner
            .as_mut()
            .ok_or_else(|| Status::failed_precondition("observation chunk has no open owner"))?;
        let owner_id = owner
            .begin
            .owner
            .as_ref()
            .map(|key| key.owner_id.as_str())
            .unwrap_or_default();
        if chunk.owner_id != owner_id
            || !owner
                .expected_families
                .contains(&chunk.observation_family_code)
            || chunk.row_count == 0
            || !valid_digest(&chunk.schema_digest)
            || !valid_digest(&chunk.chunk_digest)
        {
            return Err(Status::invalid_argument(
                "observation chunk identity or family is invalid",
            ));
        }
        let payload_bytes = match (&chunk.arrow_ipc[..], chunk.payload_reference.as_ref()) {
            (bytes, None) if !bytes.is_empty() => {
                if bytes.len() as u64 > MAX_UNACKNOWLEDGED_BYTES
                    || arrow_chunk_digest(bytes) != chunk.chunk_digest
                {
                    return Err(Status::resource_exhausted(
                        "inline Arrow chunk exceeds credit or differs from its digest",
                    ));
                }
                bytes.len() as u64
            }
            ([], Some(reference)) => {
                if reference.byte_length == 0
                    || reference.byte_length > MAX_UNACKNOWLEDGED_BYTES
                    || reference.content_digest != chunk.chunk_digest
                    || !valid_identifier(&reference.blob_id)
                    || !valid_identifier(&reference.read_only_uri)
                {
                    return Err(Status::invalid_argument("payload reference is invalid"));
                }
                reference.byte_length
            }
            _ => {
                return Err(Status::invalid_argument(
                    "exactly one chunk payload representation is required",
                ));
            }
        };
        *owner
            .observed_counts
            .entry(chunk.observation_family_code)
            .or_default() += chunk.row_count;
        let sequence = chunk.sequence;
        owner.chunks.push(chunk);
        self.next_sequence += 1;
        self.events.push(event);
        Ok(ExtractorCommand {
            command: Some(Command::ChunkAccepted(ChunkAccepted {
                sequence,
                next_credit_bytes: MAX_UNACKNOWLEDGED_BYTES.saturating_sub(payload_bytes),
                next_credit_chunks: MAX_OUTSTANDING_CHUNKS.saturating_sub(1),
            })),
        })
    }

    fn accept_owner_end(&mut self, end: OwnerEnd, event: ExtractionEvent) -> Result<(), Status> {
        self.validate_header(&end.provider_run_id, &end.compilation_unit_id, end.sequence)?;
        let owner = self
            .open_owner
            .take()
            .ok_or_else(|| Status::failed_precondition("owner end has no open owner"))?;
        let owner_id = owner
            .begin
            .owner
            .as_ref()
            .map(|key| key.owner_id.as_str())
            .unwrap_or_default();
        let reported_counts = end
            .family_counts
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<BTreeMap<_, _>>();
        if end.owner_id != owner_id
            || reported_counts != owner.observed_counts
            || end.owner_content_digest != owner_content_digest(&owner.begin, &owner.chunks)
        {
            return Err(Status::data_loss(
                "owner counts or content digest differ from accepted chunks",
            ));
        }
        self.next_sequence += 1;
        self.events.push(event);
        self.owners.push(AcceptedRustcOwner {
            begin: owner.begin,
            chunks: owner.chunks,
            end,
        });
        Ok(())
    }

    fn finish(
        mut self,
        end: CompilationEnd,
        event: ExtractionEvent,
        cancelled: bool,
    ) -> Result<AcceptedRustcCompilation, Status> {
        self.validate_header(&end.provider_run_id, &end.compilation_unit_id, end.sequence)?;
        if self.open_owner.is_some() {
            return Err(Status::failed_precondition(
                "compilation ended with an open owner",
            ));
        }
        let terminal_state = ProviderRunState::try_from(end.terminal_state)
            .map_err(|_| Status::invalid_argument("unknown provider terminal state"))?;
        let valid_terminal = match terminal_state {
            ProviderRunState::Succeeded => {
                !cancelled && end.compiler_exit_status == 0 && end.rejection_error.is_none()
            }
            ProviderRunState::Failed => {
                !cancelled
                    && end.compiler_exit_status != 0
                    && end.rejection_error == Some(RejectionRuleErrorCode::CompilerFailed as i32)
            }
            ProviderRunState::Cancelled => cancelled && end.rejection_error.is_none(),
            _ => false,
        };
        if !valid_terminal {
            return Err(Status::failed_precondition(
                "compiler stream did not reach the accepted terminal state",
            ));
        }
        if end.closed_owner_set_digest != closed_owner_set_digest(&self.owners) {
            return Err(Status::data_loss("closed-owner-set digest differs"));
        }
        self.events.push(event);
        if end.overall_stream_digest != overall_stream_digest(&self.events) {
            return Err(Status::data_loss("overall compiler stream digest differs"));
        }
        Ok(AcceptedRustcCompilation {
            admission: self.admission,
            begin: self.begin,
            owners: self.owners,
            end,
        })
    }

    fn validate_header(&self, run_id: &str, unit_id: &str, sequence: u64) -> Result<(), Status> {
        if run_id != self.admission.provider_run_id
            || unit_id != self.begin.compilation_unit_id
            || sequence != self.next_sequence
        {
            return Err(Status::failed_precondition(
                "compiler event identity or sequence differs",
            ));
        }
        if now_millis() > self.policy.provider_deadline_unix_ms {
            return Err(Status::deadline_exceeded(
                "compiler provider deadline elapsed",
            ));
        }
        Ok(())
    }
}

fn validate_begin(
    admission: &RustcRunAdmission,
    policy: &RustcProtocolPolicy,
    begin: &CompilationBegin,
) -> Result<(), Status> {
    let target = begin
        .target
        .as_ref()
        .ok_or_else(|| Status::invalid_argument("package target identity is missing"))?;
    if begin.provider_run_id != admission.provider_run_id
        || begin.workspace_id != admission.workspace_id
        || begin.analysis_context_id != admission.analysis_context_id
        || begin.source_generation != admission.source_generation
        || begin.context_manifest_digest != admission.context_manifest_digest
        || begin.source_snapshot_manifest_digest != admission.source_snapshot_manifest_digest
        || begin.resource_profile_id != admission.resource_profile_id
        || begin.rustc_version != policy.rustc_version
        || begin.rustc_commit != policy.rustc_commit
        || begin.toolchain_identity_digest != policy.toolchain_identity_digest
    {
        return Err(Status::failed_precondition(
            "compiler begin differs from the admitted run",
        ));
    }
    let identifiers = [
        begin.provider_run_id.as_str(),
        begin.compilation_unit_id.as_str(),
        begin.workspace_id.as_str(),
        begin.analysis_context_id.as_str(),
        begin.resource_profile_id.as_str(),
        target.package_id.as_str(),
        target.package_name.as_str(),
        target.target_name.as_str(),
        target.target_kind.as_str(),
        target.crate_name.as_str(),
        target.crate_type.as_str(),
        target.crate_disambiguator.as_str(),
    ];
    let digests = [
        begin.normalized_rustc_invocation_digest.as_str(),
        begin.cargo_metadata_digest.as_str(),
        begin.cargo_lock_digest.as_str(),
        begin.cargo_config_digest.as_str(),
        begin.source_snapshot_manifest_digest.as_str(),
        begin.context_manifest_digest.as_str(),
        begin.toolchain_identity_digest.as_str(),
    ];
    if identifiers.iter().any(|value| !valid_identifier(value))
        || digests.iter().any(|value| !valid_digest(value))
        || begin.requested_capability_codes.is_empty()
        || !strictly_increasing(&begin.requested_capability_codes)
        || !sorted_digests(&begin.build_script_output_digests)
        || !sorted_digests(&begin.proc_macro_output_digests)
    {
        return Err(Status::invalid_argument(
            "compiler begin contains an invalid identity, digest, or capability set",
        ));
    }
    Ok(())
}

fn strictly_increasing(values: &[u32]) -> bool {
    values.iter().all(|value| *value != 0) && values.windows(2).all(|window| window[0] < window[1])
}

fn sorted_digests(values: &[String]) -> bool {
    values.iter().all(|value| valid_digest(value))
        && values.windows(2).all(|window| window[0] < window[1])
}

/// Daemon service plus a bounded sink of fully verified compiler observations.
#[derive(Clone, Debug)]
pub struct RustcObservationService {
    policy: RustcProtocolPolicy,
    admission: RustcRunAdmission,
    active: Arc<Mutex<BTreeMap<String, ActiveRun>>>,
    accepted: mpsc::Sender<AcceptedRustcCompilation>,
}

impl RustcObservationService {
    /// Construct one private endpoint policy and its canonical-ingest receiver.
    ///
    /// # Errors
    ///
    /// Rejects incomplete identities, malformed digests, and already-expired policies.
    pub fn new(
        policy: RustcProtocolPolicy,
        admission: RustcRunAdmission,
    ) -> Result<(Self, mpsc::Receiver<AcceptedRustcCompilation>), Status> {
        policy.validate()?;
        if !valid_identifier(&admission.provider_run_id)
            || !valid_identifier(&admission.workspace_id)
            || !valid_identifier(&admission.analysis_context_id)
            || !valid_identifier(&admission.resource_profile_id)
            || !valid_digest(&admission.context_manifest_digest)
            || !valid_digest(&admission.source_snapshot_manifest_digest)
            || admission.canonical_workspace_id == [0; 16]
            || admission.canonical_analysis_context_id == [0; 16]
        {
            return Err(Status::invalid_argument("rustc run admission is invalid"));
        }
        let provider = PROVIDER_ENTRIES
            .iter()
            .find(|provider| provider.provider_id == "rustc-mir")
            .ok_or_else(|| Status::failed_precondition("rustc provider registry is absent"))?;
        let profile = PROVIDER_RESOURCE_PROFILES
            .iter()
            .find(|profile| profile.profile_id == admission.resource_profile_id)
            .ok_or_else(|| Status::failed_precondition("rustc resource profile is absent"))?;
        if provider.placement != "COMPILER_GROUP"
            || provider.resource_profile_id != profile.profile_id
            || !profile.provider_ids.contains(&provider.provider_id)
        {
            return Err(Status::failed_precondition(
                "rustc provider resource-profile binding differs",
            ));
        }
        let (accepted, receiver) = mpsc::channel(MAX_OUTSTANDING_CHUNKS as usize);
        Ok((
            Self {
                policy,
                admission,
                active: Arc::new(Mutex::new(BTreeMap::new())),
                accepted,
            },
            receiver,
        ))
    }

    /// Request cancellation through the existing reverse command stream.
    pub async fn request_cancel(&self, request: CancelCompilationRequest) -> CancelAcknowledgement {
        let command_sender = {
            let mut active = self.active.lock().await;
            let Some(run) = active.get_mut(&request.provider_run_id) else {
                return cancellation_ack(
                    request.provider_run_id,
                    CancelAcknowledgementState::NotFound,
                    None,
                );
            };
            if run.compilation_unit_id != request.compilation_unit_id {
                return cancellation_ack(
                    request.provider_run_id,
                    CancelAcknowledgementState::NotFound,
                    None,
                );
            }
            if let Some(terminal) = run.terminal_state {
                return cancellation_ack(
                    request.provider_run_id,
                    CancelAcknowledgementState::AlreadyTerminal,
                    Some(terminal),
                );
            }
            if run.cancelled {
                None
            } else {
                run.cancelled = true;
                Some(run.commands.clone())
            }
        };
        if let Some(sender) = command_sender {
            let _ = sender
                .send(Ok(ExtractorCommand {
                    command: Some(Command::Cancel(request.clone())),
                }))
                .await;
        }
        cancellation_ack(
            request.provider_run_id,
            CancelAcknowledgementState::CancellationRequested,
            None,
        )
    }

    async fn run_cancelled(&self) -> bool {
        self.active
            .lock()
            .await
            .get(&self.admission.provider_run_id)
            .is_some_and(|run| run.cancelled)
    }

    async fn mark_terminal(&self, state: ProviderRunState) {
        if let Some(run) = self
            .active
            .lock()
            .await
            .get_mut(&self.admission.provider_run_id)
        {
            run.terminal_state = Some(state);
        }
    }

    async fn next_event(
        &self,
        input: &mut tonic::Streaming<ExtractionEvent>,
    ) -> Result<Option<ExtractionEvent>, Status> {
        let remaining = self
            .policy
            .provider_deadline_unix_ms
            .saturating_sub(now_millis());
        if remaining <= 0 {
            return Err(Status::deadline_exceeded(
                "compiler provider deadline elapsed",
            ));
        }
        tokio::time::timeout(
            std::time::Duration::from_millis(remaining.cast_unsigned()),
            input.message(),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("compiler provider deadline elapsed"))?
    }

    #[allow(clippy::too_many_lines)]
    async fn process_stream(
        &self,
        mut input: tonic::Streaming<ExtractionEvent>,
        output: mpsc::Sender<Result<ExtractorCommand, Status>>,
    ) -> Result<(), Status> {
        let first_event = self
            .next_event(&mut input)
            .await?
            .ok_or_else(|| Status::invalid_argument("compiler stream ended before begin"))?;
        let Some(Event::CompilationBegin(begin)) = first_event.event.as_ref() else {
            return Err(Status::invalid_argument(
                "first compiler event must be CompilationBegin",
            ));
        };
        let begin = begin.clone();
        let mut validator = RunValidator::new(
            self.admission.clone(),
            self.policy.clone(),
            begin.clone(),
            first_event,
        )?;
        {
            let mut active = self.active.lock().await;
            match active.entry(begin.provider_run_id.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(ActiveRun {
                        compilation_unit_id: begin.compilation_unit_id.clone(),
                        commands: output.clone(),
                        cancelled: false,
                        terminal_state: None,
                    });
                }
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err(Status::already_exists("compiler run is already active"));
                }
            }
        }
        output
            .send(Ok(ExtractorCommand {
                command: Some(Command::CompilationAccepted(
                    crate::rpc::generated::codefabric::rustc::v1::CompilationAccepted {
                        provider_run_id: begin.provider_run_id.clone(),
                        compilation_unit_id: begin.compilation_unit_id.clone(),
                        accepted_generation: begin.source_generation,
                        granted_chunk_credits: MAX_OUTSTANDING_CHUNKS,
                        granted_credit_bytes: MAX_UNACKNOWLEDGED_BYTES,
                    },
                )),
            }))
            .await
            .map_err(|_| Status::cancelled("compiler command stream closed"))?;

        while let Some(event) = self.next_event(&mut input).await? {
            let cancelled = self.run_cancelled().await;
            match event.event.clone() {
                Some(Event::OwnerBegin(begin)) if !cancelled => {
                    validator.accept_owner_begin(begin, event)?;
                }
                Some(Event::OwnerObservationChunk(chunk)) if !cancelled => {
                    let sequence = chunk.sequence;
                    match validator.accept_chunk(chunk, event) {
                        Ok(command) => output
                            .send(Ok(command))
                            .await
                            .map_err(|_| Status::cancelled("compiler command stream closed"))?,
                        Err(error) => {
                            let _ = output
                                .send(Ok(ExtractorCommand {
                                    command: Some(Command::ChunkRejected(ChunkRejected {
                                        sequence,
                                        error_code: error.code().to_string(),
                                    })),
                                }))
                                .await;
                            return Err(error);
                        }
                    }
                }
                Some(Event::OwnerEnd(end)) if !cancelled => {
                    validator.accept_owner_end(end, event)?;
                }
                Some(Event::CompilationEnd(end)) => {
                    let completed = validator.finish(end, event, cancelled)?;
                    let terminal = ProviderRunState::try_from(completed.end.terminal_state)
                        .unwrap_or(ProviderRunState::ProtocolError);
                    if terminal == ProviderRunState::Succeeded {
                        self.accepted
                            .send(completed)
                            .await
                            .map_err(|_| Status::unavailable("canonical ingest sink is closed"))?;
                    }
                    self.mark_terminal(terminal).await;
                    return Ok(());
                }
                Some(Event::CompilationBegin(_)) => {
                    return Err(Status::failed_precondition(
                        "CompilationBegin may appear only once",
                    ));
                }
                Some(_) if cancelled => {
                    // Output after cancellation acknowledgement is intentionally ignored.
                }
                None => return Err(Status::invalid_argument("compiler event is empty")),
                Some(_) => {
                    return Err(Status::failed_precondition(
                        "compiler event is out of order",
                    ));
                }
            }
        }
        Err(Status::data_loss(
            "compiler stream ended before CompilationEnd",
        ))
    }
}

/// Serve one private compiler-observation endpoint with kernel peer-UID authentication.
///
/// # Errors
///
/// Returns an I/O or transport error when the private socket cannot be exclusively created,
/// secured, accepted, or served.
pub async fn serve_rustc_uds<F>(
    socket: &Path,
    allowed_uid: u32,
    service: RustcObservationService,
    shutdown: F,
) -> Result<(), RustcTransportError>
where
    F: Future<Output = ()> + Send + 'static,
{
    if socket.exists() {
        return Err(RustcTransportError::SocketExists(socket.to_path_buf()));
    }
    let listener =
        tokio::net::UnixListener::bind(socket).map_err(|source| RustcTransportError::Io {
            path: socket.to_path_buf(),
            source,
        })?;
    fs::set_permissions(socket, fs::Permissions::from_mode(0o600)).map_err(|source| {
        RustcTransportError::Io {
            path: socket.to_path_buf(),
            source,
        }
    })?;
    let incoming = stream::unfold(listener, move |listener| async move {
        let accepted = listener
            .accept()
            .await
            .and_then(|(stream, _)| AuthorizedUnixStream::authenticate(stream, allowed_uid));
        Some((accepted, listener))
    });
    let service = RustcExtractorServer::new(service)
        .max_decoding_message_size(MAX_DECODED_MESSAGE_BYTES)
        .max_encoding_message_size(4 * 1024 * 1024);
    let result = Server::builder()
        .layer(InterceptorLayer::new(SameUserInterceptor::new(allowed_uid)))
        .add_service(service)
        .serve_with_incoming_shutdown(incoming, shutdown)
        .await;
    let _ = fs::remove_file(socket);
    result.map_err(RustcTransportError::Transport)
}

/// Private compiler-observation endpoint failures.
#[derive(Debug, thiserror::Error)]
pub enum RustcTransportError {
    #[error("rustc extractor socket already exists at {0}")]
    SocketExists(PathBuf),
    #[error("rustc extractor transport I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("rustc extractor transport failed: {0}")]
    Transport(tonic::transport::Error),
}

fn cancellation_ack(
    provider_run_id: String,
    state: CancelAcknowledgementState,
    terminal: Option<ProviderRunState>,
) -> CancelAcknowledgement {
    CancelAcknowledgement {
        provider_run_id,
        state: state as i32,
        acknowledged_at_unix_ms: now_millis(),
        terminal_state: terminal.map(|value| value as i32),
        cleaning_up_components: Vec::new(),
        forced_termination: false,
    }
}

#[tonic::async_trait]
impl RustcExtractor for RustcObservationService {
    async fn handshake(
        &self,
        request: Request<ExtractorHello>,
    ) -> Result<Response<ExtractorHelloAck>, Status> {
        let hello = request.into_inner();
        if hello.protocol_major != 1
            || hello.protocol_minor != 0
            || hello.extractor_build != self.policy.extractor_build
            || hello.rustc_version != self.policy.rustc_version
            || hello.rustc_commit != self.policy.rustc_commit
            || hello.toolchain_identity_digest != self.policy.toolchain_identity_digest
            || hello.resource_profile_id != self.admission.resource_profile_id
        {
            return Err(Status::failed_precondition(
                "extractor handshake identity differs from admission",
            ));
        }
        let negotiated = negotiate_feature_bits(
            RustcFeatureMask::from_wire(hello.required_feature_bits),
            RustcFeatureMask::from_wire(hello.optional_feature_bits),
            RustcFeatureMask::from_wire(self.policy.supported_feature_bits),
            RustcFeatureMask::NONE,
        )?;
        Ok(Response::new(ExtractorHelloAck {
            protocol_major: 1,
            protocol_minor: 0,
            negotiated_feature_bits: negotiated.bits(),
            daemon_build: self.policy.daemon_build.clone(),
            output_schema_bundle_digest: self.policy.output_schema_bundle_digest.clone(),
            sandbox_profile_digest: self.policy.sandbox_profile_digest.clone(),
            maximum_outstanding_chunks: MAX_OUTSTANDING_CHUNKS,
            maximum_unacknowledged_bytes: MAX_UNACKNOWLEDGED_BYTES,
            accepted_resource_profile_id: self.admission.resource_profile_id.clone(),
            provider_deadline_unix_ms: self.policy.provider_deadline_unix_ms,
        }))
    }

    type ObserveStream = CommandStream;

    async fn observe(
        &self,
        request: Request<tonic::Streaming<ExtractionEvent>>,
    ) -> Result<Response<Self::ObserveStream>, Status> {
        let (sender, receiver) = mpsc::channel((MAX_OUTSTANDING_CHUNKS + 2) as usize);
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(error) = service
                .process_stream(request.into_inner(), sender.clone())
                .await
            {
                service.mark_terminal(ProviderRunState::ProtocolError).await;
                let _ = sender.send(Err(error)).await;
            }
        });
        let output = stream::unfold(receiver, |mut receiver| async move {
            receiver.recv().await.map(|item| (item, receiver))
        });
        Ok(Response::new(Box::pin(output)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::generated::codefabric::rustc::v1::{
        CompilerOwnerKey, DiagnosticSummary, PackageTargetIdentity,
    };

    fn b3(value: &str) -> String {
        digest(value.as_bytes())
    }

    fn fixture() -> (RustcProtocolPolicy, RustcRunAdmission, CompilationBegin) {
        let policy = RustcProtocolPolicy {
            daemon_build: "codefabricd-test".to_owned(),
            output_schema_bundle_digest: b3("schema"),
            sandbox_profile_digest: b3("sandbox"),
            extractor_build: "codefabric-rustc-extractor 0.1.0".to_owned(),
            rustc_version: "1.100.0-nightly".to_owned(),
            rustc_commit: "8fa1c96cfd489e4c27654c144ae871ce2c4db6c6".to_owned(),
            toolchain_identity_digest: b3("toolchain"),
            supported_feature_bits: 0,
            provider_deadline_unix_ms: now_millis() + 60_000,
        };
        let admission = RustcRunAdmission {
            provider_run_id: "run:test".to_owned(),
            workspace_id: "workspace:test".to_owned(),
            analysis_context_id: "context:test".to_owned(),
            canonical_workspace_id: [1; 16],
            canonical_analysis_context_id: [2; 16],
            source_generation: 7,
            context_manifest_digest: b3("context"),
            source_snapshot_manifest_digest: b3("source"),
            resource_profile_id: "compiler-semantic-standard".to_owned(),
        };
        let begin = CompilationBegin {
            provider_run_id: admission.provider_run_id.clone(),
            compilation_unit_id: "unit:test".to_owned(),
            workspace_id: admission.workspace_id.clone(),
            analysis_context_id: admission.analysis_context_id.clone(),
            source_generation: admission.source_generation,
            target: Some(PackageTargetIdentity {
                package_id: "pkg:test".to_owned(),
                package_name: "fixture".to_owned(),
                target_name: "fixture".to_owned(),
                target_kind: "lib".to_owned(),
                crate_name: "fixture".to_owned(),
                crate_type: "lib".to_owned(),
                crate_disambiguator: "test".to_owned(),
            }),
            rustc_version: policy.rustc_version.clone(),
            rustc_commit: policy.rustc_commit.clone(),
            normalized_rustc_invocation_digest: b3("args"),
            cargo_metadata_digest: b3("metadata"),
            cargo_lock_digest: b3("lock"),
            cargo_config_digest: b3("config"),
            build_script_output_digests: Vec::new(),
            proc_macro_output_digests: Vec::new(),
            source_snapshot_manifest_digest: admission.source_snapshot_manifest_digest.clone(),
            requested_capability_codes: vec![70],
            context_manifest_digest: admission.context_manifest_digest.clone(),
            resource_profile_id: admission.resource_profile_id.clone(),
            toolchain_identity_digest: policy.toolchain_identity_digest.clone(),
        };
        (policy, admission, begin)
    }

    fn event(event: Event) -> ExtractionEvent {
        ExtractionEvent { event: Some(event) }
    }

    fn accepted_stream() -> (RunValidator, CompilationEnd) {
        let (policy, admission, begin) = fixture();
        let first = event(Event::CompilationBegin(begin.clone()));
        let mut validator = RunValidator::new(admission, policy, begin, first).unwrap();
        let owner_begin = OwnerBegin {
            provider_run_id: "run:test".to_owned(),
            compilation_unit_id: "unit:test".to_owned(),
            sequence: 1,
            owner: Some(CompilerOwnerKey {
                owner_id: "owner:test".to_owned(),
                owner_kind: "MIR_BODY".to_owned(),
                file_id: "file:test".to_owned(),
                source_start: 0,
                source_end: 8,
            }),
            expected_observation_family_codes: vec![70],
        };
        validator
            .accept_owner_begin(
                owner_begin.clone(),
                event(Event::OwnerBegin(owner_begin.clone())),
            )
            .unwrap();
        let payload = b"arrow-ipc".to_vec();
        let chunk = OwnerObservationChunk {
            provider_run_id: "run:test".to_owned(),
            compilation_unit_id: "unit:test".to_owned(),
            sequence: 2,
            owner_id: "owner:test".to_owned(),
            observation_family_code: 70,
            arrow_ipc: payload.clone(),
            payload_reference: None,
            schema_digest: b3("mir-schema"),
            row_count: 1,
            chunk_digest: arrow_chunk_digest(&payload),
        };
        validator
            .accept_chunk(
                chunk.clone(),
                event(Event::OwnerObservationChunk(chunk.clone())),
            )
            .unwrap();
        let owner_end = OwnerEnd {
            provider_run_id: "run:test".to_owned(),
            compilation_unit_id: "unit:test".to_owned(),
            sequence: 3,
            owner_id: "owner:test".to_owned(),
            family_counts: [(70, 1)].into_iter().collect(),
            owner_content_digest: owner_content_digest(&owner_begin, &[chunk]),
        };
        validator
            .accept_owner_end(owner_end.clone(), event(Event::OwnerEnd(owner_end)))
            .unwrap();
        let compilation_end = CompilationEnd {
            provider_run_id: "run:test".to_owned(),
            compilation_unit_id: "unit:test".to_owned(),
            sequence: 4,
            compiler_exit_status: 0,
            closed_owner_set_digest: closed_owner_set_digest(&validator.owners),
            capability_outcomes: Vec::new(),
            diagnostic_summary: Some(DiagnosticSummary {
                error_count: 0,
                warning_count: 0,
                diagnostics_digest: b3("diagnostics"),
            }),
            overall_stream_digest: String::new(),
            terminal_state: ProviderRunState::Succeeded as i32,
            rejection_error: None,
        };
        (validator, compilation_end)
    }

    #[test]
    fn wp35_behavioral_acceptance() {
        let (validator, mut end) = accepted_stream();
        let mut events = validator.events.clone();
        events.push(event(Event::CompilationEnd(end.clone())));
        end.overall_stream_digest = overall_stream_digest(&events);
        let completed = validator
            .finish(end.clone(), event(Event::CompilationEnd(end)), false)
            .unwrap();
        assert_eq!(completed.owners.len(), 1);
        assert_eq!(completed.owners[0].chunks.len(), 1);
    }

    #[test]
    fn wp35_negative_zero_state() {
        let (validator, mut end) = accepted_stream();
        end.compiler_exit_status = 1;
        assert_eq!(
            validator
                .finish(end.clone(), event(Event::CompilationEnd(end)), false)
                .unwrap_err()
                .code(),
            tonic::Code::FailedPrecondition
        );

        let (mut validator, _) = accepted_stream();
        let bad = OwnerEnd {
            provider_run_id: "run:test".to_owned(),
            compilation_unit_id: "unit:test".to_owned(),
            sequence: 9,
            owner_id: "owner:test".to_owned(),
            family_counts: std::collections::HashMap::new(),
            owner_content_digest: b3("wrong"),
        };
        assert_eq!(
            validator
                .accept_owner_end(bad.clone(), event(Event::OwnerEnd(bad)))
                .unwrap_err()
                .code(),
            tonic::Code::FailedPrecondition
        );
    }

    #[tokio::test]
    async fn wp35_operational_acceptance_handshake_and_cancel_are_single_authority() {
        let (policy, admission, begin) = fixture();
        let (service, _accepted) =
            RustcObservationService::new(policy.clone(), admission.clone()).unwrap();
        let hello = ExtractorHello {
            protocol_major: 1,
            protocol_minor: 0,
            required_feature_bits: 0,
            optional_feature_bits: 0,
            extractor_build: policy.extractor_build.clone(),
            rustc_version: policy.rustc_version.clone(),
            rustc_commit: policy.rustc_commit.clone(),
            toolchain_identity_digest: policy.toolchain_identity_digest.clone(),
            resource_profile_id: admission.resource_profile_id.clone(),
        };
        let ack = service
            .handshake(Request::new(hello))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(
            ack.accepted_resource_profile_id,
            admission.resource_profile_id
        );
        assert_eq!(ack.maximum_outstanding_chunks, MAX_OUTSTANDING_CHUNKS);

        let (commands, _receiver) = mpsc::channel(2);
        service.active.lock().await.insert(
            admission.provider_run_id.clone(),
            ActiveRun {
                compilation_unit_id: begin.compilation_unit_id.clone(),
                commands,
                cancelled: false,
                terminal_state: None,
            },
        );
        let cancellation = service
            .request_cancel(CancelCompilationRequest {
                provider_run_id: admission.provider_run_id,
                compilation_unit_id: begin.compilation_unit_id,
                reason: "superseded".to_owned(),
            })
            .await;
        assert_eq!(
            cancellation.state,
            CancelAcknowledgementState::CancellationRequested as i32
        );
    }
}
