//! Relation-scoped Arrow IPC framing and bounded stream assembly.
//!
//! Arrow owns semantic rows. These types are the typed outer control plane: each stream is
//! bound to one relation, one application-owned schema fingerprint, and exact source/context pins.
//! A stream is accepted only after Arrow end-of-stream, a coverage/remainder trailer, and a
//! matching terminal frame arrive in that order.

use std::collections::{BTreeMap, BTreeSet};

use arrow_array::RecordBatch;
use arrow_buffer::Buffer;
use arrow_ipc::reader::StreamDecoder;
use arrow_ipc::writer::StreamWriter;
use arrow_ipc::{MessageHeader, MetadataVersion, root_as_message};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use thiserror::Error;

/// Current relation-scoped control protocol version.
pub const RELATION_IPC_PROTOCOL_VERSION: u16 = 1;

/// Exact Arrow type and IPC universe admitted at the semantic process boundary.
///
/// This value is deliberately build-static: all three Rust build domains pin these crates to
/// one release, and provider schema digests include this identity. An Arrow upgrade therefore
/// changes both the compile graph and the runtime handshake instead of silently accepting a
/// structurally similar schema from a different public type universe.
pub const ARROW_TYPE_UNIVERSE: &str =
    "arrow-array@59.2.0|arrow-schema@59.2.0|arrow-ipc@59.2.0|metadata-v5";

/// Only semantic encoding admitted by this relation-stream protocol.
pub const TYPED_RELATION_ENCODING: &str = "typed-arrow-relation-stream";

const SCHEMA_DIGEST_KEY: &str = "codefabric.schema_digest";
const RELATION_ID_KEY: &str = "codefabric.relation_id";
const RELATION_PROTOCOL_VERSION_KEY: &str = "codefabric.relation_protocol_version";
const ARROW_TYPE_UNIVERSE_KEY: &str = "codefabric.arrow_type_universe";
const SEMANTIC_ENCODING_KEY: &str = "codefabric.semantic_encoding";
const FIELD_ID_KEY: &str = "codefabric.field_id";
const FIELD_ORDINAL_KEY: &str = "codefabric.field_ordinal";

const IPC_STREAM_EOS: [u8; 8] = [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0];

/// Application-owned relation identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelationId(pub [u8; 16]);

/// Unique identity of one relation stream within a provider run.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamId(pub [u8; 16]);

/// Application-owned fingerprint of the one schema permitted on a stream.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SchemaFingerprint(pub [u8; 32]);

impl SchemaFingerprint {
    /// Read the application-owned BLAKE3 schema identity carried by exact Arrow schema metadata.
    ///
    /// This does not invent a second schema canonicalizer. The application contract owns the
    /// digest projection; this boundary binds that declared identity to the frame identity and then
    /// separately requires exact Arrow [`Schema`] equality while decoding.
    ///
    /// # Errors
    ///
    /// Returns `InvalidSchemaMetadata` when the digest is absent or not canonical lowercase
    /// `b3:` plus 64 hexadecimal digits.
    pub fn from_schema_metadata(schema: &Schema) -> Result<Self, RelationIpcErrorKind> {
        let value = schema.metadata().get(SCHEMA_DIGEST_KEY).ok_or(
            RelationIpcErrorKind::InvalidSchemaMetadata("schema digest is absent"),
        )?;
        let hexadecimal =
            value
                .strip_prefix("b3:")
                .ok_or(RelationIpcErrorKind::InvalidSchemaMetadata(
                    "schema digest lacks the b3 prefix",
                ))?;
        if hexadecimal.len() != 64
            || !hexadecimal
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(RelationIpcErrorKind::InvalidSchemaMetadata(
                "schema digest is not 32 canonical lowercase hexadecimal bytes",
            ));
        }
        let mut digest = [0_u8; 32];
        for (index, byte) in digest.iter_mut().enumerate() {
            let offset = index * 2;
            *byte = (hex_nibble(hexadecimal.as_bytes()[offset]) << 4)
                | hex_nibble(hexadecimal.as_bytes()[offset + 1]);
        }
        Ok(Self(digest))
    }
}

/// Digest pin for the exact admitted source image.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourcePin(pub [u8; 32]);

/// Digest pin for the exact semantic/compiler context.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContextPin(pub [u8; 32]);

/// Identity repeated on every frame so independently interleaved streams cannot be confused.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamIdentity {
    pub relation_id: RelationId,
    pub stream_id: StreamId,
    pub schema_fingerprint: SchemaFingerprint,
    pub source_pin: SourcePin,
    pub context_pin: ContextPin,
}

/// Header shared by data-direction frames or by the independent acknowledgement sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    pub protocol_version: u16,
    pub identity: StreamIdentity,
    pub sequence: u64,
}

impl FrameHeader {
    /// Build a header for the current protocol version.
    #[must_use]
    pub const fn current(identity: StreamIdentity, sequence: u64) -> Self {
        Self {
            protocol_version: RELATION_IPC_PROTOCOL_VERSION,
            identity,
            sequence,
        }
    }
}

/// Application-owned schema contract used to validate an incoming provider stream.
#[derive(Clone, Debug)]
pub struct RelationStreamContract {
    pub identity: StreamIdentity,
    pub schema: SchemaRef,
    pub requested_units: u64,
}

/// Opens one registered relation stream. Sequence zero is mandatory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamOpen {
    pub header: FrameHeader,
    pub requested_units: u64,
}

/// A bounded fragment of exactly one Arrow IPC stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationPayload {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

/// Receiver-to-producer credit acknowledgement.
///
/// Acknowledgements have a sequence independent from the producer data sequence. Credit can
/// only be returned for bytes covered by a newly acknowledged payload frame. A cancellation
/// acknowledgement returns no credit and terminates the stream as a typed unknown failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowControlAck {
    pub header: FrameHeader,
    pub acknowledged_sequence: Option<u64>,
    pub released_bytes: u64,
    pub cancelled: bool,
}

/// Declares that all bytes of the one Arrow IPC stream have been sent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IpcEndOfStream {
    pub header: FrameHeader,
    pub declared_ipc_bytes: u64,
    pub declared_batches: u64,
    pub declared_rows: u64,
}

/// Stable identity for one requested unit that was not completed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CoverageScope(pub [u8; 16]);

/// Why a requested unit remains outside completed coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemainderReason {
    Unsupported,
    ProviderUnavailable,
    ResourceLimit,
    InvalidSource,
    Cancelled,
    Unknown,
}

/// Counted coverage remainder. Scope identities must be unique within one trailer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageRemainder {
    pub scope: CoverageScope,
    pub unit_count: u64,
    pub reason: RemainderReason,
}

/// Terminal coverage state for an otherwise structurally valid stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalStatus {
    Complete,
    Partial,
    Unknown,
}

/// Required coverage and remainder accounting after Arrow IPC end-of-stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageTrailer {
    pub status: TerminalStatus,
    pub requested_units: u64,
    pub completed_units: u64,
    pub remainders: Vec<CoverageRemainder>,
}

impl CoverageTrailer {
    /// Construct a valid complete trailer.
    #[must_use]
    pub const fn complete(requested_units: u64) -> Self {
        Self {
            status: TerminalStatus::Complete,
            requested_units,
            completed_units: requested_units,
            remainders: Vec::new(),
        }
    }
}

/// Carries the required coverage/remainder trailer after IPC end-of-stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageTrailerFrame {
    pub header: FrameHeader,
    pub trailer: CoverageTrailer,
}

/// Final control frame. Its status must exactly match the accepted trailer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationTerminal {
    pub header: FrameHeader,
    pub status: TerminalStatus,
}

/// Typed outer protocol. Only `Payload` can carry semantic bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationIpcFrame {
    Open(StreamOpen),
    Payload(RelationPayload),
    FlowControlAck(FlowControlAck),
    IpcEnd(IpcEndOfStream),
    CoverageTrailer(CoverageTrailerFrame),
    Terminal(RelationTerminal),
}

impl RelationIpcFrame {
    #[must_use]
    pub(crate) fn header(&self) -> FrameHeader {
        match self {
            Self::Open(frame) => frame.header,
            Self::Payload(frame) => frame.header,
            Self::FlowControlAck(frame) => frame.header,
            Self::IpcEnd(frame) => frame.header,
            Self::CoverageTrailer(frame) => frame.header,
            Self::Terminal(frame) => frame.header,
        }
    }
}

/// Resource and backpressure limits enforced before accepting allocations or decoded rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationIpcLimits {
    pub max_registered_streams: usize,
    pub max_frames_per_stream: u64,
    pub max_payload_bytes_per_frame: usize,
    pub max_payload_bytes_per_stream: usize,
    pub max_total_payload_bytes: usize,
    pub initial_credit_bytes: usize,
    pub max_credit_bytes: usize,
    pub max_batches_per_stream: usize,
    pub max_rows_per_stream: usize,
    pub max_remainders_per_stream: usize,
}

impl Default for RelationIpcLimits {
    fn default() -> Self {
        Self {
            max_registered_streams: 64,
            max_frames_per_stream: 4_096,
            max_payload_bytes_per_frame: 1_048_576,
            max_payload_bytes_per_stream: 67_108_864,
            max_total_payload_bytes: 268_435_456,
            initial_credit_bytes: 4_194_304,
            max_credit_bytes: 16_777_216,
            max_batches_per_stream: 65_536,
            max_rows_per_stream: 16_777_216,
            max_remainders_per_stream: 4_096,
        }
    }
}

/// Coverage attached to every protocol failure. An invalid or absent trailer can never imply
/// complete coverage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FailureCoverage {
    Partial {
        requested_units: u64,
        completed_units: u64,
        remainder_units: u64,
    },
    Unknown {
        requested_units: Option<u64>,
        completed_units: Option<u64>,
    },
}

/// Stable protocol violation categories.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RelationIpcErrorKind {
    #[error("invalid relation IPC limit: {0}")]
    InvalidLimits(&'static str),
    #[error("stream is not registered")]
    UnregisteredStream,
    #[error("stream identity is already registered or open")]
    DuplicateStreamId,
    #[error("stream is already terminal")]
    ClosedStream,
    #[error("unsupported protocol version {actual}; expected {expected}")]
    UnsupportedProtocolVersion { expected: u16, actual: u16 },
    #[error("stream identity is invalid: {0}")]
    InvalidIdentity(&'static str),
    #[error("Arrow relation schema metadata is invalid: {0}")]
    InvalidSchemaMetadata(&'static str),
    #[error("schema fingerprint differs from the application-owned schema digest")]
    SchemaFingerprintMismatch,
    #[error("Arrow type universe differs from the pinned semantic universe")]
    ArrowTypeUniverseMismatch,
    #[error("schema exposes an opaque semantic carrier: {0}")]
    OpaqueSemanticCarrier(String),
    #[error("frame identity differs from the registered stream contract")]
    IdentityMismatch,
    #[error("duplicate sequence {actual}; expected {expected}")]
    DuplicateSequence { expected: u64, actual: u64 },
    #[error("out-of-order sequence {actual}; expected {expected}")]
    OutOfOrderSequence { expected: u64, actual: u64 },
    #[error("sequence space is exhausted")]
    SequenceOverflow,
    #[error("unexpected {frame} frame while stream is {phase}")]
    UnexpectedFrame {
        phase: &'static str,
        frame: &'static str,
    },
    #[error("{resource} limit exceeded: actual {actual}, limit {limit}")]
    LimitExceeded {
        resource: &'static str,
        limit: u64,
        actual: u64,
    },
    #[error("payload exceeds available flow-control credit: payload {payload}, credit {credit}")]
    BackpressureExceeded { payload: u64, credit: u64 },
    #[error("flow-control acknowledgement is invalid")]
    InvalidAcknowledgement,
    #[error("flow-control acknowledgement references an unknown or future payload sequence")]
    FutureAcknowledgement,
    #[error("flow-control acknowledgement repeats or regresses")]
    DuplicateAcknowledgement,
    #[error("flow-control credit exceeds newly acknowledged payload bytes")]
    ExcessAcknowledgementCredit,
    #[error("stream was cancelled")]
    Cancelled,
    #[error("payload is empty")]
    EmptyPayload,
    #[error("declared IPC bytes, batches, or rows differ from the received stream")]
    DeclaredShapeMismatch,
    #[error("Arrow IPC stream lacks its physical end-of-stream marker")]
    MissingArrowEndMarker,
    #[error("Arrow IPC stream differs from the pinned stream-v5 profile: {0}")]
    ArrowIpcProfileMismatch(&'static str),
    #[error("Arrow IPC encoding failed: {0}")]
    ArrowEncode(String),
    #[error("Arrow IPC decoding failed: {0}")]
    ArrowDecode(String),
    #[error("decoded schema differs from the registered schema contract")]
    SchemaMismatch,
    #[error("coverage/remainder trailer is invalid: {0}")]
    InvalidCoverage(&'static str),
    #[error("terminal status differs from the accepted coverage trailer")]
    TerminalStatusMismatch,
    #[error("stream ended before Arrow IPC end-of-stream")]
    MissingIpcEnd,
    #[error("registered relation stream was never opened")]
    MissingOpen,
    #[error("stream ended without the required coverage/remainder trailer")]
    MissingCoverageTrailer,
    #[error("stream ended without a terminal frame")]
    MissingTerminal,
}

/// Protocol failure with stream identity and explicit non-complete coverage.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{kind}")]
pub struct RelationIpcError {
    pub identity: Option<Box<StreamIdentity>>,
    pub coverage: FailureCoverage,
    pub kind: RelationIpcErrorKind,
}

/// Fully decoded relation accepted only after trailer and terminal validation.
#[derive(Clone, Debug)]
pub struct AssembledRelation {
    pub identity: StreamIdentity,
    pub schema: SchemaRef,
    pub batches: Vec<RecordBatch>,
    pub ipc_bytes: Vec<u8>,
    pub trailer: CoverageTrailer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StreamPhase {
    Receiving,
    IpcEnded,
    Trailed,
}

impl StreamPhase {
    const fn name(self) -> &'static str {
        match self {
            Self::Receiving => "receiving",
            Self::IpcEnded => "IPC-ended",
            Self::Trailed => "trailed",
        }
    }
}

#[derive(Debug)]
struct DecodedStream {
    batches: Vec<RecordBatch>,
}

#[derive(Debug)]
struct ActiveStream {
    contract: RelationStreamContract,
    phase: StreamPhase,
    next_sequence: u64,
    next_ack_sequence: u64,
    frame_count: u64,
    available_credit: usize,
    payload: Vec<u8>,
    payload_cumulative_by_sequence: BTreeMap<u64, usize>,
    acknowledged_payload_bytes: usize,
    last_acknowledged_sequence: Option<u64>,
    decoded: Option<DecodedStream>,
    trailer: Option<CoverageTrailer>,
}

impl ActiveStream {
    fn failure_coverage(&self) -> FailureCoverage {
        match self.trailer.as_ref() {
            Some(trailer) if trailer.status == TerminalStatus::Partial => {
                FailureCoverage::Partial {
                    requested_units: trailer.requested_units,
                    completed_units: trailer.completed_units,
                    remainder_units: trailer
                        .remainders
                        .iter()
                        .map(|remainder| remainder.unit_count)
                        .sum(),
                }
            }
            Some(trailer) => FailureCoverage::Unknown {
                requested_units: Some(trailer.requested_units),
                completed_units: (trailer.status == TerminalStatus::Unknown)
                    .then_some(trailer.completed_units),
            },
            None => FailureCoverage::Unknown {
                requested_units: Some(self.contract.requested_units),
                completed_units: None,
            },
        }
    }

    fn error(&self, kind: RelationIpcErrorKind) -> RelationIpcError {
        RelationIpcError {
            identity: Some(Box::new(self.contract.identity)),
            coverage: self.failure_coverage(),
            kind,
        }
    }
}

/// Stateful, bounded assembler for arbitrarily interleaved relation streams.
#[derive(Debug)]
pub struct RelationIpcAssembler {
    limits: RelationIpcLimits,
    contracts: BTreeMap<StreamId, RelationStreamContract>,
    active: BTreeMap<StreamId, ActiveStream>,
    closed: BTreeSet<StreamId>,
    total_payload_bytes: usize,
}

impl RelationIpcAssembler {
    /// Create an assembler after checking that its limits are internally consistent.
    ///
    /// # Errors
    ///
    /// Returns `InvalidLimits` when a zero limit or an impossible credit relationship is given.
    pub fn new(limits: RelationIpcLimits) -> Result<Self, RelationIpcError> {
        validate_limits(limits)?;
        Ok(Self {
            limits,
            contracts: BTreeMap::new(),
            active: BTreeMap::new(),
            closed: BTreeSet::new(),
            total_payload_bytes: 0,
        })
    }

    /// Register the application-owned schema contract before accepting provider bytes.
    ///
    /// # Errors
    ///
    /// Rejects invalid identity/schema bindings, opaque carrier schemas, duplicate stream
    /// identities, and registry resource overflow.
    pub fn register_contract(
        &mut self,
        contract: RelationStreamContract,
    ) -> Result<(), RelationIpcError> {
        let identity = contract.identity;
        validate_relation_contract(&contract)?;
        if self.contracts.contains_key(&identity.stream_id)
            || self.active.contains_key(&identity.stream_id)
            || self.closed.contains(&identity.stream_id)
        {
            return Err(standalone_error(
                Some(identity),
                Some(contract.requested_units),
                RelationIpcErrorKind::DuplicateStreamId,
            ));
        }
        if self.contracts.len() == self.limits.max_registered_streams {
            return Err(standalone_error(
                Some(identity),
                Some(contract.requested_units),
                limit_error(
                    "registered streams",
                    self.limits.max_registered_streams,
                    self.contracts.len().saturating_add(1),
                ),
            ));
        }
        self.contracts.insert(identity.stream_id, contract);
        Ok(())
    }

    /// Accept one outer frame. A completed relation is returned only on its valid terminal frame.
    ///
    /// # Errors
    ///
    /// Any malformed frame fails the affected known stream closed. The error always carries
    /// partial or unknown coverage, never an implicit complete result.
    pub fn push(
        &mut self,
        frame: RelationIpcFrame,
    ) -> Result<Option<AssembledRelation>, RelationIpcError> {
        let identity = frame.header().identity;
        let stream_id = identity.stream_id;
        let known = self.contracts.contains_key(&stream_id) || self.active.contains_key(&stream_id);
        let result = self.push_inner(frame);
        if result.is_err() && known {
            self.remove_active(stream_id);
            self.closed.insert(stream_id);
        }
        result
    }

    /// Verify that every registered stream was opened and reached a terminal disposition.
    ///
    /// # Errors
    ///
    /// Returns the precise missing phase with non-complete coverage for the first unfinished
    /// stream in stable stream-ID order.
    pub fn finish(self) -> Result<(), RelationIpcError> {
        if let Some(state) = self.active.values().next() {
            let kind = match state.phase {
                StreamPhase::Receiving => RelationIpcErrorKind::MissingIpcEnd,
                StreamPhase::IpcEnded => RelationIpcErrorKind::MissingCoverageTrailer,
                StreamPhase::Trailed => RelationIpcErrorKind::MissingTerminal,
            };
            return Err(state.error(kind));
        }
        if let Some(contract) = self.contracts.iter().find_map(|(stream_id, contract)| {
            (!self.closed.contains(stream_id)).then_some(contract)
        }) {
            return Err(standalone_error(
                Some(contract.identity),
                Some(contract.requested_units),
                RelationIpcErrorKind::MissingOpen,
            ));
        }
        Ok(())
    }

    fn push_inner(
        &mut self,
        frame: RelationIpcFrame,
    ) -> Result<Option<AssembledRelation>, RelationIpcError> {
        match frame {
            RelationIpcFrame::Open(frame) => self.accept_open(frame),
            RelationIpcFrame::Payload(frame) => self.accept_payload(frame),
            RelationIpcFrame::FlowControlAck(frame) => self.accept_ack(frame),
            RelationIpcFrame::IpcEnd(frame) => self.accept_ipc_end(frame),
            RelationIpcFrame::CoverageTrailer(frame) => self.accept_trailer(frame),
            RelationIpcFrame::Terminal(frame) => self.accept_terminal(frame),
        }
    }

    fn accept_open(
        &mut self,
        frame: StreamOpen,
    ) -> Result<Option<AssembledRelation>, RelationIpcError> {
        let identity = frame.header.identity;
        if self.closed.contains(&identity.stream_id) {
            return Err(standalone_error(
                Some(identity),
                None,
                RelationIpcErrorKind::ClosedStream,
            ));
        }
        if self.active.contains_key(&identity.stream_id) {
            let state = self.active.get(&identity.stream_id).expect("checked above");
            return Err(state.error(RelationIpcErrorKind::DuplicateStreamId));
        }
        let contract = self
            .contracts
            .get(&identity.stream_id)
            .cloned()
            .ok_or_else(|| {
                standalone_error(
                    Some(identity),
                    None,
                    RelationIpcErrorKind::UnregisteredStream,
                )
            })?;
        validate_protocol_version(frame.header, Some(contract.requested_units))?;
        if frame.header.identity != contract.identity
            || frame.requested_units != contract.requested_units
        {
            return Err(standalone_error(
                Some(identity),
                Some(contract.requested_units),
                RelationIpcErrorKind::IdentityMismatch,
            ));
        }
        if frame.header.sequence != 0 {
            return Err(standalone_error(
                Some(identity),
                Some(contract.requested_units),
                sequence_error(0, frame.header.sequence),
            ));
        }
        self.active.insert(
            identity.stream_id,
            ActiveStream {
                contract,
                phase: StreamPhase::Receiving,
                next_sequence: 1,
                next_ack_sequence: 0,
                frame_count: 1,
                available_credit: self.limits.initial_credit_bytes,
                payload: Vec::new(),
                payload_cumulative_by_sequence: BTreeMap::new(),
                acknowledged_payload_bytes: 0,
                last_acknowledged_sequence: None,
                decoded: None,
                trailer: None,
            },
        );
        Ok(None)
    }

    fn accept_payload(
        &mut self,
        mut frame: RelationPayload,
    ) -> Result<Option<AssembledRelation>, RelationIpcError> {
        let stream_id = frame.header.identity.stream_id;
        let limits = self.limits;
        let next_total = self
            .total_payload_bytes
            .checked_add(frame.payload.len())
            .ok_or_else(|| {
                standalone_error(
                    Some(frame.header.identity),
                    None,
                    limit_error(
                        "total payload bytes",
                        limits.max_total_payload_bytes,
                        usize::MAX,
                    ),
                )
            })?;
        let state = self.active.get_mut(&stream_id).ok_or_else(|| {
            unknown_or_closed_error(frame.header.identity, self.closed.contains(&stream_id))
        })?;
        validate_data_header(state, frame.header)?;
        require_phase(state, StreamPhase::Receiving, "payload")?;
        count_frame(state, limits)?;
        if frame.payload.is_empty() {
            return Err(state.error(RelationIpcErrorKind::EmptyPayload));
        }
        if frame.payload.len() > limits.max_payload_bytes_per_frame {
            return Err(state.error(limit_error(
                "payload frame bytes",
                limits.max_payload_bytes_per_frame,
                frame.payload.len(),
            )));
        }
        let stream_bytes = state
            .payload
            .len()
            .checked_add(frame.payload.len())
            .ok_or_else(|| {
                state.error(limit_error(
                    "stream payload bytes",
                    limits.max_payload_bytes_per_stream,
                    usize::MAX,
                ))
            })?;
        if stream_bytes > limits.max_payload_bytes_per_stream {
            return Err(state.error(limit_error(
                "stream payload bytes",
                limits.max_payload_bytes_per_stream,
                stream_bytes,
            )));
        }
        if next_total > limits.max_total_payload_bytes {
            return Err(state.error(limit_error(
                "total payload bytes",
                limits.max_total_payload_bytes,
                next_total,
            )));
        }
        if frame.payload.len() > state.available_credit {
            return Err(state.error(RelationIpcErrorKind::BackpressureExceeded {
                payload: as_u64(frame.payload.len()),
                credit: as_u64(state.available_credit),
            }));
        }
        state.available_credit -= frame.payload.len();
        state.payload.append(&mut frame.payload);
        state
            .payload_cumulative_by_sequence
            .insert(frame.header.sequence, stream_bytes);
        advance_data_sequence(state)?;
        self.total_payload_bytes = next_total;
        Ok(None)
    }

    fn accept_ack(
        &mut self,
        frame: FlowControlAck,
    ) -> Result<Option<AssembledRelation>, RelationIpcError> {
        let stream_id = frame.header.identity.stream_id;
        let limits = self.limits;
        let state = self.active.get_mut(&stream_id).ok_or_else(|| {
            unknown_or_closed_error(frame.header.identity, self.closed.contains(&stream_id))
        })?;
        validate_common_header(state, frame.header)?;
        validate_ack_sequence(state, frame.header.sequence)?;
        count_frame(state, limits)?;
        if frame.cancelled {
            if frame.acknowledged_sequence.is_some() || frame.released_bytes != 0 {
                return Err(state.error(RelationIpcErrorKind::InvalidAcknowledgement));
            }
            return Err(state.error(RelationIpcErrorKind::Cancelled));
        }
        require_phase(
            state,
            StreamPhase::Receiving,
            "flow-control acknowledgement",
        )?;
        let acknowledged_sequence = frame
            .acknowledged_sequence
            .ok_or_else(|| state.error(RelationIpcErrorKind::InvalidAcknowledgement))?;
        if state
            .last_acknowledged_sequence
            .is_some_and(|previous| acknowledged_sequence <= previous)
        {
            return Err(state.error(RelationIpcErrorKind::DuplicateAcknowledgement));
        }
        let cumulative = state
            .payload_cumulative_by_sequence
            .get(&acknowledged_sequence)
            .copied()
            .ok_or_else(|| state.error(RelationIpcErrorKind::FutureAcknowledgement))?;
        let newly_acknowledged = cumulative
            .checked_sub(state.acknowledged_payload_bytes)
            .ok_or_else(|| state.error(RelationIpcErrorKind::DuplicateAcknowledgement))?;
        let released = usize::try_from(frame.released_bytes)
            .map_err(|_| state.error(RelationIpcErrorKind::ExcessAcknowledgementCredit))?;
        if released == 0 || released != newly_acknowledged {
            return Err(state.error(RelationIpcErrorKind::ExcessAcknowledgementCredit));
        }
        let available_credit = state
            .available_credit
            .checked_add(released)
            .ok_or_else(|| state.error(RelationIpcErrorKind::ExcessAcknowledgementCredit))?;
        if available_credit > limits.max_credit_bytes {
            return Err(state.error(limit_error(
                "flow-control credit bytes",
                limits.max_credit_bytes,
                available_credit,
            )));
        }
        state.available_credit = available_credit;
        state.acknowledged_payload_bytes = cumulative;
        state.last_acknowledged_sequence = Some(acknowledged_sequence);
        state.next_ack_sequence = state
            .next_ack_sequence
            .checked_add(1)
            .ok_or_else(|| state.error(RelationIpcErrorKind::SequenceOverflow))?;
        Ok(None)
    }

    fn accept_ipc_end(
        &mut self,
        frame: IpcEndOfStream,
    ) -> Result<Option<AssembledRelation>, RelationIpcError> {
        let stream_id = frame.header.identity.stream_id;
        let limits = self.limits;
        let state = self.active.get_mut(&stream_id).ok_or_else(|| {
            unknown_or_closed_error(frame.header.identity, self.closed.contains(&stream_id))
        })?;
        validate_data_header(state, frame.header)?;
        require_phase(state, StreamPhase::Receiving, "IPC end-of-stream")?;
        count_frame(state, limits)?;
        if frame.declared_ipc_bytes != as_u64(state.payload.len())
            || frame.declared_batches > as_u64(limits.max_batches_per_stream)
            || frame.declared_rows > as_u64(limits.max_rows_per_stream)
        {
            return Err(state.error(RelationIpcErrorKind::DeclaredShapeMismatch));
        }
        let decoded = decode_arrow_stream(&state.payload, &state.contract.schema, &frame, limits)
            .map_err(|kind| state.error(kind))?;
        state.decoded = Some(decoded);
        state.phase = StreamPhase::IpcEnded;
        advance_data_sequence(state)?;
        Ok(None)
    }

    fn accept_trailer(
        &mut self,
        frame: CoverageTrailerFrame,
    ) -> Result<Option<AssembledRelation>, RelationIpcError> {
        let stream_id = frame.header.identity.stream_id;
        let limits = self.limits;
        let state = self.active.get_mut(&stream_id).ok_or_else(|| {
            unknown_or_closed_error(frame.header.identity, self.closed.contains(&stream_id))
        })?;
        validate_data_header(state, frame.header)?;
        require_phase(state, StreamPhase::IpcEnded, "coverage trailer")?;
        count_frame(state, limits)?;
        validate_coverage_trailer(
            &frame.trailer,
            state.contract.requested_units,
            limits.max_remainders_per_stream,
        )
        .map_err(|kind| state.error(kind))?;
        state.trailer = Some(frame.trailer);
        state.phase = StreamPhase::Trailed;
        advance_data_sequence(state)?;
        Ok(None)
    }

    fn accept_terminal(
        &mut self,
        frame: RelationTerminal,
    ) -> Result<Option<AssembledRelation>, RelationIpcError> {
        let stream_id = frame.header.identity.stream_id;
        {
            let state = self.active.get_mut(&stream_id).ok_or_else(|| {
                unknown_or_closed_error(frame.header.identity, self.closed.contains(&stream_id))
            })?;
            validate_data_header(state, frame.header)?;
            require_phase(state, StreamPhase::Trailed, "terminal")?;
            count_frame(state, self.limits)?;
            let trailer = state.trailer.as_ref().expect("trailed phase has a trailer");
            if frame.status != trailer.status {
                return Err(state.error(RelationIpcErrorKind::TerminalStatusMismatch));
            }
            advance_data_sequence(state)?;
        }
        let mut state = self
            .remove_active(stream_id)
            .expect("validated active stream");
        self.closed.insert(stream_id);
        let decoded = state.decoded.take().expect("trailed phase has decoded IPC");
        let trailer = state.trailer.take().expect("trailed phase has a trailer");
        Ok(Some(AssembledRelation {
            identity: state.contract.identity,
            schema: state.contract.schema,
            batches: decoded.batches,
            ipc_bytes: state.payload,
            trailer,
        }))
    }

    fn remove_active(&mut self, stream_id: StreamId) -> Option<ActiveStream> {
        let state = self.active.remove(&stream_id)?;
        self.total_payload_bytes = self
            .total_payload_bytes
            .checked_sub(state.payload.len())
            .expect("total payload accounting covers every active stream");
        Some(state)
    }
}

/// Encode one schema and all of its batches as an Arrow IPC stream, then fragment it into the
/// typed outer protocol. No semantic row is serialized into a control frame.
///
/// # Errors
///
/// Rejects schema drift, invalid coverage, zero fragment size, and Arrow encoding failures.
pub fn encode_relation_stream(
    contract: &RelationStreamContract,
    batches: &[RecordBatch],
    trailer: CoverageTrailer,
    fragment_bytes: usize,
) -> Result<Vec<RelationIpcFrame>, RelationIpcError> {
    validate_relation_contract(contract)?;
    if fragment_bytes == 0 {
        return Err(standalone_error(
            Some(contract.identity),
            Some(contract.requested_units),
            RelationIpcErrorKind::InvalidLimits("fragment_bytes must be non-zero"),
        ));
    }
    validate_coverage_trailer(&trailer, contract.requested_units, usize::MAX).map_err(|kind| {
        standalone_error(
            Some(contract.identity),
            Some(contract.requested_units),
            kind,
        )
    })?;
    let mut rows = 0_usize;
    for batch in batches {
        if batch.schema().as_ref() != contract.schema.as_ref() {
            return Err(standalone_error(
                Some(contract.identity),
                Some(contract.requested_units),
                RelationIpcErrorKind::SchemaMismatch,
            ));
        }
        rows = rows.checked_add(batch.num_rows()).ok_or_else(|| {
            standalone_error(
                Some(contract.identity),
                Some(contract.requested_units),
                RelationIpcErrorKind::DeclaredShapeMismatch,
            )
        })?;
    }
    let ipc_bytes = encode_arrow_stream(&contract.schema, batches).map_err(|kind| {
        standalone_error(
            Some(contract.identity),
            Some(contract.requested_units),
            kind,
        )
    })?;
    let mut sequence = 0_u64;
    let mut frames =
        Vec::with_capacity(4_usize.saturating_add(ipc_bytes.len().div_ceil(fragment_bytes)));
    frames.push(RelationIpcFrame::Open(StreamOpen {
        header: FrameHeader::current(contract.identity, sequence),
        requested_units: contract.requested_units,
    }));
    sequence = next_sequence(sequence, contract)?;
    for fragment in ipc_bytes.chunks(fragment_bytes) {
        frames.push(RelationIpcFrame::Payload(RelationPayload {
            header: FrameHeader::current(contract.identity, sequence),
            payload: fragment.to_vec(),
        }));
        sequence = next_sequence(sequence, contract)?;
    }
    frames.push(RelationIpcFrame::IpcEnd(IpcEndOfStream {
        header: FrameHeader::current(contract.identity, sequence),
        declared_ipc_bytes: as_u64(ipc_bytes.len()),
        declared_batches: as_u64(batches.len()),
        declared_rows: as_u64(rows),
    }));
    sequence = next_sequence(sequence, contract)?;
    let status = trailer.status;
    frames.push(RelationIpcFrame::CoverageTrailer(CoverageTrailerFrame {
        header: FrameHeader::current(contract.identity, sequence),
        trailer,
    }));
    sequence = next_sequence(sequence, contract)?;
    frames.push(RelationIpcFrame::Terminal(RelationTerminal {
        header: FrameHeader::current(contract.identity, sequence),
        status,
    }));
    Ok(frames)
}

fn validate_relation_contract(contract: &RelationStreamContract) -> Result<(), RelationIpcError> {
    validate_identity(contract.identity).map_err(|kind| {
        standalone_error(
            Some(contract.identity),
            Some(contract.requested_units),
            kind,
        )
    })?;
    validate_relation_schema(
        contract.schema.as_ref(),
        contract.identity.schema_fingerprint,
    )
    .map_err(|kind| {
        standalone_error(
            Some(contract.identity),
            Some(contract.requested_units),
            kind,
        )
    })
}

fn validate_identity(identity: StreamIdentity) -> Result<(), RelationIpcErrorKind> {
    if identity.relation_id.0 == [0; 16] {
        return Err(RelationIpcErrorKind::InvalidIdentity(
            "relation identity is zero",
        ));
    }
    if identity.stream_id.0 == [0; 16] {
        return Err(RelationIpcErrorKind::InvalidIdentity(
            "stream identity is zero",
        ));
    }
    if identity.schema_fingerprint.0 == [0; 32] {
        return Err(RelationIpcErrorKind::InvalidIdentity(
            "schema fingerprint is zero",
        ));
    }
    if identity.source_pin.0 == [0; 32] {
        return Err(RelationIpcErrorKind::InvalidIdentity("source pin is zero"));
    }
    if identity.context_pin.0 == [0; 32] {
        return Err(RelationIpcErrorKind::InvalidIdentity("context pin is zero"));
    }
    Ok(())
}

fn validate_relation_schema(
    schema: &Schema,
    expected_fingerprint: SchemaFingerprint,
) -> Result<(), RelationIpcErrorKind> {
    let metadata = schema.metadata();
    if metadata.get(ARROW_TYPE_UNIVERSE_KEY).map(String::as_str) != Some(ARROW_TYPE_UNIVERSE) {
        return Err(RelationIpcErrorKind::ArrowTypeUniverseMismatch);
    }
    if metadata.get(SEMANTIC_ENCODING_KEY).map(String::as_str) != Some(TYPED_RELATION_ENCODING) {
        return Err(RelationIpcErrorKind::InvalidSchemaMetadata(
            "semantic encoding is not the typed relation-stream profile",
        ));
    }
    if metadata
        .get(RELATION_PROTOCOL_VERSION_KEY)
        .and_then(|value| value.parse::<u16>().ok())
        != Some(RELATION_IPC_PROTOCOL_VERSION)
    {
        return Err(RelationIpcErrorKind::InvalidSchemaMetadata(
            "relation schema protocol version differs",
        ));
    }
    if metadata
        .get(RELATION_ID_KEY)
        .is_none_or(|relation_id| relation_id.trim().is_empty())
    {
        return Err(RelationIpcErrorKind::InvalidSchemaMetadata(
            "application relation identity is absent",
        ));
    }
    if SchemaFingerprint::from_schema_metadata(schema)? != expected_fingerprint {
        return Err(RelationIpcErrorKind::SchemaFingerprintMismatch);
    }
    if schema.fields().is_empty() {
        return Err(RelationIpcErrorKind::InvalidSchemaMetadata(
            "relation schema has no typed fields",
        ));
    }

    let mut field_ids = BTreeSet::new();
    let mut field_names = BTreeSet::new();
    for (ordinal, field) in schema.fields().iter().enumerate() {
        if !field_names.insert(field.name()) {
            return Err(RelationIpcErrorKind::InvalidSchemaMetadata(
                "relation field name is duplicated",
            ));
        }
        let field_id = field.metadata().get(FIELD_ID_KEY).ok_or(
            RelationIpcErrorKind::InvalidSchemaMetadata("model field identity is absent"),
        )?;
        if field_id.trim().is_empty() || !field_ids.insert(field_id) {
            return Err(RelationIpcErrorKind::InvalidSchemaMetadata(
                "model field identity is empty or duplicated",
            ));
        }
        if field
            .metadata()
            .get(FIELD_ORDINAL_KEY)
            .and_then(|value| value.parse::<usize>().ok())
            != Some(ordinal)
        {
            return Err(RelationIpcErrorKind::InvalidSchemaMetadata(
                "model field ordinal differs from Arrow field order",
            ));
        }
        validate_typed_field(field)?;
    }
    Ok(())
}

fn validate_typed_field(field: &Field) -> Result<(), RelationIpcErrorKind> {
    if has_opaque_token(field.name()) {
        return Err(RelationIpcErrorKind::OpaqueSemanticCarrier(
            field.name().to_owned(),
        ));
    }
    for (key, value) in field.metadata() {
        if has_opaque_token(key)
            || (key.eq_ignore_ascii_case("ARROW:extension:name") && has_opaque_token(value))
        {
            return Err(RelationIpcErrorKind::OpaqueSemanticCarrier(
                field.name().to_owned(),
            ));
        }
    }
    validate_typed_data_type(field.data_type(), field.name())
}

fn validate_typed_data_type(
    data_type: &DataType,
    containing_field: &str,
) -> Result<(), RelationIpcErrorKind> {
    match data_type {
        DataType::Binary | DataType::LargeBinary | DataType::BinaryView => Err(
            RelationIpcErrorKind::OpaqueSemanticCarrier(containing_field.to_owned()),
        ),
        DataType::List(field)
        | DataType::ListView(field)
        | DataType::FixedSizeList(field, _)
        | DataType::LargeList(field)
        | DataType::LargeListView(field)
        | DataType::Map(field, _) => validate_typed_field(field),
        DataType::Struct(fields) => {
            for field in fields {
                validate_typed_field(field)?;
            }
            Ok(())
        }
        DataType::Union(fields, _) => {
            for (_, field) in fields.iter() {
                validate_typed_field(field)?;
            }
            Ok(())
        }
        DataType::Dictionary(key, value) => {
            validate_typed_data_type(key, containing_field)?;
            validate_typed_data_type(value, containing_field)
        }
        DataType::RunEndEncoded(run_ends, values) => {
            validate_typed_field(run_ends)?;
            validate_typed_field(values)
        }
        _ => Ok(()),
    }
}

fn has_opaque_token(value: &str) -> bool {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|token| {
            token.eq_ignore_ascii_case("json")
                || token.eq_ignore_ascii_case("payload")
                || token.eq_ignore_ascii_case("opaque")
                || token.eq_ignore_ascii_case("blob")
        })
}

const fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!(),
    }
}

fn validate_limits(limits: RelationIpcLimits) -> Result<(), RelationIpcError> {
    let invalid = if limits.max_registered_streams == 0 {
        Some("max_registered_streams must be non-zero")
    } else if limits.max_frames_per_stream < 4 {
        Some("max_frames_per_stream must permit open, IPC end, trailer, and terminal")
    } else if limits.max_payload_bytes_per_frame == 0 {
        Some("max_payload_bytes_per_frame must be non-zero")
    } else if limits.max_payload_bytes_per_stream == 0 {
        Some("max_payload_bytes_per_stream must be non-zero")
    } else if limits.max_total_payload_bytes < limits.max_payload_bytes_per_stream {
        Some("max_total_payload_bytes must cover one maximum stream")
    } else if limits.initial_credit_bytes == 0 {
        Some("initial_credit_bytes must be non-zero")
    } else if limits.initial_credit_bytes > limits.max_credit_bytes {
        Some("initial_credit_bytes exceeds max_credit_bytes")
    } else if limits.max_batches_per_stream == 0 {
        Some("max_batches_per_stream must be non-zero")
    } else if limits.max_rows_per_stream == 0 {
        Some("max_rows_per_stream must be non-zero")
    } else if limits.max_remainders_per_stream == 0 {
        Some("max_remainders_per_stream must be non-zero")
    } else {
        None
    };
    invalid.map_or(Ok(()), |detail| {
        Err(standalone_error(
            None,
            None,
            RelationIpcErrorKind::InvalidLimits(detail),
        ))
    })
}

fn validate_protocol_version(
    header: FrameHeader,
    requested_units: Option<u64>,
) -> Result<(), RelationIpcError> {
    if header.protocol_version == RELATION_IPC_PROTOCOL_VERSION {
        return Ok(());
    }
    Err(standalone_error(
        Some(header.identity),
        requested_units,
        RelationIpcErrorKind::UnsupportedProtocolVersion {
            expected: RELATION_IPC_PROTOCOL_VERSION,
            actual: header.protocol_version,
        },
    ))
}

fn validate_common_header(
    state: &ActiveStream,
    header: FrameHeader,
) -> Result<(), RelationIpcError> {
    if header.protocol_version != RELATION_IPC_PROTOCOL_VERSION {
        return Err(
            state.error(RelationIpcErrorKind::UnsupportedProtocolVersion {
                expected: RELATION_IPC_PROTOCOL_VERSION,
                actual: header.protocol_version,
            }),
        );
    }
    if header.identity != state.contract.identity {
        return Err(state.error(RelationIpcErrorKind::IdentityMismatch));
    }
    Ok(())
}

fn validate_data_header(state: &ActiveStream, header: FrameHeader) -> Result<(), RelationIpcError> {
    validate_common_header(state, header)?;
    if header.sequence == state.next_sequence {
        Ok(())
    } else {
        Err(state.error(sequence_error(state.next_sequence, header.sequence)))
    }
}

fn validate_ack_sequence(state: &ActiveStream, actual: u64) -> Result<(), RelationIpcError> {
    if actual == state.next_ack_sequence {
        Ok(())
    } else {
        Err(state.error(sequence_error(state.next_ack_sequence, actual)))
    }
}

fn sequence_error(expected: u64, actual: u64) -> RelationIpcErrorKind {
    if actual < expected {
        RelationIpcErrorKind::DuplicateSequence { expected, actual }
    } else {
        RelationIpcErrorKind::OutOfOrderSequence { expected, actual }
    }
}

fn advance_data_sequence(state: &mut ActiveStream) -> Result<(), RelationIpcError> {
    state.next_sequence = state
        .next_sequence
        .checked_add(1)
        .ok_or_else(|| state.error(RelationIpcErrorKind::SequenceOverflow))?;
    Ok(())
}

fn count_frame(
    state: &mut ActiveStream,
    limits: RelationIpcLimits,
) -> Result<(), RelationIpcError> {
    if state.frame_count == limits.max_frames_per_stream {
        return Err(state.error(RelationIpcErrorKind::LimitExceeded {
            resource: "frames per stream",
            limit: limits.max_frames_per_stream,
            actual: state.frame_count.saturating_add(1),
        }));
    }
    state.frame_count += 1;
    Ok(())
}

fn require_phase(
    state: &ActiveStream,
    expected: StreamPhase,
    frame: &'static str,
) -> Result<(), RelationIpcError> {
    if state.phase == expected {
        return Ok(());
    }
    Err(state.error(RelationIpcErrorKind::UnexpectedFrame {
        phase: state.phase.name(),
        frame,
    }))
}

fn validate_coverage_trailer(
    trailer: &CoverageTrailer,
    expected_requested_units: u64,
    max_remainders: usize,
) -> Result<(), RelationIpcErrorKind> {
    if trailer.requested_units != expected_requested_units {
        return Err(RelationIpcErrorKind::InvalidCoverage(
            "requested unit count differs from stream open",
        ));
    }
    if trailer.completed_units > trailer.requested_units {
        return Err(RelationIpcErrorKind::InvalidCoverage(
            "completed units exceed requested units",
        ));
    }
    if trailer.remainders.len() > max_remainders {
        return Err(limit_error(
            "coverage remainders",
            max_remainders,
            trailer.remainders.len(),
        ));
    }
    let mut scopes = BTreeSet::new();
    let mut remainder_units = 0_u64;
    let mut has_unknown = false;
    for remainder in &trailer.remainders {
        if remainder.unit_count == 0 {
            return Err(RelationIpcErrorKind::InvalidCoverage(
                "remainder unit count must be non-zero",
            ));
        }
        if !scopes.insert(remainder.scope) {
            return Err(RelationIpcErrorKind::InvalidCoverage(
                "remainder scope is duplicated",
            ));
        }
        remainder_units = remainder_units.checked_add(remainder.unit_count).ok_or(
            RelationIpcErrorKind::InvalidCoverage("remainder unit count overflow"),
        )?;
        has_unknown |= remainder.reason == RemainderReason::Unknown;
    }
    let accounted_units = trailer.completed_units.checked_add(remainder_units).ok_or(
        RelationIpcErrorKind::InvalidCoverage("completed and remainder unit count overflow"),
    )?;
    if accounted_units != trailer.requested_units {
        return Err(RelationIpcErrorKind::InvalidCoverage(
            "completed and remainder units do not close the request",
        ));
    }
    match trailer.status {
        TerminalStatus::Complete
            if trailer.completed_units == trailer.requested_units
                && trailer.remainders.is_empty() =>
        {
            Ok(())
        }
        TerminalStatus::Partial
            if trailer.completed_units < trailer.requested_units
                && !trailer.remainders.is_empty()
                && !has_unknown =>
        {
            Ok(())
        }
        TerminalStatus::Unknown
            if trailer.completed_units < trailer.requested_units
                && !trailer.remainders.is_empty()
                && has_unknown =>
        {
            Ok(())
        }
        TerminalStatus::Complete => Err(RelationIpcErrorKind::InvalidCoverage(
            "complete coverage requires no remainder",
        )),
        TerminalStatus::Partial => Err(RelationIpcErrorKind::InvalidCoverage(
            "partial coverage requires a counted non-unknown remainder",
        )),
        TerminalStatus::Unknown => Err(RelationIpcErrorKind::InvalidCoverage(
            "unknown coverage requires a counted unknown remainder",
        )),
    }
}

fn encode_arrow_stream(
    schema: &SchemaRef,
    batches: &[RecordBatch],
) -> Result<Vec<u8>, RelationIpcErrorKind> {
    let mut bytes = Vec::new();
    {
        let mut writer = StreamWriter::try_new(&mut bytes, schema.as_ref())
            .map_err(|error| RelationIpcErrorKind::ArrowEncode(error.to_string()))?;
        for batch in batches {
            writer
                .write(batch)
                .map_err(|error| RelationIpcErrorKind::ArrowEncode(error.to_string()))?;
        }
        writer
            .finish()
            .map_err(|error| RelationIpcErrorKind::ArrowEncode(error.to_string()))?;
    }
    Ok(bytes)
}

fn decode_arrow_stream(
    bytes: &[u8],
    expected_schema: &SchemaRef,
    end: &IpcEndOfStream,
    limits: RelationIpcLimits,
) -> Result<DecodedStream, RelationIpcErrorKind> {
    if !bytes.ends_with(&IPC_STREAM_EOS) {
        return Err(RelationIpcErrorKind::MissingArrowEndMarker);
    }
    validate_arrow_ipc_profile(bytes)?;
    let mut decoder = StreamDecoder::new().with_require_alignment(false);
    let mut buffer = Buffer::from(bytes.to_vec());
    let mut batches = Vec::new();
    let mut rows = 0_usize;
    while !buffer.is_empty() {
        let before = buffer.len();
        if let Some(batch) = decoder
            .decode(&mut buffer)
            .map_err(|error| RelationIpcErrorKind::ArrowDecode(error.to_string()))?
        {
            if batch.schema().as_ref() != expected_schema.as_ref() {
                return Err(RelationIpcErrorKind::SchemaMismatch);
            }
            rows = rows.checked_add(batch.num_rows()).ok_or_else(|| {
                limit_error("decoded rows", limits.max_rows_per_stream, usize::MAX)
            })?;
            if rows > limits.max_rows_per_stream {
                return Err(limit_error(
                    "decoded rows",
                    limits.max_rows_per_stream,
                    rows,
                ));
            }
            if batches.len() == limits.max_batches_per_stream {
                return Err(limit_error(
                    "decoded batches",
                    limits.max_batches_per_stream,
                    batches.len().saturating_add(1),
                ));
            }
            batches.push(batch);
        }
        if buffer.len() == before {
            return Err(RelationIpcErrorKind::ArrowDecode(
                "stream decoder made no progress".to_owned(),
            ));
        }
    }
    decoder
        .finish()
        .map_err(|error| RelationIpcErrorKind::ArrowDecode(error.to_string()))?;
    if decoder.schema().as_deref() != Some(expected_schema.as_ref()) {
        return Err(RelationIpcErrorKind::SchemaMismatch);
    }
    if end.declared_batches != as_u64(batches.len()) || end.declared_rows != as_u64(rows) {
        return Err(RelationIpcErrorKind::DeclaredShapeMismatch);
    }
    Ok(DecodedStream { batches })
}

pub(crate) fn validate_arrow_ipc_profile(bytes: &[u8]) -> Result<(), RelationIpcErrorKind> {
    let mut offset = 0_usize;
    let mut message_count = 0_usize;
    loop {
        let header_end =
            offset
                .checked_add(8)
                .ok_or(RelationIpcErrorKind::ArrowIpcProfileMismatch(
                    "frame offset overflowed",
                ))?;
        if header_end > bytes.len() || bytes[offset..offset + 4] != IPC_STREAM_EOS[..4] {
            return Err(RelationIpcErrorKind::ArrowIpcProfileMismatch(
                "message lacks the continuation-marker framing",
            ));
        }
        let metadata_length = usize::try_from(u32::from_le_bytes(
            bytes[offset + 4..header_end]
                .try_into()
                .expect("four-byte metadata length"),
        ))
        .map_err(|_| {
            RelationIpcErrorKind::ArrowIpcProfileMismatch(
                "metadata length does not fit this platform",
            )
        })?;
        offset = header_end;
        if metadata_length == 0 {
            if message_count == 0 || offset != bytes.len() {
                return Err(RelationIpcErrorKind::ArrowIpcProfileMismatch(
                    "end marker is early or precedes every message",
                ));
            }
            return Ok(());
        }
        let metadata_end = offset.checked_add(metadata_length).ok_or(
            RelationIpcErrorKind::ArrowIpcProfileMismatch("metadata length overflowed"),
        )?;
        let metadata = bytes.get(offset..metadata_end).ok_or(
            RelationIpcErrorKind::ArrowIpcProfileMismatch("metadata is truncated"),
        )?;
        let message = root_as_message(metadata).map_err(|_| {
            RelationIpcErrorKind::ArrowIpcProfileMismatch("message metadata is invalid")
        })?;
        if message.version() != MetadataVersion::V5 {
            return Err(RelationIpcErrorKind::ArrowIpcProfileMismatch(
                "message metadata version is not V5",
            ));
        }
        if (message_count == 0) != (message.header_type() == MessageHeader::Schema) {
            return Err(RelationIpcErrorKind::ArrowIpcProfileMismatch(
                "schema message is absent, repeated, or out of order",
            ));
        }
        let body_length = usize::try_from(message.bodyLength()).map_err(|_| {
            RelationIpcErrorKind::ArrowIpcProfileMismatch("message body length is negative")
        })?;
        offset = metadata_end.checked_add(body_length).ok_or(
            RelationIpcErrorKind::ArrowIpcProfileMismatch("message body length overflowed"),
        )?;
        if offset > bytes.len() {
            return Err(RelationIpcErrorKind::ArrowIpcProfileMismatch(
                "message body is truncated",
            ));
        }
        message_count =
            message_count
                .checked_add(1)
                .ok_or(RelationIpcErrorKind::ArrowIpcProfileMismatch(
                    "message count overflowed",
                ))?;
    }
}

fn next_sequence(
    sequence: u64,
    contract: &RelationStreamContract,
) -> Result<u64, RelationIpcError> {
    sequence.checked_add(1).ok_or_else(|| {
        standalone_error(
            Some(contract.identity),
            Some(contract.requested_units),
            RelationIpcErrorKind::SequenceOverflow,
        )
    })
}

fn unknown_or_closed_error(identity: StreamIdentity, closed: bool) -> RelationIpcError {
    standalone_error(
        Some(identity),
        None,
        if closed {
            RelationIpcErrorKind::ClosedStream
        } else {
            RelationIpcErrorKind::UnregisteredStream
        },
    )
}

fn standalone_error(
    identity: Option<StreamIdentity>,
    requested_units: Option<u64>,
    kind: RelationIpcErrorKind,
) -> RelationIpcError {
    RelationIpcError {
        identity: identity.map(Box::new),
        coverage: FailureCoverage::Unknown {
            requested_units,
            completed_units: None,
        },
        kind,
    }
}

fn limit_error(resource: &'static str, limit: usize, actual: usize) -> RelationIpcErrorKind {
    RelationIpcErrorKind::LimitExceeded {
        resource,
        limit: as_u64(limit),
        actual: as_u64(actual),
    }
}

fn as_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow_array::builder::StringDictionaryBuilder;
    use arrow_array::types::Int32Type;
    use arrow_array::{Array as _, ArrayRef, Int32Array, RecordBatch};
    use arrow_ipc::writer::IpcWriteOptions;
    use arrow_schema::{DataType, Field, Schema};
    use serde_json::Value;

    use super::*;

    const WP33_FIXTURES: &str =
        include_str!("../contracts/acceptance/relational-fabric-v3/negative-fixtures.jsonl");

    fn claim_001_negative_fixture() -> Value {
        WP33_FIXTURES
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("valid WP33 fixture row"))
            .find(|row| row["claim_id"] == "RFV3-CLAIM-001" && row["kind"] == "negative")
            .expect("frozen Claim 001 negative fixture")
    }

    fn identity(marker: u8) -> StreamIdentity {
        StreamIdentity {
            relation_id: RelationId([marker; 16]),
            stream_id: StreamId([marker.wrapping_add(1); 16]),
            schema_fingerprint: SchemaFingerprint([marker.wrapping_add(2); 32]),
            source_pin: SourcePin([marker.wrapping_add(3); 32]),
            context_pin: ContextPin([marker.wrapping_add(4); 32]),
        }
    }

    fn typed_schema(marker: u8, fields: Vec<Field>) -> SchemaRef {
        let relation_id = format!("test.relation.{marker}");
        let descriptor = format!("{relation_id}|arrow={ARROW_TYPE_UNIVERSE}|{fields:?}");
        let digest = blake3::hash(descriptor.as_bytes());
        let fields = fields
            .into_iter()
            .enumerate()
            .map(|(ordinal, field)| {
                let mut metadata = field.metadata().clone();
                metadata.insert(
                    FIELD_ID_KEY.to_owned(),
                    format!("{relation_id}.{}", field.name()),
                );
                metadata.insert(FIELD_ORDINAL_KEY.to_owned(), ordinal.to_string());
                field.with_metadata(metadata)
            })
            .collect::<Vec<_>>();
        Arc::new(Schema::new_with_metadata(
            fields,
            [
                (RELATION_ID_KEY.to_owned(), relation_id),
                (
                    SCHEMA_DIGEST_KEY.to_owned(),
                    format!("b3:{}", digest.to_hex()),
                ),
                (
                    RELATION_PROTOCOL_VERSION_KEY.to_owned(),
                    RELATION_IPC_PROTOCOL_VERSION.to_string(),
                ),
                (
                    ARROW_TYPE_UNIVERSE_KEY.to_owned(),
                    ARROW_TYPE_UNIVERSE.to_owned(),
                ),
                (
                    SEMANTIC_ENCODING_KEY.to_owned(),
                    TYPED_RELATION_ENCODING.to_owned(),
                ),
            ]
            .into_iter()
            .collect(),
        ))
    }

    fn int_schema(marker: u8) -> SchemaRef {
        typed_schema(marker, vec![Field::new("value", DataType::Int32, false)])
    }

    fn int_batch(schema: &SchemaRef, values: Vec<i32>) -> RecordBatch {
        RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(Int32Array::from(values)) as ArrayRef],
        )
        .unwrap()
    }

    fn contract(marker: u8, requested_units: u64) -> RelationStreamContract {
        let schema = int_schema(marker);
        contract_for_schema(marker, requested_units, schema)
    }

    fn contract_for_schema(
        marker: u8,
        requested_units: u64,
        schema: SchemaRef,
    ) -> RelationStreamContract {
        let mut identity = identity(marker);
        identity.schema_fingerprint =
            SchemaFingerprint::from_schema_metadata(schema.as_ref()).unwrap();
        RelationStreamContract {
            identity,
            schema,
            requested_units,
        }
    }

    fn assembler_with(contract: &RelationStreamContract) -> RelationIpcAssembler {
        let mut assembler = RelationIpcAssembler::new(RelationIpcLimits::default()).unwrap();
        assembler.register_contract(contract.clone()).unwrap();
        assembler
    }

    fn run_frames(
        assembler: &mut RelationIpcAssembler,
        frames: Vec<RelationIpcFrame>,
    ) -> AssembledRelation {
        let mut completed = None;
        for frame in frames {
            if let Some(relation) = assembler.push(frame).unwrap() {
                assert!(completed.is_none());
                completed = Some(relation);
            }
        }
        completed.expect("terminal frame completes the relation")
    }

    fn payload_frames(frames: &[RelationIpcFrame]) -> Vec<RelationPayload> {
        frames
            .iter()
            .filter_map(|frame| match frame {
                RelationIpcFrame::Payload(payload) => Some(payload.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn wp34_int_round_trip_keeps_one_schema_and_dictionary_scope() {
        let mut builder = StringDictionaryBuilder::<Int32Type>::new();
        builder.append("alpha").unwrap();
        builder.append("beta").unwrap();
        builder.append("alpha").unwrap();
        let dictionary = builder.finish();
        let schema = typed_schema(
            10,
            vec![Field::new("kind", dictionary.data_type().clone(), false)],
        );
        let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(dictionary)]).unwrap();
        let mut identity = identity(10);
        identity.schema_fingerprint =
            SchemaFingerprint::from_schema_metadata(schema.as_ref()).unwrap();
        let contract = RelationStreamContract {
            identity,
            schema,
            requested_units: 3,
        };
        let frames =
            encode_relation_stream(&contract, &[batch], CoverageTrailer::complete(3), 37).unwrap();
        assert!(payload_frames(&frames).len() > 1);
        let relation = run_frames(&mut assembler_with(&contract), frames);
        assert_eq!(relation.batches.len(), 1);
        assert_eq!(relation.batches[0].num_rows(), 3);
        assert_eq!(
            relation.batches[0].schema().as_ref(),
            contract.schema.as_ref()
        );
        assert!(relation.ipc_bytes.ends_with(&IPC_STREAM_EOS));
    }

    #[test]
    fn empty_relation_still_carries_schema_eos_trailer_and_terminal() {
        let contract = contract(20, 0);
        let frames =
            encode_relation_stream(&contract, &[], CoverageTrailer::complete(0), 29).unwrap();
        let relation = run_frames(&mut assembler_with(&contract), frames);
        assert!(relation.batches.is_empty());
        assert_eq!(relation.trailer, CoverageTrailer::complete(0));
    }

    #[test]
    fn independently_sequenced_streams_can_interleave() {
        let first = contract(30, 2);
        let second = contract(40, 1);
        let first_frames = encode_relation_stream(
            &first,
            &[int_batch(&first.schema, vec![1, 2])],
            CoverageTrailer::complete(2),
            31,
        )
        .unwrap();
        let second_frames = encode_relation_stream(
            &second,
            &[int_batch(&second.schema, vec![9])],
            CoverageTrailer::complete(1),
            23,
        )
        .unwrap();
        let mut assembler = RelationIpcAssembler::new(RelationIpcLimits::default()).unwrap();
        assembler.register_contract(first.clone()).unwrap();
        assembler.register_contract(second.clone()).unwrap();
        let mut completed = Vec::new();
        let count = first_frames.len().max(second_frames.len());
        for index in 0..count {
            if let Some(frame) = first_frames.get(index)
                && let Some(relation) = assembler.push(frame.clone()).unwrap()
            {
                completed.push(relation.identity.stream_id);
            }
            if let Some(frame) = second_frames.get(index)
                && let Some(relation) = assembler.push(frame.clone()).unwrap()
            {
                completed.push(relation.identity.stream_id);
            }
        }
        assert_eq!(completed.len(), 2);
        assert!(completed.contains(&first.identity.stream_id));
        assert!(completed.contains(&second.identity.stream_id));
        assembler.finish().unwrap();
    }

    #[test]
    fn wp34_ops_duplicate_and_out_of_order_sequences_fail_closed() {
        let contract = contract(50, 3);
        let frames = encode_relation_stream(
            &contract,
            &[int_batch(&contract.schema, vec![1, 2, 3])],
            CoverageTrailer::complete(3),
            17,
        )
        .unwrap();
        let payloads = payload_frames(&frames);
        assert!(payloads.len() > 2);

        let mut duplicate = assembler_with(&contract);
        duplicate.push(frames[0].clone()).unwrap();
        duplicate
            .push(RelationIpcFrame::Payload(payloads[0].clone()))
            .unwrap();
        let error = duplicate
            .push(RelationIpcFrame::Payload(payloads[0].clone()))
            .unwrap_err();
        assert!(matches!(
            error.kind,
            RelationIpcErrorKind::DuplicateSequence { .. }
        ));
        assert!(matches!(error.coverage, FailureCoverage::Unknown { .. }));

        let mut out_of_order = assembler_with(&contract);
        out_of_order.push(frames[0].clone()).unwrap();
        let error = out_of_order
            .push(RelationIpcFrame::Payload(payloads[1].clone()))
            .unwrap_err();
        assert!(matches!(
            error.kind,
            RelationIpcErrorKind::OutOfOrderSequence { .. }
        ));
    }

    #[test]
    fn wp34_ops_truncation_and_corruption_are_distinct_typed_failures() {
        let contract = contract(60, 2);
        let frames = encode_relation_stream(
            &contract,
            &[int_batch(&contract.schema, vec![4, 5])],
            CoverageTrailer::complete(2),
            usize::MAX,
        )
        .unwrap();

        let mut truncated = frames.clone();
        let RelationIpcFrame::Payload(payload) = &mut truncated[1] else {
            panic!("encoded stream starts with open then payload");
        };
        payload.payload.pop();
        let received = as_u64(payload.payload.len());
        let RelationIpcFrame::IpcEnd(end) = &mut truncated[2] else {
            panic!("single-fragment stream ends after payload");
        };
        end.declared_ipc_bytes = received;
        let mut assembler = assembler_with(&contract);
        assembler.push(truncated[0].clone()).unwrap();
        assembler.push(truncated[1].clone()).unwrap();
        let error = assembler.push(truncated[2].clone()).unwrap_err();
        assert_eq!(error.kind, RelationIpcErrorKind::MissingArrowEndMarker);

        let mut corrupt = frames;
        let RelationIpcFrame::Payload(payload) = &mut corrupt[1] else {
            panic!("encoded stream starts with open then payload");
        };
        payload.payload[0] ^= 0x55;
        let mut assembler = assembler_with(&contract);
        assembler.push(corrupt[0].clone()).unwrap();
        assembler.push(corrupt[1].clone()).unwrap();
        let error = assembler.push(corrupt[2].clone()).unwrap_err();
        assert!(matches!(
            error.kind,
            RelationIpcErrorKind::ArrowIpcProfileMismatch(_)
        ));
    }

    #[test]
    fn structurally_valid_arrow_from_another_ipc_profile_is_rejected() {
        let contract = contract(159, 1);
        let batch = int_batch(&contract.schema, vec![8]);
        let options = IpcWriteOptions::try_new(64, false, MetadataVersion::V4).unwrap();
        let mut bytes = Vec::new();
        {
            let mut writer =
                StreamWriter::try_new_with_options(&mut bytes, contract.schema.as_ref(), options)
                    .unwrap();
            writer.write(&batch).unwrap();
            writer.finish().unwrap();
        }
        let mut assembler = assembler_with(&contract);
        assembler
            .push(RelationIpcFrame::Open(StreamOpen {
                header: FrameHeader::current(contract.identity, 0),
                requested_units: 1,
            }))
            .unwrap();
        assembler
            .push(RelationIpcFrame::Payload(RelationPayload {
                header: FrameHeader::current(contract.identity, 1),
                payload: bytes.clone(),
            }))
            .unwrap();
        let error = assembler
            .push(RelationIpcFrame::IpcEnd(IpcEndOfStream {
                header: FrameHeader::current(contract.identity, 2),
                declared_ipc_bytes: as_u64(bytes.len()),
                declared_batches: 1,
                declared_rows: 1,
            }))
            .unwrap_err();
        assert_eq!(
            error.kind,
            RelationIpcErrorKind::ArrowIpcProfileMismatch("message metadata version is not V5")
        );
    }

    #[test]
    fn a_second_schema_cannot_enter_one_stream_dictionary_scope() {
        let contract = contract(70, 2);
        let first =
            encode_arrow_stream(&contract.schema, &[int_batch(&contract.schema, vec![1])]).unwrap();
        let second =
            encode_arrow_stream(&contract.schema, &[int_batch(&contract.schema, vec![2])]).unwrap();
        let mut combined = first[..first.len() - IPC_STREAM_EOS.len()].to_vec();
        combined.extend_from_slice(&second);
        let open = RelationIpcFrame::Open(StreamOpen {
            header: FrameHeader::current(contract.identity, 0),
            requested_units: 2,
        });
        let payload = RelationIpcFrame::Payload(RelationPayload {
            header: FrameHeader::current(contract.identity, 1),
            payload: combined.clone(),
        });
        let end = RelationIpcFrame::IpcEnd(IpcEndOfStream {
            header: FrameHeader::current(contract.identity, 2),
            declared_ipc_bytes: as_u64(combined.len()),
            declared_batches: 2,
            declared_rows: 2,
        });
        let mut assembler = assembler_with(&contract);
        assembler.push(open).unwrap();
        assembler.push(payload).unwrap();
        let error = assembler.push(end).unwrap_err();
        assert_eq!(
            error.kind,
            RelationIpcErrorKind::ArrowIpcProfileMismatch(
                "schema message is absent, repeated, or out of order"
            )
        );
    }

    #[test]
    fn schema_contract_mismatch_is_rejected() {
        let contract = contract(80, 1);
        let other_schema = Arc::new(Schema::new(vec![Field::new(
            "other",
            DataType::Int32,
            false,
        )]));
        let bytes =
            encode_arrow_stream(&other_schema, &[int_batch(&other_schema, vec![7])]).unwrap();
        let mut assembler = assembler_with(&contract);
        assembler
            .push(RelationIpcFrame::Open(StreamOpen {
                header: FrameHeader::current(contract.identity, 0),
                requested_units: 1,
            }))
            .unwrap();
        assembler
            .push(RelationIpcFrame::Payload(RelationPayload {
                header: FrameHeader::current(contract.identity, 1),
                payload: bytes.clone(),
            }))
            .unwrap();
        let error = assembler
            .push(RelationIpcFrame::IpcEnd(IpcEndOfStream {
                header: FrameHeader::current(contract.identity, 2),
                declared_ipc_bytes: as_u64(bytes.len()),
                declared_batches: 1,
                declared_rows: 1,
            }))
            .unwrap_err();
        assert_eq!(error.kind, RelationIpcErrorKind::SchemaMismatch);
    }

    #[test]
    fn missing_trailer_and_terminal_are_not_complete() {
        let contract = contract(90, 1);
        let frames = encode_relation_stream(
            &contract,
            &[int_batch(&contract.schema, vec![1])],
            CoverageTrailer::complete(1),
            43,
        )
        .unwrap();
        let end_index = frames
            .iter()
            .position(|frame| matches!(frame, RelationIpcFrame::IpcEnd(_)))
            .unwrap();
        let trailer_index = frames
            .iter()
            .position(|frame| matches!(frame, RelationIpcFrame::CoverageTrailer(_)))
            .unwrap();

        let mut missing_trailer = assembler_with(&contract);
        for frame in &frames[..=end_index] {
            missing_trailer.push(frame.clone()).unwrap();
        }
        let error = missing_trailer.finish().unwrap_err();
        assert_eq!(error.kind, RelationIpcErrorKind::MissingCoverageTrailer);
        assert!(matches!(error.coverage, FailureCoverage::Unknown { .. }));

        let mut missing_terminal = assembler_with(&contract);
        for frame in &frames[..=trailer_index] {
            missing_terminal.push(frame.clone()).unwrap();
        }
        let error = missing_terminal.finish().unwrap_err();
        assert_eq!(error.kind, RelationIpcErrorKind::MissingTerminal);
        assert!(matches!(error.coverage, FailureCoverage::Unknown { .. }));

        let never_opened = assembler_with(&contract);
        let error = never_opened.finish().unwrap_err();
        assert_eq!(error.kind, RelationIpcErrorKind::MissingOpen);
        assert!(matches!(error.coverage, FailureCoverage::Unknown { .. }));
    }

    #[test]
    fn wp38_claim_001_negative_rejects_open_provider_coverage() {
        let fixture = claim_001_negative_fixture();
        let after = &fixture["mutation"]["after"];
        let requested_units = after["requested_units"]
            .as_u64()
            .expect("Claim 001 requested units");
        assert_eq!(after["completed_units"], 0);
        assert_eq!(after["remainders"], serde_json::json!([]));
        assert_eq!(after["state"], "open");

        let contract = contract(91, requested_units);
        let frames = encode_relation_stream(
            &contract,
            &[],
            CoverageTrailer::complete(requested_units),
            43,
        )
        .expect("construct the otherwise-valid production stream");
        let end_index = frames
            .iter()
            .position(|frame| matches!(frame, RelationIpcFrame::IpcEnd(_)))
            .expect("Claim 001 IPC end frame");
        let mut assembler = assembler_with(&contract);
        for frame in &frames[..=end_index] {
            assembler.push(frame.clone()).unwrap();
        }

        let error = assembler
            .finish()
            .expect_err("open coverage must fail closed");
        assert_eq!(error.kind, RelationIpcErrorKind::MissingCoverageTrailer);
        assert_eq!(
            error.coverage,
            FailureCoverage::Unknown {
                requested_units: Some(requested_units),
                completed_units: None,
            }
        );
        assert_eq!(
            fixture["expected_decoded"]["error"],
            "PROVIDER_REQUESTED_COVERAGE_OPEN"
        );
        assert_eq!(
            fixture["expected_decoded"]["relation_id"],
            after["relation_id"]
        );
    }

    #[test]
    fn partial_and_unknown_coverage_are_explicit_and_counted() {
        let partial_contract = contract(100, 3);
        let partial_trailer = CoverageTrailer {
            status: TerminalStatus::Partial,
            requested_units: 3,
            completed_units: 2,
            remainders: vec![CoverageRemainder {
                scope: CoverageScope([1; 16]),
                unit_count: 1,
                reason: RemainderReason::Unsupported,
            }],
        };
        let partial_frames = encode_relation_stream(
            &partial_contract,
            &[int_batch(&partial_contract.schema, vec![1, 2])],
            partial_trailer.clone(),
            41,
        )
        .unwrap();
        let partial = run_frames(&mut assembler_with(&partial_contract), partial_frames);
        assert_eq!(partial.trailer, partial_trailer);

        let unknown_contract = contract(110, 2);
        let unknown_trailer = CoverageTrailer {
            status: TerminalStatus::Unknown,
            requested_units: 2,
            completed_units: 1,
            remainders: vec![CoverageRemainder {
                scope: CoverageScope([2; 16]),
                unit_count: 1,
                reason: RemainderReason::Unknown,
            }],
        };
        let unknown_frames = encode_relation_stream(
            &unknown_contract,
            &[int_batch(&unknown_contract.schema, vec![9])],
            unknown_trailer.clone(),
            41,
        )
        .unwrap();
        let unknown = run_frames(&mut assembler_with(&unknown_contract), unknown_frames);
        assert_eq!(unknown.trailer, unknown_trailer);
    }

    #[test]
    fn invalid_coverage_and_terminal_mismatch_fail_closed() {
        let contract = contract(120, 2);
        let invalid = CoverageTrailer {
            status: TerminalStatus::Complete,
            requested_units: 2,
            completed_units: 1,
            remainders: vec![CoverageRemainder {
                scope: CoverageScope([3; 16]),
                unit_count: 1,
                reason: RemainderReason::Unsupported,
            }],
        };
        let error = encode_relation_stream(&contract, &[], invalid, 64).unwrap_err();
        assert!(matches!(
            error.kind,
            RelationIpcErrorKind::InvalidCoverage(_)
        ));

        let mut frames = encode_relation_stream(
            &contract,
            &[int_batch(&contract.schema, vec![1, 2])],
            CoverageTrailer::complete(2),
            64,
        )
        .unwrap();
        let RelationIpcFrame::Terminal(terminal) = frames.last_mut().unwrap() else {
            panic!("encoder terminates with a terminal frame");
        };
        terminal.status = TerminalStatus::Unknown;
        let mut assembler = assembler_with(&contract);
        let terminal = frames.pop().unwrap();
        for frame in frames {
            assembler.push(frame).unwrap();
        }
        let error = assembler.push(terminal).unwrap_err();
        assert_eq!(error.kind, RelationIpcErrorKind::TerminalStatusMismatch);
    }

    #[test]
    fn wp34_ops_flow_control_credit_is_bounded_and_cancellation_is_terminal() {
        let contract = contract(130, 1);
        let limits = RelationIpcLimits {
            initial_credit_bytes: 32,
            max_credit_bytes: 64,
            max_payload_bytes_per_frame: 32,
            ..RelationIpcLimits::default()
        };
        let frames = encode_relation_stream(
            &contract,
            &[int_batch(&contract.schema, vec![1])],
            CoverageTrailer::complete(1),
            32,
        )
        .unwrap();
        let payloads = payload_frames(&frames);
        let mut assembler = RelationIpcAssembler::new(limits).unwrap();
        assembler.register_contract(contract.clone()).unwrap();
        assembler.push(frames[0].clone()).unwrap();
        assembler
            .push(RelationIpcFrame::Payload(payloads[0].clone()))
            .unwrap();
        assembler
            .push(RelationIpcFrame::FlowControlAck(FlowControlAck {
                header: FrameHeader::current(contract.identity, 0),
                acknowledged_sequence: Some(payloads[0].header.sequence),
                released_bytes: as_u64(payloads[0].payload.len()),
                cancelled: false,
            }))
            .unwrap();
        assembler
            .push(RelationIpcFrame::Payload(payloads[1].clone()))
            .unwrap();

        let mut cancelled = RelationIpcAssembler::new(limits).unwrap();
        cancelled.register_contract(contract.clone()).unwrap();
        cancelled.push(frames[0].clone()).unwrap();
        let error = cancelled
            .push(RelationIpcFrame::FlowControlAck(FlowControlAck {
                header: FrameHeader::current(contract.identity, 0),
                acknowledged_sequence: None,
                released_bytes: 0,
                cancelled: true,
            }))
            .unwrap_err();
        assert_eq!(error.kind, RelationIpcErrorKind::Cancelled);
        assert!(matches!(error.coverage, FailureCoverage::Unknown { .. }));
        let error = cancelled.push(frames[0].clone()).unwrap_err();
        assert_eq!(error.kind, RelationIpcErrorKind::ClosedStream);
    }

    #[test]
    fn wp34_ops_frame_count_byte_budget_and_backpressure_are_enforced_before_allocation() {
        let contract = contract(140, 1);
        let frame_limits = RelationIpcLimits {
            max_payload_bytes_per_frame: 4,
            ..RelationIpcLimits::default()
        };
        let mut assembler = RelationIpcAssembler::new(frame_limits).unwrap();
        assembler.register_contract(contract.clone()).unwrap();
        assembler
            .push(RelationIpcFrame::Open(StreamOpen {
                header: FrameHeader::current(contract.identity, 0),
                requested_units: 1,
            }))
            .unwrap();
        let error = assembler
            .push(RelationIpcFrame::Payload(RelationPayload {
                header: FrameHeader::current(contract.identity, 1),
                payload: vec![0; 5],
            }))
            .unwrap_err();
        assert!(matches!(
            error.kind,
            RelationIpcErrorKind::LimitExceeded {
                resource: "payload frame bytes",
                ..
            }
        ));

        let frame_count_limits = RelationIpcLimits {
            max_frames_per_stream: 4,
            ..RelationIpcLimits::default()
        };
        let mut assembler = RelationIpcAssembler::new(frame_count_limits).unwrap();
        assembler.register_contract(contract.clone()).unwrap();
        assembler
            .push(RelationIpcFrame::Open(StreamOpen {
                header: FrameHeader::current(contract.identity, 0),
                requested_units: 1,
            }))
            .unwrap();
        for sequence in 1..4 {
            assembler
                .push(RelationIpcFrame::Payload(RelationPayload {
                    header: FrameHeader::current(contract.identity, sequence),
                    payload: vec![0],
                }))
                .unwrap();
        }
        let error = assembler
            .push(RelationIpcFrame::Payload(RelationPayload {
                header: FrameHeader::current(contract.identity, 4),
                payload: vec![4],
            }))
            .unwrap_err();
        assert!(matches!(
            error.kind,
            RelationIpcErrorKind::LimitExceeded {
                resource: "frames per stream",
                ..
            }
        ));

        let credit_limits = RelationIpcLimits {
            initial_credit_bytes: 4,
            max_credit_bytes: 4,
            ..RelationIpcLimits::default()
        };
        let mut assembler = RelationIpcAssembler::new(credit_limits).unwrap();
        assembler.register_contract(contract.clone()).unwrap();
        assembler
            .push(RelationIpcFrame::Open(StreamOpen {
                header: FrameHeader::current(contract.identity, 0),
                requested_units: 1,
            }))
            .unwrap();
        let error = assembler
            .push(RelationIpcFrame::Payload(RelationPayload {
                header: FrameHeader::current(contract.identity, 1),
                payload: vec![0; 5],
            }))
            .unwrap_err();
        assert!(matches!(
            error.kind,
            RelationIpcErrorKind::BackpressureExceeded { .. }
        ));
    }

    #[test]
    fn wp34_int_registration_binds_schema_fingerprint_and_exact_arrow_universe() {
        let valid = contract(151, 1);
        assembler_with(&valid);

        let mut wrong_fingerprint = valid.clone();
        wrong_fingerprint.identity.schema_fingerprint.0[0] ^= 1;
        let mut assembler = RelationIpcAssembler::new(RelationIpcLimits::default()).unwrap();
        let error = assembler.register_contract(wrong_fingerprint).unwrap_err();
        assert_eq!(error.kind, RelationIpcErrorKind::SchemaFingerprintMismatch);

        let mut metadata = valid.schema.metadata().clone();
        metadata.insert(
            ARROW_TYPE_UNIVERSE_KEY.to_owned(),
            "arrow-array@59.1.0|arrow-schema@59.1.0|arrow-ipc@59.1.0|metadata-v5".to_owned(),
        );
        let wrong_universe = RelationStreamContract {
            schema: Arc::new(Schema::new_with_metadata(
                valid.schema.fields().clone(),
                metadata,
            )),
            ..valid
        };
        let error = assembler.register_contract(wrong_universe).unwrap_err();
        assert_eq!(error.kind, RelationIpcErrorKind::ArrowTypeUniverseMismatch);
    }

    #[test]
    fn wp34_neg_opaque_schema_carriers_are_rejected_before_any_provider_bytes() {
        for (marker, field) in [
            (152, Field::new("semantic_payload", DataType::Utf8, false)),
            (153, Field::new("semantic_bytes", DataType::Binary, false)),
        ] {
            let schema = typed_schema(marker, vec![field]);
            let contract = contract_for_schema(marker, 1, schema);
            let mut assembler = RelationIpcAssembler::new(RelationIpcLimits::default()).unwrap();
            let error = assembler.register_contract(contract.clone()).unwrap_err();
            assert!(matches!(
                error.kind,
                RelationIpcErrorKind::OpaqueSemanticCarrier(_)
            ));
            let error = encode_relation_stream(
                &contract,
                &[],
                CoverageTrailer {
                    status: TerminalStatus::Partial,
                    requested_units: 1,
                    completed_units: 0,
                    remainders: vec![CoverageRemainder {
                        scope: CoverageScope([marker; 16]),
                        unit_count: 1,
                        reason: RemainderReason::Unsupported,
                    }],
                },
                64,
            )
            .unwrap_err();
            assert!(matches!(
                error.kind,
                RelationIpcErrorKind::OpaqueSemanticCarrier(_)
            ));
        }
    }

    #[test]
    fn completed_and_failed_streams_release_only_their_global_byte_budget() {
        let first = contract(154, 1);
        let second = contract(155, 1);
        let first_frames = encode_relation_stream(
            &first,
            &[int_batch(&first.schema, vec![1])],
            CoverageTrailer::complete(1),
            usize::MAX,
        )
        .unwrap();
        let second_frames = encode_relation_stream(
            &second,
            &[int_batch(&second.schema, vec![2])],
            CoverageTrailer::complete(1),
            usize::MAX,
        )
        .unwrap();
        let byte_limit = [first_frames.as_slice(), second_frames.as_slice()]
            .into_iter()
            .flat_map(payload_frames)
            .map(|payload| payload.payload.len())
            .max()
            .unwrap();
        let limits = RelationIpcLimits {
            max_payload_bytes_per_frame: byte_limit,
            max_payload_bytes_per_stream: byte_limit,
            max_total_payload_bytes: byte_limit,
            initial_credit_bytes: byte_limit,
            max_credit_bytes: byte_limit,
            ..RelationIpcLimits::default()
        };
        let mut sequential = RelationIpcAssembler::new(limits).unwrap();
        sequential.register_contract(first.clone()).unwrap();
        sequential.register_contract(second.clone()).unwrap();
        run_frames(&mut sequential, first_frames.clone());
        run_frames(&mut sequential, second_frames.clone());
        sequential.finish().unwrap();

        let failed = contract(156, 1);
        let survivor = contract(157, 1);
        let mut failed_frames = encode_relation_stream(
            &failed,
            &[int_batch(&failed.schema, vec![3])],
            CoverageTrailer::complete(1),
            usize::MAX,
        )
        .unwrap();
        let survivor_frames = encode_relation_stream(
            &survivor,
            &[int_batch(&survivor.schema, vec![4])],
            CoverageTrailer::complete(1),
            usize::MAX,
        )
        .unwrap();
        let RelationIpcFrame::Payload(payload) = &mut failed_frames[1] else {
            panic!("one-fragment test stream has a payload after open");
        };
        payload.payload[0] ^= 0x55;
        let mut interleaved = RelationIpcAssembler::new(limits).unwrap();
        interleaved.register_contract(failed.clone()).unwrap();
        interleaved.register_contract(survivor.clone()).unwrap();
        interleaved.push(failed_frames[0].clone()).unwrap();
        interleaved.push(survivor_frames[0].clone()).unwrap();
        interleaved.push(failed_frames[1].clone()).unwrap();
        let error = interleaved.push(failed_frames[2].clone()).unwrap_err();
        assert!(matches!(
            error.kind,
            RelationIpcErrorKind::ArrowIpcProfileMismatch(_)
        ));
        let completed_survivor = run_frames(&mut interleaved, survivor_frames[1..].to_vec());
        assert_eq!(completed_survivor.identity, survivor.identity);
        interleaved.finish().unwrap();
    }

    #[test]
    fn wp34_ops_cancellation_is_terminal_after_ipc_end_or_coverage_trailer() {
        let contract = contract(158, 1);
        let frames = encode_relation_stream(
            &contract,
            &[int_batch(&contract.schema, vec![1])],
            CoverageTrailer::complete(1),
            usize::MAX,
        )
        .unwrap();
        let ipc_end = frames
            .iter()
            .position(|frame| matches!(frame, RelationIpcFrame::IpcEnd(_)))
            .unwrap();
        let trailer = frames
            .iter()
            .position(|frame| matches!(frame, RelationIpcFrame::CoverageTrailer(_)))
            .unwrap();
        let cancel = RelationIpcFrame::FlowControlAck(FlowControlAck {
            header: FrameHeader::current(contract.identity, 0),
            acknowledged_sequence: None,
            released_bytes: 0,
            cancelled: true,
        });

        for terminal_prefix in [ipc_end, trailer] {
            let mut assembler = assembler_with(&contract);
            for frame in &frames[..=terminal_prefix] {
                assembler.push(frame.clone()).unwrap();
            }
            let error = assembler.push(cancel.clone()).unwrap_err();
            assert_eq!(error.kind, RelationIpcErrorKind::Cancelled);
            let error = assembler.push(frames[0].clone()).unwrap_err();
            assert_eq!(error.kind, RelationIpcErrorKind::ClosedStream);
        }
    }

    #[cfg(feature = "daemon")]
    #[test]
    fn wp34_int_production_provider_schemas_interoperate_with_the_relation_stream_boundary() {
        let schemas = crate::pyrefly_service::PyreflyRelation::ALL
            .into_iter()
            .map(|relation| (relation.family_code(), relation.schema()))
            .chain(
                crate::rustc_relation_schema::RustcRelation::ALL
                    .into_iter()
                    .map(|relation| (relation.family_code(), relation.schema())),
            );
        for (family_code, schema) in schemas {
            let marker = u8::try_from(family_code).unwrap();
            let contract = contract_for_schema(marker, 0, schema);
            let frames =
                encode_relation_stream(&contract, &[], CoverageTrailer::complete(0), 113).unwrap();
            let relation = run_frames(&mut assembler_with(&contract), frames);
            assert_eq!(relation.schema.as_ref(), contract.schema.as_ref());
            assert!(relation.batches.is_empty());
        }
    }

    #[test]
    fn wp34_neg_raw_json_cannot_masquerade_as_semantic_row_payload() {
        let contract = contract(150, 1);
        let json = br#"{"rows":[{"value":1}]}"#.to_vec();
        let mut assembler = assembler_with(&contract);
        assembler
            .push(RelationIpcFrame::Open(StreamOpen {
                header: FrameHeader::current(contract.identity, 0),
                requested_units: 1,
            }))
            .unwrap();
        assembler
            .push(RelationIpcFrame::Payload(RelationPayload {
                header: FrameHeader::current(contract.identity, 1),
                payload: json.clone(),
            }))
            .unwrap();
        let error = assembler
            .push(RelationIpcFrame::IpcEnd(IpcEndOfStream {
                header: FrameHeader::current(contract.identity, 2),
                declared_ipc_bytes: as_u64(json.len()),
                declared_batches: 1,
                declared_rows: 1,
            }))
            .unwrap_err();
        assert_eq!(error.kind, RelationIpcErrorKind::MissingArrowEndMarker);
    }
}
