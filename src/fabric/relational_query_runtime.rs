//! One admitted, authorized, Arrow-native relational query transaction.
//!
//! This module composes existing authorities without introducing another semantic registry:
//! [`FabricAdmissionRuntime`] pins one immutable epoch, [`ChildSessionPolicy`] constructs the
//! reduced DataFusion catalog, session-bound [`RelationalProgram`] values execute through that
//! child, [`ArrowResultResourcePackage`] preserves exact Arrow schemas and record batches as
//! bounded IPC, and [`PublishedArrowResultRegistry`] owns external result authorization and
//! lifetime. Relation and field identities remain stable programmatic data throughout.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_schema::SchemaRef;

use crate::cancellation::Cancellation;
use crate::relational_program::{
    CompilationObservations, RelationId, RelationalProgram, RelationalProgramCompiler,
    RelationalProgramError,
};

use super::admission::{AdmissionError, FabricAdmissionRuntime, FabricQueryLease};
use super::arrow_result_resource::{
    ArrowResultResourceError, ArrowResultResourceLimits, ArrowResultResourcePackage,
    QueryExecutionPin, ResultCoverage, ResultRelationInput, ResultResourceLease,
};
use super::child_session::resource_governance::{
    EpochResourceCoordinator, EpochResourceError, EpochWorkClass, EpochWorkRequest,
};
use super::child_session::{
    ChildRegistryAllowlist, ChildResourceLimits, ChildSessionError, ChildSessionPins,
    ChildSessionPolicy, ChildTableGrant,
};
use super::command::{EpochId, WorkspaceId};
use super::programmatic_schema::ProgrammaticRelationId;
use super::published_arrow_result::{
    OpaqueResultLeaseToken, PublishedArrowResultDescriptor, PublishedArrowResultRegistry,
    PublishedReleaseOutcome, PublishedResultAccess, PublishedResultChunk, PublishedResultOwner,
    PublishedResultReadRequest, PublishedResultRegistryError,
};
use super::request_owned_relation::RequestOwnedRelationCollection;

/// Exact table and resource authorization inputs used to derive one reduced child session.
///
/// The three policy identities are execution dependencies, not advisory labels. The runtime
/// reconstructs [`ChildSessionPins`] against the epoch it actually admitted, so a caller cannot
/// select one epoch for policy validation and execute against another.
#[derive(Clone, Debug)]
pub struct RelationalQueryAuthorization {
    access_scope: [u8; 32],
    query_policy: [u8; 32],
    resource_policy: [u8; 32],
    table_grants: Vec<ChildTableGrant>,
    child_resources: ChildResourceLimits,
    max_output_rows: usize,
    registries: ChildRegistryAllowlist,
}

impl RelationalQueryAuthorization {
    /// Construct a bounded authorization input without selecting an epoch.
    ///
    /// # Errors
    ///
    /// Rejects absent policy identities, an empty/duplicate table grant set, or a zero output
    /// bound. Unsupported registry authority remains explicit when the child is constructed.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        access_scope: [u8; 32],
        query_policy: [u8; 32],
        resource_policy: [u8; 32],
        table_grants: Vec<ChildTableGrant>,
        child_resources: ChildResourceLimits,
        max_output_rows: usize,
        registries: ChildRegistryAllowlist,
    ) -> Result<Self, RelationalQueryRuntimeError> {
        for (kind, value) in [
            ("access", access_scope),
            ("query", query_policy),
            ("resource", resource_policy),
        ] {
            if all_zero(&value) {
                return Err(RelationalQueryRuntimeError::MissingAuthorizationPin(kind));
            }
        }
        if table_grants.is_empty() {
            return Err(RelationalQueryRuntimeError::AuthorizationTablesEmpty);
        }
        let mut tables = BTreeSet::<ProgrammaticRelationId>::new();
        for grant in &table_grants {
            if !tables.insert(grant.relation_id().clone()) {
                return Err(RelationalQueryRuntimeError::DuplicateAuthorizationTable(
                    grant.relation_id().clone(),
                ));
            }
        }
        if max_output_rows == 0 {
            return Err(RelationalQueryRuntimeError::OutputRowBoundZero);
        }
        Ok(Self {
            access_scope,
            query_policy,
            resource_policy,
            table_grants,
            child_resources,
            max_output_rows,
            registries,
        })
    }

    /// Exact normalized access-scope identity consumed by the child-session policy.
    #[must_use]
    pub const fn access_scope(&self) -> &[u8; 32] {
        &self.access_scope
    }

    /// Exact semantic-query policy identity consumed by the child-session policy.
    #[must_use]
    pub const fn query_policy(&self) -> &[u8; 32] {
        &self.query_policy
    }

    /// Exact epoch resource-policy identity consumed by the child-session policy.
    #[must_use]
    pub const fn resource_policy(&self) -> &[u8; 32] {
        &self.resource_policy
    }

    /// Stable relation identities in the installed baseline table-grant set.
    pub fn table_relations(&self) -> impl ExactSizeIterator<Item = &ProgrammaticRelationId> {
        self.table_grants.iter().map(ChildTableGrant::relation_id)
    }

    /// Maximum output rows allowed by the installed baseline authorization.
    #[must_use]
    pub const fn max_output_rows(&self) -> usize {
        self.max_output_rows
    }

    /// Derive a scope-specific authorization that can only remove baseline capabilities.
    ///
    /// Query and resource-policy identities, child resource limits, and registry capabilities are
    /// preserved exactly. The caller supplies a new non-sentinel access-scope identity, a strict
    /// subset of baseline table grants, and an output-row bound no larger than the installed
    /// baseline. This makes scope policy a capability-narrowing operation rather than an
    /// independent authority mint.
    ///
    /// # Errors
    ///
    /// Rejects an absent access-scope identity, any table not present in the baseline, an empty
    /// resulting table set, or an output-row bound that is zero or wider than the baseline.
    pub fn narrow_to(
        &self,
        access_scope: [u8; 32],
        table_relations: &BTreeSet<ProgrammaticRelationId>,
        max_output_rows: usize,
    ) -> Result<Self, RelationalQueryRuntimeError> {
        if max_output_rows > self.max_output_rows {
            return Err(
                RelationalQueryRuntimeError::AuthorizationOutputRowsWidened {
                    baseline: self.max_output_rows,
                    requested: max_output_rows,
                },
            );
        }
        let baseline = self
            .table_grants
            .iter()
            .map(|grant| (grant.relation_id(), grant))
            .collect::<BTreeMap<_, _>>();
        let table_grants = table_relations
            .iter()
            .map(|relation_id| {
                baseline.get(relation_id).cloned().cloned().ok_or_else(|| {
                    RelationalQueryRuntimeError::AuthorizationTableWidened(relation_id.clone())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new(
            access_scope,
            self.query_policy,
            self.resource_policy,
            table_grants,
            self.child_resources.clone(),
            max_output_rows,
            self.registries.clone(),
        )
    }

    fn into_child_policy(self, epoch_id: EpochId) -> Result<ChildSessionPolicy, ChildSessionError> {
        ChildSessionPolicy::try_new(
            ChildSessionPins::try_new(
                epoch_id,
                self.access_scope,
                self.query_policy,
                self.resource_policy,
            )?,
            self.table_grants,
            self.child_resources,
            self.max_output_rows,
            self.registries,
        )
    }
}

/// One selected output relation and the session-bound program that derives it.
///
/// `coverage` is optional at the input type only so the transaction boundary can fail explicitly
/// when a compiler omitted completeness. A declared [`ResultCoverage`] may itself be `Unknown`;
/// that state is preserved with its cause rather than rewritten as empty or complete.
#[derive(Clone, Debug)]
pub struct SelectedQueryOutput {
    relation_id: RelationId,
    program: RelationalProgram,
    coverage: Option<ResultCoverage>,
}

impl SelectedQueryOutput {
    #[must_use]
    pub const fn new(
        relation_id: RelationId,
        program: RelationalProgram,
        coverage: Option<ResultCoverage>,
    ) -> Self {
        Self {
            relation_id,
            program,
            coverage,
        }
    }

    #[must_use]
    pub const fn relation_id(&self) -> &RelationId {
        &self.relation_id
    }

    #[must_use]
    pub const fn program(&self) -> &RelationalProgram {
        &self.program
    }

    #[must_use]
    pub const fn coverage(&self) -> Option<&ResultCoverage> {
        self.coverage.as_ref()
    }
}

/// Complete input to one admit-authorize-execute-package-publish transaction.
#[derive(Clone, Debug)]
pub struct RelationalQueryTransaction {
    owner: PublishedResultOwner,
    query_execution: QueryExecutionPin,
    authorization: RelationalQueryAuthorization,
    outputs: Vec<SelectedQueryOutput>,
    result_lease: ResultResourceLease,
    lease_token: OpaqueResultLeaseToken,
    result_limits: ArrowResultResourceLimits,
    request_inputs: BTreeMap<RelationId, Arc<RequestOwnedRelationCollection>>,
    observed_at_unix_ms: i64,
    cancellation: Cancellation,
}

impl RelationalQueryTransaction {
    /// Validate one transaction before it can consume an epoch lease.
    ///
    /// # Errors
    ///
    /// Rejects absent execution/agent identity, no outputs, duplicate output relations, or
    /// undeclared completeness. Output order is normalized by stable relation identity so causal
    /// observations and package construction are deterministic.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        owner: PublishedResultOwner,
        query_execution: QueryExecutionPin,
        authorization: RelationalQueryAuthorization,
        mut outputs: Vec<SelectedQueryOutput>,
        result_lease: ResultResourceLease,
        lease_token: OpaqueResultLeaseToken,
        result_limits: ArrowResultResourceLimits,
        observed_at_unix_ms: i64,
        cancellation: Cancellation,
    ) -> Result<Self, RelationalQueryRuntimeError> {
        if all_zero(query_execution.as_bytes()) {
            return Err(RelationalQueryRuntimeError::QueryExecutionPinMissing);
        }
        if all_zero(owner.agent_id().as_bytes()) {
            return Err(RelationalQueryRuntimeError::AgentNotAuthorized);
        }
        if outputs.is_empty() {
            return Err(RelationalQueryRuntimeError::OutputRelationsEmpty);
        }
        outputs.sort_by(|left, right| left.relation_id.cmp(&right.relation_id));
        for window in outputs.windows(2) {
            if window[0].relation_id == window[1].relation_id {
                return Err(RelationalQueryRuntimeError::DuplicateOutputRelation(
                    window[0].relation_id.as_str().to_owned(),
                ));
            }
        }
        for output in &outputs {
            if output.coverage.is_none() {
                return Err(RelationalQueryRuntimeError::CompletenessUndeclared(
                    output.relation_id.as_str().to_owned(),
                ));
            }
        }
        Ok(Self {
            owner,
            query_execution,
            authorization,
            outputs,
            result_lease,
            lease_token,
            result_limits,
            request_inputs: BTreeMap::new(),
            observed_at_unix_ms,
            cancellation,
        })
    }

    /// Attach the exact request-owned Arrow relations emitted by the epoch-bound compiler.
    ///
    /// The collection remains query-local and is retained until terminal execution. It is never
    /// installed in the epoch catalog or treated as durable authority. An empty collection is
    /// rejected so the ordinary epoch-only execution path remains explicit.
    ///
    /// # Errors
    ///
    /// Rejects an empty collection, which cannot causally affect this transaction.
    pub fn with_request_inputs(
        self,
        request_inputs: Arc<RequestOwnedRelationCollection>,
    ) -> Result<Self, RelationalQueryRuntimeError> {
        if self.outputs.len() != 1 {
            return Err(RelationalQueryRuntimeError::RequestInputOutputAmbiguous);
        }
        let output_relation = self.outputs[0].relation_id.clone();
        self.with_request_inputs_by_output([(output_relation, request_inputs)])
    }

    /// Attach independently bounded request-owned relations to their exact output programs.
    ///
    /// A multi-block request must not expose one block's request relation to another block merely
    /// because both execute in the same authenticated transaction. Each map entry therefore names
    /// the selected output whose program may consume the collection.
    ///
    /// # Errors
    ///
    /// Rejects no entries, an empty collection, an output that is not selected by this transaction,
    /// or a duplicate output key.
    pub fn with_request_inputs_by_output(
        mut self,
        request_inputs: impl IntoIterator<Item = (RelationId, Arc<RequestOwnedRelationCollection>)>,
    ) -> Result<Self, RelationalQueryRuntimeError> {
        let selected = self
            .outputs
            .iter()
            .map(|output| output.relation_id.clone())
            .collect::<BTreeSet<_>>();
        let mut attached = BTreeMap::new();
        for (output_relation, inputs) in request_inputs {
            if inputs.is_empty() {
                return Err(RelationalQueryRuntimeError::RequestInputsEmpty);
            }
            if !selected.contains(&output_relation) {
                return Err(RelationalQueryRuntimeError::UnknownRequestInputOutput(
                    output_relation.as_str().to_owned(),
                ));
            }
            if attached.insert(output_relation.clone(), inputs).is_some() {
                return Err(RelationalQueryRuntimeError::DuplicateRequestInputOutput(
                    output_relation.as_str().to_owned(),
                ));
            }
        }
        if attached.is_empty() {
            return Err(RelationalQueryRuntimeError::RequestInputsEmpty);
        }
        self.request_inputs = attached;
        Ok(self)
    }
}

/// Causal program and Arrow realization evidence for one published output relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationalQueryOutputObservation {
    relation_id: RelationId,
    schema: SchemaRef,
    row_count: u64,
    batch_count: u64,
    compilation: CompilationObservations,
}

impl RelationalQueryOutputObservation {
    #[must_use]
    pub const fn relation_id(&self) -> &RelationId {
        &self.relation_id
    }

    #[must_use]
    pub const fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    #[must_use]
    pub const fn row_count(&self) -> u64 {
        self.row_count
    }

    #[must_use]
    pub const fn batch_count(&self) -> u64 {
        self.batch_count
    }

    #[must_use]
    pub const fn compilation(&self) -> &CompilationObservations {
        &self.compilation
    }
}

/// Exact external descriptor plus causal observations from one successful transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationalQueryPublication {
    descriptor: PublishedArrowResultDescriptor,
    admission_generation: u64,
    outputs: Arc<[RelationalQueryOutputObservation]>,
}

impl RelationalQueryPublication {
    #[must_use]
    pub const fn descriptor(&self) -> &PublishedArrowResultDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub const fn admission_generation(&self) -> u64 {
        self.admission_generation
    }

    #[must_use]
    pub fn output_observations(&self) -> &[RelationalQueryOutputObservation] {
        &self.outputs
    }
}

/// Workspace-scoped composition root for admitted relational query delivery.
#[derive(Debug)]
pub struct RelationalQueryRuntime {
    workspace_id: WorkspaceId,
    admission: Arc<FabricAdmissionRuntime>,
    results: Arc<PublishedArrowResultRegistry>,
    resources: Arc<EpochResourceCoordinator>,
}

impl RelationalQueryRuntime {
    /// Bind the daemon's authenticated workspace to its admission and result-lifetime authorities.
    #[must_use]
    pub fn new(
        workspace_id: WorkspaceId,
        admission: Arc<FabricAdmissionRuntime>,
        results: Arc<PublishedArrowResultRegistry>,
        resources: Arc<EpochResourceCoordinator>,
    ) -> Self {
        Self {
            workspace_id,
            admission,
            results,
            resources,
        }
    }

    /// Workspace identity bound by this query composition root.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    /// Exact admission runtime shared with workspace activation.
    #[must_use]
    pub const fn admission(&self) -> &Arc<FabricAdmissionRuntime> {
        &self.admission
    }

    /// Cross-workspace published-result registry installed by the daemon composition root.
    #[must_use]
    pub const fn published_results(&self) -> &Arc<PublishedArrowResultRegistry> {
        &self.results
    }

    /// Exact epoch resource coordinator used for query admission and execution.
    #[must_use]
    pub const fn resources(&self) -> &Arc<EpochResourceCoordinator> {
        &self.resources
    }

    /// Admit, authorize, execute, package, and publish one exact query transaction.
    ///
    /// # Errors
    ///
    /// Fails closed on workspace/agent authority, missing or closed epoch admission, reduced-child
    /// construction, session binding/planning/execution, exact schema/row/batch/byte bounds, absent
    /// completeness, or publication. No successful result is truncated or row-transformed.
    pub async fn execute_and_publish(
        &self,
        transaction: RelationalQueryTransaction,
    ) -> Result<RelationalQueryPublication, RelationalQueryRuntimeError> {
        let epoch_lease = match self.admission.admit() {
            Ok(lease) => lease,
            Err(AdmissionError::NoActiveEpoch) => {
                return Err(RelationalQueryRuntimeError::NoActiveEpoch);
            }
            Err(error) => return Err(RelationalQueryRuntimeError::Admission(error)),
        };
        self.execute_admitted_and_publish(epoch_lease, Arc::clone(&self.resources), transaction)
            .await
    }

    /// Execute and publish against an epoch lease acquired by the caller before semantic
    /// compilation.
    ///
    /// This is the production composition seam for request compilers. The caller admits exactly
    /// once, compiles against `epoch_lease.epoch()`, resolves the matching epoch-scoped resource
    /// authority, and transfers both values here. Re-admitting after compilation would allow an
    /// activation between planning and execution to mix two epochs.
    ///
    /// # Errors
    ///
    /// In addition to ordinary transaction failures, rejects a resource coordinator belonging to
    /// any epoch other than the immutable admitted lease.
    pub async fn execute_admitted_and_publish(
        &self,
        epoch_lease: FabricQueryLease,
        resources: Arc<EpochResourceCoordinator>,
        transaction: RelationalQueryTransaction,
    ) -> Result<RelationalQueryPublication, RelationalQueryRuntimeError> {
        if transaction.owner.workspace_id() != self.workspace_id {
            return Err(RelationalQueryRuntimeError::WorkspaceNotAuthorized);
        }
        if all_zero(transaction.owner.agent_id().as_bytes()) {
            return Err(RelationalQueryRuntimeError::AgentNotAuthorized);
        }
        let admission_generation = epoch_lease.admission_generation();
        let epoch_id = epoch_lease.epoch_id();
        if resources.epoch_id() != epoch_id {
            return Err(RelationalQueryRuntimeError::ResourceEpochMismatch {
                admitted: epoch_id,
                coordinator: resources.epoch_id(),
            });
        }
        let epoch = Arc::clone(epoch_lease.epoch());
        let RelationalQueryTransaction {
            owner,
            query_execution,
            authorization,
            outputs,
            result_lease,
            lease_token,
            result_limits,
            request_inputs,
            observed_at_unix_ms,
            cancellation,
        } = transaction;
        let work = resources
            .admit(EpochWorkRequest {
                epoch_id,
                principal_id: owner.agent_id(),
                class: EpochWorkClass::InteractiveQuery,
                cancellation: cancellation.clone(),
            })
            .await?;
        let execution_resources = Arc::clone(&resources);
        let (package, observations) = work
            .run(async move {
                let child = epoch
                    .authorized_child_session(
                        authorization.into_child_policy(epoch_id)?,
                        &execution_resources,
                    )
                    .await?;
                let mut relation_inputs = Vec::with_capacity(outputs.len());
                let mut observations = Vec::with_capacity(outputs.len());
                for output in outputs {
                    let coverage = output.coverage.ok_or_else(|| {
                        RelationalQueryRuntimeError::CompletenessUndeclared(
                            output.relation_id.as_str().to_owned(),
                        )
                    })?;
                    let session_relation =
                        RelationalProgramCompiler::resolve_output_relation_with_bindings(
                            epoch.program_bindings(),
                            &output.program,
                        )?;
                    if session_relation != output.relation_id {
                        return Err(RelationalQueryRuntimeError::OutputRelationMismatch {
                            advertised: output.relation_id.as_str().to_owned(),
                            session_bound: session_relation.as_str().to_owned(),
                        });
                    }
                    let result =
                        if let Some(request_inputs) = request_inputs.get(&output.relation_id) {
                            child
                                .execute_relational_program_with_request_inputs(
                                    &output.program,
                                    request_inputs.as_ref(),
                                )
                                .await?
                        } else {
                            child.execute_relational_program(&output.program).await?
                        };
                    let row_count = u64::try_from(result.row_count())
                        .map_err(|_| RelationalQueryRuntimeError::ResultCountOverflow)?;
                    let batch_count = u64::try_from(result.batches().len())
                        .map_err(|_| RelationalQueryRuntimeError::ResultCountOverflow)?;
                    observations.push(RelationalQueryOutputObservation {
                        relation_id: output.relation_id.clone(),
                        schema: Arc::clone(result.schema()),
                        row_count,
                        batch_count,
                        compilation: result.observations().clone(),
                    });
                    relation_inputs.push(ResultRelationInput::new(
                        output.relation_id,
                        Arc::clone(result.schema()),
                        result.batches().to_vec(),
                        coverage,
                    ));
                }
                let package = Arc::new(ArrowResultResourcePackage::try_new(
                    epoch_id,
                    query_execution,
                    relation_inputs,
                    result_lease,
                    result_limits,
                )?);
                Ok::<_, RelationalQueryRuntimeError>((package, observations))
            })
            .await??;
        work.checkpoint()?;
        drop(work);
        let resource_lease = resources.retain_result(
            owner.agent_id(),
            result_lease,
            &package,
            observed_at_unix_ms,
        )?;
        if cancellation.is_cancelled() {
            return Err(EpochResourceError::Cancelled.into());
        }
        let descriptor = self.results.publish(
            owner,
            lease_token,
            epoch_lease,
            resource_lease,
            package,
            observed_at_unix_ms,
        )?;
        Ok(RelationalQueryPublication {
            descriptor,
            admission_generation,
            outputs: Arc::from(observations),
        })
    }

    /// Read one owner-bound manifest or relation chunk.
    pub fn read_chunk(
        &self,
        request: PublishedResultReadRequest,
    ) -> Result<PublishedResultChunk, RelationalQueryRuntimeError> {
        Ok(self.results.read_chunk(request)?)
    }

    /// Release one owner-bound result and its retained epoch pin.
    pub fn release(
        &self,
        access: PublishedResultAccess,
        observed_at_unix_ms: i64,
    ) -> Result<PublishedReleaseOutcome, RelationalQueryRuntimeError> {
        Ok(self.results.release(access, observed_at_unix_ms)?)
    }

    /// Collect expired live entries and released tombstones.
    pub fn collect_expired(
        &self,
        observed_at_unix_ms: i64,
    ) -> Result<usize, RelationalQueryRuntimeError> {
        Ok(self.results.collect_expired(observed_at_unix_ms)?)
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

/// Phase-specific failures from the relational query composition boundary.
#[derive(Debug, thiserror::Error)]
pub enum RelationalQueryRuntimeError {
    #[error("WORKSPACE_NOT_AUTHORIZED")]
    WorkspaceNotAuthorized,
    #[error("AGENT_NOT_AUTHORIZED")]
    AgentNotAuthorized,
    #[error("CURRENT_FACTS_UNAVAILABLE:NO_ACTIVE_EPOCH")]
    NoActiveEpoch,
    #[error(
        "INTERNAL_INVARIANT_VIOLATION:QUERY_RESOURCE_EPOCH_MISMATCH:admitted={admitted:?}:coordinator={coordinator:?}"
    )]
    ResourceEpochMismatch {
        admitted: EpochId,
        coordinator: EpochId,
    },
    #[error("INVALID_REQUEST_SCHEMA:QUERY_EXECUTION_PIN")]
    QueryExecutionPinMissing,
    #[error("INVALID_REQUEST_SCHEMA:QUERY_AUTHORIZATION_PIN:{0}")]
    MissingAuthorizationPin(&'static str),
    #[error("INVALID_REQUEST_SCHEMA:QUERY_AUTHORIZATION_TABLES_EMPTY")]
    AuthorizationTablesEmpty,
    #[error("INVALID_REQUEST_SCHEMA:DUPLICATE_QUERY_AUTHORIZATION_TABLE:{0:?}")]
    DuplicateAuthorizationTable(ProgrammaticRelationId),
    #[error("QUERY_AUTHORIZATION_SCOPE_WIDENED:TABLE:{0:?}")]
    AuthorizationTableWidened(ProgrammaticRelationId),
    #[error(
        "QUERY_AUTHORIZATION_SCOPE_WIDENED:OUTPUT_ROWS:baseline={baseline}:requested={requested}"
    )]
    AuthorizationOutputRowsWidened { baseline: usize, requested: usize },
    #[error("INVALID_REQUEST_SCHEMA:QUERY_OUTPUT_ROW_BOUND_ZERO")]
    OutputRowBoundZero,
    #[error("INVALID_REQUEST_SCHEMA:QUERY_OUTPUT_RELATIONS_EMPTY")]
    OutputRelationsEmpty,
    #[error("INVALID_REQUEST_SCHEMA:DUPLICATE_QUERY_OUTPUT_RELATION:{0}")]
    DuplicateOutputRelation(String),
    #[error("CAPABILITY_UNAVAILABLE:RESULT_COMPLETENESS_UNDECLARED:{0}")]
    CompletenessUndeclared(String),
    #[error(
        "INVALID_REQUEST_SCHEMA:QUERY_OUTPUT_RELATION_MISMATCH:advertised={advertised}:session_bound={session_bound}"
    )]
    OutputRelationMismatch {
        advertised: String,
        session_bound: String,
    },
    #[error("INTERNAL_INVARIANT_VIOLATION:QUERY_RESULT_COUNT_OVERFLOW")]
    ResultCountOverflow,
    #[error("REQUEST_INPUT_AUTHORITY_INVALID:request-owned relation collection is empty")]
    RequestInputsEmpty,
    #[error("REQUEST_INPUT_AUTHORITY_INVALID:multi-output request input attachment is ambiguous")]
    RequestInputOutputAmbiguous,
    #[error("REQUEST_INPUT_AUTHORITY_INVALID:request inputs name unknown output relation {0}")]
    UnknownRequestInputOutput(String),
    #[error("REQUEST_INPUT_AUTHORITY_INVALID:request inputs repeat output relation {0}")]
    DuplicateRequestInputOutput(String),
    #[error(transparent)]
    Admission(AdmissionError),
    #[error(transparent)]
    Child(#[from] ChildSessionError),
    #[error(transparent)]
    RelationalProgram(#[from] RelationalProgramError),
    #[error(transparent)]
    ArrowResult(#[from] ArrowResultResourceError),
    #[error(transparent)]
    PublishedResult(#[from] PublishedResultRegistryError),
    #[error(transparent)]
    ResourceGovernance(#[from] EpochResourceError),
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Cursor;

    use arrow_array::{RecordBatch, StringArray};
    use arrow_ipc::reader::StreamReader;
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::common::TableReference;
    use datafusion::datasource::MemTable;
    use serde_json::Value;

    use super::*;
    use crate::fabric::activation::{
        ActivationAttempt, ActivationChain, ActivationCommit, ActivationEvent, ActivationEventId,
        ActivationOrdinal, ActivationReadbackRef, BackendCommitRef, CompatibilityClassRef,
        FabricEpochPins, OverlaySegmentSetRef, PolicySetRef, TableVersionSetRef,
    };
    use crate::fabric::arrow_result_resource::{ResultCompleteness, ResultUnknownCause};
    use crate::fabric::command::{
        ActorId, AuthorizationRef, CommandIdentity, CommandOwnership, CommandPins, ExecutionOwner,
        ExpectedHead, FabricCommand, FabricCommandPayload, IdempotencyKey, InputReleaseRef,
        LeaseId, OperationId, OperationSelectionRef, PrincipalId, ProgramReleaseRef,
        ProofReceiptRef, ProviderSetRef, ResourceEnvelopeRef, RetentionPolicyRef, SourceGeneration,
        TransactionRef, WriterFence, WriterGeneration,
    };
    use crate::fabric::epoch_runtime::{FABRIC_CATALOG, FabricEpochRuntimeConfig};
    use crate::fabric::programmatic_epoch::{
        ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder,
    };
    use crate::fabric::programmatic_schema::{ProgrammaticRelationId, ProviderInput};
    use crate::relational_program::{
        CompilationDependency, FieldId, RelationalExpression, RelationalProgram,
    };
    use crate::schema_contract::{
        FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SchemaContract,
        SchemaRole,
    };

    const WP33_FIXTURES: &str =
        include_str!("../../contracts/acceptance/relational-fabric-v3/negative-fixtures.jsonl");

    fn claim_015_negative_fixture() -> Value {
        WP33_FIXTURES
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("valid WP33 fixture row"))
            .find(|row| row["claim_id"] == "RFV3-CLAIM-015" && row["kind"] == "negative")
            .expect("frozen Claim 015 negative fixture")
    }

    const fn id16(seed: u8) -> [u8; 16] {
        [seed; 16]
    }

    const fn id32(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn command(
        operation_seed: u8,
        workspace: WorkspaceId,
        predecessor: ExpectedHead,
        target: EpochId,
        generation: u64,
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
            expected_head: predecessor,
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
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: target,
                proof_receipt: ProofReceiptRef::from_bytes(id32(10)),
            },
        }
    }

    fn activation_event(
        event_seed: u8,
        command: &FabricCommand,
        predecessor_event_id: Option<ActivationEventId>,
        ordinal: u64,
        target: EpochId,
    ) -> ActivationEvent {
        ActivationEvent::try_from_attempt(
            ActivationEventId::from_bytes(id32(event_seed)),
            ActivationAttempt::for_test(
                *command,
                1,
                ExecutionOwner {
                    actor_id: ActorId::from_bytes(id16(33)),
                    fence: command.writer_fence,
                },
            ),
            predecessor_event_id,
            ActivationOrdinal::new(ordinal).unwrap(),
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
                proof_receipt: ProofReceiptRef::from_bytes(id32(10)),
            },
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

    const ALLOWED_RELATION: &str = "query.allowed_relation";
    const SECOND_RELATION: &str = "query.second_relation";

    fn identified_schema(relation_id: &str, fields: &[(&str, &str)]) -> Arc<Schema> {
        let fields = fields
            .iter()
            .map(|(name, field_id)| {
                Field::new(*name, DataType::Utf8, false).with_metadata(HashMap::from([(
                    FIELD_ID_METADATA_KEY.to_owned(),
                    (*field_id).to_owned(),
                )]))
            })
            .collect::<Vec<_>>();
        Arc::new(Schema::new_with_metadata(
            fields,
            HashMap::from([(RELATION_ID_METADATA_KEY.to_owned(), relation_id.to_owned())]),
        ))
    }

    fn provider_input(
        relation_id: &str,
        table_reference: TableReference,
        fields: &[(&str, &str)],
        rows: &[Vec<&str>],
    ) -> ProviderInput {
        let schema = identified_schema(relation_id, fields);
        let columns = (0..fields.len())
            .map(|column| {
                Arc::new(StringArray::from(
                    rows.iter().map(|row| row[column]).collect::<Vec<_>>(),
                )) as _
            })
            .collect::<Vec<_>>();
        let batch = RecordBatch::try_new(Arc::clone(&schema), columns).unwrap();
        let contract = Arc::new(
            SchemaContract::try_new(
                format!("provider:{relation_id}:v1"),
                table_reference.clone(),
                Arc::clone(&schema),
                Arc::clone(&schema),
                (0..fields.len())
                    .map(|index| FieldIndexMapping::direct(index, index))
                    .collect(),
            )
            .unwrap(),
        );
        ProviderInput::new(
            ProgrammaticRelationId::new(relation_id),
            table_reference,
            contract,
            Arc::new(MemTable::try_new(schema, vec![vec![batch]]).unwrap()),
        )
    }

    async fn epoch(epoch_id: EpochId) -> Arc<ProgrammaticFabricEpoch> {
        let config = FabricEpochRuntimeConfig::default();
        let mut builder = ProgrammaticFabricEpochBuilder::try_new(epoch_id, config).unwrap();
        builder
            .register_provider(provider_input(
                ALLOWED_RELATION,
                TableReference::full(FABRIC_CATALOG, "fact", "allowed_rows"),
                &[
                    ("entity_id", "query.allowed.entity_id"),
                    ("kind", "query.allowed.kind"),
                ],
                &[vec!["entity-1", "function"], vec!["entity-2", "class"]],
            ))
            .unwrap();
        builder
            .register_provider(provider_input(
                SECOND_RELATION,
                TableReference::full(FABRIC_CATALOG, "derived", "second_rows"),
                &[("entity_id", "query.second.entity_id")],
                &[vec!["entity-3"]],
            ))
            .unwrap();
        Arc::new(builder.seal_for_test().await.unwrap())
    }

    fn admitted_runtime(
        workspace: WorkspaceId,
        selected: Arc<ProgrammaticFabricEpoch>,
    ) -> (Arc<FabricAdmissionRuntime>, ActivationEvent) {
        let epoch_id = *selected.identity();
        let command = command(1, workspace, ExpectedHead::Empty, epoch_id, 1);
        let event = activation_event(1, &command, None, 1, epoch_id);
        let chain = ActivationChain::derive(workspace, [event]).unwrap();
        (
            Arc::new(
                FabricAdmissionRuntime::recover(&chain, |_| Some(Arc::clone(&selected))).unwrap(),
            ),
            event,
        )
    }

    fn owner(workspace: WorkspaceId, agent_seed: u8) -> PublishedResultOwner {
        PublishedResultOwner::new(workspace, PrincipalId::from_bytes(id16(agent_seed)))
    }

    fn token(seed: u8) -> OpaqueResultLeaseToken {
        OpaqueResultLeaseToken::try_from_bytes(id32(seed)).unwrap()
    }

    fn grant(relation_id: &str) -> ChildTableGrant {
        ChildTableGrant::try_new(ProgrammaticRelationId::new(relation_id)).unwrap()
    }

    fn program(
        epoch: &ProgrammaticFabricEpoch,
        relation_id: &str,
    ) -> (RelationId, RelationalProgram) {
        let sealed = epoch
            .relation(&ProgrammaticRelationId::new(relation_id))
            .unwrap();
        let relation_id = RelationId::new(relation_id).unwrap();
        let output_fields = (0..sealed.contract.logical_schema().fields().len())
            .map(|ordinal| {
                FieldId::new(
                    sealed
                        .contract
                        .field_id_at(SchemaRole::Logical, ordinal)
                        .unwrap(),
                )
                .unwrap()
            })
            .collect();
        let program = RelationalProgram {
            root: RelationalExpression::Input(relation_id.clone()),
            output_fields,
        };
        (relation_id, program)
    }

    fn expected_rows(relation_id: &str) -> u64 {
        if relation_id == ALLOWED_RELATION {
            2
        } else {
            1
        }
    }

    fn selected_output(epoch: &ProgrammaticFabricEpoch, relation_id: &str) -> SelectedQueryOutput {
        let (relation_id_value, program) = program(epoch, relation_id);
        SelectedQueryOutput::new(
            relation_id_value,
            program,
            Some(ResultCoverage::complete(expected_rows(relation_id))),
        )
    }

    fn authorization(
        _epoch: &ProgrammaticFabricEpoch,
        relations: &[&str],
        max_output_rows: usize,
    ) -> RelationalQueryAuthorization {
        RelationalQueryAuthorization::try_new(
            id32(0x11),
            id32(0x22),
            id32(0x33),
            relations
                .iter()
                .map(|relation_id| grant(relation_id))
                .collect(),
            child_resources(),
            max_output_rows,
            ChildRegistryAllowlist::default(),
        )
        .unwrap()
    }

    fn child_resources() -> ChildResourceLimits {
        ChildResourceLimits::try_new(8 * 1024 * 1024, 32 * 1024 * 1024, 4, 2, 128, 1).unwrap()
    }

    fn resource_coordinator(epoch: &ProgrammaticFabricEpoch) -> Arc<EpochResourceCoordinator> {
        Arc::new(
            EpochResourceCoordinator::try_new(
                *epoch.identity(),
                id32(0x33),
                super::super::child_session::resource_governance::EpochResourcePolicy::try_new(
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
                .unwrap(),
            )
            .unwrap(),
        )
    }

    fn result_limits() -> ArrowResultResourceLimits {
        ArrowResultResourceLimits::try_new(
            8,
            8,
            20_000,
            16,
            40_000,
            1 << 20,
            2 << 20,
            1 << 20,
            2 << 20,
            1 << 20,
            37,
        )
        .unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    fn transaction(
        epoch: &ProgrammaticFabricEpoch,
        result_owner: PublishedResultOwner,
        grants: &[&str],
        outputs: &[&str],
        max_output_rows: usize,
        limits: ArrowResultResourceLimits,
        query_seed: u8,
        lease_seed: u8,
        token_seed: u8,
    ) -> RelationalQueryTransaction {
        RelationalQueryTransaction::try_new(
            result_owner,
            QueryExecutionPin::from_bytes(id32(query_seed)),
            authorization(epoch, grants, max_output_rows),
            outputs
                .iter()
                .map(|relation_id| selected_output(epoch, relation_id))
                .collect(),
            ResultResourceLease::try_new(LeaseId::from_bytes(id16(lease_seed)), 1_000, 2_000)
                .unwrap(),
            token(token_seed),
            limits,
            1_500,
            Cancellation::default(),
        )
        .unwrap()
    }

    fn read_all(
        runtime: &RelationalQueryRuntime,
        access: PublishedResultAccess,
        resource_id: super::super::published_arrow_result::PublishedResultResourceId,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut offset = 0;
        loop {
            let chunk = runtime
                .read_chunk(PublishedResultReadRequest {
                    access,
                    resource_id,
                    observed_at_unix_ms: 1_500,
                    offset,
                    max_bytes: 37,
                })
                .unwrap();
            assert_eq!(chunk.offset, offset);
            bytes.extend_from_slice(&chunk.bytes);
            offset = chunk.next_offset;
            if chunk.complete {
                assert_eq!(offset, chunk.total_length);
                break;
            }
        }
        bytes
    }

    #[test]
    fn scope_authorization_can_only_narrow_baseline_capabilities() {
        let baseline = RelationalQueryAuthorization::try_new(
            id32(0x11),
            id32(0x22),
            id32(0x33),
            vec![grant(ALLOWED_RELATION), grant(SECOND_RELATION)],
            child_resources(),
            100,
            ChildRegistryAllowlist::default(),
        )
        .unwrap();
        let retained = BTreeSet::from([ProgrammaticRelationId::new(ALLOWED_RELATION)]);
        let narrowed = baseline.narrow_to(id32(0x44), &retained, 10).unwrap();
        assert_eq!(narrowed.access_scope(), &id32(0x44));
        assert_eq!(narrowed.query_policy(), baseline.query_policy());
        assert_eq!(narrowed.resource_policy(), baseline.resource_policy());
        assert_eq!(narrowed.max_output_rows(), 10);
        assert_eq!(
            narrowed.table_relations().cloned().collect::<BTreeSet<_>>(),
            retained
        );

        assert!(matches!(
            baseline.narrow_to(
                id32(0x45),
                &BTreeSet::from([ProgrammaticRelationId::new("facts.not-authorized")]),
                10,
            ),
            Err(RelationalQueryRuntimeError::AuthorizationTableWidened(_))
        ));
        assert!(matches!(
            baseline.narrow_to(id32(0x46), &retained, 101),
            Err(
                RelationalQueryRuntimeError::AuthorizationOutputRowsWidened {
                    baseline: 100,
                    requested: 101,
                }
            )
        ));
    }

    #[tokio::test]
    async fn success_is_arrow_native_deterministic_and_causally_observed() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let epoch = epoch(EpochId::from_bytes(id16(20))).await;
        let (admission, _) = admitted_runtime(workspace, Arc::clone(&epoch));
        let result_owner = owner(workspace, 0x31);
        let runtime = RelationalQueryRuntime::new(
            workspace,
            Arc::clone(&admission),
            Arc::new(PublishedArrowResultRegistry::new()),
            resource_coordinator(&epoch),
        );
        let publication = runtime
            .execute_and_publish(transaction(
                &epoch,
                result_owner,
                &[ALLOWED_RELATION],
                &[ALLOWED_RELATION],
                10_000,
                result_limits(),
                0x51,
                0x61,
                0x71,
            ))
            .await
            .unwrap();
        let expected_rows = expected_rows(ALLOWED_RELATION);
        assert_eq!(publication.descriptor().total_rows, expected_rows);
        assert_eq!(publication.output_observations().len(), 1);
        assert_eq!(
            publication.output_observations()[0].row_count(),
            expected_rows
        );
        assert!(
            publication.output_observations()[0]
                .compilation()
                .dependencies
                .contains(&CompilationDependency::Relation(
                    publication.output_observations()[0].relation_id().clone(),
                ))
        );

        let access = PublishedResultAccess {
            artifact_id: publication.descriptor().artifact_id,
            owner: result_owner,
            lease_token: token(0x71),
        };
        let ipc = read_all(
            &runtime,
            access,
            publication.descriptor().relations[0].authorization_resource_id,
        );
        let reader = StreamReader::try_new(Cursor::new(ipc), None).unwrap();
        let batches = reader.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(
            batches.iter().map(RecordBatch::num_rows).sum::<usize>(),
            usize::try_from(expected_rows).unwrap()
        );

        let rebuilt_runtime = RelationalQueryRuntime::new(
            workspace,
            admission,
            Arc::new(PublishedArrowResultRegistry::new()),
            resource_coordinator(&epoch),
        );
        let rebuilt = rebuilt_runtime
            .execute_and_publish(transaction(
                &epoch,
                result_owner,
                &[ALLOWED_RELATION],
                &[ALLOWED_RELATION],
                10_000,
                result_limits(),
                0x51,
                0x62,
                0x72,
            ))
            .await
            .unwrap();
        assert_eq!(publication.descriptor(), rebuilt.descriptor());
        assert_eq!(
            publication.output_observations(),
            rebuilt.output_observations()
        );
    }

    #[tokio::test]
    async fn no_epoch_workspace_and_denied_relation_fail_at_their_authority_boundaries() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let epoch = epoch(EpochId::from_bytes(id16(20))).await;
        let empty_chain = ActivationChain::derive(workspace, []).unwrap();
        let no_epoch = RelationalQueryRuntime::new(
            workspace,
            Arc::new(FabricAdmissionRuntime::recover(&empty_chain, |_| None).unwrap()),
            Arc::new(PublishedArrowResultRegistry::new()),
            resource_coordinator(&epoch),
        );
        assert!(matches!(
            no_epoch
                .execute_and_publish(transaction(
                    &epoch,
                    owner(workspace, 0x31),
                    &[ALLOWED_RELATION],
                    &[ALLOWED_RELATION],
                    10_000,
                    result_limits(),
                    0x51,
                    0x61,
                    0x71,
                ))
                .await,
            Err(RelationalQueryRuntimeError::NoActiveEpoch)
        ));

        let (admission, _) = admitted_runtime(workspace, Arc::clone(&epoch));
        let runtime = RelationalQueryRuntime::new(
            workspace,
            admission,
            Arc::new(PublishedArrowResultRegistry::new()),
            resource_coordinator(&epoch),
        );
        assert!(matches!(
            runtime
                .execute_and_publish(transaction(
                    &epoch,
                    owner(WorkspaceId::from_bytes(id16(2)), 0x31),
                    &[ALLOWED_RELATION],
                    &[ALLOWED_RELATION],
                    10_000,
                    result_limits(),
                    0x52,
                    0x62,
                    0x72,
                ))
                .await,
            Err(RelationalQueryRuntimeError::WorkspaceNotAuthorized)
        ));
        assert!(matches!(
            runtime
                .execute_and_publish(transaction(
                    &epoch,
                    owner(workspace, 0x31),
                    &[ALLOWED_RELATION],
                    &[SECOND_RELATION],
                    10_000,
                    result_limits(),
                    0x53,
                    0x63,
                    0x73,
                ))
                .await,
            Err(RelationalQueryRuntimeError::Child(
                ChildSessionError::DeniedProgramRelation { .. }
            ))
        ));

        let (advertised_relation, _) = program(&epoch, ALLOWED_RELATION);
        let (session_bound_relation, second_program) = program(&epoch, SECOND_RELATION);
        let second_rows = expected_rows(SECOND_RELATION);
        let mislabeled = RelationalQueryTransaction::try_new(
            owner(workspace, 0x31),
            QueryExecutionPin::from_bytes(id32(0x55)),
            authorization(&epoch, &[SECOND_RELATION], 10_000),
            vec![SelectedQueryOutput::new(
                advertised_relation.clone(),
                second_program,
                Some(ResultCoverage::complete(second_rows)),
            )],
            ResultResourceLease::try_new(LeaseId::from_bytes(id16(0x65)), 1_000, 2_000).unwrap(),
            token(0x75),
            result_limits(),
            1_500,
            Cancellation::default(),
        )
        .unwrap();
        assert!(matches!(
            runtime.execute_and_publish(mislabeled).await,
            Err(RelationalQueryRuntimeError::OutputRelationMismatch {
                advertised,
                session_bound,
            }) if advertised == advertised_relation.as_str()
                && session_bound == session_bound_relation.as_str()
        ));

        let (relation_id, relation_program) = program(&epoch, ALLOWED_RELATION);
        assert!(matches!(
            RelationalQueryTransaction::try_new(
                owner(workspace, 0x31),
                QueryExecutionPin::from_bytes(id32(0x54)),
                authorization(&epoch, &[ALLOWED_RELATION], 10_000),
                vec![SelectedQueryOutput::new(
                    relation_id.clone(),
                    relation_program,
                    None,
                )],
                ResultResourceLease::try_new(LeaseId::from_bytes(id16(0x64)), 1_000, 2_000)
                    .unwrap(),
                token(0x74),
                result_limits(),
                1_500,
                Cancellation::default(),
            ),
            Err(RelationalQueryRuntimeError::CompletenessUndeclared(relation))
                if relation == relation_id.as_str()
        ));

        let unknown = ResultCoverage::try_new(
            ResultCompleteness::Unknown,
            1,
            0,
            1,
            Some(ResultUnknownCause::try_new("CAPABILITY_GAP").unwrap()),
        )
        .unwrap();
        assert_eq!(unknown.state(), ResultCompleteness::Unknown);
    }

    #[tokio::test]
    async fn cancelled_transaction_never_consumes_epoch_capacity_or_result_lease() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let epoch = epoch(EpochId::from_bytes(id16(20))).await;
        let (admission, _) = admitted_runtime(workspace, Arc::clone(&epoch));
        let resources = resource_coordinator(&epoch);
        let runtime = RelationalQueryRuntime::new(
            workspace,
            admission,
            Arc::new(PublishedArrowResultRegistry::new()),
            Arc::clone(&resources),
        );
        let cancellation = Cancellation::with_check_interval(1);
        let mut transaction = transaction(
            &epoch,
            owner(workspace, 0x31),
            &[ALLOWED_RELATION],
            &[ALLOWED_RELATION],
            10_000,
            result_limits(),
            0x51,
            0x61,
            0x71,
        );
        transaction.cancellation = cancellation.clone();
        cancellation.cancel();

        assert!(matches!(
            runtime.execute_and_publish(transaction).await,
            Err(RelationalQueryRuntimeError::ResourceGovernance(
                EpochResourceError::Cancelled
            ))
        ));
        let observation = resources.observation().unwrap();
        assert_eq!(observation.active_work, 0);
        assert_eq!(observation.queued_work, 0);
        assert_eq!(observation.live_result_leases, 0);
        assert_eq!(observation.retained_result_bytes, 0);
    }

    #[tokio::test]
    async fn wp38_claim_015_negative_cancellation_releases_without_publication() {
        let fixture = claim_015_negative_fixture();
        let mutation = &fixture["mutation"];
        assert_eq!(mutation["input_role"], "cancellation_state");
        assert_eq!(mutation["json_pointer"], "");
        assert_eq!(mutation["before"]["cancelled"], false);
        assert_eq!(mutation["after"]["cancelled"], true);

        let workspace = WorkspaceId::from_bytes(id16(1));
        let epoch = epoch(EpochId::from_bytes(id16(20))).await;
        let (admission, _) = admitted_runtime(workspace, Arc::clone(&epoch));
        let resources = resource_coordinator(&epoch);
        let runtime = RelationalQueryRuntime::new(
            workspace,
            admission,
            Arc::new(PublishedArrowResultRegistry::new()),
            Arc::clone(&resources),
        );
        let cancellation = Cancellation::with_check_interval(1);
        let mut transaction = transaction(
            &epoch,
            owner(workspace, 0x31),
            &[ALLOWED_RELATION],
            &[ALLOWED_RELATION],
            10_000,
            result_limits(),
            0x51,
            0x61,
            0x71,
        );
        transaction.cancellation = cancellation.clone();
        cancellation.cancel();
        assert!(matches!(
            runtime.execute_and_publish(transaction).await,
            Err(RelationalQueryRuntimeError::ResourceGovernance(
                EpochResourceError::Cancelled
            ))
        ));

        let observation = resources.observation().unwrap();
        assert_eq!(observation.active_work, 0);
        assert_eq!(observation.queued_work, 0);
        assert_eq!(observation.live_result_leases, 0);
        assert_eq!(observation.retained_result_bytes, 0);
        let expected = &fixture["expected_decoded"];
        assert_eq!(expected["state"], "cancelled");
        assert_eq!(expected["public_error"], "CANCELLED");
        assert_eq!(expected["published_rows"], 0);
        assert_eq!(expected["published_resources"], 0);
        assert!(expected["resource_uri"].is_null());
        assert_eq!(
            expected["terminal_provenance"]["cancellation"],
            mutation["after"]
        );
        assert_eq!(
            expected["terminal_provenance"]["publication_state"],
            "not_published"
        );
    }

    #[tokio::test]
    async fn row_batch_schema_and_ipc_resource_overflow_fail_without_publication() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let epoch = epoch(EpochId::from_bytes(id16(20))).await;
        let (admission, _) = admitted_runtime(workspace, Arc::clone(&epoch));
        let runtime = RelationalQueryRuntime::new(
            workspace,
            admission,
            Arc::new(PublishedArrowResultRegistry::new()),
            resource_coordinator(&epoch),
        );
        let result_owner = owner(workspace, 0x31);

        assert!(matches!(
            runtime
                .execute_and_publish(transaction(
                    &epoch,
                    result_owner,
                    &[ALLOWED_RELATION],
                    &[ALLOWED_RELATION],
                    1,
                    result_limits(),
                    0x51,
                    0x61,
                    0x71,
                ))
                .await,
            Err(RelationalQueryRuntimeError::Child(
                ChildSessionError::ProgramOutputRowLimitExceeded { .. }
            ))
        ));

        let batch_limits = ArrowResultResourceLimits::try_new(
            4,
            4,
            20_000,
            1,
            40_000,
            1 << 20,
            2 << 20,
            1 << 20,
            2 << 20,
            1 << 20,
            37,
        )
        .unwrap();
        assert!(matches!(
            runtime
                .execute_and_publish(transaction(
                    &epoch,
                    result_owner,
                    &[ALLOWED_RELATION, SECOND_RELATION],
                    &[ALLOWED_RELATION, SECOND_RELATION],
                    20_000,
                    batch_limits,
                    0x52,
                    0x62,
                    0x72,
                ))
                .await,
            Err(RelationalQueryRuntimeError::ArrowResult(
                ArrowResultResourceError::TotalBatchLimitExceeded { .. }
            ))
        ));

        let schema_limits = ArrowResultResourceLimits::try_new(
            4,
            4,
            20_000,
            4,
            40_000,
            1,
            2,
            1 << 20,
            2 << 20,
            1 << 20,
            37,
        )
        .unwrap();
        assert!(matches!(
            runtime
                .execute_and_publish(transaction(
                    &epoch,
                    result_owner,
                    &[ALLOWED_RELATION],
                    &[ALLOWED_RELATION],
                    20_000,
                    schema_limits,
                    0x53,
                    0x63,
                    0x73,
                ))
                .await,
            Err(RelationalQueryRuntimeError::ArrowResult(
                ArrowResultResourceError::SchemaByteLimitExceeded { .. }
            ))
        ));

        let ipc_limits = ArrowResultResourceLimits::try_new(
            4,
            4,
            20_000,
            4,
            40_000,
            1 << 20,
            2 << 20,
            1,
            2,
            1 << 20,
            37,
        )
        .unwrap();
        assert!(matches!(
            runtime
                .execute_and_publish(transaction(
                    &epoch,
                    result_owner,
                    &[ALLOWED_RELATION],
                    &[ALLOWED_RELATION],
                    20_000,
                    ipc_limits,
                    0x54,
                    0x64,
                    0x74,
                ))
                .await,
            Err(RelationalQueryRuntimeError::ArrowResult(
                ArrowResultResourceError::IpcByteLimitExceeded { .. }
            ))
        ));
    }

    #[tokio::test]
    async fn result_reads_reauthorize_owner_and_token_and_release_is_terminal() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let epoch = epoch(EpochId::from_bytes(id16(20))).await;
        let (admission, _) = admitted_runtime(workspace, Arc::clone(&epoch));
        let runtime = RelationalQueryRuntime::new(
            workspace,
            admission,
            Arc::new(PublishedArrowResultRegistry::new()),
            resource_coordinator(&epoch),
        );
        let result_owner = owner(workspace, 0x31);
        let publication = runtime
            .execute_and_publish(transaction(
                &epoch,
                result_owner,
                &[ALLOWED_RELATION],
                &[ALLOWED_RELATION],
                10_000,
                result_limits(),
                0x51,
                0x61,
                0x71,
            ))
            .await
            .unwrap();
        let resource_id = publication.descriptor().relations[0].authorization_resource_id;
        let access = PublishedResultAccess {
            artifact_id: publication.descriptor().artifact_id,
            owner: result_owner,
            lease_token: token(0x71),
        };
        let read_request = |access| PublishedResultReadRequest {
            access,
            resource_id,
            observed_at_unix_ms: 1_500,
            offset: 0,
            max_bytes: 37,
        };
        assert!(matches!(
            runtime.read_chunk(read_request(PublishedResultAccess {
                owner: owner(workspace, 0x32),
                ..access
            })),
            Err(RelationalQueryRuntimeError::PublishedResult(
                PublishedResultRegistryError::WrongOwner
            ))
        ));
        assert!(matches!(
            runtime.read_chunk(read_request(PublishedResultAccess {
                lease_token: token(0x72),
                ..access
            })),
            Err(RelationalQueryRuntimeError::PublishedResult(
                PublishedResultRegistryError::WrongOpaqueToken
            ))
        ));
        assert_eq!(
            runtime.release(access, 1_500).unwrap(),
            PublishedReleaseOutcome::Released
        );
        assert!(matches!(
            runtime.read_chunk(read_request(access)),
            Err(RelationalQueryRuntimeError::PublishedResult(
                PublishedResultRegistryError::Released
            ))
        ));
    }

    #[tokio::test]
    async fn published_result_retains_exact_predecessor_epoch_across_swap_until_release() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let first_id = EpochId::from_bytes(id16(20));
        let second_id = EpochId::from_bytes(id16(21));
        let first = epoch(first_id).await;
        let first_weak = Arc::downgrade(&first);
        let second = epoch(second_id).await;
        let (admission, first_event) = admitted_runtime(workspace, Arc::clone(&first));
        let runtime = RelationalQueryRuntime::new(
            workspace,
            Arc::clone(&admission),
            Arc::new(PublishedArrowResultRegistry::new()),
            resource_coordinator(&first),
        );
        let result_owner = owner(workspace, 0x31);
        let publication = runtime
            .execute_and_publish(transaction(
                &first,
                result_owner,
                &[ALLOWED_RELATION],
                &[ALLOWED_RELATION],
                10_000,
                result_limits(),
                0x51,
                0x61,
                0x71,
            ))
            .await
            .unwrap();

        let second_command = command(2, workspace, ExpectedHead::Epoch(first_id), second_id, 1);
        let barrier = admission
            .close_admission(second_command.expected_head, second_command.writer_fence)
            .unwrap();
        let second_event = activation_event(
            2,
            &second_command,
            Some(first_event.event_id()),
            2,
            second_id,
        );
        let second_chain = ActivationChain::derive(workspace, [second_event, first_event]).unwrap();
        admission
            .publish_selected_epoch(barrier, &second_chain, Arc::clone(&second))
            .unwrap();
        admission
            .reopen_after_reconciliation(barrier, ExpectedHead::Epoch(second_id))
            .unwrap();
        drop(first);

        assert!(first_weak.upgrade().is_some());
        assert_eq!(admission.admit().unwrap().epoch_id(), second_id);
        runtime
            .release(
                PublishedResultAccess {
                    artifact_id: publication.descriptor().artifact_id,
                    owner: result_owner,
                    lease_token: token(0x71),
                },
                1_500,
            )
            .unwrap();
        assert!(first_weak.upgrade().is_none());
    }

    #[tokio::test]
    async fn caller_admitted_execution_cannot_mix_compilation_and_execution_epochs() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let first_id = EpochId::from_bytes(id16(20));
        let second_id = EpochId::from_bytes(id16(21));
        let first = epoch(first_id).await;
        let second = epoch(second_id).await;
        let (admission, first_event) = admitted_runtime(workspace, Arc::clone(&first));
        let first_resources = resource_coordinator(&first);
        let runtime = RelationalQueryRuntime::new(
            workspace,
            Arc::clone(&admission),
            Arc::new(PublishedArrowResultRegistry::new()),
            Arc::clone(&first_resources),
        );

        // The semantic compiler pins this lease before activation moves the current head.
        let compiled_epoch_lease = admission.admit().unwrap();
        let second_command = command(2, workspace, ExpectedHead::Epoch(first_id), second_id, 1);
        let barrier = admission
            .close_admission(second_command.expected_head, second_command.writer_fence)
            .unwrap();
        let second_event = activation_event(
            2,
            &second_command,
            Some(first_event.event_id()),
            2,
            second_id,
        );
        let second_chain = ActivationChain::derive(workspace, [second_event, first_event]).unwrap();
        admission
            .publish_selected_epoch(barrier, &second_chain, Arc::clone(&second))
            .unwrap();
        admission
            .reopen_after_reconciliation(barrier, ExpectedHead::Epoch(second_id))
            .unwrap();

        let publication = runtime
            .execute_admitted_and_publish(
                compiled_epoch_lease,
                Arc::clone(&first_resources),
                transaction(
                    &first,
                    owner(workspace, 0x31),
                    &[ALLOWED_RELATION],
                    &[ALLOWED_RELATION],
                    10_000,
                    result_limits(),
                    0x51,
                    0x61,
                    0x71,
                ),
            )
            .await
            .unwrap();
        assert_eq!(publication.descriptor().epoch_id, first_id);
        assert_eq!(admission.admit().unwrap().epoch_id(), second_id);

        let second_lease = admission.admit().unwrap();
        assert!(matches!(
            runtime
                .execute_admitted_and_publish(
                    second_lease,
                    first_resources,
                    transaction(
                        &second,
                        owner(workspace, 0x31),
                        &[ALLOWED_RELATION],
                        &[ALLOWED_RELATION],
                        10_000,
                        result_limits(),
                        0x52,
                        0x62,
                        0x72,
                    ),
                )
                .await,
            Err(RelationalQueryRuntimeError::ResourceEpochMismatch {
                admitted,
                coordinator,
            }) if admitted == second_id && coordinator == first_id
        ));
    }
}
