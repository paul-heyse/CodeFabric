//! Bounded semantic-query ingress and DataFusion execution over one pinned snapshot.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use arrow_array::{Array as _, FixedSizeBinaryArray, RecordBatch, UInt64Array};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use datafusion::common::ScalarValue;
use datafusion::logical_expr::{Expr, LogicalPlan, LogicalPlanBuilder, col, lit};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use thiserror::Error;

use crate::contracts::jcs::{CanonicalJsonError, canonicalize_slice, canonicalize_value};
use crate::fabric::{ServingQueryError, ServingQuerySession};
pub use crate::registries::QueryForm;
use crate::registries::{
    COMPLETENESS_STATE_VALUES, CompletenessState, DEPENDENCY_STATE_VALUES, DependencyState,
    FRESHNESS_STATE_VALUES, FreshnessState, LIMIT_STATE_VALUES, LimitState, PHRASE_ENTRIES,
    PhraseEntry, QUERY_AVAILABILITY_STATE_VALUES, QUERY_EXECUTION_STATE_VALUES, QUERY_FORM_VALUES,
    QueryAvailabilityState, QueryExecutionState, registry_state_name,
};

const SPECIFICATION: &str = "composable semantic CPG fact query";
const VERSION: &str = "1.3";
const MAX_REQUEST_BYTES: usize = 256 * 1024;
const MAX_QUERIES: usize = 32;
const MAX_ROWS_PER_QUERY: usize = 10_000;

macro_rules! serialize_generated_state {
    ($state:ty, $values:expr) => {
        impl Serialize for $state {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let name = registry_state_name($values, *self as u16)
                    .expect("generated state enum and registry projection are one authority");
                serializer.serialize_str(name)
            }
        }
    };
}

serialize_generated_state!(QueryExecutionState, QUERY_EXECUTION_STATE_VALUES);
serialize_generated_state!(QueryAvailabilityState, QUERY_AVAILABILITY_STATE_VALUES);
serialize_generated_state!(CompletenessState, COMPLETENESS_STATE_VALUES);
serialize_generated_state!(FreshnessState, FRESHNESS_STATE_VALUES);
serialize_generated_state!(LimitState, LIMIT_STATE_VALUES);
serialize_generated_state!(DependencyState, DEPENDENCY_STATE_VALUES);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessPolicy {
    CurrentRequired,
    WaitForCurrent,
    BestAvailableSnapshot,
}

impl QueryForm {
    pub(crate) const fn currently_supported(self) -> bool {
        true
    }

    pub(crate) fn registry_slug(self) -> &'static str {
        QUERY_FORM_VALUES
            .iter()
            .find(|entry| entry.code == self as u16)
            .expect("generated QueryForm and QUERY_FORM_VALUES are one authority")
            .slug
    }

    fn table(self) -> Result<&'static str, SemanticQueryError> {
        match self {
            Self::FindEntities => Ok("entities"),
            Self::RetrieveFacts => Ok("properties"),
            Self::FollowRelationships => Ok("relations"),
            _ => Err(SemanticQueryError::Invalid(
                "query form is registered but not active in the current execution profile"
                    .to_owned(),
            )),
        }
    }

    fn order_key(self) -> Result<&'static str, SemanticQueryError> {
        match self {
            Self::FindEntities => Ok("entity_id"),
            Self::RetrieveFacts | Self::FollowRelationships => Ok("fact_id"),
            _ => Err(SemanticQueryError::Invalid(
                "query form is registered but not active in the current execution profile"
                    .to_owned(),
            )),
        }
    }

    fn plan_node_kind(self) -> Result<&'static str, SemanticQueryError> {
        match self {
            Self::FindEntities => Ok("find-entities"),
            Self::RetrieveFacts => Ok("retrieve-facts"),
            Self::FollowRelationships => Ok("follow-relationships"),
            Self::FindPaths => Ok("find-paths"),
            Self::MatchPattern => Ok("match-pattern"),
            Self::CombineResults => Ok("combine-results"),
            Self::SummarizeFacts => Ok("summarize-facts"),
            Self::RetrieveSourceContext => Ok("retrieve-source-context"),
        }
    }

    const fn output_role(self) -> ResultRole {
        match self {
            Self::FindEntities => ResultRole::Entities,
            Self::RetrieveFacts | Self::FollowRelationships => ResultRole::Facts,
            Self::FindPaths => ResultRole::Paths,
            Self::MatchPattern => ResultRole::PatternBindings,
            Self::CombineResults => ResultRole::Groups,
            Self::SummarizeFacts => ResultRole::Summary,
            Self::RetrieveSourceContext => ResultRole::SourceContexts,
        }
    }

    fn accepts_role(self, role: ResultRole) -> bool {
        match self {
            Self::FindEntities => matches!(role, ResultRole::Entities | ResultRole::Facts),
            Self::RetrieveFacts => matches!(role, ResultRole::Entities | ResultRole::Facts),
            Self::FollowRelationships => matches!(role, ResultRole::Entities | ResultRole::Facts),
            Self::FindPaths | Self::MatchPattern => role == ResultRole::Entities,
            Self::CombineResults => role != ResultRole::Summary,
            Self::SummarizeFacts => matches!(role, ResultRole::Facts | ResultRole::Groups),
            Self::RetrieveSourceContext => {
                matches!(
                    role,
                    ResultRole::Entities | ResultRole::Facts | ResultRole::Paths
                )
            }
        }
    }
}

impl Serialize for QueryForm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.registry_slug())
    }
}

impl<'de> Deserialize<'de> for QueryForm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let slug = String::deserialize(deserializer)?;
        let entry = QUERY_FORM_VALUES
            .iter()
            .find(|entry| entry.slug == slug)
            .ok_or_else(|| D::Error::custom("unknown governed query form"))?;
        Self::try_from(entry.code).map_err(|_| D::Error::custom("invalid governed query form"))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryInput {
    #[serde(default)]
    pub entity_ids: Vec<String>,
    #[serde(default)]
    pub fact_ids: Vec<String>,
    #[serde(default)]
    pub results: Vec<PriorResultReference>,
}

/// One typed dependency on the result role produced by an earlier query block.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PriorResultReference {
    pub results_of: String,
    pub select: ResultRole,
}

/// Semantic value family carried across query-block edges.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultRole {
    Entities,
    Facts,
    Paths,
    PatternBindings,
    Groups,
    Summary,
    SourceContexts,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryPredicate {
    #[serde(default)]
    pub entity_kind_codes: Vec<u32>,
    #[serde(default)]
    pub relation_kind_codes: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryLimit {
    pub first: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticQueryClause {
    pub query_id: String,
    pub request: QueryForm,
    pub label: Option<String>,
    pub input: Option<QueryInput>,
    pub r#where: Option<QueryPredicate>,
    pub limit: Option<QueryLimit>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CostBudget {
    pub maximum_rows: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticQueryRequest {
    pub specification: String,
    pub version: String,
    pub semantic_request_id: String,
    pub workspace_id: String,
    pub freshness_policy: FreshnessPolicy,
    pub queries: Vec<SemanticQueryClause>,
    pub response_projection: Option<BTreeMap<String, bool>>,
    pub cost_budget: Option<CostBudget>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryResultRecord {
    pub query_id: String,
    pub request: QueryForm,
    pub execution_state: QueryExecutionState,
    pub availability_state: QueryAvailabilityState,
    pub completeness_state: CompletenessState,
    pub freshness_state: FreshnessState,
    pub limit_state: LimitState,
    pub dependency_state: DependencyState,
    pub resolved_semantics: BTreeMap<String, String>,
    pub entity_ids: Vec<String>,
    pub fact_ids: Vec<String>,
    pub path_ids: Vec<String>,
    pub group_ids: Vec<String>,
    pub source_context_ids: Vec<String>,
    pub coverage: BTreeMap<String, u64>,
    pub errors: Vec<SemanticErrorRecord>,
    pub notices: Vec<String>,
    #[serde(skip)]
    pub output_row_count: usize,
    #[serde(skip)]
    pub result_checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticErrorRecord {
    pub code: String,
    pub layer: String,
    pub retryable: bool,
    pub safe_message: String,
    pub field: Option<String>,
    pub semantic_phrase: Option<String>,
    pub candidate_interpretations: Vec<String>,
    pub failed_dependency_query_id: Option<String>,
    pub diagnostic_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticSnapshotResponse {
    pub snapshot_id: String,
    pub workspace_id: String,
    pub repository_id: Option<String>,
    pub worktree_id: Option<String>,
    pub source_generation: u64,
    pub source_inventory_digest: String,
    pub durable_base_publication: String,
    pub base_table_version_digest: String,
    pub overlay_generation: u64,
    pub overlay_checksum: String,
    pub analysis_context_set_id: String,
    pub analysis_context_ids: Vec<String>,
    pub freshness_state: FreshnessState,
    pub source_trust_state: String,
    pub event_stream_health: String,
    pub git_acceleration_status: String,
    pub git_operation_summary: Option<BTreeMap<String, String>>,
    pub pending_update_count: u64,
    pub ontology_version: String,
    pub schema_bundle_version: String,
    pub provider_bundle_version: String,
    pub derivation_bundle_version: String,
    pub query_language_version: String,
    pub capability_summaries: Vec<BTreeMap<String, String>>,
    pub diagnostic_references: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticQueryResponse {
    pub specification: &'static str,
    pub version: &'static str,
    pub semantic_request_id: String,
    pub execution_state: QueryExecutionState,
    pub availability_state: QueryAvailabilityState,
    pub completeness_state: CompletenessState,
    pub freshness_state: FreshnessState,
    pub limit_state: LimitState,
    pub successful_query_count: usize,
    pub failed_query_count: usize,
    pub not_executed_dependency_count: usize,
    pub snapshot: SemanticSnapshotResponse,
    pub entities: BTreeMap<String, BTreeMap<String, String>>,
    pub facts: BTreeMap<String, BTreeMap<String, String>>,
    pub paths: BTreeMap<String, BTreeMap<String, String>>,
    pub groups: BTreeMap<String, BTreeMap<String, String>>,
    pub source_contexts: BTreeMap<String, BTreeMap<String, String>>,
    pub query_results: Vec<QueryResultRecord>,
    pub errors: Vec<SemanticErrorRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedSemanticRequest {
    pub request: SemanticQueryRequest,
    pub canonical_bytes: Vec<u8>,
    pub request_digest: String,
}

/// One type-checked query block independent of a serving snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedQueryBlock {
    pub block_id: String,
    pub source_pointer: String,
    pub form: QueryForm,
    pub input_roles: Vec<ResultRole>,
    pub output_role: ResultRole,
    pub dependencies: Vec<String>,
    pub fan_in: usize,
    pub fan_out: usize,
    pub coverage_prerequisites: BTreeSet<String>,
    pub coverage_effects: BTreeSet<String>,
    pub canonical_order: Vec<&'static str>,
    pub limit: QueryLimit,
    pub maximum_memory_bytes: usize,
    pub cancellation_required: bool,
}

/// Parsed request plus the dependency-closed application semantic IR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedSemanticRequest {
    pub request: SemanticQueryRequest,
    pub canonical_bytes: Vec<u8>,
    pub request_digest: String,
    pub blocks: Vec<TypedQueryBlock>,
    pub execution_order: Vec<String>,
}

/// Snapshot-bound relational block ready for native DataFusion lowering.
#[derive(Clone, Debug)]
pub struct BoundQueryBlock {
    pub typed: TypedQueryBlock,
    pub operator: BoundOperator,
}

/// Snapshot-bound operator family: native DataFusion for relational work, application graph plan
/// for graph semantics.
#[derive(Clone, Debug)]
pub enum BoundOperator {
    Relational {
        table: &'static str,
        identity_column: &'static str,
        plan: LogicalPlan,
    },
    Graph(GraphOperatorPlan),
}

/// Typed bounded graph/set/context plan executed outside DataFusion.
#[derive(Clone, Debug)]
pub struct GraphOperatorPlan {
    pub form: QueryForm,
    pub input_roles: Vec<ResultRole>,
    pub output_role: ResultRole,
    pub output_schema: SchemaRef,
    pub canonical_order: Vec<&'static str>,
    pub maximum_results: usize,
    pub maximum_depth: usize,
    pub maximum_memory_bytes: usize,
    pub cancellation_required: bool,
}

/// One immutable snapshot binding for the complete typed DAG.
#[derive(Clone, Debug)]
pub struct BoundPlanSpec {
    pub snapshot_id: String,
    pub request_digest: String,
    pub blocks: Vec<BoundQueryBlock>,
    pub execution_order: Vec<String>,
}

/// Backward-compatible name for the now-typed ingress contract.
pub type ValidatedSemanticRequest = TypedSemanticRequest;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutedSemanticResponse {
    pub response: SemanticQueryResponse,
    pub canonical_bytes: Vec<u8>,
    pub response_digest: String,
}

#[derive(Debug, Error)]
pub enum SemanticQueryError {
    #[error("INVALID_REQUEST_SCHEMA:SEMANTIC_QUERY_INVALID:{0}")]
    Invalid(String),
    #[error(transparent)]
    Canonical(#[from] CanonicalJsonError),
    #[error(transparent)]
    Serving(#[from] ServingQueryError),
}

impl From<datafusion::error::DataFusionError> for SemanticQueryError {
    fn from(error: datafusion::error::DataFusionError) -> Self {
        Self::Serving(ServingQueryError::from(error))
    }
}

fn b3(bytes: &[u8]) -> String {
    crate::integrity::framed_digest(bytes)
}

fn valid_id(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
}

fn resolve_phrase(
    form: QueryForm,
    label: Option<&str>,
) -> Result<Option<&'static PhraseEntry>, SemanticQueryError> {
    let Some(label) = label else {
        return Ok(None);
    };
    if label.is_empty() || label.len() > 256 {
        return Err(SemanticQueryError::Invalid(
            "semantic phrase is empty or exceeds its bound".to_owned(),
        ));
    }
    let plan_node_kind = form.plan_node_kind()?;
    let mut matches = PHRASE_ENTRIES.iter().filter(|entry| {
        (entry.canonical_text == label || entry.accepted_aliases.contains(&label))
            && entry.plan_node_kind == plan_node_kind
    });
    let resolved = matches.next().ok_or_else(|| {
        SemanticQueryError::Invalid(
            "semantic phrase is unknown or incompatible with the request form".to_owned(),
        )
    })?;
    if matches.next().is_some() {
        return Err(SemanticQueryError::Invalid(
            "semantic phrase resolves ambiguously".to_owned(),
        ));
    }
    Ok(Some(resolved))
}

/// Strictly decode and canonicalize one semantic request without assigning semantic types.
///
/// # Errors
///
/// Returns an error for oversized, non-canonical, or schema-invalid JSON.
pub fn parse_request(bytes: &[u8]) -> Result<ParsedSemanticRequest, SemanticQueryError> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(SemanticQueryError::Invalid(
            "request exceeds maximum bytes".to_owned(),
        ));
    }
    let canonical_bytes = canonicalize_slice(bytes)?;
    let request: SemanticQueryRequest = serde_json::from_slice(&canonical_bytes)
        .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?;
    Ok(ParsedSemanticRequest {
        request_digest: b3(&canonical_bytes),
        request,
        canonical_bytes,
    })
}

fn evaluative_intent(canonical_bytes: &[u8]) -> bool {
    let normalized = String::from_utf8_lossy(canonical_bytes).to_ascii_lowercase();
    [
        "safe_to_refactor",
        "safe to refactor",
        "high_risk",
        "high risk",
        "should_change",
        "should change",
        "test_impacted",
        "test impacted",
        "complexity verdict",
    ]
    .iter()
    .any(|term| normalized.contains(term))
}

fn canonical_order(form: QueryForm) -> Vec<&'static str> {
    match form {
        QueryForm::FindEntities => vec!["entity_id"],
        QueryForm::RetrieveFacts | QueryForm::FollowRelationships => vec!["fact_id"],
        QueryForm::FindPaths => vec!["path_length", "path_id"],
        QueryForm::MatchPattern => vec!["binding_id"],
        QueryForm::CombineResults => vec!["group_id"],
        QueryForm::SummarizeFacts => vec!["summary_key"],
        QueryForm::RetrieveSourceContext => vec!["source_file_id", "span_start"],
    }
}

/// Type-check block roles, dependency topology, resource contracts, and semantic policy.
///
/// # Errors
///
/// Rejects unknown/inactive forms, cycles, role mismatches, invalid source identifiers,
/// evaluative intent, and requests outside the bounded execution profile.
pub fn type_request(
    parsed: ParsedSemanticRequest,
) -> Result<TypedSemanticRequest, SemanticQueryError> {
    let ParsedSemanticRequest {
        request,
        canonical_bytes,
        request_digest,
    } = parsed;
    if request.specification != SPECIFICATION
        || request.version != VERSION
        || !valid_id(&request.semantic_request_id, 128)
        || !valid_id(&request.workspace_id, 128)
        || request.queries.is_empty()
        || request.queries.len() > MAX_QUERIES
    {
        return Err(SemanticQueryError::Invalid(
            "request identity, version, or query count is invalid".to_owned(),
        ));
    }
    if evaluative_intent(&canonical_bytes) {
        return Err(SemanticQueryError::Invalid(
            "evaluative intent is outside the fact substrate".to_owned(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut forms = BTreeMap::new();
    let total_rows = request.queries.iter().try_fold(0_usize, |total, query| {
        if !valid_id(&query.query_id, 128) || !ids.insert(query.query_id.as_str()) {
            return Err(SemanticQueryError::Invalid(
                "query IDs must be unique bounded identifiers".to_owned(),
            ));
        }
        if !query.request.currently_supported() {
            return Err(SemanticQueryError::Invalid(
                "query form is registered but not active in the current execution profile"
                    .to_owned(),
            ));
        }
        forms.insert(query.query_id.clone(), query.request);
        resolve_phrase(query.request, query.label.as_deref())?;
        if let Some(input) = &query.input {
            if input
                .entity_ids
                .iter()
                .chain(&input.fact_ids)
                .any(|identity| !valid_id(identity, 192))
            {
                return Err(SemanticQueryError::Invalid(
                    "query input contains an invalid public identity".to_owned(),
                ));
            }
        }
        let limit = query.limit.unwrap_or(QueryLimit {
            first: 100,
            offset: 0,
        });
        if limit.first == 0 || limit.first > MAX_ROWS_PER_QUERY || limit.offset > 1_000_000 {
            return Err(SemanticQueryError::Invalid(
                "query pagination is outside the accepted bound".to_owned(),
            ));
        }
        total
            .checked_add(limit.first)
            .ok_or_else(|| SemanticQueryError::Invalid("request row budget overflow".to_owned()))
    })?;
    if request
        .cost_budget
        .is_some_and(|budget| budget.maximum_rows == 0 || total_rows > budget.maximum_rows)
    {
        return Err(SemanticQueryError::Invalid(
            "query limits exceed the request cost budget".to_owned(),
        ));
    }
    if request
        .response_projection
        .as_ref()
        .is_some_and(|projection| {
            projection.keys().any(|field| {
                !matches!(
                    field.as_str(),
                    "canonical_semantic_identity" | "semantic_kind" | "source_context" | "coverage"
                )
            })
        })
    {
        return Err(SemanticQueryError::Invalid(
            "response projection contains an unsupported semantic field".to_owned(),
        ));
    }
    let mut dependencies = BTreeMap::<String, Vec<String>>::new();
    let mut fan_out = BTreeMap::<String, usize>::new();
    for query in &request.queries {
        let references = query
            .input
            .as_ref()
            .map_or(&[][..], |input| input.results.as_slice());
        let mut seen = BTreeSet::new();
        for reference in references {
            if reference.results_of == query.query_id || !seen.insert(&reference.results_of) {
                return Err(SemanticQueryError::Invalid(
                    "query dependency is self-referential or duplicated".to_owned(),
                ));
            }
            let producer = forms.get(&reference.results_of).ok_or_else(|| {
                SemanticQueryError::Invalid("query dependency names an unknown block".to_owned())
            })?;
            if producer.output_role() != reference.select
                || !query.request.accepts_role(reference.select)
            {
                return Err(SemanticQueryError::Invalid(format!(
                    "query dependency {} has incompatible result role at /queries/{}/input/results",
                    reference.results_of,
                    request
                        .queries
                        .iter()
                        .position(|candidate| candidate.query_id == query.query_id)
                        .unwrap_or_default()
                )));
            }
            *fan_out.entry(reference.results_of.clone()).or_default() += 1;
        }
        dependencies.insert(
            query.query_id.clone(),
            references
                .iter()
                .map(|reference| reference.results_of.clone())
                .collect(),
        );
    }
    let mut indegree = dependencies
        .iter()
        .map(|(block, dependencies)| (block.clone(), dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(block, _)| block.clone())
        .collect::<BTreeSet<_>>();
    let mut execution_order = Vec::with_capacity(request.queries.len());
    while let Some(block) = ready.pop_first() {
        execution_order.push(block.clone());
        for (candidate, candidate_dependencies) in &dependencies {
            if candidate_dependencies.contains(&block) {
                let degree = indegree
                    .get_mut(candidate)
                    .expect("dependency graph and indegree map are one IR");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(candidate.clone());
                }
            }
        }
    }
    if execution_order.len() != request.queries.len() {
        return Err(SemanticQueryError::Invalid(
            "query dependency graph contains a cycle".to_owned(),
        ));
    }
    let blocks = request
        .queries
        .iter()
        .enumerate()
        .map(|(index, query)| {
            let limit = query.limit.unwrap_or(QueryLimit {
                first: 100,
                offset: 0,
            });
            let input_roles = query
                .input
                .as_ref()
                .map_or(&[][..], |input| input.results.as_slice())
                .iter()
                .map(|reference| reference.select)
                .collect::<Vec<_>>();
            Ok(TypedQueryBlock {
                block_id: query.query_id.clone(),
                source_pointer: format!("/queries/{index}"),
                form: query.request,
                input_roles,
                output_role: query.request.output_role(),
                dependencies: dependencies
                    .get(&query.query_id)
                    .cloned()
                    .unwrap_or_default(),
                fan_in: query.input.as_ref().map_or(0, |input| input.results.len()),
                fan_out: fan_out.get(&query.query_id).copied().unwrap_or_default(),
                coverage_prerequisites: BTreeSet::from(["snapshot_pinned".to_owned()]),
                coverage_effects: BTreeSet::from([format!(
                    "{}_rows_observed",
                    query.request.registry_slug().replace(' ', "_")
                )]),
                canonical_order: canonical_order(query.request),
                limit,
                maximum_memory_bytes: limit
                    .first
                    .checked_add(1)
                    .and_then(|rows| rows.checked_mul(1024))
                    .ok_or_else(|| {
                        SemanticQueryError::Invalid("query memory bound overflow".to_owned())
                    })?,
                cancellation_required: true,
            })
        })
        .collect::<Result<Vec<_>, SemanticQueryError>>()?;
    let typed = TypedSemanticRequest {
        request,
        canonical_bytes,
        request_digest,
        blocks,
        execution_order,
    };
    Ok(typed)
}

/// Parse and type-check one semantic request.
///
/// # Errors
///
/// Returns any parsing, policy, role, topology, or resource-contract failure.
pub fn validate_request(bytes: &[u8]) -> Result<ValidatedSemanticRequest, SemanticQueryError> {
    type_request(parse_request(bytes)?)
}

fn id16_bytes(value: &str, expected_prefix: &str) -> Result<[u8; 16], SemanticQueryError> {
    if !value.starts_with(expected_prefix) {
        return Err(SemanticQueryError::Invalid(format!(
            "public identity {value} has the wrong semantic domain"
        )));
    }
    let encoded = value.rsplit(':').next().unwrap_or_default();
    if encoded.len() != 32 || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(SemanticQueryError::Invalid(
            "public identity has an invalid Id16 payload".to_owned(),
        ));
    }
    let mut bytes = [0_u8; 16];
    for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(pair)
            .map_err(|_| SemanticQueryError::Invalid("identity is not UTF-8 hex".to_owned()))?;
        bytes[index] = u8::from_str_radix(pair, 16)
            .map_err(|_| SemanticQueryError::Invalid("identity is not hex".to_owned()))?;
    }
    Ok(bytes)
}

fn id16_scalar(value: &str, expected_prefix: &str) -> Result<ScalarValue, SemanticQueryError> {
    Ok(ScalarValue::FixedSizeBinary(
        16,
        Some(id16_bytes(value, expected_prefix)?.to_vec()),
    ))
}

fn any_of(column: &'static str, values: Vec<ScalarValue>) -> Option<Expr> {
    (!values.is_empty())
        .then(|| col(column).in_list(values.into_iter().map(lit).collect::<Vec<_>>(), false))
}

fn all_of(expressions: Vec<Expr>) -> Option<Expr> {
    expressions.into_iter().reduce(Expr::and)
}

async fn lower_relational_block(
    session: &ServingQuerySession,
    typed: &TypedQueryBlock,
    query: &SemanticQueryClause,
) -> Result<BoundQueryBlock, SemanticQueryError> {
    let table = typed.form.table()?;
    let identity_column = typed.form.order_key()?;
    let mut predicates = Vec::new();
    if let Some(input) = &query.input {
        match typed.form {
            QueryForm::FindEntities => {
                let values = input
                    .entity_ids
                    .iter()
                    .map(|value| id16_scalar(value, "entity:"))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(predicate) = any_of("entity_id", values) {
                    predicates.push(predicate);
                }
                if !input.fact_ids.is_empty() {
                    return Err(SemanticQueryError::Invalid(
                        "find-entities fact inputs require an explicit typed dependency".to_owned(),
                    ));
                }
            }
            QueryForm::RetrieveFacts | QueryForm::FollowRelationships => {
                let values = input
                    .fact_ids
                    .iter()
                    .map(|value| id16_scalar(value, "fact:"))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(predicate) = any_of("fact_id", values) {
                    predicates.push(predicate);
                }
                if !input.entity_ids.is_empty() {
                    return Err(SemanticQueryError::Invalid(
                        "entity-to-fact selection requires an explicit typed dependency".to_owned(),
                    ));
                }
            }
            _ => {
                return Err(SemanticQueryError::Invalid(
                    "graph form reached relational lowering".to_owned(),
                ));
            }
        }
    }
    if let Some(filter) = &query.r#where {
        match typed.form {
            QueryForm::FindEntities => {
                if !filter.relation_kind_codes.is_empty() {
                    return Err(SemanticQueryError::Invalid(
                        "relation predicate cannot bind to an entity scan".to_owned(),
                    ));
                }
                if let Some(predicate) = any_of(
                    "entity_kind_code",
                    filter
                        .entity_kind_codes
                        .iter()
                        .map(|value| {
                            i16::try_from(*value)
                                .map(|value| ScalarValue::Int16(Some(value)))
                                .map_err(|_| {
                                    SemanticQueryError::Invalid(
                                        "entity kind code exceeds Code16".to_owned(),
                                    )
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                ) {
                    predicates.push(predicate);
                }
            }
            QueryForm::FollowRelationships => {
                if !filter.entity_kind_codes.is_empty() {
                    return Err(SemanticQueryError::Invalid(
                        "entity predicate cannot bind to a relation scan".to_owned(),
                    ));
                }
                if let Some(predicate) = any_of(
                    "relation_kind_code",
                    filter
                        .relation_kind_codes
                        .iter()
                        .map(|value| {
                            i16::try_from(*value)
                                .map(|value| ScalarValue::Int16(Some(value)))
                                .map_err(|_| {
                                    SemanticQueryError::Invalid(
                                        "relation kind code exceeds Code16".to_owned(),
                                    )
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                ) {
                    predicates.push(predicate);
                }
            }
            QueryForm::RetrieveFacts => {
                if !filter.entity_kind_codes.is_empty() || !filter.relation_kind_codes.is_empty() {
                    return Err(SemanticQueryError::Invalid(
                        "kind predicates do not apply to property facts".to_owned(),
                    ));
                }
            }
            _ => unreachable!("inactive graph forms are rejected during typing"),
        }
    }
    let mut builder = LogicalPlanBuilder::from(session.table_plan(table).await?);
    if let Some(predicate) = all_of(predicates) {
        builder = builder.filter(predicate)?;
    }
    builder = builder.project(vec![col(identity_column)])?;
    builder = builder.sort(
        typed
            .canonical_order
            .iter()
            .map(|column| col(*column).sort(true, true))
            .collect::<Vec<_>>(),
    )?;
    let fetch = typed
        .limit
        .first
        .checked_add(1)
        .ok_or_else(|| SemanticQueryError::Invalid("query fetch bound overflow".to_owned()))?;
    let plan = builder.limit(typed.limit.offset, Some(fetch))?.build()?;
    session.validate_query_plan(&plan)?;
    Ok(BoundQueryBlock {
        typed: typed.clone(),
        operator: BoundOperator::Relational {
            table,
            identity_column,
            plan,
        },
    })
}

fn graph_operator_plan(typed: &TypedQueryBlock) -> Result<GraphOperatorPlan, SemanticQueryError> {
    if matches!(
        typed.form,
        QueryForm::FindEntities | QueryForm::RetrieveFacts | QueryForm::FollowRelationships
    ) {
        return Err(SemanticQueryError::Invalid(
            "relational form reached graph lowering".to_owned(),
        ));
    }
    let identity_name = match typed.output_role {
        ResultRole::Paths => "path_id",
        ResultRole::PatternBindings => "binding_id",
        ResultRole::Groups | ResultRole::Summary => "group_id",
        ResultRole::SourceContexts => "source_context_id",
        ResultRole::Entities | ResultRole::Facts => "result_id",
    };
    let output_schema = Arc::new(Schema::new(vec![
        Field::new(identity_name, DataType::FixedSizeBinary(16), false),
        Field::new("ordinal", DataType::UInt64, false),
        Field::new("cardinality", DataType::UInt64, false),
    ]));
    Ok(GraphOperatorPlan {
        form: typed.form,
        input_roles: typed.input_roles.clone(),
        output_role: typed.output_role,
        output_schema,
        canonical_order: typed.canonical_order.clone(),
        maximum_results: typed.limit.first.saturating_add(1),
        maximum_depth: 64,
        maximum_memory_bytes: typed.maximum_memory_bytes,
        cancellation_required: typed.cancellation_required,
    })
}

/// Bind the typed relational DAG to one immutable serving snapshot and validate its native plans.
///
/// # Errors
///
/// Rejects snapshot catalog drift, invalid semantic filters, unsupported relational bindings, and
/// any post-lowering table/function/plan-family policy violation.
pub async fn bind_request(
    session: &ServingQuerySession,
    typed: &TypedSemanticRequest,
) -> Result<BoundPlanSpec, SemanticQueryError> {
    let mut blocks = Vec::with_capacity(typed.blocks.len());
    for block in &typed.blocks {
        let query = typed
            .request
            .queries
            .iter()
            .find(|query| query.query_id == block.block_id)
            .expect("typed block retains its parsed query");
        if matches!(
            block.form,
            QueryForm::FindEntities | QueryForm::RetrieveFacts | QueryForm::FollowRelationships
        ) {
            blocks.push(lower_relational_block(session, block, query).await?);
        } else {
            blocks.push(BoundQueryBlock {
                typed: block.clone(),
                operator: BoundOperator::Graph(graph_operator_plan(block)?),
            });
        }
    }
    Ok(BoundPlanSpec {
        snapshot_id: session.snapshot_manifest().snapshot_id,
        request_digest: typed.request_digest.clone(),
        blocks,
        execution_order: typed.execution_order.clone(),
    })
}

#[cfg(test)]
fn query_sql(query: &SemanticQueryClause) -> Result<String, SemanticQueryError> {
    let limit = query.limit.unwrap_or(QueryLimit {
        first: 100,
        offset: 0,
    });
    Ok(format!(
        "SELECT * FROM {} ORDER BY {} LIMIT {} OFFSET {}",
        query.request.table()?,
        query.request.order_key()?,
        limit.first,
        limit.offset
    ))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct GraphEdge {
    fact_id: [u8; 16],
    source_id: [u8; 16],
    target_id: [u8; 16],
}

#[derive(Clone, Debug, Default)]
struct BlockValues {
    entity_ids: Vec<String>,
    fact_ids: Vec<String>,
    path_ids: Vec<String>,
    group_ids: Vec<String>,
    source_context_ids: Vec<String>,
}

impl BlockValues {
    fn all_ids(&self) -> impl Iterator<Item = &String> {
        self.entity_ids
            .iter()
            .chain(&self.fact_ids)
            .chain(&self.path_ids)
            .chain(&self.group_ids)
            .chain(&self.source_context_ids)
    }
}

#[derive(Debug)]
struct GraphExecution {
    batches: Vec<RecordBatch>,
    values: BlockValues,
    coverage: BTreeMap<String, u64>,
    completeness: CompletenessState,
    limit_state: LimitState,
}

#[derive(Debug)]
struct PathSearch {
    path: Option<Vec<[u8; 16]>>,
    depth_bound_reached: bool,
}

fn graph_result_identity(form: QueryForm, values: &[&[u8]]) -> ([u8; 16], String) {
    let mut fingerprint = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::ServingQuery,
    );
    fingerprint.update(&(form as u16).to_be_bytes());
    for value in values {
        fingerprint.update(&(value.len() as u64).to_be_bytes());
        fingerprint.update(value);
    }
    let digest = fingerprint.finalize();
    let mut id = [0_u8; 16];
    id.copy_from_slice(&digest[..16]);
    let public = crate::identity::encode_public_id(
        crate::identity::IdentityDomain::ResultArtifact,
        None,
        id,
    )
    .expect("result-artifact identity has no kind slug");
    (id, public)
}

fn graph_batch(
    plan: &GraphOperatorPlan,
    rows: &[([u8; 16], u64)],
) -> Result<RecordBatch, SemanticQueryError> {
    let identities = crate::fabric::id16_array(rows.iter().map(|(identity, _)| Some(identity)));
    let ordinals = UInt64Array::from_iter_values(0..u64::try_from(rows.len()).unwrap_or(u64::MAX));
    let cardinalities = UInt64Array::from_iter_values(rows.iter().map(|(_, value)| *value));
    Ok(RecordBatch::try_new(
        Arc::clone(&plan.output_schema),
        vec![
            Arc::new(identities),
            Arc::new(ordinals),
            Arc::new(cardinalities),
        ],
    )
    .map_err(ServingQueryError::from)?)
}

async fn load_graph_edges(
    session: &ServingQuerySession,
) -> Result<Vec<GraphEdge>, SemanticQueryError> {
    let plan = LogicalPlanBuilder::from(session.table_plan("relations").await?)
        .project(vec![col("fact_id"), col("source_id"), col("target_id")])?
        .sort(vec![col("fact_id").sort(true, true)])?
        .build()?;
    session.validate_query_plan(&plan)?;
    let result = session.query_plan("graph-edge-snapshot", plan).await?;
    let mut edges = Vec::new();
    for batch in &result.batches {
        let array = |name: &str| -> Result<&FixedSizeBinaryArray, SemanticQueryError> {
            let index = batch.schema().index_of(name).map_err(|_| {
                SemanticQueryError::Invalid(format!("graph edge column {name} is absent"))
            })?;
            batch
                .column(index)
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .ok_or_else(|| {
                    SemanticQueryError::Invalid(format!("graph edge column {name} is not Id16"))
                })
        };
        let facts = array("fact_id")?;
        let sources = array("source_id")?;
        let targets = array("target_id")?;
        for row in 0..batch.num_rows() {
            if facts.is_null(row) || sources.is_null(row) || targets.is_null(row) {
                return Err(SemanticQueryError::Invalid(
                    "graph edge identity is unexpectedly null".to_owned(),
                ));
            }
            edges.push(GraphEdge {
                fact_id: facts.value(row).try_into().map_err(|_| {
                    SemanticQueryError::Invalid("graph fact identity width drifted".to_owned())
                })?,
                source_id: sources.value(row).try_into().map_err(|_| {
                    SemanticQueryError::Invalid("graph source identity width drifted".to_owned())
                })?,
                target_id: targets.value(row).try_into().map_err(|_| {
                    SemanticQueryError::Invalid("graph target identity width drifted".to_owned())
                })?,
            });
        }
    }
    edges.sort();
    edges.dedup();
    Ok(edges)
}

fn shortest_path(
    edges: &[GraphEdge],
    start: [u8; 16],
    target: [u8; 16],
    maximum_depth: usize,
    cancellation: &crate::cancellation::Cancellation,
) -> Result<PathSearch, SemanticQueryError> {
    let mut adjacency = BTreeMap::<[u8; 16], Vec<[u8; 16]>>::new();
    for edge in edges {
        adjacency
            .entry(edge.source_id)
            .or_default()
            .push(edge.target_id);
    }
    for targets in adjacency.values_mut() {
        targets.sort();
        targets.dedup();
    }
    let mut queue = VecDeque::from([(start, vec![start])]);
    let mut visited = BTreeSet::from([start]);
    let mut polls = 0_u32;
    let mut depth_bound_reached = false;
    while let Some((current, path)) = queue.pop_front() {
        polls = polls.saturating_add(1);
        if polls % cancellation.check_interval() == 0 && cancellation.is_cancelled() {
            return Err(SemanticQueryError::Invalid(
                "graph execution was cancelled".to_owned(),
            ));
        }
        if current == target {
            return Ok(PathSearch {
                path: Some(path),
                depth_bound_reached,
            });
        }
        if path.len().saturating_sub(1) >= maximum_depth {
            depth_bound_reached |= adjacency.get(&current).is_some_and(|next| !next.is_empty());
            continue;
        }
        for next in adjacency.get(&current).into_iter().flatten() {
            if visited.insert(*next) {
                let mut next_path = path.clone();
                next_path.push(*next);
                queue.push_back((*next, next_path));
            }
        }
    }
    Ok(PathSearch {
        path: None,
        depth_bound_reached,
    })
}

fn graph_inputs(
    query: &SemanticQueryClause,
    dependencies: &[String],
    completed: &BTreeMap<String, BlockValues>,
) -> BlockValues {
    let mut values = BlockValues::default();
    if let Some(input) = &query.input {
        values.entity_ids.extend(input.entity_ids.clone());
        values.fact_ids.extend(input.fact_ids.clone());
    }
    for dependency in dependencies {
        if let Some(produced) = completed.get(dependency) {
            values.entity_ids.extend(produced.entity_ids.clone());
            values.fact_ids.extend(produced.fact_ids.clone());
            values.path_ids.extend(produced.path_ids.clone());
            values.group_ids.extend(produced.group_ids.clone());
            values
                .source_context_ids
                .extend(produced.source_context_ids.clone());
        }
    }
    values.entity_ids.sort();
    values.entity_ids.dedup();
    values.fact_ids.sort();
    values.fact_ids.dedup();
    values.path_ids.sort();
    values.path_ids.dedup();
    values.group_ids.sort();
    values.group_ids.dedup();
    values.source_context_ids.sort();
    values.source_context_ids.dedup();
    values
}

fn execute_graph_operator(
    plan: &GraphOperatorPlan,
    input: &BlockValues,
    edges: &[GraphEdge],
    context_ids: &[String],
    cancellation: &crate::cancellation::Cancellation,
) -> Result<GraphExecution, SemanticQueryError> {
    if cancellation.is_cancelled() {
        return Err(SemanticQueryError::Invalid(
            "graph execution was cancelled".to_owned(),
        ));
    }
    let estimated_working_bytes = edges
        .len()
        .checked_mul(std::mem::size_of::<GraphEdge>())
        .and_then(|bytes| bytes.checked_mul(3))
        .ok_or_else(|| SemanticQueryError::Invalid("graph memory estimate overflow".to_owned()))?;
    if estimated_working_bytes > plan.maximum_memory_bytes {
        return Err(SemanticQueryError::Invalid(
            "graph operator memory bound exceeded".to_owned(),
        ));
    }
    let mut output = BlockValues::default();
    let mut rows = Vec::<([u8; 16], u64)>::new();
    let mut coverage = BTreeMap::from([
        (
            "examined_edges".to_owned(),
            u64::try_from(edges.len()).unwrap_or(u64::MAX),
        ),
        ("negative_proof_available".to_owned(), 0),
    ]);
    match plan.form {
        QueryForm::FindPaths => {
            let mut entities = input
                .entity_ids
                .iter()
                .map(|value| id16_bytes(value, "entity:"))
                .collect::<Result<Vec<_>, _>>()?;
            if entities.is_empty() {
                entities.extend(
                    edges
                        .iter()
                        .flat_map(|edge| [edge.source_id, edge.target_id]),
                );
                entities.sort();
                entities.dedup();
            }
            for pair in entities.windows(2) {
                let searched =
                    shortest_path(edges, pair[0], pair[1], plan.maximum_depth, cancellation)?;
                if searched.depth_bound_reached {
                    coverage.insert("depth_bound_reached".to_owned(), 1);
                }
                if let Some(path) = searched.path {
                    let bytes = path
                        .iter()
                        .flat_map(|identity| identity.as_slice())
                        .copied()
                        .collect::<Vec<_>>();
                    let (identity, public) = graph_result_identity(plan.form, &[&bytes]);
                    rows.push((identity, u64::try_from(path.len()).unwrap_or(u64::MAX)));
                    output.path_ids.push(public);
                }
            }
            coverage.insert(
                "reachable_paths".to_owned(),
                u64::try_from(rows.len()).unwrap_or(u64::MAX),
            );
        }
        QueryForm::MatchPattern => {
            let selected = input
                .entity_ids
                .iter()
                .map(|value| id16_bytes(value, "entity:"))
                .collect::<Result<BTreeSet<_>, _>>()?;
            for edge in edges.iter().filter(|edge| {
                selected.is_empty()
                    || selected.contains(&edge.source_id)
                    || selected.contains(&edge.target_id)
            }) {
                let bytes = [edge.source_id.as_slice(), edge.target_id.as_slice()].concat();
                let (identity, public) = graph_result_identity(plan.form, &[&bytes]);
                rows.push((identity, 2));
                output.group_ids.push(public);
            }
            coverage.insert(
                "matched_bindings".to_owned(),
                u64::try_from(rows.len()).unwrap_or(u64::MAX),
            );
        }
        QueryForm::CombineResults => {
            for value in input.all_ids() {
                let (identity, public) = graph_result_identity(plan.form, &[value.as_bytes()]);
                rows.push((identity, 1));
                output.group_ids.push(public);
            }
            coverage.insert(
                "combined_members".to_owned(),
                u64::try_from(rows.len()).unwrap_or(u64::MAX),
            );
        }
        QueryForm::SummarizeFacts => {
            let values = input.all_ids().map(String::as_bytes).collect::<Vec<_>>();
            let (identity, public) = graph_result_identity(plan.form, &values);
            rows.push((identity, u64::try_from(values.len()).unwrap_or(u64::MAX)));
            output.group_ids.push(public);
            coverage.insert(
                "summarized_values".to_owned(),
                u64::try_from(values.len()).unwrap_or(u64::MAX),
            );
        }
        QueryForm::RetrieveSourceContext => {
            for context in context_ids {
                let raw = id16_bytes(context, "context:")?;
                rows.push((raw, 1));
                output.source_context_ids.push(context.clone());
            }
            coverage.insert(
                "source_contexts".to_owned(),
                u64::try_from(rows.len()).unwrap_or(u64::MAX),
            );
        }
        QueryForm::FindEntities | QueryForm::RetrieveFacts | QueryForm::FollowRelationships => {
            return Err(SemanticQueryError::Invalid(
                "relational form reached graph execution".to_owned(),
            ));
        }
    }
    if plan.form == QueryForm::FindPaths {
        rows.sort_unstable_by_key(|(identity, path_length)| (*path_length, *identity));
    } else {
        rows.sort_unstable();
    }
    rows.dedup();
    output.path_ids.sort();
    output.path_ids.dedup();
    output.group_ids.sort();
    output.group_ids.dedup();
    output.source_context_ids.sort();
    output.source_context_ids.dedup();
    let limit_reached = rows.len() > plan.maximum_results.saturating_sub(1);
    if limit_reached {
        rows.truncate(plan.maximum_results.saturating_sub(1));
        output.path_ids.truncate(rows.len());
        output.group_ids.truncate(rows.len());
        output.source_context_ids.truncate(rows.len());
    }
    let completeness = if (rows.is_empty()
        && matches!(plan.form, QueryForm::FindPaths | QueryForm::MatchPattern))
        || coverage.get("depth_bound_reached") == Some(&1)
    {
        CompletenessState::Indeterminate
    } else if limit_reached {
        CompletenessState::Partial
    } else {
        CompletenessState::Complete
    };
    Ok(GraphExecution {
        batches: vec![graph_batch(plan, &rows)?],
        values: output,
        coverage,
        completeness,
        limit_state: if limit_reached {
            LimitState::ExplicitLimitReached
        } else {
            LimitState::NotApplied
        },
    })
}

/// Execute all clauses through the existing immutable DataFusion snapshot session.
///
/// # Errors
///
/// Returns an error when a generated semantic mapping is invalid, snapshot execution fails, or the
/// canonical response cannot be encoded within the registered limits.
#[allow(clippy::too_many_lines)] // One pinned session keeps all clause results and response identities snapshot-coherent.
pub async fn execute_request(
    session: &ServingQuerySession,
    validated: ValidatedSemanticRequest,
    freshness: FreshnessState,
) -> Result<ExecutedSemanticResponse, SemanticQueryError> {
    execute_request_with_cancellation(
        session,
        validated,
        freshness,
        crate::cancellation::Cancellation::default(),
    )
    .await
}

/// Execute one request with a control-boundary cancellation handle shared by graph operators.
///
/// # Errors
///
/// Returns an error when validation, snapshot execution, graph execution, or response encoding
/// fails, including when cooperative cancellation is observed.
#[allow(clippy::too_many_lines)] // One pinned session keeps all clause results and response identities snapshot-coherent.
pub async fn execute_request_with_cancellation(
    session: &ServingQuerySession,
    validated: ValidatedSemanticRequest,
    freshness: FreshnessState,
    cancellation: crate::cancellation::Cancellation,
) -> Result<ExecutedSemanticResponse, SemanticQueryError> {
    let manifest = session.snapshot_manifest();
    let bound = bind_request(session, &validated).await?;
    let mut results = Vec::with_capacity(validated.request.queries.len());
    let mut entities = BTreeMap::new();
    let mut facts = BTreeMap::new();
    let mut paths = BTreeMap::new();
    let mut groups = BTreeMap::new();
    let mut completed = BTreeMap::<String, BlockValues>::new();
    let context_ids = manifest
        .body
        .contexts
        .records
        .iter()
        .map(|record| record.analysis_context_id.clone())
        .collect::<Vec<_>>();
    let graph_edges = if bound
        .blocks
        .iter()
        .any(|block| matches!(block.operator, BoundOperator::Graph(_)))
    {
        load_graph_edges(session).await?
    } else {
        Vec::new()
    };
    for block_id in &bound.execution_order {
        let block = bound
            .blocks
            .iter()
            .find(|block| block.typed.block_id == *block_id)
            .expect("bound execution order names a bound block");
        let query = validated
            .request
            .queries
            .iter()
            .find(|query| query.query_id == *block_id)
            .expect("typed execution order names a parsed block");
        let (
            values,
            coverage,
            completeness_state,
            limit_state,
            output_row_count,
            result_checksum,
            notices,
        ) = match &block.operator {
            BoundOperator::Relational {
                table,
                identity_column,
                plan,
            } => {
                let result = session
                    .query_plan(&block.typed.block_id, plan.clone())
                    .await?;
                let produced_rows = result.artifact.output_row_count;
                let mut ids = response_ids(&result.batches, query.request)?;
                let limit_reached = produced_rows > block.typed.limit.first;
                ids.truncate(block.typed.limit.first);
                let output_row_count = ids.len();
                let result_checksum = b3(ids.join("\0").as_bytes());
                let mut values = BlockValues::default();
                match query.request {
                    QueryForm::FindEntities => values.entity_ids = ids,
                    QueryForm::RetrieveFacts | QueryForm::FollowRelationships => {
                        values.fact_ids = ids;
                    }
                    _ => unreachable!("graph form cannot own a relational plan"),
                }
                (
                    values,
                    BTreeMap::from([
                        (
                            "returned_rows".to_owned(),
                            u64::try_from(output_row_count).unwrap_or(u64::MAX),
                        ),
                        ("native_datafusion_plan".to_owned(), 1),
                        (
                            format!("table:{table}:column:{identity_column}"),
                            u64::try_from(produced_rows).unwrap_or(u64::MAX),
                        ),
                    ]),
                    if limit_reached {
                        CompletenessState::Partial
                    } else {
                        CompletenessState::Complete
                    },
                    if limit_reached {
                        LimitState::ExplicitLimitReached
                    } else {
                        LimitState::NotApplied
                    },
                    output_row_count,
                    result_checksum,
                    Vec::new(),
                )
            }
            BoundOperator::Graph(plan) => {
                let input = graph_inputs(query, &block.typed.dependencies, &completed);
                let executed = execute_graph_operator(
                    plan,
                    &input,
                    &graph_edges,
                    &context_ids,
                    &cancellation,
                )?;
                let output_row_count = executed.batches.iter().map(RecordBatch::num_rows).sum();
                let mut checksums = Vec::with_capacity(executed.batches.len() * 32);
                for batch in &executed.batches {
                    checksums.extend_from_slice(
                        &crate::fabric::batch_checksum(batch)
                            .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
                    );
                }
                let notices = if executed.completeness == CompletenessState::Indeterminate {
                    vec![
                        "empty graph result has an unknown remainder and is not proof of absence"
                            .to_owned(),
                    ]
                } else {
                    Vec::new()
                };
                (
                    executed.values,
                    executed.coverage,
                    executed.completeness,
                    executed.limit_state,
                    output_row_count,
                    b3(&checksums),
                    notices,
                )
            }
        };
        for entity_id in &values.entity_ids {
            entities.insert(
                entity_id.clone(),
                BTreeMap::from([("entity_id".to_owned(), entity_id.clone())]),
            );
        }
        for fact_id in &values.fact_ids {
            facts.insert(
                fact_id.clone(),
                BTreeMap::from([("fact_id".to_owned(), fact_id.clone())]),
            );
        }
        for path_id in &values.path_ids {
            paths.insert(
                path_id.clone(),
                BTreeMap::from([("path_id".to_owned(), path_id.clone())]),
            );
        }
        for group_id in &values.group_ids {
            groups.insert(
                group_id.clone(),
                BTreeMap::from([("group_id".to_owned(), group_id.clone())]),
            );
        }
        let phrase = resolve_phrase(query.request, query.label.as_deref())?;
        let mut resolved_semantics = match &block.operator {
            BoundOperator::Relational {
                table,
                identity_column,
                ..
            } => BTreeMap::from([
                (
                    "operator_family".to_owned(),
                    "datafusion-relational".to_owned(),
                ),
                ("table".to_owned(), (*table).to_owned()),
                ("order_key".to_owned(), (*identity_column).to_owned()),
            ]),
            BoundOperator::Graph(plan) => BTreeMap::from([
                ("operator_family".to_owned(), "application-graph".to_owned()),
                (
                    "plan_node".to_owned(),
                    plan.form.plan_node_kind()?.to_owned(),
                ),
            ]),
        };
        if let Some(phrase) = phrase {
            resolved_semantics.insert("phrase_id".to_owned(), phrase.phrase_id.to_owned());
            resolved_semantics.insert(
                "canonical_phrase".to_owned(),
                phrase.canonical_text.to_owned(),
            );
        }
        results.push(QueryResultRecord {
            query_id: query.query_id.clone(),
            request: query.request,
            execution_state: QueryExecutionState::Complete,
            availability_state: if completeness_state == CompletenessState::Indeterminate {
                QueryAvailabilityState::Partial
            } else {
                QueryAvailabilityState::Available
            },
            completeness_state,
            freshness_state: freshness,
            limit_state,
            dependency_state: if block.typed.dependencies.is_empty() {
                DependencyState::NotApplicable
            } else {
                DependencyState::Ready
            },
            resolved_semantics,
            entity_ids: values.entity_ids.clone(),
            fact_ids: values.fact_ids.clone(),
            path_ids: values.path_ids.clone(),
            group_ids: values.group_ids.clone(),
            source_context_ids: values.source_context_ids.clone(),
            coverage,
            errors: Vec::new(),
            notices,
            output_row_count,
            result_checksum,
        });
        completed.insert(block_id.clone(), values);
    }
    let snapshot = snapshot_response(&manifest, freshness);
    let source_contexts = manifest
        .body
        .contexts
        .records
        .iter()
        .map(|record| {
            (
                record.analysis_context_id.clone(),
                BTreeMap::from([
                    (
                        "analysis_context_id".to_owned(),
                        record.analysis_context_id.clone(),
                    ),
                    (
                        "context_manifest_digest".to_owned(),
                        record.context_manifest_digest.clone(),
                    ),
                ]),
            )
        })
        .collect();
    let aggregate_limit = if results
        .iter()
        .any(|result| result.limit_state == LimitState::ExplicitLimitReached)
    {
        LimitState::ExplicitLimitReached
    } else {
        LimitState::NotApplied
    };
    let aggregate_completeness = if results
        .iter()
        .any(|result| result.completeness_state == CompletenessState::Indeterminate)
    {
        CompletenessState::Indeterminate
    } else if results
        .iter()
        .all(|result| result.completeness_state == CompletenessState::Complete)
    {
        CompletenessState::Complete
    } else {
        CompletenessState::Partial
    };
    let response = SemanticQueryResponse {
        specification: "composable semantic CPG fact query response",
        version: VERSION,
        semantic_request_id: validated.request.semantic_request_id,
        execution_state: QueryExecutionState::Complete,
        availability_state: if aggregate_completeness == CompletenessState::Indeterminate {
            QueryAvailabilityState::Partial
        } else {
            QueryAvailabilityState::Available
        },
        completeness_state: aggregate_completeness,
        freshness_state: freshness,
        limit_state: aggregate_limit,
        successful_query_count: results.len(),
        failed_query_count: 0,
        not_executed_dependency_count: 0,
        snapshot,
        entities,
        facts,
        paths,
        groups,
        source_contexts,
        query_results: results,
        errors: Vec::new(),
    };
    let value = serde_json::to_value(&response)
        .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?;
    let canonical_bytes = canonicalize_value(&value)?;
    Ok(ExecutedSemanticResponse {
        response_digest: b3(&canonical_bytes),
        response,
        canonical_bytes,
    })
}

fn response_ids(
    batches: &[arrow_array::RecordBatch],
    form: QueryForm,
) -> Result<Vec<String>, SemanticQueryError> {
    let column_name = form.order_key()?;
    let (domain, kind_slug) = match form {
        QueryForm::FindEntities => (crate::identity::IdentityDomain::Entity, "unknown"),
        QueryForm::RetrieveFacts => (crate::identity::IdentityDomain::PropertyFact, "property"),
        QueryForm::FollowRelationships => {
            (crate::identity::IdentityDomain::RelationFact, "relation")
        }
        _ => {
            return Err(SemanticQueryError::Invalid(
                "inactive query form reached response decoding".to_owned(),
            ));
        }
    };
    let mut ids = Vec::new();
    for batch in batches {
        let index = batch
            .schema()
            .index_of(column_name)
            .map_err(|_| SemanticQueryError::Invalid("result identity column is absent".into()))?;
        let values = batch
            .column(index)
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .ok_or_else(|| {
                SemanticQueryError::Invalid("result identity column is not Id16".into())
            })?;
        for row in 0..values.len() {
            if values.is_null(row) || values.value(row).len() != 16 {
                return Err(SemanticQueryError::Invalid(
                    "result identity is null or has invalid width".into(),
                ));
            }
            let raw: [u8; 16] = values.value(row).try_into().map_err(|_| {
                SemanticQueryError::Invalid("result identity has invalid width".into())
            })?;
            ids.push(
                crate::identity::encode_public_id(domain, Some(kind_slug), raw)
                    .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
            );
        }
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

pub(crate) fn snapshot_response(
    manifest: &crate::snapshot::ServingSnapshotManifest,
    freshness: FreshnessState,
) -> SemanticSnapshotResponse {
    let mut versions = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::SnapshotBaseTableVersions,
    );
    for table in &manifest.body.base_publication.tables {
        versions.update(&table.table_code.to_be_bytes());
        versions.update(&table.delta_version.to_be_bytes());
        versions.update(table.schema_digest.as_bytes());
    }
    let bundle_version = |identity: &str| {
        identity
            .rsplit_once(':')
            .map_or_else(|| identity.to_owned(), |(_, version)| version.to_owned())
    };
    SemanticSnapshotResponse {
        snapshot_id: manifest.snapshot_id.clone(),
        workspace_id: manifest.body.workspace_id.clone(),
        repository_id: manifest.body.repository_id.clone(),
        worktree_id: manifest.body.worktree_id.clone(),
        source_generation: manifest.body.source.source_generation,
        source_inventory_digest: manifest.body.source.inventory_digest.clone(),
        durable_base_publication: manifest.body.base_publication.publication_id.clone(),
        base_table_version_digest: crate::integrity::frame_digest(versions.finalize()),
        overlay_generation: manifest.body.overlay.overlay_generation,
        overlay_checksum: manifest.body.overlay.overlay_digest.clone(),
        analysis_context_set_id: manifest.body.contexts.context_set_id.clone(),
        analysis_context_ids: manifest
            .body
            .contexts
            .records
            .iter()
            .map(|record| record.analysis_context_id.clone())
            .collect(),
        freshness_state: freshness,
        source_trust_state: manifest.body.source.source_trust_state.clone(),
        event_stream_health: manifest.body.source.event_stream_health.clone(),
        git_acceleration_status: manifest.body.source.git_acceleration_status.clone(),
        git_operation_summary: None,
        pending_update_count: manifest
            .body
            .source
            .admitted_event_sequence
            .saturating_sub(manifest.body.source.reconciled_event_sequence),
        ontology_version: bundle_version(&manifest.body.bundles.ontology_bundle_id),
        schema_bundle_version: bundle_version(&manifest.body.bundles.schema_bundle_id),
        provider_bundle_version: bundle_version(&manifest.body.bundles.provider_bundle_id),
        derivation_bundle_version: bundle_version(&manifest.body.bundles.derivation_bundle_id),
        query_language_version: bundle_version(&manifest.body.bundles.query_language_bundle_id),
        capability_summaries: Vec::new(),
        diagnostic_references: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn public_entity(byte: u8) -> String {
        crate::identity::encode_public_id(
            crate::identity::IdentityDomain::Entity,
            Some("unknown"),
            [byte; 16],
        )
        .unwrap()
    }

    fn public_fact(byte: u8) -> String {
        crate::identity::encode_public_id(
            crate::identity::IdentityDomain::RelationFact,
            Some("relation"),
            [byte; 16],
        )
        .unwrap()
    }

    fn eight_form_request() -> Vec<u8> {
        canonicalize_value(&serde_json::json!({
            "specification": SPECIFICATION,
            "version": VERSION,
            "semantic_request_id": "wp75-eight-form",
            "workspace_id": "workspace:00000000000000000000000000000000",
            "freshness_policy": "best_available_snapshot",
            "queries": [
                {"query_id":"entities","request":"find code entities","label":null,"input":null,"where":null,"limit":{"first":10,"offset":0}},
                {"query_id":"properties","request":"retrieve facts about code","label":null,"input":null,"where":null,"limit":{"first":10,"offset":0}},
                {"query_id":"relations","request":"follow code relationships","label":null,"input":null,"where":null,"limit":{"first":10,"offset":0}},
                {"query_id":"paths","request":"find connecting fact paths","label":null,"input":{"results":[{"results_of":"entities","select":"entities"}]},"where":null,"limit":{"first":10,"offset":0}},
                {"query_id":"patterns","request":"match a code fact pattern","label":null,"input":{"results":[{"results_of":"entities","select":"entities"}]},"where":null,"limit":{"first":10,"offset":0}},
                {"query_id":"combined","request":"combine result sets","label":null,"input":{"results":[{"results_of":"properties","select":"facts"},{"results_of":"relations","select":"facts"}]},"where":null,"limit":{"first":10,"offset":0}},
                {"query_id":"summary","request":"summarize objective facts","label":null,"input":{"results":[{"results_of":"combined","select":"groups"}]},"where":null,"limit":{"first":10,"offset":0}},
                {"query_id":"context","request":"retrieve source and syntax context","label":null,"input":{"results":[{"results_of":"paths","select":"paths"}]},"where":null,"limit":{"first":10,"offset":0}}
            ],
            "response_projection": {"canonical_semantic_identity":true,"coverage":true},
            "cost_budget": {"maximum_rows":80}
        }))
        .unwrap()
    }

    fn graph_plan(typed: &TypedSemanticRequest, form: QueryForm) -> GraphOperatorPlan {
        let block = typed
            .blocks
            .iter()
            .find(|block| block.form == form)
            .unwrap();
        graph_operator_plan(block).unwrap()
    }

    fn graph_edges() -> Vec<GraphEdge> {
        vec![
            GraphEdge {
                fact_id: [0x11; 16],
                source_id: [0x01; 16],
                target_id: [0x02; 16],
            },
            GraphEdge {
                fact_id: [0x12; 16],
                source_id: [0x02; 16],
                target_id: [0x03; 16],
            },
        ]
    }

    fn request() -> Vec<u8> {
        br#"{
          "specification":"composable semantic CPG fact query",
          "version":"1.3",
          "semantic_request_id":"gate-b",
          "workspace_id":"workspace:00000000000000000000000000000000",
          "freshness_policy":"best_available_snapshot",
          "queries":[
            {"query_id":"q1","request":"find code entities","label":null,"input":null,"where":null,"limit":{"first":10,"offset":0}},
            {"query_id":"q2","request":"retrieve facts about code","label":null,"input":null,"where":null,"limit":{"first":10,"offset":0}},
            {"query_id":"q3","request":"follow code relationships","label":null,"input":null,"where":null,"limit":{"first":10,"offset":0}}
          ],
          "response_projection":null,
          "cost_budget":{"maximum_rows":30}
        }"#
        .to_vec()
    }

    #[test]
    fn wp38_behavioral_acceptance() {
        let validated = validate_request(&request()).unwrap();
        assert_eq!(validated.request.queries.len(), 3);
        assert!(validated.request_digest.starts_with("b3:"));
        assert_eq!(
            query_sql(&validated.request.queries[0]).unwrap(),
            "SELECT * FROM entities ORDER BY entity_id LIMIT 10 OFFSET 0"
        );
    }

    #[test]
    fn wp38_negative_zero_state() {
        let unknown = String::from_utf8(request()).unwrap().replace(
            "\"response_projection\":null",
            "\"response_projection\":null,\"surprise\":true",
        );
        assert!(validate_request(unknown.as_bytes()).is_err());
        let duplicate = String::from_utf8(request())
            .unwrap()
            .replace("\"query_id\":\"q2\"", "\"query_id\":\"q1\"");
        assert!(validate_request(duplicate.as_bytes()).is_err());
        let over_budget = String::from_utf8(request())
            .unwrap()
            .replace("\"maximum_rows\":30", "\"maximum_rows\":29");
        assert!(validate_request(over_budget.as_bytes()).is_err());
        let incompatible_phrase = String::from_utf8(request()).unwrap().replace(
            "\"query_id\":\"q1\",\"request\":\"find code entities\",\"label\":null",
            "\"query_id\":\"q1\",\"request\":\"find code entities\",\"label\":\"call targets\"",
        );
        assert!(validate_request(incompatible_phrase.as_bytes()).is_err());
    }

    #[test]
    fn wp38_structural_acceptance() {
        let phrase = resolve_phrase(QueryForm::FindEntities, Some("syntax nodes"))
            .unwrap()
            .unwrap();
        assert_eq!(phrase.phrase_id, "Q51_SYNTAX_NODES");
        assert_eq!(phrase.plan_node_kind, "find-entities");
        let alias = resolve_phrase(QueryForm::FindEntities, Some("syntax occurrences"))
            .unwrap()
            .unwrap();
        assert_eq!(alias.phrase_id, phrase.phrase_id);
    }

    #[test]
    fn wp38_operational_acceptance() {
        let first = validate_request(&request()).unwrap();
        let second = validate_request(&first.canonical_bytes).unwrap();
        assert_eq!(first.canonical_bytes, second.canonical_bytes);
        assert_eq!(first.request_digest, second.request_digest);
        assert_eq!(
            first
                .request
                .queries
                .iter()
                .map(|query| query.request)
                .collect::<Vec<_>>(),
            vec![
                QueryForm::FindEntities,
                QueryForm::RetrieveFacts,
                QueryForm::FollowRelationships,
            ]
        );
    }

    #[test]
    fn wp62_behavioral_acceptance() {
        let filtered = String::from_utf8(request())
            .unwrap()
            .replace(
                r#""query_id":"q1","request":"find code entities","label":null,"input":null,"where":null"#,
                r#""query_id":"q1","request":"find code entities","label":null,"input":null,"where":{"entity_kind_codes":[10],"relation_kind_codes":[]}"#,
            )
            .replace(
                "\"response_projection\":null",
                "\"response_projection\":{\"canonical_semantic_identity\":true,\"coverage\":true}",
            );
        let typed = type_request(parse_request(filtered.as_bytes()).unwrap()).unwrap();
        assert_eq!(typed.blocks.len(), 3);
        assert_eq!(typed.blocks[0].form, QueryForm::FindEntities);
        assert_eq!(typed.blocks[0].output_role, ResultRole::Entities);
        assert_eq!(typed.blocks[0].canonical_order, ["entity_id"]);
        assert_eq!(
            typed.request.queries[0]
                .r#where
                .as_ref()
                .unwrap()
                .entity_kind_codes,
            [10]
        );
        assert_eq!(
            typed.request.response_projection,
            Some(BTreeMap::from([
                ("canonical_semantic_identity".to_owned(), true),
                ("coverage".to_owned(), true),
            ]))
        );
    }

    #[test]
    fn wp62_structural_acceptance() {
        let dependent = String::from_utf8(request()).unwrap().replace(
            r#""query_id":"q2","request":"retrieve facts about code","label":null,"input":null"#,
            r#""query_id":"q2","request":"retrieve facts about code","label":null,"input":{"results":[{"results_of":"q1","select":"entities"}]}"#,
        );
        let typed = validate_request(dependent.as_bytes()).unwrap();
        assert_eq!(typed.execution_order, ["q1", "q2", "q3"]);
        assert_eq!(typed.blocks[0].output_role, ResultRole::Entities);
        assert_eq!(typed.blocks[0].fan_out, 1);
        assert_eq!(typed.blocks[1].input_roles, [ResultRole::Entities]);
        assert_eq!(typed.blocks[1].source_pointer, "/queries/1");
        assert!(typed.blocks.iter().all(|block| {
            block.cancellation_required
                && block.maximum_memory_bytes > 0
                && !block.canonical_order.is_empty()
                && !block.coverage_effects.is_empty()
        }));
        let source = include_str!("semantic_query.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        for legacy in [
            "execution_state: &'static str",
            "availability_state: &'static str",
            "completeness_state: &'static str",
            "freshness_state: &'static str",
            "limit_state: &'static str",
            "dependency_state: &'static str",
        ] {
            assert!(
                !source.contains(legacy),
                "legacy state field remains: {legacy}"
            );
        }
    }

    #[test]
    fn wp62_negative_zero_state() {
        let evaluative = String::from_utf8(request())
            .unwrap()
            .replace("\"label\":null", "\"label\":\"safe to refactor\"");
        let error = validate_request(evaluative.as_bytes()).unwrap_err();
        assert!(error.to_string().contains("evaluative intent"));

        let cycle = String::from_utf8(request())
            .unwrap()
            .replace(
                r#""query_id":"q1","request":"find code entities","label":null,"input":null"#,
                r#""query_id":"q1","request":"find code entities","label":null,"input":{"results":[{"results_of":"q2","select":"facts"}]}"#,
            )
            .replace(
                r#""query_id":"q2","request":"retrieve facts about code","label":null,"input":null"#,
                r#""query_id":"q2","request":"retrieve facts about code","label":null,"input":{"results":[{"results_of":"q1","select":"entities"}]}"#,
            );
        assert!(
            validate_request(cycle.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("cycle")
        );

        let mismatch = String::from_utf8(request()).unwrap().replace(
            r#""query_id":"q2","request":"retrieve facts about code","label":null,"input":null"#,
            r#""query_id":"q2","request":"retrieve facts about code","label":null,"input":{"results":[{"results_of":"q1","select":"facts"}]}"#,
        );
        assert!(
            validate_request(mismatch.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("incompatible result role")
        );
    }

    #[test]
    fn wp75_behavioral_acceptance() {
        let typed = validate_request(&eight_form_request()).unwrap();
        assert_eq!(typed.blocks.len(), QUERY_FORM_VALUES.len());
        assert!(
            typed
                .blocks
                .iter()
                .all(|block| block.form.currently_supported())
        );

        let cancellation = crate::cancellation::Cancellation::with_check_interval(1);
        let path_input = BlockValues {
            entity_ids: vec![public_entity(1), public_entity(3)],
            ..BlockValues::default()
        };
        let paths = execute_graph_operator(
            &graph_plan(&typed, QueryForm::FindPaths),
            &path_input,
            &graph_edges(),
            &[],
            &cancellation,
        )
        .unwrap();
        assert_eq!(paths.completeness, CompletenessState::Complete);
        assert_eq!(paths.values.path_ids.len(), 1);
        assert_eq!(paths.batches[0].num_rows(), 1);

        let patterns = execute_graph_operator(
            &graph_plan(&typed, QueryForm::MatchPattern),
            &BlockValues {
                entity_ids: vec![public_entity(2)],
                ..BlockValues::default()
            },
            &graph_edges(),
            &[],
            &cancellation,
        )
        .unwrap();
        assert_eq!(patterns.values.group_ids.len(), 2);

        let combined = execute_graph_operator(
            &graph_plan(&typed, QueryForm::CombineResults),
            &BlockValues {
                fact_ids: vec![public_fact(0x11), public_fact(0x12)],
                ..BlockValues::default()
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
        assert_eq!(combined.values.group_ids.len(), 2);

        let summarized = execute_graph_operator(
            &graph_plan(&typed, QueryForm::SummarizeFacts),
            &combined.values,
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
        assert_eq!(summarized.coverage["summarized_values"], 2);

        let source_context = execute_graph_operator(
            &graph_plan(&typed, QueryForm::RetrieveSourceContext),
            &paths.values,
            &[],
            &["context:03030303030303030303030303030303".to_owned()],
            &cancellation,
        )
        .unwrap();
        assert_eq!(source_context.values.source_context_ids.len(), 1);
    }

    #[test]
    fn wp75_structural_acceptance() {
        let typed = validate_request(&eight_form_request()).unwrap();
        let forms = typed
            .blocks
            .iter()
            .map(|block| block.form)
            .collect::<BTreeSet<_>>();
        assert_eq!(forms.len(), QUERY_FORM_VALUES.len());
        for block in &typed.blocks {
            assert_eq!(block.output_role, block.form.output_role());
            assert_eq!(block.canonical_order, canonical_order(block.form));
            assert!(block.cancellation_required);
            if matches!(
                block.form,
                QueryForm::FindEntities | QueryForm::RetrieveFacts | QueryForm::FollowRelationships
            ) {
                assert!(block.form.table().is_ok());
                assert!(graph_operator_plan(block).is_err());
            } else {
                let plan = graph_operator_plan(block).unwrap();
                assert_eq!(plan.form, block.form);
                assert_eq!(plan.output_role, block.output_role);
                assert_eq!(plan.canonical_order, block.canonical_order);
                assert_eq!(plan.output_schema.fields().len(), 3);
                assert!(plan.cancellation_required);
                assert!(plan.maximum_memory_bytes > 0);
            }
        }
    }

    #[test]
    fn wp75_negative_zero_state() {
        let mut cycle: serde_json::Value = serde_json::from_slice(&eight_form_request()).unwrap();
        cycle["queries"][0]["input"] = serde_json::json!({
            "results":[{"results_of":"properties","select":"facts"}]
        });
        cycle["queries"][1]["input"] = serde_json::json!({
            "results":[{"results_of":"entities","select":"entities"}]
        });
        assert!(
            validate_request(&canonicalize_value(&cycle).unwrap())
                .unwrap_err()
                .to_string()
                .contains("cycle")
        );

        let mut mismatch: serde_json::Value =
            serde_json::from_slice(&eight_form_request()).unwrap();
        mismatch["queries"][3]["input"] = serde_json::json!({
            "results":[{"results_of":"properties","select":"facts"}]
        });
        assert!(
            validate_request(&canonicalize_value(&mismatch).unwrap())
                .unwrap_err()
                .to_string()
                .contains("incompatible result role")
        );

        let typed = validate_request(&eight_form_request()).unwrap();
        let cancelled = crate::cancellation::Cancellation::with_check_interval(1);
        cancelled.cancel();
        assert!(
            execute_graph_operator(
                &graph_plan(&typed, QueryForm::MatchPattern),
                &BlockValues::default(),
                &graph_edges(),
                &[],
                &cancelled,
            )
            .unwrap_err()
            .to_string()
            .contains("cancelled")
        );

        let cancellation = crate::cancellation::Cancellation::with_check_interval(1);
        let empty = execute_graph_operator(
            &graph_plan(&typed, QueryForm::MatchPattern),
            &BlockValues::default(),
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
        assert_eq!(empty.completeness, CompletenessState::Indeterminate);
        assert_eq!(empty.coverage["negative_proof_available"], 0);

        let mut bounded = graph_plan(&typed, QueryForm::FindPaths);
        bounded.maximum_depth = 1;
        let overflow = execute_graph_operator(
            &bounded,
            &BlockValues {
                entity_ids: vec![public_entity(1), public_entity(3)],
                ..BlockValues::default()
            },
            &graph_edges(),
            &[],
            &cancellation,
        )
        .unwrap();
        assert_eq!(overflow.completeness, CompletenessState::Indeterminate);
        assert_eq!(overflow.coverage["depth_bound_reached"], 1);

        bounded.maximum_memory_bytes = 1;
        assert!(
            execute_graph_operator(
                &bounded,
                &BlockValues::default(),
                &graph_edges(),
                &[],
                &cancellation,
            )
            .unwrap_err()
            .to_string()
            .contains("memory bound")
        );
    }

    #[test]
    fn wp75_operational_acceptance() {
        let typed = validate_request(&eight_form_request()).unwrap();
        assert_eq!(
            typed
                .blocks
                .iter()
                .map(|block| block.form.registry_slug())
                .collect::<BTreeSet<_>>(),
            QUERY_FORM_VALUES
                .iter()
                .map(|entry| entry.slug)
                .collect::<BTreeSet<_>>()
        );
        let plan = graph_plan(&typed, QueryForm::FindPaths);
        let input = BlockValues {
            entity_ids: vec![public_entity(1), public_entity(3)],
            ..BlockValues::default()
        };
        let cancellation = crate::cancellation::Cancellation::with_check_interval(1);
        let first =
            execute_graph_operator(&plan, &input, &graph_edges(), &[], &cancellation).unwrap();
        let second = execute_graph_operator(
            &plan,
            &input,
            &graph_edges().into_iter().rev().collect::<Vec<_>>(),
            &[],
            &cancellation,
        )
        .unwrap();
        assert_eq!(first.values.path_ids, second.values.path_ids);
        assert_eq!(
            crate::fabric::batch_checksum(&first.batches[0]).unwrap(),
            crate::fabric::batch_checksum(&second.batches[0]).unwrap()
        );
    }
}
