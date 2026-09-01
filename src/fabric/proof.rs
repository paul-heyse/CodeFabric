//! Executable proof and capability closure for one exact fabric candidate.
//!
//! The evaluator consumes independently owned semantic expectations and required causal faults,
//! joins them to exact candidate execution, coverage, violations, and provenance edges, and emits
//! queryable Arrow relations. A terminal pass is derived only when every requested oracle has
//! independent expectations, complete coverage, closed provenance, and detected required faults.
//! No persisted flag or opaque serialized row payload participates in the decision.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use arrow_array::builder::FixedSizeBinaryBuilder;
use arrow_array::{ArrayRef, BooleanArray, Int8Array, Int16Array, RecordBatch, UInt64Array};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use thiserror::Error;

use super::activation::{OverlaySegmentSetRef, PolicySetRef, TableVersionSetRef};
use super::command::{
    ApplicationReleaseRef, EpochId, InputReleaseRef, ProgramReleaseRef, ProviderReleaseRef,
    ProviderSetRef, ResourceEnvelopeRef, SourceAuthorityRef, SourceGeneration, SourceImageSetRef,
};

mod delta_history;

#[cfg(feature = "daemon")]
pub use delta_history::DeltaActivationCandidateProofRelations;
pub use delta_history::{
    ProofDeltaHistoryPublication, ProofDeltaHistoryTargets, ProofDeltaWorkspaceRoot,
    ProofDeltaWriteIdentity, ProofRelationsDeltaError, persist_proof_relations,
    provision_proof_relation_histories, reopen_proof_relations,
};

const MAX_ORACLES: usize = 4_096;
const MAX_CAPABILITIES: usize = 4_096;
const MAX_EXPECTATIONS: usize = 65_536;
const MAX_FAULTS: usize = 65_536;
const MAX_SCOPES_PER_ORACLE: usize = 65_536;
const MAX_TOTAL_COVERAGE_SCOPES: usize = 1_048_576;
const MAX_VIOLATIONS: usize = 262_144;
const MAX_PROVENANCE_EDGES: usize = 1_048_576;
const MAX_PROVENANCE_TRAVERSAL_BOUND: usize = 16_777_216;

macro_rules! proof_identity {
    ($(#[$meta:meta])* $name:ident, $width:expr) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; $width]);

        impl $name {
            /// Construct a nonzero typed identity.
            #[must_use]
            pub const fn new(bytes: [u8; $width]) -> Option<Self> {
                let mut index = 0;
                while index < $width {
                    if bytes[index] != 0 {
                        return Some(Self(bytes));
                    }
                    index += 1;
                }
                None
            }

            /// Borrow the canonical identity bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; $width] {
                &self.0
            }
        }
    };
}

proof_identity!(
    /// Stable executable-oracle identity.
    OracleId,
    16
);
proof_identity!(
    /// Stable capability identity whose status is derived from required oracles.
    CapabilityId,
    16
);
proof_identity!(
    /// Independently authored semantic-expectation identity.
    ExpectationId,
    16
);
proof_identity!(
    /// Independently authored required causal fault or mutant identity.
    CausalFaultId,
    16
);
proof_identity!(
    /// Stable violation-row identity emitted by an executable oracle.
    ViolationId,
    16
);
proof_identity!(
    /// Stable identity of one exact oracle execution.
    ProofRunId,
    16
);
proof_identity!(
    /// Stable requested coverage-scope identity.
    CoverageScopeId,
    16
);
proof_identity!(
    /// Stable proof relation identity supplied by the application proof contract.
    ProofRelationId,
    16
);
proof_identity!(
    /// Accountable ownership identity for producers, authors, and reviewers.
    ProofOwnerId,
    32
);
proof_identity!(
    /// Exact implementation identity of an executable oracle.
    OracleImplementationRef,
    32
);
proof_identity!(
    /// Independently authored semantic claim identity.
    SemanticClaimRef,
    32
);
proof_identity!(
    /// Human-reviewable source anchor for an expectation.
    SourceAnchorRef,
    32
);
proof_identity!(
    /// Exact program/input identity for a causal fault.
    CausalFaultProgramRef,
    32
);

/// Exact inputs selected by the candidate before a proof receipt exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofCandidatePins {
    pub epoch: EpochId,
    pub input_release: InputReleaseRef,
    pub program_release: ProgramReleaseRef,
    pub application_release: ApplicationReleaseRef,
    pub source_authority: SourceAuthorityRef,
    pub source_generation: SourceGeneration,
    pub source_images: SourceImageSetRef,
    pub provider_release: ProviderReleaseRef,
    pub provider_set: ProviderSetRef,
    pub table_versions: TableVersionSetRef,
    pub overlay_segments: OverlaySegmentSetRef,
    pub policy_set: PolicySetRef,
    pub resource_envelope: ResourceEnvelopeRef,
}

/// Authorship, review, and acceptance provenance for an independent input row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndependentEvidenceAuthority {
    pub author: ProofOwnerId,
    pub reviewer: ProofOwnerId,
    pub acceptance_authority: ProofOwnerId,
}

/// Requested executable oracle and the exact relation its violations must inhabit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleRequest {
    pub oracle_id: OracleId,
    pub implementation: OracleImplementationRef,
    pub violation_relation: ProofRelationId,
    pub requested_scopes: Vec<CoverageScopeId>,
}

/// Capability whose status must be computed for the candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityRequest {
    pub capability_id: CapabilityId,
}

/// Relational many-to-many requirement between a capability and an oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityOracleRequirement {
    pub capability_id: CapabilityId,
    pub oracle_id: OracleId,
}

/// Independently authored semantic expectation consumed by one oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticExpectation {
    pub expectation_id: ExpectationId,
    pub oracle_id: OracleId,
    pub coverage_scope: CoverageScopeId,
    pub claim: SemanticClaimRef,
    pub source_anchor: SourceAnchorRef,
    pub authority: IndependentEvidenceAuthority,
}

/// Causal effect that a required injected fault must demonstrate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequiredCausalEffect {
    StructuralRejection,
    SemanticDiscrimination,
}

impl RequiredCausalEffect {
    /// Stable Arrow relation code for this causal-effect class.
    #[must_use]
    pub const fn code(self) -> i8 {
        match self {
            Self::StructuralRejection => 1,
            Self::SemanticDiscrimination => 2,
        }
    }
}

/// Independently authored causal fault. Every row is mandatory by construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequiredCausalFault {
    pub fault_id: CausalFaultId,
    pub oracle_id: OracleId,
    pub coverage_scope: CoverageScopeId,
    pub program: CausalFaultProgramRef,
    pub required_effect: RequiredCausalEffect,
    pub authority: IndependentEvidenceAuthority,
}

/// Why one requested coverage scope could not be completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageUnavailableReason {
    MissingInput,
    ProviderUnavailable,
    ResourceLimit,
    Cancelled,
    InvalidCandidate,
    Unknown,
}

impl CoverageUnavailableReason {
    /// Stable Arrow relation code for this unavailable reason.
    #[must_use]
    pub const fn code(self) -> i8 {
        match self {
            Self::MissingInput => 1,
            Self::ProviderUnavailable => 2,
            Self::ResourceLimit => 3,
            Self::Cancelled => 4,
            Self::InvalidCandidate => 5,
            Self::Unknown => 6,
        }
    }
}

/// Explicit unavailable coverage row for one requested scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnavailableCoverage {
    pub scope: CoverageScopeId,
    pub reason: CoverageUnavailableReason,
}

/// Actual execution evidence for one requested oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleExecution {
    pub oracle_id: OracleId,
    pub run_id: ProofRunId,
    pub candidate_pins: ProofCandidatePins,
    pub completed_scopes: Vec<CoverageScopeId>,
    pub unavailable_scopes: Vec<UnavailableCoverage>,
}

/// Typed violation class; fault-scoped violations prove causality and do not fail the baseline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofViolationKind {
    Invariant,
    SemanticMismatch,
    AuthorizationMismatch,
    UnknownSemanticsMismatch,
    ConstructionRejected,
}

impl ProofViolationKind {
    /// Stable Arrow relation code for this violation class.
    #[must_use]
    pub const fn code(self) -> i8 {
        match self {
            Self::Invariant => 1,
            Self::SemanticMismatch => 2,
            Self::AuthorizationMismatch => 3,
            Self::UnknownSemanticsMismatch => 4,
            Self::ConstructionRejected => 5,
        }
    }
}

/// Violation relation row emitted by an executable oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofViolation {
    pub violation_id: ViolationId,
    pub oracle_id: OracleId,
    pub expectation_id: Option<ExpectationId>,
    pub fault_id: Option<CausalFaultId>,
    pub kind: ProofViolationKind,
}

/// Observed outcome of executing one required causal fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CausalFaultOutcome {
    Detected { violation_id: ViolationId },
    Survived,
    Unavailable { reason: CoverageUnavailableReason },
}

/// Actual causal-fault execution row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CausalFaultExecution {
    pub fault_id: CausalFaultId,
    pub outcome: CausalFaultOutcome,
}

/// Typed node in the proof provenance graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProvenanceSubject {
    OracleRun(ProofRunId),
    OracleImplementation(OracleImplementationRef),
    ViolationRelation(ProofRelationId),
    Capability(CapabilityId),
    Expectation(ExpectationId),
    SemanticClaim(SemanticClaimRef),
    SourceAnchor(SourceAnchorRef),
    CausalFault(CausalFaultId),
    CausalFaultProgram(CausalFaultProgramRef),
    Epoch(EpochId),
    InputRelease(InputReleaseRef),
    ProgramRelease(ProgramReleaseRef),
    ApplicationRelease(ApplicationReleaseRef),
    SourceAuthority(SourceAuthorityRef),
    SourceGeneration(SourceGeneration),
    SourceImages(SourceImageSetRef),
    ProviderRelease(ProviderReleaseRef),
    ProviderSet(ProviderSetRef),
    TableVersions(TableVersionSetRef),
    OverlaySegments(OverlaySegmentSetRef),
    PolicySet(PolicySetRef),
    ResourceEnvelope(ResourceEnvelopeRef),
}

impl ProvenanceSubject {
    /// Stable Arrow relation code for this provenance-subject family.
    #[must_use]
    pub const fn kind_code(self) -> i8 {
        match self {
            Self::OracleRun(_) => 1,
            Self::OracleImplementation(_) => 2,
            Self::ViolationRelation(_) => 3,
            Self::Capability(_) => 4,
            Self::Expectation(_) => 5,
            Self::SemanticClaim(_) => 6,
            Self::SourceAnchor(_) => 7,
            Self::CausalFault(_) => 8,
            Self::CausalFaultProgram(_) => 9,
            Self::Epoch(_) => 10,
            Self::InputRelease(_) => 11,
            Self::ProgramRelease(_) => 12,
            Self::ApplicationRelease(_) => 13,
            Self::SourceAuthority(_) => 14,
            Self::SourceGeneration(_) => 15,
            Self::SourceImages(_) => 16,
            Self::ProviderRelease(_) => 17,
            Self::ProviderSet(_) => 18,
            Self::TableVersions(_) => 19,
            Self::OverlaySegments(_) => 20,
            Self::PolicySet(_) => 21,
            Self::ResourceEnvelope(_) => 22,
        }
    }

    fn encoded_id(self) -> [u8; 32] {
        let mut encoded = [0_u8; 32];
        match self {
            Self::OracleRun(value) => encoded[..16].copy_from_slice(value.as_bytes()),
            Self::OracleImplementation(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::ViolationRelation(value) => encoded[..16].copy_from_slice(value.as_bytes()),
            Self::Capability(value) => encoded[..16].copy_from_slice(value.as_bytes()),
            Self::Expectation(value) => encoded[..16].copy_from_slice(value.as_bytes()),
            Self::SemanticClaim(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::SourceAnchor(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::CausalFault(value) => encoded[..16].copy_from_slice(value.as_bytes()),
            Self::CausalFaultProgram(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::Epoch(value) => encoded[..16].copy_from_slice(value.as_bytes()),
            Self::InputRelease(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::ProgramRelease(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::ApplicationRelease(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::SourceAuthority(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::SourceGeneration(value) => {
                encoded[24..].copy_from_slice(&value.get().to_be_bytes());
            }
            Self::SourceImages(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::ProviderRelease(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::ProviderSet(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::TableVersions(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::OverlaySegments(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::PolicySet(value) => encoded.copy_from_slice(value.as_bytes()),
            Self::ResourceEnvelope(value) => encoded.copy_from_slice(value.as_bytes()),
        }
        encoded
    }
}

/// Direct or multi-input lineage edge emitted with proof execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofProvenanceEdge {
    pub from: ProvenanceSubject,
    pub to: ProvenanceSubject,
}

/// Candidate evidence emitted by the programmatic production/provider path.
#[derive(Clone, Copy, Debug)]
pub struct CandidateProofInput<'a> {
    pub producer_owner: ProofOwnerId,
    pub candidate_pins: ProofCandidatePins,
    pub oracle_requests: &'a [OracleRequest],
    pub capability_requests: &'a [CapabilityRequest],
    pub capability_requirements: &'a [CapabilityOracleRequirement],
    pub oracle_executions: &'a [OracleExecution],
    pub violations: &'a [ProofViolation],
    pub fault_executions: &'a [CausalFaultExecution],
    pub provenance_edges: &'a [ProofProvenanceEdge],
}

/// Expectation and fault input supplied through the independently owned proof port.
#[derive(Clone, Copy, Debug)]
pub struct IndependentProofInput<'a> {
    pub expectations: &'a [SemanticExpectation],
    pub required_faults: &'a [RequiredCausalFault],
}

/// Terminal status derived from exact proof rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofTerminalStatus {
    Pass,
    Fail,
    Unknown,
}

impl ProofTerminalStatus {
    /// Stable Arrow relation code for this terminal status.
    #[must_use]
    pub const fn code(self) -> i8 {
        match self {
            Self::Pass => 1,
            Self::Fail => 2,
            Self::Unknown => 3,
        }
    }
}

/// Typed observation of one executable-oracle result from a completed proof evaluation.
///
/// This is the application-owned bridge for downstream capability derivation. It exposes exact
/// identities and the derived terminal state without exposing mutable proof internals or asking a
/// downstream consumer to reinterpret Arrow column positions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofOracleObservation {
    epoch: EpochId,
    oracle_id: OracleId,
    implementation: OracleImplementationRef,
    run_id: Option<ProofRunId>,
    status: ProofTerminalStatus,
}

impl ProofOracleObservation {
    #[must_use]
    pub const fn epoch(self) -> EpochId {
        self.epoch
    }

    #[must_use]
    pub const fn oracle_id(self) -> OracleId {
        self.oracle_id
    }

    #[must_use]
    pub const fn implementation(self) -> OracleImplementationRef {
        self.implementation
    }

    #[must_use]
    pub const fn run_id(self) -> Option<ProofRunId> {
        self.run_id
    }

    #[must_use]
    pub const fn status(self) -> ProofTerminalStatus {
        self.status
    }
}

/// Capability status derived from its required oracle results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofCapabilityStatus {
    Supported,
    Unavailable,
    Unknown,
}

impl ProofCapabilityStatus {
    /// Stable Arrow relation code for this capability status.
    #[must_use]
    pub const fn code(self) -> i8 {
        match self {
            Self::Supported => 1,
            Self::Unavailable => 2,
            Self::Unknown => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Computed closure state for one requested coverage scope.
pub enum CoverageState {
    Completed,
    Unavailable,
    Uncovered,
}

impl CoverageState {
    /// Stable Arrow relation code for this coverage state.
    #[must_use]
    pub const fn code(self) -> i8 {
        match self {
            Self::Completed => 1,
            Self::Unavailable => 2,
            Self::Uncovered => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Computed outcome state for one required causal fault.
pub enum FaultResultState {
    Detected,
    Survived,
    Unavailable,
    Missing,
    EvidenceMismatch,
}

impl FaultResultState {
    /// Stable Arrow relation code for this required-fault state.
    #[must_use]
    pub const fn code(self) -> i8 {
        match self {
            Self::Detected => 1,
            Self::Survived => 2,
            Self::Unavailable => 3,
            Self::Missing => 4,
            Self::EvidenceMismatch => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Terminal impact of one typed proof issue.
pub enum IssueDisposition {
    Failure,
    Unknown,
}

impl IssueDisposition {
    /// Stable Arrow relation code for this issue disposition.
    #[must_use]
    pub const fn code(self) -> i8 {
        match self {
            Self::Failure => 1,
            Self::Unknown => 2,
        }
    }
}

/// Stable issue codes emitted as relation rows rather than free-form proof messages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofIssueCode {
    NoOracleRequests,
    EmptyExpectationSet,
    OracleHasNoExpectation,
    OracleHasNoRequestedCoverage,
    MissingOracleExecution,
    CandidatePinMismatch,
    UncoveredRequestedScope,
    ExplicitUnavailableScope,
    ProducerAuthoredExpectation,
    ExpectationReviewNotIndependent,
    OracleHasNoRequiredFault,
    ProducerAuthoredFault,
    FaultReviewNotIndependent,
    BaselineViolation,
    MissingFaultExecution,
    FaultExecutionUnavailable,
    RequiredFaultSurvived,
    FaultEvidenceMismatch,
    MissingProvenance,
    CapabilityHasNoOracle,
}

impl ProofIssueCode {
    /// Stable Arrow relation code for this proof issue.
    #[must_use]
    pub const fn code(self) -> i16 {
        match self {
            Self::NoOracleRequests => 1,
            Self::EmptyExpectationSet => 2,
            Self::OracleHasNoExpectation => 3,
            Self::OracleHasNoRequestedCoverage => 4,
            Self::MissingOracleExecution => 5,
            Self::CandidatePinMismatch => 6,
            Self::UncoveredRequestedScope => 7,
            Self::ExplicitUnavailableScope => 8,
            Self::ProducerAuthoredExpectation => 9,
            Self::ExpectationReviewNotIndependent => 10,
            Self::OracleHasNoRequiredFault => 11,
            Self::ProducerAuthoredFault => 12,
            Self::FaultReviewNotIndependent => 13,
            Self::BaselineViolation => 14,
            Self::MissingFaultExecution => 15,
            Self::FaultExecutionUnavailable => 16,
            Self::RequiredFaultSurvived => 17,
            Self::FaultEvidenceMismatch => 18,
            Self::MissingProvenance => 19,
            Self::CapabilityHasNoOracle => 20,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DerivedIssue {
    oracle_id: Option<OracleId>,
    capability_id: Option<CapabilityId>,
    code: ProofIssueCode,
    disposition: IssueDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OracleResultRow {
    oracle_id: OracleId,
    implementation: OracleImplementationRef,
    violation_relation: ProofRelationId,
    run_id: Option<ProofRunId>,
    status: ProofTerminalStatus,
    requested_scope_count: u64,
    completed_scope_count: u64,
    unavailable_scope_count: u64,
    uncovered_scope_count: u64,
    expectation_count: u64,
    baseline_violation_count: u64,
    required_fault_count: u64,
    detected_fault_count: u64,
    provenance_closed: bool,
    missing_provenance_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CoverageResultRow {
    oracle_id: OracleId,
    scope_id: CoverageScopeId,
    state: CoverageState,
    unavailable_reason: Option<CoverageUnavailableReason>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FaultResultRow {
    fault_id: CausalFaultId,
    oracle_id: OracleId,
    coverage_scope: CoverageScopeId,
    program: CausalFaultProgramRef,
    required_effect: RequiredCausalEffect,
    authority: IndependentEvidenceAuthority,
    state: FaultResultState,
    violation_id: Option<ViolationId>,
    unavailable_reason: Option<CoverageUnavailableReason>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CapabilityResultRow {
    capability_id: CapabilityId,
    status: ProofCapabilityStatus,
    required_oracle_count: u64,
    pass_count: u64,
    fail_count: u64,
    unknown_count: u64,
}

/// One queryable Arrow relation and its exact schema contract.
#[derive(Clone, Debug)]
pub struct ProofRelationOutput {
    schema: SchemaRef,
    batch: RecordBatch,
}

impl ProofRelationOutput {
    fn try_new(schema: SchemaRef, batch: RecordBatch) -> Result<Self, ProofError> {
        if batch.schema_ref().as_ref() != schema.as_ref() {
            return Err(ProofError::ArrowSchemaDrift);
        }
        Ok(Self { schema, batch })
    }

    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub const fn batch(&self) -> &RecordBatch {
        &self.batch
    }
}

/// Queryable proof/capability relations for one exact candidate.
#[derive(Clone, Debug)]
pub struct ProofRelations {
    terminal: ProofTerminalStatus,
    candidate_pins: ProofCandidatePins,
    oracle_observations: Arc<[ProofOracleObservation]>,
    proof_run: ProofRelationOutput,
    oracle_results: ProofRelationOutput,
    capability_results: ProofRelationOutput,
    expectations: ProofRelationOutput,
    coverage_results: ProofRelationOutput,
    fault_results: ProofRelationOutput,
    violation_results: ProofRelationOutput,
    provenance_edges: ProofRelationOutput,
    issues: ProofRelationOutput,
}

/// Architectural proof-relation role. Catalog names remain model supplied; this enum only
/// identifies which computed output is being bound.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProofRelationKind {
    ProofRun,
    OracleResult,
    CapabilityResult,
    Expectation,
    CoverageResult,
    FaultResult,
    ViolationResult,
    ProvenanceEdge,
    Issue,
}

impl ProofRelationKind {
    /// Complete computed proof output census.
    pub const ALL: [Self; 9] = [
        Self::ProofRun,
        Self::OracleResult,
        Self::CapabilityResult,
        Self::Expectation,
        Self::CoverageResult,
        Self::FaultResult,
        Self::ViolationResult,
        Self::ProvenanceEdge,
        Self::Issue,
    ];
}

impl ProofRelations {
    /// Terminal candidate status derived by the evaluator; never independently settable.
    #[must_use]
    pub const fn terminal(&self) -> ProofTerminalStatus {
        self.terminal
    }

    /// Exact candidate pins consumed by the evaluation that produced every relation.
    #[must_use]
    pub const fn candidate_pins(&self) -> ProofCandidatePins {
        self.candidate_pins
    }

    /// Typed oracle results derived by the same evaluation that emitted the Arrow proof tables.
    #[must_use]
    pub fn oracle_observations(&self) -> &[ProofOracleObservation] {
        &self.oracle_observations
    }

    /// Resolve one architectural output without assigning a static catalog/table name.
    #[must_use]
    pub const fn relation(&self, kind: ProofRelationKind) -> &ProofRelationOutput {
        match kind {
            ProofRelationKind::ProofRun => &self.proof_run,
            ProofRelationKind::OracleResult => &self.oracle_results,
            ProofRelationKind::CapabilityResult => &self.capability_results,
            ProofRelationKind::Expectation => &self.expectations,
            ProofRelationKind::CoverageResult => &self.coverage_results,
            ProofRelationKind::FaultResult => &self.fault_results,
            ProofRelationKind::ViolationResult => &self.violation_results,
            ProofRelationKind::ProvenanceEdge => &self.provenance_edges,
            ProofRelationKind::Issue => &self.issues,
        }
    }
}

/// Invalid proof input or Arrow realization.
#[derive(Debug, Error)]
pub enum ProofError {
    #[error("proof input resource limit exceeded: {0}")]
    ResourceLimit(&'static str),
    #[error("exact candidate pin uses the all-zero sentinel")]
    ZeroCandidatePin,
    #[error("duplicate oracle request")]
    DuplicateOracle,
    #[error("duplicate capability request")]
    DuplicateCapability,
    #[error("duplicate capability/oracle requirement")]
    DuplicateCapabilityRequirement,
    #[error("capability requirement references an unknown capability or oracle")]
    UnknownCapabilityRequirement,
    #[error("duplicate expectation identity")]
    DuplicateExpectation,
    #[error("expectation references an unknown oracle or unrequested scope")]
    UnknownExpectationReference,
    #[error("duplicate causal-fault identity")]
    DuplicateFault,
    #[error("causal fault references an unknown oracle or unrequested scope")]
    UnknownFaultReference,
    #[error("duplicate oracle execution")]
    DuplicateOracleExecution,
    #[error("oracle execution references an unknown oracle")]
    UnknownOracleExecution,
    #[error("coverage has a duplicate, overlapping, or unrequested scope")]
    InvalidCoverage,
    #[error("duplicate violation identity")]
    DuplicateViolation,
    #[error("violation references an unknown or mismatched oracle, expectation, or fault")]
    UnknownViolationReference,
    #[error("duplicate causal-fault execution")]
    DuplicateFaultExecution,
    #[error("causal-fault execution references an unknown fault")]
    UnknownFaultExecution,
    #[error("duplicate provenance edge")]
    DuplicateProvenanceEdge,
    #[error("Arrow proof relation schema drift")]
    ArrowSchemaDrift,
    #[error(transparent)]
    Arrow(#[from] ArrowError),
}

/// Execute proof/capability closure and materialize every result as typed Arrow relations.
///
/// # Errors
///
/// Rejects malformed relational input: duplicate identities, dangling references, contradictory
/// coverage, invalid exact pins, resource-bound violations, and Arrow schema construction errors.
/// Ordinary proof failure or missing evidence is returned as `Fail` or `Unknown` relation rows.
pub fn evaluate_candidate_proof(
    candidate: &CandidateProofInput<'_>,
    independent: &IndependentProofInput<'_>,
) -> Result<ProofRelations, ProofError> {
    validate_resource_bounds(candidate, independent)?;
    validate_candidate_pins(&candidate.candidate_pins)?;

    let oracles = index_oracles(candidate.oracle_requests)?;
    let capabilities = index_capabilities(candidate.capability_requests)?;
    let capability_requirements =
        index_capability_requirements(candidate.capability_requirements, &capabilities, &oracles)?;
    let expectations = index_expectations(independent.expectations, &oracles)?;
    let faults = index_faults(independent.required_faults, &oracles)?;
    let executions = index_oracle_executions(candidate.oracle_executions, &oracles)?;
    let violations = index_violations(candidate.violations, &oracles, &expectations, &faults)?;
    let fault_executions = index_fault_executions(candidate.fault_executions, &faults)?;
    let adjacency = index_provenance(candidate.provenance_edges)?;

    let expectations_by_oracle = group_expectations(independent.expectations);
    let faults_by_oracle = group_faults(independent.required_faults);
    let violations_by_oracle = group_baseline_violations(candidate.violations);
    let capabilities_by_oracle = group_capabilities_by_oracle(candidate.capability_requirements);

    let mut issues = initial_candidate_issues(candidate.oracle_requests, independent.expectations);

    let mut oracle_rows = Vec::with_capacity(oracles.len());
    let mut coverage_rows = Vec::new();
    let mut fault_rows = Vec::with_capacity(faults.len());
    for request in oracles.values() {
        let oracle_expectations = expectations_by_oracle
            .get(&request.oracle_id)
            .map_or(&[][..], Vec::as_slice);
        let oracle_faults = faults_by_oracle
            .get(&request.oracle_id)
            .map_or(&[][..], Vec::as_slice);
        let baseline_violations = violations_by_oracle
            .get(&request.oracle_id)
            .map_or(&[][..], Vec::as_slice);
        let required_capabilities = capabilities_by_oracle
            .get(&request.oracle_id)
            .map_or(&[][..], Vec::as_slice);
        let execution = executions.get(&request.oracle_id).copied();

        let evaluation = evaluate_oracle(
            candidate.producer_owner,
            &candidate.candidate_pins,
            request,
            oracle_expectations,
            oracle_faults,
            baseline_violations,
            required_capabilities,
            execution,
            &violations,
            &fault_executions,
            &adjacency,
            &mut issues,
            &mut coverage_rows,
            &mut fault_rows,
        );
        oracle_rows.push(evaluation);
    }

    let oracle_statuses = oracle_rows
        .iter()
        .map(|row| (row.oracle_id, row.status))
        .collect::<BTreeMap<_, _>>();
    let capability_rows = evaluate_capabilities(
        &capabilities,
        &capability_requirements,
        &oracle_statuses,
        &mut issues,
    );

    let terminal = derive_candidate_terminal(&oracle_rows, &capability_rows, &issues);
    let proof_run = build_proof_run_relation(
        candidate,
        independent,
        terminal,
        &oracle_rows,
        &capability_rows,
    )?;
    let oracle_results = build_oracle_relation(candidate.candidate_pins.epoch, &oracle_rows)?;
    let capability_results =
        build_capability_relation(candidate.candidate_pins.epoch, &capability_rows)?;
    let expectations =
        build_expectation_relation(candidate.candidate_pins.epoch, independent.expectations)?;
    let coverage_results = build_coverage_relation(candidate.candidate_pins.epoch, &coverage_rows)?;
    let fault_results = build_fault_relation(candidate.candidate_pins.epoch, &fault_rows)?;
    let violation_results =
        build_violation_relation(candidate.candidate_pins.epoch, candidate.violations)?;
    let provenance_edges =
        build_provenance_relation(candidate.candidate_pins.epoch, candidate.provenance_edges)?;
    let issues = build_issue_relation(candidate.candidate_pins.epoch, &issues)?;
    let oracle_observations = oracle_rows
        .iter()
        .map(|row| ProofOracleObservation {
            epoch: candidate.candidate_pins.epoch,
            oracle_id: row.oracle_id,
            implementation: row.implementation,
            run_id: row.run_id,
            status: row.status,
        })
        .collect::<Vec<_>>()
        .into();

    Ok(ProofRelations {
        terminal,
        candidate_pins: candidate.candidate_pins,
        oracle_observations,
        proof_run,
        oracle_results,
        capability_results,
        expectations,
        coverage_results,
        fault_results,
        violation_results,
        provenance_edges,
        issues,
    })
}

fn initial_candidate_issues(
    oracle_requests: &[OracleRequest],
    expectations: &[SemanticExpectation],
) -> Vec<DerivedIssue> {
    let mut issues = Vec::new();
    if oracle_requests.is_empty() {
        issues.push(DerivedIssue {
            oracle_id: None,
            capability_id: None,
            code: ProofIssueCode::NoOracleRequests,
            disposition: IssueDisposition::Unknown,
        });
    }
    if expectations.is_empty() {
        issues.push(DerivedIssue {
            oracle_id: None,
            capability_id: None,
            code: ProofIssueCode::EmptyExpectationSet,
            disposition: IssueDisposition::Unknown,
        });
    }
    issues
}

fn validate_resource_bounds(
    candidate: &CandidateProofInput<'_>,
    independent: &IndependentProofInput<'_>,
) -> Result<(), ProofError> {
    let limits = [
        (
            candidate.oracle_requests.len(),
            MAX_ORACLES,
            "oracle requests",
        ),
        (
            candidate.capability_requests.len(),
            MAX_CAPABILITIES,
            "capability requests",
        ),
        (
            candidate.capability_requirements.len(),
            MAX_EXPECTATIONS,
            "capability requirements",
        ),
        (
            independent.expectations.len(),
            MAX_EXPECTATIONS,
            "expectations",
        ),
        (
            independent.required_faults.len(),
            MAX_FAULTS,
            "required faults",
        ),
        (
            candidate.oracle_executions.len(),
            MAX_ORACLES,
            "oracle executions",
        ),
        (candidate.violations.len(), MAX_VIOLATIONS, "violations"),
        (
            candidate.fault_executions.len(),
            MAX_FAULTS,
            "fault executions",
        ),
        (
            candidate.provenance_edges.len(),
            MAX_PROVENANCE_EDGES,
            "provenance edges",
        ),
    ];
    if let Some((_, _, name)) = limits.into_iter().find(|(actual, limit, _)| actual > limit) {
        return Err(ProofError::ResourceLimit(name));
    }
    if candidate
        .oracle_requests
        .iter()
        .any(|request| request.requested_scopes.len() > MAX_SCOPES_PER_ORACLE)
    {
        return Err(ProofError::ResourceLimit("coverage scopes per oracle"));
    }
    let requested_scope_count = candidate
        .oracle_requests
        .iter()
        .try_fold(0_usize, |count, request| {
            count.checked_add(request.requested_scopes.len())
        })
        .ok_or(ProofError::ResourceLimit("total requested coverage scopes"))?;
    if requested_scope_count > MAX_TOTAL_COVERAGE_SCOPES {
        return Err(ProofError::ResourceLimit("total requested coverage scopes"));
    }
    let execution_scope_count = candidate
        .oracle_executions
        .iter()
        .try_fold(0_usize, |count, execution| {
            count
                .checked_add(execution.completed_scopes.len())
                .and_then(|count| count.checked_add(execution.unavailable_scopes.len()))
        })
        .ok_or(ProofError::ResourceLimit("total execution coverage rows"))?;
    if execution_scope_count > MAX_TOTAL_COVERAGE_SCOPES {
        return Err(ProofError::ResourceLimit("total execution coverage rows"));
    }
    let traversal_bound = candidate
        .oracle_requests
        .len()
        .checked_mul(candidate.provenance_edges.len())
        .ok_or(ProofError::ResourceLimit("provenance traversal bound"))?;
    if traversal_bound > MAX_PROVENANCE_TRAVERSAL_BOUND {
        return Err(ProofError::ResourceLimit("provenance traversal bound"));
    }
    Ok(())
}

fn validate_candidate_pins(pins: &ProofCandidatePins) -> Result<(), ProofError> {
    let nonzero = [
        pins.epoch.as_bytes().as_slice(),
        pins.input_release.as_bytes().as_slice(),
        pins.program_release.as_bytes().as_slice(),
        pins.application_release.as_bytes().as_slice(),
        pins.source_authority.as_bytes().as_slice(),
        pins.source_images.as_bytes().as_slice(),
        pins.provider_release.as_bytes().as_slice(),
        pins.provider_set.as_bytes().as_slice(),
        pins.table_versions.as_bytes().as_slice(),
        pins.overlay_segments.as_bytes().as_slice(),
        pins.policy_set.as_bytes().as_slice(),
        pins.resource_envelope.as_bytes().as_slice(),
    ]
    .into_iter()
    .all(|bytes| bytes.iter().any(|byte| *byte != 0));
    if nonzero {
        Ok(())
    } else {
        Err(ProofError::ZeroCandidatePin)
    }
}

fn index_oracles(rows: &[OracleRequest]) -> Result<BTreeMap<OracleId, &OracleRequest>, ProofError> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        if indexed.insert(row.oracle_id, row).is_some() {
            return Err(ProofError::DuplicateOracle);
        }
        if row.requested_scopes.iter().collect::<BTreeSet<_>>().len() != row.requested_scopes.len()
        {
            return Err(ProofError::InvalidCoverage);
        }
    }
    Ok(indexed)
}

fn index_capabilities(
    rows: &[CapabilityRequest],
) -> Result<BTreeMap<CapabilityId, &CapabilityRequest>, ProofError> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        if indexed.insert(row.capability_id, row).is_some() {
            return Err(ProofError::DuplicateCapability);
        }
    }
    Ok(indexed)
}

fn index_capability_requirements(
    rows: &[CapabilityOracleRequirement],
    capabilities: &BTreeMap<CapabilityId, &CapabilityRequest>,
    oracles: &BTreeMap<OracleId, &OracleRequest>,
) -> Result<BTreeMap<CapabilityId, Vec<OracleId>>, ProofError> {
    let mut unique = BTreeSet::new();
    let mut indexed = BTreeMap::<CapabilityId, Vec<OracleId>>::new();
    for row in rows {
        if !capabilities.contains_key(&row.capability_id) || !oracles.contains_key(&row.oracle_id) {
            return Err(ProofError::UnknownCapabilityRequirement);
        }
        if !unique.insert((row.capability_id, row.oracle_id)) {
            return Err(ProofError::DuplicateCapabilityRequirement);
        }
        indexed
            .entry(row.capability_id)
            .or_default()
            .push(row.oracle_id);
    }
    Ok(indexed)
}

fn index_expectations<'a>(
    rows: &'a [SemanticExpectation],
    oracles: &BTreeMap<OracleId, &OracleRequest>,
) -> Result<BTreeMap<ExpectationId, &'a SemanticExpectation>, ProofError> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        let Some(oracle) = oracles.get(&row.oracle_id) else {
            return Err(ProofError::UnknownExpectationReference);
        };
        if !oracle.requested_scopes.contains(&row.coverage_scope) {
            return Err(ProofError::UnknownExpectationReference);
        }
        if indexed.insert(row.expectation_id, row).is_some() {
            return Err(ProofError::DuplicateExpectation);
        }
    }
    Ok(indexed)
}

fn index_faults<'a>(
    rows: &'a [RequiredCausalFault],
    oracles: &BTreeMap<OracleId, &OracleRequest>,
) -> Result<BTreeMap<CausalFaultId, &'a RequiredCausalFault>, ProofError> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        let Some(oracle) = oracles.get(&row.oracle_id) else {
            return Err(ProofError::UnknownFaultReference);
        };
        if !oracle.requested_scopes.contains(&row.coverage_scope) {
            return Err(ProofError::UnknownFaultReference);
        }
        if indexed.insert(row.fault_id, row).is_some() {
            return Err(ProofError::DuplicateFault);
        }
    }
    Ok(indexed)
}

fn index_oracle_executions<'a>(
    rows: &'a [OracleExecution],
    oracles: &BTreeMap<OracleId, &OracleRequest>,
) -> Result<BTreeMap<OracleId, &'a OracleExecution>, ProofError> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        let Some(request) = oracles.get(&row.oracle_id) else {
            return Err(ProofError::UnknownOracleExecution);
        };
        if indexed.insert(row.oracle_id, row).is_some() {
            return Err(ProofError::DuplicateOracleExecution);
        }
        let completed = row
            .completed_scopes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let unavailable = row
            .unavailable_scopes
            .iter()
            .map(|coverage| coverage.scope)
            .collect::<BTreeSet<_>>();
        if completed.len() != row.completed_scopes.len()
            || unavailable.len() != row.unavailable_scopes.len()
            || !completed.is_disjoint(&unavailable)
            || completed
                .union(&unavailable)
                .any(|scope| !request.requested_scopes.contains(scope))
        {
            return Err(ProofError::InvalidCoverage);
        }
    }
    Ok(indexed)
}

fn index_violations<'a>(
    rows: &'a [ProofViolation],
    oracles: &BTreeMap<OracleId, &OracleRequest>,
    expectations: &BTreeMap<ExpectationId, &SemanticExpectation>,
    faults: &BTreeMap<CausalFaultId, &RequiredCausalFault>,
) -> Result<BTreeMap<ViolationId, &'a ProofViolation>, ProofError> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        if !oracles.contains_key(&row.oracle_id) {
            return Err(ProofError::UnknownViolationReference);
        }
        if let Some(expectation_id) = row.expectation_id
            && expectations
                .get(&expectation_id)
                .is_none_or(|expectation| expectation.oracle_id != row.oracle_id)
        {
            return Err(ProofError::UnknownViolationReference);
        }
        if let Some(fault_id) = row.fault_id
            && faults
                .get(&fault_id)
                .is_none_or(|fault| fault.oracle_id != row.oracle_id)
        {
            return Err(ProofError::UnknownViolationReference);
        }
        if indexed.insert(row.violation_id, row).is_some() {
            return Err(ProofError::DuplicateViolation);
        }
    }
    Ok(indexed)
}

fn index_fault_executions<'a>(
    rows: &'a [CausalFaultExecution],
    faults: &BTreeMap<CausalFaultId, &RequiredCausalFault>,
) -> Result<BTreeMap<CausalFaultId, &'a CausalFaultExecution>, ProofError> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        if !faults.contains_key(&row.fault_id) {
            return Err(ProofError::UnknownFaultExecution);
        }
        if indexed.insert(row.fault_id, row).is_some() {
            return Err(ProofError::DuplicateFaultExecution);
        }
    }
    Ok(indexed)
}

fn index_provenance(
    rows: &[ProofProvenanceEdge],
) -> Result<BTreeMap<ProvenanceSubject, Vec<ProvenanceSubject>>, ProofError> {
    let mut unique = BTreeSet::new();
    let mut adjacency = BTreeMap::<ProvenanceSubject, Vec<ProvenanceSubject>>::new();
    for row in rows {
        if !unique.insert((row.from, row.to)) {
            return Err(ProofError::DuplicateProvenanceEdge);
        }
        adjacency.entry(row.from).or_default().push(row.to);
    }
    Ok(adjacency)
}

fn group_expectations(
    rows: &[SemanticExpectation],
) -> BTreeMap<OracleId, Vec<&SemanticExpectation>> {
    let mut grouped = BTreeMap::<OracleId, Vec<&SemanticExpectation>>::new();
    for row in rows {
        grouped.entry(row.oracle_id).or_default().push(row);
    }
    grouped
}

fn group_faults(rows: &[RequiredCausalFault]) -> BTreeMap<OracleId, Vec<&RequiredCausalFault>> {
    let mut grouped = BTreeMap::<OracleId, Vec<&RequiredCausalFault>>::new();
    for row in rows {
        grouped.entry(row.oracle_id).or_default().push(row);
    }
    grouped
}

fn group_baseline_violations(rows: &[ProofViolation]) -> BTreeMap<OracleId, Vec<&ProofViolation>> {
    let mut grouped = BTreeMap::<OracleId, Vec<&ProofViolation>>::new();
    for row in rows.iter().filter(|row| row.fault_id.is_none()) {
        grouped.entry(row.oracle_id).or_default().push(row);
    }
    grouped
}

fn group_capabilities_by_oracle(
    rows: &[CapabilityOracleRequirement],
) -> BTreeMap<OracleId, Vec<CapabilityId>> {
    let mut grouped = BTreeMap::<OracleId, Vec<CapabilityId>>::new();
    for row in rows {
        grouped
            .entry(row.oracle_id)
            .or_default()
            .push(row.capability_id);
    }
    grouped
}

#[allow(clippy::too_many_arguments)]
fn evaluate_oracle(
    producer_owner: ProofOwnerId,
    candidate_pins: &ProofCandidatePins,
    request: &OracleRequest,
    expectations: &[&SemanticExpectation],
    faults: &[&RequiredCausalFault],
    baseline_violations: &[&ProofViolation],
    capabilities: &[CapabilityId],
    execution: Option<&OracleExecution>,
    violations: &BTreeMap<ViolationId, &ProofViolation>,
    fault_executions: &BTreeMap<CausalFaultId, &CausalFaultExecution>,
    adjacency: &BTreeMap<ProvenanceSubject, Vec<ProvenanceSubject>>,
    issues: &mut Vec<DerivedIssue>,
    coverage_rows: &mut Vec<CoverageResultRow>,
    fault_rows: &mut Vec<FaultResultRow>,
) -> OracleResultRow {
    let issue_start = issues.len();
    validate_independent_oracle_inputs(producer_owner, request, expectations, faults, issues);
    let coverage =
        evaluate_requested_coverage(candidate_pins, request, execution, issues, coverage_rows);
    for _ in baseline_violations {
        push_oracle_issue(
            issues,
            request.oracle_id,
            ProofIssueCode::BaselineViolation,
            IssueDisposition::Failure,
        );
    }
    let detected_fault_count = evaluate_required_faults(
        request,
        expectations,
        faults,
        violations,
        fault_executions,
        issues,
        fault_rows,
    );
    let (provenance_closed, missing_provenance_count) = evaluate_provenance_closure(
        candidate_pins,
        request,
        expectations,
        faults,
        capabilities,
        execution,
        adjacency,
    );
    if !provenance_closed {
        push_oracle_issue(
            issues,
            request.oracle_id,
            ProofIssueCode::MissingProvenance,
            IssueDisposition::Unknown,
        );
    }

    let status = terminal_from_issues(&issues[issue_start..]);
    OracleResultRow {
        oracle_id: request.oracle_id,
        implementation: request.implementation,
        violation_relation: request.violation_relation,
        run_id: execution.map(|execution| execution.run_id),
        status,
        requested_scope_count: bounded_count(request.requested_scopes.len()),
        completed_scope_count: bounded_count(coverage.completed),
        unavailable_scope_count: bounded_count(coverage.unavailable),
        uncovered_scope_count: bounded_count(coverage.uncovered),
        expectation_count: bounded_count(expectations.len()),
        baseline_violation_count: bounded_count(baseline_violations.len()),
        required_fault_count: bounded_count(faults.len()),
        detected_fault_count: bounded_count(detected_fault_count),
        provenance_closed,
        missing_provenance_count: bounded_count(missing_provenance_count),
    }
}

fn validate_independent_oracle_inputs(
    producer_owner: ProofOwnerId,
    request: &OracleRequest,
    expectations: &[&SemanticExpectation],
    faults: &[&RequiredCausalFault],
    issues: &mut Vec<DerivedIssue>,
) {
    if expectations.is_empty() {
        push_oracle_issue(
            issues,
            request.oracle_id,
            ProofIssueCode::OracleHasNoExpectation,
            IssueDisposition::Unknown,
        );
    }
    for expectation in expectations {
        if authority_overlaps_producer(expectation.authority, producer_owner) {
            push_oracle_issue(
                issues,
                request.oracle_id,
                ProofIssueCode::ProducerAuthoredExpectation,
                IssueDisposition::Failure,
            );
        }
        if !authority_roles_are_independent(expectation.authority) {
            push_oracle_issue(
                issues,
                request.oracle_id,
                ProofIssueCode::ExpectationReviewNotIndependent,
                IssueDisposition::Failure,
            );
        }
    }
    if faults.is_empty() {
        push_oracle_issue(
            issues,
            request.oracle_id,
            ProofIssueCode::OracleHasNoRequiredFault,
            IssueDisposition::Unknown,
        );
    }
    for fault in faults {
        if authority_overlaps_producer(fault.authority, producer_owner) {
            push_oracle_issue(
                issues,
                request.oracle_id,
                ProofIssueCode::ProducerAuthoredFault,
                IssueDisposition::Failure,
            );
        }
        if !authority_roles_are_independent(fault.authority) {
            push_oracle_issue(
                issues,
                request.oracle_id,
                ProofIssueCode::FaultReviewNotIndependent,
                IssueDisposition::Failure,
            );
        }
    }
    if request.requested_scopes.is_empty() {
        push_oracle_issue(
            issues,
            request.oracle_id,
            ProofIssueCode::OracleHasNoRequestedCoverage,
            IssueDisposition::Unknown,
        );
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CoverageCounts {
    completed: usize,
    unavailable: usize,
    uncovered: usize,
}

fn evaluate_requested_coverage(
    candidate_pins: &ProofCandidatePins,
    request: &OracleRequest,
    execution: Option<&OracleExecution>,
    issues: &mut Vec<DerivedIssue>,
    coverage_rows: &mut Vec<CoverageResultRow>,
) -> CoverageCounts {
    let mut counts = CoverageCounts::default();
    if let Some(execution) = execution {
        if execution.candidate_pins != *candidate_pins {
            push_oracle_issue(
                issues,
                request.oracle_id,
                ProofIssueCode::CandidatePinMismatch,
                IssueDisposition::Failure,
            );
        }
        for scope in &request.requested_scopes {
            if execution.completed_scopes.contains(scope) {
                counts.completed += 1;
                coverage_rows.push(CoverageResultRow {
                    oracle_id: request.oracle_id,
                    scope_id: *scope,
                    state: CoverageState::Completed,
                    unavailable_reason: None,
                });
            } else if let Some(unavailable) = execution
                .unavailable_scopes
                .iter()
                .find(|unavailable| unavailable.scope == *scope)
            {
                counts.unavailable += 1;
                coverage_rows.push(CoverageResultRow {
                    oracle_id: request.oracle_id,
                    scope_id: *scope,
                    state: CoverageState::Unavailable,
                    unavailable_reason: Some(unavailable.reason),
                });
                push_oracle_issue(
                    issues,
                    request.oracle_id,
                    ProofIssueCode::ExplicitUnavailableScope,
                    IssueDisposition::Unknown,
                );
            } else {
                counts.uncovered += 1;
                coverage_rows.push(CoverageResultRow {
                    oracle_id: request.oracle_id,
                    scope_id: *scope,
                    state: CoverageState::Uncovered,
                    unavailable_reason: None,
                });
                push_oracle_issue(
                    issues,
                    request.oracle_id,
                    ProofIssueCode::UncoveredRequestedScope,
                    IssueDisposition::Unknown,
                );
            }
        }
    } else {
        push_oracle_issue(
            issues,
            request.oracle_id,
            ProofIssueCode::MissingOracleExecution,
            IssueDisposition::Unknown,
        );
        counts.uncovered = request.requested_scopes.len();
        coverage_rows.extend(
            request
                .requested_scopes
                .iter()
                .map(|scope| CoverageResultRow {
                    oracle_id: request.oracle_id,
                    scope_id: *scope,
                    state: CoverageState::Uncovered,
                    unavailable_reason: None,
                }),
        );
    }
    counts
}

#[allow(clippy::too_many_arguments)]
fn evaluate_required_faults(
    request: &OracleRequest,
    expectations: &[&SemanticExpectation],
    faults: &[&RequiredCausalFault],
    violations: &BTreeMap<ViolationId, &ProofViolation>,
    fault_executions: &BTreeMap<CausalFaultId, &CausalFaultExecution>,
    issues: &mut Vec<DerivedIssue>,
    fault_rows: &mut Vec<FaultResultRow>,
) -> usize {
    let mut detected_fault_count = 0_usize;
    for fault in faults {
        let (state, violation_id, unavailable_reason) = match fault_executions.get(&fault.fault_id)
        {
            None => {
                push_oracle_issue(
                    issues,
                    request.oracle_id,
                    ProofIssueCode::MissingFaultExecution,
                    IssueDisposition::Unknown,
                );
                (FaultResultState::Missing, None, None)
            }
            Some(execution) => match execution.outcome {
                CausalFaultOutcome::Survived => {
                    push_oracle_issue(
                        issues,
                        request.oracle_id,
                        ProofIssueCode::RequiredFaultSurvived,
                        IssueDisposition::Failure,
                    );
                    (FaultResultState::Survived, None, None)
                }
                CausalFaultOutcome::Unavailable { reason } => {
                    push_oracle_issue(
                        issues,
                        request.oracle_id,
                        ProofIssueCode::FaultExecutionUnavailable,
                        IssueDisposition::Unknown,
                    );
                    (FaultResultState::Unavailable, None, Some(reason))
                }
                CausalFaultOutcome::Detected { violation_id } => {
                    let valid = violations.get(&violation_id).is_some_and(|violation| {
                        violation.oracle_id == request.oracle_id
                            && violation.fault_id == Some(fault.fault_id)
                            && causal_violation_matches(
                                fault.required_effect,
                                fault.coverage_scope,
                                violation,
                                expectations,
                            )
                    });
                    if valid {
                        detected_fault_count += 1;
                        (FaultResultState::Detected, Some(violation_id), None)
                    } else {
                        push_oracle_issue(
                            issues,
                            request.oracle_id,
                            ProofIssueCode::FaultEvidenceMismatch,
                            IssueDisposition::Failure,
                        );
                        (FaultResultState::EvidenceMismatch, Some(violation_id), None)
                    }
                }
            },
        };
        fault_rows.push(FaultResultRow {
            fault_id: fault.fault_id,
            oracle_id: request.oracle_id,
            coverage_scope: fault.coverage_scope,
            program: fault.program,
            required_effect: fault.required_effect,
            authority: fault.authority,
            state,
            violation_id,
            unavailable_reason,
        });
    }
    detected_fault_count
}

fn evaluate_provenance_closure(
    candidate_pins: &ProofCandidatePins,
    request: &OracleRequest,
    expectations: &[&SemanticExpectation],
    faults: &[&RequiredCausalFault],
    capabilities: &[CapabilityId],
    execution: Option<&OracleExecution>,
    adjacency: &BTreeMap<ProvenanceSubject, Vec<ProvenanceSubject>>,
) -> (bool, usize) {
    let required =
        required_provenance_subjects(candidate_pins, request, expectations, faults, capabilities);
    let Some(execution) = execution else {
        return (false, required.len());
    };
    let reachable = reachable_subjects(ProvenanceSubject::OracleRun(execution.run_id), adjacency);
    let missing = required
        .iter()
        .filter(|subject| !reachable.contains(subject))
        .count();
    (missing == 0, missing)
}

fn authority_overlaps_producer(
    authority: IndependentEvidenceAuthority,
    producer: ProofOwnerId,
) -> bool {
    authority.author == producer
        || authority.reviewer == producer
        || authority.acceptance_authority == producer
}

fn authority_roles_are_independent(authority: IndependentEvidenceAuthority) -> bool {
    authority.author != authority.reviewer
        && authority.author != authority.acceptance_authority
        && authority.reviewer != authority.acceptance_authority
}

fn causal_violation_matches(
    required: RequiredCausalEffect,
    fault_scope: CoverageScopeId,
    violation: &ProofViolation,
    expectations: &[&SemanticExpectation],
) -> bool {
    match required {
        RequiredCausalEffect::StructuralRejection => {
            violation.kind == ProofViolationKind::ConstructionRejected
        }
        RequiredCausalEffect::SemanticDiscrimination => {
            violation.expectation_id.is_some_and(|expectation_id| {
                expectations.iter().any(|expectation| {
                    expectation.expectation_id == expectation_id
                        && expectation.coverage_scope == fault_scope
                })
            }) && matches!(
                violation.kind,
                ProofViolationKind::SemanticMismatch
                    | ProofViolationKind::AuthorizationMismatch
                    | ProofViolationKind::UnknownSemanticsMismatch
            )
        }
    }
}

fn required_provenance_subjects(
    pins: &ProofCandidatePins,
    request: &OracleRequest,
    expectations: &[&SemanticExpectation],
    faults: &[&RequiredCausalFault],
    capabilities: &[CapabilityId],
) -> BTreeSet<ProvenanceSubject> {
    let mut required = BTreeSet::from([
        ProvenanceSubject::Epoch(pins.epoch),
        ProvenanceSubject::InputRelease(pins.input_release),
        ProvenanceSubject::ProgramRelease(pins.program_release),
        ProvenanceSubject::ApplicationRelease(pins.application_release),
        ProvenanceSubject::SourceAuthority(pins.source_authority),
        ProvenanceSubject::SourceGeneration(pins.source_generation),
        ProvenanceSubject::SourceImages(pins.source_images),
        ProvenanceSubject::ProviderRelease(pins.provider_release),
        ProvenanceSubject::ProviderSet(pins.provider_set),
        ProvenanceSubject::TableVersions(pins.table_versions),
        ProvenanceSubject::OverlaySegments(pins.overlay_segments),
        ProvenanceSubject::PolicySet(pins.policy_set),
        ProvenanceSubject::ResourceEnvelope(pins.resource_envelope),
        ProvenanceSubject::OracleImplementation(request.implementation),
        ProvenanceSubject::ViolationRelation(request.violation_relation),
    ]);
    for capability in capabilities {
        required.insert(ProvenanceSubject::Capability(*capability));
    }
    for expectation in expectations {
        required.insert(ProvenanceSubject::Expectation(expectation.expectation_id));
        required.insert(ProvenanceSubject::SemanticClaim(expectation.claim));
        required.insert(ProvenanceSubject::SourceAnchor(expectation.source_anchor));
    }
    for fault in faults {
        required.insert(ProvenanceSubject::CausalFault(fault.fault_id));
        required.insert(ProvenanceSubject::CausalFaultProgram(fault.program));
    }
    required
}

fn reachable_subjects(
    root: ProvenanceSubject,
    adjacency: &BTreeMap<ProvenanceSubject, Vec<ProvenanceSubject>>,
) -> BTreeSet<ProvenanceSubject> {
    let mut visited = BTreeSet::from([root]);
    let mut queue = VecDeque::from([root]);
    while let Some(subject) = queue.pop_front() {
        if let Some(children) = adjacency.get(&subject) {
            for child in children {
                if visited.insert(*child) {
                    queue.push_back(*child);
                }
            }
        }
    }
    visited
}

fn push_oracle_issue(
    issues: &mut Vec<DerivedIssue>,
    oracle_id: OracleId,
    code: ProofIssueCode,
    disposition: IssueDisposition,
) {
    issues.push(DerivedIssue {
        oracle_id: Some(oracle_id),
        capability_id: None,
        code,
        disposition,
    });
}

fn terminal_from_issues(issues: &[DerivedIssue]) -> ProofTerminalStatus {
    if issues
        .iter()
        .any(|issue| issue.disposition == IssueDisposition::Failure)
    {
        ProofTerminalStatus::Fail
    } else if issues
        .iter()
        .any(|issue| issue.disposition == IssueDisposition::Unknown)
    {
        ProofTerminalStatus::Unknown
    } else {
        ProofTerminalStatus::Pass
    }
}

fn evaluate_capabilities(
    capabilities: &BTreeMap<CapabilityId, &CapabilityRequest>,
    requirements: &BTreeMap<CapabilityId, Vec<OracleId>>,
    oracle_statuses: &BTreeMap<OracleId, ProofTerminalStatus>,
    issues: &mut Vec<DerivedIssue>,
) -> Vec<CapabilityResultRow> {
    let mut rows = Vec::with_capacity(capabilities.len());
    for capability_id in capabilities.keys().copied() {
        let required = requirements
            .get(&capability_id)
            .map_or(&[][..], Vec::as_slice);
        let pass_count = required
            .iter()
            .filter(|oracle| oracle_statuses.get(oracle) == Some(&ProofTerminalStatus::Pass))
            .count();
        let fail_count = required
            .iter()
            .filter(|oracle| oracle_statuses.get(oracle) == Some(&ProofTerminalStatus::Fail))
            .count();
        let unknown_count = required.len() - pass_count - fail_count;
        let status = if required.is_empty() {
            issues.push(DerivedIssue {
                oracle_id: None,
                capability_id: Some(capability_id),
                code: ProofIssueCode::CapabilityHasNoOracle,
                disposition: IssueDisposition::Unknown,
            });
            ProofCapabilityStatus::Unknown
        } else if fail_count > 0 {
            ProofCapabilityStatus::Unavailable
        } else if unknown_count > 0 {
            ProofCapabilityStatus::Unknown
        } else {
            ProofCapabilityStatus::Supported
        };
        rows.push(CapabilityResultRow {
            capability_id,
            status,
            required_oracle_count: bounded_count(required.len()),
            pass_count: bounded_count(pass_count),
            fail_count: bounded_count(fail_count),
            unknown_count: bounded_count(unknown_count),
        });
    }
    rows
}

fn derive_candidate_terminal(
    oracle_rows: &[OracleResultRow],
    capability_rows: &[CapabilityResultRow],
    issues: &[DerivedIssue],
) -> ProofTerminalStatus {
    if oracle_rows
        .iter()
        .any(|row| row.status == ProofTerminalStatus::Fail)
        || capability_rows
            .iter()
            .any(|row| row.status == ProofCapabilityStatus::Unavailable)
        || issues
            .iter()
            .any(|issue| issue.disposition == IssueDisposition::Failure)
    {
        ProofTerminalStatus::Fail
    } else if oracle_rows.is_empty()
        || oracle_rows
            .iter()
            .any(|row| row.status == ProofTerminalStatus::Unknown)
        || capability_rows
            .iter()
            .any(|row| row.status == ProofCapabilityStatus::Unknown)
        || issues
            .iter()
            .any(|issue| issue.disposition == IssueDisposition::Unknown)
    {
        ProofTerminalStatus::Unknown
    } else {
        ProofTerminalStatus::Pass
    }
}

fn bounded_count(value: usize) -> u64 {
    u64::try_from(value).expect("proof input is bounded below u64::MAX")
}

fn fixed_binary_array<const WIDTH: usize>(
    values: impl IntoIterator<Item = Option<[u8; WIDTH]>>,
) -> Result<ArrayRef, ProofError> {
    let mut builder = FixedSizeBinaryBuilder::new(i32::try_from(WIDTH).expect("small fixed width"));
    for value in values {
        if let Some(value) = value {
            builder.append_value(value)?;
        } else {
            builder.append_null();
        }
    }
    Ok(Arc::new(builder.finish()))
}

fn proof_run_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("epoch_id", DataType::FixedSizeBinary(16), false),
        Field::new("input_release", DataType::FixedSizeBinary(32), false),
        Field::new("program_release", DataType::FixedSizeBinary(32), false),
        Field::new("application_release", DataType::FixedSizeBinary(32), false),
        Field::new("source_authority", DataType::FixedSizeBinary(32), false),
        Field::new("source_generation", DataType::UInt64, false),
        Field::new("source_images", DataType::FixedSizeBinary(32), false),
        Field::new("provider_release", DataType::FixedSizeBinary(32), false),
        Field::new("provider_set", DataType::FixedSizeBinary(32), false),
        Field::new("table_versions", DataType::FixedSizeBinary(32), false),
        Field::new("overlay_segments", DataType::FixedSizeBinary(32), false),
        Field::new("policy_set", DataType::FixedSizeBinary(32), false),
        Field::new("resource_envelope", DataType::FixedSizeBinary(32), false),
        Field::new("producer_owner", DataType::FixedSizeBinary(32), false),
        Field::new("terminal_status", DataType::Int8, false),
        Field::new("oracle_count", DataType::UInt64, false),
        Field::new("pass_count", DataType::UInt64, false),
        Field::new("fail_count", DataType::UInt64, false),
        Field::new("unknown_count", DataType::UInt64, false),
        Field::new("expectation_count", DataType::UInt64, false),
        Field::new("required_fault_count", DataType::UInt64, false),
        Field::new("detected_fault_count", DataType::UInt64, false),
        Field::new("unavailable_scope_count", DataType::UInt64, false),
        Field::new("uncovered_scope_count", DataType::UInt64, false),
        Field::new("missing_provenance_count", DataType::UInt64, false),
        Field::new("capability_count", DataType::UInt64, false),
    ]))
}

fn build_proof_run_relation(
    candidate: &CandidateProofInput<'_>,
    independent: &IndependentProofInput<'_>,
    terminal: ProofTerminalStatus,
    oracle_rows: &[OracleResultRow],
    capability_rows: &[CapabilityResultRow],
) -> Result<ProofRelationOutput, ProofError> {
    let pins = candidate.candidate_pins;
    let pass_count = oracle_rows
        .iter()
        .filter(|row| row.status == ProofTerminalStatus::Pass)
        .count();
    let fail_count = oracle_rows
        .iter()
        .filter(|row| row.status == ProofTerminalStatus::Fail)
        .count();
    let unknown_count = oracle_rows.len() - pass_count - fail_count;
    let required_fault_count = oracle_rows
        .iter()
        .map(|row| row.required_fault_count)
        .sum::<u64>();
    let detected_fault_count = oracle_rows
        .iter()
        .map(|row| row.detected_fault_count)
        .sum::<u64>();
    let unavailable_scope_count = oracle_rows
        .iter()
        .map(|row| row.unavailable_scope_count)
        .sum::<u64>();
    let uncovered_scope_count = oracle_rows
        .iter()
        .map(|row| row.uncovered_scope_count)
        .sum::<u64>();
    let missing_provenance_count = oracle_rows
        .iter()
        .map(|row| row.missing_provenance_count)
        .sum::<u64>();
    let schema = proof_run_schema();
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            fixed_binary_array([Some(*pins.epoch.as_bytes())])?,
            fixed_binary_array([Some(*pins.input_release.as_bytes())])?,
            fixed_binary_array([Some(*pins.program_release.as_bytes())])?,
            fixed_binary_array([Some(*pins.application_release.as_bytes())])?,
            fixed_binary_array([Some(*pins.source_authority.as_bytes())])?,
            Arc::new(UInt64Array::from(vec![pins.source_generation.get()])),
            fixed_binary_array([Some(*pins.source_images.as_bytes())])?,
            fixed_binary_array([Some(*pins.provider_release.as_bytes())])?,
            fixed_binary_array([Some(*pins.provider_set.as_bytes())])?,
            fixed_binary_array([Some(*pins.table_versions.as_bytes())])?,
            fixed_binary_array([Some(*pins.overlay_segments.as_bytes())])?,
            fixed_binary_array([Some(*pins.policy_set.as_bytes())])?,
            fixed_binary_array([Some(*pins.resource_envelope.as_bytes())])?,
            fixed_binary_array([Some(*candidate.producer_owner.as_bytes())])?,
            Arc::new(Int8Array::from(vec![terminal.code()])),
            Arc::new(UInt64Array::from(vec![bounded_count(oracle_rows.len())])),
            Arc::new(UInt64Array::from(vec![bounded_count(pass_count)])),
            Arc::new(UInt64Array::from(vec![bounded_count(fail_count)])),
            Arc::new(UInt64Array::from(vec![bounded_count(unknown_count)])),
            Arc::new(UInt64Array::from(vec![bounded_count(
                independent.expectations.len(),
            )])),
            Arc::new(UInt64Array::from(vec![required_fault_count])),
            Arc::new(UInt64Array::from(vec![detected_fault_count])),
            Arc::new(UInt64Array::from(vec![unavailable_scope_count])),
            Arc::new(UInt64Array::from(vec![uncovered_scope_count])),
            Arc::new(UInt64Array::from(vec![missing_provenance_count])),
            Arc::new(UInt64Array::from(vec![bounded_count(
                capability_rows.len(),
            )])),
        ],
    )?;
    ProofRelationOutput::try_new(schema, batch)
}

fn oracle_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("epoch_id", DataType::FixedSizeBinary(16), false),
        Field::new("oracle_id", DataType::FixedSizeBinary(16), false),
        Field::new(
            "oracle_implementation",
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new("violation_relation", DataType::FixedSizeBinary(16), false),
        Field::new("run_id", DataType::FixedSizeBinary(16), true),
        Field::new("status", DataType::Int8, false),
        Field::new("requested_scope_count", DataType::UInt64, false),
        Field::new("completed_scope_count", DataType::UInt64, false),
        Field::new("unavailable_scope_count", DataType::UInt64, false),
        Field::new("uncovered_scope_count", DataType::UInt64, false),
        Field::new("expectation_count", DataType::UInt64, false),
        Field::new("baseline_violation_count", DataType::UInt64, false),
        Field::new("required_fault_count", DataType::UInt64, false),
        Field::new("detected_fault_count", DataType::UInt64, false),
        Field::new("provenance_closed", DataType::Boolean, false),
        Field::new("missing_provenance_count", DataType::UInt64, false),
    ]))
}

fn build_oracle_relation(
    epoch: EpochId,
    rows: &[OracleResultRow],
) -> Result<ProofRelationOutput, ProofError> {
    let schema = oracle_schema();
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            fixed_binary_array(rows.iter().map(|_| Some(*epoch.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.oracle_id.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.implementation.as_bytes())))?,
            fixed_binary_array(
                rows.iter()
                    .map(|row| Some(*row.violation_relation.as_bytes())),
            )?,
            fixed_binary_array(rows.iter().map(|row| row.run_id.map(|id| *id.as_bytes())))?,
            Arc::new(Int8Array::from(
                rows.iter().map(|row| row.status.code()).collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.requested_scope_count)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.completed_scope_count)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.unavailable_scope_count)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.uncovered_scope_count)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.expectation_count)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.baseline_violation_count)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.required_fault_count)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.detected_fault_count)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(BooleanArray::from(
                rows.iter()
                    .map(|row| row.provenance_closed)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.missing_provenance_count)
                    .collect::<Vec<_>>(),
            )),
        ],
    )?;
    ProofRelationOutput::try_new(schema, batch)
}

fn capability_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("epoch_id", DataType::FixedSizeBinary(16), false),
        Field::new("capability_id", DataType::FixedSizeBinary(16), false),
        Field::new("status", DataType::Int8, false),
        Field::new("required_oracle_count", DataType::UInt64, false),
        Field::new("pass_count", DataType::UInt64, false),
        Field::new("fail_count", DataType::UInt64, false),
        Field::new("unknown_count", DataType::UInt64, false),
    ]))
}

fn expectation_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("epoch_id", DataType::FixedSizeBinary(16), false),
        Field::new("expectation_id", DataType::FixedSizeBinary(16), false),
        Field::new("oracle_id", DataType::FixedSizeBinary(16), false),
        Field::new("coverage_scope", DataType::FixedSizeBinary(16), false),
        Field::new("semantic_claim", DataType::FixedSizeBinary(32), false),
        Field::new("source_anchor", DataType::FixedSizeBinary(32), false),
        Field::new("author_owner", DataType::FixedSizeBinary(32), false),
        Field::new("reviewer_owner", DataType::FixedSizeBinary(32), false),
        Field::new(
            "acceptance_authority_owner",
            DataType::FixedSizeBinary(32),
            false,
        ),
    ]))
}

fn build_expectation_relation(
    epoch: EpochId,
    rows: &[SemanticExpectation],
) -> Result<ProofRelationOutput, ProofError> {
    let schema = expectation_schema();
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            fixed_binary_array(rows.iter().map(|_| Some(*epoch.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.expectation_id.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.oracle_id.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.coverage_scope.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.claim.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.source_anchor.as_bytes())))?,
            fixed_binary_array(
                rows.iter()
                    .map(|row| Some(*row.authority.author.as_bytes())),
            )?,
            fixed_binary_array(
                rows.iter()
                    .map(|row| Some(*row.authority.reviewer.as_bytes())),
            )?,
            fixed_binary_array(
                rows.iter()
                    .map(|row| Some(*row.authority.acceptance_authority.as_bytes())),
            )?,
        ],
    )?;
    ProofRelationOutput::try_new(schema, batch)
}

fn build_capability_relation(
    epoch: EpochId,
    rows: &[CapabilityResultRow],
) -> Result<ProofRelationOutput, ProofError> {
    let schema = capability_schema();
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            fixed_binary_array(rows.iter().map(|_| Some(*epoch.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.capability_id.as_bytes())))?,
            Arc::new(Int8Array::from(
                rows.iter().map(|row| row.status.code()).collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter()
                    .map(|row| row.required_oracle_count)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter().map(|row| row.pass_count).collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter().map(|row| row.fail_count).collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter().map(|row| row.unknown_count).collect::<Vec<_>>(),
            )),
        ],
    )?;
    ProofRelationOutput::try_new(schema, batch)
}

fn coverage_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("epoch_id", DataType::FixedSizeBinary(16), false),
        Field::new("oracle_id", DataType::FixedSizeBinary(16), false),
        Field::new("scope_id", DataType::FixedSizeBinary(16), false),
        Field::new("coverage_state", DataType::Int8, false),
        Field::new("unavailable_reason", DataType::Int8, true),
    ]))
}

fn build_coverage_relation(
    epoch: EpochId,
    rows: &[CoverageResultRow],
) -> Result<ProofRelationOutput, ProofError> {
    let schema = coverage_schema();
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            fixed_binary_array(rows.iter().map(|_| Some(*epoch.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.oracle_id.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.scope_id.as_bytes())))?,
            Arc::new(Int8Array::from(
                rows.iter().map(|row| row.state.code()).collect::<Vec<_>>(),
            )),
            Arc::new(Int8Array::from(
                rows.iter()
                    .map(|row| row.unavailable_reason.map(CoverageUnavailableReason::code))
                    .collect::<Vec<_>>(),
            )),
        ],
    )?;
    ProofRelationOutput::try_new(schema, batch)
}

fn fault_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("epoch_id", DataType::FixedSizeBinary(16), false),
        Field::new("fault_id", DataType::FixedSizeBinary(16), false),
        Field::new("oracle_id", DataType::FixedSizeBinary(16), false),
        Field::new("coverage_scope", DataType::FixedSizeBinary(16), false),
        Field::new("fault_program", DataType::FixedSizeBinary(32), false),
        Field::new("required_effect", DataType::Int8, false),
        Field::new("author_owner", DataType::FixedSizeBinary(32), false),
        Field::new("reviewer_owner", DataType::FixedSizeBinary(32), false),
        Field::new(
            "acceptance_authority_owner",
            DataType::FixedSizeBinary(32),
            false,
        ),
        Field::new("result_state", DataType::Int8, false),
        Field::new("violation_id", DataType::FixedSizeBinary(16), true),
        Field::new("unavailable_reason", DataType::Int8, true),
    ]))
}

fn build_fault_relation(
    epoch: EpochId,
    rows: &[FaultResultRow],
) -> Result<ProofRelationOutput, ProofError> {
    let schema = fault_schema();
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            fixed_binary_array(rows.iter().map(|_| Some(*epoch.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.fault_id.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.oracle_id.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.coverage_scope.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.program.as_bytes())))?,
            Arc::new(Int8Array::from(
                rows.iter()
                    .map(|row| row.required_effect.code())
                    .collect::<Vec<_>>(),
            )),
            fixed_binary_array(
                rows.iter()
                    .map(|row| Some(*row.authority.author.as_bytes())),
            )?,
            fixed_binary_array(
                rows.iter()
                    .map(|row| Some(*row.authority.reviewer.as_bytes())),
            )?,
            fixed_binary_array(
                rows.iter()
                    .map(|row| Some(*row.authority.acceptance_authority.as_bytes())),
            )?,
            Arc::new(Int8Array::from(
                rows.iter().map(|row| row.state.code()).collect::<Vec<_>>(),
            )),
            fixed_binary_array(
                rows.iter()
                    .map(|row| row.violation_id.map(|id| *id.as_bytes())),
            )?,
            Arc::new(Int8Array::from(
                rows.iter()
                    .map(|row| row.unavailable_reason.map(CoverageUnavailableReason::code))
                    .collect::<Vec<_>>(),
            )),
        ],
    )?;
    ProofRelationOutput::try_new(schema, batch)
}

fn violation_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("epoch_id", DataType::FixedSizeBinary(16), false),
        Field::new("violation_id", DataType::FixedSizeBinary(16), false),
        Field::new("oracle_id", DataType::FixedSizeBinary(16), false),
        Field::new("expectation_id", DataType::FixedSizeBinary(16), true),
        Field::new("fault_id", DataType::FixedSizeBinary(16), true),
        Field::new("violation_kind", DataType::Int8, false),
    ]))
}

fn build_violation_relation(
    epoch: EpochId,
    rows: &[ProofViolation],
) -> Result<ProofRelationOutput, ProofError> {
    let schema = violation_schema();
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            fixed_binary_array(rows.iter().map(|_| Some(*epoch.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.violation_id.as_bytes())))?,
            fixed_binary_array(rows.iter().map(|row| Some(*row.oracle_id.as_bytes())))?,
            fixed_binary_array(
                rows.iter()
                    .map(|row| row.expectation_id.map(|id| *id.as_bytes())),
            )?,
            fixed_binary_array(rows.iter().map(|row| row.fault_id.map(|id| *id.as_bytes())))?,
            Arc::new(Int8Array::from(
                rows.iter().map(|row| row.kind.code()).collect::<Vec<_>>(),
            )),
        ],
    )?;
    ProofRelationOutput::try_new(schema, batch)
}

fn provenance_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("epoch_id", DataType::FixedSizeBinary(16), false),
        Field::new("from_kind", DataType::Int8, false),
        Field::new("from_id", DataType::FixedSizeBinary(32), false),
        Field::new("to_kind", DataType::Int8, false),
        Field::new("to_id", DataType::FixedSizeBinary(32), false),
    ]))
}

fn build_provenance_relation(
    epoch: EpochId,
    rows: &[ProofProvenanceEdge],
) -> Result<ProofRelationOutput, ProofError> {
    let schema = provenance_schema();
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            fixed_binary_array(rows.iter().map(|_| Some(*epoch.as_bytes())))?,
            Arc::new(Int8Array::from(
                rows.iter()
                    .map(|row| row.from.kind_code())
                    .collect::<Vec<_>>(),
            )),
            fixed_binary_array(rows.iter().map(|row| Some(row.from.encoded_id())))?,
            Arc::new(Int8Array::from(
                rows.iter()
                    .map(|row| row.to.kind_code())
                    .collect::<Vec<_>>(),
            )),
            fixed_binary_array(rows.iter().map(|row| Some(row.to.encoded_id())))?,
        ],
    )?;
    ProofRelationOutput::try_new(schema, batch)
}

fn issue_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("epoch_id", DataType::FixedSizeBinary(16), false),
        Field::new("oracle_id", DataType::FixedSizeBinary(16), true),
        Field::new("capability_id", DataType::FixedSizeBinary(16), true),
        Field::new("issue_code", DataType::Int16, false),
        Field::new("disposition", DataType::Int8, false),
    ]))
}

fn build_issue_relation(
    epoch: EpochId,
    rows: &[DerivedIssue],
) -> Result<ProofRelationOutput, ProofError> {
    let schema = issue_schema();
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            fixed_binary_array(rows.iter().map(|_| Some(*epoch.as_bytes())))?,
            fixed_binary_array(
                rows.iter()
                    .map(|row| row.oracle_id.map(|id| *id.as_bytes())),
            )?,
            fixed_binary_array(
                rows.iter()
                    .map(|row| row.capability_id.map(|id| *id.as_bytes())),
            )?,
            Arc::new(Int16Array::from(
                rows.iter().map(|row| row.code.code()).collect::<Vec<_>>(),
            )),
            Arc::new(Int8Array::from(
                rows.iter()
                    .map(|row| row.disposition.code())
                    .collect::<Vec<_>>(),
            )),
        ],
    )?;
    ProofRelationOutput::try_new(schema, batch)
}

#[cfg(test)]
pub(super) fn test_relations_for_epoch(epoch: EpochId) -> ProofRelations {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "epoch_id",
        DataType::FixedSizeBinary(16),
        false,
    )]));
    let mut values = FixedSizeBinaryBuilder::new(16);
    values
        .append_value(epoch.as_bytes())
        .expect("test epoch has the exact Arrow width");
    let batch = RecordBatch::try_new(Arc::clone(&schema), vec![Arc::new(values.finish())])
        .expect("test proof batch matches its schema");
    let output = ProofRelationOutput::try_new(schema, batch).expect("test proof output is exact");
    ProofRelations {
        terminal: ProofTerminalStatus::Unknown,
        candidate_pins: ProofCandidatePins {
            epoch,
            input_release: InputReleaseRef::from_bytes([1; 32]),
            program_release: ProgramReleaseRef::from_bytes([2; 32]),
            application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes([2; 32]),
            source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes([2; 32]),
            provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes([2; 32]),
            source_generation: SourceGeneration::new(1),
            source_images: SourceImageSetRef::from_bytes([3; 32]),
            provider_set: ProviderSetRef::from_bytes([4; 32]),
            table_versions: TableVersionSetRef::from_bytes([5; 32]),
            overlay_segments: OverlaySegmentSetRef::from_bytes([6; 32]),
            policy_set: PolicySetRef::from_bytes([7; 32]),
            resource_envelope: ResourceEnvelopeRef::from_bytes([8; 32]),
        },
        oracle_observations: Arc::from([]),
        proof_run: output.clone(),
        oracle_results: output.clone(),
        capability_results: output.clone(),
        expectations: output.clone(),
        coverage_results: output.clone(),
        fault_results: output.clone(),
        violation_results: output.clone(),
        provenance_edges: output.clone(),
        issues: output,
    }
}

#[cfg(test)]
pub(crate) fn test_relations_with_oracle(
    epoch: EpochId,
    oracle_id: OracleId,
    implementation: OracleImplementationRef,
    run_id: Option<ProofRunId>,
    status: ProofTerminalStatus,
) -> ProofRelations {
    let mut relations = test_relations_for_epoch(epoch);
    relations.terminal = status;
    relations.oracle_observations = Arc::from([ProofOracleObservation {
        epoch,
        oracle_id,
        implementation,
        run_id,
        status,
    }]);
    relations
}

#[cfg(test)]
mod tests {
    use arrow_array::{Array as _, BooleanArray, Int8Array, UInt64Array};

    use super::*;

    fn id16(marker: u8) -> [u8; 16] {
        [marker; 16]
    }

    fn id32(marker: u8) -> [u8; 32] {
        [marker; 32]
    }

    fn oracle_id() -> OracleId {
        OracleId::new(id16(1)).unwrap()
    }

    fn capability_id() -> CapabilityId {
        CapabilityId::new(id16(2)).unwrap()
    }

    fn expectation_id() -> ExpectationId {
        ExpectationId::new(id16(3)).unwrap()
    }

    fn fault_id() -> CausalFaultId {
        CausalFaultId::new(id16(4)).unwrap()
    }

    fn violation_id() -> ViolationId {
        ViolationId::new(id16(5)).unwrap()
    }

    fn run_id() -> ProofRunId {
        ProofRunId::new(id16(6)).unwrap()
    }

    fn scope_id() -> CoverageScopeId {
        CoverageScopeId::new(id16(7)).unwrap()
    }

    fn producer_owner() -> ProofOwnerId {
        ProofOwnerId::new(id32(20)).unwrap()
    }

    fn independent_authority() -> IndependentEvidenceAuthority {
        IndependentEvidenceAuthority {
            author: ProofOwnerId::new(id32(21)).unwrap(),
            reviewer: ProofOwnerId::new(id32(22)).unwrap(),
            acceptance_authority: ProofOwnerId::new(id32(23)).unwrap(),
        }
    }

    fn pins() -> ProofCandidatePins {
        ProofCandidatePins {
            epoch: EpochId::from_bytes(id16(30)),
            input_release: InputReleaseRef::from_bytes(id32(31)),
            program_release: ProgramReleaseRef::from_bytes(id32(32)),
            application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(id32(
                32,
            )),
            source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(id32(32)),
            provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(id32(32)),
            source_generation: SourceGeneration::new(33),
            source_images: SourceImageSetRef::from_bytes(id32(34)),
            provider_set: ProviderSetRef::from_bytes(id32(35)),
            table_versions: TableVersionSetRef::from_bytes(id32(36)),
            overlay_segments: OverlaySegmentSetRef::from_bytes(id32(37)),
            policy_set: PolicySetRef::from_bytes(id32(38)),
            resource_envelope: ResourceEnvelopeRef::from_bytes(id32(39)),
        }
    }

    #[derive(Clone)]
    struct Fixture {
        producer_owner: ProofOwnerId,
        pins: ProofCandidatePins,
        oracles: Vec<OracleRequest>,
        capabilities: Vec<CapabilityRequest>,
        capability_requirements: Vec<CapabilityOracleRequirement>,
        expectations: Vec<SemanticExpectation>,
        faults: Vec<RequiredCausalFault>,
        executions: Vec<OracleExecution>,
        violations: Vec<ProofViolation>,
        fault_executions: Vec<CausalFaultExecution>,
        provenance: Vec<ProofProvenanceEdge>,
    }

    impl Fixture {
        fn complete() -> Self {
            let pins = pins();
            let oracle = OracleRequest {
                oracle_id: oracle_id(),
                implementation: OracleImplementationRef::new(id32(40)).unwrap(),
                violation_relation: ProofRelationId::new(id16(41)).unwrap(),
                requested_scopes: vec![scope_id()],
            };
            let expectation = SemanticExpectation {
                expectation_id: expectation_id(),
                oracle_id: oracle_id(),
                coverage_scope: scope_id(),
                claim: SemanticClaimRef::new(id32(42)).unwrap(),
                source_anchor: SourceAnchorRef::new(id32(43)).unwrap(),
                authority: independent_authority(),
            };
            let fault = RequiredCausalFault {
                fault_id: fault_id(),
                oracle_id: oracle_id(),
                coverage_scope: scope_id(),
                program: CausalFaultProgramRef::new(id32(44)).unwrap(),
                required_effect: RequiredCausalEffect::SemanticDiscrimination,
                authority: independent_authority(),
            };
            let execution = OracleExecution {
                oracle_id: oracle_id(),
                run_id: run_id(),
                candidate_pins: pins,
                completed_scopes: vec![scope_id()],
                unavailable_scopes: vec![],
            };
            let fault_violation = ProofViolation {
                violation_id: violation_id(),
                oracle_id: oracle_id(),
                expectation_id: Some(expectation_id()),
                fault_id: Some(fault_id()),
                kind: ProofViolationKind::SemanticMismatch,
            };
            let root = ProvenanceSubject::OracleRun(run_id());
            let provenance_subjects = [
                ProvenanceSubject::Epoch(pins.epoch),
                ProvenanceSubject::InputRelease(pins.input_release),
                ProvenanceSubject::ProgramRelease(pins.program_release),
                ProvenanceSubject::ApplicationRelease(pins.application_release),
                ProvenanceSubject::SourceAuthority(pins.source_authority),
                ProvenanceSubject::SourceGeneration(pins.source_generation),
                ProvenanceSubject::SourceImages(pins.source_images),
                ProvenanceSubject::ProviderRelease(pins.provider_release),
                ProvenanceSubject::ProviderSet(pins.provider_set),
                ProvenanceSubject::TableVersions(pins.table_versions),
                ProvenanceSubject::OverlaySegments(pins.overlay_segments),
                ProvenanceSubject::PolicySet(pins.policy_set),
                ProvenanceSubject::ResourceEnvelope(pins.resource_envelope),
                ProvenanceSubject::OracleImplementation(oracle.implementation),
                ProvenanceSubject::ViolationRelation(oracle.violation_relation),
                ProvenanceSubject::Capability(capability_id()),
                ProvenanceSubject::Expectation(expectation.expectation_id),
                ProvenanceSubject::SemanticClaim(expectation.claim),
                ProvenanceSubject::SourceAnchor(expectation.source_anchor),
                ProvenanceSubject::CausalFault(fault.fault_id),
                ProvenanceSubject::CausalFaultProgram(fault.program),
            ];
            Self {
                producer_owner: producer_owner(),
                pins,
                oracles: vec![oracle],
                capabilities: vec![CapabilityRequest {
                    capability_id: capability_id(),
                }],
                capability_requirements: vec![CapabilityOracleRequirement {
                    capability_id: capability_id(),
                    oracle_id: oracle_id(),
                }],
                expectations: vec![expectation],
                faults: vec![fault],
                executions: vec![execution],
                violations: vec![fault_violation],
                fault_executions: vec![CausalFaultExecution {
                    fault_id: fault_id(),
                    outcome: CausalFaultOutcome::Detected {
                        violation_id: violation_id(),
                    },
                }],
                provenance: provenance_subjects
                    .into_iter()
                    .map(|subject| ProofProvenanceEdge {
                        from: root,
                        to: subject,
                    })
                    .collect(),
            }
        }

        fn candidate_input(&self) -> CandidateProofInput<'_> {
            CandidateProofInput {
                producer_owner: self.producer_owner,
                candidate_pins: self.pins,
                oracle_requests: &self.oracles,
                capability_requests: &self.capabilities,
                capability_requirements: &self.capability_requirements,
                oracle_executions: &self.executions,
                violations: &self.violations,
                fault_executions: &self.fault_executions,
                provenance_edges: &self.provenance,
            }
        }

        fn independent_input(&self) -> IndependentProofInput<'_> {
            IndependentProofInput {
                expectations: &self.expectations,
                required_faults: &self.faults,
            }
        }

        fn evaluate(&self) -> Result<ProofRelations, ProofError> {
            evaluate_candidate_proof(&self.candidate_input(), &self.independent_input())
        }
    }

    fn int8_value(batch: &RecordBatch, name: &str, row: usize) -> i8 {
        batch
            .column_by_name(name)
            .unwrap()
            .as_any()
            .downcast_ref::<Int8Array>()
            .unwrap()
            .value(row)
    }

    fn u64_value(batch: &RecordBatch, name: &str, row: usize) -> u64 {
        batch
            .column_by_name(name)
            .unwrap()
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap()
            .value(row)
    }

    #[test]
    fn complete_independent_proof_passes_and_emits_queryable_arrow() {
        let fixture = Fixture::complete();
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Pass);
        assert_eq!(result.candidate_pins(), pins());
        assert_eq!(result.oracle_observations().len(), 1);
        let oracle = result.oracle_observations()[0];
        assert_eq!(oracle.epoch(), pins().epoch);
        assert_eq!(oracle.oracle_id(), oracle_id());
        assert_eq!(oracle.run_id(), Some(run_id()));
        assert_eq!(oracle.status(), ProofTerminalStatus::Pass);
        assert_eq!(
            result.proof_run.batch.schema_ref(),
            &result.proof_run.schema
        );
        assert_eq!(result.proof_run.batch.num_rows(), 1);
        assert_eq!(
            int8_value(&result.proof_run.batch, "terminal_status", 0),
            ProofTerminalStatus::Pass.code()
        );
        assert_eq!(result.oracle_results.batch.num_rows(), 1);
        assert_eq!(
            int8_value(&result.oracle_results.batch, "status", 0),
            ProofTerminalStatus::Pass.code()
        );
        assert!(
            result
                .oracle_results
                .batch
                .column_by_name("provenance_closed")
                .unwrap()
                .as_any()
                .downcast_ref::<BooleanArray>()
                .unwrap()
                .value(0)
        );
        assert_eq!(
            int8_value(&result.capability_results.batch, "status", 0),
            ProofCapabilityStatus::Supported.code()
        );
        assert_eq!(result.expectations.batch.num_rows(), 1);
        assert_eq!(result.coverage_results.batch.num_rows(), 1);
        assert_eq!(result.fault_results.batch.num_rows(), 1);
        assert_eq!(result.violation_results.batch.num_rows(), 1);
        assert_eq!(result.provenance_edges.batch.num_rows(), 21);
        assert_eq!(result.issues.batch.num_rows(), 0);
    }

    #[test]
    fn empty_expectations_cannot_authorize_candidate() {
        let mut fixture = Fixture::complete();
        fixture.expectations.clear();
        fixture.faults.clear();
        fixture.violations.clear();
        fixture.fault_executions.clear();
        fixture.provenance.retain(|edge| {
            !matches!(
                edge.to,
                ProvenanceSubject::Expectation(_)
                    | ProvenanceSubject::SemanticClaim(_)
                    | ProvenanceSubject::SourceAnchor(_)
                    | ProvenanceSubject::CausalFault(_)
                    | ProvenanceSubject::CausalFaultProgram(_)
            )
        });
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Unknown);
        assert_eq!(
            int8_value(&result.oracle_results.batch, "status", 0),
            ProofTerminalStatus::Unknown.code()
        );
    }

    #[test]
    fn uncovered_requested_scope_is_unknown_not_pass() {
        let mut fixture = Fixture::complete();
        fixture.executions[0].completed_scopes.clear();
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Unknown);
        assert_eq!(
            u64_value(&result.proof_run.batch, "uncovered_scope_count", 0),
            1
        );
        assert_eq!(
            int8_value(&result.coverage_results.batch, "coverage_state", 0),
            CoverageState::Uncovered.code()
        );
    }

    #[test]
    fn producer_authored_expectation_is_a_terminal_failure() {
        let mut fixture = Fixture::complete();
        fixture.expectations[0].authority.author = fixture.producer_owner;
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Fail);
        assert_eq!(
            int8_value(&result.oracle_results.batch, "status", 0),
            ProofTerminalStatus::Fail.code()
        );
    }

    #[test]
    fn missing_provenance_is_explicit_unknown() {
        let mut fixture = Fixture::complete();
        fixture.provenance.retain(|edge| {
            edge.to != ProvenanceSubject::ProgramRelease(fixture.pins.program_release)
        });
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Unknown);
        assert_eq!(
            u64_value(&result.oracle_results.batch, "missing_provenance_count", 0),
            1
        );
    }

    #[test]
    fn surviving_required_mutant_is_a_terminal_failure() {
        let mut fixture = Fixture::complete();
        fixture.fault_executions[0].outcome = CausalFaultOutcome::Survived;
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Fail);
        assert_eq!(
            int8_value(&result.fault_results.batch, "result_state", 0),
            FaultResultState::Survived.code()
        );
    }

    #[test]
    fn semantic_fault_requires_independent_expectation_mismatch() {
        let mut fixture = Fixture::complete();
        fixture.violations[0].expectation_id = None;
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Fail);
        assert_eq!(
            int8_value(&result.fault_results.batch, "result_state", 0),
            FaultResultState::EvidenceMismatch.code()
        );
    }

    #[test]
    fn semantic_fault_cannot_borrow_a_mismatch_from_another_scope() {
        let mut fixture = Fixture::complete();
        let other_scope = CoverageScopeId::new(id16(70)).unwrap();
        fixture.oracles[0].requested_scopes.push(other_scope);
        fixture.executions[0].completed_scopes.push(other_scope);
        fixture.faults[0].coverage_scope = other_scope;
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Fail);
        assert_eq!(
            int8_value(&result.fault_results.batch, "result_state", 0),
            FaultResultState::EvidenceMismatch.code()
        );
    }

    #[test]
    fn acceptance_authority_must_be_independent_from_expectation_author() {
        let mut fixture = Fixture::complete();
        fixture.expectations[0].authority.acceptance_authority =
            fixture.expectations[0].authority.author;
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Fail);
    }

    #[test]
    fn producer_authored_required_fault_is_a_terminal_failure() {
        let mut fixture = Fixture::complete();
        fixture.faults[0].authority.author = fixture.producer_owner;
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Fail);
    }

    #[test]
    fn missing_fault_execution_is_unknown() {
        let mut fixture = Fixture::complete();
        fixture.fault_executions.clear();
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Unknown);
        assert_eq!(
            int8_value(&result.fault_results.batch, "result_state", 0),
            FaultResultState::Missing.code()
        );
    }

    #[test]
    fn execution_for_different_candidate_pins_fails() {
        let mut fixture = Fixture::complete();
        fixture.executions[0].candidate_pins.provider_set = ProviderSetRef::from_bytes(id32(99));
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Fail);
    }

    #[test]
    fn zero_candidate_pin_is_rejected_before_proof_execution() {
        let mut fixture = Fixture::complete();
        fixture.pins.program_release = ProgramReleaseRef::from_bytes([0; 32]);
        let error = fixture.evaluate().unwrap_err();
        assert!(matches!(error, ProofError::ZeroCandidatePin));
    }

    #[test]
    fn baseline_violation_fails_while_fault_violation_only_proves_sensitivity() {
        let mut fixture = Fixture::complete();
        fixture.violations.push(ProofViolation {
            violation_id: ViolationId::new(id16(88)).unwrap(),
            oracle_id: oracle_id(),
            expectation_id: Some(expectation_id()),
            fault_id: None,
            kind: ProofViolationKind::SemanticMismatch,
        });
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Fail);
        assert_eq!(
            u64_value(&result.oracle_results.batch, "baseline_violation_count", 0),
            1
        );
    }

    #[test]
    fn capability_without_required_oracle_is_unknown() {
        let mut fixture = Fixture::complete();
        fixture.capability_requirements.clear();
        fixture
            .provenance
            .retain(|edge| !matches!(edge.to, ProvenanceSubject::Capability(_)));
        let result = fixture.evaluate().unwrap();
        assert_eq!(result.terminal, ProofTerminalStatus::Unknown);
        assert_eq!(
            int8_value(&result.capability_results.batch, "status", 0),
            ProofCapabilityStatus::Unknown.code()
        );
    }

    #[test]
    fn contradictory_coverage_is_rejected_before_evaluation() {
        let mut fixture = Fixture::complete();
        fixture.executions[0]
            .unavailable_scopes
            .push(UnavailableCoverage {
                scope: scope_id(),
                reason: CoverageUnavailableReason::Unknown,
            });
        let error = fixture.evaluate().unwrap_err();
        assert!(matches!(error, ProofError::InvalidCoverage));
    }
}
