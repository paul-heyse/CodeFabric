//! Exact activation and query primitives for one programmatic fabric workspace.
//!
//! The workspace record is administrative identity only; it cannot synthesize providers,
//! transformations, policies, or a query catalog. The target release owns construction. This
//! module retains only the exact activation-selected runtime, epoch query authority, command-port
//! context, and receipt/reconciliation capabilities that the production kernel and WP32 recovery
//! path consume.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroU64;
use std::sync::{Arc, RwLock};

use super::activation::{ActivationEventId, FabricEpochPins};
use super::activation_control_delta::DeltaActivationRuntimeAuthority;
use super::activation_transaction::ActivationReconciliationReceiptCache;
use super::admission::{AdmissionError, FabricAdmissionRuntime};
use super::arrow_result_resource::ArrowResultResourceLimits;
use super::child_session::resource_governance::{EpochResourceCoordinator, EpochResourceError};
use super::command::{
    ApplicationReleaseRef, EpochId, InputReleaseRef, ProgramReleaseRef, ProviderReleaseRef,
    SourceAuthorityRef, WorkspaceId, WriterFence,
};
use super::command_runtime_manager::{
    WorkspaceFabricCommandRuntimeFactoryError, WorkspaceFabricCommandRuntimeParts,
};
use super::programmatic_delta_runtime::ProgrammaticDeltaRuntime;
use super::programmatic_epoch::ProgrammaticFabricEpoch;
use super::published_arrow_result::PublishedArrowResultRegistry;
use super::relational_query_runtime::{RelationalQueryAuthorization, RelationalQueryRuntime};
use super::request_owned_relation::RequestOwnedRelationLimits;
use crate::identity::{IdentityDomain, IdentityError, encode_public_id};
use crate::relational_semantic_query::{
    EpochBoundSemanticExecutionCatalog, EpochBoundSemanticIngressCatalog, ProducerClosureProof,
    ProducerFamilyDisposition, SemanticQueryAuthority, SemanticQueryClass,
};

/// Stable identity of the retained exact workspace runtime shape.
pub const PROGRAMMATIC_WORKSPACE_FACTORY_ID: &str =
    "codefabric.programmatic-workspace.datafusion55.delta-exact.v1";

/// Exact release inputs which cannot be inferred from a workspace record or compiled features.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammaticWorkspaceReleasePins {
    input_release: InputReleaseRef,
    program_release: ProgramReleaseRef,
    provider_release: ProviderReleaseRef,
    application_release: ApplicationReleaseRef,
    source_authority: SourceAuthorityRef,
}

impl ProgrammaticWorkspaceReleasePins {
    /// Construct a complete non-sentinel release vector.
    pub fn try_new(
        input_release: InputReleaseRef,
        program_release: ProgramReleaseRef,
        provider_release: ProviderReleaseRef,
        application_release: ApplicationReleaseRef,
        source_authority: SourceAuthorityRef,
    ) -> Result<Self, ProgrammaticWorkspaceCompositionError> {
        let pins = Self {
            input_release,
            program_release,
            provider_release,
            application_release,
            source_authority,
        };
        pins.validate()?;
        Ok(pins)
    }

    fn validate(self) -> Result<(), ProgrammaticWorkspaceCompositionError> {
        for (kind, pin) in [
            ("input", self.input_release.as_bytes()),
            ("program", self.program_release.as_bytes()),
            ("provider", self.provider_release.as_bytes()),
            ("application", self.application_release.as_bytes()),
            ("source authority", self.source_authority.as_bytes()),
        ] {
            if all_zero(pin) {
                return Err(ProgrammaticWorkspaceCompositionError::MissingReleasePin(
                    kind,
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn input_release(self) -> InputReleaseRef {
        self.input_release
    }

    #[must_use]
    pub const fn program_release(self) -> ProgramReleaseRef {
        self.program_release
    }

    #[must_use]
    pub const fn provider_release(self) -> ProviderReleaseRef {
        self.provider_release
    }

    #[must_use]
    pub const fn application_release(self) -> ApplicationReleaseRef {
        self.application_release
    }

    #[must_use]
    pub const fn source_authority(self) -> SourceAuthorityRef {
        self.source_authority
    }
}

/// One exact Delta component named in the startup observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticTableVersionObservation {
    pub relation_id: Arc<str>,
    pub canonical_root: Arc<str>,
    pub version: u64,
}

/// Structured startup observation. It reports composition but never authorizes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticWorkspaceStartupObservation {
    pub factory_id: &'static str,
    pub workspace_id: WorkspaceId,
    pub epoch_id: EpochId,
    pub activation_event_id: ActivationEventId,
    pub active_fence: WriterFence,
    pub source_generation: u64,
    pub provider_set_pin: [u8; 32],
    pub overlay_segment_set_pin: [u8; 32],
    pub policy_set_pin: [u8; 32],
    pub proof_receipt_pin: [u8; 32],
    pub activation_control_root: Arc<str>,
    pub activation_control_version: u64,
    pub releases: ProgrammaticWorkspaceReleasePins,
    pub program_catalog_pin: [u8; 32],
    pub execution_catalog_pin: [u8; 32],
    pub program_release_pin: [u8; 32],
    pub producer_closure_proof_pin: [u8; 32],
    pub request_owned_relation_limits_pin: [u8; 32],
    pub resource_policy_pin: [u8; 32],
    pub runtime_configuration: Arc<str>,
    pub schema_authority: Arc<str>,
    pub relation_count: usize,
    pub table_versions: Arc<[ProgrammaticTableVersionObservation]>,
}

/// Query authority which is valid for exactly one workspace and one immutable epoch.
pub struct WorkspaceEpochQueryAuthority {
    workspace_id: WorkspaceId,
    activation_pins: FabricEpochPins,
    epoch: Arc<ProgrammaticFabricEpoch>,
    resources: Arc<EpochResourceCoordinator>,
    ingress_catalog: Arc<EpochBoundSemanticIngressCatalog>,
    execution_catalog: Arc<EpochBoundSemanticExecutionCatalog>,
    producer_closure: Arc<ProducerClosureProof>,
    authorization: RelationalQueryAuthorization,
    request_owned_relation_limits: RequestOwnedRelationLimits,
    result_limits: ArrowResultResourceLimits,
    result_lease_millis: NonZeroU64,
}

impl fmt::Debug for WorkspaceEpochQueryAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceEpochQueryAuthority")
            .field("workspace_id", &self.workspace_id)
            .field("epoch_id", self.epoch.identity())
            .field("program_catalog_pin", &"REDACTED_IDENTITY")
            .field("execution_catalog_pin", &"REDACTED_IDENTITY")
            .field("program_release_pin", &"REDACTED_IDENTITY")
            .field("producer_closure_proof_pin", &"REDACTED_IDENTITY")
            .finish_non_exhaustive()
    }
}

impl WorkspaceEpochQueryAuthority {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        workspace_id: WorkspaceId,
        activation_pins: FabricEpochPins,
        epoch: Arc<ProgrammaticFabricEpoch>,
        resources: Arc<EpochResourceCoordinator>,
        ingress_catalog: Arc<EpochBoundSemanticIngressCatalog>,
        execution_catalog: Arc<EpochBoundSemanticExecutionCatalog>,
        producer_closure: Arc<ProducerClosureProof>,
        authorization: RelationalQueryAuthorization,
        request_owned_relation_limits: RequestOwnedRelationLimits,
        result_limits: ArrowResultResourceLimits,
        result_lease_millis: u64,
    ) -> Result<Self, ProgrammaticWorkspaceCompositionError> {
        if activation_pins.epoch != *epoch.identity() {
            return Err(
                ProgrammaticWorkspaceCompositionError::SelectedEpochMismatch {
                    selected: activation_pins.epoch,
                    supplied: *epoch.identity(),
                },
            );
        }
        if activation_pins.table_versions != epoch.observation_publication().table_version_set_ref()
        {
            return Err(ProgrammaticWorkspaceCompositionError::SelectedTableVersionsMismatch);
        }
        if activation_pins.resource_envelope.as_bytes() != resources.resource_policy() {
            return Err(ProgrammaticWorkspaceCompositionError::ResourcePolicyMismatch);
        }
        if *epoch.identity() != resources.epoch_id() {
            return Err(
                ProgrammaticWorkspaceCompositionError::QueryResourceEpochMismatch {
                    epoch: *epoch.identity(),
                    resource: resources.epoch_id(),
                },
            );
        }
        validate_semantic_authority(&ingress_catalog, &execution_catalog, &producer_closure)?;
        if authorization.query_policy() != &ingress_catalog.policy_pin {
            return Err(ProgrammaticWorkspaceCompositionError::QueryPolicyMismatch);
        }
        if authorization.resource_policy() != resources.resource_policy() {
            return Err(ProgrammaticWorkspaceCompositionError::QueryResourcePolicyMismatch);
        }
        let expected_fabric_epoch_pin = programmatic_fabric_epoch_authority_pin(&epoch);
        if ingress_catalog.fabric_epoch_pin != expected_fabric_epoch_pin {
            return Err(
                ProgrammaticWorkspaceCompositionError::FabricEpochAuthorityPinMismatch {
                    expected: expected_fabric_epoch_pin,
                    supplied: ingress_catalog.fabric_epoch_pin,
                },
            );
        }
        let result_lease_millis = NonZeroU64::new(result_lease_millis)
            .ok_or(ProgrammaticWorkspaceCompositionError::ZeroResultLease)?;
        Ok(Self {
            workspace_id,
            activation_pins,
            epoch,
            resources,
            ingress_catalog,
            execution_catalog,
            producer_closure,
            authorization,
            request_owned_relation_limits,
            result_limits,
            result_lease_millis,
        })
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn activation_pins(&self) -> FabricEpochPins {
        self.activation_pins
    }

    #[must_use]
    pub fn epoch_id(&self) -> EpochId {
        *self.epoch.identity()
    }

    #[must_use]
    pub const fn epoch(&self) -> &Arc<ProgrammaticFabricEpoch> {
        &self.epoch
    }

    #[must_use]
    pub const fn resources(&self) -> &Arc<EpochResourceCoordinator> {
        &self.resources
    }

    #[must_use]
    pub const fn ingress_catalog(&self) -> &Arc<EpochBoundSemanticIngressCatalog> {
        &self.ingress_catalog
    }

    #[must_use]
    pub const fn execution_catalog(&self) -> &Arc<EpochBoundSemanticExecutionCatalog> {
        &self.execution_catalog
    }

    #[must_use]
    pub const fn producer_closure(&self) -> &Arc<ProducerClosureProof> {
        &self.producer_closure
    }

    #[must_use]
    pub const fn authorization(&self) -> &RelationalQueryAuthorization {
        &self.authorization
    }

    #[must_use]
    pub const fn request_owned_relation_limits(&self) -> RequestOwnedRelationLimits {
        self.request_owned_relation_limits
    }

    #[must_use]
    pub const fn result_limits(&self) -> ArrowResultResourceLimits {
        self.result_limits
    }

    #[must_use]
    pub const fn result_lease_millis(&self) -> u64 {
        self.result_lease_millis.get()
    }

    fn exact_identity_matches(&self, other: &Self) -> bool {
        self.workspace_id == other.workspace_id
            && self.activation_pins == other.activation_pins
            && self.epoch_id() == other.epoch_id()
            && programmatic_fabric_epoch_authority_pin(&self.epoch)
                == programmatic_fabric_epoch_authority_pin(&other.epoch)
            && self.resources.epoch_id() == other.resources.epoch_id()
            && self.resources.resource_policy() == other.resources.resource_policy()
            && self.ingress_catalog.fabric_epoch_pin == other.ingress_catalog.fabric_epoch_pin
            && self.ingress_catalog.program_catalog_pin == other.ingress_catalog.program_catalog_pin
            && self.ingress_catalog.source_pin == other.ingress_catalog.source_pin
            && self.ingress_catalog.policy_pin == other.ingress_catalog.policy_pin
            && self.ingress_catalog.producer_closure_proof_pin
                == other.ingress_catalog.producer_closure_proof_pin
            && self.ingress_catalog.limits_pin == other.ingress_catalog.limits_pin
            && self.execution_catalog.fabric_epoch_pin == other.execution_catalog.fabric_epoch_pin
            && self.execution_catalog.program_catalog_pin
                == other.execution_catalog.program_catalog_pin
            && self.execution_catalog.source_pin == other.execution_catalog.source_pin
            && self.execution_catalog.policy_pin == other.execution_catalog.policy_pin
            && self.execution_catalog.producer_closure_proof_pin
                == other.execution_catalog.producer_closure_proof_pin
            && self.execution_catalog.execution_catalog_pin
                == other.execution_catalog.execution_catalog_pin
            && self.execution_catalog.program_release_pin
                == other.execution_catalog.program_release_pin
            && self.execution_catalog.authority == other.execution_catalog.authority
            && self.execution_catalog.semantic_class == other.execution_catalog.semantic_class
            && self.producer_closure.proof_pin == other.producer_closure.proof_pin
            && self.producer_closure.application_authority_id
                == other.producer_closure.application_authority_id
            && self.authorization.access_scope() == other.authorization.access_scope()
            && self.authorization.query_policy() == other.authorization.query_policy()
            && self.authorization.resource_policy() == other.authorization.resource_policy()
            && self.authorization.max_output_rows() == other.authorization.max_output_rows()
            && self
                .authorization
                .table_relations()
                .eq(other.authorization.table_relations())
            && self.request_owned_relation_limits == other.request_owned_relation_limits
            && self.result_limits == other.result_limits
            && self.result_lease_millis == other.result_lease_millis
    }
}

/// Workspace-local registry resolving an already-admitted epoch to its exact query authority.
///
/// Activation may install a successor before opening it. Old entries remain available while an
/// admitted query or published result retains that immutable epoch.
pub struct WorkspaceEpochQueryAuthorityRegistry {
    workspace_id: WorkspaceId,
    by_epoch: RwLock<BTreeMap<EpochId, Arc<WorkspaceEpochQueryAuthority>>>,
}

impl fmt::Debug for WorkspaceEpochQueryAuthorityRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.by_epoch.read().map_or(0, |entries| entries.len());
        formatter
            .debug_struct("WorkspaceEpochQueryAuthorityRegistry")
            .field("workspace_id", &self.workspace_id)
            .field("epoch_count", &count)
            .finish_non_exhaustive()
    }
}

impl WorkspaceEpochQueryAuthorityRegistry {
    #[cfg(test)]
    fn with_initial(authority: Arc<WorkspaceEpochQueryAuthority>) -> Self {
        let workspace_id = authority.workspace_id();
        Self {
            workspace_id,
            by_epoch: RwLock::new(BTreeMap::from([(authority.epoch_id(), authority)])),
        }
    }

    /// Install one complete successor authority before activation can expose its epoch.
    pub fn install(
        &self,
        authority: Arc<WorkspaceEpochQueryAuthority>,
    ) -> Result<(), WorkspaceEpochQueryAuthorityRegistryError> {
        if authority.workspace_id() != self.workspace_id {
            return Err(WorkspaceEpochQueryAuthorityRegistryError::WorkspaceMismatch);
        }
        let epoch_id = authority.epoch_id();
        let mut entries = self
            .by_epoch
            .write()
            .map_err(|_| WorkspaceEpochQueryAuthorityRegistryError::Poisoned)?;
        if entries.contains_key(&epoch_id) {
            return Err(WorkspaceEpochQueryAuthorityRegistryError::DuplicateEpoch(
                epoch_id,
            ));
        }
        entries.insert(epoch_id, authority);
        Ok(())
    }

    /// Install one successor, or accept an already-installed authority only when every stable
    /// workspace/epoch/resource/catalog/policy identity is equal.
    pub fn install_or_validate(
        &self,
        authority: Arc<WorkspaceEpochQueryAuthority>,
    ) -> Result<(), WorkspaceEpochQueryAuthorityRegistryError> {
        if authority.workspace_id() != self.workspace_id {
            return Err(WorkspaceEpochQueryAuthorityRegistryError::WorkspaceMismatch);
        }
        let epoch_id = authority.epoch_id();
        let mut entries = self
            .by_epoch
            .write()
            .map_err(|_| WorkspaceEpochQueryAuthorityRegistryError::Poisoned)?;
        match entries.get(&epoch_id) {
            Some(existing) if existing.exact_identity_matches(&authority) => Ok(()),
            Some(_) => Err(WorkspaceEpochQueryAuthorityRegistryError::AuthorityMismatch(epoch_id)),
            None => {
                entries.insert(epoch_id, authority);
                Ok(())
            }
        }
    }

    /// Resolve only the exact epoch ID already pinned by admission.
    pub fn resolve(
        &self,
        epoch_id: EpochId,
    ) -> Result<Arc<WorkspaceEpochQueryAuthority>, WorkspaceEpochQueryAuthorityRegistryError> {
        self.by_epoch
            .read()
            .map_err(|_| WorkspaceEpochQueryAuthorityRegistryError::Poisoned)?
            .get(&epoch_id)
            .cloned()
            .ok_or(WorkspaceEpochQueryAuthorityRegistryError::UnknownEpoch(
                epoch_id,
            ))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_epoch.read().map_or(0, |entries| entries.len())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum WorkspaceEpochQueryAuthorityRegistryError {
    #[error("query authority belongs to another workspace")]
    WorkspaceMismatch,
    #[error("query authority already exists for epoch {0:?}")]
    DuplicateEpoch(EpochId),
    #[error("query authority for epoch {0:?} differs from the already-installed authority")]
    AuthorityMismatch(EpochId),
    #[error("no query authority exists for admitted epoch {0:?}")]
    UnknownEpoch(EpochId),
    #[error("query authority registry lock is poisoned")]
    Poisoned,
}

/// Exact daemon-owned capabilities available when constructing one workspace command router.
///
/// This value is created only after activation selection has been reconstructed, the immutable
/// epoch has been reopened, query admission and resource governance have been installed, and the
/// exact epoch query authority is registered. A command effect therefore cannot capture a
/// parallel admission gate or query-authority registry during pre-composition setup.
#[derive(Clone)]
pub struct ProgrammaticCommandRuntimeContext {
    workspace_id: WorkspaceId,
    admission: Arc<FabricAdmissionRuntime>,
    resources: Arc<EpochResourceCoordinator>,
    published_results: Arc<PublishedArrowResultRegistry>,
    query_authorities: Arc<WorkspaceEpochQueryAuthorityRegistry>,
    query_runtime: Arc<RelationalQueryRuntime>,
    delta_runtime: Arc<ProgrammaticDeltaRuntime>,
    activation_authority: Arc<DeltaActivationRuntimeAuthority>,
    receipt_cache: Arc<ActivationReconciliationReceiptCache>,
}

impl ProgrammaticCommandRuntimeContext {
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn admission(&self) -> &Arc<FabricAdmissionRuntime> {
        &self.admission
    }

    #[must_use]
    pub const fn resources(&self) -> &Arc<EpochResourceCoordinator> {
        &self.resources
    }

    #[must_use]
    pub const fn published_results(&self) -> &Arc<PublishedArrowResultRegistry> {
        &self.published_results
    }

    #[must_use]
    pub const fn query_authorities(&self) -> &Arc<WorkspaceEpochQueryAuthorityRegistry> {
        &self.query_authorities
    }

    #[must_use]
    pub const fn query_runtime(&self) -> &Arc<RelationalQueryRuntime> {
        &self.query_runtime
    }

    #[must_use]
    pub const fn delta_runtime(&self) -> &Arc<ProgrammaticDeltaRuntime> {
        &self.delta_runtime
    }

    #[must_use]
    pub const fn activation_authority(&self) -> &Arc<DeltaActivationRuntimeAuthority> {
        &self.activation_authority
    }

    #[must_use]
    pub const fn receipt_cache(&self) -> &Arc<ActivationReconciliationReceiptCache> {
        &self.receipt_cache
    }
}

/// Post-authority production constructor for one complete command runtime dependency closure.
pub trait ProgrammaticCommandRuntimePartsFactory: Send + Sync + 'static {
    /// Build the exhaustive command-effect router against the exact installed workspace objects.
    ///
    /// # Errors
    ///
    /// Returns a concrete port-composition failure without opening command ingress.
    fn build(
        &self,
        context: ProgrammaticCommandRuntimeContext,
    ) -> Result<WorkspaceFabricCommandRuntimeParts, WorkspaceFabricCommandRuntimeFactoryError>;
}

/// Fully composed workspace retained by the daemon after atomic registration.
pub struct ProgrammaticWorkspaceRuntime {
    workspace_id: WorkspaceId,
    admission: Arc<FabricAdmissionRuntime>,
    published_results: Arc<PublishedArrowResultRegistry>,
    query_authorities: Arc<WorkspaceEpochQueryAuthorityRegistry>,
    query_runtime: Arc<RelationalQueryRuntime>,
    delta_runtime: Arc<ProgrammaticDeltaRuntime>,
    activation_authority: Arc<DeltaActivationRuntimeAuthority>,
    receipt_cache: Arc<ActivationReconciliationReceiptCache>,
    command_runtime: WorkspaceFabricCommandRuntimeParts,
    startup: ProgrammaticWorkspaceStartupObservation,
}

impl fmt::Debug for ProgrammaticWorkspaceRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgrammaticWorkspaceRuntime")
            .field("workspace_id", &self.workspace_id)
            .field("active_head", &self.admission.active_head())
            .field("query_authorities", &self.query_authorities)
            .finish_non_exhaustive()
    }
}

impl ProgrammaticWorkspaceRuntime {
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    /// Released public workspace identity corresponding to the internal typed identity.
    ///
    /// # Errors
    ///
    /// Returns the strict public-identity encoder error rather than emitting an ad-hoc string.
    pub fn public_workspace_id(&self) -> Result<String, IdentityError> {
        encode_public_id(
            IdentityDomain::Workspace,
            None,
            *self.workspace_id.as_bytes(),
        )
    }

    #[must_use]
    pub const fn admission(&self) -> &Arc<FabricAdmissionRuntime> {
        &self.admission
    }

    #[must_use]
    pub const fn published_results(&self) -> &Arc<PublishedArrowResultRegistry> {
        &self.published_results
    }

    #[must_use]
    pub const fn query_authorities(&self) -> &Arc<WorkspaceEpochQueryAuthorityRegistry> {
        &self.query_authorities
    }

    #[must_use]
    pub const fn query_runtime(&self) -> &Arc<RelationalQueryRuntime> {
        &self.query_runtime
    }

    #[must_use]
    pub const fn delta_runtime(&self) -> &Arc<ProgrammaticDeltaRuntime> {
        &self.delta_runtime
    }

    #[must_use]
    pub const fn activation_authority(&self) -> &Arc<DeltaActivationRuntimeAuthority> {
        &self.activation_authority
    }

    #[must_use]
    pub const fn receipt_cache(&self) -> &Arc<ActivationReconciliationReceiptCache> {
        &self.receipt_cache
    }

    #[must_use]
    pub fn command_runtime_parts(&self) -> WorkspaceFabricCommandRuntimeParts {
        self.command_runtime.clone()
    }

    #[must_use]
    pub const fn startup_observation(&self) -> &ProgrammaticWorkspaceStartupObservation {
        &self.startup
    }
}

impl Drop for ProgrammaticWorkspaceRuntime {
    fn drop(&mut self) {
        // This is a synchronous safety net for partial construction and owner teardown. The
        // daemon's ordered async shutdown still closes admission before joining command workers.
        let _ = self.admission.close_for_shutdown();
    }
}

/// Derive the only semantic epoch pin accepted by the workspace query authority.
///
/// The pin is a projection of the sealed epoch itself: its typed epoch ID, complete exact Delta
/// version vector, application-owned schema authority, and DataFusion runtime configuration. A
/// caller-supplied label cannot stand in for any of those authorities.
#[must_use]
pub fn programmatic_fabric_epoch_authority_pin(epoch: &ProgrammaticFabricEpoch) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    frame_digest(
        &mut hasher,
        b"codefabric.programmatic-workspace.fabric-epoch-authority.v1",
    );
    frame_digest(&mut hasher, epoch.identity().as_bytes());
    let publication = epoch.observation_publication();
    frame_digest(&mut hasher, publication.table_version_set_ref().as_bytes());
    frame_digest(
        &mut hasher,
        &(publication.table_version_set().len() as u128).to_be_bytes(),
    );
    for (relation_id, pin) in publication.table_versions() {
        frame_digest(&mut hasher, relation_id.as_bytes());
        frame_digest(&mut hasher, pin.canonical_root().as_str().as_bytes());
        frame_digest(&mut hasher, &pin.version().to_be_bytes());
    }
    frame_digest(&mut hasher, epoch.schema_authority_id().as_bytes());
    frame_digest(
        &mut hasher,
        epoch.runtime_configuration_identity().as_bytes(),
    );
    *hasher.finalize().as_bytes()
}

/// Canonical identity of the explicit request-owned relation allocation envelope.
#[must_use]
pub fn request_owned_relation_limits_pin(limits: RequestOwnedRelationLimits) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    frame_digest(
        &mut hasher,
        b"codefabric.programmatic-workspace.request-owned-relation-limits.v1",
    );
    for value in [
        limits.max_relations(),
        limits.max_rows_per_relation(),
        limits.max_fields_per_relation(),
        limits.max_cells_per_relation(),
        limits.max_total_rows(),
        limits.max_total_cells(),
        limits.max_total_text_bytes(),
    ] {
        frame_digest(&mut hasher, &(value as u128).to_be_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn frame_digest(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn validate_semantic_authority(
    ingress: &EpochBoundSemanticIngressCatalog,
    execution: &EpochBoundSemanticExecutionCatalog,
    producer_closure: &ProducerClosureProof,
) -> Result<(), ProgrammaticWorkspaceCompositionError> {
    for (kind, pin) in [
        ("ingress fabric epoch", ingress.fabric_epoch_pin),
        ("ingress program catalog", ingress.program_catalog_pin),
        ("ingress source", ingress.source_pin),
        ("ingress policy", ingress.policy_pin),
        (
            "ingress producer closure",
            ingress.producer_closure_proof_pin,
        ),
        ("ingress limits", ingress.limits_pin),
        ("execution fabric epoch", execution.fabric_epoch_pin),
        ("execution program catalog", execution.program_catalog_pin),
        ("execution source", execution.source_pin),
        ("execution policy", execution.policy_pin),
        (
            "execution producer closure",
            execution.producer_closure_proof_pin,
        ),
        ("execution catalog", execution.execution_catalog_pin),
        ("execution program release", execution.program_release_pin),
        ("producer closure proof", producer_closure.proof_pin),
    ] {
        require_semantic_pin(kind, pin)?;
    }

    for (kind, ingress_pin, execution_pin) in [
        (
            "fabric epoch",
            ingress.fabric_epoch_pin,
            execution.fabric_epoch_pin,
        ),
        (
            "program catalog",
            ingress.program_catalog_pin,
            execution.program_catalog_pin,
        ),
        ("source", ingress.source_pin, execution.source_pin),
        ("policy", ingress.policy_pin, execution.policy_pin),
        (
            "producer closure",
            ingress.producer_closure_proof_pin,
            execution.producer_closure_proof_pin,
        ),
    ] {
        if ingress_pin != execution_pin {
            return Err(ProgrammaticWorkspaceCompositionError::SemanticAuthorityPinMismatch(kind));
        }
    }
    if ingress.producer_closure_proof_pin != producer_closure.proof_pin {
        return Err(
            ProgrammaticWorkspaceCompositionError::SemanticAuthorityPinMismatch(
                "installed producer closure proof",
            ),
        );
    }

    let application_authority = match &execution.authority {
        SemanticQueryAuthority::ApplicationOwned(authority) if !authority.trim().is_empty() => {
            authority
        }
        SemanticQueryAuthority::ApplicationOwned(_) => {
            return Err(ProgrammaticWorkspaceCompositionError::EmptyProgramAuthority);
        }
        SemanticQueryAuthority::ProviderNative(_) => {
            return Err(ProgrammaticWorkspaceCompositionError::ProviderNativeProgramAuthority);
        }
    };
    match &execution.semantic_class {
        SemanticQueryClass::Fact(class) if !class.trim().is_empty() => {}
        SemanticQueryClass::Fact(_) => {
            return Err(ProgrammaticWorkspaceCompositionError::EmptySemanticClass);
        }
        SemanticQueryClass::Judgment(_) => {
            return Err(ProgrammaticWorkspaceCompositionError::JudgmentSemanticClass);
        }
    }
    if producer_closure.application_authority_id.trim().is_empty() {
        return Err(ProgrammaticWorkspaceCompositionError::EmptyProducerAuthority);
    }
    if producer_closure.application_authority_id.as_ref() != application_authority.as_ref() {
        return Err(ProgrammaticWorkspaceCompositionError::ProducerAuthorityMismatch);
    }

    let mut ingress_programs = BTreeMap::<&str, [u8; 32]>::new();
    for program in &ingress.program_bindings {
        require_nonempty("ingress program binding", &program.program_binding_id)?;
        require_semantic_pin("program binding", program.program_binding_pin)?;
        require_semantic_pin("ingress execution program", program.execution_program_pin)?;
        if ingress_programs
            .insert(&program.program_binding_id, program.execution_program_pin)
            .is_some()
        {
            return Err(
                ProgrammaticWorkspaceCompositionError::DuplicateProgramBinding(
                    program.program_binding_id.to_string(),
                ),
            );
        }
    }
    if ingress_programs.is_empty() {
        return Err(ProgrammaticWorkspaceCompositionError::EmptyProgramCatalog);
    }

    let mut execution_programs = BTreeMap::<&str, [u8; 32]>::new();
    for program in &execution.programs {
        require_nonempty("execution program binding", &program.program_binding_id)?;
        require_semantic_pin("execution program", program.execution_program_pin)?;
        if execution_programs
            .insert(&program.program_binding_id, program.execution_program_pin)
            .is_some()
        {
            return Err(
                ProgrammaticWorkspaceCompositionError::DuplicateProgramBinding(
                    program.program_binding_id.to_string(),
                ),
            );
        }
    }
    if ingress_programs != execution_programs {
        return Err(ProgrammaticWorkspaceCompositionError::ProgramCatalogCoverageMismatch);
    }

    for (program_id, pin) in execution
        .operators
        .iter()
        .map(|row| (row.program_binding_id.as_ref(), row.execution_program_pin))
        .chain(
            execution
                .consumer_slots
                .iter()
                .map(|row| (row.program_binding_id.as_ref(), row.execution_program_pin)),
        )
        .chain(
            execution
                .selections
                .iter()
                .map(|row| (row.program_binding_id.as_ref(), row.execution_program_pin)),
        )
        .chain(
            execution
                .returns
                .iter()
                .map(|row| (row.program_binding_id.as_ref(), row.execution_program_pin)),
        )
        .chain(
            execution
                .required_fact_families
                .iter()
                .map(|row| (row.program_binding_id.as_ref(), row.execution_program_pin)),
        )
        .chain(
            execution
                .request_inputs
                .iter()
                .map(|row| (row.program_binding_id.as_ref(), row.execution_program_pin)),
        )
    {
        require_semantic_pin("execution program reference", pin)?;
        if execution_programs.get(program_id) != Some(&pin) {
            return Err(
                ProgrammaticWorkspaceCompositionError::ExecutionProgramReferenceMismatch(
                    program_id.to_owned(),
                ),
            );
        }
    }
    for row in &execution.returns {
        require_semantic_pin("return realization", row.realization_pin)?;
    }
    for row in &execution.request_inputs {
        require_semantic_pin("request-input handoff", row.handoff_pin)?;
    }
    for row in &execution.scopes {
        require_semantic_pin("scope handoff", row.handoff_pin)?;
    }

    let mut closure_families = BTreeSet::new();
    for row in &producer_closure.families {
        require_nonempty("producer family", &row.family_id)?;
        if !closure_families.insert(row.family_id.as_ref()) {
            return Err(
                ProgrammaticWorkspaceCompositionError::DuplicateProducerFamily(
                    row.family_id.to_string(),
                ),
            );
        }
        match &row.disposition {
            ProducerFamilyDisposition::RuntimeProducer(runtime) => {
                if runtime.authority_id.as_ref() != application_authority.as_ref() {
                    return Err(ProgrammaticWorkspaceCompositionError::ProducerAuthorityMismatch);
                }
                for (kind, pin) in [
                    ("producer input", runtime.input_pin),
                    ("producer invalidation", runtime.invalidation_pin),
                    ("producer materialization", runtime.materialization_pin),
                    ("producer completeness", runtime.completeness_proof_pin),
                    ("producer proof", runtime.producer_proof_pin),
                ] {
                    require_semantic_pin(kind, pin)?;
                }
            }
            ProducerFamilyDisposition::UnsupportedRemainder(remainder) => {
                if remainder.authority_id.as_ref() != application_authority.as_ref() {
                    return Err(ProgrammaticWorkspaceCompositionError::ProducerAuthorityMismatch);
                }
                require_semantic_pin("unsupported producer remainder", remainder.proof_pin)?;
            }
        }
    }
    let mut required_families = BTreeSet::new();
    for row in &execution.required_fact_families {
        require_nonempty("required producer family", &row.family_id)?;
        required_families.insert(row.family_id.as_ref());
    }
    if !required_families.is_subset(&closure_families) {
        return Err(ProgrammaticWorkspaceCompositionError::ProducerClosureCoverageMismatch);
    }
    Ok(())
}

fn require_semantic_pin(
    kind: &'static str,
    pin: [u8; 32],
) -> Result<(), ProgrammaticWorkspaceCompositionError> {
    if all_zero(&pin) {
        Err(ProgrammaticWorkspaceCompositionError::MissingSemanticPin(
            kind,
        ))
    } else {
        Ok(())
    }
}

fn require_nonempty(
    kind: &'static str,
    value: &str,
) -> Result<(), ProgrammaticWorkspaceCompositionError> {
    if value.trim().is_empty() {
        Err(ProgrammaticWorkspaceCompositionError::EmptySemanticIdentity(kind))
    } else {
        Ok(())
    }
}

const fn all_zero<const N: usize>(value: &[u8; N]) -> bool {
    let mut index = 0;
    while index < value.len() {
        if value[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

/// Fail-closed exact workspace-authority errors. No variant authorizes a fallback backend.
#[derive(Debug, thiserror::Error)]
pub enum ProgrammaticWorkspaceCompositionError {
    #[error("required {0} release pin is absent")]
    MissingReleasePin(&'static str),
    #[error("selected epoch {selected:?} differs from supplied builder {supplied:?}")]
    SelectedEpochMismatch {
        selected: EpochId,
        supplied: EpochId,
    },
    #[error("selected activation table-version reference differs from the supplied exact vector")]
    SelectedTableVersionsMismatch,
    #[error("selected activation resource envelope differs from the supplied policy")]
    ResourcePolicyMismatch,
    #[error("query authorization policy differs from the installed semantic catalog policy")]
    QueryPolicyMismatch,
    #[error("query authorization resource policy differs from the admitted epoch resource policy")]
    QueryResourcePolicyMismatch,
    #[error("query resource epoch {resource:?} differs from epoch authority {epoch:?}")]
    QueryResourceEpochMismatch { epoch: EpochId, resource: EpochId },
    #[error(
        "semantic fabric epoch authority pin differs from the sealed epoch: expected {expected:02x?}, supplied {supplied:02x?}"
    )]
    FabricEpochAuthorityPinMismatch {
        expected: [u8; 32],
        supplied: [u8; 32],
    },
    #[error("result lease duration must be non-zero")]
    ZeroResultLease,
    #[error("required semantic authority pin {0} is absent")]
    MissingSemanticPin(&'static str),
    #[error("semantic authority pin {0} differs between ingress and execution")]
    SemanticAuthorityPinMismatch(&'static str),
    #[error("semantic identity {0} is empty")]
    EmptySemanticIdentity(&'static str),
    #[error("semantic program catalog has no program bindings")]
    EmptyProgramCatalog,
    #[error("semantic program binding {0} is duplicated")]
    DuplicateProgramBinding(String),
    #[error("ingress and execution program catalogs do not enumerate the same exact programs")]
    ProgramCatalogCoverageMismatch,
    #[error("execution row references an absent or differently pinned program {0}")]
    ExecutionProgramReferenceMismatch(String),
    #[error("semantic program authority is empty")]
    EmptyProgramAuthority,
    #[error("provider-native program authority cannot select application semantics")]
    ProviderNativeProgramAuthority,
    #[error("semantic fact class is empty")]
    EmptySemanticClass,
    #[error("judgment semantics cannot be installed in the query factory")]
    JudgmentSemanticClass,
    #[error("producer-closure application authority is empty")]
    EmptyProducerAuthority,
    #[error("producer-closure authority differs from the application-owned execution authority")]
    ProducerAuthorityMismatch,
    #[error("producer-closure family {0} is duplicated")]
    DuplicateProducerFamily(String),
    #[error("producer closure does not cover every execution-catalog fact family")]
    ProducerClosureCoverageMismatch,
    #[error(transparent)]
    Resources(#[from] EpochResourceError),
    #[error(transparent)]
    Admission(#[from] AdmissionError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::activation::{
        ActivationAttempt, ActivationChain, ActivationCommit, ActivationEvent, ActivationOrdinal,
        ActivationReadbackRef, BackendCommitRef, CompatibilityClassRef, FabricEpochPins,
        OverlaySegmentSetRef, PolicySetRef,
    };
    use crate::fabric::activation_transaction::ActivationAdmissionPort;
    use crate::fabric::child_session::resource_governance::EpochResourcePolicy;
    use crate::fabric::child_session::{
        ChildRegistryAllowlist, ChildResourceLimits, ChildTableGrant,
    };
    use crate::fabric::command::{
        ActorId, AuthorizationRef, CommandIdentity, CommandOwnership, CommandPins, ExecutionOwner,
        FabricCommand, FabricCommandPayload, IdempotencyKey, InputReleaseRef, LeaseId, OperationId,
        OperationSelectionRef, PrincipalId, ProgramReleaseRef, ProofReceiptRef, ProviderSetRef,
        ResourceEnvelopeRef, RetentionPolicyRef, SourceGeneration, TransactionRef,
        WriterGeneration,
    };
    use crate::fabric::programmatic_activation_admission::{
        ExactProgrammaticSuccessorQueryAuthorityRecipe, ProgrammaticActivationAdmission,
    };
    use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
    use crate::fabric::programmatic_schema::ProgrammaticRelationId;
    use crate::identity::decode_public_id;
    use crate::relational_program::{FieldId, RelationId};
    use crate::relational_semantic_query::{
        EpochBoundExecutionProgramRow, EpochBoundProgramBindingRow, ReleasedSemanticForm,
    };

    const fn id16(seed: u8) -> [u8; 16] {
        [seed; 16]
    }

    const fn id32(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn release_pins(
        input: u8,
        program: u8,
        provider: u8,
        application: u8,
        source: u8,
    ) -> Result<ProgrammaticWorkspaceReleasePins, ProgrammaticWorkspaceCompositionError> {
        ProgrammaticWorkspaceReleasePins::try_new(
            InputReleaseRef::from_bytes(id32(input)),
            ProgramReleaseRef::from_bytes(id32(program)),
            crate::fabric::command::ProviderReleaseRef::from_bytes(id32(provider)),
            crate::fabric::command::ApplicationReleaseRef::from_bytes(id32(application)),
            crate::fabric::command::SourceAuthorityRef::from_bytes(id32(source)),
        )
    }

    fn child_resources() -> ChildResourceLimits {
        ChildResourceLimits::try_new(8 * 1024 * 1024, 32 * 1024 * 1024, 4, 2, 128, 1).unwrap()
    }

    fn resource_policy() -> EpochResourcePolicy {
        EpochResourcePolicy::try_new(
            child_resources(),
            super::super::child_session::resource_governance::test_lifecycle_work_class_policies(),
            4,
            1,
            8,
            30_000,
            1,
            2,
            8,
            64 * 1024 * 1024,
            60_000,
        )
        .unwrap()
    }

    fn catalogs(
        fabric_epoch_pin: [u8; 32],
    ) -> (
        Arc<EpochBoundSemanticIngressCatalog>,
        Arc<EpochBoundSemanticExecutionCatalog>,
    ) {
        let ingress = Arc::new(EpochBoundSemanticIngressCatalog {
            fabric_epoch_pin,
            program_catalog_pin: id32(1),
            source_pin: id32(2),
            policy_pin: id32(3),
            producer_closure_proof_pin: id32(4),
            limits_pin: id32(5),
            program_bindings: vec![EpochBoundProgramBindingRow {
                program_binding_id: Arc::from("program.test"),
                program_binding_pin: id32(6),
                compatibility_form: ReleasedSemanticForm::FindCodeEntities,
                output_role_id: Arc::from("role.test"),
                execution_program_pin: id32(7),
            }],
            consumer_slots: Vec::new(),
            selections: Vec::new(),
            returns: Vec::new(),
            scopes: Vec::new(),
            request_inputs: Vec::new(),
        });
        let execution = Arc::new(EpochBoundSemanticExecutionCatalog {
            fabric_epoch_pin,
            program_catalog_pin: id32(1),
            source_pin: id32(2),
            policy_pin: id32(3),
            producer_closure_proof_pin: id32(4),
            execution_catalog_pin: id32(8),
            program_release_pin: id32(9),
            authority: SemanticQueryAuthority::ApplicationOwned(Arc::from("query.application")),
            semantic_class: SemanticQueryClass::Fact(Arc::from("objective_fact")),
            programs: vec![EpochBoundExecutionProgramRow {
                program_binding_id: Arc::from("program.test"),
                execution_program_pin: id32(7),
                root_node_id: Arc::from("node.test"),
                output_relation_id: RelationId::new("result.test").unwrap(),
                output_fields: Vec::<FieldId>::new(),
            }],
            operators: Vec::new(),
            relation_schemas: Vec::new(),
            consumer_slots: Vec::new(),
            selections: Vec::new(),
            returns: Vec::new(),
            required_fact_families: Vec::new(),
            request_inputs: Vec::new(),
            scopes: Vec::new(),
        });
        (ingress, execution)
    }

    fn closure() -> Arc<ProducerClosureProof> {
        Arc::new(ProducerClosureProof {
            proof_pin: id32(4),
            application_authority_id: Arc::from("query.application"),
            families: Vec::new(),
        })
    }

    fn request_owned_relation_limits() -> RequestOwnedRelationLimits {
        RequestOwnedRelationLimits::try_new(4, 64, 16, 1_024, 128, 2_048, 64 * 1024).unwrap()
    }

    fn authorization() -> RelationalQueryAuthorization {
        RelationalQueryAuthorization::try_new(
            id32(4),
            id32(3),
            id32(6),
            vec![ChildTableGrant::try_new(ProgrammaticRelationId::new("facts.test")).unwrap()],
            child_resources(),
            1_024,
            ChildRegistryAllowlist::default(),
        )
        .unwrap()
    }

    fn result_limits() -> ArrowResultResourceLimits {
        ArrowResultResourceLimits::try_new(
            4,
            8,
            10_000,
            16,
            20_000,
            1 << 20,
            2 << 20,
            1 << 20,
            2 << 20,
            1 << 20,
            64 * 1024,
        )
        .unwrap()
    }

    fn query_authority_pins(epoch: &ProgrammaticFabricEpoch) -> FabricEpochPins {
        FabricEpochPins {
            epoch: *epoch.identity(),
            input_release: InputReleaseRef::from_bytes(id32(0x71)),
            program_release: ProgramReleaseRef::from_bytes(id32(0x72)),
            application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(id32(
                0x72,
            )),
            source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(id32(0x72)),
            provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(id32(0x72)),
            source_generation: SourceGeneration::new(1),
            provider_set: ProviderSetRef::from_bytes(id32(0x73)),
            table_versions: epoch.observation_publication().table_version_set_ref(),
            overlay_segments: OverlaySegmentSetRef::from_bytes(id32(0x74)),
            policy_set: PolicySetRef::from_bytes(id32(0x75)),
            resource_envelope: ResourceEnvelopeRef::from_bytes(id32(6)),
            proof_receipt: ProofReceiptRef::from_bytes(id32(0x76)),
        }
    }

    async fn query_authority(
        workspace_id: WorkspaceId,
        epoch_id: EpochId,
    ) -> Arc<WorkspaceEpochQueryAuthority> {
        let epoch = Arc::new(
            ProgrammaticFabricEpochBuilder::try_new(
                epoch_id,
                super::super::epoch_runtime::FabricEpochRuntimeConfig::default(),
            )
            .unwrap()
            .seal_for_test()
            .await
            .unwrap(),
        );
        let resources = Arc::new(
            EpochResourceCoordinator::try_new(epoch_id, id32(6), resource_policy()).unwrap(),
        );
        let fabric_epoch_pin = programmatic_fabric_epoch_authority_pin(&epoch);
        let (ingress_catalog, execution_catalog) = catalogs(fabric_epoch_pin);
        let pins = query_authority_pins(&epoch);
        Arc::new(
            WorkspaceEpochQueryAuthority::try_new(
                workspace_id,
                pins,
                epoch,
                resources,
                ingress_catalog,
                execution_catalog,
                closure(),
                authorization(),
                request_owned_relation_limits(),
                result_limits(),
                60_000,
            )
            .unwrap(),
        )
    }

    fn successor_recipe(
        authority: &WorkspaceEpochQueryAuthority,
        pins: FabricEpochPins,
    ) -> ExactProgrammaticSuccessorQueryAuthorityRecipe {
        ExactProgrammaticSuccessorQueryAuthorityRecipe::try_new(
            authority.workspace_id(),
            pins,
            programmatic_fabric_epoch_authority_pin(authority.epoch()),
            authority.resources().policy().clone(),
            authority.ingress_catalog().as_ref().clone(),
            authority.execution_catalog().as_ref().clone(),
            authority.producer_closure().as_ref().clone(),
            authority.authorization().clone(),
            authority.request_owned_relation_limits(),
            authority.result_limits(),
            authority.result_lease_millis(),
        )
        .expect("complete exact successor recipe")
    }

    fn activation_command_for_authority(
        authority: &WorkspaceEpochQueryAuthority,
        operation_seed: u8,
        fence_seed: u8,
        expected_head: super::super::command::ExpectedHead,
    ) -> FabricCommand {
        let epoch_id = authority.epoch_id();
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(id16(operation_seed)),
                idempotency_key: IdempotencyKey::from_bytes(id32(operation_seed)),
            },
            ownership: CommandOwnership {
                workspace_id: authority.workspace_id(),
                principal_id: PrincipalId::from_bytes(id16(0x31)),
                authorization: AuthorizationRef::from_bytes(id32(0x32)),
            },
            expected_head,
            writer_fence: WriterFence {
                lease_id: LeaseId::from_bytes(id16(fence_seed)),
                generation: WriterGeneration::new(1).expect("writer generation"),
            },
            pins: CommandPins {
                input_release: InputReleaseRef::from_bytes(id32(0x33)),
                program_release: ProgramReleaseRef::from_bytes(id32(0x34)),
                application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(
                    id32(0x34),
                ),
                source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(id32(
                    0x34,
                )),
                provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(id32(
                    0x34,
                )),
                source_generation: SourceGeneration::new(1),
                provider_set: ProviderSetRef::from_bytes(id32(0x35)),
            },
            resources: ResourceEnvelopeRef::from_bytes(*authority.resources().resource_policy()),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: epoch_id,
                proof_receipt: ProofReceiptRef::from_bytes(id32(0x36)),
            },
        }
    }

    fn activation_event_for_authority(
        authority: &WorkspaceEpochQueryAuthority,
        command: FabricCommand,
        event_seed: u8,
        predecessor_event_id: Option<super::super::activation::ActivationEventId>,
        ordinal: u64,
    ) -> ActivationEvent {
        ActivationEvent::try_from_attempt(
            super::super::activation::ActivationEventId::from_bytes(id32(event_seed)),
            ActivationAttempt::for_test(
                command,
                1,
                ExecutionOwner {
                    actor_id: ActorId::from_bytes(id16(0x37)),
                    fence: command.writer_fence,
                },
            ),
            predecessor_event_id,
            ActivationOrdinal::new(ordinal).expect("activation ordinal"),
            FabricEpochPins {
                epoch: authority.epoch_id(),
                input_release: command.pins.input_release,
                program_release: command.pins.program_release,
                application_release: command.pins.application_release,
                source_authority: command.pins.source_authority,
                provider_release: command.pins.provider_release,
                source_generation: command.pins.source_generation,
                provider_set: command.pins.provider_set,
                table_versions: authority
                    .epoch()
                    .observation_publication()
                    .table_version_set_ref(),
                overlay_segments: OverlaySegmentSetRef::from_bytes(id32(0x38)),
                policy_set: PolicySetRef::from_bytes(id32(0x39)),
                resource_envelope: command.resources,
                proof_receipt: ProofReceiptRef::from_bytes(id32(0x36)),
            },
            CompatibilityClassRef::from_bytes(id32(0x3a)),
            RetentionPolicyRef::from_bytes(id32(0x3b)),
            ActivationCommit {
                operation_selection: OperationSelectionRef::from_bytes(id32(
                    event_seed.wrapping_add(1),
                )),
                transaction: TransactionRef::from_bytes(id32(event_seed.wrapping_add(2))),
                backend_commit: BackendCommitRef::from_bytes(id32(event_seed.wrapping_add(3))),
                readback: ActivationReadbackRef::from_bytes(id32(event_seed.wrapping_add(4))),
            },
        )
        .expect("activation event")
    }

    #[test]
    fn release_pins_reject_every_missing_identity() {
        assert!(matches!(
            release_pins(0, 2, 3, 4, 5),
            Err(ProgrammaticWorkspaceCompositionError::MissingReleasePin(
                "input"
            ))
        ));
        assert!(matches!(
            release_pins(1, 0, 3, 4, 5),
            Err(ProgrammaticWorkspaceCompositionError::MissingReleasePin(
                "program"
            ))
        ));
        assert!(matches!(
            release_pins(1, 2, 0, 4, 5),
            Err(ProgrammaticWorkspaceCompositionError::MissingReleasePin(
                "provider"
            ))
        ));
        assert!(matches!(
            release_pins(1, 2, 3, 0, 5),
            Err(ProgrammaticWorkspaceCompositionError::MissingReleasePin(
                "application"
            ))
        ));
        assert!(matches!(
            release_pins(1, 2, 3, 4, 0),
            Err(ProgrammaticWorkspaceCompositionError::MissingReleasePin(
                "source authority"
            ))
        ));
        assert!(release_pins(1, 2, 3, 4, 5).is_ok());
    }

    #[test]
    fn source_authority_is_explicit_and_independent_of_release_vector() {
        let baseline = release_pins(1, 2, 3, 4, 5).unwrap();
        for changed in [
            release_pins(9, 2, 3, 4, 5).unwrap(),
            release_pins(1, 9, 3, 4, 5).unwrap(),
            release_pins(1, 2, 9, 4, 5).unwrap(),
            release_pins(1, 2, 3, 9, 5).unwrap(),
        ] {
            assert_eq!(changed.source_authority(), baseline.source_authority());
        }
        assert_ne!(
            release_pins(1, 2, 3, 4, 9).unwrap().source_authority(),
            baseline.source_authority()
        );
    }

    #[tokio::test]
    async fn epoch_query_registry_is_exact_and_workspace_scoped() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let first_epoch = EpochId::from_bytes(id16(10));
        let first = query_authority(workspace, first_epoch).await;
        assert_eq!(
            first.ingress_catalog().fabric_epoch_pin,
            programmatic_fabric_epoch_authority_pin(first.epoch())
        );
        assert_eq!(
            first.execution_catalog().fabric_epoch_pin,
            first.ingress_catalog().fabric_epoch_pin
        );
        assert_eq!(
            first.request_owned_relation_limits(),
            request_owned_relation_limits()
        );
        let registry = WorkspaceEpochQueryAuthorityRegistry::with_initial(Arc::clone(&first));
        assert!(Arc::ptr_eq(&registry.resolve(first_epoch).unwrap(), &first));
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.install(Arc::clone(&first)).unwrap_err(),
            WorkspaceEpochQueryAuthorityRegistryError::DuplicateEpoch(first_epoch)
        );

        let second_epoch = EpochId::from_bytes(id16(11));
        let foreign = query_authority(WorkspaceId::from_bytes(id16(2)), second_epoch).await;
        assert_eq!(
            registry.install(foreign).unwrap_err(),
            WorkspaceEpochQueryAuthorityRegistryError::WorkspaceMismatch
        );
        assert_eq!(
            registry.resolve(second_epoch).unwrap_err(),
            WorkspaceEpochQueryAuthorityRegistryError::UnknownEpoch(second_epoch)
        );
    }

    #[tokio::test]
    async fn activation_admission_installs_before_swap_and_rejects_substituted_authority() {
        let workspace = WorkspaceId::from_bytes(id16(0x51));
        let first_epoch = EpochId::from_bytes(id16(0x52));
        let successor_epoch = EpochId::from_bytes(id16(0x53));
        let first = query_authority(workspace, first_epoch).await;
        let successor = query_authority(workspace, successor_epoch).await;
        let first_command = activation_command_for_authority(
            &first,
            0x54,
            0x55,
            super::super::command::ExpectedHead::Empty,
        );
        let first_event = activation_event_for_authority(&first, first_command, 0x56, None, 1);
        let first_chain = ActivationChain::derive(workspace, [first_event]).expect("first chain");
        let successor_command = activation_command_for_authority(
            &successor,
            0x57,
            0x58,
            super::super::command::ExpectedHead::Epoch(first_epoch),
        );
        let successor_event = activation_event_for_authority(
            &successor,
            successor_command,
            0x59,
            Some(first_event.event_id()),
            2,
        );
        let selected_chain = ActivationChain::derive(workspace, [successor_event, first_event])
            .expect("selected chain");

        let runtime = Arc::new(
            FabricAdmissionRuntime::recover(&first_chain, |_| Some(Arc::clone(first.epoch())))
                .expect("initial admission"),
        );
        let registry = Arc::new(WorkspaceEpochQueryAuthorityRegistry::with_initial(
            Arc::clone(&first),
        ));
        let admission = ProgrammaticActivationAdmission::new(
            workspace,
            Arc::clone(&runtime),
            Arc::clone(&registry),
            Arc::new(successor_recipe(&successor, successor_event.pins())),
        );
        let barrier = admission
            .close_admission(
                successor_command.expected_head,
                successor_command.writer_fence,
            )
            .await
            .expect("close admission");
        admission
            .publish_selected_epoch(barrier, &selected_chain, Arc::clone(successor.epoch()))
            .await
            .expect("install and publish successor");
        let installed = registry
            .resolve(successor_epoch)
            .expect("installed authority");
        assert!(Arc::ptr_eq(installed.epoch(), successor.epoch()));
        assert_eq!(
            runtime.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );
        admission
            .reconcile_and_reopen(
                barrier,
                super::super::command::ExpectedHead::Epoch(successor_epoch),
            )
            .await
            .expect("reopen successor");
        assert!(Arc::ptr_eq(
            runtime.admit().expect("successor query lease").epoch(),
            successor.epoch()
        ));

        let failed_runtime = Arc::new(
            FabricAdmissionRuntime::recover(&first_chain, |_| Some(Arc::clone(first.epoch())))
                .expect("initial admission"),
        );
        let failed_registry = Arc::new(WorkspaceEpochQueryAuthorityRegistry::with_initial(
            Arc::clone(&first),
        ));
        let failed_admission = ProgrammaticActivationAdmission::new(
            workspace,
            Arc::clone(&failed_runtime),
            Arc::clone(&failed_registry),
            Arc::new(successor_recipe(&successor, successor_event.pins())),
        );
        let mismatched_fence = WriterFence {
            lease_id: LeaseId::from_bytes(id16(0x5a)),
            generation: WriterGeneration::new(1).expect("writer generation"),
        };
        let barrier = failed_admission
            .close_admission(successor_command.expected_head, mismatched_fence)
            .await
            .expect("close admission with independently supplied fence");
        assert_eq!(
            failed_admission
                .publish_selected_epoch(barrier, &selected_chain, Arc::clone(successor.epoch()),)
                .await
                .unwrap_err(),
            AdmissionError::SelectedEventFenceMismatch
        );
        let dormant = failed_registry
            .resolve(successor_epoch)
            .expect("dormant successor was installed before failed swap");
        assert!(Arc::ptr_eq(dormant.epoch(), successor.epoch()));
        assert_eq!(
            failed_runtime.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );

        let substituted = query_authority(workspace, successor_epoch).await;
        let substituted_runtime = Arc::new(
            FabricAdmissionRuntime::recover(&first_chain, |_| Some(Arc::clone(first.epoch())))
                .expect("initial admission"),
        );
        let substituted_registry = Arc::new(WorkspaceEpochQueryAuthorityRegistry::with_initial(
            Arc::clone(&first),
        ));
        let substituted_admission = ProgrammaticActivationAdmission::new(
            workspace,
            Arc::clone(&substituted_runtime),
            Arc::clone(&substituted_registry),
            Arc::new(successor_recipe(&substituted, successor_event.pins())),
        );
        let barrier = substituted_admission
            .close_admission(
                successor_command.expected_head,
                successor_command.writer_fence,
            )
            .await
            .expect("close admission");
        assert_eq!(
            substituted_admission
                .publish_selected_epoch(barrier, &selected_chain, Arc::clone(successor.epoch()),)
                .await
                .unwrap_err(),
            AdmissionError::SuccessorQueryAuthorityMismatch(successor_epoch)
        );
        assert_eq!(
            substituted_registry.resolve(successor_epoch).unwrap_err(),
            WorkspaceEpochQueryAuthorityRegistryError::UnknownEpoch(successor_epoch)
        );
        assert_eq!(
            substituted_runtime.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );
    }

    #[test]
    fn semantic_catalog_authority_rejects_pin_and_program_drift() {
        let (ingress, mut execution) = catalogs(id32(10));
        Arc::make_mut(&mut execution).source_pin = id32(11);
        assert!(matches!(
            validate_semantic_authority(&ingress, &execution, &closure()),
            Err(ProgrammaticWorkspaceCompositionError::SemanticAuthorityPinMismatch("source"))
        ));

        let (ingress, mut execution) = catalogs(id32(10));
        Arc::make_mut(&mut execution).programs[0].execution_program_pin = id32(12);
        assert!(matches!(
            validate_semantic_authority(&ingress, &execution, &closure()),
            Err(ProgrammaticWorkspaceCompositionError::ProgramCatalogCoverageMismatch)
        ));
    }

    #[tokio::test]
    async fn arbitrary_epoch_labels_cannot_authorize_a_sealed_epoch() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let epoch_id = EpochId::from_bytes(id16(12));
        let epoch = Arc::new(
            ProgrammaticFabricEpochBuilder::try_new(
                epoch_id,
                super::super::epoch_runtime::FabricEpochRuntimeConfig::default(),
            )
            .unwrap()
            .seal_for_test()
            .await
            .unwrap(),
        );
        let resources = Arc::new(
            EpochResourceCoordinator::try_new(epoch_id, id32(6), resource_policy()).unwrap(),
        );
        let (ingress, execution) = catalogs(id32(99));
        let pins = query_authority_pins(&epoch);
        assert!(matches!(
            WorkspaceEpochQueryAuthority::try_new(
                workspace,
                pins,
                Arc::clone(&epoch),
                resources,
                ingress,
                execution,
                closure(),
                authorization(),
                request_owned_relation_limits(),
                result_limits(),
                60_000,
            ),
            Err(ProgrammaticWorkspaceCompositionError::FabricEpochAuthorityPinMismatch {
                supplied,
                ..
            }) if supplied == id32(99)
        ));
    }

    #[test]
    fn request_owned_limits_identity_is_complete_and_stable() {
        let limits = request_owned_relation_limits();
        assert_eq!(
            request_owned_relation_limits_pin(limits),
            request_owned_relation_limits_pin(limits)
        );
        let changed =
            RequestOwnedRelationLimits::try_new(5, 64, 16, 1_024, 128, 2_048, 64 * 1024).unwrap();
        assert_ne!(
            request_owned_relation_limits_pin(limits),
            request_owned_relation_limits_pin(changed)
        );
    }

    #[test]
    fn workspace_public_identity_is_strictly_canonical() {
        let workspace = WorkspaceId::from_bytes(id16(0x42));
        let public =
            encode_public_id(IdentityDomain::Workspace, None, *workspace.as_bytes()).unwrap();
        assert_eq!(
            WorkspaceId::from_bytes(
                decode_public_id(IdentityDomain::Workspace, None, &public).unwrap()
            ),
            workspace
        );
        assert!(decode_public_id(IdentityDomain::Workspace, None, "workspace:not-hex").is_err());
        assert!(
            decode_public_id(
                IdentityDomain::Workspace,
                None,
                "repository:00000000000000000000000000000000"
            )
            .is_err()
        );
    }
}
