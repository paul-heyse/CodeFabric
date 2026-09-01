//! Production semantic-query backend over exact programmatic epoch authority.
//!
//! The RPC layer owns authentication and released-envelope validation. This backend admits one
//! immutable epoch before semantic projection, resolves only that epoch's application catalogs,
//! compiles directly by program binding ID, materializes request-owned Arrow relations, derives a
//! reduced child authorization from normalized scope rows, and publishes Arrow resources into the
//! daemon-wide registry. No bootstrap catalog or form-selected executor participates.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;

use crate::cancellation::Cancellation;
use crate::identity::{IdentityDomain, decode_public_id, encode_public_id};
use crate::query_service::{
    PublishedArrowSemanticSuccess, SemanticBackendExecutionContext, SemanticBackendOutcome,
    SemanticQueryBackend,
};
use crate::registries::FreshnessState;
use crate::relational_semantic_query::{
    CompiledEpochBoundScopeHandoff, EpochBoundSemanticExecutionCatalog, EpochBoundSemanticIngress,
    SemanticBlockDisposition, SemanticClauseValue, compile_epoch_bound_semantic_request,
    validate_epoch_bound_semantic_ingress,
};
use crate::semantic_query_contract::{
    COMPILED_V2_0_SCOPE_DEFINITIONS, ParsedSemanticRequest, SemanticQueryError,
    SemanticSnapshotResponse,
};

use super::admission::AdmissionError;
use super::arrow_result_resource::ResultResourceLease;
use super::command::{ExpectedHead, WorkspaceId};
use super::production_kernel::{
    ActiveWorkspaceLease, CompiledPolicyAuthority, CompiledQueryAuthority, CompiledSemanticRelease,
    LifecycleAuthority, WorkspaceSlotRegistry,
};
use super::programmatic_schema::ProgrammaticRelationId;
use super::programmatic_workspace::{ProgrammaticWorkspaceRuntime, WorkspaceEpochQueryAuthority};
use super::published_arrow_result::PublishedResultOwner;
use super::query_artifact::{
    QueryArtifactStage, QueryArtifactStageState, QueryExecutionArtifactAccumulator,
};
use super::relational_query_runtime::{RelationalQueryAuthorization, RelationalQueryTransaction};
use super::request_owned_relation::RequestOwnedRelationCollection;

const REQUEST_CONTENT_PIN_DOMAIN: &[u8] = b"codefabric.programmatic-semantic-request-content.v1\0";
const COMPILED_QUERY_RELEASE_PIN_DOMAIN: &[u8] =
    b"codefabric.compiled-semantic-query-release.v2.0\0";

/// Explicit application port from the released request DTO to normalized epoch-bound relations.
///
/// Implementations are installed by the compiled semantic release. There is intentionally no
/// default implementation and no lookup by released query form inside this backend.
pub trait ProgrammaticSemanticIngressPort: Send + Sync + 'static {
    /// Stable non-sentinel identity of this transformation release.
    fn authority_pin(&self) -> [u8; 32];

    /// Cheap request-shape preflight used before the asynchronous accepted-query task is spawned.
    fn validate_request(
        &self,
        request: &ParsedSemanticRequest,
    ) -> Result<(), ProgrammaticQueryPortError>;

    /// Project one request against exactly the already-admitted epoch authority.
    fn project(
        &self,
        request: &ParsedSemanticRequest,
        workspace: &ProgrammaticWorkspaceRuntime,
        authority: &WorkspaceEpochQueryAuthority,
    ) -> Result<EpochBoundSemanticIngress, ProgrammaticQueryPortError>;
}

/// Application policy port consuming every normalized scope handoff.
pub trait ProgrammaticScopeAuthorizationPort: Send + Sync + 'static {
    /// Exact policy identity implemented by this port.
    fn policy_pin(&self) -> [u8; 32];

    /// Derive the complete reduced-child authorization for one authenticated owner.
    fn authorize(
        &self,
        request: &ParsedSemanticRequest,
        owner: PublishedResultOwner,
        workspace: &ProgrammaticWorkspaceRuntime,
        authority: &WorkspaceEpochQueryAuthority,
        scopes: &[CompiledEpochBoundScopeHandoff],
    ) -> Result<RelationalQueryAuthorization, ProgrammaticQueryPortError>;
}

/// One exact normalized scope outcome admitted by application policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammaticScopeCapabilityGrant {
    authorization_input_id: Arc<str>,
    scope_content_pin: [u8; 32],
    table_relations: BTreeSet<ProgrammaticRelationId>,
    max_output_rows: usize,
}

impl ProgrammaticScopeCapabilityGrant {
    /// Construct a non-empty capability-narrowing rule for one exact scope relation value.
    ///
    /// # Errors
    ///
    /// Rejects absent identities, an empty table subset, or a zero output-row bound.
    pub fn try_new(
        authorization_input_id: impl Into<Arc<str>>,
        scope_content_pin: [u8; 32],
        table_relations: BTreeSet<ProgrammaticRelationId>,
        max_output_rows: usize,
    ) -> Result<Self, ProgrammaticQueryPortError> {
        let authorization_input_id = authorization_input_id.into();
        if authorization_input_id.trim().is_empty()
            || scope_content_pin == [0; 32]
            || table_relations.is_empty()
            || max_output_rows == 0
        {
            return Err(ProgrammaticQueryPortError::Rejected(
                "scope capability rule is incomplete".to_owned(),
            ));
        }
        Ok(Self {
            authorization_input_id,
            scope_content_pin,
            table_relations,
            max_output_rows,
        })
    }
}

/// Exact application policy which can only narrow an epoch's baseline child capabilities.
#[derive(Clone, Debug)]
pub struct ExactProgrammaticScopeAuthorization {
    policy_pin: [u8; 32],
    grants: BTreeMap<(Arc<str>, [u8; 32]), ProgrammaticScopeCapabilityGrant>,
}

impl ExactProgrammaticScopeAuthorization {
    /// Install the complete accepted scope-value relation for one policy release.
    ///
    /// # Errors
    ///
    /// Rejects a missing policy identity, duplicate exact scope keys, or an empty rule set.
    pub fn try_new(
        policy_pin: [u8; 32],
        grants: impl IntoIterator<Item = ProgrammaticScopeCapabilityGrant>,
    ) -> Result<Self, ProgrammaticQueryPortError> {
        if policy_pin == [0; 32] {
            return Err(ProgrammaticQueryPortError::Rejected(
                "scope policy identity is absent".to_owned(),
            ));
        }
        let mut by_scope = BTreeMap::new();
        for grant in grants {
            let key = (
                Arc::clone(&grant.authorization_input_id),
                grant.scope_content_pin,
            );
            if by_scope.insert(key, grant).is_some() {
                return Err(ProgrammaticQueryPortError::Rejected(
                    "scope capability rule is duplicated".to_owned(),
                ));
            }
        }
        if by_scope.is_empty() {
            return Err(ProgrammaticQueryPortError::Rejected(
                "scope policy has no accepted values".to_owned(),
            ));
        }
        Ok(Self {
            policy_pin,
            grants: by_scope,
        })
    }
}

impl ProgrammaticScopeAuthorizationPort for ExactProgrammaticScopeAuthorization {
    fn policy_pin(&self) -> [u8; 32] {
        self.policy_pin
    }

    fn authorize(
        &self,
        request: &ParsedSemanticRequest,
        owner: PublishedResultOwner,
        _workspace: &ProgrammaticWorkspaceRuntime,
        authority: &WorkspaceEpochQueryAuthority,
        scopes: &[CompiledEpochBoundScopeHandoff],
    ) -> Result<RelationalQueryAuthorization, ProgrammaticQueryPortError> {
        let baseline = authority.authorization();
        if baseline.query_policy() != &self.policy_pin {
            return Err(ProgrammaticQueryPortError::Rejected(
                "scope policy differs from the installed baseline authorization".to_owned(),
            ));
        }
        let mut retained_tables = baseline.table_relations().cloned().collect::<BTreeSet<_>>();
        let mut max_output_rows = baseline.max_output_rows();
        let mut consumed_inputs = BTreeSet::new();
        let mut scope_identity = blake3::Hasher::new();
        frame_scope_identity(
            &mut scope_identity,
            b"codefabric.programmatic-scope-authorization.v1",
        );
        frame_scope_identity(&mut scope_identity, &self.policy_pin);
        frame_scope_identity(&mut scope_identity, owner.agent_id().as_bytes());
        frame_scope_identity(
            &mut scope_identity,
            &canonical_request_content_pin(&request.canonical_bytes),
        );
        for scope in scopes {
            if !consumed_inputs.insert(Arc::clone(&scope.authorization_input_id)) {
                return Err(ProgrammaticQueryPortError::Rejected(format!(
                    "authorization input {} is repeated",
                    scope.authorization_input_id
                )));
            }
            let key = (Arc::clone(&scope.authorization_input_id), scope.content_pin);
            let grant = self.grants.get(&key).ok_or_else(|| {
                ProgrammaticQueryPortError::Rejected(format!(
                    "scope {} has no exact application-policy grant",
                    scope.authorization_input_id
                ))
            })?;
            retained_tables.retain(|relation_id| grant.table_relations.contains(relation_id));
            max_output_rows = max_output_rows.min(grant.max_output_rows);
            frame_scope_identity(&mut scope_identity, scope.authorization_input_id.as_bytes());
            frame_scope_identity(&mut scope_identity, &scope.handoff_pin);
            frame_scope_identity(&mut scope_identity, &scope.content_pin);
        }
        let access_scope = *scope_identity.finalize().as_bytes();
        baseline
            .narrow_to(access_scope, &retained_tables, max_output_rows)
            .map_err(|error| ProgrammaticQueryPortError::Rejected(error.to_string()))
    }
}

#[derive(Clone, Debug)]
struct CompiledV20ScopeRule {
    authorization_input_id: Arc<str>,
    handoff_pin: [u8; 32],
}

/// Request-independent application policy for the sole compiled 2.0 scope projection.
///
/// The execution catalog supplies the preinstalled scope identities and handoff pins. At request
/// time this port reconstructs every expected normalized value from the parsed request, validates
/// the compiler handoff row-for-row, and only then narrows the epoch baseline. It never learns a
/// request content pin during backend construction and therefore remains valid for later requests.
#[derive(Clone, Debug)]
pub struct CompiledV20ProgrammaticScopeAuthorization {
    policy_pin: [u8; 32],
    rules: BTreeMap<Arc<str>, CompiledV20ScopeRule>,
    table_relations: BTreeSet<ProgrammaticRelationId>,
    max_output_rows: usize,
}

impl CompiledV20ProgrammaticScopeAuthorization {
    /// Bind the sole compiled 2.0 scope policy to one installed execution catalog.
    ///
    /// # Errors
    ///
    /// Rejects a policy mismatch, an incomplete/duplicate scope set, sentinel handoff pins, an
    /// empty table capability, or a zero result bound.
    pub(crate) fn try_new(
        _compiled_query: &CompiledQueryAuthority,
        _compiled_policy: &CompiledPolicyAuthority,
        policy_pin: [u8; 32],
        execution_catalog: &EpochBoundSemanticExecutionCatalog,
        table_relations: BTreeSet<ProgrammaticRelationId>,
        max_output_rows: usize,
    ) -> Result<Self, ProgrammaticQueryPortError> {
        if policy_pin == [0; 32]
            || execution_catalog.policy_pin != policy_pin
            || table_relations.is_empty()
            || max_output_rows == 0
        {
            return Err(ProgrammaticQueryPortError::Rejected(
                "compiled 2.0 scope policy authority is incomplete".to_owned(),
            ));
        }
        let expected = COMPILED_V2_0_SCOPE_DEFINITIONS
            .into_iter()
            .map(|definition| (definition.scope_id, definition.authorization_input_id))
            .collect::<BTreeMap<_, _>>();
        let mut rules = BTreeMap::new();
        for row in &execution_catalog.scopes {
            if row.handoff_pin == [0; 32]
                || row.authorization_input_id.trim().is_empty()
                || expected.get(row.scope_id.as_ref()).copied()
                    != Some(row.authorization_input_id.as_ref())
                || rules
                    .insert(
                        Arc::clone(&row.scope_id),
                        CompiledV20ScopeRule {
                            authorization_input_id: Arc::clone(&row.authorization_input_id),
                            handoff_pin: row.handoff_pin,
                        },
                    )
                    .is_some()
            {
                return Err(ProgrammaticQueryPortError::Rejected(
                    "compiled 2.0 scope catalog is ambiguous".to_owned(),
                ));
            }
        }
        if rules.keys().map(AsRef::as_ref).collect::<BTreeSet<_>>()
            != expected.keys().copied().collect::<BTreeSet<_>>()
        {
            return Err(ProgrammaticQueryPortError::Rejected(
                "compiled 2.0 scope catalog is incomplete".to_owned(),
            ));
        }
        Ok(Self {
            policy_pin,
            rules,
            table_relations,
            max_output_rows,
        })
    }
}

impl ProgrammaticScopeAuthorizationPort for CompiledV20ProgrammaticScopeAuthorization {
    fn policy_pin(&self) -> [u8; 32] {
        self.policy_pin
    }

    fn authorize(
        &self,
        request: &ParsedSemanticRequest,
        owner: PublishedResultOwner,
        workspace: &ProgrammaticWorkspaceRuntime,
        authority: &WorkspaceEpochQueryAuthority,
        scopes: &[CompiledEpochBoundScopeHandoff],
    ) -> Result<RelationalQueryAuthorization, ProgrammaticQueryPortError> {
        let baseline = authority.authorization();
        if baseline.query_policy() != &self.policy_pin
            || workspace.workspace_id() != authority.workspace_id()
            || request.request.workspace_id
                != workspace
                    .public_workspace_id()
                    .map_err(|error| ProgrammaticQueryPortError::Rejected(error.to_string()))?
        {
            return Err(ProgrammaticQueryPortError::Rejected(
                "compiled 2.0 scope policy differs from admitted authority".to_owned(),
            ));
        }
        let expected = compiled_v2_0_scope_values(request);
        validate_compiled_v2_0_scope_handoffs(&self.rules, &expected, scopes)?;

        let mut scope_identity = blake3::Hasher::new();
        frame_scope_identity(
            &mut scope_identity,
            b"codefabric.compiled-v2.0-scope-authorization.v1",
        );
        frame_scope_identity(&mut scope_identity, &self.policy_pin);
        frame_scope_identity(&mut scope_identity, owner.agent_id().as_bytes());
        frame_scope_identity(
            &mut scope_identity,
            &canonical_request_content_pin(&request.canonical_bytes),
        );
        for scope in scopes {
            frame_scope_identity(&mut scope_identity, scope.scope_id.as_bytes());
            frame_scope_identity(&mut scope_identity, scope.authorization_input_id.as_bytes());
            frame_scope_identity(&mut scope_identity, &scope.handoff_pin);
            frame_scope_identity(&mut scope_identity, &scope.content_pin);
        }

        let retained_tables = baseline
            .table_relations()
            .filter(|relation| self.table_relations.contains(*relation))
            .cloned()
            .collect::<BTreeSet<_>>();
        baseline
            .narrow_to(
                *scope_identity.finalize().as_bytes(),
                &retained_tables,
                baseline.max_output_rows().min(self.max_output_rows),
            )
            .map_err(|error| ProgrammaticQueryPortError::Rejected(error.to_string()))
    }
}

fn compiled_v2_0_scope_values(
    request: &ParsedSemanticRequest,
) -> BTreeMap<&'static str, Vec<SemanticClauseValue>> {
    let request = &request.request;
    let expected = COMPILED_V2_0_SCOPE_DEFINITIONS
        .into_iter()
        .filter_map(|definition| {
            let operands = request.compiled_v2_0_scope_operands(definition.role);
            (!operands.is_empty()).then(|| {
                (
                    definition.scope_id,
                    operands
                        .into_iter()
                        .map(|operand| SemanticClauseValue::Text(Arc::from(operand)))
                        .collect(),
                )
            })
        })
        .collect();
    expected
}

fn validate_compiled_v2_0_scope_handoffs(
    rules: &BTreeMap<Arc<str>, CompiledV20ScopeRule>,
    expected: &BTreeMap<&'static str, Vec<SemanticClauseValue>>,
    scopes: &[CompiledEpochBoundScopeHandoff],
) -> Result<(), ProgrammaticQueryPortError> {
    if scopes.len() != expected.len() {
        return Err(ProgrammaticQueryPortError::Rejected(
            "compiled 2.0 scope handoff set is incomplete".to_owned(),
        ));
    }
    let mut observed = BTreeSet::new();
    for scope in scopes {
        let rule = rules.get(scope.scope_id.as_ref()).ok_or_else(|| {
            ProgrammaticQueryPortError::Rejected(format!(
                "scope {} is outside compiled 2.0 policy",
                scope.scope_id
            ))
        })?;
        let values = expected.get(scope.scope_id.as_ref()).ok_or_else(|| {
            ProgrammaticQueryPortError::Rejected(format!(
                "scope {} was not caused by the released request",
                scope.scope_id
            ))
        })?;
        if !observed.insert(Arc::clone(&scope.scope_id))
            || scope.authorization_input_id != rule.authorization_input_id
            || scope.handoff_pin != rule.handoff_pin
            || scope.rows.len() != values.len()
            || scope
                .rows
                .iter()
                .zip(values)
                .enumerate()
                .any(|(ordinal, (row, expected_value))| {
                    row.scope_id != scope.scope_id
                        || usize::try_from(row.ordinal).ok() != Some(ordinal)
                        || &row.value != expected_value
                })
        {
            return Err(ProgrammaticQueryPortError::Rejected(format!(
                "scope {} differs from the compiled 2.0 request projection",
                scope.scope_id
            )));
        }
    }
    if observed
        != expected
            .keys()
            .map(|scope| Arc::<str>::from(*scope))
            .collect::<BTreeSet<_>>()
    {
        return Err(ProgrammaticQueryPortError::Rejected(
            "compiled 2.0 scope handoff omitted a request scope".to_owned(),
        ));
    }
    Ok(())
}

/// Public, non-authoritative projection of one exact programmatic epoch.
pub trait ProgrammaticSnapshotProjectionPort: Send + Sync + 'static {
    /// Stable non-sentinel identity of the public projection implementation.
    fn authority_pin(&self) -> [u8; 32];

    fn project(
        &self,
        public_workspace_id: &str,
        workspace: &ProgrammaticWorkspaceRuntime,
        authority: &WorkspaceEpochQueryAuthority,
        freshness: FreshnessState,
    ) -> Result<SemanticSnapshotResponse, ProgrammaticQueryPortError>;
}

/// Deterministic public compatibility projection over one exact programmatic epoch.
///
/// Every internal authority value comes from the composed workspace's activation event, exact
/// table vector, release vector, or sealed epoch observation. Compatibility-only strings are
/// explicitly labelled rather than being consulted by any internal planner or selector.
#[derive(Clone, Debug)]
pub struct ExactProgrammaticSnapshotProjection {
    authority_pin: [u8; 32],
}

impl ExactProgrammaticSnapshotProjection {
    // This exact implementation has a stable built-in identity, but the production port bundle
    // must still select it explicitly. Providing `Default` would create an unintended fallback.
    #[allow(clippy::new_without_default)]
    #[must_use]
    pub fn new() -> Self {
        let mut hasher = blake3::Hasher::new();
        frame_scope_identity(
            &mut hasher,
            b"codefabric.programmatic-public-snapshot-projection.v1",
        );
        Self {
            authority_pin: *hasher.finalize().as_bytes(),
        }
    }
}

impl ProgrammaticSnapshotProjectionPort for ExactProgrammaticSnapshotProjection {
    fn authority_pin(&self) -> [u8; 32] {
        self.authority_pin
    }

    fn project(
        &self,
        public_workspace_id: &str,
        workspace: &ProgrammaticWorkspaceRuntime,
        authority: &WorkspaceEpochQueryAuthority,
        freshness: FreshnessState,
    ) -> Result<SemanticSnapshotResponse, ProgrammaticQueryPortError> {
        let startup = workspace.startup_observation();
        let pins = authority.activation_pins();
        if startup.workspace_id != authority.workspace_id()
            || pins.epoch != authority.epoch_id()
            || pins.table_versions
                != authority
                    .epoch()
                    .observation_publication()
                    .table_version_set_ref()
            || pins.resource_envelope.as_bytes() != authority.resources().resource_policy()
            || workspace.admission().active_head() != ExpectedHead::Epoch(authority.epoch_id())
            || public_workspace_id
                != workspace
                    .public_workspace_id()
                    .map_err(|error| ProgrammaticQueryPortError::Rejected(error.to_string()))?
        {
            return Err(ProgrammaticQueryPortError::Rejected(
                "snapshot projection authority differs from the admitted workspace epoch"
                    .to_owned(),
            ));
        }
        let snapshot_id = encode_public_id(
            IdentityDomain::ServingSnapshot,
            None,
            *authority.epoch_id().as_bytes(),
        )
        .map_err(|error| ProgrammaticQueryPortError::Rejected(error.to_string()))?;
        let publication_id = encode_public_id(
            IdentityDomain::Publication,
            None,
            public_id16(
                b"codefabric.programmatic-activation-publication.v1",
                pins.proof_receipt.as_bytes(),
            ),
        )
        .map_err(|error| ProgrammaticQueryPortError::Rejected(error.to_string()))?;
        let application_release = startup.releases.application_release();
        let context_id = public_id16(
            b"codefabric.programmatic-analysis-context.v1",
            application_release.as_bytes(),
        );
        let analysis_context_set_id =
            encode_public_id(IdentityDomain::ContextSet, None, context_id)
                .map_err(|error| ProgrammaticQueryPortError::Rejected(error.to_string()))?;
        let analysis_context_id =
            encode_public_id(IdentityDomain::AnalysisContext, None, context_id)
                .map_err(|error| ProgrammaticQueryPortError::Rejected(error.to_string()))?;
        let mut table_hasher = blake3::Hasher::new();
        frame_scope_identity(
            &mut table_hasher,
            b"codefabric.programmatic-public-table-vector.v1",
        );
        for (relation_id, table) in authority
            .epoch()
            .observation_publication()
            .table_version_set()
            .components()
        {
            frame_scope_identity(&mut table_hasher, relation_id.as_bytes());
            frame_scope_identity(
                &mut table_hasher,
                table.canonical_root().as_str().as_bytes(),
            );
            frame_scope_identity(&mut table_hasher, &table.version().to_be_bytes());
        }
        let mut capability = BTreeMap::new();
        capability.insert("factory_id".to_owned(), startup.factory_id.to_owned());
        capability.insert(
            "program_catalog_pin".to_owned(),
            digest_text(&authority.ingress_catalog().program_catalog_pin),
        );
        capability.insert(
            "producer_closure_proof_pin".to_owned(),
            digest_text(&authority.producer_closure().proof_pin),
        );
        capability.insert(
            "policy_set_pin".to_owned(),
            digest_text(pins.policy_set.as_bytes()),
        );
        capability.insert(
            "proof_receipt_pin".to_owned(),
            digest_text(pins.proof_receipt.as_bytes()),
        );
        Ok(SemanticSnapshotResponse {
            snapshot_id,
            workspace_id: public_workspace_id.to_owned(),
            repository_id: None,
            worktree_id: None,
            source_generation: pins.source_generation.get(),
            source_inventory_digest: digest_text(startup.releases.source_authority().as_bytes()),
            durable_base_publication: publication_id,
            base_table_version_digest: digest_text(table_hasher.finalize().as_bytes()),
            // The target has one immutable overlay-set pin, not a mutable overlay generation.
            overlay_generation: 0,
            overlay_checksum: digest_text(pins.overlay_segments.as_bytes()),
            analysis_context_set_id,
            analysis_context_ids: vec![analysis_context_id],
            freshness_state: freshness,
            source_trust_state: "EXACT_TYPED_INPUTS".to_owned(),
            event_stream_health: "ACTIVATION_CHAIN_SELECTED".to_owned(),
            git_acceleration_status: "NON_AUTHORITY".to_owned(),
            git_operation_summary: None,
            pending_update_count: 0,
            ontology_version: "not-applicable-programmatic-authority".to_owned(),
            schema_bundle_version: authority.epoch().schema_authority_id().to_owned(),
            provider_bundle_version: digest_text(pins.provider_set.as_bytes()),
            derivation_bundle_version: digest_text(
                startup.releases.application_release().as_bytes(),
            ),
            query_language_version: "2.0".to_owned(),
            capability_summaries: vec![capability],
            diagnostic_references: Vec::new(),
        })
    }
}

/// Complete, explicit backend ports. No member has a fallback or `Default` implementation.
pub struct ProgrammaticSemanticQueryPorts {
    application_release: [u8; 32],
    ingress: Arc<dyn ProgrammaticSemanticIngressPort>,
    scope_authorization: Arc<dyn ProgrammaticScopeAuthorizationPort>,
    snapshot: Arc<dyn ProgrammaticSnapshotProjectionPort>,
}

impl fmt::Debug for ProgrammaticSemanticQueryPorts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgrammaticSemanticQueryPorts")
            .field("application_release", &"REDACTED_IDENTITY")
            .field("ingress", &"installed")
            .field("scope_authorization", &"installed")
            .field("snapshot", &"installed")
            .finish()
    }
}

impl ProgrammaticSemanticQueryPorts {
    /// Construct ports only under the compiled release and when every implementation names a
    /// real release/policy identity.
    pub(crate) fn try_new(
        compiled_release: &CompiledQueryAuthority,
        ingress: Arc<dyn ProgrammaticSemanticIngressPort>,
        scope_authorization: Arc<dyn ProgrammaticScopeAuthorizationPort>,
        snapshot: Arc<dyn ProgrammaticSnapshotProjectionPort>,
    ) -> Result<Self, ProgrammaticSemanticQueryBackendError> {
        let application_release = compiled_query_release_pin(compiled_release);
        for (kind, pin) in [
            ("semantic ingress", ingress.authority_pin()),
            ("scope authorization", scope_authorization.policy_pin()),
            ("snapshot projection", snapshot.authority_pin()),
        ] {
            if pin == [0; 32] {
                return Err(ProgrammaticSemanticQueryBackendError::MissingPortPin(kind));
            }
        }
        if ingress.authority_pin() != application_release {
            return Err(ProgrammaticSemanticQueryBackendError::PortReleaseMismatch(
                "semantic ingress",
            ));
        }
        Ok(Self {
            application_release,
            ingress,
            scope_authorization,
            snapshot,
        })
    }

    #[must_use]
    pub const fn application_release(&self) -> [u8; 32] {
        self.application_release
    }
}

/// Read-only query routing over target-owned active-workspace and lifecycle authority.
pub struct ProgrammaticSemanticQueryBackend {
    workspace_slots: Arc<WorkspaceSlotRegistry>,
    lifecycle: Arc<LifecycleAuthority>,
    published_results: Arc<super::published_arrow_result::PublishedArrowResultRegistry>,
    ports: ProgrammaticSemanticQueryPorts,
}

impl fmt::Debug for ProgrammaticSemanticQueryBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgrammaticSemanticQueryBackend")
            .field("workspace_count", &self.workspace_slots.len())
            .field("lifecycle", &self.lifecycle.observe())
            .field("ports", &self.ports)
            .finish_non_exhaustive()
    }
}

impl ProgrammaticSemanticQueryBackend {
    fn require_semantic_admission(&self) -> Result<(), SemanticQueryError> {
        if self.lifecycle.observe().semantic_admission_open() {
            Ok(())
        } else {
            Err(query_error(
                "semantic_admission",
                "production lifecycle authority has not opened semantic admission",
            ))
        }
    }

    #[must_use]
    pub const fn published_results(
        &self,
    ) -> &Arc<super::published_arrow_result::PublishedArrowResultRegistry> {
        &self.published_results
    }

    /// Lease the exact active workspace selected by the target-owned slot registry.
    ///
    /// Release and policy validation deliberately happens after the lease is acquired: a later
    /// slot swap cannot change this request's runtime while these checks and its execution run.
    fn workspace_lease(
        &self,
        public_workspace_id: &str,
    ) -> Result<ActiveWorkspaceLease, SemanticQueryError> {
        let workspace_id = decode_public_id(IdentityDomain::Workspace, None, public_workspace_id)
            .map(WorkspaceId::from_bytes)
            .map_err(|error| query_error("workspace_route", error.to_string()))?;
        let slot = self.workspace_slots.slot(workspace_id).ok_or_else(|| {
            query_error(
                "workspace_route",
                "workspace is absent from the production slot registry",
            )
        })?;
        let lease = slot
            .lease()
            .map_err(|error| query_error("workspace_route", error.to_string()))?;
        let workspace = lease.workspace().runtime();
        let startup = workspace.startup_observation();
        if *startup.releases.application_release().as_bytes() != self.ports.application_release() {
            return Err(query_error(
                "application_release",
                "active workspace differs from the installed query application release",
            ));
        }
        let authority = workspace
            .query_authorities()
            .resolve(lease.workspace().selection().epoch_id())
            .map_err(|error| query_error("epoch_authority", error.to_string()))?;
        if authority.authorization().query_policy() != &self.ports.scope_authorization.policy_pin()
        {
            return Err(query_error(
                "scope_policy",
                "active workspace differs from the installed query-policy release",
            ));
        }
        Ok(lease)
    }

    fn project_snapshot(
        &self,
        public_workspace_id: &str,
        workspace: &ProgrammaticWorkspaceRuntime,
        authority: &WorkspaceEpochQueryAuthority,
        freshness: FreshnessState,
    ) -> Result<SemanticSnapshotResponse, SemanticQueryError> {
        let snapshot = self
            .ports
            .snapshot
            .project(public_workspace_id, workspace, authority, freshness)
            .map_err(|error| query_error("snapshot_projection", error.to_string()))?;
        if snapshot.workspace_id != public_workspace_id
            || snapshot.freshness_state != freshness
            || snapshot.snapshot_id.is_empty()
            || snapshot.source_inventory_digest.is_empty()
            || snapshot.durable_base_publication.is_empty()
            || snapshot.base_table_version_digest.is_empty()
        {
            return Err(query_error(
                "snapshot_projection",
                "programmatic public snapshot differs from its admitted authority",
            ));
        }
        Ok(snapshot)
    }
}

#[async_trait]
impl SemanticQueryBackend for ProgrammaticSemanticQueryBackend {
    fn validate_execution_request(
        &self,
        request: &ParsedSemanticRequest,
    ) -> Result<(), SemanticQueryError> {
        self.require_semantic_admission()?;
        let workspace_lease = self.workspace_lease(&request.request.workspace_id)?;
        let validation = self
            .ports
            .ingress
            .validate_request(request)
            .map_err(|error| query_error("programmatic_ingress", error.to_string()));
        drop(workspace_lease);
        validation
    }

    async fn execute(
        &self,
        request: ParsedSemanticRequest,
        freshness: FreshnessState,
        cancellation: Cancellation,
        context: SemanticBackendExecutionContext,
        artifacts: QueryExecutionArtifactAccumulator,
    ) -> SemanticBackendOutcome {
        if cancellation.is_cancelled() {
            return cancelled(
                &artifacts,
                "admission",
                "query was cancelled before admission",
            );
        }
        if let Err(error) = self.require_semantic_admission() {
            return failed_error(&artifacts, "semantic_admission", error);
        }
        if request.request.workspace_id != context.workspace_id() {
            return failed(
                &artifacts,
                "workspace_route",
                "authenticated workspace differs from request workspace",
            );
        }
        let workspace_lease = match self.workspace_lease(context.workspace_id()) {
            Ok(workspace) => workspace,
            Err(error) => return failed_error(&artifacts, "workspace_route", error),
        };
        let workspace = workspace_lease.workspace().runtime();
        let context_registry = context.published_results();
        if !Arc::ptr_eq(&context_registry, &self.published_results)
            || !Arc::ptr_eq(workspace.published_results(), &self.published_results)
        {
            return failed(
                &artifacts,
                "result_authority",
                "query and workspace do not share the daemon result registry",
            );
        }

        let epoch_lease = match workspace.admission().admit() {
            Ok(lease) => lease,
            Err(AdmissionError::NoActiveEpoch) => {
                return failed(&artifacts, "admission", "workspace has no active epoch");
            }
            Err(error) => return failed(&artifacts, "admission", error.to_string()),
        };
        let authority = match workspace
            .query_authorities()
            .resolve(epoch_lease.epoch_id())
        {
            Ok(authority) => authority,
            Err(error) => return failed(&artifacts, "epoch_authority", error.to_string()),
        };
        if !Arc::ptr_eq(authority.epoch(), epoch_lease.epoch()) {
            return failed(
                &artifacts,
                "epoch_authority",
                "admission and query authority retain different epoch capabilities",
            );
        }
        // Project control metadata before publishing any Arrow resource. A projection failure can
        // therefore terminate without leaving a live registry entry that the legacy failure path
        // cannot authenticate and release.
        let snapshot = match self.project_snapshot(
            context.workspace_id(),
            workspace.as_ref(),
            &authority,
            freshness,
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => return failed_error(&artifacts, "snapshot_projection", error),
        };

        artifacts.set_phase("semantic_binding");
        let ingress = match self
            .ports
            .ingress
            .project(&request, workspace.as_ref(), &authority)
        {
            Ok(ingress) => ingress,
            Err(error) => return failed(&artifacts, "programmatic_ingress", error.to_string()),
        };
        if ingress.semantic_request_id.as_ref() != request.request.semantic_request_id
            || ingress.request_content_pin
                != canonical_request_content_pin(&request.canonical_bytes)
        {
            return failed(
                &artifacts,
                "programmatic_ingress",
                "epoch-bound ingress is not causally bound to the accepted canonical request",
            );
        }
        let validated =
            match validate_epoch_bound_semantic_ingress(ingress, authority.ingress_catalog()) {
                Ok(validated) => validated,
                Err(error) => return failed(&artifacts, "semantic_binding", error.to_string()),
            };
        let compiled = match compile_epoch_bound_semantic_request(
            &validated,
            authority.execution_catalog(),
            authority.producer_closure(),
        ) {
            Ok(compiled) => compiled,
            Err(error) => return failed(&artifacts, "logical_planning", error.to_string()),
        };
        record_complete_stage(
            &artifacts,
            "binding",
            [("semantic_blocks", compiled.compiled().blocks().len())],
        );

        let (compiled, handoff) = compiled.into_parts();
        let mut outputs = Vec::with_capacity(compiled.blocks().len());
        let mut output_by_query = BTreeMap::new();
        for block in compiled.blocks() {
            if block.disposition() != SemanticBlockDisposition::Compiled {
                return failed(
                    &artifacts,
                    "logical_planning",
                    format!(
                        "query block {} is not executable: {:?} {:?}",
                        block.query_id(),
                        block.disposition(),
                        block.issues()
                    ),
                );
            }
            let Some(output) = block.output().cloned() else {
                return failed(
                    &artifacts,
                    "logical_planning",
                    format!(
                        "compiled query block {} has no selected output",
                        block.query_id()
                    ),
                );
            };
            if output_by_query
                .insert(Arc::clone(block.query_id()), output.relation_id().clone())
                .is_some()
            {
                return failed(
                    &artifacts,
                    "logical_planning",
                    format!("compiled query block {} is duplicated", block.query_id()),
                );
            }
            outputs.push(output);
        }
        if outputs.is_empty() {
            return failed(
                &artifacts,
                "logical_planning",
                "epoch-bound compiler produced no executable outputs",
            );
        }
        if self.ports.scope_authorization.policy_pin() != handoff.policy_pin {
            return failed(
                &artifacts,
                "scope_authorization",
                "scope authorization port differs from the compiled policy pin",
            );
        }
        let authorization = match self.ports.scope_authorization.authorize(
            &request,
            context.owner(),
            workspace.as_ref(),
            &authority,
            &handoff.scopes,
        ) {
            Ok(authorization) => authorization,
            Err(error) => return failed(&artifacts, "scope_authorization", error.to_string()),
        };
        if authorization.query_policy() != &handoff.policy_pin {
            return failed(
                &artifacts,
                "scope_authorization",
                "scope authorization returned a query-policy identity outside the compiled handoff",
            );
        }
        if authorization.resource_policy() != authority.resources().resource_policy() {
            return failed(
                &artifacts,
                "scope_authorization",
                "scope authorization returned a resource-policy identity outside the admitted epoch",
            );
        }
        let mut handoffs_by_output = BTreeMap::new();
        for request_input in handoff.request_inputs {
            let Some(output_relation) = output_by_query.get(&request_input.query_id).cloned()
            else {
                return failed(
                    &artifacts,
                    "request_input",
                    format!(
                        "request input {} names unknown query {}",
                        request_input.input_id, request_input.query_id
                    ),
                );
            };
            handoffs_by_output
                .entry(output_relation)
                .or_insert_with(Vec::new)
                .push(request_input);
        }
        let mut request_inputs_by_output = Vec::with_capacity(handoffs_by_output.len());
        let mut request_owned_relation_count = 0_usize;
        for (output_relation, request_handoffs) in handoffs_by_output {
            let inputs = match RequestOwnedRelationCollection::try_materialize(
                request_handoffs,
                authority.request_owned_relation_limits(),
            ) {
                Ok(inputs) => inputs,
                Err(error) => return failed(&artifacts, "request_input", error.to_string()),
            };
            request_owned_relation_count =
                request_owned_relation_count.saturating_add(inputs.len());
            request_inputs_by_output.push((output_relation, Arc::new(inputs)));
        }
        record_complete_stage(
            &artifacts,
            "logical_planning",
            [
                ("selected_outputs", outputs.len()),
                ("request_owned_relations", request_owned_relation_count),
                ("scope_handoffs", handoff.scopes.len()),
            ],
        );

        let issued_at = crate::query_service::now_millis();
        let lease_duration = match i64::try_from(authority.result_lease_millis()) {
            Ok(duration) => duration,
            Err(_) => return failed(&artifacts, "result_lease", "result lease is too large"),
        };
        let Some(expires_at) = issued_at.checked_add(lease_duration) else {
            return failed(
                &artifacts,
                "result_lease",
                "result lease timestamp overflows",
            );
        };
        let result_lease =
            match ResultResourceLease::try_new(context.result_lease_id(), issued_at, expires_at) {
                Ok(lease) => lease,
                Err(error) => return failed(&artifacts, "result_lease", error.to_string()),
            };
        let transaction = match RelationalQueryTransaction::try_new(
            context.owner(),
            context.query_execution_pin(),
            authorization,
            outputs,
            result_lease,
            context.result_lease_token(),
            authority.result_limits(),
            issued_at,
            cancellation.clone(),
        ) {
            Ok(transaction) => transaction,
            Err(error) => return failed(&artifacts, "logical_planning", error.to_string()),
        };
        let transaction = if request_inputs_by_output.is_empty() {
            transaction
        } else {
            match transaction.with_request_inputs_by_output(request_inputs_by_output) {
                Ok(transaction) => transaction,
                Err(error) => return failed(&artifacts, "request_input", error.to_string()),
            }
        };
        if cancellation.is_cancelled() {
            return cancelled(
                &artifacts,
                "physical_execution",
                "query was cancelled before execution",
            );
        }
        artifacts.set_phase("physical_execution");
        let publication = match workspace
            .query_runtime()
            .execute_admitted_and_publish(
                epoch_lease,
                Arc::clone(authority.resources()),
                transaction,
            )
            .await
        {
            Ok(publication) => publication,
            Err(error) if cancellation.is_cancelled() => {
                return cancelled(&artifacts, "physical_execution", error.to_string());
            }
            Err(error) => return failed(&artifacts, "physical_execution", error.to_string()),
        };
        record_complete_stage(
            &artifacts,
            "physical_execution",
            [
                ("result_relations", publication.output_observations().len()),
                ("result_rows", publication.descriptor().total_rows as usize),
            ],
        );
        artifacts.set_phase("published_arrow");
        record_complete_stage(&artifacts, "response_encoding", []);
        artifacts.record_coverage("result_rows", publication.descriptor().total_rows);
        SemanticBackendOutcome::PublishedArrow(PublishedArrowSemanticSuccess::new(
            publication,
            context.result_lease_token(),
            snapshot,
            artifacts.snapshot(),
        ))
    }

    async fn public_snapshot(
        &self,
        workspace_id: &str,
    ) -> Result<SemanticSnapshotResponse, SemanticQueryError> {
        self.require_semantic_admission()?;
        let workspace_lease = self.workspace_lease(workspace_id)?;
        let workspace = workspace_lease.workspace().runtime();
        let lease = workspace
            .admission()
            .admit()
            .map_err(|error| query_error("admission", error.to_string()))?;
        let authority = workspace
            .query_authorities()
            .resolve(lease.epoch_id())
            .map_err(|error| query_error("epoch_authority", error.to_string()))?;
        if !Arc::ptr_eq(authority.epoch(), lease.epoch()) {
            return Err(query_error(
                "epoch_authority",
                "admission and query authority retain different epoch capabilities",
            ));
        }
        self.project_snapshot(
            workspace_id,
            workspace.as_ref(),
            &authority,
            FreshnessState::Current,
        )
    }
}

/// Deterministic pin of the already canonical released request bytes.
#[must_use]
pub fn canonical_request_content_pin(canonical_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    for part in [REQUEST_CONTENT_PIN_DOMAIN, canonical_bytes] {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

/// Canonical application-release identity of the sole compiled semantic-query release.
///
/// The opaque capability prevents callers from selecting a suite or query-language mapping. The
/// identity is derived solely from the compiled suite and the sole released query language; it is
/// never accepted as an operational input.
#[must_use]
pub(crate) fn compiled_query_release_pin(_authority: &CompiledQueryAuthority) -> [u8; 32] {
    let suite = CompiledSemanticRelease::current().suite();
    let mut hasher = blake3::Hasher::new();
    for part in [
        COMPILED_QUERY_RELEASE_PIN_DOMAIN,
        suite.suite_id().as_bytes(),
        suite.suite_version().as_bytes(),
        b"composable semantic CPG fact query".as_slice(),
        b"2.0".as_slice(),
    ] {
        frame_scope_identity(&mut hasher, part);
    }
    *hasher.finalize().as_bytes()
}

fn frame_scope_identity(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn digest_text(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(67);
    text.push_str("b3:");
    for byte in bytes {
        text.push(char::from(HEX[usize::from(byte >> 4)]));
        text.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    text
}

fn public_id16(domain: &[u8], authority: &[u8]) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    frame_scope_identity(&mut hasher, domain);
    frame_scope_identity(&mut hasher, authority);
    let mut id = [0_u8; 16];
    id.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    id
}

fn record_complete_stage<const N: usize>(
    artifacts: &QueryExecutionArtifactAccumulator,
    stage: &str,
    metrics: [(&str, usize); N],
) {
    artifacts.record_stage(QueryArtifactStage {
        block_id: "request".to_owned(),
        stage: stage.to_owned(),
        state: QueryArtifactStageState::Complete,
        artifact: None,
        unavailable_reason: None,
        metrics: metrics
            .into_iter()
            .map(|(name, value)| (name.to_owned(), u64::try_from(value).unwrap_or(u64::MAX)))
            .collect(),
    });
}

fn failed(
    artifacts: &QueryExecutionArtifactAccumulator,
    stage: &str,
    message: impl Into<String>,
) -> SemanticBackendOutcome {
    failed_error(artifacts, stage, query_error(stage, message))
}

fn failed_error(
    artifacts: &QueryExecutionArtifactAccumulator,
    stage: &str,
    error: SemanticQueryError,
) -> SemanticBackendOutcome {
    artifacts.set_phase("failed");
    artifacts.set_failure(stage);
    SemanticBackendOutcome::Failed {
        error,
        evidence: artifacts.snapshot(),
    }
}

fn cancelled(
    artifacts: &QueryExecutionArtifactAccumulator,
    stage: &str,
    message: impl Into<String>,
) -> SemanticBackendOutcome {
    artifacts.set_phase("cancelled");
    artifacts.set_failure(stage);
    SemanticBackendOutcome::Cancelled {
        error: query_error(stage, message),
        evidence: artifacts.snapshot(),
    }
}

fn query_error(stage: &str, message: impl Into<String>) -> SemanticQueryError {
    SemanticQueryError::Phase {
        code: "PROGRAMMATIC_QUERY_REJECTED",
        phase: "programmatic_query",
        pointer: stage.to_owned(),
        message: message.into(),
    }
}

/// Explicit construction/projection failures raised by application ports.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProgrammaticQueryPortError {
    #[error("programmatic query port rejected input: {0}")]
    Rejected(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ProgrammaticSemanticQueryBackendError {
    #[error("programmatic query {0} port has no authority pin")]
    MissingPortPin(&'static str),
    #[error("programmatic query {0} port does not belong to the compiled release")]
    PortReleaseMismatch(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relational_semantic_query::EpochBoundScopeRow;
    use crate::semantic_query_contract::parse_request;

    fn parsed_scope_request(include_optional_scopes: bool) -> ParsedSemanticRequest {
        let scope = if include_optional_scopes {
            serde_json::json!({
                "workspace_id": "workspace:00112233445566778899aabbccddeeff",
                "codebase": "codebase:current",
                "languages": ["Rust", "Python"],
                "source_boundaries": [{"root": "src", "kind": "path"}],
                "analysis_contexts": {
                    "mode": "explicit",
                    "context_ids": ["analysis:one"]
                },
                "representations": ["syntax"],
                "external_entities": "endpoint-only"
            })
        } else {
            serde_json::json!({
                "workspace_id": "workspace:00112233445566778899aabbccddeeff"
            })
        };
        let value = serde_json::json!({
            "specification": "composable semantic CPG fact query",
            "version": "2.0",
            "semantic_request_id": "request.scope-causality",
            "scope": scope,
            "freshness": {"policy": "best_available_snapshot"},
            "queries": [{
                "request": "find code entities",
                "query_id": "q1",
                "looking_for": "functions",
                "within": [],
                "where": [],
                "return": {"limit": {"maximum_results": 1}}
            }]
        });
        parse_request(&serde_json::to_vec(&value).expect("request JSON"))
            .expect("compiled v2 request")
    }

    fn scope_handoff_fixture(
        request: &ParsedSemanticRequest,
    ) -> (
        BTreeMap<Arc<str>, CompiledV20ScopeRule>,
        BTreeMap<&'static str, Vec<SemanticClauseValue>>,
        Vec<CompiledEpochBoundScopeHandoff>,
    ) {
        let expected = compiled_v2_0_scope_values(request);
        let mut rules = BTreeMap::new();
        let mut scopes = Vec::new();
        for (index, definition) in COMPILED_V2_0_SCOPE_DEFINITIONS.into_iter().enumerate() {
            let handoff_pin = [u8::try_from(index + 1).expect("eight scopes"); 32];
            rules.insert(
                Arc::from(definition.scope_id),
                CompiledV20ScopeRule {
                    authorization_input_id: Arc::from(definition.authorization_input_id),
                    handoff_pin,
                },
            );
            let Some(values) = expected.get(definition.scope_id) else {
                continue;
            };
            let scope_id = Arc::<str>::from(definition.scope_id);
            scopes.push(CompiledEpochBoundScopeHandoff {
                scope_id: Arc::clone(&scope_id),
                authorization_input_id: Arc::from(definition.authorization_input_id),
                rows: values
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(ordinal, value)| EpochBoundScopeRow {
                        scope_id: Arc::clone(&scope_id),
                        ordinal: u32::try_from(ordinal).expect("bounded scope ordinal"),
                        value,
                    })
                    .collect(),
                handoff_pin,
                content_pin: [u8::try_from(index + 11).expect("eight scopes"); 32],
            });
        }
        (rules, expected, scopes)
    }

    struct IngressProbe([u8; 32]);

    impl ProgrammaticSemanticIngressPort for IngressProbe {
        fn authority_pin(&self) -> [u8; 32] {
            self.0
        }

        fn validate_request(
            &self,
            _request: &ParsedSemanticRequest,
        ) -> Result<(), ProgrammaticQueryPortError> {
            Err(ProgrammaticQueryPortError::Rejected(
                "probe is construction-only".to_owned(),
            ))
        }

        fn project(
            &self,
            _request: &ParsedSemanticRequest,
            _workspace: &ProgrammaticWorkspaceRuntime,
            _authority: &WorkspaceEpochQueryAuthority,
        ) -> Result<EpochBoundSemanticIngress, ProgrammaticQueryPortError> {
            Err(ProgrammaticQueryPortError::Rejected(
                "probe is construction-only".to_owned(),
            ))
        }
    }

    struct ScopeProbe([u8; 32]);

    impl ProgrammaticScopeAuthorizationPort for ScopeProbe {
        fn policy_pin(&self) -> [u8; 32] {
            self.0
        }

        fn authorize(
            &self,
            _request: &ParsedSemanticRequest,
            _owner: PublishedResultOwner,
            _workspace: &ProgrammaticWorkspaceRuntime,
            _authority: &WorkspaceEpochQueryAuthority,
            _scopes: &[CompiledEpochBoundScopeHandoff],
        ) -> Result<RelationalQueryAuthorization, ProgrammaticQueryPortError> {
            Err(ProgrammaticQueryPortError::Rejected(
                "probe is construction-only".to_owned(),
            ))
        }
    }

    struct SnapshotProbe([u8; 32]);

    impl ProgrammaticSnapshotProjectionPort for SnapshotProbe {
        fn authority_pin(&self) -> [u8; 32] {
            self.0
        }

        fn project(
            &self,
            _public_workspace_id: &str,
            _workspace: &ProgrammaticWorkspaceRuntime,
            _authority: &WorkspaceEpochQueryAuthority,
            _freshness: FreshnessState,
        ) -> Result<SemanticSnapshotResponse, ProgrammaticQueryPortError> {
            Err(ProgrammaticQueryPortError::Rejected(
                "probe is construction-only".to_owned(),
            ))
        }
    }

    fn probes(
        ingress: [u8; 32],
        policy: [u8; 32],
        snapshot: [u8; 32],
    ) -> Result<ProgrammaticSemanticQueryPorts, ProgrammaticSemanticQueryBackendError> {
        ProgrammaticSemanticQueryPorts::try_new(
            super::super::production_kernel::CompiledSemanticRelease::current().query_authority(),
            Arc::new(IngressProbe(ingress)),
            Arc::new(ScopeProbe(policy)),
            Arc::new(SnapshotProbe(snapshot)),
        )
    }

    #[test]
    fn port_bundle_requires_application_and_every_component_identity() {
        let release = super::super::production_kernel::CompiledSemanticRelease::current();
        let release_pin = compiled_query_release_pin(release.query_authority());
        for (expected, pins) in [
            ("semantic ingress", ([0; 32], [2; 32], [3; 32])),
            ("scope authorization", (release_pin, [0; 32], [3; 32])),
            ("snapshot projection", (release_pin, [2; 32], [0; 32])),
        ] {
            assert!(matches!(
                probes(pins.0, pins.1, pins.2),
                Err(ProgrammaticSemanticQueryBackendError::MissingPortPin(kind)) if kind == expected
            ));
        }
        assert!(matches!(
            probes([1; 32], [2; 32], [3; 32]),
            Err(ProgrammaticSemanticQueryBackendError::PortReleaseMismatch(
                "semantic ingress"
            ))
        ));
        let ports = probes(release_pin, [2; 32], [3; 32]).unwrap();
        assert_eq!(
            ports.application_release(),
            compiled_query_release_pin(release.query_authority())
        );
        assert_ne!(ports.application_release(), [0; 32]);
    }

    #[test]
    fn lifecycle_authority_is_the_only_semantic_admission_gate() {
        use super::super::production_kernel::ProductionLifecyclePhase;

        let lifecycle = Arc::new(LifecycleAuthority::new());
        let release = super::super::production_kernel::CompiledSemanticRelease::current();
        let backend = ProgrammaticSemanticQueryBackend {
            workspace_slots: Arc::new(WorkspaceSlotRegistry::new()),
            lifecycle: Arc::clone(&lifecycle),
            published_results: Arc::new(
                super::super::published_arrow_result::PublishedArrowResultRegistry::new(),
            ),
            ports: probes(
                compiled_query_release_pin(release.query_authority()),
                [2; 32],
                [3; 32],
            )
            .unwrap(),
        };

        let error = backend.require_semantic_admission().unwrap_err();
        assert!(matches!(
            error,
            SemanticQueryError::Phase { ref pointer, .. } if pointer == "semantic_admission"
        ));

        for (expected, next) in [
            (
                ProductionLifecyclePhase::Configured,
                ProductionLifecyclePhase::DaemonLeased,
            ),
            (
                ProductionLifecyclePhase::DaemonLeased,
                ProductionLifecyclePhase::WriterFenced,
            ),
            (
                ProductionLifecyclePhase::WriterFenced,
                ProductionLifecyclePhase::EndpointsBoundBootstrapping,
            ),
            (
                ProductionLifecyclePhase::EndpointsBoundBootstrapping,
                ProductionLifecyclePhase::SoleTargetAuthorityObserved,
            ),
            (
                ProductionLifecyclePhase::SoleTargetAuthorityObserved,
                ProductionLifecyclePhase::SoleTargetAuthorityCommitted,
            ),
            (
                ProductionLifecyclePhase::SoleTargetAuthorityCommitted,
                ProductionLifecyclePhase::Ready,
            ),
        ] {
            lifecycle.advance(expected, next).unwrap();
        }
        backend.require_semantic_admission().unwrap();

        lifecycle
            .advance(
                ProductionLifecyclePhase::Ready,
                ProductionLifecyclePhase::Draining,
            )
            .unwrap();
        assert!(backend.require_semantic_admission().is_err());
    }

    #[test]
    fn compiled_v2_scope_authorization_rejects_omitted_or_changed_causal_operands() {
        let request = parsed_scope_request(true);
        let (rules, expected, scopes) = scope_handoff_fixture(&request);
        validate_compiled_v2_0_scope_handoffs(&rules, &expected, &scopes)
            .expect("unchanged causal scope handoffs");

        let mut omitted = scopes.clone();
        omitted.retain(|scope| scope.scope_id.as_ref() != "scope.workspace-id");
        let omitted_error = validate_compiled_v2_0_scope_handoffs(&rules, &expected, &omitted)
            .expect_err("omitted request scope must be rejected");
        assert!(
            omitted_error
                .to_string()
                .contains("handoff set is incomplete")
        );

        let mut changed = scopes;
        let workspace = changed
            .iter_mut()
            .find(|scope| scope.scope_id.as_ref() == "scope.workspace-id")
            .expect("workspace scope");
        workspace.rows[0].value = SemanticClauseValue::Text(Arc::from("workspace:changed"));
        let changed_error = validate_compiled_v2_0_scope_handoffs(&rules, &expected, &changed)
            .expect_err("changed request scope must be rejected");
        assert!(
            changed_error
                .to_string()
                .contains("differs from the compiled 2.0 request projection")
        );

        let minimal = parsed_scope_request(false);
        let (_, minimal_expected, minimal_scopes) = scope_handoff_fixture(&minimal);
        assert_eq!(
            minimal_expected.keys().copied().collect::<Vec<_>>(),
            ["scope.workspace-id"]
        );
        assert_eq!(minimal_scopes.len(), 1);
    }
}
