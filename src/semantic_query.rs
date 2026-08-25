//! Bounded semantic-query ingress and DataFusion execution over one pinned snapshot.

use std::collections::{BTreeMap, BTreeSet};

use arrow_array::{Array as _, FixedSizeBinaryArray};
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
        matches!(
            self,
            Self::FindEntities | Self::RetrieveFacts | Self::FollowRelationships
        )
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
            _ => Err(SemanticQueryError::Invalid(
                "query form is registered but not active in the current execution profile"
                    .to_owned(),
            )),
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
    pub table: &'static str,
    pub identity_column: &'static str,
    pub plan: LogicalPlan,
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

fn id16_scalar(value: &str, expected_prefix: &str) -> Result<ScalarValue, SemanticQueryError> {
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
    let mut bytes = Vec::with_capacity(16);
    for pair in encoded.as_bytes().chunks_exact(2) {
        let pair = std::str::from_utf8(pair)
            .map_err(|_| SemanticQueryError::Invalid("identity is not UTF-8 hex".to_owned()))?;
        bytes.push(
            u8::from_str_radix(pair, 16)
                .map_err(|_| SemanticQueryError::Invalid("identity is not hex".to_owned()))?,
        );
    }
    Ok(ScalarValue::FixedSizeBinary(16, Some(bytes)))
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
        table,
        identity_column,
        plan,
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
        blocks.push(lower_relational_block(session, block, query).await?);
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
    let manifest = session.snapshot_manifest();
    let bound = bind_request(session, &validated).await?;
    let mut results = Vec::with_capacity(validated.request.queries.len());
    let mut entities = BTreeMap::new();
    let mut facts = BTreeMap::new();
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
        let result = session
            .query_plan(&block.typed.block_id, block.plan.clone())
            .await?;
        let produced_rows = result.artifact.output_row_count;
        let mut ids = response_ids(&result.batches, query.request)?;
        let limit_reached = produced_rows > block.typed.limit.first;
        ids.truncate(block.typed.limit.first);
        let returned_row_count = ids.len();
        let result_checksum = b3(ids.join("\0").as_bytes());
        let (entity_ids, fact_ids) = match query.request {
            QueryForm::FindEntities => (ids, Vec::new()),
            QueryForm::RetrieveFacts | QueryForm::FollowRelationships => (Vec::new(), ids),
            _ => {
                return Err(SemanticQueryError::Invalid(
                    "inactive query form reached execution".to_owned(),
                ));
            }
        };
        for entity_id in &entity_ids {
            entities.insert(
                entity_id.clone(),
                BTreeMap::from([("entity_id".to_owned(), entity_id.clone())]),
            );
        }
        for fact_id in &fact_ids {
            facts.insert(
                fact_id.clone(),
                BTreeMap::from([("fact_id".to_owned(), fact_id.clone())]),
            );
        }
        let phrase = resolve_phrase(query.request, query.label.as_deref())?;
        let mut resolved_semantics = BTreeMap::from([
            ("table".to_owned(), query.request.table()?.to_owned()),
            (
                "order_key".to_owned(),
                query.request.order_key()?.to_owned(),
            ),
        ]);
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
            availability_state: QueryAvailabilityState::Available,
            completeness_state: if limit_reached {
                CompletenessState::Partial
            } else {
                CompletenessState::Complete
            },
            freshness_state: freshness,
            limit_state: if limit_reached {
                LimitState::ExplicitLimitReached
            } else {
                LimitState::NotApplied
            },
            dependency_state: if block.typed.dependencies.is_empty() {
                DependencyState::NotApplicable
            } else {
                DependencyState::Ready
            },
            resolved_semantics,
            entity_ids,
            fact_ids,
            path_ids: Vec::new(),
            group_ids: Vec::new(),
            source_context_ids: manifest
                .body
                .contexts
                .records
                .iter()
                .map(|record| record.analysis_context_id.clone())
                .collect(),
            coverage: BTreeMap::from([(
                "returned_rows".to_owned(),
                u64::try_from(returned_row_count).unwrap_or(u64::MAX),
            )]),
            errors: Vec::new(),
            notices: Vec::new(),
            output_row_count: returned_row_count,
            result_checksum,
        });
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
        availability_state: QueryAvailabilityState::Available,
        completeness_state: aggregate_completeness,
        freshness_state: freshness,
        limit_state: aggregate_limit,
        successful_query_count: results.len(),
        failed_query_count: 0,
        not_executed_dependency_count: 0,
        snapshot,
        entities,
        facts,
        paths: BTreeMap::new(),
        groups: BTreeMap::new(),
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
}
