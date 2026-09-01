//! Immutable activation events and the pure current-head query.
//!
//! This module intentionally contains no mutable pointer, SQLite cache, Delta
//! writer, or `ArcSwap`. Durable adapters append these values; recovery derives
//! the one valid event-chain head from all accepted rows before any serving
//! handle may be published.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;
use std::sync::Arc;

use datafusion::execution::SessionState;

#[cfg(feature = "daemon")]
use super::command::{CommandPins, ExecutionOwner, FabricCommand, FabricCommandPayload};
use super::command::{
    EpochId, ExpectedHead, OperationId, OperationSelectionRef, ProofReceiptRef,
    ResourceEnvelopeRef, RetentionPolicyRef, SourceGeneration, TransactionRef, WorkspaceId,
    WriterFence,
};
#[cfg(feature = "daemon")]
use super::command_effect_contract::{ValidatedCommandAttempt, ValidatedCommandRecovery};
use super::delta_exact::ExactDeltaPin;
use super::delta_write::{
    ApplicationTransactionMarker, CommitReadVersionEvidence, CommittedDeltaWrite,
};
use crate::schema_contract::SchemaContract;

const CONTROL_BINDING_DIGEST_DOMAIN: &[u8] = b"codefabric.activation-control.binding.v1";
const CONTROL_RELATION_DIGEST_DOMAIN: &[u8] = b"codefabric.activation-control.relation-pin.v1";
const BACKEND_COMMIT_DIGEST_DOMAIN: &[u8] = b"codefabric.activation.backend-commit.v1";
const DURABLE_ROW_DIGEST_DOMAIN: &[u8] = b"codefabric.activation.durable-row.v1";
const READBACK_DIGEST_DOMAIN: &[u8] = b"codefabric.activation.readback.v1";
const TABLE_VERSION_SET_DIGEST_DOMAIN: &[u8] = b"codefabric.activation.table-version-set.v1";

struct CanonicalEvidenceDigest(blake3::Hasher);

impl CanonicalEvidenceDigest {
    fn new(domain: &[u8]) -> Self {
        let mut digest = Self(blake3::Hasher::new());
        digest.frame(domain);
        digest
    }

    fn frame(&mut self, bytes: &[u8]) {
        self.0.update(&(bytes.len() as u64).to_be_bytes());
        self.0.update(bytes);
    }

    fn optional_frame(&mut self, bytes: Option<&[u8]>) {
        match bytes {
            Some(bytes) => {
                self.frame(&[1]);
                self.frame(bytes);
            }
            None => self.frame(&[0]),
        }
    }

    fn finish(self) -> [u8; 32] {
        *self.0.finalize().as_bytes()
    }
}

fn canonical_arrow_schema(
    schema: &arrow_schema::SchemaRef,
) -> Result<Vec<u8>, ActivationControlBindingError> {
    let value = serde_json::to_value(schema.as_ref())
        .map_err(|error| ActivationControlBindingError::CanonicalSchema(error.to_string()))?;
    crate::contracts::jcs::canonicalize_value(&value)
        .map_err(|error| ActivationControlBindingError::CanonicalSchema(error.to_string()))
}

macro_rules! activation_identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            #[must_use]
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

activation_identity!(
    /// Stable identity of one immutable activation event row.
    ActivationEventId
);

/// Reducer-validated activation attempt required by every durable activation boundary.
///
/// The token has no public constructor. The command adapter creates it only after the shared
/// effect contract has proved the exact reducer state, transaction executor, active fence, and
/// predecessor. Durable request and event constructors consume this token instead of accepting a
/// raw command plus a merely monotonic writer generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(feature = "daemon")]
pub struct ActivationAttempt {
    validated: ValidatedCommandAttempt,
}

#[cfg(feature = "daemon")]
impl ActivationAttempt {
    pub(crate) const fn from_validated(validated: ValidatedCommandAttempt) -> Self {
        Self { validated }
    }

    /// Exact admitted command bound to this execution attempt.
    #[must_use]
    pub const fn command(&self) -> &FabricCommand {
        self.validated.command_ref()
    }

    /// Reducer-owned attempt number.
    #[must_use]
    pub const fn attempt(self) -> u32 {
        self.validated.attempt()
    }

    /// Exact actor and fence that may attempt the activation append.
    #[must_use]
    pub const fn execution_owner(self) -> ExecutionOwner {
        self.validated.execution_owner()
    }

    /// Exact transaction already proved by the durable reducer state, when this token was
    /// reconstructed for recovery rather than issued for initial execution.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn prepared_transaction(self) -> Option<TransactionRef> {
        self.validated.prepared_transaction()
    }

    #[cfg(test)]
    pub(crate) const fn for_test(
        command: FabricCommand,
        attempt: u32,
        execution_owner: ExecutionOwner,
    ) -> Self {
        Self {
            validated: ValidatedCommandAttempt::for_test(command, attempt, execution_owner),
        }
    }
}

/// Reducer-validated activation recovery authority.
///
/// This token preserves both the immutable transaction attempt and the exact actor/fence that
/// the reducer authorized to read markers and converge post-commit projections. It has no public
/// constructor, so a numerically higher raw writer generation cannot authorize recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(feature = "daemon")]
pub struct ActivationRecoveryAttempt {
    validated: ValidatedCommandRecovery,
}

#[cfg(feature = "daemon")]
impl ActivationRecoveryAttempt {
    pub(crate) const fn from_validated(validated: ValidatedCommandRecovery) -> Self {
        Self { validated }
    }

    /// Immutable transaction attempt being recovered.
    #[must_use]
    pub const fn attempt(self) -> ActivationAttempt {
        ActivationAttempt::from_validated(self.validated.attempt())
    }

    /// Exact current actor and fence authorized to perform recovery reads.
    #[must_use]
    pub const fn active_recovery_owner(self) -> ExecutionOwner {
        self.validated.active_recovery_owner()
    }

    #[cfg(test)]
    pub(crate) const fn for_test(
        attempt: ActivationAttempt,
        active_recovery_owner: ExecutionOwner,
    ) -> Self {
        Self {
            validated: ValidatedCommandRecovery::for_test(attempt.validated, active_recovery_owner),
        }
    }
}
activation_identity!(
    /// Exact Delta-version vector selected by an epoch.
    TableVersionSetRef
);

/// Complete, canonically ordered set of exact Delta relation states selected
/// by one epoch.
///
/// The relation identity is application meaning; the canonical table root and
/// version are Delta state identity. Keeping all three values makes the set
/// sufficient for cold reconstruction instead of treating its digest as a
/// reversible manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableVersionSet {
    reference: TableVersionSetRef,
    components: BTreeMap<Arc<str>, ExactDeltaPin>,
}

impl TableVersionSet {
    /// Construct a non-empty set and derive its canonical reference.
    ///
    /// Input order is irrelevant. Duplicate or empty relation identities fail
    /// closed, including duplicates that happen to name the same exact pin.
    pub fn try_new<I, R>(components: I) -> Result<Self, TableVersionSetError>
    where
        I: IntoIterator<Item = (R, ExactDeltaPin)>,
        R: Into<Arc<str>>,
    {
        let mut ordered = BTreeMap::new();
        for (relation_id, pin) in components {
            let relation_id = relation_id.into();
            if relation_id.trim().is_empty() {
                return Err(TableVersionSetError::EmptyRelationIdentity);
            }
            if ordered.insert(Arc::clone(&relation_id), pin).is_some() {
                return Err(TableVersionSetError::DuplicateRelationIdentity(relation_id));
            }
        }
        if ordered.is_empty() {
            return Err(TableVersionSetError::EmptySet);
        }
        let component_count = u64::try_from(ordered.len())
            .map_err(|_| TableVersionSetError::ComponentCountOverflow)?;
        let mut digest = CanonicalEvidenceDigest::new(TABLE_VERSION_SET_DIGEST_DOMAIN);
        digest.frame(&component_count.to_be_bytes());
        for (relation_id, pin) in &ordered {
            digest.frame(relation_id.as_bytes());
            digest.frame(pin.canonical_root().as_str().as_bytes());
            digest.frame(&pin.version().to_be_bytes());
        }
        Ok(Self {
            reference: TableVersionSetRef::from_bytes(digest.finish()),
            components: ordered,
        })
    }

    /// Canonical content identity persisted in the activation event pins.
    #[must_use]
    pub const fn reference(&self) -> TableVersionSetRef {
        self.reference
    }

    /// Number of exact relation states in this publication.
    #[must_use]
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Resolve one exact component by its stable relation identity.
    #[must_use]
    pub fn pin(&self, relation_id: &str) -> Option<&ExactDeltaPin> {
        self.components.get(relation_id)
    }

    /// Iterate the canonical relation-ID order used by the reference digest.
    pub fn components(
        &self,
    ) -> impl ExactSizeIterator<Item = (&str, &ExactDeltaPin)> + DoubleEndedIterator {
        self.components
            .iter()
            .map(|(relation_id, pin)| (relation_id.as_ref(), pin))
    }
}

/// Invalid exact-version publication set.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TableVersionSetError {
    #[error("a table-version set must contain at least one relation")]
    EmptySet,
    #[error("a table-version component has an empty relation identity")]
    EmptyRelationIdentity,
    #[error("table-version relation identity {0} occurs more than once")]
    DuplicateRelationIdentity(Arc<str>),
    #[error("table-version component count exceeds u64")]
    ComponentCountOverflow,
}
activation_identity!(
    /// Exact immutable in-memory segment set selected by an epoch.
    OverlaySegmentSetRef
);
activation_identity!(
    /// Exact compiled policy set selected by an epoch.
    PolicySetRef
);
activation_identity!(
    /// Application-owned compatibility class used for admission and rollback.
    CompatibilityClassRef
);
activation_identity!(
    /// Backend commit identity read back after the activation append.
    BackendCommitRef
);
activation_identity!(
    /// Durable evidence that readback returned the exact appended event.
    ActivationReadbackRef
);

/// Canonical identity of the concrete DataFusion session and model-compiled
/// provider/transformation binding used to read or append the
/// activation-control relation.
///
/// The identity has no raw-byte constructor. It can only be derived from the
/// observed [`SessionState`] and the exact executable [`SchemaContract`]
/// produced for the selected provider/transformation output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedActivationControlBinding {
    session_id: Arc<str>,
    physical_binding_id: Arc<str>,
    source_schema_identity: Arc<str>,
    qualifier: Arc<str>,
    logical_schema_digest: [u8; 32],
    storage_schema_digest: [u8; 32],
    fingerprint: [u8; 32],
}

impl SealedActivationControlBinding {
    /// Observe and bind the exact session/schema pair selected for the control
    /// relation.
    ///
    /// # Errors
    ///
    /// Rejects empty session/binding identities or schemas which cannot be
    /// encoded through the canonical Arrow JSON contract. `physical_binding_id`
    /// is the identity of the provider/transformation output selected by the
    /// caller; no bootstrap relation table is consulted.
    pub fn try_from_session_and_contract(
        session: &SessionState,
        physical_binding_id: &str,
        contract: &SchemaContract,
    ) -> Result<Self, ActivationControlBindingError> {
        Self::try_from_session_identity_and_contract(
            session.session_id(),
            physical_binding_id,
            contract,
        )
    }

    /// Reconstruct a binding from an exact session identity already read from
    /// durable Delta evidence.
    ///
    /// This constructor is crate-private because a caller-supplied string is
    /// not a live-session observation. The activation-control codec uses it
    /// only after the session identity and binding fingerprint have both been
    /// decoded from a schema-validated durable row.
    pub(crate) fn try_from_recorded_session_and_contract(
        session_id: &str,
        physical_binding_id: &str,
        contract: &SchemaContract,
    ) -> Result<Self, ActivationControlBindingError> {
        Self::try_from_session_identity_and_contract(session_id, physical_binding_id, contract)
    }

    fn try_from_session_identity_and_contract(
        session_id: &str,
        physical_binding_id: &str,
        contract: &SchemaContract,
    ) -> Result<Self, ActivationControlBindingError> {
        if session_id.trim().is_empty() {
            return Err(ActivationControlBindingError::EmptySessionId);
        }
        if physical_binding_id.trim().is_empty() {
            return Err(ActivationControlBindingError::EmptyPhysicalBindingId);
        }
        let logical_schema = canonical_arrow_schema(contract.logical_schema())?;
        let storage_schema = canonical_arrow_schema(contract.storage_schema())?;
        let logical_schema_digest = *blake3::hash(&logical_schema).as_bytes();
        let storage_schema_digest = *blake3::hash(&storage_schema).as_bytes();
        let qualifier = contract.qualifier().to_string();
        let mut digest = CanonicalEvidenceDigest::new(CONTROL_BINDING_DIGEST_DOMAIN);
        digest.frame(session_id.as_bytes());
        digest.frame(physical_binding_id.as_bytes());
        digest.frame(contract.source_schema_identity().as_bytes());
        digest.frame(qualifier.as_bytes());
        digest.frame(&logical_schema);
        digest.frame(&storage_schema);

        Ok(Self {
            session_id: Arc::from(session_id),
            physical_binding_id: Arc::from(physical_binding_id),
            source_schema_identity: Arc::from(contract.source_schema_identity()),
            qualifier: Arc::from(qualifier),
            logical_schema_digest,
            storage_schema_digest,
            fingerprint: digest.finish(),
        })
    }

    /// Reobserve the live session/contract pair and prove it is the same
    /// immutable binding carried by a transaction contract.
    pub fn revalidate(
        &self,
        session: &SessionState,
        physical_binding_id: &str,
        contract: &SchemaContract,
    ) -> Result<(), ActivationControlBindingError> {
        let observed = Self::try_from_session_and_contract(session, physical_binding_id, contract)?;
        if &observed != self {
            return Err(ActivationControlBindingError::ObservationMismatch);
        }
        Ok(())
    }

    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    #[must_use]
    pub fn physical_binding_id(&self) -> &str {
        &self.physical_binding_id
    }

    #[must_use]
    pub fn source_schema_identity(&self) -> &str {
        &self.source_schema_identity
    }

    #[must_use]
    pub fn qualifier(&self) -> &str {
        &self.qualifier
    }

    #[must_use]
    pub const fn logical_schema_digest(&self) -> &[u8; 32] {
        &self.logical_schema_digest
    }

    #[must_use]
    pub const fn storage_schema_digest(&self) -> &[u8; 32] {
        &self.storage_schema_digest
    }

    /// Canonical fingerprint used to bind durable commands to this exact
    /// session/schema observation.
    #[must_use]
    pub const fn fingerprint(&self) -> &[u8; 32] {
        &self.fingerprint
    }

    #[cfg(test)]
    pub(crate) fn for_test(session_id: &str, physical_binding_id: &str) -> Self {
        let source_schema_identity = "test:provider-contract:system.activation_event";
        let qualifier = "codefabric._storage.activation_event";
        let logical_schema_digest = *blake3::hash(b"test logical activation schema").as_bytes();
        let storage_schema_digest = *blake3::hash(b"test storage activation schema").as_bytes();
        let mut digest = CanonicalEvidenceDigest::new(CONTROL_BINDING_DIGEST_DOMAIN);
        for value in [
            session_id,
            physical_binding_id,
            source_schema_identity,
            qualifier,
        ] {
            digest.frame(value.as_bytes());
        }
        digest.frame(&logical_schema_digest);
        digest.frame(&storage_schema_digest);
        Self {
            session_id: Arc::from(session_id),
            physical_binding_id: Arc::from(physical_binding_id),
            source_schema_identity: Arc::from(source_schema_identity),
            qualifier: Arc::from(qualifier),
            logical_schema_digest,
            storage_schema_digest,
            fingerprint: digest.finish(),
        }
    }
}

/// Exact predecessor state and sealed session/schema binding for one
/// activation-control append or reconciliation read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationControlRelationPin {
    table: ExactDeltaPin,
    binding: SealedActivationControlBinding,
    fingerprint: [u8; 32],
}

impl ActivationControlRelationPin {
    #[must_use]
    pub fn new(table: ExactDeltaPin, binding: SealedActivationControlBinding) -> Self {
        let mut digest = CanonicalEvidenceDigest::new(CONTROL_RELATION_DIGEST_DOMAIN);
        digest.frame(table.canonical_root().as_str().as_bytes());
        digest.frame(&table.version().to_be_bytes());
        digest.frame(binding.fingerprint());
        Self {
            table,
            binding,
            fingerprint: digest.finish(),
        }
    }

    #[must_use]
    pub const fn table(&self) -> &ExactDeltaPin {
        &self.table
    }

    #[must_use]
    pub const fn binding(&self) -> &SealedActivationControlBinding {
        &self.binding
    }

    /// Canonical identity of the exact table/session/schema tuple.
    #[must_use]
    pub const fn fingerprint(&self) -> &[u8; 32] {
        &self.fingerprint
    }
}

/// Failure to bind a DataFusion session to one application-owned control relation.
#[derive(Debug, thiserror::Error)]
pub enum ActivationControlBindingError {
    #[error("activation-control session ID is empty")]
    EmptySessionId,
    #[error("activation-control physical-binding ID is empty")]
    EmptyPhysicalBindingId,
    #[error("activation-control live session/schema observation differs from the sealed binding")]
    ObservationMismatch,
    #[error("activation-control Arrow schema canonicalization failed: {0}")]
    CanonicalSchema(String),
}

/// Monotonic position in one workspace activation-event chain. It detects a
/// missing predecessor without using commit time as semantic ordering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActivationOrdinal(NonZeroU64);

impl ActivationOrdinal {
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Complete semantic and physical pins selected by one activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FabricEpochPins {
    pub epoch: EpochId,
    pub input_release: super::command::InputReleaseRef,
    pub program_release: super::command::ProgramReleaseRef,
    pub application_release: super::command::ApplicationReleaseRef,
    pub source_authority: super::command::SourceAuthorityRef,
    pub source_generation: SourceGeneration,
    pub provider_release: super::command::ProviderReleaseRef,
    pub provider_set: super::command::ProviderSetRef,
    pub table_versions: TableVersionSetRef,
    pub overlay_segments: OverlaySegmentSetRef,
    pub policy_set: PolicySetRef,
    pub resource_envelope: ResourceEnvelopeRef,
    pub proof_receipt: ProofReceiptRef,
}

impl FabricEpochPins {
    #[cfg(feature = "daemon")]
    fn command_pins_match(self, command: &FabricCommand) -> bool {
        let CommandPins {
            input_release,
            program_release,
            application_release,
            source_authority,
            source_generation,
            provider_release,
            provider_set,
        } = command.pins;
        self.input_release == input_release
            && self.program_release == program_release
            && self.application_release == application_release
            && self.source_authority == source_authority
            && self.source_generation == source_generation
            && self.provider_release == provider_release
            && self.provider_set == provider_set
            && self.resource_envelope == command.resources
    }
}

/// Commit-history read-version evidence available for an activation append.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationDeltaReadVersionEvidence {
    Exact(u64),
    NotExposedByCommitHistory,
}

impl From<CommitReadVersionEvidence> for ActivationDeltaReadVersionEvidence {
    fn from(value: CommitReadVersionEvidence) -> Self {
        match value {
            CommitReadVersionEvidence::Exact(version) => Self::Exact(version),
            CommitReadVersionEvidence::NotExposedByCommitHistory => Self::NotExposedByCommitHistory,
        }
    }
}

/// Explicit observations read from the exact committed Delta state.
///
/// This is not itself authority: [`ActivationDeltaCommitEvidence::try_new`]
/// binds these observations to the command's exact control predecessor,
/// transaction, and writer fence before a receipt can be derived.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationDeltaCommitObservation {
    predecessor: ExactDeltaPin,
    committed: ExactDeltaPin,
    marker: ApplicationTransactionMarker,
    marker_observed_in: ExactDeltaPin,
    operation_id: OperationId,
    writer_generation: super::command::WriterGeneration,
    session_id: Arc<str>,
    read_version: ActivationDeltaReadVersionEvidence,
    num_retries: u64,
}

impl ActivationDeltaCommitObservation {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        predecessor: ExactDeltaPin,
        committed: ExactDeltaPin,
        marker: ApplicationTransactionMarker,
        marker_observed_in: ExactDeltaPin,
        operation_id: OperationId,
        writer_generation: super::command::WriterGeneration,
        session_id: impl Into<Arc<str>>,
        read_version: ActivationDeltaReadVersionEvidence,
        num_retries: u64,
    ) -> Result<Self, ActivationDeltaEvidenceError> {
        let session_id = session_id.into();
        if session_id.trim().is_empty() {
            return Err(ActivationDeltaEvidenceError::EmptyObservedSessionId);
        }
        Ok(Self {
            predecessor,
            committed,
            marker,
            marker_observed_in,
            operation_id,
            writer_generation,
            session_id,
            read_version,
            num_retries,
        })
    }

    /// Project the already validated output of the shared zero-retry Delta
    /// writer into activation-specific evidence.
    pub fn from_controlled_write(
        write: &CommittedDeltaWrite,
    ) -> Result<Self, ActivationDeltaEvidenceError> {
        Self::try_new(
            write.predecessor().clone(),
            write.committed().clone(),
            write.marker_evidence().marker().clone(),
            write.marker_evidence().observed_in().clone(),
            write.operation_id(),
            write.writer_generation(),
            Arc::<str>::from(write.session_id()),
            write.read_version_evidence().into(),
            write.num_retries(),
        )
    }

    #[must_use]
    pub const fn committed(&self) -> &ExactDeltaPin {
        &self.committed
    }
}

/// Validated exact Delta commit evidence used to derive backend and readback
/// references. Construction rejects every observation which would require a
/// rebase, implicit-latest lookup, nonzero retry, or session substitution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationDeltaCommitEvidence {
    control_predecessor: ActivationControlRelationPin,
    committed: ExactDeltaPin,
    marker_observed_in: ExactDeltaPin,
    operation_id: OperationId,
    transaction: TransactionRef,
    execution_fence: WriterFence,
    read_version: ActivationDeltaReadVersionEvidence,
    num_retries: u64,
    backend_commit: BackendCommitRef,
}

impl ActivationDeltaCommitEvidence {
    /// Bind exact Delta observations to the immutable activation transaction.
    ///
    /// # Errors
    ///
    /// Rejects a different root, anything other than predecessor+1, a marker
    /// not visible in that exact committed snapshot, a noncanonical marker,
    /// operation/writer/session drift, an inconsistent exposed read version,
    /// or any Delta retry.
    pub fn try_new(
        control_predecessor: ActivationControlRelationPin,
        transaction: TransactionRef,
        operation_id: OperationId,
        execution_fence: WriterFence,
        observation: ActivationDeltaCommitObservation,
    ) -> Result<Self, ActivationDeltaEvidenceError> {
        let predecessor = control_predecessor.table();
        if observation.predecessor != *predecessor {
            return Err(ActivationDeltaEvidenceError::PredecessorObservationMismatch);
        }
        let expected_version = predecessor.version().checked_add(1).ok_or(
            ActivationDeltaEvidenceError::ControlVersionOverflow(predecessor.version()),
        )?;
        if observation.committed.canonical_root() != predecessor.canonical_root() {
            return Err(ActivationDeltaEvidenceError::CommittedRootMismatch);
        }
        if observation.committed.version() != expected_version {
            return Err(ActivationDeltaEvidenceError::CommittedVersionMismatch {
                expected: expected_version,
                observed: observation.committed.version(),
            });
        }
        if observation.marker_observed_in != observation.committed {
            return Err(ActivationDeltaEvidenceError::MarkerObservationMismatch);
        }
        let expected_marker = ApplicationTransactionMarker::from_transaction_ref(transaction);
        if observation.marker != expected_marker {
            return Err(ActivationDeltaEvidenceError::TransactionMarkerMismatch);
        }
        if observation.operation_id != operation_id {
            return Err(ActivationDeltaEvidenceError::OperationMismatch);
        }
        if observation.writer_generation != execution_fence.generation {
            return Err(ActivationDeltaEvidenceError::WriterGenerationMismatch);
        }
        if observation.session_id.as_ref() != control_predecessor.binding().session_id() {
            return Err(ActivationDeltaEvidenceError::SessionBindingMismatch);
        }
        if let ActivationDeltaReadVersionEvidence::Exact(version) = observation.read_version
            && version != predecessor.version()
        {
            return Err(ActivationDeltaEvidenceError::ReadVersionMismatch {
                expected: predecessor.version(),
                observed: version,
            });
        }
        if observation.num_retries != 0 {
            return Err(ActivationDeltaEvidenceError::NonzeroRetryCount(
                observation.num_retries,
            ));
        }

        let mut digest = CanonicalEvidenceDigest::new(BACKEND_COMMIT_DIGEST_DOMAIN);
        digest.frame(control_predecessor.fingerprint());
        digest.frame(observation.committed.canonical_root().as_str().as_bytes());
        digest.frame(&observation.committed.version().to_be_bytes());
        digest.frame(observation.marker.application_id().as_bytes());
        digest.frame(&observation.marker.application_version().to_be_bytes());
        digest.frame(
            observation
                .marker_observed_in
                .canonical_root()
                .as_str()
                .as_bytes(),
        );
        digest.frame(&observation.marker_observed_in.version().to_be_bytes());
        digest.frame(operation_id.as_bytes());
        digest.frame(transaction.as_bytes());
        digest.frame(execution_fence.lease_id.as_bytes());
        digest.frame(&execution_fence.generation.get().to_be_bytes());
        match observation.read_version {
            ActivationDeltaReadVersionEvidence::Exact(version) => {
                digest.frame(&[1]);
                digest.frame(&version.to_be_bytes());
            }
            ActivationDeltaReadVersionEvidence::NotExposedByCommitHistory => {
                digest.frame(&[0]);
            }
        }
        digest.frame(&observation.num_retries.to_be_bytes());
        let backend_commit = BackendCommitRef::from_bytes(digest.finish());

        Ok(Self {
            control_predecessor,
            committed: observation.committed,
            marker_observed_in: observation.marker_observed_in,
            operation_id,
            transaction,
            execution_fence,
            read_version: observation.read_version,
            num_retries: observation.num_retries,
            backend_commit,
        })
    }

    #[must_use]
    pub const fn control_predecessor(&self) -> &ActivationControlRelationPin {
        &self.control_predecessor
    }

    #[must_use]
    pub const fn committed(&self) -> &ExactDeltaPin {
        &self.committed
    }

    #[must_use]
    pub const fn marker_observed_in(&self) -> &ExactDeltaPin {
        &self.marker_observed_in
    }

    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    #[must_use]
    pub const fn transaction(&self) -> TransactionRef {
        self.transaction
    }

    #[must_use]
    pub const fn execution_fence(&self) -> WriterFence {
        self.execution_fence
    }

    #[must_use]
    pub const fn read_version(&self) -> ActivationDeltaReadVersionEvidence {
        self.read_version
    }

    #[must_use]
    pub const fn num_retries(&self) -> u64 {
        self.num_retries
    }

    #[must_use]
    pub const fn backend_commit(&self) -> BackendCommitRef {
        self.backend_commit
    }
}

/// Commit fields which exist in the append payload before Delta assigns the
/// committed table version and before exact readback can be observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurableActivationCommit {
    pub operation_selection: OperationSelectionRef,
    pub transaction: TransactionRef,
}

/// Decoded application-owned activation-control row.
///
/// Backend-commit and readback references are deliberately absent because they
/// are post-commit evidence. They are reconstructed from the row plus
/// [`ActivationDeltaCommitEvidence`], avoiding circular or placeholder data in
/// the append itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurableActivationRow {
    pub event_id: ActivationEventId,
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub predecessor_event_id: Option<ActivationEventId>,
    pub predecessor_epoch: ExpectedHead,
    pub ordinal: ActivationOrdinal,
    pub execution_fence: WriterFence,
    pub pins: FabricEpochPins,
    pub compatibility: CompatibilityClassRef,
    pub retention: RetentionPolicyRef,
    pub commit: DurableActivationCommit,
}

impl DurableActivationRow {
    /// Construct the exact pre-commit row from reducer-validated authority.
    #[cfg(feature = "daemon")]
    #[allow(clippy::too_many_arguments)]
    pub fn try_from_attempt(
        event_id: ActivationEventId,
        attempt: ActivationAttempt,
        predecessor_event_id: Option<ActivationEventId>,
        ordinal: ActivationOrdinal,
        pins: FabricEpochPins,
        compatibility: CompatibilityClassRef,
        retention: RetentionPolicyRef,
        commit: DurableActivationCommit,
    ) -> Result<Self, ActivationError> {
        let command = attempt.command();
        let execution_fence = attempt.execution_owner().fence;
        let expected_target = match command.payload {
            FabricCommandPayload::ActivateEpoch {
                candidate_epoch,
                proof_receipt,
            } => {
                if proof_receipt != pins.proof_receipt {
                    return Err(ActivationError::ProofReceiptMismatch);
                }
                candidate_epoch
            }
            FabricCommandPayload::RollbackEpoch { target_epoch, .. } => target_epoch,
            _ => return Err(ActivationError::CommandDoesNotSelectEpoch),
        };
        if expected_target != pins.epoch {
            return Err(ActivationError::SelectedEpochMismatch {
                command: expected_target,
                event: pins.epoch,
            });
        }
        if !pins.command_pins_match(command) {
            return Err(ActivationError::CommandPinMismatch);
        }
        Ok(Self {
            event_id,
            workspace_id: command.ownership.workspace_id,
            operation_id: command.identity.operation_id,
            predecessor_event_id,
            predecessor_epoch: command.expected_head,
            ordinal,
            execution_fence,
            pins,
            compatibility,
            retention,
            commit,
        })
    }

    pub(crate) fn canonical_digest(self) -> [u8; 32] {
        let mut digest = CanonicalEvidenceDigest::new(DURABLE_ROW_DIGEST_DOMAIN);
        digest.frame(self.event_id.as_bytes());
        digest.frame(self.workspace_id.as_bytes());
        digest.frame(self.operation_id.as_bytes());
        digest.optional_frame(
            self.predecessor_event_id
                .as_ref()
                .map(ActivationEventId::as_bytes)
                .map(<[u8; 32]>::as_slice),
        );
        match self.predecessor_epoch {
            ExpectedHead::Empty => digest.frame(&[0]),
            ExpectedHead::Epoch(epoch) => {
                digest.frame(&[1]);
                digest.frame(epoch.as_bytes());
            }
        }
        digest.frame(&self.ordinal.get().to_be_bytes());
        digest.frame(self.execution_fence.lease_id.as_bytes());
        digest.frame(&self.execution_fence.generation.get().to_be_bytes());
        digest.frame(self.pins.epoch.as_bytes());
        digest.frame(self.pins.input_release.as_bytes());
        digest.frame(self.pins.program_release.as_bytes());
        digest.frame(self.pins.application_release.as_bytes());
        digest.frame(self.pins.source_authority.as_bytes());
        digest.frame(&self.pins.source_generation.get().to_be_bytes());
        digest.frame(self.pins.provider_release.as_bytes());
        digest.frame(self.pins.provider_set.as_bytes());
        digest.frame(self.pins.table_versions.as_bytes());
        digest.frame(self.pins.overlay_segments.as_bytes());
        digest.frame(self.pins.policy_set.as_bytes());
        digest.frame(self.pins.resource_envelope.as_bytes());
        digest.frame(self.pins.proof_receipt.as_bytes());
        digest.frame(self.compatibility.as_bytes());
        digest.frame(self.retention.as_bytes());
        digest.frame(self.commit.operation_selection.as_bytes());
        digest.frame(self.commit.transaction.as_bytes());
        digest.finish()
    }
}

/// Failure to turn explicit Delta observations into durable activation
/// evidence or to bind that evidence to one decoded row.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ActivationDeltaEvidenceError {
    #[error("observed activation-control session ID is empty")]
    EmptyObservedSessionId,
    #[error("activation-control predecessor version {0} cannot advance")]
    ControlVersionOverflow(u64),
    #[error("observed Delta predecessor differs from the command's exact control pin")]
    PredecessorObservationMismatch,
    #[error("committed activation-control root differs from the exact predecessor root")]
    CommittedRootMismatch,
    #[error(
        "committed activation-control version differs: expected {expected}, observed {observed}"
    )]
    CommittedVersionMismatch { expected: u64, observed: u64 },
    #[error("application marker was not observed in the exact committed activation-control pin")]
    MarkerObservationMismatch,
    #[error("observed Delta application marker differs from the transaction-derived marker")]
    TransactionMarkerMismatch,
    #[error("observed Delta operation differs from the activation row operation")]
    OperationMismatch,
    #[error("observed Delta writer generation differs from the activation execution fence")]
    WriterGenerationMismatch,
    #[error("observed Delta session differs from the sealed activation-control binding")]
    SessionBindingMismatch,
    #[error("observed Delta read version differs: expected {expected}, observed {observed}")]
    ReadVersionMismatch { expected: u64, observed: u64 },
    #[error("activation-control Delta write reported {0} retries; exactly zero is required")]
    NonzeroRetryCount(u64),
    #[error("decoded activation row transaction differs from the exact Delta marker")]
    RowTransactionMismatch,
    #[error("decoded activation row fence differs from the exact Delta commit evidence")]
    RowFenceMismatch,
    #[error("decoded activation row was not read from the exact committed Delta pin")]
    RowReadbackPinMismatch,
}

/// Exact durable commit/readback evidence for the final activation append.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationCommit {
    pub operation_selection: OperationSelectionRef,
    pub transaction: TransactionRef,
    pub backend_commit: BackendCommitRef,
    pub readback: ActivationReadbackRef,
}

/// One append-only selection event. The event graph uses predecessor event IDs,
/// not selected epoch IDs, so a governed rollback may reselect a retained epoch
/// without creating a cycle in activation history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationEvent {
    event_id: ActivationEventId,
    workspace_id: WorkspaceId,
    operation_id: OperationId,
    predecessor_event_id: Option<ActivationEventId>,
    predecessor_epoch: ExpectedHead,
    ordinal: ActivationOrdinal,
    execution_fence: WriterFence,
    pins: FabricEpochPins,
    compatibility: CompatibilityClassRef,
    retention: RetentionPolicyRef,
    commit: ActivationCommit,
}

impl ActivationEvent {
    /// Construct an event from the exact reducer-validated activation/rollback attempt whose
    /// durable result it records.
    ///
    /// # Errors
    ///
    /// Rejects non-selection attempts or any disagreement in target, predecessor,
    /// compiler/model/source/provider/resource pins, or activation proof receipt. The exact
    /// execution fence is obtained only from the validated attempt token.
    #[cfg(feature = "daemon")]
    #[allow(clippy::too_many_arguments)]
    pub fn try_from_attempt(
        event_id: ActivationEventId,
        attempt: ActivationAttempt,
        predecessor_event_id: Option<ActivationEventId>,
        ordinal: ActivationOrdinal,
        pins: FabricEpochPins,
        compatibility: CompatibilityClassRef,
        retention: RetentionPolicyRef,
        commit: ActivationCommit,
    ) -> Result<Self, ActivationError> {
        let row = DurableActivationRow::try_from_attempt(
            event_id,
            attempt,
            predecessor_event_id,
            ordinal,
            pins,
            compatibility,
            retention,
            DurableActivationCommit {
                operation_selection: commit.operation_selection,
                transaction: commit.transaction,
            },
        )?;
        Ok(Self::from_durable_parts(
            row,
            commit.backend_commit,
            commit.readback,
        ))
    }

    /// Reconstruct one event after reading a decoded durable row from the exact
    /// committed activation-control version.
    ///
    /// Unlike [`Self::try_from_attempt`], this path does not require an
    /// ephemeral reducer token and is therefore usable during restart. The
    /// exact Delta evidence must match the row's operation, transaction, and
    /// writer fence before canonical receipts are derived.
    pub fn try_from_durable_row(
        row: DurableActivationRow,
        observed_in: &ExactDeltaPin,
        evidence: &ActivationDeltaCommitEvidence,
    ) -> Result<Self, ActivationDeltaEvidenceError> {
        if observed_in != evidence.committed() {
            return Err(ActivationDeltaEvidenceError::RowReadbackPinMismatch);
        }
        if row.operation_id != evidence.operation_id {
            return Err(ActivationDeltaEvidenceError::OperationMismatch);
        }
        if row.commit.transaction != evidence.transaction {
            return Err(ActivationDeltaEvidenceError::RowTransactionMismatch);
        }
        if row.execution_fence != evidence.execution_fence {
            return Err(ActivationDeltaEvidenceError::RowFenceMismatch);
        }
        let backend_commit = evidence.backend_commit();
        let mut readback = CanonicalEvidenceDigest::new(READBACK_DIGEST_DOMAIN);
        readback.frame(evidence.control_predecessor().fingerprint());
        readback.frame(observed_in.canonical_root().as_str().as_bytes());
        readback.frame(&observed_in.version().to_be_bytes());
        readback.frame(backend_commit.as_bytes());
        readback.frame(&row.canonical_digest());
        let readback = ActivationReadbackRef::from_bytes(readback.finish());
        Ok(Self::from_durable_parts(row, backend_commit, readback))
    }

    const fn from_durable_parts(
        row: DurableActivationRow,
        backend_commit: BackendCommitRef,
        readback: ActivationReadbackRef,
    ) -> Self {
        Self {
            event_id: row.event_id,
            workspace_id: row.workspace_id,
            operation_id: row.operation_id,
            predecessor_event_id: row.predecessor_event_id,
            predecessor_epoch: row.predecessor_epoch,
            ordinal: row.ordinal,
            execution_fence: row.execution_fence,
            pins: row.pins,
            compatibility: row.compatibility,
            retention: row.retention,
            commit: ActivationCommit {
                operation_selection: row.commit.operation_selection,
                transaction: row.commit.transaction,
                backend_commit,
                readback,
            },
        }
    }

    /// Return the exact pre-commit payload represented by this event. Derived
    /// backend/readback receipts are intentionally excluded.
    #[must_use]
    pub const fn durable_row(self) -> DurableActivationRow {
        DurableActivationRow {
            event_id: self.event_id,
            workspace_id: self.workspace_id,
            operation_id: self.operation_id,
            predecessor_event_id: self.predecessor_event_id,
            predecessor_epoch: self.predecessor_epoch,
            ordinal: self.ordinal,
            execution_fence: self.execution_fence,
            pins: self.pins,
            compatibility: self.compatibility,
            retention: self.retention,
            commit: DurableActivationCommit {
                operation_selection: self.commit.operation_selection,
                transaction: self.commit.transaction,
            },
        }
    }

    #[must_use]
    pub const fn event_id(self) -> ActivationEventId {
        self.event_id
    }

    #[must_use]
    pub const fn workspace_id(self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn operation_id(self) -> OperationId {
        self.operation_id
    }

    #[must_use]
    pub const fn predecessor_event_id(self) -> Option<ActivationEventId> {
        self.predecessor_event_id
    }

    #[must_use]
    pub const fn predecessor_epoch(self) -> ExpectedHead {
        self.predecessor_epoch
    }

    #[must_use]
    pub const fn ordinal(self) -> ActivationOrdinal {
        self.ordinal
    }

    #[must_use]
    pub const fn execution_fence(self) -> WriterFence {
        self.execution_fence
    }

    #[must_use]
    pub const fn pins(self) -> FabricEpochPins {
        self.pins
    }

    #[must_use]
    pub const fn compatibility(self) -> CompatibilityClassRef {
        self.compatibility
    }

    #[must_use]
    pub const fn retention(self) -> RetentionPolicyRef {
        self.retention
    }

    #[must_use]
    pub const fn commit(self) -> ActivationCommit {
        self.commit
    }
}

/// Validated activation chain and its uniquely derived current event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationChain {
    workspace_id: WorkspaceId,
    ordered_events: Vec<ActivationEvent>,
}

impl ActivationChain {
    /// Derive the unique linear head from an unordered set of durable rows.
    ///
    /// # Errors
    ///
    /// Rejects duplicates, forks, missing predecessors, gaps, cycles,
    /// disconnected components, workspace disagreement, and regressing writer
    /// generations.
    pub fn derive(
        workspace_id: WorkspaceId,
        events: impl IntoIterator<Item = ActivationEvent>,
    ) -> Result<Self, ActivationError> {
        let mut by_id = BTreeMap::new();
        let mut operations = BTreeSet::new();
        let mut selections = BTreeSet::new();
        let mut transactions = BTreeSet::new();
        for event in events {
            if event.workspace_id != workspace_id {
                return Err(ActivationError::WorkspaceMismatch {
                    expected: workspace_id,
                    actual: event.workspace_id,
                });
            }
            if by_id.insert(event.event_id, event).is_some() {
                return Err(ActivationError::DuplicateEvent(event.event_id));
            }
            if !operations.insert(event.operation_id) {
                return Err(ActivationError::DuplicateOperation(event.operation_id));
            }
            if !selections.insert(event.commit.operation_selection) {
                return Err(ActivationError::DuplicateOperationSelection(
                    event.commit.operation_selection,
                ));
            }
            if !transactions.insert(event.commit.transaction) {
                return Err(ActivationError::DuplicateTransaction(
                    event.commit.transaction,
                ));
            }
        }
        if by_id.is_empty() {
            return Ok(Self {
                workspace_id,
                ordered_events: Vec::new(),
            });
        }

        let roots = by_id
            .values()
            .filter(|event| event.predecessor_event_id.is_none())
            .copied()
            .collect::<Vec<_>>();
        if roots.len() != 1 {
            return Err(ActivationError::RootCount(roots.len()));
        }
        let root = roots[0];
        if root.predecessor_epoch != ExpectedHead::Empty || root.ordinal.get() != 1 {
            return Err(ActivationError::InvalidRoot(root.event_id));
        }

        let mut child_by_parent = BTreeMap::new();
        for event in by_id.values().copied() {
            let Some(parent_id) = event.predecessor_event_id else {
                continue;
            };
            let parent = by_id
                .get(&parent_id)
                .ok_or(ActivationError::MissingPredecessor {
                    event: event.event_id,
                    predecessor: parent_id,
                })?;
            if child_by_parent.insert(parent_id, event.event_id).is_some() {
                return Err(ActivationError::Fork(parent_id));
            }
            if event.predecessor_epoch != ExpectedHead::Epoch(parent.pins.epoch) {
                return Err(ActivationError::PredecessorEpochMismatch {
                    event: event.event_id,
                    predecessor: parent_id,
                });
            }
            if event.ordinal.get() != parent.ordinal.get() + 1 {
                return Err(ActivationError::OrdinalGap {
                    event: event.event_id,
                    expected: parent.ordinal.get() + 1,
                    actual: event.ordinal.get(),
                });
            }
            if event.execution_fence.generation < parent.execution_fence.generation {
                return Err(ActivationError::WriterGenerationRegression {
                    event: event.event_id,
                    predecessor: parent_id,
                });
            }
        }

        let mut ordered_events = Vec::with_capacity(by_id.len());
        let mut next = root.event_id;
        loop {
            let event = *by_id
                .get(&next)
                .expect("root and every linked child were validated present");
            ordered_events.push(event);
            let Some(child) = child_by_parent.get(&next).copied() else {
                break;
            };
            next = child;
        }
        if ordered_events.len() != by_id.len() {
            return Err(ActivationError::DisconnectedOrCyclicHistory {
                reachable: ordered_events.len(),
                total: by_id.len(),
            });
        }
        Ok(Self {
            workspace_id,
            ordered_events,
        })
    }

    /// Reconstruct a restart-safe chain from decoded durable rows and the
    /// exact Delta commit evidence for each row.
    ///
    /// # Errors
    ///
    /// Reports the event whose receipt evidence failed before applying the
    /// ordinary chain/fork/predecessor validation.
    pub fn try_from_durable_rows(
        workspace_id: WorkspaceId,
        rows: impl IntoIterator<
            Item = (
                DurableActivationRow,
                ExactDeltaPin,
                ActivationDeltaCommitEvidence,
            ),
        >,
    ) -> Result<Self, ActivationReconstructionError> {
        let events = rows
            .into_iter()
            .map(|(row, observed_in, evidence)| {
                let event_id = row.event_id;
                ActivationEvent::try_from_durable_row(row, &observed_in, &evidence)
                    .map_err(|source| ActivationReconstructionError::Row { event_id, source })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::derive(workspace_id, events).map_err(ActivationReconstructionError::Chain)
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub fn events(&self) -> &[ActivationEvent] {
        &self.ordered_events
    }

    #[must_use]
    pub fn head_event(&self) -> Option<&ActivationEvent> {
        self.ordered_events.last()
    }

    /// Current is a query over the chain, including the empty-workspace state.
    #[must_use]
    pub fn current_head(&self) -> ExpectedHead {
        self.head_event().map_or(ExpectedHead::Empty, |event| {
            ExpectedHead::Epoch(event.pins.epoch)
        })
    }
}

/// Restart reconstruction failure, preserving whether exact row evidence or
/// the decoded event graph failed closed.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ActivationReconstructionError {
    #[error("activation row {event_id:?} failed exact Delta evidence validation: {source}")]
    Row {
        event_id: ActivationEventId,
        source: ActivationDeltaEvidenceError,
    },
    #[error(transparent)]
    Chain(ActivationError),
}

/// Typed reasons an activation event or chain is not authoritative.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ActivationError {
    #[error("the command does not select a fabric epoch")]
    CommandDoesNotSelectEpoch,
    #[error("activation command proof receipt differs from the selected epoch proof")]
    ProofReceiptMismatch,
    #[error("selected epoch differs: command {command:?}, event {event:?}")]
    SelectedEpochMismatch { command: EpochId, event: EpochId },
    #[error("activation compiler/model/source/provider/resource pins differ from the command")]
    CommandPinMismatch,
    #[error("event belongs to workspace {actual:?}, expected {expected:?}")]
    WorkspaceMismatch {
        expected: WorkspaceId,
        actual: WorkspaceId,
    },
    #[error("duplicate activation event {0:?}")]
    DuplicateEvent(ActivationEventId),
    #[error("duplicate activation operation {0:?}")]
    DuplicateOperation(OperationId),
    #[error("duplicate operation-selection record {0:?}")]
    DuplicateOperationSelection(OperationSelectionRef),
    #[error("duplicate activation transaction {0:?}")]
    DuplicateTransaction(TransactionRef),
    #[error("activation history has {0} roots, expected exactly one")]
    RootCount(usize),
    #[error("activation root {0:?} does not start at empty ordinal one")]
    InvalidRoot(ActivationEventId),
    #[error("event {event:?} names missing predecessor {predecessor:?}")]
    MissingPredecessor {
        event: ActivationEventId,
        predecessor: ActivationEventId,
    },
    #[error("predecessor event {0:?} has more than one child")]
    Fork(ActivationEventId),
    #[error("event {event:?} predecessor epoch differs from event {predecessor:?}")]
    PredecessorEpochMismatch {
        event: ActivationEventId,
        predecessor: ActivationEventId,
    },
    #[error("event {event:?} ordinal differs: expected {expected}, actual {actual}")]
    OrdinalGap {
        event: ActivationEventId,
        expected: u64,
        actual: u64,
    },
    #[error("event {event:?} writer generation regresses from {predecessor:?}")]
    WriterGenerationRegression {
        event: ActivationEventId,
        predecessor: ActivationEventId,
    },
    #[error("activation history is disconnected or cyclic: reachable {reachable}, total {total}")]
    DisconnectedOrCyclicHistory { reachable: usize, total: usize },
}

#[cfg(all(test, feature = "daemon"))]
mod tests {
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::common::TableReference;
    use datafusion::execution::SessionStateBuilder;

    use super::*;
    use crate::fabric::command::{
        ActorId, AuthorizationRef, CommandIdentity, CommandOwnership, IdempotencyKey,
        InputReleaseRef, LeaseId, PrincipalId, ProgramReleaseRef, ProviderSetRef,
        RollbackAuthorizationRef, WriterGeneration,
    };
    use crate::schema_contract::FieldIndexMapping;
    use url::Url;

    const fn id16(seed: u8) -> [u8; 16] {
        [seed; 16]
    }

    const fn id32(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn control_relation(version: u64) -> ActivationControlRelationPin {
        let physical_binding_id = "binding.system.activation-control.delta";
        let schema = Arc::new(Schema::new(vec![Field::new(
            "event_id",
            DataType::FixedSizeBinary(32),
            false,
        )]));
        let contract = SchemaContract::try_new(
            "model-epoch:test:system.activation_event",
            TableReference::full("codefabric", "_storage", "activation_event"),
            Arc::clone(&schema),
            schema,
            vec![FieldIndexMapping::direct(0, 0)],
        )
        .unwrap();
        let session = SessionStateBuilder::new().with_default_features().build();
        let binding = SealedActivationControlBinding::try_from_session_and_contract(
            &session,
            physical_binding_id,
            &contract,
        )
        .unwrap();
        let root = Url::parse("memory:///codefabric/activation-control").unwrap();
        ActivationControlRelationPin::new(ExactDeltaPin::new(&root, version).unwrap(), binding)
    }

    #[test]
    fn table_version_set_is_order_independent_but_root_and_version_sensitive() {
        let first_root = Url::parse("s3://codefabric-test/observations/first").unwrap();
        let second_root = Url::parse("s3://codefabric-test/observations/second").unwrap();
        let first_pin = ExactDeltaPin::new(&first_root, 7).unwrap();
        let second_pin = ExactDeltaPin::new(&second_root, 11).unwrap();
        let forward = TableVersionSet::try_new([
            (Arc::<str>::from("system.first"), first_pin.clone()),
            (Arc::<str>::from("system.second"), second_pin.clone()),
        ])
        .unwrap();
        let reverse = TableVersionSet::try_new([
            (Arc::<str>::from("system.second"), second_pin.clone()),
            (Arc::<str>::from("system.first"), first_pin),
        ])
        .unwrap();
        assert_eq!(forward, reverse);
        assert_eq!(
            forward
                .components()
                .map(|(relation, _)| relation)
                .collect::<Vec<_>>(),
            vec!["system.first", "system.second"]
        );

        let advanced = TableVersionSet::try_new([
            (
                Arc::<str>::from("system.first"),
                ExactDeltaPin::new(&first_root, 8).unwrap(),
            ),
            (Arc::<str>::from("system.second"), second_pin),
        ])
        .unwrap();
        assert_ne!(forward.reference(), advanced.reference());
        assert!(matches!(
            TableVersionSet::try_new(std::iter::empty::<(Arc<str>, ExactDeltaPin)>()),
            Err(TableVersionSetError::EmptySet)
        ));
    }

    fn commit_evidence(
        control: ActivationControlRelationPin,
        command: &FabricCommand,
        transaction: TransactionRef,
    ) -> ActivationDeltaCommitEvidence {
        let root = control.table().canonical_root().clone();
        let committed = ExactDeltaPin::new(&root, control.table().version() + 1).unwrap();
        let observation = ActivationDeltaCommitObservation::try_new(
            control.table().clone(),
            committed.clone(),
            ApplicationTransactionMarker::from_transaction_ref(transaction),
            committed,
            command.identity.operation_id,
            command.writer_fence.generation,
            Arc::<str>::from(control.binding().session_id()),
            ActivationDeltaReadVersionEvidence::Exact(control.table().version()),
            0,
        )
        .unwrap();
        ActivationDeltaCommitEvidence::try_new(
            control,
            transaction,
            command.identity.operation_id,
            command.writer_fence,
            observation,
        )
        .unwrap()
    }

    fn command(
        operation_seed: u8,
        workspace: WorkspaceId,
        expected_head: ExpectedHead,
        target: EpochId,
        generation: u64,
        proof_seed: u8,
        rollback: bool,
    ) -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(id16(operation_seed)),
                idempotency_key: IdempotencyKey::from_bytes(id32(operation_seed)),
            },
            ownership: CommandOwnership {
                workspace_id: workspace,
                principal_id: PrincipalId::from_bytes(id16(2)),
                authorization: AuthorizationRef::from_bytes(id32(3)),
            },
            expected_head,
            writer_fence: WriterFence {
                lease_id: LeaseId::from_bytes(id16(4)),
                generation: WriterGeneration::new(generation).unwrap(),
            },
            pins: CommandPins {
                input_release: InputReleaseRef::from_bytes(id32(5)),
                program_release: ProgramReleaseRef::from_bytes(id32(6)),
                application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(
                    id32(6),
                ),
                source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(id32(6)),
                provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(id32(6)),
                source_generation: SourceGeneration::new(7),
                provider_set: ProviderSetRef::from_bytes(id32(8)),
            },
            resources: ResourceEnvelopeRef::from_bytes(id32(9)),
            payload: if rollback {
                FabricCommandPayload::RollbackEpoch {
                    target_epoch: target,
                    authorization: RollbackAuthorizationRef::from_bytes(id32(10)),
                }
            } else {
                FabricCommandPayload::ActivateEpoch {
                    candidate_epoch: target,
                    proof_receipt: ProofReceiptRef::from_bytes(id32(proof_seed)),
                }
            },
        }
    }

    fn pins(command: &FabricCommand, target: EpochId, proof_seed: u8) -> FabricEpochPins {
        FabricEpochPins {
            epoch: target,
            input_release: command.pins.input_release,
            program_release: command.pins.program_release,
            application_release: command.pins.application_release,
            source_authority: command.pins.source_authority,
            provider_release: command.pins.provider_release,
            source_generation: command.pins.source_generation,
            provider_set: command.pins.provider_set,
            table_versions: TableVersionSetRef::from_bytes(id32(11)),
            overlay_segments: OverlaySegmentSetRef::from_bytes(id32(12)),
            policy_set: PolicySetRef::from_bytes(id32(13)),
            resource_envelope: command.resources,
            proof_receipt: ProofReceiptRef::from_bytes(id32(proof_seed)),
        }
    }

    fn attempt(command: &FabricCommand) -> ActivationAttempt {
        ActivationAttempt::for_test(
            *command,
            1,
            ExecutionOwner {
                actor_id: ActorId::from_bytes(id16(33)),
                fence: command.writer_fence,
            },
        )
    }

    fn event(
        event_seed: u8,
        command: &FabricCommand,
        predecessor: Option<ActivationEventId>,
        ordinal: u64,
        target: EpochId,
        proof_seed: u8,
    ) -> ActivationEvent {
        ActivationEvent::try_from_attempt(
            ActivationEventId::from_bytes(id32(event_seed)),
            attempt(command),
            predecessor,
            ActivationOrdinal::new(ordinal).unwrap(),
            pins(command, target, proof_seed),
            CompatibilityClassRef::from_bytes(id32(14)),
            RetentionPolicyRef::from_bytes(id32(15)),
            ActivationCommit {
                operation_selection: OperationSelectionRef::from_bytes(id32(event_seed + 30)),
                transaction: TransactionRef::from_bytes(id32(event_seed + 60)),
                backend_commit: BackendCommitRef::from_bytes(id32(event_seed + 90)),
                readback: ActivationReadbackRef::from_bytes(id32(event_seed + 120)),
            },
        )
        .unwrap()
    }

    #[test]
    fn derives_one_head_from_unordered_events() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let first_epoch = EpochId::from_bytes(id16(20));
        let second_epoch = EpochId::from_bytes(id16(21));
        let first_command = command(1, workspace, ExpectedHead::Empty, first_epoch, 1, 40, false);
        let first = event(1, &first_command, None, 1, first_epoch, 40);
        let second_command = command(
            2,
            workspace,
            ExpectedHead::Epoch(first_epoch),
            second_epoch,
            1,
            41,
            false,
        );
        let second = event(
            2,
            &second_command,
            Some(first.event_id()),
            2,
            second_epoch,
            41,
        );

        let chain = ActivationChain::derive(workspace, [second, first]).unwrap();
        assert_eq!(chain.events(), &[first, second]);
        assert_eq!(chain.current_head(), ExpectedHead::Epoch(second_epoch));
    }

    #[test]
    fn rollback_reselects_an_epoch_without_cycling_event_history() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let first_epoch = EpochId::from_bytes(id16(20));
        let second_epoch = EpochId::from_bytes(id16(21));
        let first_command = command(1, workspace, ExpectedHead::Empty, first_epoch, 1, 40, false);
        let first = event(1, &first_command, None, 1, first_epoch, 40);
        let second_command = command(
            2,
            workspace,
            ExpectedHead::Epoch(first_epoch),
            second_epoch,
            1,
            41,
            false,
        );
        let second = event(
            2,
            &second_command,
            Some(first.event_id()),
            2,
            second_epoch,
            41,
        );
        let rollback_command = command(
            3,
            workspace,
            ExpectedHead::Epoch(second_epoch),
            first_epoch,
            2,
            40,
            true,
        );
        let rollback = event(
            3,
            &rollback_command,
            Some(second.event_id()),
            3,
            first_epoch,
            40,
        );

        let chain = ActivationChain::derive(workspace, [rollback, first, second]).unwrap();
        assert_eq!(chain.current_head(), ExpectedHead::Epoch(first_epoch));
        assert_eq!(chain.events().len(), 3);
    }

    #[test]
    fn fork_missing_predecessor_and_generation_regression_fail_closed() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let root_epoch = EpochId::from_bytes(id16(20));
        let root_command = command(1, workspace, ExpectedHead::Empty, root_epoch, 2, 40, false);
        let root = event(1, &root_command, None, 1, root_epoch, 40);
        let next_epoch = EpochId::from_bytes(id16(21));
        let next_command = command(
            2,
            workspace,
            ExpectedHead::Epoch(root_epoch),
            next_epoch,
            1,
            41,
            false,
        );
        let next = event(2, &next_command, Some(root.event_id()), 2, next_epoch, 41);
        assert!(matches!(
            ActivationChain::derive(workspace, [root, next]),
            Err(ActivationError::WriterGenerationRegression { .. })
        ));

        let next_command = command(
            2,
            workspace,
            ExpectedHead::Epoch(root_epoch),
            next_epoch,
            2,
            41,
            false,
        );
        let child_a = event(2, &next_command, Some(root.event_id()), 2, next_epoch, 41);
        let third_epoch = EpochId::from_bytes(id16(22));
        let third_command = command(
            3,
            workspace,
            ExpectedHead::Epoch(root_epoch),
            third_epoch,
            2,
            42,
            false,
        );
        let child_b = event(3, &third_command, Some(root.event_id()), 2, third_epoch, 42);
        assert!(matches!(
            ActivationChain::derive(workspace, [root, child_a, child_b]),
            Err(ActivationError::Fork(_))
        ));

        let missing = ActivationEventId::from_bytes(id32(99));
        let orphan = event(2, &next_command, Some(missing), 2, next_epoch, 41);
        assert!(matches!(
            ActivationChain::derive(workspace, [root, orphan]),
            Err(ActivationError::MissingPredecessor { .. })
        ));
    }

    #[test]
    fn command_target_proof_and_pin_disagreement_are_rejected() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let target = EpochId::from_bytes(id16(20));
        let command = command(1, workspace, ExpectedHead::Empty, target, 1, 40, false);
        let mut wrong_proof = pins(&command, target, 41);
        let base_args = (
            ActivationEventId::from_bytes(id32(1)),
            None,
            ActivationOrdinal::new(1).unwrap(),
            CompatibilityClassRef::from_bytes(id32(14)),
            RetentionPolicyRef::from_bytes(id32(15)),
            ActivationCommit {
                operation_selection: OperationSelectionRef::from_bytes(id32(31)),
                transaction: TransactionRef::from_bytes(id32(61)),
                backend_commit: BackendCommitRef::from_bytes(id32(91)),
                readback: ActivationReadbackRef::from_bytes(id32(121)),
            },
        );
        assert!(matches!(
            ActivationEvent::try_from_attempt(
                base_args.0,
                attempt(&command),
                base_args.1,
                base_args.2,
                wrong_proof,
                base_args.3,
                base_args.4,
                base_args.5,
            ),
            Err(ActivationError::ProofReceiptMismatch)
        ));

        wrong_proof.proof_receipt = ProofReceiptRef::from_bytes(id32(40));
        wrong_proof.provider_set = ProviderSetRef::from_bytes(id32(99));
        assert!(matches!(
            ActivationEvent::try_from_attempt(
                base_args.0,
                attempt(&command),
                base_args.1,
                base_args.2,
                wrong_proof,
                base_args.3,
                base_args.4,
                base_args.5,
            ),
            Err(ActivationError::CommandPinMismatch)
        ));
    }

    #[test]
    fn exact_delta_evidence_reconstructs_event_and_chain_without_attempt_token() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let target = EpochId::from_bytes(id16(20));
        let command = command(1, workspace, ExpectedHead::Empty, target, 7, 40, false);
        let transaction = TransactionRef::from_bytes(id32(61));
        let row = DurableActivationRow::try_from_attempt(
            ActivationEventId::from_bytes(id32(1)),
            attempt(&command),
            None,
            ActivationOrdinal::new(1).unwrap(),
            pins(&command, target, 40),
            CompatibilityClassRef::from_bytes(id32(14)),
            RetentionPolicyRef::from_bytes(id32(15)),
            DurableActivationCommit {
                operation_selection: OperationSelectionRef::from_bytes(id32(31)),
                transaction,
            },
        )
        .unwrap();
        let evidence = commit_evidence(control_relation(8), &command, transaction);
        let observed_in = evidence.committed().clone();

        let first = ActivationEvent::try_from_durable_row(row, &observed_in, &evidence).unwrap();
        let second = ActivationEvent::try_from_durable_row(row, &observed_in, &evidence).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.durable_row(), row);
        assert_eq!(first.commit().backend_commit, evidence.backend_commit());

        let chain =
            ActivationChain::try_from_durable_rows(workspace, [(row, observed_in, evidence)])
                .unwrap();
        assert_eq!(chain.head_event(), Some(&first));
        assert_eq!(chain.current_head(), ExpectedHead::Epoch(target));
    }

    #[test]
    fn receipt_changes_with_exact_commit_or_row_and_rejects_nonzero_retry() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let target = EpochId::from_bytes(id16(20));
        let command = command(1, workspace, ExpectedHead::Empty, target, 7, 40, false);
        let transaction = TransactionRef::from_bytes(id32(61));
        let row = DurableActivationRow::try_from_attempt(
            ActivationEventId::from_bytes(id32(1)),
            attempt(&command),
            None,
            ActivationOrdinal::new(1).unwrap(),
            pins(&command, target, 40),
            CompatibilityClassRef::from_bytes(id32(14)),
            RetentionPolicyRef::from_bytes(id32(15)),
            DurableActivationCommit {
                operation_selection: OperationSelectionRef::from_bytes(id32(31)),
                transaction,
            },
        )
        .unwrap();
        let first_evidence = commit_evidence(control_relation(8), &command, transaction);
        let first_observed_in = first_evidence.committed().clone();
        let first = ActivationEvent::try_from_durable_row(row, &first_observed_in, &first_evidence)
            .unwrap();

        let second_evidence = commit_evidence(control_relation(9), &command, transaction);
        let second_observed_in = second_evidence.committed().clone();
        let second =
            ActivationEvent::try_from_durable_row(row, &second_observed_in, &second_evidence)
                .unwrap();
        assert_ne!(
            first.commit().backend_commit,
            second.commit().backend_commit
        );
        assert_ne!(first.commit().readback, second.commit().readback);

        let mut changed_row = row;
        changed_row.compatibility = CompatibilityClassRef::from_bytes(id32(99));
        let changed =
            ActivationEvent::try_from_durable_row(changed_row, &first_observed_in, &first_evidence)
                .unwrap();
        assert_eq!(
            first.commit().backend_commit,
            changed.commit().backend_commit
        );
        assert_ne!(first.commit().readback, changed.commit().readback);

        let control = control_relation(8);
        let root = control.table().canonical_root().clone();
        let committed = ExactDeltaPin::new(&root, 9).unwrap();
        let retried = ActivationDeltaCommitObservation::try_new(
            control.table().clone(),
            committed.clone(),
            ApplicationTransactionMarker::from_transaction_ref(transaction),
            committed,
            command.identity.operation_id,
            command.writer_fence.generation,
            Arc::<str>::from(control.binding().session_id()),
            ActivationDeltaReadVersionEvidence::Exact(8),
            1,
        )
        .unwrap();
        assert_eq!(
            ActivationDeltaCommitEvidence::try_new(
                control,
                transaction,
                command.identity.operation_id,
                command.writer_fence,
                retried,
            ),
            Err(ActivationDeltaEvidenceError::NonzeroRetryCount(1))
        );
    }

    #[test]
    fn sealed_control_binding_uses_programmatic_schema_and_rejects_substitution() {
        let physical_binding_id = "binding.system.activation-control.delta";
        let schema = Arc::new(Schema::new(vec![Field::new(
            "event_id",
            DataType::FixedSizeBinary(32),
            false,
        )]));
        let contract = SchemaContract::try_new(
            "provider-contract:test:activation-event",
            TableReference::full("codefabric", "_storage", "activation_event"),
            Arc::clone(&schema),
            schema,
            vec![FieldIndexMapping::direct(0, 0)],
        )
        .unwrap();
        let session = SessionStateBuilder::new().with_default_features().build();
        let binding = SealedActivationControlBinding::try_from_session_and_contract(
            &session,
            physical_binding_id,
            &contract,
        )
        .unwrap();
        binding
            .revalidate(&session, physical_binding_id, &contract)
            .unwrap();

        let substituted = SealedActivationControlBinding::try_from_session_and_contract(
            &session,
            "binding.other",
            &contract,
        )
        .unwrap();
        assert_ne!(binding.fingerprint(), substituted.fingerprint());

        let other_session = SessionStateBuilder::new().with_default_features().build();
        assert!(matches!(
            binding.revalidate(&other_session, physical_binding_id, &contract),
            Err(ActivationControlBindingError::ObservationMismatch)
        ));
    }

    #[test]
    fn empty_history_derives_empty_head() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let chain = ActivationChain::derive(workspace, []).unwrap();
        assert_eq!(chain.current_head(), ExpectedHead::Empty);
        assert!(chain.head_event().is_none());
    }
}
