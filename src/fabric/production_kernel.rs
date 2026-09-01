//! Phase-typed ownership for the production daemon kernel.
//!
//! This module is deliberately small at the operational boundary. Semantic authority is owned by
//! [`CompiledSemanticRelease`], while each installed workspace is one indivisible
//! [`ActiveWorkspace`]. Callers can observe lifecycle state, but only the startup coordinator and
//! daemon kernel can advance it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use arc_swap::{ArcSwap, ArcSwapOption};
use thiserror::Error;

use super::activation::{ActivationEvent, ActivationEventId, TableVersionSet};
use super::activation_control_delta::{ActivationControlHorizon, ExactSelectedActivation};
use super::command::{EpochId, ProofReceiptRef, WorkspaceId, WriterFence};
use super::derived_producer_closure::{
    CompiledDerivedProducerClosure, DerivedProducerClosureError, DerivedProducerClosureExecution,
    ProducerClosureCancellation, ProducerClosureResourceBounds,
    compile_release_owned_derived_producer_closure,
};
use super::programmatic_epoch::{ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder};
use super::programmatic_ingress_port::ApplicationOwnedSemanticIngressPort;
use super::programmatic_query_backend::{
    CompiledV20ProgrammaticScopeAuthorization, ExactProgrammaticSnapshotProjection,
    ProgrammaticQueryPortError, ProgrammaticSemanticQueryBackendError,
    ProgrammaticSemanticQueryPorts,
};
use super::programmatic_schema::ProgrammaticRelationId;
use super::programmatic_workspace::ProgrammaticWorkspaceRuntime;
use super::proof::{
    ProofError, ProofTerminalStatus, ReleaseProducerClosureProofInput,
    ReleaseProducerClosureProofResult, evaluate_release_producer_closure,
};
use crate::production_provider_recipe::{
    ProductionProviderAuthority, ProductionProviderRecipeError, ProductionProviderRuns,
    admit_production_provider_relations,
};
use crate::production_query_recipe::{
    ProductionQueryRecipeError, ProductionSemanticQueryRecipe, ProductionSemanticQueryRecipeInput,
};
use crate::programmatic_derived_analysis::{
    ProgrammaticDerivedAnalysisError, ReleasedProgrammaticDerivedAnalysisOutcome,
    admit_and_compose_released_programmatic_derived_analyses,
};
use crate::provider_admission::{
    ExactProgrammaticProviderRuns, ProgrammaticProviderAdmissionOutcome,
};
use crate::relational_semantic_query::EpochBoundSemanticIngressLimits;
use crate::workspace_registry::WorkspaceRecord;

/// Sole compiled suite selected by this production release.
pub const COMPILED_SUITE_ID: &str = "codefabric-relational-data-fabric";
/// Synchronized version of every role in the sole compiled suite.
pub const COMPILED_SUITE_VERSION: &str = "2.2.0";

/// Immutable identity of one synchronized authoritative suite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SuiteIdentity {
    suite_id: &'static str,
    suite_version: &'static str,
}

impl SuiteIdentity {
    /// Return the sole production suite identity compiled into this binary.
    #[must_use]
    pub const fn current() -> Self {
        Self {
            suite_id: COMPILED_SUITE_ID,
            suite_version: COMPILED_SUITE_VERSION,
        }
    }

    #[must_use]
    pub const fn suite_id(self) -> &'static str {
        self.suite_id
    }

    #[must_use]
    pub const fn suite_version(self) -> &'static str {
        self.suite_version
    }

    #[must_use]
    pub fn display(self) -> String {
        format!("{}@{}", self.suite_id, self.suite_version)
    }
}

/// Capability proving that a provider recipe came from the compiled release.
///
/// The field is private so no sibling module can manufacture this authority. Operational provider
/// runs and source pins remain variable inputs, but schemas, relation descriptors, field roles,
/// coverage semantics, and admission programs require this token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompiledProviderAuthority(());

/// Capability proving that transformation and analysis programs came from the compiled release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompiledTransformationAuthority(());

/// Capability proving that the eight query programs came from the compiled release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompiledQueryAuthority(());

/// Capability proving that proof programs and producer closure came from the compiled release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompiledProofAuthority(());

/// Capability proving that policy and reduced-child construction came from the compiled release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompiledPolicyAuthority(());

/// Immutable semantic authority compiled into the daemon.
///
/// Every semantic constructor requires one of the private capabilities below. The public
/// constructor accepts no caller-authored schema, descriptor, transformation, producer, proof,
/// policy, catalog, or query-program value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledSemanticRelease {
    suite: SuiteIdentity,
    providers: CompiledProviderAuthority,
    transformations: CompiledTransformationAuthority,
    queries: CompiledQueryAuthority,
    proof: CompiledProofAuthority,
    policy: CompiledPolicyAuthority,
}

impl CompiledSemanticRelease {
    /// Construct the sole compiled release. Operational configuration cannot substitute another
    /// suite, catalog, schema, producer closure, or query program.
    #[must_use]
    pub const fn current() -> Self {
        Self {
            suite: SuiteIdentity::current(),
            providers: CompiledProviderAuthority(()),
            transformations: CompiledTransformationAuthority(()),
            queries: CompiledQueryAuthority(()),
            proof: CompiledProofAuthority(()),
            policy: CompiledPolicyAuthority(()),
        }
    }

    #[must_use]
    pub const fn suite(self) -> SuiteIdentity {
        self.suite
    }

    #[must_use]
    pub(crate) const fn provider_authority(&self) -> &CompiledProviderAuthority {
        &self.providers
    }

    #[must_use]
    pub(crate) const fn transformation_authority(&self) -> &CompiledTransformationAuthority {
        &self.transformations
    }

    #[must_use]
    pub(crate) const fn query_authority(&self) -> &CompiledQueryAuthority {
        &self.queries
    }

    #[must_use]
    pub(crate) const fn proof_authority(&self) -> &CompiledProofAuthority {
        &self.proof
    }

    #[must_use]
    pub(crate) const fn policy_authority(&self) -> &CompiledPolicyAuthority {
        &self.policy
    }

    /// Admit one operational provider-run set through the sole compiled descriptor release.
    ///
    /// Source/context pins and requested-unit counts remain explicit operational inputs. Relation
    /// schemas, field roles, coverage semantics, and admission programs are selected privately by
    /// this release and cannot be supplied by the caller.
    ///
    /// # Errors
    ///
    /// Returns a typed descriptor, schema, or atomic provider-admission failure.
    pub(crate) fn admit_provider_relations(
        &self,
        builder: ProgrammaticFabricEpochBuilder,
        authority: ProductionProviderAuthority,
        runs: ProductionProviderRuns<'_>,
    ) -> Result<ProgrammaticProviderAdmissionOutcome, ProductionProviderRecipeError> {
        admit_production_provider_relations(self.provider_authority(), builder, authority, runs)
    }

    /// Admit operational provider runs and install one release-owned derived-analysis program.
    ///
    /// The provider batches and candidate builder remain operational inputs. The composition can
    /// only be constructed inside this compiled release with its non-forgeable transformation
    /// capability; callers cannot submit an alternate family census, schema, disposition, or
    /// transformation program.
    ///
    /// # Errors
    ///
    /// Returns a typed provider-admission, transformation-contract, or atomic composition error.
    pub(crate) fn admit_and_compose_derived_analyses(
        &self,
        builder: ProgrammaticFabricEpochBuilder,
        runs: ExactProgrammaticProviderRuns<'_>,
    ) -> Result<ReleasedProgrammaticDerivedAnalysisOutcome, ProgrammaticDerivedAnalysisError> {
        admit_and_compose_released_programmatic_derived_analyses(
            self.transformation_authority(),
            self.proof_authority(),
            self.query_authority(),
            builder,
            runs,
        )
    }

    /// Compile the eight released query programs against one exact sealed epoch and executed
    /// producer closure.
    ///
    /// The caller supplies only source/policy/resource inputs and the non-forgeable result of the
    /// release-owned closure execution. Query forms, operands, scopes, schemas, and catalog
    /// identities remain private compiled outputs.
    ///
    /// # Errors
    ///
    /// Returns a typed epoch, program, or producer-closure mismatch.
    pub fn compile_semantic_query_recipe(
        &self,
        epoch: &ProgrammaticFabricEpoch,
        input: ProductionSemanticQueryRecipeInput,
        closure_execution: &DerivedProducerClosureExecution,
    ) -> Result<ProductionSemanticQueryRecipe, ProductionQueryRecipeError> {
        ProductionSemanticQueryRecipe::try_from_executed_closure(
            self.query_authority(),
            epoch,
            input,
            closure_execution,
        )
    }

    /// Compile the release-owned producer and transitive query closure as native DataFusion plans.
    ///
    /// The sealed epoch and resource bounds remain explicit operational inputs. Exact relation
    /// identities, providers, schemas, field roles, output contracts, semantic identities, and
    /// proof programs are resolved or constructed privately by this release.
    ///
    /// # Errors
    ///
    /// Returns a typed relation/schema drift or logical-plan construction failure.
    pub async fn compile_producer_closure(
        &self,
        epoch: &ProgrammaticFabricEpoch,
        bounds: ProducerClosureResourceBounds,
    ) -> Result<CompiledDerivedProducerClosure, DerivedProducerClosureError> {
        compile_release_owned_derived_producer_closure(self.proof_authority(), epoch, bounds).await
    }

    /// Compile, execute, decode, and prove the release-owned producer closure for one exact epoch.
    ///
    /// This is the sole production bridge from a sealed candidate into queryable producer
    /// authority. The returned capability contains the actual decoded Arrow rows and can only be
    /// constructed when their release binding is valid and their derived terminal status is
    /// `Pass`. Cancellation or a resource failure returns no partially proved value.
    ///
    /// # Errors
    ///
    /// Returns a typed compilation, execution, cancellation, proof-binding, or semantic-closure
    /// failure.
    pub(crate) async fn prove_producer_closure(
        &self,
        epoch: &ProgrammaticFabricEpoch,
        bounds: ProducerClosureResourceBounds,
        cancellation: &ProducerClosureCancellation,
    ) -> Result<ProvedDerivedProducerClosure, CompiledProducerClosureProofError> {
        let compiled = self.compile_producer_closure(epoch, bounds).await?;
        let execution = compiled
            .execute_with_cancellation(&epoch.context(), cancellation)
            .await?;
        let proof = evaluate_release_producer_closure(
            ReleaseProducerClosureProofInput::try_from_execution(
                self.proof_authority(),
                &execution,
            )?,
        );
        if proof.terminal() != ProofTerminalStatus::Pass {
            return Err(CompiledProducerClosureProofError::SemanticClosureRejected {
                operation_id: Arc::clone(proof.operation_id()),
                violation_rows: proof.violations().len(),
                issue_rows: proof.issues().len(),
            });
        }
        Ok(ProvedDerivedProducerClosure { execution, proof })
    }

    /// Compose the exact ingress, authorization, and snapshot ports for one compiled query recipe.
    ///
    /// The recipe is a non-forgeable output of [`Self::compile_semantic_query_recipe`]. The caller
    /// may vary only explicit policy/resource inputs and the table capabilities admitted for the
    /// child session. The release chooses the v2.0 mapping and concrete snapshot implementation.
    ///
    /// # Errors
    ///
    /// Returns a typed mapping, policy, or port-bundle mismatch.
    pub fn compose_semantic_query_ports(
        &self,
        recipe: &ProductionSemanticQueryRecipe,
        limits: EpochBoundSemanticIngressLimits,
        policy_pin: [u8; 32],
        table_relations: BTreeSet<ProgrammaticRelationId>,
        max_output_rows: usize,
    ) -> Result<ProgrammaticSemanticQueryPorts, CompiledSemanticQueryPortsError> {
        let ingress =
            ApplicationOwnedSemanticIngressPort::try_compiled_v2_0(self.query_authority(), limits)?;
        let scope = CompiledV20ProgrammaticScopeAuthorization::try_new(
            self.query_authority(),
            self.policy_authority(),
            policy_pin,
            recipe.execution_catalog(),
            table_relations,
            max_output_rows,
        )?;
        Ok(ProgrammaticSemanticQueryPorts::try_new(
            self.query_authority(),
            Arc::new(ingress),
            Arc::new(scope),
            Arc::new(ExactProgrammaticSnapshotProjection::new()),
        )?)
    }
}

/// Non-forgeable successful producer closure retained for query-program compilation.
#[derive(Clone, Debug)]
pub(crate) struct ProvedDerivedProducerClosure {
    execution: DerivedProducerClosureExecution,
    proof: ReleaseProducerClosureProofResult,
}

impl ProvedDerivedProducerClosure {
    #[must_use]
    pub(crate) const fn execution(&self) -> &DerivedProducerClosureExecution {
        &self.execution
    }

    #[must_use]
    pub(crate) const fn proof(&self) -> &ReleaseProducerClosureProofResult {
        &self.proof
    }
}

/// Closed failures before a candidate can acquire proved producer authority.
#[derive(Debug, Error)]
pub(crate) enum CompiledProducerClosureProofError {
    #[error(transparent)]
    Closure(#[from] DerivedProducerClosureError),
    #[error(transparent)]
    Proof(#[from] ProofError),
    #[error(
        "release producer closure {operation_id:?} was rejected by decoded rows: {violation_rows} violation rows, {issue_rows} structural issue rows"
    )]
    SemanticClosureRejected {
        operation_id: Arc<str>,
        violation_rows: usize,
        issue_rows: usize,
    },
}

/// Closed failures while composing the release-owned semantic query ports.
#[derive(Debug, Error)]
pub enum CompiledSemanticQueryPortsError {
    #[error(transparent)]
    Port(#[from] ProgrammaticQueryPortError),
    #[error(transparent)]
    Bundle(#[from] ProgrammaticSemanticQueryBackendError),
}

/// Operational-only workspace registrations loaded from the coordinator-owned database.
///
/// This registry carries explicit roots, policy/lifecycle state, and source identity. It cannot
/// supply relation schemas, query plans, producer choices, epoch selection, or semantic Ready.
#[derive(Clone, Debug)]
pub struct OperationalWorkspaceRegistry {
    records: Arc<[WorkspaceRecord]>,
}

impl OperationalWorkspaceRegistry {
    /// Close and validate one operational registration census.
    ///
    /// # Errors
    ///
    /// Rejects duplicate workspace identities; an empty census remains an honest bootstrapping
    /// condition rather than manufacturing a default workspace.
    pub(crate) fn try_from_records(
        mut records: Vec<WorkspaceRecord>,
    ) -> Result<Self, OperationalWorkspaceRegistryError> {
        records.sort_by_key(|record| record.workspace_id);
        if records
            .windows(2)
            .any(|pair| pair[0].workspace_id == pair[1].workspace_id)
        {
            return Err(OperationalWorkspaceRegistryError::DuplicateWorkspace);
        }
        Ok(Self {
            records: records.into(),
        })
    }

    #[must_use]
    pub fn records(&self) -> &[WorkspaceRecord] {
        &self.records
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum OperationalWorkspaceRegistryError {
    #[error("operational workspace registry contains a duplicate workspace")]
    DuplicateWorkspace,
}

/// Observable phase of the one production lifecycle authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionLifecyclePhase {
    Configured,
    DaemonLeased,
    WriterFenced,
    CommandRecovered,
    GenesisRequired,
    SelectedEpochRecovered,
    EpochBuiltAndProved,
    WorkspaceInstalledClosed,
    EndpointsBoundBootstrapping,
    SoleTargetAuthorityObserved,
    SoleTargetAuthorityCommitted,
    Ready,
    Draining,
    Stopped,
    FailedClosed,
}

impl ProductionLifecyclePhase {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Configured => "CONFIGURED",
            Self::DaemonLeased => "DAEMON_LEASED",
            Self::WriterFenced => "WRITER_FENCED",
            Self::CommandRecovered => "COMMAND_RECOVERED",
            Self::GenesisRequired => "GENESIS_REQUIRED",
            Self::SelectedEpochRecovered => "SELECTED_EPOCH_RECOVERED",
            Self::EpochBuiltAndProved => "EPOCH_BUILT_AND_PROVED",
            Self::WorkspaceInstalledClosed => "WORKSPACE_INSTALLED_CLOSED",
            Self::EndpointsBoundBootstrapping => "ENDPOINTS_BOUND_BOOTSTRAPPING",
            Self::SoleTargetAuthorityObserved => "SOLE_TARGET_AUTHORITY_OBSERVED",
            Self::SoleTargetAuthorityCommitted => "SOLE_TARGET_AUTHORITY_COMMITTED",
            Self::Ready => "READY",
            Self::Draining => "DRAINING",
            Self::Stopped => "STOPPED",
            Self::FailedClosed => "FAILED_CLOSED",
        }
    }

    const fn allows(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Configured, Self::DaemonLeased)
                | (Self::DaemonLeased, Self::WriterFenced)
                | (
                    Self::WriterFenced,
                    Self::CommandRecovered | Self::EndpointsBoundBootstrapping
                )
                | (
                    Self::CommandRecovered,
                    Self::GenesisRequired | Self::SelectedEpochRecovered
                )
                | (
                    Self::GenesisRequired | Self::SelectedEpochRecovered,
                    Self::EpochBuiltAndProved
                )
                | (Self::EpochBuiltAndProved, Self::WorkspaceInstalledClosed)
                | (
                    Self::WorkspaceInstalledClosed,
                    Self::EndpointsBoundBootstrapping
                )
                | (
                    Self::EndpointsBoundBootstrapping,
                    Self::SoleTargetAuthorityObserved
                )
                | (
                    Self::SoleTargetAuthorityObserved,
                    Self::SoleTargetAuthorityCommitted
                )
                | (Self::SoleTargetAuthorityCommitted, Self::Ready)
                | (Self::Ready, Self::Draining)
                | (Self::Draining, Self::Stopped)
        )
    }
}

/// Read-only lifecycle projection. Sequence is owned by the authority rather than a service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleProjection {
    phase: ProductionLifecyclePhase,
    sequence: u64,
    failure_code: Option<Arc<str>>,
}

impl LifecycleProjection {
    #[must_use]
    pub const fn phase(&self) -> ProductionLifecyclePhase {
        self.phase
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub fn failure_code(&self) -> Option<&str> {
        self.failure_code.as_deref()
    }

    #[must_use]
    pub const fn semantic_admission_open(&self) -> bool {
        matches!(self.phase, ProductionLifecyclePhase::Ready)
    }
}

/// One process-wide lifecycle authority with serialized legal transitions and atomic readers.
pub struct LifecycleAuthority {
    projection: ArcSwap<LifecycleProjection>,
    transition: Mutex<()>,
}

impl fmt::Debug for LifecycleAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LifecycleAuthority")
            .field("projection", &self.projection.load_full())
            .finish()
    }
}

impl Default for LifecycleAuthority {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleAuthority {
    #[must_use]
    pub fn new() -> Self {
        Self {
            projection: ArcSwap::from_pointee(LifecycleProjection {
                phase: ProductionLifecyclePhase::Configured,
                sequence: 0,
                failure_code: None,
            }),
            transition: Mutex::new(()),
        }
    }

    /// Load one immutable lifecycle observation.
    #[must_use]
    pub fn observe(&self) -> Arc<LifecycleProjection> {
        self.projection.load_full()
    }

    /// Advance exactly one legal edge from the caller's observed phase.
    ///
    /// # Errors
    ///
    /// Rejects stale observations, illegal edges, a poisoned transition owner, or sequence
    /// exhaustion. No state changes on failure.
    pub(crate) fn advance(
        &self,
        expected: ProductionLifecyclePhase,
        next: ProductionLifecyclePhase,
    ) -> Result<Arc<LifecycleProjection>, ProductionLifecycleError> {
        let _guard = self
            .transition
            .lock()
            .map_err(|_| ProductionLifecycleError::OwnerUnavailable)?;
        let current = self.projection.load_full();
        if current.phase != expected {
            return Err(ProductionLifecycleError::StalePhase {
                expected,
                observed: current.phase,
            });
        }
        if !expected.allows(next) {
            return Err(ProductionLifecycleError::IllegalTransition { expected, next });
        }
        let projection = Arc::new(LifecycleProjection {
            phase: next,
            sequence: current
                .sequence
                .checked_add(1)
                .ok_or(ProductionLifecycleError::SequenceExhausted)?,
            failure_code: None,
        });
        self.projection.store(Arc::clone(&projection));
        Ok(projection)
    }

    /// Fail closed from any nonterminal phase without manufacturing a successful predecessor.
    pub(crate) fn fail_closed(
        &self,
        code: impl Into<Arc<str>>,
    ) -> Result<Arc<LifecycleProjection>, ProductionLifecycleError> {
        let _guard = self
            .transition
            .lock()
            .map_err(|_| ProductionLifecycleError::OwnerUnavailable)?;
        let current = self.projection.load_full();
        if current.phase == ProductionLifecyclePhase::Stopped {
            return Err(ProductionLifecycleError::AlreadyStopped);
        }
        let code = code.into();
        if code.is_empty() {
            return Err(ProductionLifecycleError::EmptyFailureCode);
        }
        let projection = Arc::new(LifecycleProjection {
            phase: ProductionLifecyclePhase::FailedClosed,
            sequence: current
                .sequence
                .checked_add(1)
                .ok_or(ProductionLifecycleError::SequenceExhausted)?,
            failure_code: Some(code),
        });
        self.projection.store(Arc::clone(&projection));
        Ok(projection)
    }

    /// Enter draining from any nonterminal state during joined cleanup.
    pub(crate) fn begin_draining(
        &self,
    ) -> Result<Arc<LifecycleProjection>, ProductionLifecycleError> {
        let _guard = self
            .transition
            .lock()
            .map_err(|_| ProductionLifecycleError::OwnerUnavailable)?;
        let current = self.projection.load_full();
        if current.phase == ProductionLifecyclePhase::Stopped {
            return Err(ProductionLifecycleError::AlreadyStopped);
        }
        if current.phase == ProductionLifecyclePhase::Draining {
            return Ok(current);
        }
        let projection = Arc::new(LifecycleProjection {
            phase: ProductionLifecyclePhase::Draining,
            sequence: current
                .sequence
                .checked_add(1)
                .ok_or(ProductionLifecycleError::SequenceExhausted)?,
            failure_code: current.failure_code.clone(),
        });
        self.projection.store(Arc::clone(&projection));
        Ok(projection)
    }

    pub(crate) fn finish_stopped(
        &self,
    ) -> Result<Arc<LifecycleProjection>, ProductionLifecycleError> {
        self.advance(
            ProductionLifecyclePhase::Draining,
            ProductionLifecyclePhase::Stopped,
        )
    }
}

/// Exact durable selection reconstructed from one completed activation-control readback.
///
/// The constructor consumes the private readback aggregate. Consequently the event, complete
/// reversible vector, writer fence, control horizon, and proof reference cannot be selected or
/// recomputed independently by a workspace/runtime caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedEpochRecord {
    event: ActivationEvent,
    table_versions: Arc<TableVersionSet>,
    control_horizon: ActivationControlHorizon,
    proof_reference: ProofReceiptRef,
}

impl SelectedEpochRecord {
    pub(crate) fn from_exact_readback(selection: &ExactSelectedActivation) -> Self {
        let proof_reference = selection.proof_reference();
        Self {
            event: selection.event(),
            table_versions: Arc::clone(selection.table_versions()),
            control_horizon: selection.control_horizon().clone(),
            proof_reference,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        event: ActivationEvent,
        table_versions: Arc<TableVersionSet>,
        control_horizon: ActivationControlHorizon,
    ) -> Self {
        Self {
            event,
            table_versions,
            control_horizon,
            proof_reference: event.pins().proof_receipt,
        }
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.event.workspace_id()
    }

    #[must_use]
    pub const fn epoch_id(&self) -> EpochId {
        self.event.pins().epoch
    }

    #[must_use]
    pub const fn activation_event_id(&self) -> ActivationEventId {
        self.event.event_id()
    }

    #[must_use]
    pub const fn writer_fence(&self) -> WriterFence {
        self.event.execution_fence()
    }

    #[must_use]
    pub const fn event(&self) -> ActivationEvent {
        self.event
    }

    #[must_use]
    pub const fn table_versions(&self) -> &Arc<TableVersionSet> {
        &self.table_versions
    }

    #[must_use]
    pub const fn control_horizon(&self) -> &ActivationControlHorizon {
        &self.control_horizon
    }

    #[must_use]
    pub const fn proof_reference(&self) -> ProofReceiptRef {
        self.proof_reference
    }
}

/// One indivisible query/mutation authority installed into a workspace slot.
pub struct ActiveWorkspace {
    #[cfg(not(test))]
    runtime: Arc<ProgrammaticWorkspaceRuntime>,
    #[cfg(test)]
    runtime: Option<Arc<ProgrammaticWorkspaceRuntime>>,
    selection: SelectedEpochRecord,
}

impl fmt::Debug for ActiveWorkspace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveWorkspace")
            .field("workspace_id", &self.selection.workspace_id())
            .field("epoch_id", &self.selection.epoch_id())
            .finish_non_exhaustive()
    }
}

impl ActiveWorkspace {
    /// Bind one complete runtime to the exact durable selection which caused its construction.
    ///
    /// # Errors
    ///
    /// Rejects a workspace, epoch, query-authority, or exact-vector substitution. The selection is
    /// never derived from the runtime and the runtime cannot choose another durable record.
    pub fn try_new(
        selection: SelectedEpochRecord,
        runtime: Arc<ProgrammaticWorkspaceRuntime>,
    ) -> Result<Self, ActiveWorkspaceError> {
        let authority = runtime.query_authority();
        if runtime.workspace_id() != selection.workspace_id()
            || authority.workspace_id() != selection.workspace_id()
            || authority.epoch_id() != selection.epoch_id()
            || authority.activation_pins().table_versions != selection.table_versions().reference()
            || authority.activation_pins().proof_receipt != selection.proof_reference()
            || !Arc::ptr_eq(authority.epoch(), runtime.epoch())
        {
            return Err(ActiveWorkspaceError::AuthoritySubstitution);
        }
        Ok(Self {
            #[cfg(not(test))]
            runtime,
            #[cfg(test)]
            runtime: Some(runtime),
            selection,
        })
    }

    #[cfg(test)]
    pub(crate) fn selection_probe(selection: SelectedEpochRecord) -> Self {
        Self {
            runtime: None,
            selection,
        }
    }

    #[must_use]
    pub fn runtime(&self) -> &Arc<ProgrammaticWorkspaceRuntime> {
        #[cfg(not(test))]
        {
            &self.runtime
        }
        #[cfg(test)]
        {
            self.runtime
                .as_ref()
                .expect("selection-only ActiveWorkspace probe has no runtime")
        }
    }

    #[must_use]
    pub const fn selection(&self) -> &SelectedEpochRecord {
        &self.selection
    }
}

/// One lease over an immutable active workspace. A later swap cannot change this lease.
#[derive(Clone, Debug)]
pub struct ActiveWorkspaceLease {
    workspace: Arc<ActiveWorkspace>,
}

impl ActiveWorkspaceLease {
    #[must_use]
    pub const fn workspace(&self) -> &Arc<ActiveWorkspace> {
        &self.workspace
    }
}

/// Atomic owner for exactly one workspace's installed authority.
pub struct WorkspaceSlot {
    workspace_id: WorkspaceId,
    active: ArcSwapOption<ActiveWorkspace>,
    installation: Mutex<()>,
    retired: AtomicBool,
}

/// Process-owned, close-once mapping from operational workspaces to atomic authority slots.
///
/// The operational registry may name workspaces, but it cannot install semantic authority. Each
/// entry therefore begins empty and can become queryable only through [`WorkspaceSlot`]'s exact
/// runtime-derived installation path.
#[derive(Debug, Default)]
pub struct WorkspaceSlotRegistry {
    slots: OnceLock<BTreeMap<WorkspaceId, Arc<WorkspaceSlot>>>,
    shutdown: AtomicBool,
}

impl WorkspaceSlotRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: OnceLock::new(),
            shutdown: AtomicBool::new(false),
        }
    }

    /// Close the operational workspace census exactly once.
    ///
    /// # Errors
    ///
    /// Rejects a repeated close. Duplicate workspace identities are already rejected by
    /// [`OperationalWorkspaceRegistry`].
    pub(crate) fn close_from_operational_registry(
        &self,
        registry: &OperationalWorkspaceRegistry,
    ) -> Result<(), WorkspaceSlotRegistryError> {
        let slots = registry
            .records()
            .iter()
            .map(|record| {
                let workspace_id = WorkspaceId::from_bytes(record.workspace_id);
                (workspace_id, Arc::new(WorkspaceSlot::empty(workspace_id)))
            })
            .collect();
        self.slots
            .set(slots)
            .map_err(|_| WorkspaceSlotRegistryError::AlreadyClosed)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.get().map_or(0, BTreeMap::len)
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.slots.get().is_some()
    }

    /// Close admission through every slot while preserving already-issued workspace leases.
    ///
    /// # Errors
    ///
    /// Rejects a repeated shutdown or a poisoned slot installation owner. The registry remains
    /// observably shut down after the first call even when clearing one slot reports an error.
    pub(crate) fn shutdown(&self) -> Result<usize, WorkspaceSlotRegistryError> {
        self.shutdown
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| WorkspaceSlotRegistryError::AlreadyShutdown)?;
        let Some(slots) = self.slots.get() else {
            return Ok(0);
        };
        let mut owner_unavailable = false;
        for slot in slots.values() {
            if slot.retire().is_err() {
                owner_unavailable = true;
            }
        }
        if owner_unavailable {
            return Err(WorkspaceSlotRegistryError::OwnerUnavailable);
        }
        Ok(slots.len())
    }

    #[must_use]
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn slot(&self, workspace_id: WorkspaceId) -> Option<Arc<WorkspaceSlot>> {
        self.slots.get()?.get(&workspace_id).cloned()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum WorkspaceSlotRegistryError {
    #[error("production workspace-slot registry is already closed")]
    AlreadyClosed,
    #[error("production workspace-slot registry is already shut down")]
    AlreadyShutdown,
    #[error("production workspace-slot installation owner is unavailable during shutdown")]
    OwnerUnavailable,
}

impl fmt::Debug for WorkspaceSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceSlot")
            .field("workspace_id", &self.workspace_id)
            .field("installed", &self.active.load().is_some())
            .finish()
    }
}

impl WorkspaceSlot {
    #[must_use]
    pub fn empty(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            active: ArcSwapOption::empty(),
            installation: Mutex::new(()),
            retired: AtomicBool::new(false),
        }
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    /// Acquire one immutable active-workspace lease.
    ///
    /// # Errors
    ///
    /// Fails while no exact selected epoch is installed; callers never fall back to an older or
    /// separately assembled bundle.
    pub fn lease(&self) -> Result<ActiveWorkspaceLease, ActiveWorkspaceError> {
        if self.retired.load(Ordering::Acquire) {
            return Err(ActiveWorkspaceError::Retired(self.workspace_id));
        }
        self.active
            .load_full()
            .map(|workspace| ActiveWorkspaceLease { workspace })
            .ok_or(ActiveWorkspaceError::NotInstalled(self.workspace_id))
    }

    /// Install the first exact active workspace while semantic admission remains closed.
    pub fn install_initial(
        &self,
        active: Arc<ActiveWorkspace>,
    ) -> Result<Arc<ActiveWorkspace>, ActiveWorkspaceError> {
        let _guard = self
            .installation
            .lock()
            .map_err(|_| ActiveWorkspaceError::OwnerUnavailable)?;
        if self.retired.load(Ordering::Acquire) {
            return Err(ActiveWorkspaceError::Retired(self.workspace_id));
        }
        if self.active.load().is_some() {
            return Err(ActiveWorkspaceError::AlreadyInstalled(self.workspace_id));
        }
        self.validate_workspace(&active)?;
        self.active.store(Some(Arc::clone(&active)));
        Ok(active)
    }

    /// Atomically advance to another exact active workspace and return the retained old lease.
    pub fn swap(
        &self,
        active: Arc<ActiveWorkspace>,
    ) -> Result<ActiveWorkspaceLease, ActiveWorkspaceError> {
        let _guard = self
            .installation
            .lock()
            .map_err(|_| ActiveWorkspaceError::OwnerUnavailable)?;
        if self.retired.load(Ordering::Acquire) {
            return Err(ActiveWorkspaceError::Retired(self.workspace_id));
        }
        let previous = self
            .active
            .load_full()
            .ok_or(ActiveWorkspaceError::NotInstalled(self.workspace_id))?;
        self.validate_workspace(&active)?;
        self.active.store(Some(active));
        Ok(ActiveWorkspaceLease {
            workspace: previous,
        })
    }

    fn validate_workspace(&self, workspace: &ActiveWorkspace) -> Result<(), ActiveWorkspaceError> {
        if workspace.selection.workspace_id() != self.workspace_id {
            return Err(ActiveWorkspaceError::WorkspaceSubstitution {
                expected: self.workspace_id,
                observed: workspace.selection.workspace_id(),
            });
        }
        Ok(())
    }

    fn retire(&self) -> Result<(), WorkspaceSlotRegistryError> {
        let _guard = self
            .installation
            .lock()
            .map_err(|_| WorkspaceSlotRegistryError::OwnerUnavailable)?;
        self.retired.store(true, Ordering::Release);
        self.active.store(None);
        Ok(())
    }
}

/// Stable lifecycle-authority failures.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ProductionLifecycleError {
    #[error("lifecycle transition owner is unavailable")]
    OwnerUnavailable,
    #[error("lifecycle sequence is exhausted")]
    SequenceExhausted,
    #[error("lifecycle is already stopped")]
    AlreadyStopped,
    #[error("failed-closed lifecycle code is empty")]
    EmptyFailureCode,
    #[error("stale lifecycle phase: expected {expected:?}, observed {observed:?}")]
    StalePhase {
        expected: ProductionLifecyclePhase,
        observed: ProductionLifecyclePhase,
    },
    #[error("illegal lifecycle transition from {expected:?} to {next:?}")]
    IllegalTransition {
        expected: ProductionLifecyclePhase,
        next: ProductionLifecyclePhase,
    },
}

/// Stable atomic-workspace installation failures.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ActiveWorkspaceError {
    #[error("active workspace is not installed for {0:?}")]
    NotInstalled(WorkspaceId),
    #[error("active workspace is already installed for {0:?}")]
    AlreadyInstalled(WorkspaceId),
    #[error("active workspace slot is retired for {0:?}")]
    Retired(WorkspaceId),
    #[error("workspace slot owner is unavailable")]
    OwnerUnavailable,
    #[error("active workspace query authority is incomplete: {0}")]
    QueryAuthority(String),
    #[error("active workspace query authority was substituted")]
    AuthoritySubstitution,
    #[error("workspace substitution: expected {expected:?}, observed {observed:?}")]
    WorkspaceSubstitution {
        expected: WorkspaceId,
        observed: WorkspaceId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_release_has_one_unsubstitutable_suite_identity() {
        let release = CompiledSemanticRelease::current();
        assert_eq!(release.suite().suite_id(), COMPILED_SUITE_ID);
        assert_eq!(release.suite().suite_version(), COMPILED_SUITE_VERSION);
        assert_eq!(
            release.suite().display(),
            "codefabric-relational-data-fabric@2.2.0"
        );
    }

    #[test]
    fn lifecycle_rejects_skips_stale_writers_and_false_ready() {
        let lifecycle = LifecycleAuthority::new();
        assert!(!lifecycle.observe().semantic_admission_open());
        assert!(matches!(
            lifecycle.advance(
                ProductionLifecyclePhase::Configured,
                ProductionLifecyclePhase::Ready
            ),
            Err(ProductionLifecycleError::IllegalTransition { .. })
        ));
        lifecycle
            .advance(
                ProductionLifecyclePhase::Configured,
                ProductionLifecyclePhase::DaemonLeased,
            )
            .unwrap();
        assert!(matches!(
            lifecycle.advance(
                ProductionLifecyclePhase::Configured,
                ProductionLifecyclePhase::DaemonLeased
            ),
            Err(ProductionLifecycleError::StalePhase { .. })
        ));
        assert_eq!(
            lifecycle.observe().phase(),
            ProductionLifecyclePhase::DaemonLeased
        );
    }

    #[test]
    fn failure_does_not_manufacture_a_successful_predecessor() {
        let lifecycle = LifecycleAuthority::new();
        let failed = lifecycle.fail_closed("WRITER_AUTHORITY_MISSING").unwrap();
        assert_eq!(failed.phase(), ProductionLifecyclePhase::FailedClosed);
        assert_eq!(failed.failure_code(), Some("WRITER_AUTHORITY_MISSING"));
        assert!(!failed.semantic_admission_open());
        let draining = lifecycle.begin_draining().unwrap();
        assert_eq!(draining.phase(), ProductionLifecyclePhase::Draining);
        assert_eq!(draining.failure_code(), Some("WRITER_AUTHORITY_MISSING"));
        assert_eq!(
            lifecycle.finish_stopped().unwrap().phase(),
            ProductionLifecyclePhase::Stopped
        );
    }

    #[test]
    fn empty_workspace_slot_never_falls_back() {
        let workspace_id = WorkspaceId::from_bytes([7; 16]);
        let slot = WorkspaceSlot::empty(workspace_id);
        assert_eq!(
            slot.lease().unwrap_err(),
            ActiveWorkspaceError::NotInstalled(workspace_id)
        );
    }
}
