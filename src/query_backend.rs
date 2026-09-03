//! Application-owned semantic execution port consumed by the public v2 transport.
//!
//! Protobuf owns control framing only. The programmatic relational backend receives one already
//! authenticated operation through these types and remains the sole owner of semantic parsing,
//! planning, execution, and Arrow publication.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use crate::cancellation::Cancellation;
use crate::fabric::arrow_result_resource::QueryExecutionPin;
use crate::fabric::command::LeaseId;
use crate::fabric::published_arrow_result::{OpaqueResultLeaseToken, PublishedResultOwner};
use crate::fabric::relational_query_runtime::StreamedRelationalQueryPublication;
use crate::fabric::streamed_result_package::{
    ResultPublicationIntentRecorder, StreamedResultPackageBuilder,
};
use crate::fabric::{
    QueryExecutionArtifactAccumulator, QueryExecutionArtifactEvidence, QueryExecutionContext,
};
use crate::freshness::FreshnessState;
use crate::semantic_query_contract::{
    ParsedSemanticRequest, SemanticQueryError, SemanticSnapshotResponse,
};

/// Application-owned guarded-input kind; transport enums are mapped only at the RPC edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticInputKind {
    String,
    Integer,
    Boolean,
    Enum,
    StringCollection,
    IntegerCollection,
    BooleanCollection,
    EnumCollection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticStringFormat {
    Plain,
    Identifier,
    ReleaseVersion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticCollectionItemKind {
    String,
    Integer,
    Boolean,
    Enum,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticInputConstraints {
    String {
        minimum_length: Option<u32>,
        maximum_length: Option<u32>,
        format: SemanticStringFormat,
    },
    Integer {
        minimum: Option<i64>,
        maximum: Option<i64>,
    },
    Enum {
        minimum_selections: u32,
        maximum_selections: u32,
    },
    Collection {
        item_kind: SemanticCollectionItemKind,
        minimum_items: u32,
        maximum_items: u32,
        unique_items: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticInputValue {
    String(String),
    Integer(i64),
    Boolean(bool),
    Choice(String),
    Strings(Vec<String>),
    Integers(Vec<i64>),
    Booleans(Vec<bool>),
    Choices(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAuthorizedChoice {
    pub choice_id: String,
    pub presentation_key: String,
    pub value: SemanticInputValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticInputRequirement {
    pub semantic_field_id: String,
    pub input_kind: SemanticInputKind,
    pub presentation_key: String,
    pub description_key: Option<String>,
    pub required: bool,
    pub constraints: Option<SemanticInputConstraints>,
    pub authorized_choices: Vec<SemanticAuthorizedChoice>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticInputAnswer {
    pub semantic_field_id: String,
    pub value: SemanticInputValue,
}

/// Application-owned semantic operation after every guarded input has been revalidated.
///
/// The original released request remains immutable. Guard answers are retained separately and
/// included in the coordinator operation identity so a valid continuation cannot be accepted and
/// then executed as the unanswered request.
#[derive(Clone, Debug)]
pub struct ResolvedSemanticExecutionRequest {
    parsed: ParsedSemanticRequest,
    answers: Vec<SemanticInputAnswer>,
    canonical_operation: Vec<u8>,
}

/// One immutable semantic operation bound to the exact admitted workspace/epoch capability and
/// its public snapshot projection. The backend-specific authority is never reconstructed by the
/// transport and survives from the final atomic start leg through execution.
#[derive(Clone, Debug)]
pub struct PreparedSemanticExecution<A> {
    resolved: ResolvedSemanticExecutionRequest,
    authority: A,
    snapshot: SemanticSnapshotResponse,
}

impl<A> PreparedSemanticExecution<A> {
    #[must_use]
    pub fn new(
        resolved: ResolvedSemanticExecutionRequest,
        authority: A,
        snapshot: SemanticSnapshotResponse,
    ) -> Self {
        Self {
            resolved,
            authority,
            snapshot,
        }
    }

    #[must_use]
    pub const fn resolved(&self) -> &ResolvedSemanticExecutionRequest {
        &self.resolved
    }

    #[must_use]
    pub const fn authority(&self) -> &A {
        &self.authority
    }

    #[must_use]
    pub const fn snapshot(&self) -> &SemanticSnapshotResponse {
        &self.snapshot
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        ResolvedSemanticExecutionRequest,
        A,
        SemanticSnapshotResponse,
    ) {
        (self.resolved, self.authority, self.snapshot)
    }
}

impl ResolvedSemanticExecutionRequest {
    /// Bind one validated answer set to the released request.
    ///
    /// # Errors
    ///
    /// Rejects duplicate/invalid field identities or an operation envelope that cannot be
    /// canonicalized.
    pub fn try_new(
        parsed: ParsedSemanticRequest,
        mut answers: Vec<SemanticInputAnswer>,
    ) -> Result<Self, SemanticQueryError> {
        answers.sort_by(|left, right| left.semantic_field_id.cmp(&right.semantic_field_id));
        if answers.iter().any(|answer| {
            answer.semantic_field_id.is_empty()
                || answer.semantic_field_id.len() > 128
                || !answer.semantic_field_id.is_ascii()
        }) || answers
            .windows(2)
            .any(|pair| pair[0].semantic_field_id == pair[1].semantic_field_id)
        {
            return Err(SemanticQueryError::Invalid(
                "guarded input field identity is invalid or duplicated".to_owned(),
            ));
        }
        let canonical_operation = if answers.is_empty() {
            parsed.canonical_bytes.clone()
        } else {
            let request: JsonValue =
                serde_json::from_slice(&parsed.canonical_bytes).map_err(|_| {
                    SemanticQueryError::Invalid(
                        "canonical semantic request cannot be bound to guarded input".to_owned(),
                    )
                })?;
            let guarded_input = answers
                .iter()
                .map(answer_json)
                .collect::<Result<Vec<_>, _>>()?;
            serde_json_canonicalizer::to_vec(&serde_json::json!({
                "guarded_input": guarded_input,
                "semantic_request": request,
            }))
            .map_err(|_| {
                SemanticQueryError::Invalid(
                    "guarded semantic operation cannot be canonicalized".to_owned(),
                )
            })?
        };
        Ok(Self {
            parsed,
            answers,
            canonical_operation,
        })
    }

    #[must_use]
    pub const fn parsed(&self) -> &ParsedSemanticRequest {
        &self.parsed
    }

    #[must_use]
    pub fn answers(&self) -> &[SemanticInputAnswer] {
        &self.answers
    }

    #[must_use]
    pub fn canonical_operation(&self) -> &[u8] {
        &self.canonical_operation
    }

    #[must_use]
    pub fn into_parsed(self) -> ParsedSemanticRequest {
        self.parsed
    }
}

/// Pure preparation result for one accumulated guarded-input state.
#[derive(Clone, Debug)]
pub enum SemanticExecutionPreparation {
    InputRequired(Vec<SemanticInputRequirement>),
    Ready(ResolvedSemanticExecutionRequest),
}

#[async_trait]
pub trait SemanticQueryBackend: Send + Sync + 'static {
    /// Backend-owned immutable capability retained across atomic start and execution.
    type ExecutionAuthority: Clone + std::fmt::Debug + Send + Sync + 'static;

    /// Return the exact package builder used by production execution so restart recovery can
    /// reopen retained manifest-last packages through the same sink and bounds.
    fn retained_package_builder(&self) -> Option<StreamedResultPackageBuilder> {
        None
    }

    /// Return the application-release binding when this backend is release-bound.
    ///
    /// Generic independently testable backends may remain unbound. Production backends expose
    /// only opaque application identity bytes so this inward contract does not import release
    /// compiler types.
    fn application_release_pin(&self) -> Option<[u8; 32]> {
        None
    }

    /// Validate semantic executability against the installed epoch without starting work.
    fn validate_execution_request(
        &self,
        request: &ParsedSemanticRequest,
    ) -> Result<(), SemanticQueryError>;

    /// Revalidate accumulated answers and return the next typed requirement or a resolved
    /// operation, without reserving or starting work.
    fn prepare_execution_request(
        &self,
        request: &ParsedSemanticRequest,
        answers: &[SemanticInputAnswer],
    ) -> Result<SemanticExecutionPreparation, SemanticQueryError>;

    /// Revalidate and atomically bind one fully resolved operation to the exact active
    /// workspace/epoch capability. Pure validation and guarded-input preparation never call this
    /// method and therefore create no execution lease or coordinator state.
    fn admit_execution_request(
        &self,
        resolved: ResolvedSemanticExecutionRequest,
    ) -> Result<PreparedSemanticExecution<Self::ExecutionAuthority>, SemanticQueryError>;

    /// Execute one accepted operation against the exact leased active workspace.
    async fn execute(
        &self,
        prepared: PreparedSemanticExecution<Self::ExecutionAuthority>,
        freshness: FreshnessState,
        cancellation: Cancellation,
        context: SemanticBackendExecutionContext,
        artifacts: QueryExecutionArtifactAccumulator,
    ) -> SemanticBackendOutcome;
}

fn answer_json(answer: &SemanticInputAnswer) -> Result<JsonValue, SemanticQueryError> {
    let value = match &answer.value {
        SemanticInputValue::String(value) => serde_json::json!({"string": value}),
        SemanticInputValue::Integer(value) => serde_json::json!({"integer": value}),
        SemanticInputValue::Boolean(value) => serde_json::json!({"boolean": value}),
        SemanticInputValue::Choice(value) => serde_json::json!({"choice": value}),
        SemanticInputValue::Strings(values) => serde_json::json!({"strings": values}),
        SemanticInputValue::Integers(values) => serde_json::json!({"integers": values}),
        SemanticInputValue::Booleans(values) => serde_json::json!({"booleans": values}),
        SemanticInputValue::Choices(values) => serde_json::json!({"choices": values}),
    };
    Ok(serde_json::json!({
        "semantic_field_id": answer.semantic_field_id,
        "value": value,
    }))
}

/// Authenticated execution identity and immutable relational authorities for one operation.
#[derive(Clone, Debug)]
pub struct SemanticBackendExecutionContext {
    execution: QueryExecutionContext,
    principal_id: Arc<str>,
    workspace_id: Arc<str>,
    owner: PublishedResultOwner,
    query_execution_pin: QueryExecutionPin,
    result_lease_id: LeaseId,
    result_lease_token: OpaqueResultLeaseToken,
    publication_intent: Arc<dyn ResultPublicationIntentRecorder>,
    deadline: Instant,
}

impl SemanticBackendExecutionContext {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        execution: QueryExecutionContext,
        principal_id: impl Into<Arc<str>>,
        workspace_id: impl Into<Arc<str>>,
        owner: PublishedResultOwner,
        query_execution_pin: QueryExecutionPin,
        result_lease_id: LeaseId,
        result_lease_token: OpaqueResultLeaseToken,
        publication_intent: Arc<dyn ResultPublicationIntentRecorder>,
        deadline: Instant,
    ) -> Self {
        Self {
            execution,
            principal_id: principal_id.into(),
            workspace_id: workspace_id.into(),
            owner,
            query_execution_pin,
            result_lease_id,
            result_lease_token,
            publication_intent,
            deadline,
        }
    }

    #[must_use]
    pub const fn execution(&self) -> &QueryExecutionContext {
        &self.execution
    }

    #[must_use]
    pub fn principal_id(&self) -> &str {
        &self.principal_id
    }

    #[must_use]
    pub fn agent_instance_id(&self) -> &str {
        self.principal_id()
    }

    #[must_use]
    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    #[must_use]
    pub const fn owner(&self) -> PublishedResultOwner {
        self.owner
    }

    #[must_use]
    pub const fn query_execution_pin(&self) -> QueryExecutionPin {
        self.query_execution_pin
    }

    #[must_use]
    pub const fn result_lease_id(&self) -> LeaseId {
        self.result_lease_id
    }

    #[must_use]
    pub const fn result_lease_token(&self) -> OpaqueResultLeaseToken {
        self.result_lease_token
    }

    #[must_use]
    pub fn publication_intent(&self) -> Arc<dyn ResultPublicationIntentRecorder> {
        Arc::clone(&self.publication_intent)
    }

    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }
}

/// Successful relational publication plus public snapshot and execution evidence.
#[derive(Debug)]
pub struct PublishedArrowSemanticSuccess {
    publication: StreamedRelationalQueryPublication,
    snapshot: SemanticSnapshotResponse,
    evidence: QueryExecutionArtifactEvidence,
}

impl PublishedArrowSemanticSuccess {
    #[must_use]
    pub const fn new(
        publication: StreamedRelationalQueryPublication,
        snapshot: SemanticSnapshotResponse,
        evidence: QueryExecutionArtifactEvidence,
    ) -> Self {
        Self {
            publication,
            snapshot,
            evidence,
        }
    }

    #[must_use]
    pub const fn publication(&self) -> &StreamedRelationalQueryPublication {
        &self.publication
    }

    #[must_use]
    pub const fn lease_token(&self) -> &OpaqueResultLeaseToken {
        self.publication.lease_token_ref()
    }

    #[must_use]
    pub const fn snapshot(&self) -> &SemanticSnapshotResponse {
        &self.snapshot
    }

    #[must_use]
    pub const fn evidence(&self) -> &QueryExecutionArtifactEvidence {
        &self.evidence
    }

    /// Transfer the sealed package and its retained epoch/resource permits to the v2 registry.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        StreamedRelationalQueryPublication,
        SemanticSnapshotResponse,
        QueryExecutionArtifactEvidence,
    ) {
        (self.publication, self.snapshot, self.evidence)
    }
}

/// Closed terminal outcome of one programmatic relational execution.
#[derive(Debug)]
pub enum SemanticBackendOutcome {
    PublishedArrow(PublishedArrowSemanticSuccess),
    Failed {
        error: SemanticQueryError,
        evidence: QueryExecutionArtifactEvidence,
    },
    Cancelled {
        error: SemanticQueryError,
        evidence: QueryExecutionArtifactEvidence,
    },
}

/// Current wall-clock observation for retention and durable control records only.
#[must_use]
pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}
