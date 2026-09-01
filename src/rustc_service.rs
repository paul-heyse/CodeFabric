//! Daemon-hosted validation and flow control for the compiler-wrapper protocol.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::future::Future;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use arrow_ipc::reader::StreamReader;
use futures::{Stream, stream};
use prost::Message;
use tokio::sync::{Mutex, mpsc, oneshot};
use tonic::service::InterceptorLayer;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::registries::RustcFeatureMask;
use crate::registries::{PROVIDER_ENTRIES, PROVIDER_RESOURCE_PROFILES};
use crate::rpc::generated::codefabric::provider::v1::{
    CancelAcknowledgement, CancelAcknowledgementState, ChunkRejected, ProviderRunState,
};
use crate::rpc::generated::codefabric::rustc::v1::extraction_event::Event;
use crate::rpc::generated::codefabric::rustc::v1::extractor_command::Command;
use crate::rpc::generated::codefabric::rustc::v1::rustc_extractor_server::RustcExtractor;
use crate::rpc::generated::codefabric::rustc::v1::rustc_extractor_server::RustcExtractorServer;
use crate::rpc::generated::codefabric::rustc::v1::{
    CancelCompilationRequest, CompilationBegin, CompilationEnd, ExtractionEvent, ExtractorCommand,
    ExtractorHello, ExtractorHelloAck, OwnerBegin, OwnerEnd, OwnerObservationChunk,
    OwnerRelationIpcFrame, RejectionRuleErrorCode,
};
use crate::rpc::{AuthorizedUnixStream, SameUserInterceptor, negotiate_feature_bits};

use crate::provider_sandbox::{
    GeneratedSandboxProfile, ProviderSandboxLaunchMaterial, ProviderSandboxLauncher,
    SandboxCapabilityMatrix, SandboxMechanism,
};
use crate::relation_ipc::{
    FlowControlAck, FrameHeader, RelationIpcAssembler, RelationIpcFrame, RelationIpcLimits,
    StreamId, TerminalStatus,
};
use crate::relation_ipc_wire::{
    decode_relation_frame, encode_relation_frame, relation_stream_contract,
};
use crate::rust_compilation_trust::{
    RustCompilationAdmissionProof, RustCompilationCancellationSignal, RustCompilationInputs,
    RustCompilationLaunchPlan, RustCompilationLauncherReceipt, RustCompilationPrivatePaths,
    RustCompilationProtocolBinding, RustCompilationRunRequest, RustCompilationTrustError,
    RustCompilationTrustMode, RustCompilationTrustPolicy, compile_rust_compilation_launch_plan,
    issue_rust_compilation_admission_proof, supervise_rust_compilation,
};
use crate::rustc_relation_schema::{RustcRelation, schema_bundle_digest};

include!("digest_frames.rs");

/// AC-G-31 maximum number of chunks a wrapper may have in flight.
pub const MAX_OUTSTANDING_CHUNKS: u32 = 4;
/// AC-G-31 maximum unacknowledged payload bytes per compilation.
pub const MAX_UNACKNOWLEDGED_BYTES: u64 = 16 * 1024 * 1024;
const MAX_DECODED_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_RELATION_ROWS: u64 = 1_000_000;
const IPC_STREAM_EOS: [u8; 8] = [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0];

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

fn validate_typed_relation_chunk(chunk: &OwnerObservationChunk) -> Result<(), Status> {
    let relation =
        RustcRelation::from_family_code(chunk.observation_family_code).ok_or_else(|| {
            Status::invalid_argument(
                "rustc observation family is not a pinned typed relation contract",
            )
        })?;
    if chunk.payload_reference.is_some()
        || chunk.arrow_ipc.is_empty()
        || chunk.arrow_ipc.len() as u64 > MAX_UNACKNOWLEDGED_BYTES
        || chunk.row_count > MAX_RELATION_ROWS
        || !chunk.arrow_ipc.ends_with(&IPC_STREAM_EOS)
        || chunk.schema_digest != relation.schema_digest()
    {
        return Err(Status::invalid_argument(
            "rustc relation violates its inline Arrow stream contract",
        ));
    }
    crate::relation_ipc::validate_arrow_ipc_profile(&chunk.arrow_ipc).map_err(|error| {
        Status::invalid_argument(format!(
            "rustc relation differs from the pinned Arrow IPC profile: {error}"
        ))
    })?;
    let mut reader =
        StreamReader::try_new(Cursor::new(&chunk.arrow_ipc), None).map_err(|error| {
            Status::invalid_argument(format!("invalid rustc Arrow stream: {error}"))
        })?;
    if reader.schema().as_ref() != relation.schema().as_ref() {
        return Err(Status::invalid_argument(
            "rustc relation schema differs from the application-owned contract",
        ));
    }
    let batch = reader
        .next()
        .transpose()
        .map_err(|error| Status::invalid_argument(format!("invalid rustc Arrow batch: {error}")))?
        .ok_or_else(|| Status::invalid_argument("rustc relation contains no Arrow batch"))?;
    if reader
        .next()
        .transpose()
        .map_err(|error| Status::invalid_argument(format!("invalid rustc Arrow batch: {error}")))?
        .is_some()
        || u64::try_from(batch.num_rows()).unwrap_or(u64::MAX) != chunk.row_count
    {
        return Err(Status::invalid_argument(
            "rustc relation must contain exactly one declared-size Arrow batch",
        ));
    }
    Ok(())
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

/// Compute the stream digest with the terminal digest field cleared.
#[must_use]
pub fn overall_stream_digest(events: &[ExtractionEvent]) -> String {
    let fields = events.iter().map(canonical_event_bytes).collect::<Vec<_>>();
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
            || self.output_schema_bundle_digest != schema_bundle_digest()
            || !valid_sandbox_profile_digest(&self.sandbox_profile_digest)
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
    trust_binding: RustCompilationProtocolBinding,
}

/// Compiler relations qualified by a real, matching untrusted launcher receipt.
///
/// The raw protocol result remains useful for diagnostics, but only this type can enter exact
/// provider admission. Construction consumes the raw result so the receipt cannot be detached
/// after validation.
#[derive(Clone, Debug, PartialEq)]
pub struct TrustQualifiedRustcCompilation {
    accepted: AcceptedRustcCompilation,
    trust_proof: RustCompilationAdmissionProof,
}

impl TrustQualifiedRustcCompilation {
    /// Join one daemon-accepted compiler stream to its independently observed launcher receipt.
    ///
    /// # Errors
    ///
    /// Rejects a receipt from another plan/run or a compiler terminal that is not the contained
    /// successful terminal required for semantic registration.
    pub fn try_new(
        accepted: AcceptedRustcCompilation,
        trust_proof: RustCompilationAdmissionProof,
    ) -> Result<Self, RustCompilationTrustError> {
        let qualified = Self {
            accepted,
            trust_proof,
        };
        qualified.validate()?;
        Ok(qualified)
    }

    /// Revalidate the launch-plan, launcher-terminal, and compiler-stream join immediately before
    /// provider registration.
    ///
    /// # Errors
    ///
    /// Rejects changed protocol pins, a receipt/proof detached from its plan, or either terminal
    /// ceasing to represent one proved contained success.
    pub fn validate(&self) -> Result<(), RustCompilationTrustError> {
        let admission = &self.accepted.admission;
        let proof = &self.trust_proof;
        self.accepted.trust_binding.validate_untrusted_protocol(
            &admission.provider_run_id,
            &admission.workspace_id,
            &admission.analysis_context_id,
            admission.source_generation,
            &admission.context_manifest_digest,
            &admission.source_snapshot_manifest_digest,
            &admission.resource_profile_id,
            &proof.provenance().sandbox_profile_digest,
            &proof.provenance().toolchain_digest,
        )?;
        let compiler_succeeded = ProviderRunState::try_from(self.accepted.end.terminal_state)
            .is_ok_and(|state| state == ProviderRunState::Succeeded)
            && self.accepted.end.compiler_exit_status == 0;
        let launcher_succeeded = proof.terminal().terminal_state
            == crate::rust_compilation_trust::RustCompilationTerminalState::Succeeded
            && proof.terminal().exit_code == Some(0)
            && proof.terminal().accounting_quality
                == crate::rust_compilation_trust::RustCompilationAccountingQuality::KernelComplete
            && proof.terminal().process_sample_count > 0
            && proof.terminal().process_group_empty
            && proof.terminal().output_manifest_digest.is_some();
        let capability = proof.capability();
        let provenance = proof.provenance();
        if self.accepted.trust_binding != *proof.protocol_binding()
            || !compiler_succeeded
            || !launcher_succeeded
            || capability.workspace_id != admission.workspace_id
            || capability.provider_run_id != admission.provider_run_id
            || capability.trust_mode
                != crate::rust_compilation_trust::RustCompilationTrustMode::UntrustedSandboxed
            || capability.trust_state
                != crate::rust_compilation_trust::RustCompilationTrustState::ProvedUntrustedContainment
            || !capability.available
            || capability.degraded
            || capability.launcher_receipt_digest != proof.launcher_receipt_digest()
            || provenance.workspace_id != admission.workspace_id
            || provenance.provider_run_id != admission.provider_run_id
            || provenance.source_generation != admission.source_generation
            || provenance.source_snapshot_digest
                != self.accepted.trust_binding.source_snapshot_digest()
            || provenance.exact_toolchain_release
                != self.accepted.trust_binding.exact_toolchain_release()
            || provenance.plan_digest != self.accepted.trust_binding.plan_digest()
            || provenance.launcher_receipt_digest != proof.launcher_receipt_digest()
        {
            return Err(RustCompilationTrustError::CompilerObservationBindingMismatch);
        }
        Ok(())
    }

    #[must_use]
    pub const fn accepted(&self) -> &AcceptedRustcCompilation {
        &self.accepted
    }

    #[must_use]
    pub const fn trust_proof(&self) -> &RustCompilationAdmissionProof {
        &self.trust_proof
    }

    #[cfg(test)]
    pub(crate) fn accepted_mut_for_test(&mut self) -> &mut AcceptedRustcCompilation {
        &mut self.accepted
    }

    #[cfg(test)]
    pub(crate) fn test_only(accepted: AcceptedRustcCompilation) -> Self {
        let proof = RustCompilationAdmissionProof::test_only(accepted.trust_binding.clone());
        Self::try_new(accepted, proof).expect("test compiler stream and trust proof are aligned")
    }
}

#[cfg(test)]
impl AcceptedRustcCompilation {
    pub(crate) fn test_only(
        admission: RustcRunAdmission,
        begin: CompilationBegin,
        owners: Vec<AcceptedRustcOwner>,
        end: CompilationEnd,
    ) -> Self {
        let trust_binding = RustCompilationProtocolBinding::test_only(
            &admission.provider_run_id,
            &admission.workspace_id,
            &admission.analysis_context_id,
            admission.source_generation,
            &admission.context_manifest_digest,
            &admission.source_snapshot_manifest_digest,
            &admission.resource_profile_id,
            &digest(b"test-sandbox"),
            &digest(b"test-toolchain"),
        );
        Self {
            admission,
            begin,
            owners,
            end,
            trust_binding,
        }
    }
}

#[derive(Clone, Debug)]
struct ActiveRun {
    compilation_unit_id: String,
    commands: Option<mpsc::Sender<Result<ExtractorCommand, Status>>>,
    cancelled: bool,
    terminal_state: Option<ProviderRunState>,
}

#[derive(Debug)]
struct OpenOwner {
    begin: OwnerBegin,
    expected_families: BTreeSet<u32>,
    observed_counts: BTreeMap<u32, u64>,
    chunks: Vec<OwnerObservationChunk>,
    relation_by_stream: BTreeMap<StreamId, RustcRelation>,
    logical_sequence_by_stream: BTreeMap<StreamId, u64>,
    next_ack_sequence: BTreeMap<StreamId, u64>,
    assembler: RelationIpcAssembler,
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
    trust_binding: RustCompilationProtocolBinding,
}

impl RunValidator {
    fn new(
        admission: RustcRunAdmission,
        policy: RustcProtocolPolicy,
        begin: CompilationBegin,
        first_event: ExtractionEvent,
        trust_binding: RustCompilationProtocolBinding,
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
            trust_binding,
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
        if expected_families.contains(&0)
            || expected_families
                .iter()
                .any(|family| RustcRelation::from_family_code(*family).is_none())
        {
            return Err(Status::invalid_argument(
                "compiler owner declares an unpinned observation family",
            ));
        }
        if expected_families.len() != begin.expected_observation_family_codes.len()
            || !self.owner_ids.insert(owner.owner_id.clone())
        {
            return Err(Status::already_exists(
                "compiler owner or observation family is duplicated",
            ));
        }
        let limits = RelationIpcLimits {
            max_registered_streams: expected_families.len(),
            max_frames_per_stream: 64,
            max_payload_bytes_per_frame: crate::relation_ipc_contract::RELATION_IPC_FRAGMENT_BYTES,
            max_payload_bytes_per_stream: usize::try_from(MAX_UNACKNOWLEDGED_BYTES)
                .unwrap_or(usize::MAX),
            max_total_payload_bytes: 256 * 1024 * 1024,
            initial_credit_bytes: usize::try_from(MAX_UNACKNOWLEDGED_BYTES).unwrap_or(usize::MAX),
            max_credit_bytes: usize::try_from(MAX_UNACKNOWLEDGED_BYTES).unwrap_or(usize::MAX),
            max_batches_per_stream: 1,
            max_rows_per_stream: usize::try_from(MAX_RELATION_ROWS).unwrap_or(usize::MAX),
            max_remainders_per_stream: 64,
        };
        let mut assembler = RelationIpcAssembler::new(limits)
            .map_err(|error| Status::internal(format!("invalid relation limits: {error}")))?;
        let mut relation_by_stream = BTreeMap::new();
        let mut next_ack_sequence = BTreeMap::new();
        for family in &expected_families {
            let relation = RustcRelation::from_family_code(*family)
                .expect("expected family registry checked above");
            let contract = relation_stream_contract(
                relation.relation_id(),
                relation.schema(),
                &self.admission.provider_run_id,
                &owner.owner_id,
                &self.admission.source_snapshot_manifest_digest,
                &self.admission.context_manifest_digest,
                1,
            )
            .map_err(Status::invalid_argument)?;
            relation_by_stream.insert(contract.identity.stream_id, relation);
            next_ack_sequence.insert(contract.identity.stream_id, 0);
            assembler.register_contract(contract).map_err(|error| {
                Status::invalid_argument(format!("invalid relation contract: {error}"))
            })?;
        }
        self.next_sequence += 1;
        self.events.push(event);
        self.open_owner = Some(OpenOwner {
            begin,
            expected_families,
            observed_counts: BTreeMap::new(),
            chunks: Vec::new(),
            relation_by_stream,
            logical_sequence_by_stream: BTreeMap::new(),
            next_ack_sequence,
            assembler,
        });
        Ok(())
    }

    fn accept_relation_frame(
        &mut self,
        relation_frame: OwnerRelationIpcFrame,
        event: ExtractionEvent,
        cancelled: bool,
    ) -> Result<Option<ExtractorCommand>, Status> {
        self.validate_header(
            &relation_frame.provider_run_id,
            &relation_frame.compilation_unit_id,
            relation_frame.sequence,
        )?;
        let owner = self
            .open_owner
            .as_mut()
            .ok_or_else(|| Status::failed_precondition("relation frame has no open owner"))?;
        let owner_id = owner
            .begin
            .owner
            .as_ref()
            .map(|key| key.owner_id.as_str())
            .unwrap_or_default();
        let relation = RustcRelation::from_family_code(relation_frame.observation_family_code)
            .ok_or_else(|| Status::invalid_argument("relation frame family is unregistered"))?;
        if relation_frame.owner_id != owner_id
            || !owner
                .expected_families
                .contains(&relation_frame.observation_family_code)
        {
            return Err(Status::invalid_argument(
                "relation frame owner or family differs",
            ));
        }
        let frame = decode_relation_frame(
            relation_frame
                .frame
                .ok_or_else(|| Status::invalid_argument("relation frame is absent"))?,
        )
        .map_err(|error| Status::invalid_argument(format!("invalid relation frame: {error}")))?;
        if matches!(frame, RelationIpcFrame::FlowControlAck(_)) {
            return Err(Status::invalid_argument(
                "provider sent a receiver-direction acknowledgement",
            ));
        }
        let frame_header = frame.header();
        let stream_id = frame_header.identity.stream_id;
        if owner.relation_by_stream.get(&stream_id) != Some(&relation) {
            return Err(Status::failed_precondition(
                "relation frame differs from the application-owned contract",
            ));
        }
        if matches!(frame, RelationIpcFrame::Open(_))
            && owner
                .logical_sequence_by_stream
                .insert(stream_id, relation_frame.sequence)
                .is_some()
        {
            return Err(Status::already_exists("relation stream open is duplicated"));
        }
        let payload = match &frame {
            RelationIpcFrame::Payload(payload) => Some((
                payload.header.identity,
                payload.header.sequence,
                payload.payload.len(),
            )),
            _ => None,
        };
        let assembled = owner.assembler.push(frame).map_err(|error| {
            Status::data_loss(format!("relation stream failed closed: {error}"))
        })?;
        let acknowledgement = if let Some((identity, payload_sequence, payload_bytes)) = payload {
            let ack_sequence = owner
                .next_ack_sequence
                .get(&stream_id)
                .copied()
                .ok_or_else(|| Status::internal("relation acknowledgement state is absent"))?;
            let acknowledgement = if cancelled {
                RelationIpcFrame::FlowControlAck(FlowControlAck {
                    header: FrameHeader::current(identity, ack_sequence),
                    acknowledged_sequence: None,
                    released_bytes: 0,
                    cancelled: true,
                })
            } else {
                RelationIpcFrame::FlowControlAck(FlowControlAck {
                    header: FrameHeader::current(identity, ack_sequence),
                    acknowledged_sequence: Some(payload_sequence),
                    released_bytes: u64::try_from(payload_bytes).unwrap_or(u64::MAX),
                    cancelled: false,
                })
            };
            match owner.assembler.push(acknowledgement.clone()) {
                Ok(_) if !cancelled => {}
                Err(error)
                    if cancelled
                        && error.kind == crate::relation_ipc::RelationIpcErrorKind::Cancelled => {}
                Ok(_) => {
                    return Err(Status::internal(
                        "local cancellation proof did not terminate the relation stream",
                    ));
                }
                Err(error) => {
                    return Err(Status::internal(format!(
                        "local relation credit proof failed: {error}"
                    )));
                }
            }
            if !cancelled {
                *owner
                    .next_ack_sequence
                    .get_mut(&stream_id)
                    .expect("registered stream has acknowledgement state") += 1;
            }
            Some(ExtractorCommand {
                command: Some(Command::RelationIpcAck(
                    encode_relation_frame(&acknowledgement).map_err(Status::internal)?,
                )),
            })
        } else {
            None
        };
        if let Some(assembled) = assembled {
            if assembled.trailer.status != TerminalStatus::Complete || assembled.batches.len() != 1
            {
                return Err(Status::failed_precondition(
                    "successful rustc relation is not complete or single-batch",
                ));
            }
            let logical_sequence = owner
                .logical_sequence_by_stream
                .remove(&stream_id)
                .ok_or_else(|| Status::failed_precondition("relation terminal lacks its open"))?;
            let row_count = u64::try_from(assembled.batches[0].num_rows()).unwrap_or(u64::MAX);
            let arrow_ipc = assembled.ipc_bytes;
            let chunk = OwnerObservationChunk {
                provider_run_id: relation_frame.provider_run_id,
                compilation_unit_id: relation_frame.compilation_unit_id,
                sequence: logical_sequence,
                owner_id: relation_frame.owner_id,
                observation_family_code: relation_frame.observation_family_code,
                chunk_digest: arrow_chunk_digest(&arrow_ipc),
                arrow_ipc,
                payload_reference: None,
                schema_digest: relation.schema_digest(),
                row_count,
            };
            validate_typed_relation_chunk(&chunk)?;
            if owner
                .observed_counts
                .insert(chunk.observation_family_code, chunk.row_count)
                .is_some()
            {
                return Err(Status::already_exists("relation terminal is duplicated"));
            }
            owner.chunks.push(chunk);
        }
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| Status::resource_exhausted("compiler event sequence is exhausted"))?;
        self.events.push(event);
        Ok(acknowledgement)
    }

    fn cancellation_ack_for_relation(
        &self,
        relation_frame: &OwnerRelationIpcFrame,
    ) -> Option<ExtractorCommand> {
        let frame = decode_relation_frame(relation_frame.frame.clone()?).ok()?;
        let identity = frame.header().identity;
        let ack_sequence = self
            .open_owner
            .as_ref()?
            .next_ack_sequence
            .get(&identity.stream_id)
            .copied()?;
        let cancellation = RelationIpcFrame::FlowControlAck(FlowControlAck {
            header: FrameHeader::current(identity, ack_sequence),
            acknowledged_sequence: None,
            released_bytes: 0,
            cancelled: true,
        });
        Some(ExtractorCommand {
            command: Some(Command::RelationIpcAck(
                encode_relation_frame(&cancellation).ok()?,
            )),
        })
    }

    fn accept_owner_end(&mut self, end: OwnerEnd, event: ExtractionEvent) -> Result<(), Status> {
        self.validate_header(&end.provider_run_id, &end.compilation_unit_id, end.sequence)?;
        let owner = self
            .open_owner
            .take()
            .ok_or_else(|| Status::failed_precondition("owner end has no open owner"))?;
        owner.assembler.finish().map_err(|error| {
            Status::data_loss(format!(
                "owner ended before every relation terminal: {error}"
            ))
        })?;
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
        if self.open_owner.is_some() && !cancelled {
            return Err(Status::failed_precondition(
                "compilation ended with an open owner",
            ));
        }
        if cancelled {
            self.open_owner.take();
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
            trust_binding: self.trust_binding,
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
    trust_binding: RustCompilationProtocolBinding,
    active: Arc<Mutex<BTreeMap<String, ActiveRun>>>,
    accepted: mpsc::Sender<AcceptedRustcCompilation>,
    supervisor_cancellation: RustCompilationCancellationSignal,
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
        trust_binding: RustCompilationProtocolBinding,
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
        trust_binding
            .validate_untrusted_protocol(
                &admission.provider_run_id,
                &admission.workspace_id,
                &admission.analysis_context_id,
                admission.source_generation,
                &admission.context_manifest_digest,
                &admission.source_snapshot_manifest_digest,
                &admission.resource_profile_id,
                &policy.sandbox_profile_digest,
                &policy.toolchain_identity_digest,
            )
            .map_err(|error| {
                Status::failed_precondition(format!(
                    "rustc launch-plan binding differs from protocol admission: {error}"
                ))
            })?;
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
                trust_binding,
                active: Arc::new(Mutex::new(BTreeMap::new())),
                accepted,
                supervisor_cancellation: RustCompilationCancellationSignal::default(),
            },
            receiver,
        ))
    }

    /// Run-wide cancellation edge consumed by the Cargo process-group supervisor.
    #[must_use]
    pub fn supervisor_cancellation_signal(&self) -> RustCompilationCancellationSignal {
        self.supervisor_cancellation.clone()
    }

    /// Request cancellation through the existing reverse command stream.
    pub async fn request_cancel(&self, request: CancelCompilationRequest) -> CancelAcknowledgement {
        if request.provider_run_id != self.admission.provider_run_id {
            return cancellation_ack(
                request.provider_run_id,
                CancelAcknowledgementState::NotFound,
                None,
            );
        }
        let command_sender = {
            let mut active = self.active.lock().await;
            let Some(run) = active.get_mut(&request.compilation_unit_id) else {
                return cancellation_ack(
                    request.provider_run_id,
                    CancelAcknowledgementState::NotFound,
                    None,
                );
            };
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
                run.commands.clone()
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

    /// Request cooperative cancellation for every active compilation unit in one provider run.
    ///
    /// The launcher still owns process-group escalation. This method first exercises the existing
    /// reverse command stream so each wrapper can close its protocol stream deliberately.
    pub async fn request_cancel_run(&self, provider_run_id: &str) -> usize {
        if provider_run_id != self.admission.provider_run_id {
            return 0;
        }
        let commands = {
            let mut active = self.active.lock().await;
            active
                .values_mut()
                .filter_map(|run| {
                    if run.terminal_state.is_some() || run.cancelled {
                        return None;
                    }
                    run.cancelled = true;
                    run.commands.clone().map(|sender| {
                        (
                            sender,
                            CancelCompilationRequest {
                                provider_run_id: provider_run_id.to_owned(),
                                compilation_unit_id: run.compilation_unit_id.clone(),
                                reason: "provider-run-cancelled".to_owned(),
                            },
                        )
                    })
                })
                .collect::<Vec<_>>()
        };
        let requested = commands.len();
        for (sender, request) in commands {
            let _ = sender
                .send(Ok(ExtractorCommand {
                    command: Some(Command::Cancel(request)),
                }))
                .await;
        }
        // The reverse stream is cooperative and unit-scoped. Publish the shared signal only
        // after every active unit has received its command; the supervisor then owns bounded
        // escalation for the complete Cargo process group.
        self.supervisor_cancellation.request();
        requested
    }

    /// Snapshot terminal state by compilation-unit identity.
    #[must_use]
    pub async fn terminal_states(&self) -> BTreeMap<String, ProviderRunState> {
        self.active
            .lock()
            .await
            .iter()
            .filter_map(|(unit_id, run)| run.terminal_state.map(|state| (unit_id.clone(), state)))
            .collect()
    }

    async fn run_cancelled(&self, compilation_unit_id: &str) -> bool {
        self.active
            .lock()
            .await
            .get(compilation_unit_id)
            .is_some_and(|run| run.cancelled)
    }

    async fn mark_terminal(&self, compilation_unit_id: &str, state: ProviderRunState) {
        if let Some(run) = self.active.lock().await.get_mut(compilation_unit_id) {
            run.terminal_state = Some(state);
            run.commands.take();
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
        let compilation_unit_id = begin.compilation_unit_id.clone();
        let mut validator = RunValidator::new(
            self.admission.clone(),
            self.policy.clone(),
            begin.clone(),
            first_event,
            self.trust_binding.clone(),
        )?;
        {
            let mut active = self.active.lock().await;
            match active.entry(compilation_unit_id.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(ActiveRun {
                        compilation_unit_id: begin.compilation_unit_id.clone(),
                        commands: Some(output.clone()),
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

        let result = async {
            while let Some(event) = self.next_event(&mut input).await? {
                let cancelled = self.run_cancelled(&compilation_unit_id).await;
                match event.event.clone() {
                    Some(Event::OwnerBegin(begin)) => {
                        validator.accept_owner_begin(begin, event)?;
                    }
                    Some(Event::OwnerRelationIpcFrame(frame)) => {
                        match validator.accept_relation_frame(frame.clone(), event, cancelled) {
                            Ok(Some(command)) => output
                                .send(Ok(command))
                                .await
                                .map_err(|_| Status::cancelled("compiler command stream closed"))?,
                            Ok(None) => {}
                            Err(error) => {
                                if let Some(command) =
                                    validator.cancellation_ack_for_relation(&frame)
                                {
                                    let _ = output.send(Ok(command)).await;
                                }
                                return Err(error);
                            }
                        }
                    }
                    Some(Event::OwnerObservationChunk(chunk)) => {
                        let _ = output
                            .send(Ok(ExtractorCommand {
                                command: Some(Command::ChunkRejected(ChunkRejected {
                                    sequence: chunk.sequence,
                                    error_code: "LEGACY_WHOLE_RELATION_CHUNK_REJECTED".to_owned(),
                                })),
                            }))
                            .await;
                        return Err(Status::failed_precondition(
                            "legacy whole-relation Arrow chunks are no longer admitted",
                        ));
                    }
                    Some(Event::OwnerEnd(end)) => {
                        validator.accept_owner_end(end, event)?;
                    }
                    Some(Event::CompilationEnd(end)) => {
                        let completed = validator.finish(end, event, cancelled)?;
                        let terminal = ProviderRunState::try_from(completed.end.terminal_state)
                            .unwrap_or(ProviderRunState::ProtocolError);
                        if terminal == ProviderRunState::Succeeded {
                            self.accepted.send(completed).await.map_err(|_| {
                                Status::unavailable("canonical ingest sink is closed")
                            })?;
                        }
                        self.mark_terminal(&compilation_unit_id, terminal).await;
                        return Ok(());
                    }
                    Some(Event::CompilationBegin(_)) => {
                        return Err(Status::failed_precondition(
                            "CompilationBegin may appear only once",
                        ));
                    }
                    None => return Err(Status::invalid_argument("compiler event is empty")),
                }
            }
            Err(Status::data_loss(
                "compiler stream ended before CompilationEnd",
            ))
        }
        .await;
        if result.is_err() {
            self.mark_terminal(&compilation_unit_id, ProviderRunState::ProtocolError)
                .await;
        }
        result
    }
}

/// Closed daemon inputs for one untrusted Rust semantic-provider lifecycle.
///
/// The caller supplies independently resolved source, context, toolchain, and protocol inputs;
/// this transaction compiles their exact launch plan and never accepts a preconstructed compiler
/// response or a trusted-local fallback.
pub struct UntrustedRustcProviderLifecycle<'a> {
    pub trust_policy: &'a RustCompilationTrustPolicy,
    pub sandbox_capabilities: &'a SandboxCapabilityMatrix,
    pub sandbox_profile: &'a GeneratedSandboxProfile,
    pub compilation_inputs: &'a RustCompilationInputs,
    pub private_paths: &'a RustCompilationPrivatePaths,
    pub compilation_request: &'a RustCompilationRunRequest,
    pub protocol_policy: RustcProtocolPolicy,
    pub run_admission: RustcRunAdmission,
    pub allowed_uid: u32,
    pub launch_material: ProviderSandboxLaunchMaterial<'a>,
}

enum OwnedRustcLaunchMaterial {
    DarwinProfile(PathBuf),
    LinuxSeccomp(fs::File),
}

impl OwnedRustcLaunchMaterial {
    fn clone_untrusted(
        mechanism: SandboxMechanism,
        material: ProviderSandboxLaunchMaterial<'_>,
    ) -> Result<Self, RustcProviderLifecycleError> {
        match (mechanism, material) {
            (
                SandboxMechanism::DarwinSeatbelt,
                ProviderSandboxLaunchMaterial::DarwinProfile(path),
            ) => Ok(Self::DarwinProfile(path.to_path_buf())),
            (
                SandboxMechanism::LinuxBubblewrap,
                ProviderSandboxLaunchMaterial::LinuxSeccomp(file),
            ) => Ok(Self::LinuxSeccomp(file.try_clone().map_err(|source| {
                RustcProviderLifecycleError::LaunchMaterialIo { source }
            })?)),
            _ => Err(RustcProviderLifecycleError::LaunchMaterialMismatch),
        }
    }

    fn as_borrowed(&self) -> ProviderSandboxLaunchMaterial<'_> {
        match self {
            Self::DarwinProfile(path) => ProviderSandboxLaunchMaterial::DarwinProfile(path),
            Self::LinuxSeccomp(file) => ProviderSandboxLaunchMaterial::LinuxSeccomp(file),
        }
    }
}

/// Run one complete untrusted Rust provider transaction from launch-plan compilation through
/// receipt-qualified semantic output.
///
/// The private UDS is bound before Cargo starts, compiler observations are drained concurrently,
/// and no raw result escapes this function. Every observed compilation unit must end successfully
/// before the exact launcher receipt can qualify its Arrow relations.
///
/// # Errors
///
/// Rejects unavailable containment/accounting, trusted-local or mismatched launch material,
/// source/context/toolchain drift, missing or failed protocol terminals, transport failure, and
/// any receipt/compiler-stream mismatch. All failures remove the private socket and return no
/// semantic result.
pub async fn run_untrusted_rustc_provider_lifecycle(
    lifecycle: UntrustedRustcProviderLifecycle<'_>,
) -> Result<Vec<TrustQualifiedRustcCompilation>, RustcProviderLifecycleError> {
    if lifecycle.trust_policy.trust_mode != RustCompilationTrustMode::UntrustedSandboxed {
        return Err(RustCompilationTrustError::UntrustedAdmissionRequired.into());
    }
    let plan = compile_rust_compilation_launch_plan(
        lifecycle.trust_policy,
        lifecycle.sandbox_capabilities,
        lifecycle.sandbox_profile,
        lifecycle.compilation_inputs,
        lifecycle.private_paths,
        lifecycle.compilation_request,
        None,
    )?;
    let launch_material = OwnedRustcLaunchMaterial::clone_untrusted(
        lifecycle.sandbox_profile.mechanism,
        lifecycle.launch_material,
    )?;
    let capabilities = lifecycle.sandbox_capabilities.clone();
    let profile = lifecycle.sandbox_profile.clone();
    let private_paths = lifecycle.private_paths.clone();
    execute_prepared_rustc_lifecycle(
        plan,
        lifecycle.private_paths.extractor_socket_path.clone(),
        lifecycle.protocol_policy,
        lifecycle.run_admission,
        lifecycle.allowed_uid,
        move |plan, cancellation| async move {
            tokio::task::spawn_blocking(move || {
                let launcher = ProviderSandboxLauncher::new(capabilities);
                supervise_rust_compilation(
                    &plan,
                    &private_paths,
                    &launcher,
                    &profile,
                    launch_material.as_borrowed(),
                    &cancellation,
                )
            })
            .await
            .map_err(RustcProviderLifecycleError::SupervisorTask)?
            .map_err(Into::into)
        },
    )
    .await
}

async fn execute_prepared_rustc_lifecycle<F, Fut>(
    plan: RustCompilationLaunchPlan,
    socket: PathBuf,
    protocol_policy: RustcProtocolPolicy,
    run_admission: RustcRunAdmission,
    allowed_uid: u32,
    supervisor: F,
) -> Result<Vec<TrustQualifiedRustcCompilation>, RustcProviderLifecycleError>
where
    F: FnOnce(RustCompilationLaunchPlan, RustCompilationCancellationSignal) -> Fut,
    Fut: Future<Output = Result<RustCompilationLauncherReceipt, RustcProviderLifecycleError>>,
{
    let binding = plan.protocol_binding()?;
    let (service, mut accepted_receiver) =
        RustcObservationService::new(protocol_policy, run_admission, binding)
            .map_err(RustcProviderLifecycleError::Protocol)?;
    let listener = bind_rustc_uds(&socket)?;
    let monitor = service.clone();
    let cancellation = service.supervisor_cancellation_signal();
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();
    let server_socket = socket.clone();
    let server = tokio::spawn(async move {
        serve_bound_rustc_uds(listener, &server_socket, allowed_uid, service, async move {
            let _ = shutdown_receiver.await;
        })
        .await
    });
    let collector = tokio::spawn(async move {
        let mut accepted = Vec::new();
        while let Some(compilation) = accepted_receiver.recv().await {
            accepted.push(compilation);
        }
        accepted
    });

    let receipt = supervisor(plan.clone(), cancellation).await;
    let _ = shutdown_sender.send(());
    let server_result = server
        .await
        .map_err(RustcProviderLifecycleError::ServerTask)?;
    let terminal_states = monitor.terminal_states().await;
    drop(monitor);
    let mut accepted = collector
        .await
        .map_err(RustcProviderLifecycleError::CollectorTask)?;
    server_result?;
    let receipt = receipt?;

    if terminal_states.is_empty() || accepted.is_empty() {
        return Err(RustcProviderLifecycleError::MissingCompilerTerminal);
    }
    if terminal_states.len() != accepted.len()
        || terminal_states
            .values()
            .any(|state| *state != ProviderRunState::Succeeded)
    {
        return Err(RustcProviderLifecycleError::IncompleteCompilerTerminal);
    }
    accepted.sort_by(|left, right| {
        left.begin
            .compilation_unit_id
            .cmp(&right.begin.compilation_unit_id)
    });
    if accepted
        .windows(2)
        .any(|window| window[0].begin.compilation_unit_id == window[1].begin.compilation_unit_id)
    {
        return Err(RustcProviderLifecycleError::DuplicateCompilationUnit);
    }
    let proof = issue_rust_compilation_admission_proof(&plan, &receipt)?;
    accepted
        .into_iter()
        .map(|compilation| TrustQualifiedRustcCompilation::try_new(compilation, proof.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Closed lifecycle failures. No variant carries partially accepted semantic output.
#[derive(Debug, thiserror::Error)]
pub enum RustcProviderLifecycleError {
    #[error(transparent)]
    Trust(#[from] RustCompilationTrustError),
    #[error("rustc protocol admission failed: {0}")]
    Protocol(Status),
    #[error(transparent)]
    Transport(#[from] RustcTransportError),
    #[error("untrusted rustc launch material does not match the proved sandbox mechanism")]
    LaunchMaterialMismatch,
    #[error("untrusted rustc launch material could not be duplicated: {source}")]
    LaunchMaterialIo {
        #[source]
        source: std::io::Error,
    },
    #[error("rustc supervisor task failed: {0}")]
    SupervisorTask(tokio::task::JoinError),
    #[error("rustc UDS server task failed: {0}")]
    ServerTask(tokio::task::JoinError),
    #[error("rustc observation collector task failed: {0}")]
    CollectorTask(tokio::task::JoinError),
    #[error("rustc launcher completed without a verified compiler terminal")]
    MissingCompilerTerminal,
    #[error("not every observed rustc compilation unit reached one successful terminal")]
    IncompleteCompilerTerminal,
    #[error("rustc lifecycle observed a duplicate compilation-unit identity")]
    DuplicateCompilationUnit,
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
    let listener = bind_rustc_uds(socket)?;
    serve_bound_rustc_uds(listener, socket, allowed_uid, service, shutdown).await
}

fn bind_rustc_uds(socket: &Path) -> Result<tokio::net::UnixListener, RustcTransportError> {
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
    Ok(listener)
}

async fn serve_bound_rustc_uds<F>(
    listener: tokio::net::UnixListener,
    socket: &Path,
    allowed_uid: u32,
    service: RustcObservationService,
    shutdown: F,
) -> Result<(), RustcTransportError>
where
    F: Future<Output = ()> + Send + 'static,
{
    struct RemoveSocket(PathBuf);

    impl Drop for RemoveSocket {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    let _remove_socket = RemoveSocket(socket.to_path_buf());
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
    use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::provider_sandbox::{ProviderTrustProfile, SandboxProbeObservation};
    use crate::rpc::generated::codefabric::provider::v1::BlobReference;
    use crate::rpc::generated::codefabric::rustc::v1::rustc_extractor_client::RustcExtractorClient;
    use crate::rpc::generated::codefabric::rustc::v1::{
        CompilerOwnerKey, DiagnosticSummary, PackageTargetIdentity,
    };
    use crate::rust_compilation_trust::{
        RustCompilationContextPins, RustCompilationResourceLimits, RustExecutableExtensionPolicy,
    };
    use arrow_array::RecordBatch;
    use arrow_ipc::writer::StreamWriter;
    use hyper_util::rt::TokioIo;
    use tempfile::TempDir;
    use tonic::transport::{Channel, Endpoint};
    use tower::service_fn;

    fn b3(value: &str) -> String {
        digest(value.as_bytes())
    }

    fn fixture() -> (RustcProtocolPolicy, RustcRunAdmission, CompilationBegin) {
        let policy = RustcProtocolPolicy {
            daemon_build: "codefabricd-test".to_owned(),
            output_schema_bundle_digest: schema_bundle_digest(),
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

    struct LifecycleHarness {
        _root: TempDir,
        launch_marker: PathBuf,
        inputs: RustCompilationInputs,
        paths: RustCompilationPrivatePaths,
        request: RustCompilationRunRequest,
        capabilities: SandboxCapabilityMatrix,
        profile: GeneratedSandboxProfile,
        trust_policy: RustCompilationTrustPolicy,
        protocol_policy: RustcProtocolPolicy,
        admission: RustcRunAdmission,
        allowed_uid: u32,
    }

    fn lifecycle_limits() -> RustCompilationResourceLimits {
        RustCompilationResourceLimits {
            wall_time_millis: 30_000,
            stdout_bytes: 1024 * 1024,
            stderr_bytes: 1024 * 1024,
            artifact_bytes: 32 * 1024 * 1024,
            process_count: 32,
            file_count: 10_000,
            single_file_bytes: 8 * 1024 * 1024,
            cpu_seconds: 120,
            memory_bytes: 1024 * 1024 * 1024,
            open_files: 128,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn lifecycle_harness() -> LifecycleHarness {
        let root = tempfile::tempdir().unwrap();
        let root_path = fs::canonicalize(root.path()).unwrap();
        let workspace = root_path.join("workspace");
        let dependencies = root_path.join("dependencies");
        let private_parent = root_path.join("private");
        fs::create_dir(&workspace).unwrap();
        fs::create_dir(&dependencies).unwrap();
        let mut private_builder = fs::DirBuilder::new();
        private_builder.mode(0o700).create(&private_parent).unwrap();
        fs::write(
            workspace.join("Cargo.toml"),
            b"[package]\nname='fixture'\nversion='0.0.0'\n",
        )
        .unwrap();
        let toolchain_bin = dependencies.join("toolchain/bin");
        fs::create_dir_all(&toolchain_bin).unwrap();
        fs::create_dir(dependencies.join("cargo-home")).unwrap();
        fs::create_dir(dependencies.join("rustup-home")).unwrap();
        let cargo = toolchain_bin.join("cargo");
        let rustc = toolchain_bin.join("rustc");
        let wrapper = toolchain_bin.join("codefabric-rustc-extractor");
        let launch_marker = root_path.join("host-cargo-bypass-marker");
        fs::write(
            &cargo,
            format!("#!/bin/sh\ntouch '{}'\nexit 0\n", launch_marker.display()),
        )
        .unwrap();
        for executable in [&cargo, &rustc, &wrapper] {
            if !executable.exists() {
                fs::write(executable, b"#!/bin/sh\nexit 0\n").unwrap();
            }
            fs::set_permissions(executable, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let inputs = RustCompilationInputs::inspect(
            &workspace,
            &dependencies,
            &cargo,
            &rustc,
            &wrapper,
            &b3("source-image"),
            &b3("dependencies"),
            &b3("toolchain"),
            "rustc-1.100.0-nightly-2026-08-18",
        )
        .unwrap();
        let paths = RustCompilationPrivatePaths::prepare(&private_parent, "lifecycle").unwrap();
        let request = RustCompilationRunRequest {
            manifest_relative_path: "Cargo.toml".into(),
            package_names: vec!["fixture".into()],
            feature_names: Vec::new(),
            all_targets: true,
            build_scripts_present: true,
            procedural_macros_present: true,
            context: RustCompilationContextPins {
                provider_run_id: "run:test".into(),
                workspace_id: "workspace:test".into(),
                analysis_context_id: "context:test".into(),
                source_generation: 7,
                context_manifest_digest: b3("context"),
                resource_profile_id: "compiler-semantic-standard".into(),
                source_snapshot_manifest_digest: b3("source"),
                cargo_metadata_digest: b3("metadata"),
                cargo_lock_digest: b3("lock"),
                cargo_config_digest: b3("config"),
            },
        };
        let behavior = [
            "launch-confined",
            "leased-workspace-read-allowed",
            "workspace-write-denied",
            "out-of-root-write-denied",
            "live-workspace-read-denied",
            "credential-read-denied",
            "git-read-denied",
            "network-denied",
            "inherited-fd-read-denied",
            "child-process-contained",
            "resource-limit-enforceable",
            "cleanup-escape-denied",
            "output-write-allowed",
        ]
        .into_iter()
        .map(|name| (name.to_owned(), true))
        .collect();
        let capabilities = SandboxCapabilityMatrix::evaluate_for_test(&SandboxProbeObservation {
            mechanism: SandboxMechanism::DarwinSeatbelt,
            executable_path: "/usr/bin/sandbox-exec".into(),
            executable_version: "darwin-seatbelt-system".into(),
            owned_by_root: true,
            executable_mode: 0o755,
            setuid: false,
            behavior,
        });
        let profile = GeneratedSandboxProfile::generate(
            ProviderTrustProfile::UntrustedSandboxed,
            SandboxMechanism::DarwinSeatbelt,
            &inputs.workspace_view,
            &inputs.dependency_view,
            &paths.run_root,
        )
        .unwrap();
        let (mut protocol_policy, admission, _) = fixture();
        protocol_policy.sandbox_profile_digest = profile.sha256_digest.clone();
        let allowed_uid = fs::metadata(&root_path).unwrap().uid();
        LifecycleHarness {
            _root: root,
            launch_marker,
            inputs,
            paths,
            request,
            capabilities,
            profile,
            trust_policy: RustCompilationTrustPolicy::untrusted_sandboxed_v1(
                lifecycle_limits(),
                RustExecutableExtensionPolicy::ExecuteInsideSelectedLauncher,
            ),
            protocol_policy,
            admission,
            allowed_uid,
        }
    }

    fn lifecycle_plan(harness: &LifecycleHarness) -> RustCompilationLaunchPlan {
        compile_rust_compilation_launch_plan(
            &harness.trust_policy,
            &harness.capabilities,
            &harness.profile,
            &harness.inputs,
            &harness.paths,
            &harness.request,
            None,
        )
        .unwrap()
    }

    fn trust_binding(
        policy: &RustcProtocolPolicy,
        admission: &RustcRunAdmission,
    ) -> RustCompilationProtocolBinding {
        RustCompilationProtocolBinding::test_only(
            &admission.provider_run_id,
            &admission.workspace_id,
            &admission.analysis_context_id,
            admission.source_generation,
            &admission.context_manifest_digest,
            &admission.source_snapshot_manifest_digest,
            &admission.resource_profile_id,
            &policy.sandbox_profile_digest,
            &policy.toolchain_identity_digest,
        )
    }

    fn event(event: Event) -> ExtractionEvent {
        ExtractionEvent { event: Some(event) }
    }

    fn typed_relation_chunk(relation: RustcRelation) -> OwnerObservationChunk {
        let schema = relation.schema();
        let batch = RecordBatch::new_empty(Arc::clone(&schema));
        let mut arrow_ipc = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut arrow_ipc, &schema).unwrap();
            writer.write(&batch).unwrap();
            writer.finish().unwrap();
        }
        OwnerObservationChunk {
            provider_run_id: "run:test".to_owned(),
            compilation_unit_id: "unit:test".to_owned(),
            sequence: 2,
            owner_id: "owner:test".to_owned(),
            observation_family_code: relation.family_code(),
            schema_digest: relation.schema_digest(),
            row_count: 0,
            chunk_digest: arrow_chunk_digest(&arrow_ipc),
            arrow_ipc,
            payload_reference: None,
        }
    }

    fn owner_begin(family_code: u32) -> OwnerBegin {
        OwnerBegin {
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
            expected_observation_family_codes: vec![family_code],
        }
    }

    fn accepted_stream() -> (RunValidator, CompilationEnd) {
        let (policy, admission, begin) = fixture();
        let first = event(Event::CompilationBegin(begin.clone()));
        let binding = trust_binding(&policy, &admission);
        let mut validator = RunValidator::new(admission, policy, begin, first, binding).unwrap();
        let relation = RustcRelation::MirBody;
        let owner_begin = owner_begin(relation.family_code());
        validator
            .accept_owner_begin(
                owner_begin.clone(),
                event(Event::OwnerBegin(owner_begin.clone())),
            )
            .unwrap();
        let chunk = typed_relation_chunk(relation);
        let identity = crate::relation_ipc_contract::relation_wire_identity(
            relation.relation_id(),
            &relation.schema_digest(),
            &validator.admission.provider_run_id,
            "owner:test",
            &validator.admission.source_snapshot_manifest_digest,
            &validator.admission.context_manifest_digest,
        )
        .unwrap();
        let frames = crate::relation_ipc_proto::encode_relation_frames(
            identity,
            &chunk.arrow_ipc,
            1,
            chunk.row_count,
            &crate::relation_ipc_proto::RelationCoverage::complete(1),
        )
        .unwrap();
        for (offset, frame) in frames.into_iter().enumerate() {
            let relation_frame = OwnerRelationIpcFrame {
                provider_run_id: "run:test".to_owned(),
                compilation_unit_id: "unit:test".to_owned(),
                sequence: 2 + u64::try_from(offset).unwrap(),
                owner_id: "owner:test".to_owned(),
                observation_family_code: relation.family_code(),
                frame: Some(frame),
            };
            let command = validator
                .accept_relation_frame(
                    relation_frame.clone(),
                    event(Event::OwnerRelationIpcFrame(relation_frame)),
                    false,
                )
                .unwrap();
            if let Some(command) = command {
                let Some(Command::RelationIpcAck(acknowledgement)) = command.command else {
                    panic!("payload credit uses the relation acknowledgement surface")
                };
                let acknowledgement =
                    crate::relation_ipc_proto::decode_flow_control_ack(&acknowledgement).unwrap();
                assert_eq!(acknowledgement.header.identity, identity);
                assert!(!acknowledgement.cancelled);
            }
        }
        let chunk = validator
            .open_owner
            .as_ref()
            .unwrap()
            .chunks
            .first()
            .unwrap()
            .clone();
        let owner_end = OwnerEnd {
            provider_run_id: "run:test".to_owned(),
            compilation_unit_id: "unit:test".to_owned(),
            sequence: validator.next_sequence,
            owner_id: "owner:test".to_owned(),
            family_counts: [(relation.family_code(), 0)].into_iter().collect(),
            owner_content_digest: owner_content_digest(&owner_begin, &[chunk]),
        };
        validator
            .accept_owner_end(owner_end.clone(), event(Event::OwnerEnd(owner_end)))
            .unwrap();
        let compilation_end = CompilationEnd {
            provider_run_id: "run:test".to_owned(),
            compilation_unit_id: "unit:test".to_owned(),
            sequence: validator.next_sequence,
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

    fn completed_event_stream() -> Vec<ExtractionEvent> {
        completed_event_stream_for_generation(7)
    }

    fn completed_event_stream_for_generation(source_generation: u64) -> Vec<ExtractionEvent> {
        let (validator, mut end) = accepted_stream();
        let mut events = validator.events.clone();
        let Some(Event::CompilationBegin(begin)) = events[0].event.as_mut() else {
            panic!("accepted fixture begins with the compiler begin event")
        };
        begin.source_generation = source_generation;
        let mut digest_events = events.clone();
        digest_events.push(event(Event::CompilationEnd(end.clone())));
        end.overall_stream_digest = overall_stream_digest(&digest_events);
        events.push(event(Event::CompilationEnd(end)));
        events
    }

    async fn send_lifecycle_events(
        socket: PathBuf,
        policy: RustcProtocolPolicy,
        admission: RustcRunAdmission,
        events: Vec<ExtractionEvent>,
    ) -> Result<(), Status> {
        let channel: Channel = Endpoint::from_static("http://[::]:50051")
            .connect_with_connector(service_fn(move |_| {
                let socket = socket.clone();
                async move {
                    tokio::net::UnixStream::connect(socket)
                        .await
                        .map(TokioIo::new)
                }
            }))
            .await
            .expect("bound rustc lifecycle socket accepts the exact same-user client");
        let mut client = RustcExtractorClient::new(channel);
        client
            .handshake(ExtractorHello {
                protocol_major: 1,
                protocol_minor: 0,
                required_feature_bits: 0,
                optional_feature_bits: 0,
                extractor_build: policy.extractor_build,
                rustc_version: policy.rustc_version,
                rustc_commit: policy.rustc_commit,
                toolchain_identity_digest: policy.toolchain_identity_digest,
                resource_profile_id: admission.resource_profile_id,
            })
            .await?;
        let mut commands = client
            .observe(tokio_stream::iter(events))
            .await?
            .into_inner();
        while commands.message().await?.is_some() {}
        Ok(())
    }

    async fn execute_test_lifecycle(
        harness: &LifecycleHarness,
        events: Vec<ExtractionEvent>,
        tolerate_stream_failure: bool,
    ) -> Result<Vec<TrustQualifiedRustcCompilation>, RustcProviderLifecycleError> {
        let plan = lifecycle_plan(harness);
        let socket = harness.paths.extractor_socket_path.clone();
        let policy = harness.protocol_policy.clone();
        let admission = harness.admission.clone();
        execute_prepared_rustc_lifecycle(
            plan,
            harness.paths.extractor_socket_path.clone(),
            harness.protocol_policy.clone(),
            harness.admission.clone(),
            harness.allowed_uid,
            move |plan, _cancellation| async move {
                let stream_result = send_lifecycle_events(socket, policy, admission, events).await;
                if !tolerate_stream_failure {
                    stream_result.map_err(RustcProviderLifecycleError::Protocol)?;
                }
                RustCompilationLauncherReceipt::test_only_contained_success(&plan)
                    .map_err(Into::into)
            },
        )
        .await
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
    fn relation_payload_cancellation_is_acknowledged_and_never_reconciled() {
        let (policy, admission, begin) = fixture();
        let first = event(Event::CompilationBegin(begin.clone()));
        let binding = trust_binding(&policy, &admission);
        let mut validator = RunValidator::new(admission, policy, begin, first, binding).unwrap();
        let relation = RustcRelation::MirBody;
        let owner_begin = owner_begin(relation.family_code());
        validator
            .accept_owner_begin(owner_begin.clone(), event(Event::OwnerBegin(owner_begin)))
            .unwrap();
        let chunk = typed_relation_chunk(relation);
        let identity = crate::relation_ipc_contract::relation_wire_identity(
            relation.relation_id(),
            &relation.schema_digest(),
            &validator.admission.provider_run_id,
            "owner:test",
            &validator.admission.source_snapshot_manifest_digest,
            &validator.admission.context_manifest_digest,
        )
        .unwrap();
        let frames = crate::relation_ipc_proto::encode_relation_frames(
            identity,
            &chunk.arrow_ipc,
            1,
            0,
            &crate::relation_ipc_proto::RelationCoverage::complete(1),
        )
        .unwrap();
        let mut frames = frames.into_iter();
        let open = OwnerRelationIpcFrame {
            provider_run_id: "run:test".to_owned(),
            compilation_unit_id: "unit:test".to_owned(),
            sequence: 2,
            owner_id: "owner:test".to_owned(),
            observation_family_code: relation.family_code(),
            frame: Some(frames.next().unwrap()),
        };
        assert!(
            validator
                .accept_relation_frame(
                    open.clone(),
                    event(Event::OwnerRelationIpcFrame(open)),
                    true,
                )
                .unwrap()
                .is_none()
        );
        let payload = OwnerRelationIpcFrame {
            provider_run_id: "run:test".to_owned(),
            compilation_unit_id: "unit:test".to_owned(),
            sequence: 3,
            owner_id: "owner:test".to_owned(),
            observation_family_code: relation.family_code(),
            frame: Some(frames.next().unwrap()),
        };
        let acknowledgement = validator
            .accept_relation_frame(
                payload.clone(),
                event(Event::OwnerRelationIpcFrame(payload)),
                true,
            )
            .unwrap()
            .unwrap();
        let Some(Command::RelationIpcAck(acknowledgement)) = acknowledgement.command else {
            panic!("cancelled payload uses a relation cancellation acknowledgement")
        };
        let acknowledgement =
            crate::relation_ipc_proto::decode_flow_control_ack(&acknowledgement).unwrap();
        assert_eq!(acknowledgement.header.identity, identity);
        assert!(acknowledgement.cancelled);

        let mut end = CompilationEnd {
            provider_run_id: "run:test".to_owned(),
            compilation_unit_id: "unit:test".to_owned(),
            sequence: validator.next_sequence,
            compiler_exit_status: 0,
            closed_owner_set_digest: closed_owner_set_digest(&[]),
            capability_outcomes: Vec::new(),
            diagnostic_summary: Some(DiagnosticSummary {
                error_count: 0,
                warning_count: 0,
                diagnostics_digest: b3("cancelled"),
            }),
            overall_stream_digest: String::new(),
            terminal_state: ProviderRunState::Cancelled as i32,
            rejection_error: None,
        };
        let mut events = validator.events.clone();
        events.push(event(Event::CompilationEnd(end.clone())));
        end.overall_stream_digest = overall_stream_digest(&events);
        let completed = validator
            .finish(end.clone(), event(Event::CompilationEnd(end)), true)
            .unwrap();
        assert!(completed.owners.is_empty());
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

    #[test]
    fn typed_relation_ingress_rejects_unknown_and_opaque_referenced_payloads() {
        let (policy, admission, begin) = fixture();
        let first = event(Event::CompilationBegin(begin.clone()));
        let binding = trust_binding(&policy, &admission);
        let mut validator = RunValidator::new(admission, policy, begin, first, binding).unwrap();
        let unknown_begin = owner_begin(70);
        assert_eq!(
            validator
                .accept_owner_begin(
                    unknown_begin.clone(),
                    event(Event::OwnerBegin(unknown_begin)),
                )
                .unwrap_err()
                .code(),
            tonic::Code::InvalidArgument
        );

        let mut unknown = typed_relation_chunk(RustcRelation::MirBody);
        unknown.observation_family_code = 70;
        assert_eq!(
            validate_typed_relation_chunk(&unknown).unwrap_err().code(),
            tonic::Code::InvalidArgument
        );

        let mut referenced = typed_relation_chunk(RustcRelation::MirBody);
        referenced.arrow_ipc.clear();
        referenced.payload_reference = Some(BlobReference {
            blob_id: "opaque-payload".to_owned(),
            content_digest: referenced.chunk_digest.clone(),
            byte_length: 1,
            read_only_uri: "file:opaque-payload".to_owned(),
        });
        assert_eq!(
            validate_typed_relation_chunk(&referenced)
                .unwrap_err()
                .code(),
            tonic::Code::InvalidArgument
        );
    }

    #[tokio::test]
    async fn wp35_operational_acceptance_handshake_and_cancel_are_single_authority() {
        let (policy, admission, begin) = fixture();
        let binding = trust_binding(&policy, &admission);
        let (service, _accepted) =
            RustcObservationService::new(policy.clone(), admission.clone(), binding).unwrap();
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
            begin.compilation_unit_id.clone(),
            ActiveRun {
                compilation_unit_id: begin.compilation_unit_id.clone(),
                commands: Some(commands),
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

    #[test]
    fn observation_service_rejects_mismatched_launch_plan_binding_before_listen() {
        let (policy, admission, _) = fixture();
        let mismatched = RustCompilationProtocolBinding::test_only(
            &admission.provider_run_id,
            &admission.workspace_id,
            &admission.analysis_context_id,
            admission.source_generation,
            &admission.context_manifest_digest,
            &digest(b"changed-source-manifest"),
            &admission.resource_profile_id,
            &policy.sandbox_profile_digest,
            &policy.toolchain_identity_digest,
        );

        let Err(error) = RustcObservationService::new(policy, admission, mismatched) else {
            panic!("a mismatched launch-plan binding must fail before the endpoint is usable");
        };
        assert_eq!(error.code(), tonic::Code::FailedPrecondition);
        assert!(error.message().contains("launch-plan binding differs"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn orchestrated_untrusted_accounting_failure_never_executes_host_cargo() {
        let harness = lifecycle_harness();
        let result = run_untrusted_rustc_provider_lifecycle(UntrustedRustcProviderLifecycle {
            trust_policy: &harness.trust_policy,
            sandbox_capabilities: &harness.capabilities,
            sandbox_profile: &harness.profile,
            compilation_inputs: &harness.inputs,
            private_paths: &harness.paths,
            compilation_request: &harness.request,
            protocol_policy: harness.protocol_policy.clone(),
            run_admission: harness.admission.clone(),
            allowed_uid: harness.allowed_uid,
            launch_material: ProviderSandboxLaunchMaterial::DarwinProfile(Path::new(
                "/not-consumed-without-kernel-accounting",
            )),
        })
        .await
        .unwrap_err();

        assert!(matches!(
            result,
            RustcProviderLifecycleError::Trust(
                RustCompilationTrustError::CompleteAccountingUnavailable
            )
        ));
        assert!(!harness.launch_marker.exists());
        assert!(!harness.paths.extractor_socket_path.exists());
    }

    #[tokio::test]
    async fn orchestrated_trusted_local_bypass_is_rejected_before_spawn() {
        let harness = lifecycle_harness();
        let trusted_local = RustCompilationTrustPolicy::trusted_local_v1(
            lifecycle_limits(),
            RustExecutableExtensionPolicy::ExecuteInsideSelectedLauncher,
        );
        let result = run_untrusted_rustc_provider_lifecycle(UntrustedRustcProviderLifecycle {
            trust_policy: &trusted_local,
            sandbox_capabilities: &harness.capabilities,
            sandbox_profile: &harness.profile,
            compilation_inputs: &harness.inputs,
            private_paths: &harness.paths,
            compilation_request: &harness.request,
            protocol_policy: harness.protocol_policy.clone(),
            run_admission: harness.admission.clone(),
            allowed_uid: harness.allowed_uid,
            launch_material: ProviderSandboxLaunchMaterial::DarwinProfile(Path::new(
                "/trusted-local-bypass",
            )),
        })
        .await
        .unwrap_err();

        assert!(matches!(
            result,
            RustcProviderLifecycleError::Trust(
                RustCompilationTrustError::UntrustedAdmissionRequired
            )
        ));
        assert!(!harness.launch_marker.exists());
        assert!(!harness.paths.extractor_socket_path.exists());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn orchestrated_missing_terminal_returns_no_semantic_output() {
        let harness = lifecycle_harness();
        let mut events = completed_event_stream();
        let Some(ExtractionEvent {
            event: Some(Event::CompilationEnd(_)),
        }) = events.pop()
        else {
            panic!("fixture ends with the compiler terminal")
        };

        let result = execute_test_lifecycle(&harness, events, true)
            .await
            .unwrap_err();

        assert!(matches!(
            result,
            RustcProviderLifecycleError::MissingCompilerTerminal
        ));
        assert!(!harness.paths.extractor_socket_path.exists());
    }

    #[tokio::test]
    async fn orchestrated_context_mismatch_never_invokes_supervisor() {
        let harness = lifecycle_harness();
        let mut mismatched = harness.admission.clone();
        mismatched.analysis_context_id = "context:changed".into();
        let invoked = Arc::new(AtomicBool::new(false));
        let observed_invocation = Arc::clone(&invoked);
        let result = execute_prepared_rustc_lifecycle(
            lifecycle_plan(&harness),
            harness.paths.extractor_socket_path.clone(),
            harness.protocol_policy.clone(),
            mismatched,
            harness.allowed_uid,
            move |plan, _cancellation| {
                observed_invocation.store(true, Ordering::Release);
                async move {
                    RustCompilationLauncherReceipt::test_only_contained_success(&plan)
                        .map_err(Into::into)
                }
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(result, RustcProviderLifecycleError::Protocol(_)));
        assert!(!invoked.load(Ordering::Acquire));
        assert!(!harness.launch_marker.exists());
        assert!(!harness.paths.extractor_socket_path.exists());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn orchestrated_clean_and_incremental_generations_emit_equal_arrow_semantics() {
        let clean = lifecycle_harness();
        let clean_output = execute_test_lifecycle(&clean, completed_event_stream(), false)
            .await
            .unwrap();

        let mut incremental = lifecycle_harness();
        incremental.request.context.source_generation = 8;
        incremental.admission.source_generation = 8;
        let incremental_output = execute_test_lifecycle(
            &incremental,
            completed_event_stream_for_generation(8),
            false,
        )
        .await
        .unwrap();

        let semantic_payloads = |runs: &[TrustQualifiedRustcCompilation]| {
            runs.iter()
                .flat_map(|run| &run.accepted().owners)
                .flat_map(|owner| &owner.chunks)
                .map(|chunk| {
                    (
                        chunk.observation_family_code,
                        chunk.schema_digest.clone(),
                        chunk.arrow_ipc.clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            semantic_payloads(&clean_output),
            semantic_payloads(&incremental_output)
        );
        assert_eq!(clean_output[0].accepted().admission.source_generation, 7);
        assert_eq!(
            incremental_output[0].accepted().admission.source_generation,
            8
        );
    }

    #[tokio::test]
    async fn cargo_run_tracks_and_cancels_multiple_compilation_units_independently() {
        let (mut policy, admission, _) = fixture();
        policy.sandbox_profile_digest = format!("sha256:{}", "ab".repeat(32));
        let binding = trust_binding(&policy, &admission);
        let (service, _accepted) =
            RustcObservationService::new(policy, admission.clone(), binding).unwrap();
        let (first_sender, mut first_commands) = mpsc::channel(2);
        let (second_sender, mut second_commands) = mpsc::channel(2);
        {
            let mut active = service.active.lock().await;
            for (unit_id, commands) in
                [("unit:first", first_sender), ("unit:second", second_sender)]
            {
                active.insert(
                    unit_id.to_owned(),
                    ActiveRun {
                        compilation_unit_id: unit_id.to_owned(),
                        commands: Some(commands),
                        cancelled: false,
                        terminal_state: None,
                    },
                );
            }
        }

        assert_eq!(
            service.request_cancel_run(&admission.provider_run_id).await,
            2
        );
        assert!(service.supervisor_cancellation_signal().is_requested());
        for (expected_unit, receiver) in [
            ("unit:first", &mut first_commands),
            ("unit:second", &mut second_commands),
        ] {
            let command = receiver.recv().await.unwrap().unwrap();
            let Some(Command::Cancel(request)) = command.command else {
                panic!("run cancellation must use the reverse compiler command stream");
            };
            assert_eq!(request.provider_run_id, admission.provider_run_id);
            assert_eq!(request.compilation_unit_id, expected_unit);
        }
        assert!(service.run_cancelled("unit:first").await);
        assert!(service.run_cancelled("unit:second").await);

        service
            .mark_terminal("unit:first", ProviderRunState::Cancelled)
            .await;
        service
            .mark_terminal("unit:second", ProviderRunState::Succeeded)
            .await;
        assert_eq!(
            service.terminal_states().await,
            BTreeMap::from([
                ("unit:first".to_owned(), ProviderRunState::Cancelled),
                ("unit:second".to_owned(), ProviderRunState::Succeeded),
            ])
        );
    }
}
