//! Bounded semantic-query ingress and DataFusion execution over one pinned snapshot.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::sync::Arc;

#[cfg(test)]
use arrow_array::ListArray;
use arrow_array::{
    Array as _, FixedSizeBinaryArray, Int16Array, Int32Array, RecordBatch, StringArray, UInt64Array,
};
use arrow_schema::{DataType, SchemaRef};
use datafusion::common::ScalarValue;
use datafusion::datasource::{MemTable, provider_as_source};
use datafusion::functions_aggregate::expr_fn::count;
use datafusion::logical_expr::{Expr, JoinType, LogicalPlan, LogicalPlanBuilder, col, lit};
use petgraph::Direction;
use petgraph::algo::{kosaraju_scc, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{Data, EdgeRef as _, IntoEdgesDirected, Reversed, VisitMap as _, Visitable};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use thiserror::Error;

use crate::contracts::jcs::{CanonicalJsonError, canonicalize_slice, canonicalize_value};
use crate::fabric::{
    QueryArtifactStage, QueryArtifactStageState, QueryExecutionArtifactAccumulator,
    QueryExecutionArtifactEvidence, QueryExecutionContext, QueryPlanArtifact, ServingQueryError,
    ServingQuerySession,
};
pub use crate::model_generated::query_forms::{
    PatternBinding, PatternRelationship, PriorResultReference, QUERY_FORM_CONTRACT_DIGEST,
    QUERY_FORM_CONTRACT_ID, QUERY_FORM_CONTRACT_VERSION, QUERY_FORM_CONTRACTS, QueryFormDescriptor,
    ResultRole, ReturnLimit, ReturnSpec, SemanticQueryClause, SemanticReference,
};
use crate::ontology_executor::OntologyProgramCompiler;
pub use crate::registries::QueryForm;
use crate::registries::{
    COMPLETENESS_STATE_VALUES, CompletenessState, DEPENDENCY_STATE_VALUES, DependencyState,
    FRESHNESS_STATE_VALUES, FreshnessState, LIMIT_STATE_VALUES, LimitState,
    QUERY_AVAILABILITY_STATE_VALUES, QUERY_EXECUTION_STATE_VALUES, QUERY_FORM_VALUES,
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
        match self {
            Self::FindEntities
            | Self::RetrieveFacts
            | Self::FollowRelationships
            | Self::FindPaths
            | Self::MatchPattern
            | Self::CombineResults
            | Self::SummarizeFacts
            | Self::RetrieveSourceContext => true,
        }
    }

    pub(crate) const fn executor_registered(self) -> bool {
        match self {
            Self::FindEntities
            | Self::RetrieveFacts
            | Self::FollowRelationships
            | Self::FindPaths
            | Self::MatchPattern
            | Self::CombineResults
            | Self::SummarizeFacts
            | Self::RetrieveSourceContext => true,
        }
    }

    pub(crate) fn registry_slug(self) -> &'static str {
        QUERY_FORM_VALUES
            .iter()
            .find(|entry| entry.code == self as u16)
            .expect("generated QueryForm and QUERY_FORM_VALUES are one authority")
            .slug
    }

    const fn plan_node_kind(self) -> &'static str {
        match self {
            Self::FindEntities => "find-entities",
            Self::RetrieveFacts => "retrieve-facts",
            Self::FollowRelationships => "follow-relationships",
            Self::FindPaths => "find-paths",
            Self::MatchPattern => "match-pattern",
            Self::CombineResults => "combine-results",
            Self::SummarizeFacts => "summarize-facts",
            Self::RetrieveSourceContext => "retrieve-source-context",
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryLimit {
    pub first: usize,
    #[serde(default)]
    pub offset: usize,
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
    pub resolved_phrases: Vec<ResolvedPhrase>,
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

/// One governed phrase resolved to its registry-owned semantic projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedPhrase {
    pub source_pointer: String,
    pub phrase_id: String,
    pub canonical_text: String,
    pub contract_family: String,
    pub contract_code: String,
    pub language_code: Option<u16>,
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
    Relational(Box<RelationalOperatorPlan>),
    Graph(GraphOperatorPlan),
}

/// Application-owned relational semantics compiled only to built-in DataFusion nodes.
#[derive(Clone, Debug)]
pub struct RelationalOperatorPlan {
    pub form: QueryForm,
    pub source_tables: Vec<&'static str>,
    pub identity_column: &'static str,
    pub output_schema: SchemaRef,
    pub canonical_order: Vec<&'static str>,
    pub template_plan: LogicalPlan,
    pub runtime: RelationalRuntime,
}

/// The parameterized relational operation retained beside its transparent plan exemplar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationalRuntime {
    Snapshot,
    Combine {
        operation: SetOperation,
        identity_role: ResultRole,
    },
    Summarize {
        summary_names: Vec<String>,
        group_by: Vec<String>,
    },
    SourceContext {
        context_fields: Vec<String>,
        text_handling: SourceTextHandling,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetOperation {
    Union,
    Intersection,
    Difference,
    InnerJoin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceTextHandling {
    Omit,
    ExactBytes,
    DecodedText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GraphDirection {
    Outgoing,
    Incoming,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GraphDistance {
    Direct,
    Transitive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathPolicy {
    DeterministicShortest,
    AllShortest,
    AllSimple,
}

/// Semantic graph operation retained independently of the physical petgraph projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum GraphSemantics {
    Follow {
        direction: GraphDirection,
        distance: GraphDistance,
        stop_when: Vec<String>,
    },
    Paths {
        direction: GraphDirection,
        path_policy: PathPolicy,
        through_codes: Vec<i32>,
    },
    Pattern {
        bindings: Vec<PatternBinding>,
        relationships: Vec<PatternRelationship>,
    },
}

/// Typed bounded graph plan executed outside DataFusion.
#[derive(Clone, Debug)]
pub struct GraphOperatorPlan {
    pub form: QueryForm,
    pub block_id: String,
    pub semantics: GraphSemantics,
    pub relationship_kind_codes: Vec<i32>,
    pub certainty_codes: Vec<i16>,
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
    pub plan_template_id: String,
    pub bound_query_id: String,
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
    pub plan_artifacts: Vec<QueryPlanArtifact>,
}

/// Phase-aware backend outcome that never discards artifacts captured before termination.
#[derive(Debug)]
pub enum SemanticTerminalOutcome {
    Succeeded {
        response: Box<ExecutedSemanticResponse>,
        evidence: QueryExecutionArtifactEvidence,
    },
    Failed {
        error: SemanticQueryError,
        evidence: QueryExecutionArtifactEvidence,
    },
    Cancelled {
        error: SemanticQueryError,
        evidence: QueryExecutionArtifactEvidence,
    },
}

#[derive(Debug, Error)]
pub enum SemanticQueryError {
    #[error("INVALID_REQUEST_SCHEMA:SEMANTIC_QUERY_INVALID:{0}")]
    Invalid(String),
    #[error("{code}:{phase}:{pointer}:{message}")]
    Phase {
        code: &'static str,
        phase: &'static str,
        pointer: String,
        message: String,
    },
    #[error(transparent)]
    Canonical(#[from] CanonicalJsonError),
    #[error(transparent)]
    Serving(#[from] ServingQueryError),
}

fn phase_error(
    code: &'static str,
    phase: &'static str,
    pointer: impl Into<String>,
    message: impl Into<String>,
) -> SemanticQueryError {
    SemanticQueryError::Phase {
        code,
        phase,
        pointer: pointer.into(),
        message: message.into(),
    }
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

#[cfg(test)]
fn ontology_compiler()
-> Result<crate::ontology_executor::OntologyProgramCompiler, SemanticQueryError> {
    let package = crate::ontology_program::build_ontology_program_package(
        &crate::ontology_program::OntologyPackagingProfile::default(),
    )
    .map_err(|error| {
        phase_error(
            "ONTOLOGY_PROGRAM_INVALID",
            "semantic_binding",
            "",
            error.to_string(),
        )
    })?;
    crate::ontology_executor::OntologyProgramCompiler::decode(&package).map_err(|error| {
        phase_error(
            "ONTOLOGY_PROGRAM_INVALID",
            "semantic_binding",
            "",
            error.to_string(),
        )
    })
}

fn resolve_phrase_in<'a>(
    compiler: &'a crate::ontology_executor::OntologyProgramCompiler,
    form: QueryForm,
    label: Option<&str>,
) -> Result<Option<&'a crate::ontology_executor::DecodedQueryPhrase>, SemanticQueryError> {
    let Some(label) = label else {
        return Ok(None);
    };
    if label.is_empty() || label.len() > 256 {
        return Err(SemanticQueryError::Invalid(
            "semantic phrase is empty or exceeds its bound".to_owned(),
        ));
    }
    let plan_node_kind = form.plan_node_kind();
    let mut matches = compiler.query_phrases.values().filter(|entry| {
        (entry.canonical_text == label || entry.accepted_aliases.iter().any(|alias| alias == label))
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

#[cfg(test)]
fn resolve_phrase(
    form: QueryForm,
    label: Option<&str>,
) -> Result<Option<crate::ontology_executor::DecodedQueryPhrase>, SemanticQueryError> {
    let compiler = ontology_compiler()?;
    Ok(resolve_phrase_in(&compiler, form, label)?.cloned())
}

fn resolved_phrase_in(
    compiler: &crate::ontology_executor::OntologyProgramCompiler,
    form: QueryForm,
    value: &str,
    pointer: impl Into<String>,
) -> Result<ResolvedPhrase, SemanticQueryError> {
    let pointer = pointer.into();
    let entry = resolve_phrase_in(compiler, form, Some(value)).map_err(|error| {
        phase_error(
            "SEMANTIC_PHRASE_UNSUPPORTED",
            "semantic_binding",
            &pointer,
            error.to_string(),
        )
    })?;
    let entry = entry.ok_or_else(|| {
        phase_error(
            "SEMANTIC_PHRASE_REQUIRED",
            "semantic_binding",
            &pointer,
            "a governed semantic phrase is required",
        )
    })?;
    let language_code = if entry
        .required_modifiers
        .iter()
        .any(|value| value == "python")
    {
        Some(crate::registries::Language::Python as u16)
    } else if entry.required_modifiers.iter().any(|value| value == "rust") {
        Some(crate::registries::Language::Rust as u16)
    } else {
        None
    };
    Ok(ResolvedPhrase {
        source_pointer: pointer,
        phrase_id: entry.phrase_id.clone(),
        canonical_text: entry.canonical_text.clone(),
        contract_family: entry.contract_family.clone(),
        contract_code: entry.contract_code.clone(),
        language_code,
    })
}

fn resolve_query_phrases(
    compiler: &crate::ontology_executor::OntologyProgramCompiler,
    query: &SemanticQueryClause,
    query_pointer: &str,
) -> Result<Vec<ResolvedPhrase>, SemanticQueryError> {
    let form = query.form();
    let mut phrases = Vec::new();
    match query {
        SemanticQueryClause::FindEntities { looking_for, .. } => {
            phrases.push(resolved_phrase_in(
                compiler,
                form,
                looking_for,
                format!("{query_pointer}/looking_for"),
            )?);
        }
        SemanticQueryClause::RetrieveFacts { facts, .. } => {
            for (index, phrase) in facts.iter().enumerate() {
                phrases.push(resolved_phrase_in(
                    compiler,
                    form,
                    phrase,
                    format!("{query_pointer}/facts/{index}"),
                )?);
            }
        }
        SemanticQueryClause::FollowRelationships { relationship, .. } => {
            phrases.push(resolved_phrase_in(
                compiler,
                form,
                relationship,
                format!("{query_pointer}/relationship"),
            )?);
        }
        SemanticQueryClause::FindPaths { through, .. } => {
            for (index, phrase) in through.iter().enumerate() {
                phrases.push(resolved_phrase_in(
                    compiler,
                    form,
                    phrase,
                    format!("{query_pointer}/through/{index}"),
                )?);
            }
        }
        SemanticQueryClause::MatchPattern {
            bindings,
            relationships,
            ..
        } => {
            for (index, binding) in bindings.iter().enumerate() {
                // Binding selectors use the find-entities phrase vocabulary even though the
                // enclosing operator is the pattern matcher.
                phrases.push(resolved_phrase_in(
                    compiler,
                    QueryForm::FindEntities,
                    &binding.looking_for,
                    format!("{query_pointer}/bindings/{index}/looking_for"),
                )?);
            }
            for (index, relationship) in relationships.iter().enumerate() {
                phrases.push(resolved_phrase_in(
                    compiler,
                    QueryForm::FollowRelationships,
                    &relationship.relationship,
                    format!("{query_pointer}/relationships/{index}/relationship"),
                )?);
            }
        }
        SemanticQueryClause::SummarizeFacts { summaries, .. } => {
            for (index, phrase) in summaries.iter().enumerate() {
                phrases.push(resolved_phrase_in(
                    compiler,
                    form,
                    phrase,
                    format!("{query_pointer}/summaries/{index}"),
                )?);
            }
        }
        SemanticQueryClause::CombineResults { .. }
        | SemanticQueryClause::RetrieveSourceContext { .. } => {}
    }
    Ok(phrases)
}

fn query_where_conditions(query: &SemanticQueryClause) -> &[String] {
    match query {
        SemanticQueryClause::FindEntities {
            where_conditions, ..
        }
        | SemanticQueryClause::RetrieveFacts {
            where_conditions, ..
        }
        | SemanticQueryClause::FollowRelationships {
            where_conditions, ..
        }
        | SemanticQueryClause::FindPaths {
            where_conditions, ..
        }
        | SemanticQueryClause::MatchPattern {
            where_conditions, ..
        }
        | SemanticQueryClause::SummarizeFacts {
            where_conditions, ..
        }
        | SemanticQueryClause::RetrieveSourceContext {
            where_conditions, ..
        } => where_conditions,
        SemanticQueryClause::CombineResults { .. } => &[],
    }
}

fn validate_structural_conditions(
    query: &SemanticQueryClause,
    pointer: &str,
) -> Result<(), SemanticQueryError> {
    for (index, condition) in query_where_conditions(query).iter().enumerate() {
        if !matches!(
            condition.as_str(),
            "entities whose semantic kind is function"
                | "language is Python"
                | "language is Rust"
                | "certainty is exact"
                | "certainty is sound may"
                | "certainty is unresolved"
        ) {
            return Err(phase_error(
                "SEMANTIC_CONDITION_UNSUPPORTED",
                "structural_policy",
                format!("{pointer}/where/{index}"),
                "condition is not a governed typed predicate",
            ));
        }
    }
    Ok(())
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
    QUERY_FORM_CONTRACTS
        .iter()
        .find(|descriptor| descriptor.code == form as u16)
        .expect("generated form enum and form contract are one authority")
        .canonical_order
        .to_vec()
}

/// Type-check block roles, dependency topology, resource contracts, and semantic policy.
///
/// # Errors
///
/// Rejects unknown/inactive forms, cycles, role mismatches, invalid source identifiers,
/// evaluative intent, and requests outside the bounded execution profile.
#[allow(clippy::too_many_lines)] // One pass validates the complete request DAG and role contract.
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
        let query_id = query.query_id();
        let form = query.form();
        if !valid_id(query_id, 128) || !ids.insert(query_id) {
            return Err(SemanticQueryError::Invalid(
                "query IDs must be unique bounded identifiers".to_owned(),
            ));
        }
        if !form.currently_supported() {
            return Err(SemanticQueryError::Invalid(
                "query form is registered but not active in the current execution profile"
                    .to_owned(),
            ));
        }
        forms.insert(query_id.to_owned(), form);
        if query
            .direct_entity_ids()
            .into_iter()
            .chain(query.direct_fact_ids())
            .any(|identity| !valid_id(identity, 192))
        {
            return Err(SemanticQueryError::Invalid(
                "query input contains an invalid public identity".to_owned(),
            ));
        }
        let limit = QueryLimit {
            first: query.maximum_results(),
            offset: 0,
        };
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
        let query_id = query.query_id();
        let form = query.form();
        let references = query.result_references();
        let mut seen = BTreeSet::new();
        for reference in &references {
            if reference.results_of == query_id || !seen.insert(&reference.results_of) {
                return Err(SemanticQueryError::Invalid(
                    "query dependency is self-referential or duplicated".to_owned(),
                ));
            }
            let producer = forms.get(&reference.results_of).ok_or_else(|| {
                SemanticQueryError::Invalid("query dependency names an unknown block".to_owned())
            })?;
            if producer.output_role() != reference.select || !form.accepts_role(reference.select) {
                return Err(SemanticQueryError::Invalid(format!(
                    "query dependency {} has incompatible result role at /queries/{}/input/results",
                    reference.results_of,
                    request
                        .queries
                        .iter()
                        .position(|candidate| candidate.query_id() == query_id)
                        .unwrap_or_default()
                )));
            }
            *fan_out.entry(reference.results_of.clone()).or_default() += 1;
        }
        dependencies.insert(
            query_id.to_owned(),
            references
                .iter()
                .map(|reference| reference.results_of.clone())
                .collect(),
        );
    }
    let mut dependency_graph =
        DiGraph::<String, (), u32>::with_capacity(dependencies.len(), fan_out.values().sum());
    let mut dependency_nodes = BTreeMap::new();
    for query_id in dependencies.keys() {
        let node = dependency_graph
            .try_add_node(query_id.clone())
            .map_err(|error| {
                phase_error(
                    "QUERY_DEPENDENCY_INDEX_OVERFLOW",
                    "type_check",
                    "/queries",
                    error.to_string(),
                )
            })?;
        dependency_nodes.insert(query_id.clone(), node);
    }
    for (consumer, producers) in &dependencies {
        for producer in producers {
            dependency_graph
                .try_add_edge(dependency_nodes[producer], dependency_nodes[consumer], ())
                .map_err(|error| {
                    phase_error(
                        "QUERY_DEPENDENCY_INDEX_OVERFLOW",
                        "type_check",
                        "/queries",
                        error.to_string(),
                    )
                })?;
        }
    }
    if toposort(&dependency_graph, None).is_err() {
        let mut cycle = kosaraju_scc(&dependency_graph)
            .into_iter()
            .filter(|component| {
                component.len() > 1
                    || component.first().is_some_and(|node| {
                        dependency_graph
                            .edges_connecting(*node, *node)
                            .next()
                            .is_some()
                    })
            })
            .map(|component| {
                let mut members = component
                    .into_iter()
                    .map(|node| dependency_graph[node].clone())
                    .collect::<Vec<_>>();
                members.sort();
                members
            })
            .min()
            .unwrap_or_default();
        cycle.sort();
        return Err(phase_error(
            "QUERY_DEPENDENCY_CYCLE",
            "type_check",
            "/queries",
            format!(
                "query dependency graph contains cycle [{}]",
                cycle.join(",")
            ),
        ));
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
                let degree = indegree.get_mut(candidate).ok_or_else(|| {
                    SemanticQueryError::Invalid(
                        "dependency graph and indegree map diverged".to_owned(),
                    )
                })?;
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(candidate.clone());
                }
            }
        }
    }
    debug_assert_eq!(execution_order.len(), request.queries.len());
    let blocks = request
        .queries
        .iter()
        .enumerate()
        .map(|(index, query)| {
            let query_id = query.query_id();
            let form = query.form();
            let source_pointer = format!("/queries/{index}");
            validate_structural_conditions(query, &source_pointer)?;
            let references = query.result_references();
            let limit = QueryLimit {
                first: query.maximum_results(),
                offset: 0,
            };
            let input_roles = references
                .iter()
                .map(|reference| reference.select)
                .collect::<Vec<_>>();
            Ok(TypedQueryBlock {
                block_id: query_id.to_owned(),
                source_pointer,
                form,
                input_roles,
                output_role: form.output_role(),
                // Phrase resolution is package-dependent and therefore occurs only after a
                // retained ontology package is selected by a serving lease.
                resolved_phrases: Vec::new(),
                dependencies: dependencies.get(query_id).cloned().unwrap_or_default(),
                fan_in: references.len(),
                fan_out: fan_out.get(query_id).copied().unwrap_or_default(),
                coverage_prerequisites: BTreeSet::from(["snapshot_pinned".to_owned()]),
                coverage_effects: BTreeSet::from([format!(
                    "{}_rows_observed",
                    form.registry_slug().replace(' ', "_")
                )]),
                canonical_order: canonical_order(form),
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

/// Reject a syntactically and semantically valid request when any form lacks a proved executor.
///
/// This fence is called by the production service before snapshot acquisition, so generated
/// contract coverage never becomes an unsupported capability advertisement.
///
/// # Errors
///
/// Returns the governed unsupported-capability error when any requested form lacks a registered
/// production executor.
pub fn require_registered_executors(
    request: &ValidatedSemanticRequest,
) -> Result<(), SemanticQueryError> {
    if let Some(form) = request
        .blocks
        .iter()
        .map(|block| block.form)
        .find(|form| !form.executor_registered())
    {
        return Err(SemanticQueryError::Invalid(format!(
            "query form '{}' is governed but its production executor is not registered",
            form.registry_slug()
        )));
    }
    Ok(())
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
    for (index, pair) in encoded.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let pair = std::str::from_utf8(pair)
            .map_err(|_| SemanticQueryError::Invalid("identity is not UTF-8 hex".to_owned()))?;
        bytes[index] = u8::from_str_radix(pair, 16)
            .map_err(|_| SemanticQueryError::Invalid("identity is not hex".to_owned()))?;
    }
    Ok(bytes)
}

fn id16_literal(
    value: &str,
    expected_prefix: &str,
    domain: &str,
) -> Result<Expr, SemanticQueryError> {
    crate::schema_registry::DomainTypedLiteral::new(domain, id16_bytes(value, expected_prefix)?)
        .map(crate::schema_registry::DomainTypedLiteral::into_expr)
        .map_err(|error| SemanticQueryError::Invalid(error.to_string()))
}

fn any_of(column: &'static str, values: Vec<ScalarValue>) -> Option<Expr> {
    (!values.is_empty())
        .then(|| col(column).in_list(values.into_iter().map(lit).collect::<Vec<_>>(), false))
}

fn any_of_expr(column: &'static str, values: Vec<Expr>) -> Option<Expr> {
    (!values.is_empty()).then(|| col(column).in_list(values, false))
}

fn all_of(expressions: Vec<Expr>) -> Option<Expr> {
    expressions.into_iter().reduce(Expr::and)
}

fn compiled_enum_code(
    compiler: &OntologyProgramCompiler,
    domain: &str,
    name: &str,
) -> Result<i16, SemanticQueryError> {
    compiler
        .enum_values
        .get(&(domain.to_owned(), name.to_owned()))
        .copied()
        .and_then(|value| i16::try_from(value).ok())
        .ok_or_else(|| {
            phase_error(
                "ONTOLOGY_ENUM_VALUE_MISSING",
                "semantic_binding",
                "",
                format!("retained ontology package has no code16 value {domain}.{name}"),
            )
        })
}

fn query_projection_codes(
    compiler: &OntologyProgramCompiler,
    phrases: &[ResolvedPhrase],
    target_kind: &str,
) -> Vec<ScalarValue> {
    phrases
        .iter()
        .flat_map(|phrase| {
            compiler
                .query_projection_codes
                .get(&(phrase.phrase_id.clone(), target_kind.to_owned()))
                .into_iter()
                .flatten()
                .copied()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|code| ScalarValue::Int32(Some(code)))
        .collect()
}

fn entity_kind_codes(
    compiler: &OntologyProgramCompiler,
    phrases: &[ResolvedPhrase],
) -> Vec<ScalarValue> {
    query_projection_codes(compiler, phrases, "entity_kind")
}

fn relation_kind_codes(
    compiler: &OntologyProgramCompiler,
    phrases: &[ResolvedPhrase],
) -> Vec<ScalarValue> {
    query_projection_codes(compiler, phrases, "relation_kind")
}

fn property_kind_codes(
    compiler: &OntologyProgramCompiler,
    phrases: &[ResolvedPhrase],
) -> Vec<ScalarValue> {
    query_projection_codes(compiler, phrases, "property_kind")
}

fn language_predicate(column: &'static str, phrases: &[ResolvedPhrase]) -> Option<Expr> {
    let values = phrases
        .iter()
        .filter_map(|phrase| phrase.language_code)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|code| ScalarValue::Int16(Some(i16::try_from(code).unwrap_or(i16::MAX))))
        .collect::<Vec<_>>();
    any_of(column, values)
}

fn compiled_condition_predicate(
    compiler: &OntologyProgramCompiler,
    condition: &str,
    qualifier: &str,
) -> Result<Expr, SemanticQueryError> {
    compiler
        .lower_phrase_text_for_qualifier(condition, qualifier)
        .map_err(|error| {
            phase_error(
                "SEMANTIC_PHRASE_UNSUPPORTED",
                "logical_compile",
                "",
                error.to_string(),
            )
        })
}

fn condition_predicates(
    compiler: &OntologyProgramCompiler,
    query: &SemanticQueryClause,
    qualifier: &str,
) -> Result<Vec<Expr>, SemanticQueryError> {
    query_where_conditions(query)
        .iter()
        .map(|condition| compiled_condition_predicate(compiler, condition, qualifier))
        .collect()
}

fn bounded_plan(
    builder: LogicalPlanBuilder,
    order: &[&'static str],
    limit: QueryLimit,
) -> Result<LogicalPlan, SemanticQueryError> {
    let fetch = limit.first.checked_add(1).ok_or_else(|| {
        phase_error(
            "QUERY_BOUND_OVERFLOW",
            "logical_compile",
            "",
            "fetch bound overflow",
        )
    })?;
    Ok(builder
        .sort(
            order
                .iter()
                .map(|column| col(*column).sort(true, true))
                .collect::<Vec<_>>(),
        )?
        .limit(limit.offset, Some(fetch))?
        .build()?)
}

async fn compile_find_entities(
    session: &ServingQuerySession,
    compiler: &OntologyProgramCompiler,
    typed: &TypedQueryBlock,
    query: &SemanticQueryClause,
) -> Result<LogicalPlan, SemanticQueryError> {
    let mut predicates = condition_predicates(compiler, query, "entity")?;
    let identities = query
        .direct_entity_ids()
        .into_iter()
        .map(|value| id16_literal(value, "entity:", "entity"))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(predicate) = any_of_expr("entity.entity_id", identities) {
        predicates.push(predicate);
    }
    let kinds = entity_kind_codes(compiler, &typed.resolved_phrases);
    if let Some(predicate) = any_of("entity.entity_kind_code", kinds) {
        predicates.push(predicate);
    }
    if let Some(predicate) = language_predicate("entity.language", &typed.resolved_phrases) {
        predicates.push(predicate);
    }
    let mut entities =
        LogicalPlanBuilder::from(session.table_plan("entities").await?).alias("entity")?;
    if let Some(predicate) = all_of(predicates) {
        entities = entities.filter(predicate)?;
    }
    let files = LogicalPlanBuilder::from(session.table_plan("files").await?)
        .alias("source_file")?
        .build()?;
    let builder = entities
        .join_on(
            files,
            JoinType::Left,
            [col("entity.file_id").eq(col("source_file.file_id"))],
        )?
        .project(vec![
            col("source_file.path_display").alias("source_file_path"),
            col("entity.start_byte").alias("span_start"),
            col("entity.entity_kind_code").alias("semantic_kind"),
            col("entity.qualified_name"),
            col("entity.entity_id"),
            lit(query.query_id()).alias("origin_query_id"),
            lit(ScalarValue::Int16(Some(compiled_enum_code(
                compiler,
                "EVIDENCE_CERTAINTY",
                "SOURCE_EXACT",
            )?)))
            .alias("certainty_code"),
        ])?;
    bounded_plan(builder, &typed.canonical_order, typed.limit)
}

fn fact_about_predicates(
    query: &SemanticQueryClause,
    subject_column: &'static str,
    object_column: Option<&'static str>,
) -> Result<Option<Expr>, SemanticQueryError> {
    let entities = query
        .direct_entity_ids()
        .into_iter()
        .map(|value| id16_literal(value, "entity:", "entity"))
        .collect::<Result<Vec<_>, _>>()?;
    if entities.is_empty() {
        return Ok(None);
    }
    let subject = any_of_expr(subject_column, entities.clone()).expect("non-empty identities");
    Ok(Some(object_column.map_or(subject.clone(), |object| {
        subject.or(any_of_expr(object, entities).expect("non-empty identities"))
    })))
}

async fn compile_retrieve_facts(
    session: &ServingQuerySession,
    compiler: &OntologyProgramCompiler,
    typed: &TypedQueryBlock,
    query: &SemanticQueryClause,
) -> Result<LogicalPlan, SemanticQueryError> {
    let direct_facts = query
        .direct_fact_ids()
        .into_iter()
        .map(|value| id16_literal(value, "fact:", "fact"))
        .collect::<Result<Vec<_>, _>>()?;

    let mut property_predicates = condition_predicates(compiler, query, "property")?;
    if let Some(predicate) = any_of_expr("property.fact_id", direct_facts.clone()) {
        property_predicates.push(predicate);
    }
    if let Some(predicate) = fact_about_predicates(query, "property.subject_entity_id", None)? {
        property_predicates.push(predicate);
    }
    let property_kinds = property_kind_codes(compiler, &typed.resolved_phrases);
    let relation_kinds = relation_kind_codes(compiler, &typed.resolved_phrases);
    if let Some(predicate) = any_of("property.property_kind_code", property_kinds.clone()) {
        property_predicates.push(predicate);
    } else if !relation_kinds.is_empty() {
        property_predicates.push(lit(false));
    }
    let mut properties =
        LogicalPlanBuilder::from(session.table_plan("properties").await?).alias("property")?;
    if let Some(predicate) = all_of(property_predicates) {
        properties = properties.filter(predicate)?;
    }
    let properties = properties
        .project(vec![
            col("property.fact_id"),
            col("property.owner_id").alias("semantic_owner"),
            col("property.start_byte").alias("source_location"),
            lit("property").alias("fact_class"),
            col("property.property_kind_code").alias("relationship"),
            col("property.subject_entity_id").alias("subject_id"),
            col("property.value_entity_id").alias("object_or_value"),
            col("property.certainty_code"),
            col("property.resolution_code"),
            col("property.file_id"),
            col("property.end_byte").alias("span_end"),
            lit(query.query_id()).alias("origin_query_id"),
        ])?
        .build()?;

    let mut relation_predicates = condition_predicates(compiler, query, "relation")?;
    if let Some(predicate) = any_of_expr("relation.fact_id", direct_facts) {
        relation_predicates.push(predicate);
    }
    if let Some(predicate) =
        fact_about_predicates(query, "relation.source_id", Some("relation.target_id"))?
    {
        relation_predicates.push(predicate);
    }
    if let Some(predicate) = any_of("relation.relation_kind_code", relation_kinds) {
        relation_predicates.push(predicate);
    } else if !property_kinds.is_empty() {
        relation_predicates.push(lit(false));
    }
    let mut relations =
        LogicalPlanBuilder::from(session.table_plan("relations").await?).alias("relation")?;
    if let Some(predicate) = all_of(relation_predicates) {
        relations = relations.filter(predicate)?;
    }
    let relations = relations
        .project(vec![
            col("relation.fact_id"),
            col("relation.owner_id").alias("semantic_owner"),
            col("relation.start_byte").alias("source_location"),
            lit("relation").alias("fact_class"),
            col("relation.relation_kind_code").alias("relationship"),
            col("relation.source_id").alias("subject_id"),
            col("relation.target_id").alias("object_or_value"),
            col("relation.certainty_code"),
            col("relation.resolution_code"),
            col("relation.file_id"),
            col("relation.end_byte").alias("span_end"),
            lit(query.query_id()).alias("origin_query_id"),
        ])?
        .build()?;
    let builder = LogicalPlanBuilder::from(properties).union(relations)?;
    bounded_plan(builder, &typed.canonical_order, typed.limit)
}

fn dependency_identity(role: ResultRole) -> Result<&'static str, SemanticQueryError> {
    match role {
        ResultRole::Entities => Ok("entity_id"),
        ResultRole::Facts => Ok("fact_id"),
        ResultRole::Paths => Ok("path_id"),
        ResultRole::PatternBindings => Ok("binding_id"),
        ResultRole::Groups => Ok("group_key"),
        ResultRole::SourceContexts => Ok("source_file_id"),
        ResultRole::Summary => Err(phase_error(
            "QUERY_ROLE_INCOMPATIBLE",
            "type_check",
            "",
            "summary output has no set identity domain",
        )),
    }
}

fn dependency_schema(
    compiler: &OntologyProgramCompiler,
    role: ResultRole,
) -> Result<SchemaRef, SemanticQueryError> {
    let result_schema_id = match role {
        ResultRole::Entities => "result.find_entities.v2",
        ResultRole::Facts => "result.retrieve_facts.v2",
        ResultRole::Paths => "result.find_paths.v2",
        ResultRole::PatternBindings => "result.match_pattern.v2",
        ResultRole::Groups => "result.combine_results.v2",
        ResultRole::SourceContexts => "result.retrieve_source_context.v2",
        ResultRole::Summary => {
            return Err(phase_error(
                "QUERY_ROLE_INCOMPATIBLE",
                "logical_compile",
                "",
                "summary output has no dependency identity schema",
            ));
        }
    };
    compiler
        .project_result_schema(
            result_schema_id,
            &[
                dependency_identity(role)?,
                "origin_query_id",
                "certainty_code",
            ],
        )
        .ok_or_else(|| {
            phase_error(
                "QUERY_RESULT_SCHEMA_MISSING",
                "logical_compile",
                "",
                format!("compiled dependency projection {result_schema_id} is absent"),
            )
        })
}

fn empty_dependency_plan(
    compiler: &OntologyProgramCompiler,
    name: &str,
    role: ResultRole,
) -> Result<LogicalPlan, SemanticQueryError> {
    let schema = dependency_schema(compiler, role)?;
    let empty = RecordBatch::new_empty(Arc::clone(&schema));
    let provider = Arc::new(MemTable::try_new(schema, vec![vec![empty]])?);
    Ok(LogicalPlanBuilder::scan(
        format!("cpg_serving.{name}"),
        provider_as_source(provider),
        None,
    )?
    .build()?)
}

fn parse_set_operation(value: &str, pointer: &str) -> Result<SetOperation, SemanticQueryError> {
    match value {
        "union by entity identity" | "union by fact identity" | "union by result identity" => {
            Ok(SetOperation::Union)
        }
        "intersection by entity identity"
        | "intersection by fact identity"
        | "intersection by result identity" => Ok(SetOperation::Intersection),
        "difference by entity identity"
        | "difference by fact identity"
        | "difference by result identity" => Ok(SetOperation::Difference),
        "join by entity identity" | "join by fact identity" | "join by result identity" => {
            Ok(SetOperation::InnerJoin)
        }
        _ => Err(phase_error(
            "QUERY_SET_OPERATION_UNSUPPORTED",
            "semantic_binding",
            pointer,
            "set operation is not governed",
        )),
    }
}

fn combine_plans(
    plans: Vec<LogicalPlan>,
    operation: SetOperation,
    identity: &'static str,
) -> Result<LogicalPlan, SemanticQueryError> {
    let mut plans = plans.into_iter();
    let first = plans.next().ok_or_else(|| {
        phase_error(
            "QUERY_DEPENDENCY_REQUIRED",
            "logical_compile",
            "",
            "set operation has no input",
        )
    })?;
    plans.try_fold(first, |left, right| {
        Ok(match operation {
            SetOperation::Union => LogicalPlanBuilder::from(left)
                .union_distinct(right)?
                .build()?,
            SetOperation::Intersection => LogicalPlanBuilder::intersect(left, right, false)?,
            SetOperation::Difference => LogicalPlanBuilder::except(left, right, false)?,
            SetOperation::InnerJoin => LogicalPlanBuilder::from(left)
                .join(
                    right,
                    JoinType::Inner,
                    (vec![identity], vec![identity]),
                    None,
                )?
                .project(vec![
                    col(identity),
                    col("origin_query_id"),
                    col("certainty_code"),
                ])?
                .build()?,
        })
    })
}

fn compile_combine_template(
    compiler: &OntologyProgramCompiler,
    typed: &TypedQueryBlock,
    query: &SemanticQueryClause,
) -> Result<(LogicalPlan, RelationalRuntime), SemanticQueryError> {
    let SemanticQueryClause::CombineResults {
        inputs,
        combination,
        identity,
        ..
    } = query
    else {
        return Err(phase_error(
            "QUERY_FORM_COMPILER_MISMATCH",
            "logical_compile",
            &typed.source_pointer,
            "combine compiler received another form",
        ));
    };
    if inputs.len() < 2 {
        return Err(phase_error(
            "QUERY_DEPENDENCY_REQUIRED",
            "type_check",
            format!("{}/inputs", typed.source_pointer),
            "combine requires at least two inputs",
        ));
    }
    let identity_role = inputs[0].select;
    if inputs.iter().any(|input| input.select != identity_role) {
        return Err(phase_error(
            "QUERY_IDENTITY_DOMAIN_MISMATCH",
            "type_check",
            format!("{}/inputs", typed.source_pointer),
            "combine inputs do not share one identity domain",
        ));
    }
    if identity.as_deref().is_some_and(|declared| {
        !matches!(
            (identity_role, declared),
            (ResultRole::Entities, "entity identity")
                | (ResultRole::Facts, "fact identity")
                | (_, "result identity")
        )
    }) {
        return Err(phase_error(
            "QUERY_IDENTITY_DOMAIN_MISMATCH",
            "semantic_binding",
            format!("{}/identity", typed.source_pointer),
            "declared identity is incompatible with the typed input role",
        ));
    }
    let operation = parse_set_operation(
        combination,
        &format!("{}/combination", typed.source_pointer),
    )?;
    let identity_column = dependency_identity(identity_role)?;
    let plans = (0..inputs.len())
        .map(|index| {
            empty_dependency_plan(compiler, &format!("query_input_{index}"), identity_role)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let combined = combine_plans(plans, operation, identity_column)?;
    let builder = LogicalPlanBuilder::from(combined).project(vec![
        col(identity_column).alias("group_key"),
        col("origin_query_id"),
        col("certainty_code"),
    ])?;
    Ok((
        bounded_plan(builder, &typed.canonical_order, typed.limit)?,
        RelationalRuntime::Combine {
            operation,
            identity_role,
        },
    ))
}

fn compile_summary_template(
    compiler: &OntologyProgramCompiler,
    typed: &TypedQueryBlock,
    query: &SemanticQueryClause,
) -> Result<(LogicalPlan, RelationalRuntime), SemanticQueryError> {
    let SemanticQueryClause::SummarizeFacts {
        summaries,
        group_by,
        ..
    } = query
    else {
        return Err(phase_error(
            "QUERY_FORM_COMPILER_MISMATCH",
            "logical_compile",
            &typed.source_pointer,
            "summary compiler received another form",
        ));
    };
    let role = typed
        .input_roles
        .first()
        .copied()
        .unwrap_or(ResultRole::Facts);
    let identity = dependency_identity(role)?;
    let input = empty_dependency_plan(compiler, "query_input_summary", role)?;
    let summary_name = summaries.first().ok_or_else(|| {
        phase_error(
            "QUERY_SUMMARY_REQUIRED",
            "semantic_binding",
            format!("{}/summaries", typed.source_pointer),
            "at least one objective summary is required",
        )
    })?;
    let builder = LogicalPlanBuilder::from(input)
        .aggregate(
            Vec::<Expr>::new(),
            vec![count(col(identity)).alias("summary_value")],
        )?
        .project(vec![
            lit("all").alias("group_key"),
            lit(summary_name.clone()).alias("summary_name"),
            col("summary_value"),
            lit(query.query_id()).alias("origin_query_id"),
        ])?;
    Ok((
        bounded_plan(builder, &typed.canonical_order, typed.limit)?,
        RelationalRuntime::Summarize {
            summary_names: summaries.clone(),
            group_by: group_by.clone(),
        },
    ))
}

fn compile_source_context_template(
    compiler: &OntologyProgramCompiler,
    typed: &TypedQueryBlock,
    query: &SemanticQueryClause,
) -> Result<(LogicalPlan, RelationalRuntime), SemanticQueryError> {
    let SemanticQueryClause::RetrieveSourceContext {
        context,
        text_handling,
        ..
    } = query
    else {
        return Err(phase_error(
            "QUERY_FORM_COMPILER_MISMATCH",
            "logical_compile",
            &typed.source_pointer,
            "source-context compiler received another form",
        ));
    };
    for (index, field) in context.iter().enumerate() {
        if !matches!(
            field.as_str(),
            "source location" | "source file" | "exact span" | "enclosing syntax"
        ) {
            return Err(phase_error(
                "QUERY_CONTEXT_FIELD_UNSUPPORTED",
                "semantic_binding",
                format!("{}/context/{index}", typed.source_pointer),
                "source-context field is not governed",
            ));
        }
    }
    let handling = match text_handling.as_deref() {
        None | Some("omit text") => SourceTextHandling::Omit,
        Some("exact bytes") => SourceTextHandling::ExactBytes,
        Some("decoded text when available") => SourceTextHandling::DecodedText,
        Some(_) => {
            return Err(phase_error(
                "QUERY_TEXT_HANDLING_UNSUPPORTED",
                "semantic_binding",
                format!("{}/text_handling", typed.source_pointer),
                "text handling is not governed",
            ));
        }
    };
    let schema = compiler
        .result_schema("result.retrieve_source_context.v2")
        .ok_or_else(|| {
            phase_error(
                "QUERY_RESULT_SCHEMA_MISSING",
                "logical_compile",
                &typed.source_pointer,
                "compiled source-context result schema is absent",
            )
        })?;
    let empty = RecordBatch::new_empty(Arc::clone(&schema));
    let provider = Arc::new(MemTable::try_new(schema, vec![vec![empty]])?);
    let plan = bounded_plan(
        LogicalPlanBuilder::scan(
            "cpg_serving.query_input_source_context",
            provider_as_source(provider),
            None,
        )?,
        &typed.canonical_order,
        typed.limit,
    )?;
    Ok((
        plan,
        RelationalRuntime::SourceContext {
            context_fields: context.clone(),
            text_handling: handling,
        },
    ))
}

#[allow(clippy::too_many_lines)] // All relational forms share one policy-enforced DataFusion lowering fence.
async fn lower_relational_block(
    session: &ServingQuerySession,
    compiler: &OntologyProgramCompiler,
    typed: &TypedQueryBlock,
    query: &SemanticQueryClause,
) -> Result<BoundQueryBlock, SemanticQueryError> {
    let (plan, source_tables, identity_column, runtime) = match typed.form {
        QueryForm::FindEntities => (
            compile_find_entities(session, compiler, typed, query).await?,
            vec!["entities", "files"],
            "entity_id",
            RelationalRuntime::Snapshot,
        ),
        QueryForm::RetrieveFacts => (
            compile_retrieve_facts(session, compiler, typed, query).await?,
            vec!["properties", "relations"],
            "fact_id",
            RelationalRuntime::Snapshot,
        ),
        QueryForm::CombineResults => {
            let (plan, runtime) = compile_combine_template(compiler, typed, query)?;
            (plan, Vec::new(), "group_key", runtime)
        }
        QueryForm::SummarizeFacts => {
            let (plan, runtime) = compile_summary_template(compiler, typed, query)?;
            (plan, Vec::new(), "group_key", runtime)
        }
        QueryForm::RetrieveSourceContext => {
            let (plan, runtime) = compile_source_context_template(compiler, typed, query)?;
            (
                plan,
                vec!["entities", "relations", "properties", "files"],
                "source_file_id",
                runtime,
            )
        }
        QueryForm::FollowRelationships | QueryForm::FindPaths | QueryForm::MatchPattern => {
            return Err(phase_error(
                "QUERY_FORM_COMPILER_MISMATCH",
                "logical_compile",
                &typed.source_pointer,
                "graph form reached relational lowering",
            ));
        }
    };
    session.validate_query_plan(&plan).map_err(|error| {
        phase_error(
            "QUERY_PLAN_POLICY_REJECTED",
            "structural_policy",
            &typed.source_pointer,
            error.to_string(),
        )
    })?;
    let output_schema = Arc::new(plan.schema().as_arrow().clone());
    Ok(BoundQueryBlock {
        typed: typed.clone(),
        operator: BoundOperator::Relational(Box::new(RelationalOperatorPlan {
            form: typed.form,
            source_tables,
            identity_column,
            output_schema,
            canonical_order: typed.canonical_order.clone(),
            template_plan: plan,
            runtime,
        })),
    })
}

fn graph_direction(value: Option<&str>) -> GraphDirection {
    let value = value.unwrap_or("outgoing").to_ascii_lowercase();
    if value.contains("callee to caller")
        || value.contains("incoming")
        || value.contains("predecessor")
        || value.contains("target to source")
    {
        GraphDirection::Incoming
    } else if value.contains("both") || value.contains("either direction") {
        GraphDirection::Both
    } else {
        GraphDirection::Outgoing
    }
}

fn graph_distance(value: Option<&str>) -> GraphDistance {
    if value
        .unwrap_or("one relationship step")
        .to_ascii_lowercase()
        .contains("transitiv")
    {
        GraphDistance::Transitive
    } else {
        GraphDistance::Direct
    }
}

fn path_policy(value: &str, pointer: &str) -> Result<PathPolicy, SemanticQueryError> {
    let normalized = value.to_ascii_lowercase();
    if normalized.contains("all simple") {
        Ok(PathPolicy::AllSimple)
    } else if normalized.contains("all shortest") {
        Ok(PathPolicy::AllShortest)
    } else if normalized.contains("one shortest")
        || normalized.contains("one deterministic shortest")
    {
        Ok(PathPolicy::DeterministicShortest)
    } else {
        Err(phase_error(
            "QUERY_PATH_POLICY_UNSUPPORTED",
            "semantic_resolution",
            pointer,
            format!("unsupported bounded path policy {value:?}"),
        ))
    }
}

fn resolved_relation_kind_codes(
    compiler: &OntologyProgramCompiler,
    phrases: &[ResolvedPhrase],
) -> Vec<i32> {
    let mut codes = relation_kind_codes(compiler, phrases)
        .into_iter()
        .filter_map(|value| match value {
            ScalarValue::Int32(Some(code)) => Some(code),
            _ => None,
        })
        .collect::<Vec<_>>();
    codes.sort_unstable();
    codes.dedup();
    codes
}

fn graph_certainty_codes(
    compiler: &OntologyProgramCompiler,
    query: &SemanticQueryClause,
) -> Result<Vec<i16>, SemanticQueryError> {
    let mut codes = BTreeSet::new();
    for condition in query_where_conditions(query) {
        let operation = compiler
            .phrases
            .values()
            .find(|operation| operation.canonical_text == *condition)
            .ok_or_else(|| {
                phase_error(
                    "SEMANTIC_PHRASE_UNSUPPORTED",
                    "logical_compile",
                    "",
                    format!("unbound semantic condition {condition:?}"),
                )
            })?;
        codes.extend(operation.operand_codes.iter().copied());
    }
    Ok(codes.into_iter().collect())
}

fn graph_output_schema(
    compiler: &OntologyProgramCompiler,
    form: QueryForm,
) -> Result<SchemaRef, SemanticQueryError> {
    let result_schema_id = match form {
        QueryForm::FollowRelationships => "result.follow_relationships.v2",
        QueryForm::FindPaths => "result.find_paths.v2",
        QueryForm::MatchPattern => "result.match_pattern.v2",
        _ => {
            return Err(phase_error(
                "QUERY_FORM_COMPILER_MISMATCH",
                "logical_compile",
                "",
                "relational form reached graph output schema",
            ));
        }
    };
    compiler.result_schema(result_schema_id).ok_or_else(|| {
        phase_error(
            "QUERY_RESULT_SCHEMA_MISSING",
            "logical_compile",
            "",
            format!("compiled graph result schema {result_schema_id} is absent"),
        )
    })
}

#[allow(clippy::too_many_lines)] // The semantic IR construction exhaustively covers three governed graph variants.
fn graph_operator_plan(
    compiler: &OntologyProgramCompiler,
    typed: &TypedQueryBlock,
    query: &SemanticQueryClause,
) -> Result<GraphOperatorPlan, SemanticQueryError> {
    if matches!(
        typed.form,
        QueryForm::FindEntities
            | QueryForm::RetrieveFacts
            | QueryForm::CombineResults
            | QueryForm::SummarizeFacts
            | QueryForm::RetrieveSourceContext
    ) {
        return Err(SemanticQueryError::Invalid(
            "relational form reached graph lowering".to_owned(),
        ));
    }
    let relationship_kind_codes = resolved_relation_kind_codes(compiler, &typed.resolved_phrases);
    let (semantics, maximum_depth) = match query {
        SemanticQueryClause::FollowRelationships {
            direction,
            distance,
            stop_when,
            ..
        } => (
            GraphSemantics::Follow {
                direction: graph_direction(direction.as_deref()),
                distance: graph_distance(distance.as_deref()),
                stop_when: stop_when.clone(),
            },
            if graph_distance(distance.as_deref()) == GraphDistance::Direct {
                1
            } else {
                64
            },
        ),
        SemanticQueryClause::FindPaths {
            direction,
            through,
            path_policy: requested_policy,
            maximum_length,
            ..
        } => {
            let selected_policy = path_policy(
                requested_policy,
                &format!("{}/path_policy", typed.source_pointer),
            )?;
            if selected_policy == PathPolicy::AllSimple && maximum_length.is_none() {
                return Err(phase_error(
                    "QUERY_PATH_UNBOUNDED",
                    "resource_contract",
                    format!("{}/maximum_length", typed.source_pointer),
                    "all-simple path enumeration requires an explicit maximum_length",
                ));
            }
            (
                GraphSemantics::Paths {
                    direction: graph_direction(direction.as_deref()),
                    path_policy: selected_policy,
                    through_codes: resolved_relation_kind_codes(
                        compiler,
                        &typed
                            .resolved_phrases
                            .iter()
                            .filter(|phrase| {
                                through.iter().any(|value| {
                                    value == &phrase.canonical_text
                                        || value == &phrase.contract_code
                                })
                            })
                            .cloned()
                            .collect::<Vec<_>>(),
                    ),
                },
                maximum_length.unwrap_or(64),
            )
        }
        SemanticQueryClause::MatchPattern {
            bindings,
            relationships,
            ..
        } => (
            GraphSemantics::Pattern {
                bindings: bindings.clone(),
                relationships: relationships.clone(),
            },
            64,
        ),
        _ => {
            return Err(phase_error(
                "QUERY_FORM_COMPILER_MISMATCH",
                "logical_compile",
                &typed.source_pointer,
                "query variant does not match graph form",
            ));
        }
    };
    if maximum_depth == 0 {
        return Err(phase_error(
            "QUERY_GRAPH_BOUND_INVALID",
            "resource_contract",
            &typed.source_pointer,
            "graph maximum depth must be positive",
        ));
    }
    Ok(GraphOperatorPlan {
        form: typed.form,
        block_id: typed.block_id.clone(),
        semantics,
        relationship_kind_codes,
        certainty_codes: graph_certainty_codes(compiler, query)?,
        input_roles: typed.input_roles.clone(),
        output_role: typed.output_role,
        output_schema: graph_output_schema(compiler, typed.form)?,
        canonical_order: typed.canonical_order.clone(),
        maximum_results: typed.limit.first,
        maximum_depth,
        maximum_memory_bytes: typed.maximum_memory_bytes,
        cancellation_required: typed.cancellation_required,
    })
}

fn semantic_identity(
    domain: crate::identity::SemanticFingerprintDomain,
    canonical: &[u8],
) -> String {
    let mut fingerprint = crate::identity::semantic_fingerprint(domain);
    fingerprint.update(&(canonical.len() as u64).to_be_bytes());
    fingerprint.update(canonical);
    crate::integrity::frame_digest(fingerprint.finalize())
}

fn semantic_plan_template(
    blocks: &[BoundQueryBlock],
    execution_order: &[String],
) -> Result<(Vec<u8>, String), SemanticQueryError> {
    let mut serialized_blocks = Vec::with_capacity(blocks.len());
    for block in blocks {
        let operator = match &block.operator {
            BoundOperator::Relational(plan) => serde_json::json!({
                "family": "datafusion_relational",
                "provider": {
                    "catalog": "codefabric",
                    "schema": "cpg_serving",
                    "tables": plan.source_tables,
                    "snapshot_bound": true,
                },
                "identity_column": plan.identity_column,
                "output_schema": serde_json::to_value(plan.output_schema.as_ref())
                    .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
                "logical_plan": crate::fabric::logical_plan_template_serialization(&plan.template_plan)?,
            }),
            BoundOperator::Graph(plan) => serde_json::json!({
                "family": "application_graph",
                "node": plan.form.plan_node_kind(),
                "semantics": plan.semantics,
                "relationship_kind_codes": plan.relationship_kind_codes,
                "certainty_codes": plan.certainty_codes,
                "input_roles": plan.input_roles,
                "output_role": plan.output_role,
                "output_schema": serde_json::to_value(plan.output_schema.as_ref())
                    .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
                "canonical_order": plan.canonical_order,
                "maximum_results": "parameter.first",
                "maximum_depth": plan.maximum_depth,
                "maximum_memory_bytes": "(parameter.first + 1) * 1024",
                "cancellation_required": plan.cancellation_required,
            }),
        };
        serialized_blocks.push(serde_json::json!({
            "block_id": block.typed.block_id,
            "source_pointer": block.typed.source_pointer,
            "form": block.typed.form,
            "input_roles": block.typed.input_roles,
            "output_role": block.typed.output_role,
            "dependencies": block.typed.dependencies,
            "fan_in": block.typed.fan_in,
            "fan_out": block.typed.fan_out,
            "coverage_prerequisites": block.typed.coverage_prerequisites,
            "coverage_effects": block.typed.coverage_effects,
            "canonical_order": block.typed.canonical_order,
            "parameter_slots": {
                "label": {"type": "semantic_phrase", "nullable": true},
                "input_entity_ids": {"type": "Id16[]"},
                "input_fact_ids": {"type": "Id16[]"},
                "entity_kind_codes": {"type": "Code16[]"},
                "relation_kind_codes": {"type": "Code16[]"},
                "first": {"type": "usize"},
                "offset": {"type": "usize"},
            },
            "resource_contract": {
                "maximum_memory_bytes": "(parameter.first + 1) * 1024",
                "cancellation_required": block.typed.cancellation_required,
            },
            "operator": operator,
        }));
    }
    let template = serde_json::json!({
        "version": "QueryPlanTemplateV1",
        "family": "typed_semantic_dag",
        "datafusion_version": datafusion::DATAFUSION_VERSION,
        "arrow_version": arrow::ARROW_VERSION,
        "blocks": serialized_blocks,
        "execution_order": execution_order,
    });
    let canonical = canonicalize_value(&template)?;
    let identity = semantic_identity(
        crate::identity::SemanticFingerprintDomain::QueryPlanTemplateV1,
        &canonical,
    );
    Ok((canonical, identity))
}

fn bound_semantic_query_identity(
    typed: &TypedSemanticRequest,
    plan_template_id: &str,
    snapshot_manifest_digest: &str,
    execution_config_digest: &str,
) -> Result<String, SemanticQueryError> {
    let bound = serde_json::json!({
        "version": "BoundSemanticQueryV1",
        "plan_template_id": plan_template_id,
        "queries": typed.request.queries,
        "snapshot_manifest_digest": snapshot_manifest_digest,
        "execution_config_digest": execution_config_digest,
    });
    let canonical = canonicalize_value(&bound)?;
    Ok(semantic_identity(
        crate::identity::SemanticFingerprintDomain::BoundSemanticQueryV1,
        &canonical,
    ))
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
    let compiler = session.ontology_compiler();
    let mut blocks = Vec::with_capacity(typed.blocks.len());
    for block in &typed.blocks {
        let query = typed
            .request
            .queries
            .iter()
            .find(|query| query.query_id() == block.block_id)
            .ok_or_else(|| {
                SemanticQueryError::Invalid(
                    "typed block does not retain its parsed query".to_owned(),
                )
            })?;
        let mut bound_block = block.clone();
        bound_block.resolved_phrases =
            resolve_query_phrases(&compiler, query, &bound_block.source_pointer)?;
        if matches!(
            bound_block.form,
            QueryForm::FindEntities
                | QueryForm::RetrieveFacts
                | QueryForm::CombineResults
                | QueryForm::SummarizeFacts
                | QueryForm::RetrieveSourceContext
        ) {
            blocks.push(lower_relational_block(session, &compiler, &bound_block, query).await?);
        } else {
            blocks.push(BoundQueryBlock {
                operator: BoundOperator::Graph(graph_operator_plan(
                    &compiler,
                    &bound_block,
                    query,
                )?),
                typed: bound_block,
            });
        }
    }
    let (_, plan_template_id) = semantic_plan_template(&blocks, &typed.execution_order)?;
    let manifest = session.snapshot_manifest();
    let bound_query_id = bound_semantic_query_identity(
        typed,
        &plan_template_id,
        &manifest.manifest_digest,
        &session.execution_config_digest()?,
    )?;
    Ok(BoundPlanSpec {
        snapshot_id: manifest.snapshot_id,
        plan_template_id,
        bound_query_id,
        request_digest: typed.request_digest.clone(),
        blocks,
        execution_order: typed.execution_order.clone(),
    })
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[allow(clippy::struct_field_names)] // The suffix distinguishes the three governed identity domains.
struct GraphEdge {
    fact_id: [u8; 16],
    source_id: [u8; 16],
    target_id: [u8; 16],
    relation_family_code: i16,
    relation_kind_code: i32,
    distance: Option<i32>,
    directness_code: i16,
    certainty_code: i16,
    resolution_code: i16,
}

#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_field_names)] // Each collection is named for its governed result identity domain.
struct ResponseValues {
    entity_ids: Vec<String>,
    fact_ids: Vec<String>,
    path_ids: Vec<String>,
    group_ids: Vec<String>,
    source_context_ids: Vec<String>,
}

#[derive(Clone, Debug)]
struct CompletedBlock {
    role: ResultRole,
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
    values: ResponseValues,
    completeness: CompletenessState,
}

fn completed_scan(
    name: &str,
    completed: &CompletedBlock,
) -> Result<LogicalPlan, SemanticQueryError> {
    let batches = if completed.batches.is_empty() {
        vec![RecordBatch::new_empty(Arc::clone(&completed.schema))]
    } else {
        completed.batches.clone()
    };
    for batch in &batches {
        if batch.schema() != completed.schema {
            return Err(phase_error(
                "QUERY_DEPENDENCY_SCHEMA_MISMATCH",
                "response_verification",
                name,
                "dependency batch schema differs from its typed Arrow contract",
            ));
        }
    }
    let provider = Arc::new(MemTable::try_new(
        Arc::clone(&completed.schema),
        vec![batches],
    )?);
    Ok(LogicalPlanBuilder::scan(
        format!("cpg_serving.{name}"),
        provider_as_source(provider),
        None,
    )?
    .build()?)
}

fn dependency_identity_in_schema(completed: &CompletedBlock) -> Result<&str, SemanticQueryError> {
    let preferred = dependency_identity(completed.role)?;
    if completed.schema.field_with_name(preferred).is_ok() {
        return Ok(preferred);
    }
    [
        "group_id",
        "source_context_id",
        "binding_id",
        "path_id",
        "fact_id",
        "entity_id",
    ]
    .into_iter()
    .find(|name| completed.schema.field_with_name(name).is_ok())
    .ok_or_else(|| {
        phase_error(
            "QUERY_DEPENDENCY_SCHEMA_MISMATCH",
            "response_verification",
            "",
            "dependency Arrow schema has no identity column for its role",
        )
    })
}

fn normalized_dependency_plan(
    compiler: &OntologyProgramCompiler,
    dependency_id: &str,
    completed: &CompletedBlock,
) -> Result<LogicalPlan, SemanticQueryError> {
    let identity = dependency_identity_in_schema(completed)?;
    let input = completed_scan(&format!("query_input_{dependency_id}"), completed)?;
    let origin = if completed.schema.field_with_name("origin_query_id").is_ok() {
        col("origin_query_id")
    } else {
        lit(dependency_id)
    };
    let certainty = if completed.schema.field_with_name("certainty_code").is_ok() {
        col("certainty_code")
    } else {
        lit(ScalarValue::Int16(Some(compiled_enum_code(
            compiler,
            "EVIDENCE_CERTAINTY",
            "UNRESOLVED",
        )?)))
    };
    Ok(LogicalPlanBuilder::from(input)
        .project(vec![
            col(identity).alias(dependency_identity(completed.role)?),
            origin.alias("origin_query_id"),
            certainty.alias("certainty_code"),
        ])?
        .build()?)
}

fn compile_runtime_combine(
    compiler: &OntologyProgramCompiler,
    block: &BoundQueryBlock,
    operation: SetOperation,
    identity_role: ResultRole,
    completed: &BTreeMap<String, CompletedBlock>,
) -> Result<LogicalPlan, SemanticQueryError> {
    let plans = block
        .typed
        .dependencies
        .iter()
        .map(|dependency| {
            let result = completed.get(dependency).ok_or_else(|| {
                phase_error(
                    "QUERY_DEPENDENCY_NOT_READY",
                    "execution",
                    &block.typed.source_pointer,
                    format!("dependency {dependency} has no completed Arrow result"),
                )
            })?;
            if result.role != identity_role {
                return Err(phase_error(
                    "QUERY_IDENTITY_DOMAIN_MISMATCH",
                    "execution",
                    &block.typed.source_pointer,
                    "runtime dependency role differs from the bound identity domain",
                ));
            }
            normalized_dependency_plan(compiler, dependency, result)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let identity = dependency_identity(identity_role)?;
    let combined = combine_plans(plans, operation, identity)?;
    bounded_plan(
        LogicalPlanBuilder::from(combined).project(vec![
            col(identity).alias("group_key"),
            col("origin_query_id"),
            col("certainty_code"),
        ])?,
        &block.typed.canonical_order,
        block.typed.limit,
    )
}

fn summary_group_expressions(group_by: &[String]) -> Result<Vec<Expr>, SemanticQueryError> {
    group_by
        .iter()
        .enumerate()
        .map(|(index, group)| match group.as_str() {
            "origin query" => Ok(col("origin_query_id")),
            "certainty" => Ok(col("certainty_code")),
            _ => Err(phase_error(
                "QUERY_SUMMARY_GROUP_UNSUPPORTED",
                "semantic_binding",
                format!("/group_by/{index}"),
                "summary grouping is not a governed objective dimension",
            )),
        })
        .collect()
}

fn compile_runtime_summary(
    compiler: &OntologyProgramCompiler,
    block: &BoundQueryBlock,
    query: &SemanticQueryClause,
    summary_names: &[String],
    group_by: &[String],
    completed: &BTreeMap<String, CompletedBlock>,
) -> Result<LogicalPlan, SemanticQueryError> {
    let mut inputs = block
        .typed
        .dependencies
        .iter()
        .map(|dependency| {
            completed
                .get(dependency)
                .ok_or_else(|| {
                    phase_error(
                        "QUERY_DEPENDENCY_NOT_READY",
                        "execution",
                        &block.typed.source_pointer,
                        format!("dependency {dependency} has no completed Arrow result"),
                    )
                })
                .and_then(|result| normalized_dependency_plan(compiler, dependency, result))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter();
    let first = inputs.next().ok_or_else(|| {
        phase_error(
            "QUERY_DEPENDENCY_REQUIRED",
            "execution",
            &block.typed.source_pointer,
            "summary has no completed input",
        )
    })?;
    let input = inputs.try_fold(first, |left, right| {
        Ok::<_, SemanticQueryError>(LogicalPlanBuilder::from(left).union(right)?.build()?)
    })?;
    let role = block
        .typed
        .input_roles
        .first()
        .copied()
        .unwrap_or(ResultRole::Facts);
    let identity = dependency_identity(role)?;
    let group_expr = summary_group_expressions(group_by)?;
    let mut summary_plans = Vec::new();
    for summary_name in summary_names {
        let aggregated = LogicalPlanBuilder::from(input.clone())
            .aggregate(
                group_expr.clone(),
                vec![count(col(identity)).alias("summary_value")],
            )?
            .build()?;
        let group_key = if group_by.is_empty() {
            lit("all")
        } else {
            // The generated public contract treats the group key as presentation data. The
            // exact typed grouping columns remain beside it in the Arrow schema.
            col(group_by[0].replace(' ', "_"))
        };
        let mut projection = vec![
            group_key.alias("group_key"),
            lit(summary_name.clone()).alias("summary_name"),
            col("summary_value"),
            lit(query.query_id()).alias("origin_query_id"),
        ];
        projection.extend(group_expr.clone());
        summary_plans.push(
            LogicalPlanBuilder::from(aggregated)
                .project(projection)?
                .build()?,
        );
    }
    let mut summaries = summary_plans.into_iter();
    let first = summaries.next().ok_or_else(|| {
        phase_error(
            "QUERY_SUMMARY_REQUIRED",
            "execution",
            &block.typed.source_pointer,
            "summary compiler has no objective summary",
        )
    })?;
    let plan = summaries.try_fold(first, |left, right| {
        Ok::<_, SemanticQueryError>(LogicalPlanBuilder::from(left).union(right)?.build()?)
    })?;
    bounded_plan(
        LogicalPlanBuilder::from(plan),
        &block.typed.canonical_order,
        block.typed.limit,
    )
}

async fn source_locator_plan(
    session: &ServingQuerySession,
    dependency_id: &str,
    completed: &CompletedBlock,
) -> Result<LogicalPlan, SemanticQueryError> {
    let input = completed_scan(&format!("query_context_{dependency_id}"), completed)?;
    let identity = dependency_identity_in_schema(completed)?;
    let input = LogicalPlanBuilder::from(input)
        .project(vec![
            col(identity).alias(identity),
            if completed.schema.field_with_name("origin_query_id").is_ok() {
                col("origin_query_id")
            } else {
                lit(dependency_id).alias("origin_query_id")
            },
        ])?
        .alias("query_input")?;
    match completed.role {
        ResultRole::Entities => {
            let entities = LogicalPlanBuilder::from(session.table_plan("entities").await?)
                .alias("entity")?
                .build()?;
            Ok(input
                .join_on(
                    entities,
                    JoinType::Inner,
                    [col("query_input.entity_id").eq(col("entity.entity_id"))],
                )?
                .project(vec![
                    col("entity.file_id"),
                    col("entity.start_byte").alias("span_start"),
                    col("entity.end_byte").alias("span_end"),
                    col("query_input.origin_query_id"),
                ])?
                .build()?)
        }
        ResultRole::Facts => {
            let relations = LogicalPlanBuilder::from(session.table_plan("relations").await?)
                .alias("relation")?
                .build()?;
            let relation_locations = input
                .clone()
                .join_on(
                    relations,
                    JoinType::Inner,
                    [col("query_input.fact_id").eq(col("relation.fact_id"))],
                )?
                .project(vec![
                    col("relation.file_id"),
                    col("relation.start_byte").alias("span_start"),
                    col("relation.end_byte").alias("span_end"),
                    col("query_input.origin_query_id"),
                ])?
                .build()?;
            let properties = LogicalPlanBuilder::from(session.table_plan("properties").await?)
                .alias("property")?
                .build()?;
            let property_locations = input
                .join_on(
                    properties,
                    JoinType::Inner,
                    [col("query_input.fact_id").eq(col("property.fact_id"))],
                )?
                .project(vec![
                    col("property.file_id"),
                    col("property.start_byte").alias("span_start"),
                    col("property.end_byte").alias("span_end"),
                    col("query_input.origin_query_id"),
                ])?
                .build()?;
            Ok(LogicalPlanBuilder::from(relation_locations)
                .union(property_locations)?
                .build()?)
        }
        ResultRole::Paths | ResultRole::PatternBindings
            if completed.schema.field_with_name("fact_id").is_ok() =>
        {
            let facts = CompletedBlock {
                role: ResultRole::Facts,
                schema: Arc::clone(&completed.schema),
                batches: completed.batches.clone(),
                values: completed.values.clone(),
                completeness: completed.completeness,
            };
            Box::pin(source_locator_plan(session, dependency_id, &facts)).await
        }
        ResultRole::Paths
        | ResultRole::PatternBindings
        | ResultRole::Groups
        | ResultRole::Summary
        | ResultRole::SourceContexts => Err(phase_error(
            "QUERY_CONTEXT_INPUT_UNLOCATABLE",
            "semantic_binding",
            dependency_id,
            "input role has no source-locatable entity or fact witness",
        )),
    }
}

async fn compile_runtime_source_context(
    session: &ServingQuerySession,
    block: &BoundQueryBlock,
    handling: SourceTextHandling,
    completed: &BTreeMap<String, CompletedBlock>,
) -> Result<LogicalPlan, SemanticQueryError> {
    let mut locators = Vec::new();
    for dependency in &block.typed.dependencies {
        let result = completed.get(dependency).ok_or_else(|| {
            phase_error(
                "QUERY_DEPENDENCY_NOT_READY",
                "execution",
                &block.typed.source_pointer,
                format!("dependency {dependency} has no completed Arrow result"),
            )
        })?;
        locators.push(source_locator_plan(session, dependency, result).await?);
    }
    let mut locators = locators.into_iter();
    let first = locators.next().ok_or_else(|| {
        phase_error(
            "QUERY_CONTEXT_INPUT_REQUIRED",
            "execution",
            &block.typed.source_pointer,
            "source-context request has no resolved input",
        )
    })?;
    let locator = locators.try_fold(first, |left, right| {
        Ok::<_, SemanticQueryError>(LogicalPlanBuilder::from(left).union(right)?.build()?)
    })?;
    let files = LogicalPlanBuilder::from(session.table_plan("files").await?)
        .alias("source_file")?
        .build()?;
    let source_bytes = match handling {
        SourceTextHandling::ExactBytes => col("source_file.source_bytes"),
        SourceTextHandling::Omit | SourceTextHandling::DecodedText => {
            lit(ScalarValue::Binary(None))
        }
    };
    let decoded_text = match handling {
        SourceTextHandling::DecodedText => col("source_file.decoded_text"),
        SourceTextHandling::Omit | SourceTextHandling::ExactBytes => lit(ScalarValue::Utf8(None)),
    };
    let builder = LogicalPlanBuilder::from(locator)
        .alias("locator")?
        .join_on(
            files,
            JoinType::Inner,
            [col("locator.file_id").eq(col("source_file.file_id"))],
        )?
        .filter(
            col("locator.span_start")
                .gt_eq(lit(0_i64))
                .and(col("locator.span_end").gt_eq(col("locator.span_start")))
                .and(col("locator.span_end").lt_eq(col("source_file.byte_len"))),
        )?
        .project(vec![
            col("source_file.file_id").alias("source_file_id"),
            col("source_file.path_display").alias("source_file_path"),
            col("locator.span_start"),
            col("locator.span_end"),
            col("source_file.source_digest"),
            source_bytes.alias("source_bytes"),
            decoded_text.alias("decoded_text"),
            col("locator.origin_query_id"),
        ])?
        .distinct()?;
    bounded_plan(builder, &block.typed.canonical_order, block.typed.limit)
}

async fn runtime_relational_plan(
    session: &ServingQuerySession,
    block: &BoundQueryBlock,
    query: &SemanticQueryClause,
    plan: &RelationalOperatorPlan,
    completed: &BTreeMap<String, CompletedBlock>,
) -> Result<LogicalPlan, SemanticQueryError> {
    let compiler = session.ontology_compiler();
    let runtime_plan = match &plan.runtime {
        RelationalRuntime::Snapshot => plan.template_plan.clone(),
        RelationalRuntime::Combine {
            operation,
            identity_role,
        } => compile_runtime_combine(&compiler, block, *operation, *identity_role, completed)?,
        RelationalRuntime::Summarize {
            summary_names,
            group_by,
        } => compile_runtime_summary(&compiler, block, query, summary_names, group_by, completed)?,
        RelationalRuntime::SourceContext { text_handling, .. } => {
            compile_runtime_source_context(session, block, *text_handling, completed).await?
        }
    };
    session
        .validate_query_plan(&runtime_plan)
        .map_err(|error| {
            phase_error(
                "QUERY_PLAN_POLICY_REJECTED",
                "structural_policy",
                &block.typed.source_pointer,
                error.to_string(),
            )
        })?;
    Ok(runtime_plan)
}

#[derive(Debug)]
struct GraphExecution {
    batches: Vec<RecordBatch>,
    values: ResponseValues,
    coverage: BTreeMap<String, u64>,
    completeness: CompletenessState,
    limit_state: LimitState,
}

#[derive(Debug)]
struct QueryGraph {
    graph: DiGraph<[u8; 16], usize, u32>,
    nodes: BTreeMap<[u8; 16], NodeIndex<u32>>,
    edges: Vec<GraphEdge>,
    parallel_edge_count: usize,
}

#[derive(Clone, Debug, Default)]
struct SeedSet {
    entities: BTreeSet<[u8; 16]>,
    facts: BTreeSet<[u8; 16]>,
    unconstrained: bool,
}

#[derive(Clone, Debug, Default)]
struct GraphInputs {
    starting: SeedSet,
    ending: SeedSet,
    binding_candidates: BTreeMap<String, SeedSet>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PathWitness {
    node_ids: Vec<[u8; 16]>,
    edge_payloads: Vec<usize>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum GraphOutputRow {
    Follow {
        edge_payload: usize,
        distance: usize,
    },
    Path {
        path_id: [u8; 16],
        witness: PathWitness,
    },
    Pattern {
        binding_id: [u8; 16],
        names: Vec<String>,
        entity_ids: Vec<[u8; 16]>,
        edge_payloads: Vec<usize>,
    },
}

fn graph_result_identity(form: QueryForm, values: &[&[u8]]) -> ([u8; 16], String) {
    let mut fingerprint = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::QueryResultValueV1,
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

impl QueryGraph {
    fn build(
        mut edges: Vec<GraphEdge>,
        maximum_memory_bytes: usize,
    ) -> Result<Self, SemanticQueryError> {
        edges.sort();
        for pair in edges.windows(2) {
            if pair[0].fact_id == pair[1].fact_id && pair[0] != pair[1] {
                return Err(phase_error(
                    "QUERY_GRAPH_FACT_ID_CONFLICT",
                    "graph_physical_build",
                    "",
                    "one fact identity maps to conflicting relation rows",
                ));
            }
        }
        edges.dedup();
        let node_ids = edges
            .iter()
            .flat_map(|edge| [edge.source_id, edge.target_id])
            .collect::<BTreeSet<_>>();
        let estimated_bytes = node_ids
            .len()
            .checked_mul(96)
            .and_then(|bytes| {
                edges
                    .len()
                    .checked_mul(std::mem::size_of::<GraphEdge>().saturating_add(48))
                    .and_then(|edge_bytes| bytes.checked_add(edge_bytes))
            })
            .ok_or_else(|| {
                phase_error(
                    "QUERY_GRAPH_MEMORY_OVERFLOW",
                    "resource_contract",
                    "",
                    "graph memory estimate overflow",
                )
            })?;
        if estimated_bytes > maximum_memory_bytes {
            return Err(phase_error(
                "QUERY_GRAPH_MEMORY_LIMIT",
                "resource_contract",
                "",
                format!(
                    "graph operator memory bound exceeded: {estimated_bytes} > {maximum_memory_bytes}"
                ),
            ));
        }
        let mut graph = DiGraph::<[u8; 16], usize, u32>::with_capacity(node_ids.len(), edges.len());
        let mut nodes = BTreeMap::new();
        for node_id in node_ids {
            let node = graph.try_add_node(node_id).map_err(|error| {
                phase_error(
                    "QUERY_GRAPH_INDEX_OVERFLOW",
                    "graph_physical_build",
                    "",
                    error.to_string(),
                )
            })?;
            nodes.insert(node_id, node);
        }
        for (payload, edge) in edges.iter().enumerate() {
            graph
                .try_add_edge(nodes[&edge.source_id], nodes[&edge.target_id], payload)
                .map_err(|error| {
                    phase_error(
                        "QUERY_GRAPH_INDEX_OVERFLOW",
                        "graph_physical_build",
                        "",
                        error.to_string(),
                    )
                })?;
        }
        let unique_endpoint_pairs = edges
            .iter()
            .map(|edge| (edge.source_id, edge.target_id))
            .collect::<BTreeSet<_>>()
            .len();
        Ok(Self {
            graph,
            nodes,
            parallel_edge_count: edges.len().saturating_sub(unique_endpoint_pairs),
            edges,
        })
    }
}

fn graph_steps<G>(graph: G, node: NodeIndex<u32>) -> Vec<(usize, NodeIndex<u32>)>
where
    G: Copy + Data<EdgeWeight = usize> + IntoEdgesDirected<NodeId = NodeIndex<u32>>,
    G::EdgeRef: petgraph::visit::EdgeRef<NodeId = NodeIndex<u32>>,
{
    graph
        .edges_directed(node, Direction::Outgoing)
        .map(|edge| (*edge.weight(), edge.target()))
        .collect()
}

fn directed_steps(
    graph: &QueryGraph,
    node: NodeIndex<u32>,
    direction: GraphDirection,
) -> Vec<(usize, NodeIndex<u32>)> {
    let mut steps = match direction {
        GraphDirection::Outgoing => graph_steps(&graph.graph, node),
        GraphDirection::Incoming => graph_steps(Reversed(&graph.graph), node),
        GraphDirection::Both => {
            let mut steps = graph_steps(&graph.graph, node);
            steps.extend(graph_steps(Reversed(&graph.graph), node));
            steps
        }
    };
    steps.sort_unstable_by_key(|(payload, next)| {
        (graph.edges[*payload].fact_id, graph.graph[*next])
    });
    steps.dedup();
    steps
}

fn packed_ids(values: &[[u8; 16]]) -> Vec<u8> {
    values
        .iter()
        .flat_map(<[u8; 16]>::as_slice)
        .copied()
        .collect()
}

#[allow(clippy::too_many_lines)] // One exhaustive encoder preserves the distinct Arrow contract of each graph form.
fn graph_batch(
    plan: &GraphOperatorPlan,
    graph: &QueryGraph,
    rows: &[GraphOutputRow],
) -> Result<RecordBatch, SemanticQueryError> {
    let columns = match plan.form {
        QueryForm::FollowRelationships => {
            let payloads = rows
                .iter()
                .filter_map(|row| match row {
                    GraphOutputRow::Follow { edge_payload, .. } => Some(*edge_payload),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let distances = rows.iter().filter_map(|row| match row {
                GraphOutputRow::Follow { distance, .. } => Some(*distance),
                _ => None,
            });
            vec![
                crate::fabric::id16_array(
                    payloads
                        .iter()
                        .map(|payload| Some(&graph.edges[*payload].fact_id)),
                ),
                crate::fabric::id16_array(
                    payloads
                        .iter()
                        .map(|payload| Some(&graph.edges[*payload].source_id)),
                ),
                crate::fabric::id16_array(
                    payloads
                        .iter()
                        .map(|payload| Some(&graph.edges[*payload].target_id)),
                ),
                Arc::new(Int32Array::from_iter_values(
                    payloads
                        .iter()
                        .map(|payload| graph.edges[*payload].relation_kind_code),
                )),
                Arc::new(Int16Array::from_iter_values(
                    payloads
                        .iter()
                        .map(|payload| graph.edges[*payload].directness_code),
                )),
                Arc::new(Int16Array::from_iter_values(
                    payloads
                        .iter()
                        .map(|payload| graph.edges[*payload].certainty_code),
                )),
                Arc::new(Int16Array::from_iter_values(
                    payloads
                        .iter()
                        .map(|payload| graph.edges[*payload].resolution_code),
                )),
                Arc::new(UInt64Array::from_iter_values(
                    distances.map(|distance| u64::try_from(distance).unwrap_or(u64::MAX)),
                )),
                crate::fabric::id16_array(
                    payloads
                        .iter()
                        .map(|payload| Some(&graph.edges[*payload].fact_id)),
                ),
                Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                    plan.block_id.as_str(),
                    payloads.len(),
                ))),
            ]
        }
        QueryForm::FindPaths => {
            let paths = rows
                .iter()
                .filter_map(|row| match row {
                    GraphOutputRow::Path { path_id, witness } => Some((path_id, witness)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let first_facts = paths.iter().map(|(_, witness)| {
                witness
                    .edge_payloads
                    .first()
                    .map(|payload| &graph.edges[*payload].fact_id)
            });
            let fact_ids = paths
                .iter()
                .map(|(_, witness)| {
                    witness
                        .edge_payloads
                        .iter()
                        .map(|payload| graph.edges[*payload].fact_id)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let certainty = paths
                .iter()
                .map(|(_, witness)| {
                    witness
                        .edge_payloads
                        .iter()
                        .map(|payload| graph.edges[*payload].certainty_code.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect::<Vec<_>>();
            vec![
                crate::fabric::id16_array(paths.iter().map(|(path_id, _)| Some(*path_id))),
                crate::fabric::id16_array(first_facts),
                crate::fabric::id16_array(
                    paths.iter().map(|(_, witness)| witness.node_ids.first()),
                ),
                crate::fabric::id16_array(paths.iter().map(|(_, witness)| witness.node_ids.last())),
                crate::fabric::id16_list_array(
                    plan.output_schema
                        .field_with_name("ordered_entity_ids")
                        .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
                    paths.iter().map(|(_, witness)| witness.node_ids.as_slice()),
                )
                .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
                crate::fabric::id16_list_array(
                    plan.output_schema
                        .field_with_name("ordered_fact_ids")
                        .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
                    fact_ids.iter().map(Vec::as_slice),
                )
                .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
                Arc::new(UInt64Array::from_iter_values(paths.iter().map(
                    |(_, witness)| u64::try_from(witness.edge_payloads.len()).unwrap_or(u64::MAX),
                ))),
                Arc::new(StringArray::from_iter_values(certainty.iter())),
                Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                    plan.block_id.as_str(),
                    paths.len(),
                ))),
            ]
        }
        QueryForm::MatchPattern => {
            let matches = rows
                .iter()
                .filter_map(|row| match row {
                    GraphOutputRow::Pattern {
                        binding_id,
                        names,
                        entity_ids,
                        edge_payloads,
                    } => Some((binding_id, names, entity_ids, edge_payloads)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let fact_ids = matches
                .iter()
                .map(|(_, _, _, payloads)| {
                    payloads
                        .iter()
                        .map(|payload| graph.edges[*payload].fact_id)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            vec![
                crate::fabric::id16_array(
                    matches
                        .iter()
                        .map(|(binding_id, _, _, _)| Some(*binding_id)),
                ),
                crate::fabric::id16_array(matches.iter().map(|(_, _, _, payloads)| {
                    payloads
                        .first()
                        .map(|payload| &graph.edges[*payload].fact_id)
                })),
                Arc::new(StringArray::from_iter_values(
                    matches.iter().map(|(_, names, _, _)| names.join(",")),
                )),
                crate::fabric::id16_list_array(
                    plan.output_schema
                        .field_with_name("binding_entity_ids")
                        .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
                    matches.iter().map(|(_, _, ids, _)| ids.as_slice()),
                )
                .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
                crate::fabric::id16_list_array(
                    plan.output_schema
                        .field_with_name("witness_fact_ids")
                        .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
                    fact_ids.iter().map(Vec::as_slice),
                )
                .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
                Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                    plan.block_id.as_str(),
                    matches.len(),
                ))),
            ]
        }
        _ => {
            return Err(phase_error(
                "QUERY_FORM_COMPILER_MISMATCH",
                "graph_execution",
                &plan.block_id,
                "relational form reached graph batch encoding",
            ));
        }
    };
    RecordBatch::try_new(Arc::clone(&plan.output_schema), columns)
        .map_err(ServingQueryError::from)
        .map_err(SemanticQueryError::from)
}

#[allow(clippy::too_many_lines)] // Projection, validation, and physical census stay in one fail-closed build boundary.
async fn load_graph_projection(
    session: &ServingQuerySession,
    execution: &QueryExecutionContext,
    graph_plan: &GraphOperatorPlan,
    artifacts: &QueryExecutionArtifactAccumulator,
    cancellation: &crate::cancellation::Cancellation,
) -> Result<(QueryGraph, QueryPlanArtifact), SemanticQueryError> {
    let mut builder = LogicalPlanBuilder::from(session.table_plan("relations").await?);
    if !graph_plan.relationship_kind_codes.is_empty() {
        builder = builder.filter(
            col("relation_kind_code").in_list(
                graph_plan
                    .relationship_kind_codes
                    .iter()
                    .copied()
                    .map(lit)
                    .collect::<Vec<_>>(),
                false,
            ),
        )?;
    }
    if !graph_plan.certainty_codes.is_empty() {
        builder = builder.filter(
            col("certainty_code").in_list(
                graph_plan
                    .certainty_codes
                    .iter()
                    .copied()
                    .map(lit)
                    .collect::<Vec<_>>(),
                false,
            ),
        )?;
    }
    let plan = builder
        .project(vec![
            col("fact_id"),
            col("source_id"),
            col("target_id"),
            col("relation_family_code"),
            col("relation_kind_code"),
            col("distance"),
            col("directness_code"),
            col("certainty_code"),
            col("resolution_code"),
        ])?
        .sort(vec![col("fact_id").sort(true, true)])?
        .build()?;
    session.validate_query_plan(&plan)?;
    let result = session
        .query_plan_in_execution_observed_with_cancellation(
            &format!("{}-graph-input", graph_plan.block_id),
            plan,
            execution,
            artifacts,
            cancellation,
        )
        .await?;
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
        let i16s = |name: &str| -> Result<&Int16Array, SemanticQueryError> {
            let index = batch.schema().index_of(name).map_err(|_| {
                SemanticQueryError::Invalid(format!("graph edge column {name} is absent"))
            })?;
            batch
                .column(index)
                .as_any()
                .downcast_ref::<Int16Array>()
                .ok_or_else(|| {
                    SemanticQueryError::Invalid(format!("graph edge column {name} is not Int16"))
                })
        };
        let i32s = |name: &str| -> Result<&Int32Array, SemanticQueryError> {
            let index = batch.schema().index_of(name).map_err(|_| {
                SemanticQueryError::Invalid(format!("graph edge column {name} is absent"))
            })?;
            batch
                .column(index)
                .as_any()
                .downcast_ref::<Int32Array>()
                .ok_or_else(|| {
                    SemanticQueryError::Invalid(format!("graph edge column {name} is not Int32"))
                })
        };
        let families = i16s("relation_family_code")?;
        let kinds = i32s("relation_kind_code")?;
        let distances = i32s("distance")?;
        let directness = i16s("directness_code")?;
        let certainties = i16s("certainty_code")?;
        let resolutions = i16s("resolution_code")?;
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
                relation_family_code: families.value(row),
                relation_kind_code: kinds.value(row),
                distance: (!distances.is_null(row)).then(|| distances.value(row)),
                directness_code: directness.value(row),
                certainty_code: certainties.value(row),
                resolution_code: resolutions.value(row),
            });
        }
    }
    let graph = QueryGraph::build(edges, graph_plan.maximum_memory_bytes)?;
    let mut artifact = result.artifact;
    artifact.execution_metrics.insert(
        "application_graph_nodes".to_owned(),
        u64::try_from(graph.graph.node_count()).unwrap_or(u64::MAX),
    );
    artifact.execution_metrics.insert(
        "application_graph_edges".to_owned(),
        u64::try_from(graph.graph.edge_count()).unwrap_or(u64::MAX),
    );
    artifact.execution_metrics.insert(
        "application_graph_parallel_edges".to_owned(),
        u64::try_from(graph.parallel_edge_count).unwrap_or(u64::MAX),
    );
    write!(
        artifact.physical_plan,
        "\nApplicationGraphProjection[petgraph=0.8.3,index=u32,nodes={},edges={},parallel_edges={},algorithm={:?},maximum_depth={},memory_bound={},cancellation=poll-each-expansion]",
        graph.graph.node_count(),
        graph.graph.edge_count(),
        graph.parallel_edge_count,
        graph_plan.semantics,
        graph_plan.maximum_depth,
        graph_plan.maximum_memory_bytes,
    )
    .expect("writing to String cannot fail");
    Ok((graph, artifact))
}

fn poll_graph_cancellation(
    cancellation: &crate::cancellation::Cancellation,
    polls: &mut u32,
) -> Result<(), SemanticQueryError> {
    *polls = polls.saturating_add(1);
    if polls.is_multiple_of(cancellation.check_interval()) && cancellation.is_cancelled() {
        return Err(phase_error(
            "QUERY_EXECUTION_CANCELLED",
            "graph_execution",
            "",
            "graph execution was cancelled",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_shortest_witnesses(
    graph: &QueryGraph,
    current: NodeIndex<u32>,
    start: NodeIndex<u32>,
    predecessors: &BTreeMap<NodeIndex<u32>, Vec<(NodeIndex<u32>, usize)>>,
    reversed_nodes: &mut Vec<NodeIndex<u32>>,
    reversed_edges: &mut Vec<usize>,
    output: &mut Vec<PathWitness>,
    maximum_results: usize,
    cancellation: &crate::cancellation::Cancellation,
    polls: &mut u32,
) -> Result<(), SemanticQueryError> {
    poll_graph_cancellation(cancellation, polls)?;
    if output.len() > maximum_results {
        return Ok(());
    }
    if current == start {
        output.push(PathWitness {
            node_ids: reversed_nodes
                .iter()
                .rev()
                .map(|node| graph.graph[*node])
                .collect(),
            edge_payloads: reversed_edges.iter().rev().copied().collect(),
        });
        return Ok(());
    }
    for (predecessor, payload) in predecessors.get(&current).into_iter().flatten() {
        reversed_nodes.push(*predecessor);
        reversed_edges.push(*payload);
        collect_shortest_witnesses(
            graph,
            *predecessor,
            start,
            predecessors,
            reversed_nodes,
            reversed_edges,
            output,
            maximum_results,
            cancellation,
            polls,
        )?;
        reversed_edges.pop();
        reversed_nodes.pop();
        if output.len() > maximum_results {
            break;
        }
    }
    Ok(())
}

fn shortest_paths(
    graph: &QueryGraph,
    start_id: [u8; 16],
    target_id: [u8; 16],
    direction: GraphDirection,
    maximum_depth: usize,
    maximum_results: usize,
    cancellation: &crate::cancellation::Cancellation,
) -> Result<(Vec<PathWitness>, bool), SemanticQueryError> {
    let (Some(&start), Some(&target)) = (graph.nodes.get(&start_id), graph.nodes.get(&target_id))
    else {
        return Ok((Vec::new(), false));
    };
    if start == target {
        return Ok((Vec::new(), false));
    }
    let mut queue = VecDeque::from([start]);
    let mut distances = BTreeMap::from([(start, 0_usize)]);
    let mut predecessors = BTreeMap::<NodeIndex<u32>, Vec<(NodeIndex<u32>, usize)>>::new();
    let mut polls = 0_u32;
    let mut depth_bound_reached = false;
    let mut target_distance = None;
    while let Some(current) = queue.pop_front() {
        poll_graph_cancellation(cancellation, &mut polls)?;
        let distance = distances[&current];
        if target_distance.is_some_and(|target| distance >= target) {
            continue;
        }
        let steps = directed_steps(graph, current, direction);
        if distance >= maximum_depth {
            depth_bound_reached |= !steps.is_empty();
            continue;
        }
        for (payload, next) in steps {
            let candidate_distance = distance.saturating_add(1);
            match distances.get(&next).copied() {
                None => {
                    distances.insert(next, candidate_distance);
                    predecessors.insert(next, vec![(current, payload)]);
                    if next == target {
                        target_distance = Some(candidate_distance);
                    }
                    queue.push_back(next);
                }
                Some(existing) if existing == candidate_distance => {
                    predecessors
                        .entry(next)
                        .or_default()
                        .push((current, payload));
                }
                _ => {}
            }
        }
    }
    if target_distance.is_none() {
        return Ok((Vec::new(), depth_bound_reached));
    }
    for entries in predecessors.values_mut() {
        entries.sort_unstable_by_key(|(node, payload)| {
            (graph.edges[*payload].fact_id, graph.graph[*node])
        });
        entries.dedup();
    }
    let mut output = Vec::new();
    collect_shortest_witnesses(
        graph,
        target,
        start,
        &predecessors,
        &mut vec![target],
        &mut Vec::new(),
        &mut output,
        maximum_results,
        cancellation,
        &mut polls,
    )?;
    output.sort();
    output.dedup();
    Ok((output, depth_bound_reached))
}

#[allow(clippy::too_many_arguments)]
fn collect_simple_paths(
    graph: &QueryGraph,
    current: NodeIndex<u32>,
    target: NodeIndex<u32>,
    direction: GraphDirection,
    maximum_depth: usize,
    visited: &mut BTreeSet<NodeIndex<u32>>,
    nodes: &mut Vec<NodeIndex<u32>>,
    edges: &mut Vec<usize>,
    output: &mut Vec<PathWitness>,
    maximum_results: usize,
    cancellation: &crate::cancellation::Cancellation,
    polls: &mut u32,
) -> Result<bool, SemanticQueryError> {
    poll_graph_cancellation(cancellation, polls)?;
    if output.len() > maximum_results {
        return Ok(false);
    }
    if current == target {
        output.push(PathWitness {
            node_ids: nodes.iter().map(|node| graph.graph[*node]).collect(),
            edge_payloads: edges.clone(),
        });
        return Ok(false);
    }
    let steps = directed_steps(graph, current, direction);
    if edges.len() >= maximum_depth {
        return Ok(!steps.is_empty());
    }
    let mut bound_reached = false;
    for (payload, next) in steps {
        if !visited.insert(next) {
            continue;
        }
        nodes.push(next);
        edges.push(payload);
        bound_reached |= collect_simple_paths(
            graph,
            next,
            target,
            direction,
            maximum_depth,
            visited,
            nodes,
            edges,
            output,
            maximum_results,
            cancellation,
            polls,
        )?;
        edges.pop();
        nodes.pop();
        visited.remove(&next);
        if output.len() > maximum_results {
            break;
        }
    }
    Ok(bound_reached)
}

fn all_simple_paths(
    graph: &QueryGraph,
    start_id: [u8; 16],
    target_id: [u8; 16],
    direction: GraphDirection,
    maximum_depth: usize,
    maximum_results: usize,
    cancellation: &crate::cancellation::Cancellation,
) -> Result<(Vec<PathWitness>, bool), SemanticQueryError> {
    let (Some(&start), Some(&target)) = (graph.nodes.get(&start_id), graph.nodes.get(&target_id))
    else {
        return Ok((Vec::new(), false));
    };
    let mut output = Vec::new();
    let bound_reached = collect_simple_paths(
        graph,
        start,
        target,
        direction,
        maximum_depth,
        &mut BTreeSet::from([start]),
        &mut vec![start],
        &mut Vec::new(),
        &mut output,
        maximum_results,
        cancellation,
        &mut 0,
    )?;
    output.sort();
    output.dedup();
    Ok((output, bound_reached))
}

fn collect_id_column(
    completed: &CompletedBlock,
    column_name: &str,
    output: &mut BTreeSet<[u8; 16]>,
) -> Result<bool, SemanticQueryError> {
    let Ok(schema_index) = completed.schema.index_of(column_name) else {
        return Ok(false);
    };
    if completed.schema.field(schema_index).data_type() != &DataType::FixedSizeBinary(16) {
        return Err(phase_error(
            "QUERY_DEPENDENCY_SCHEMA_MISMATCH",
            "dependency_binding",
            column_name,
            "dependency identity column is not Id16",
        ));
    }
    for batch in &completed.batches {
        let index = batch.schema().index_of(column_name).map_err(|_| {
            phase_error(
                "QUERY_DEPENDENCY_SCHEMA_MISMATCH",
                "dependency_binding",
                column_name,
                "dependency batch omits an identity declared by its schema",
            )
        })?;
        let Some(values) = batch
            .column(index)
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
        else {
            return Err(phase_error(
                "QUERY_DEPENDENCY_SCHEMA_MISMATCH",
                "dependency_binding",
                column_name,
                "dependency identity column is not Id16",
            ));
        };
        for row in 0..values.len() {
            if !values.is_null(row) {
                output.insert(values.value(row).try_into().map_err(|_| {
                    phase_error(
                        "QUERY_DEPENDENCY_SCHEMA_MISMATCH",
                        "dependency_binding",
                        column_name,
                        "dependency identity width drifted",
                    )
                })?);
            }
        }
    }
    Ok(true)
}

fn bind_seed_reference(
    reference: &SemanticReference,
    completed: &BTreeMap<String, CompletedBlock>,
    output: &mut SeedSet,
) -> Result<(), SemanticQueryError> {
    match reference {
        SemanticReference::Entity { entity_id } => {
            output.entities.insert(id16_bytes(entity_id, "entity:")?);
        }
        SemanticReference::Fact { fact_id } => {
            output.facts.insert(id16_bytes(fact_id, "fact:")?);
        }
        SemanticReference::Phrase(_) => output.unconstrained = true,
        SemanticReference::PriorResult(reference) => {
            let produced = completed.get(&reference.results_of).ok_or_else(|| {
                phase_error(
                    "QUERY_DEPENDENCY_NOT_READY",
                    "dependency_binding",
                    &reference.results_of,
                    "prior-result Arrow batches are unavailable",
                )
            })?;
            match produced.role {
                ResultRole::Entities => {
                    if !collect_id_column(produced, "entity_id", &mut output.entities)? {
                        return Err(phase_error(
                            "QUERY_DEPENDENCY_SCHEMA_MISMATCH",
                            "dependency_binding",
                            &reference.results_of,
                            "entity dependency omits entity_id",
                        ));
                    }
                }
                ResultRole::Facts => {
                    let mut found = collect_id_column(produced, "fact_id", &mut output.facts)?;
                    found |= collect_id_column(produced, "source_id", &mut output.entities)?;
                    found |= collect_id_column(produced, "target_id", &mut output.entities)?;
                    found |= collect_id_column(produced, "subject_id", &mut output.entities)?;
                    found |= collect_id_column(produced, "object_or_value", &mut output.entities)?;
                    if !found {
                        return Err(phase_error(
                            "QUERY_DEPENDENCY_SCHEMA_MISMATCH",
                            "dependency_binding",
                            &reference.results_of,
                            "fact dependency omits typed fact and endpoint identities",
                        ));
                    }
                }
                _ => {
                    return Err(phase_error(
                        "QUERY_ROLE_INCOMPATIBLE",
                        "dependency_binding",
                        &reference.results_of,
                        "graph seed requires entity or fact Arrow output",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn bind_seed_set(
    references: &[SemanticReference],
    completed: &BTreeMap<String, CompletedBlock>,
) -> Result<SeedSet, SemanticQueryError> {
    let mut output = SeedSet::default();
    for reference in references {
        bind_seed_reference(reference, completed, &mut output)?;
    }
    Ok(output)
}

fn graph_inputs(
    query: &SemanticQueryClause,
    completed: &BTreeMap<String, CompletedBlock>,
) -> Result<GraphInputs, SemanticQueryError> {
    let mut inputs = GraphInputs::default();
    match query {
        SemanticQueryClause::FollowRelationships { starting_from, .. } => {
            inputs.starting = bind_seed_set(starting_from, completed)?;
        }
        SemanticQueryClause::FindPaths {
            starting_from,
            ending_at,
            ..
        } => {
            inputs.starting = bind_seed_set(starting_from, completed)?;
            inputs.ending = bind_seed_set(ending_at, completed)?;
        }
        SemanticQueryClause::MatchPattern { bindings, .. } => {
            for binding in bindings {
                let mut candidates = SeedSet::default();
                if let Some(reference) = &binding.within {
                    bind_seed_reference(reference, completed, &mut candidates)?;
                } else {
                    candidates.unconstrained = true;
                }
                inputs
                    .binding_candidates
                    .insert(binding.name.clone(), candidates);
            }
        }
        _ => {
            return Err(phase_error(
                "QUERY_FORM_COMPILER_MISMATCH",
                "dependency_binding",
                query.query_id(),
                "relational form reached graph dependency binding",
            ));
        }
    }
    Ok(inputs)
}

fn seed_nodes(
    graph: &QueryGraph,
    seeds: &SeedSet,
    direction: GraphDirection,
) -> BTreeSet<[u8; 16]> {
    let mut entities = seeds.entities.clone();
    for fact_id in &seeds.facts {
        for edge in graph.edges.iter().filter(|edge| edge.fact_id == *fact_id) {
            match direction {
                GraphDirection::Outgoing => {
                    entities.insert(edge.source_id);
                }
                GraphDirection::Incoming => {
                    entities.insert(edge.target_id);
                }
                GraphDirection::Both => {
                    entities.extend([edge.source_id, edge.target_id]);
                }
            }
        }
    }
    if seeds.unconstrained {
        entities.extend(graph.nodes.keys().copied());
    }
    entities
}

fn relationship_witness(
    graph: &QueryGraph,
    source: [u8; 16],
    target: [u8; 16],
    relationship: &PatternRelationship,
    maximum_depth: usize,
    cancellation: &crate::cancellation::Cancellation,
) -> Result<Option<Vec<usize>>, SemanticQueryError> {
    let direction = graph_direction(relationship.direction.as_deref());
    if graph_distance(relationship.distance.as_deref()) == GraphDistance::Direct {
        let Some(&source_node) = graph.nodes.get(&source) else {
            return Ok(None);
        };
        return Ok(directed_steps(graph, source_node, direction)
            .into_iter()
            .find(|(_, next)| graph.graph[*next] == target)
            .map(|(payload, _)| vec![payload]));
    }
    Ok(shortest_paths(
        graph,
        source,
        target,
        direction,
        maximum_depth,
        1,
        cancellation,
    )?
    .0
    .into_iter()
    .next()
    .map(|path| path.edge_payloads))
}

#[allow(clippy::too_many_arguments)]
fn match_pattern_bindings(
    graph: &QueryGraph,
    bindings: &[String],
    candidates: &BTreeMap<String, Vec<[u8; 16]>>,
    relationships: &[PatternRelationship],
    position: usize,
    assigned: &mut BTreeMap<String, [u8; 16]>,
    output: &mut Vec<GraphOutputRow>,
    maximum_results: usize,
    maximum_depth: usize,
    cancellation: &crate::cancellation::Cancellation,
    polls: &mut u32,
) -> Result<(), SemanticQueryError> {
    poll_graph_cancellation(cancellation, polls)?;
    if output.len() > maximum_results {
        return Ok(());
    }
    if position < bindings.len() {
        let binding = &bindings[position];
        for candidate in candidates.get(binding).into_iter().flatten() {
            assigned.insert(binding.clone(), *candidate);
            let mut partially_valid = true;
            for relationship in relationships {
                if let (Some(source), Some(target)) = (
                    assigned.get(&relationship.from),
                    assigned.get(&relationship.to),
                ) && relationship_witness(
                    graph,
                    *source,
                    *target,
                    relationship,
                    maximum_depth,
                    cancellation,
                )?
                .is_none()
                {
                    partially_valid = false;
                    break;
                }
            }
            if partially_valid {
                match_pattern_bindings(
                    graph,
                    bindings,
                    candidates,
                    relationships,
                    position + 1,
                    assigned,
                    output,
                    maximum_results,
                    maximum_depth,
                    cancellation,
                    polls,
                )?;
            }
            assigned.remove(binding);
            if output.len() > maximum_results {
                break;
            }
        }
        return Ok(());
    }
    let mut names = assigned.keys().cloned().collect::<Vec<_>>();
    names.sort();
    let entity_ids = names.iter().map(|name| assigned[name]).collect::<Vec<_>>();
    let mut edge_payloads = Vec::new();
    for relationship in relationships {
        let Some(mut witness) = relationship_witness(
            graph,
            assigned[&relationship.from],
            assigned[&relationship.to],
            relationship,
            maximum_depth,
            cancellation,
        )?
        else {
            return Ok(());
        };
        edge_payloads.append(&mut witness);
    }
    edge_payloads.sort_unstable_by_key(|payload| graph.edges[*payload].fact_id);
    edge_payloads.dedup();
    let names_bytes = names.join("\0");
    let entities_bytes = packed_ids(&entity_ids);
    let facts_bytes = packed_ids(
        &edge_payloads
            .iter()
            .map(|payload| graph.edges[*payload].fact_id)
            .collect::<Vec<_>>(),
    );
    let (binding_id, _) = graph_result_identity(
        QueryForm::MatchPattern,
        &[names_bytes.as_bytes(), &entities_bytes, &facts_bytes],
    );
    output.push(GraphOutputRow::Pattern {
        binding_id,
        names,
        entity_ids,
        edge_payloads,
    });
    Ok(())
}

#[allow(clippy::too_many_lines)] // One bounded kernel dispatches the three governed graph semantics.
fn execute_graph_operator(
    plan: &GraphOperatorPlan,
    input: &GraphInputs,
    graph: &QueryGraph,
    cancellation: &crate::cancellation::Cancellation,
) -> Result<GraphExecution, SemanticQueryError> {
    let mut polls = 0;
    poll_graph_cancellation(cancellation, &mut polls)?;
    let mut rows = Vec::<GraphOutputRow>::new();
    let mut coverage = BTreeMap::from([
        (
            "examined_edges".to_owned(),
            u64::try_from(graph.edges.len()).unwrap_or(u64::MAX),
        ),
        (
            "graph_nodes".to_owned(),
            u64::try_from(graph.nodes.len()).unwrap_or(u64::MAX),
        ),
        (
            "parallel_edges".to_owned(),
            u64::try_from(graph.parallel_edge_count).unwrap_or(u64::MAX),
        ),
        ("negative_proof_available".to_owned(), 0),
    ]);
    let mut depth_bound_reached = false;
    match &plan.semantics {
        GraphSemantics::Follow {
            direction,
            distance,
            ..
        } => {
            let seeds = seed_nodes(graph, &input.starting, *direction);
            for seed in seeds {
                let Some(&seed_node) = graph.nodes.get(&seed) else {
                    continue;
                };
                let mut queue = VecDeque::from([(seed_node, 0_usize)]);
                let mut visited = graph.graph.visit_map();
                visited.visit(seed_node);
                while let Some((current, depth)) = queue.pop_front() {
                    poll_graph_cancellation(cancellation, &mut polls)?;
                    let steps = directed_steps(graph, current, *direction);
                    if depth >= plan.maximum_depth {
                        depth_bound_reached |= !steps.is_empty();
                        continue;
                    }
                    for (payload, next) in steps {
                        rows.push(GraphOutputRow::Follow {
                            edge_payload: payload,
                            distance: depth.saturating_add(1),
                        });
                        if *distance == GraphDistance::Transitive && visited.visit(next) {
                            queue.push_back((next, depth.saturating_add(1)));
                        }
                    }
                }
            }
            let components = if *distance == GraphDistance::Transitive {
                kosaraju_scc(&graph.graph)
                    .into_iter()
                    .filter(|component| component.len() > 1)
                    .count()
            } else {
                0
            };
            coverage.insert(
                "cyclic_component_count".to_owned(),
                u64::try_from(components).unwrap_or(u64::MAX),
            );
        }
        GraphSemantics::Paths {
            direction,
            path_policy,
            ..
        } => {
            let starts = seed_nodes(graph, &input.starting, *direction);
            let ends = seed_nodes(graph, &input.ending, *direction);
            for start in &starts {
                for end in &ends {
                    if start == end {
                        continue;
                    }
                    let (mut paths, bound_reached) = match path_policy {
                        PathPolicy::AllSimple => all_simple_paths(
                            graph,
                            *start,
                            *end,
                            *direction,
                            plan.maximum_depth,
                            plan.maximum_results,
                            cancellation,
                        )?,
                        PathPolicy::AllShortest | PathPolicy::DeterministicShortest => {
                            shortest_paths(
                                graph,
                                *start,
                                *end,
                                *direction,
                                plan.maximum_depth,
                                plan.maximum_results,
                                cancellation,
                            )?
                        }
                    };
                    depth_bound_reached |= bound_reached;
                    if *path_policy == PathPolicy::DeterministicShortest {
                        paths.truncate(1);
                    }
                    for witness in paths {
                        let entities = packed_ids(&witness.node_ids);
                        let facts = packed_ids(
                            &witness
                                .edge_payloads
                                .iter()
                                .map(|payload| graph.edges[*payload].fact_id)
                                .collect::<Vec<_>>(),
                        );
                        let (path_id, _) = graph_result_identity(plan.form, &[&entities, &facts]);
                        rows.push(GraphOutputRow::Path { path_id, witness });
                    }
                }
            }
        }
        GraphSemantics::Pattern {
            bindings,
            relationships,
        } => {
            let mut candidates = BTreeMap::new();
            for binding in bindings {
                let seeds = input
                    .binding_candidates
                    .get(&binding.name)
                    .cloned()
                    .unwrap_or_else(|| SeedSet {
                        unconstrained: true,
                        ..SeedSet::default()
                    });
                let mut ids = seed_nodes(graph, &seeds, GraphDirection::Both)
                    .into_iter()
                    .collect::<Vec<_>>();
                ids.sort_unstable();
                candidates.insert(binding.name.clone(), ids);
            }
            let mut binding_order = bindings
                .iter()
                .map(|binding| binding.name.clone())
                .collect::<Vec<_>>();
            binding_order.sort_by_key(|name| {
                (
                    candidates.get(name).map_or(usize::MAX, Vec::len),
                    name.clone(),
                )
            });
            match_pattern_bindings(
                graph,
                &binding_order,
                &candidates,
                relationships,
                0,
                &mut BTreeMap::new(),
                &mut rows,
                plan.maximum_results,
                plan.maximum_depth,
                cancellation,
                &mut polls,
            )?;
        }
    }
    rows.sort_by(|left, right| match (left, right) {
        (
            GraphOutputRow::Follow {
                edge_payload: left,
                distance: left_distance,
            },
            GraphOutputRow::Follow {
                edge_payload: right,
                distance: right_distance,
            },
        ) => (graph.edges[*left].fact_id, *left_distance)
            .cmp(&(graph.edges[*right].fact_id, *right_distance)),
        (
            GraphOutputRow::Path {
                path_id: left_id,
                witness: left_path,
            },
            GraphOutputRow::Path {
                path_id: right_id,
                witness: right_path,
            },
        ) => (left_path.edge_payloads.len(), *left_id)
            .cmp(&(right_path.edge_payloads.len(), *right_id)),
        _ => left.cmp(right),
    });
    rows.dedup();
    if plan.form == QueryForm::FollowRelationships {
        let mut by_fact = BTreeMap::new();
        for row in rows {
            if let GraphOutputRow::Follow { edge_payload, .. } = &row {
                by_fact
                    .entry(graph.edges[*edge_payload].fact_id)
                    .or_insert(row);
            }
        }
        rows = by_fact.into_values().collect();
    }
    let limit_reached = rows.len() > plan.maximum_results;
    if limit_reached {
        rows.truncate(plan.maximum_results);
    }
    let mut values = ResponseValues::default();
    for row in &rows {
        match row {
            GraphOutputRow::Follow { edge_payload, .. } => values.fact_ids.push(
                crate::identity::encode_public_id(
                    crate::identity::IdentityDomain::RelationFact,
                    Some("relation"),
                    graph.edges[*edge_payload].fact_id,
                )
                .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
            ),
            GraphOutputRow::Path { path_id, .. } => values.path_ids.push(
                crate::identity::encode_public_id(
                    crate::identity::IdentityDomain::ResultArtifact,
                    None,
                    *path_id,
                )
                .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
            ),
            GraphOutputRow::Pattern { binding_id, .. } => values.group_ids.push(
                crate::identity::encode_public_id(
                    crate::identity::IdentityDomain::ResultArtifact,
                    None,
                    *binding_id,
                )
                .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
            ),
        }
    }
    if depth_bound_reached {
        coverage.insert("depth_bound_reached".to_owned(), 1);
    }
    coverage.insert(
        "returned_rows".to_owned(),
        u64::try_from(rows.len()).unwrap_or(u64::MAX),
    );
    let completeness = if rows.is_empty() || depth_bound_reached {
        CompletenessState::Indeterminate
    } else if limit_reached {
        CompletenessState::Partial
    } else {
        CompletenessState::Complete
    };
    Ok(GraphExecution {
        batches: vec![graph_batch(plan, graph, &rows)?],
        values,
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
    let execution = QueryExecutionContext {
        execution_id: format!("direct:{}", validated.request_digest),
        semantic_request_id: validated.request.semantic_request_id.clone(),
        mcp_call_id: "not-applicable".to_owned(),
    };
    let artifacts = QueryExecutionArtifactAccumulator::new(execution.clone());
    match execute_request_in_context(
        session,
        validated,
        freshness,
        cancellation,
        execution,
        artifacts,
    )
    .await
    {
        SemanticTerminalOutcome::Succeeded { response, .. } => Ok(*response),
        SemanticTerminalOutcome::Failed { error, .. }
        | SemanticTerminalOutcome::Cancelled { error, .. } => Err(error),
    }
}

/// Execute one request under a boundary-allocated execution identity.
///
/// # Errors
///
/// Returns the same typed validation, snapshot, graph, execution, and encoding failures as
/// [`execute_request_with_cancellation`].
#[allow(clippy::too_many_lines)] // One pinned session keeps all clause results and response identities snapshot-coherent.
pub async fn execute_request_in_context(
    session: &ServingQuerySession,
    validated: ValidatedSemanticRequest,
    freshness: FreshnessState,
    cancellation: crate::cancellation::Cancellation,
    execution: QueryExecutionContext,
    artifacts: QueryExecutionArtifactAccumulator,
) -> SemanticTerminalOutcome {
    artifacts.set_phase("snapshot_binding");
    let outcome = execute_request_in_context_inner(
        session,
        validated,
        freshness,
        cancellation.clone(),
        execution,
        &artifacts,
    )
    .await;
    if outcome.is_err() && artifacts.snapshot().failing_stage.is_none() {
        let phase = artifacts.snapshot().lifecycle_phase;
        artifacts.set_failure(phase);
    }
    let evidence = artifacts.snapshot();
    match outcome {
        Ok(response) => SemanticTerminalOutcome::Succeeded {
            response: Box::new(response),
            evidence,
        },
        Err(error) if cancellation.is_cancelled() => {
            SemanticTerminalOutcome::Cancelled { error, evidence }
        }
        Err(error) => SemanticTerminalOutcome::Failed { error, evidence },
    }
}

#[allow(clippy::too_many_lines)] // One pinned session keeps DAG state and phase evidence snapshot-coherent.
async fn execute_request_in_context_inner(
    session: &ServingQuerySession,
    validated: ValidatedSemanticRequest,
    freshness: FreshnessState,
    cancellation: crate::cancellation::Cancellation,
    execution: QueryExecutionContext,
    artifacts: &QueryExecutionArtifactAccumulator,
) -> Result<ExecutedSemanticResponse, SemanticQueryError> {
    let manifest = session.snapshot_manifest();
    artifacts.pin_snapshot(&manifest);
    let bound = bind_request(session, &validated)
        .await
        .inspect_err(|error| {
            artifacts.set_failure("binding");
            artifacts.record_stage(QueryArtifactStage {
                block_id: "request".to_owned(),
                stage: "binding".to_owned(),
                state: QueryArtifactStageState::UnavailableWithReason,
                artifact: None,
                unavailable_reason: Some(error.to_string()),
                metrics: BTreeMap::new(),
            });
        })?;
    artifacts.set_phase("execution");
    artifacts.record_stage(QueryArtifactStage {
        block_id: "request".to_owned(),
        stage: "binding".to_owned(),
        state: QueryArtifactStageState::Complete,
        artifact: Some(format!("{bound:?}")),
        unavailable_reason: None,
        metrics: BTreeMap::new(),
    });
    let mut plan_artifacts = Vec::new();
    let mut results = Vec::with_capacity(validated.request.queries.len());
    let mut entities = BTreeMap::new();
    let mut facts = BTreeMap::new();
    let mut paths = BTreeMap::new();
    let mut groups = BTreeMap::new();
    let mut source_contexts = BTreeMap::new();
    let mut completed = BTreeMap::<String, CompletedBlock>::new();
    for block_id in &bound.execution_order {
        artifacts.set_phase(format!("block:{block_id}:execution"));
        let block = bound
            .blocks
            .iter()
            .find(|block| block.typed.block_id == *block_id)
            .ok_or_else(|| {
                SemanticQueryError::Invalid("bound execution order names no bound block".to_owned())
            })?;
        let query = validated
            .request
            .queries
            .iter()
            .find(|query| query.query_id() == *block_id)
            .ok_or_else(|| {
                SemanticQueryError::Invalid(
                    "typed execution order names no parsed block".to_owned(),
                )
            })?;
        let (
            values,
            coverage,
            completeness_state,
            limit_state,
            output_row_count,
            result_checksum,
            notices,
            output_batches,
            output_schema,
        ) = match &block.operator {
            BoundOperator::Relational(plan_spec) => {
                let plan =
                    runtime_relational_plan(session, block, query, plan_spec, &completed).await?;
                let output_schema = Arc::new(plan.schema().as_arrow().clone());
                let result = session
                    .query_plan_in_execution_observed_with_cancellation(
                        &block.typed.block_id,
                        plan,
                        &execution,
                        artifacts,
                        &cancellation,
                    )
                    .await?;
                plan_artifacts.push(result.artifact.clone());
                let produced_rows = result.artifact.output_row_count;
                let output_batches = limited_batches(&result.batches, block.typed.limit.first);
                let limit_reached = produced_rows > block.typed.limit.first;
                let output_row_count = output_batches.iter().map(RecordBatch::num_rows).sum();
                let values = relational_response_values(&output_batches, query.form())?;
                let mut checksum_material = Vec::new();
                for batch in &output_batches {
                    checksum_material.extend_from_slice(
                        &crate::fabric::batch_checksum(batch).map_err(|error| {
                            phase_error(
                                "QUERY_OUTPUT_CHECKSUM_FAILED",
                                "response_verification",
                                &block.typed.source_pointer,
                                error.to_string(),
                            )
                        })?,
                    );
                }
                let table_coverage = if plan_spec.source_tables.is_empty() {
                    "query-local-arrow".to_owned()
                } else {
                    plan_spec.source_tables.join(",")
                };
                let dependency_indeterminate = block.typed.dependencies.iter().any(|dependency| {
                    completed.get(dependency).is_some_and(|result| {
                        result.completeness == CompletenessState::Indeterminate
                    })
                });
                let completeness = if dependency_indeterminate || output_row_count == 0 {
                    CompletenessState::Indeterminate
                } else if limit_reached {
                    CompletenessState::Partial
                } else {
                    CompletenessState::Complete
                };
                let mut coverage = BTreeMap::from([
                    (
                        "returned_rows".to_owned(),
                        u64::try_from(output_row_count).unwrap_or(u64::MAX),
                    ),
                    ("native_datafusion_plan".to_owned(), 1),
                    (
                        format!(
                            "tables:{table_coverage}:column:{}",
                            plan_spec.identity_column
                        ),
                        u64::try_from(produced_rows).unwrap_or(u64::MAX),
                    ),
                    ("negative_proof_available".to_owned(), 0),
                ]);
                if output_row_count == 0 {
                    coverage.insert("empty_result".to_owned(), 1);
                }
                (
                    values,
                    coverage,
                    completeness,
                    if limit_reached {
                        LimitState::ExplicitLimitReached
                    } else {
                        LimitState::NotApplied
                    },
                    output_row_count,
                    b3(&checksum_material),
                    if completeness == CompletenessState::Indeterminate {
                        vec![
                            "empty or dependency-limited relational result is not proof of absence"
                                .to_owned(),
                        ]
                    } else {
                        Vec::new()
                    },
                    output_batches,
                    output_schema,
                )
            }
            BoundOperator::Graph(plan) => {
                let input = graph_inputs(query, &completed)?;
                let (graph, mut artifact) =
                    load_graph_projection(session, &execution, plan, artifacts, &cancellation)
                        .await?;
                let executed = execute_graph_operator(plan, &input, &graph, &cancellation)?;
                let output_row_count = executed.batches.iter().map(RecordBatch::num_rows).sum();
                let mut checksums = Vec::with_capacity(executed.batches.len() * 32);
                for batch in &executed.batches {
                    checksums.extend_from_slice(
                        &crate::fabric::batch_checksum(batch)
                            .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?,
                    );
                }
                let graph_result_checksum = b3(&checksums);
                artifact.output_row_count = output_row_count;
                artifact.output_schema = plan
                    .output_schema
                    .fields()
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                artifact.result_checksum.clone_from(&graph_result_checksum);
                artifact.execution_metrics.insert(
                    "application_graph_output_rows".to_owned(),
                    u64::try_from(output_row_count).unwrap_or(u64::MAX),
                );
                write!(
                    artifact.physical_plan_with_full_metrics,
                    "\nApplicationGraphOutput[rows={output_row_count},schema={}]",
                    plan.output_schema
                )
                .expect("writing to String cannot fail");
                artifacts.record_plan(artifact.clone());
                plan_artifacts.push(artifact);
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
                    graph_result_checksum,
                    notices,
                    executed.batches,
                    Arc::clone(&plan.output_schema),
                )
            }
        };
        let output_batches = canonical_dependency_batches(
            &output_schema,
            output_batches,
            &block.typed.source_pointer,
        )?;
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
        for source_context_id in &values.source_context_ids {
            source_contexts.insert(
                source_context_id.clone(),
                BTreeMap::from([("source_context_id".to_owned(), source_context_id.clone())]),
            );
        }
        let mut resolved_semantics = match &block.operator {
            BoundOperator::Relational(plan) => BTreeMap::from([
                (
                    "operator_family".to_owned(),
                    "datafusion-relational".to_owned(),
                ),
                ("tables".to_owned(), plan.source_tables.join(",")),
                ("order_key".to_owned(), plan.identity_column.to_owned()),
            ]),
            BoundOperator::Graph(plan) => BTreeMap::from([
                ("operator_family".to_owned(), "application-graph".to_owned()),
                (
                    "plan_node".to_owned(),
                    plan.form.plan_node_kind().to_owned(),
                ),
            ]),
        };
        if !block.typed.resolved_phrases.is_empty() {
            resolved_semantics.insert(
                "phrase_ids".to_owned(),
                block
                    .typed
                    .resolved_phrases
                    .iter()
                    .map(|phrase| phrase.phrase_id.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            );
            resolved_semantics.insert(
                "projection_ids".to_owned(),
                block
                    .typed
                    .resolved_phrases
                    .iter()
                    .map(|phrase| phrase.contract_code.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        results.push(QueryResultRecord {
            query_id: query.query_id().to_owned(),
            request: query.form(),
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
        completed.insert(
            block_id.clone(),
            CompletedBlock {
                role: block.typed.output_role,
                schema: output_schema,
                batches: output_batches,
                values,
                completeness: completeness_state,
            },
        );
        artifacts.record_coverage(
            format!("block:{block_id}:output_rows"),
            u64::try_from(output_row_count).unwrap_or(u64::MAX),
        );
    }
    artifacts.set_phase("response_encoding");
    let snapshot = snapshot_response(&manifest, freshness);
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
    artifacts.record_stage(QueryArtifactStage {
        block_id: "response".to_owned(),
        stage: "response_encoding".to_owned(),
        state: QueryArtifactStageState::Complete,
        artifact: Some(b3(&canonical_bytes)),
        unavailable_reason: None,
        metrics: BTreeMap::from([(
            "canonical_response_bytes".to_owned(),
            u64::try_from(canonical_bytes.len()).unwrap_or(u64::MAX),
        )]),
    });
    artifacts.set_phase("complete");
    Ok(ExecutedSemanticResponse {
        response_digest: b3(&canonical_bytes),
        response,
        canonical_bytes,
        plan_artifacts,
    })
}

fn limited_batches(batches: &[RecordBatch], maximum_rows: usize) -> Vec<RecordBatch> {
    let mut remaining = maximum_rows;
    let mut selected = Vec::new();
    for batch in batches {
        if remaining == 0 {
            break;
        }
        let rows = remaining.min(batch.num_rows());
        selected.push(batch.slice(0, rows));
        remaining -= rows;
    }
    selected
}

fn canonical_dependency_batches(
    schema: &SchemaRef,
    batches: Vec<RecordBatch>,
    source_pointer: &str,
) -> Result<Vec<RecordBatch>, SemanticQueryError> {
    batches
        .into_iter()
        .map(|batch| {
            if batch.num_columns() != schema.fields().len()
                || batch.schema().fields().iter().zip(schema.fields()).any(
                    |(observed, expected)| {
                        observed.name() != expected.name()
                            || observed.data_type() != expected.data_type()
                    },
                )
            {
                return Err(phase_error(
                    "QUERY_OUTPUT_SCHEMA_MISMATCH",
                    "response_verification",
                    source_pointer,
                    "physical output columns differ from the typed Arrow contract",
                ));
            }
            RecordBatch::try_new(Arc::clone(schema), batch.columns().to_vec()).map_err(|error| {
                phase_error(
                    "QUERY_OUTPUT_SCHEMA_MISMATCH",
                    "response_verification",
                    source_pointer,
                    error.to_string(),
                )
            })
        })
        .collect()
}

fn fixed_id_at(
    batch: &RecordBatch,
    column_name: &str,
    row: usize,
) -> Result<[u8; 16], SemanticQueryError> {
    let index = batch.schema().index_of(column_name).map_err(|_| {
        phase_error(
            "QUERY_OUTPUT_SCHEMA_MISMATCH",
            "response_verification",
            column_name,
            "result identity column is absent",
        )
    })?;
    let values = batch
        .column(index)
        .as_any()
        .downcast_ref::<FixedSizeBinaryArray>()
        .ok_or_else(|| {
            phase_error(
                "QUERY_OUTPUT_SCHEMA_MISMATCH",
                "response_verification",
                column_name,
                "result identity column is not Id16",
            )
        })?;
    if values.is_null(row) || values.value(row).len() != 16 {
        return Err(phase_error(
            "QUERY_OUTPUT_IDENTITY_INVALID",
            "response_verification",
            column_name,
            "result identity is null or has invalid width",
        ));
    }
    values.value(row).try_into().map_err(|_| {
        phase_error(
            "QUERY_OUTPUT_IDENTITY_INVALID",
            "response_verification",
            column_name,
            "result identity has invalid width",
        )
    })
}

fn encode_result_id(
    domain: crate::identity::IdentityDomain,
    kind_slug: Option<&str>,
    raw: [u8; 16],
) -> Result<String, SemanticQueryError> {
    crate::identity::encode_public_id(domain, kind_slug, raw).map_err(|error| {
        phase_error(
            "QUERY_OUTPUT_IDENTITY_INVALID",
            "response_verification",
            "",
            error.to_string(),
        )
    })
}

#[allow(clippy::too_many_lines)] // Exhaustive response verification keeps every form/domain pairing fail-closed.
fn relational_response_values(
    batches: &[RecordBatch],
    form: QueryForm,
) -> Result<ResponseValues, SemanticQueryError> {
    let mut output = ResponseValues::default();
    for batch in batches {
        for row in 0..batch.num_rows() {
            match form {
                QueryForm::FindEntities => output.entity_ids.push(encode_result_id(
                    crate::identity::IdentityDomain::Entity,
                    Some("unknown"),
                    fixed_id_at(batch, "entity_id", row)?,
                )?),
                QueryForm::RetrieveFacts => {
                    let class_index = batch.schema().index_of("fact_class").map_err(|_| {
                        phase_error(
                            "QUERY_OUTPUT_SCHEMA_MISMATCH",
                            "response_verification",
                            "fact_class",
                            "fact result omits its identity domain discriminator",
                        )
                    })?;
                    let classes = batch
                        .column(class_index)
                        .as_any()
                        .downcast_ref::<StringArray>()
                        .ok_or_else(|| {
                            phase_error(
                                "QUERY_OUTPUT_SCHEMA_MISMATCH",
                                "response_verification",
                                "fact_class",
                                "fact class is not Utf8",
                            )
                        })?;
                    let (domain, kind) = match classes.value(row) {
                        "relation" => (crate::identity::IdentityDomain::RelationFact, "relation"),
                        "property" => (crate::identity::IdentityDomain::PropertyFact, "property"),
                        _ => {
                            return Err(phase_error(
                                "QUERY_OUTPUT_IDENTITY_INVALID",
                                "response_verification",
                                "fact_class",
                                "fact class is outside the governed identity domains",
                            ));
                        }
                    };
                    output.fact_ids.push(encode_result_id(
                        domain,
                        Some(kind),
                        fixed_id_at(batch, "fact_id", row)?,
                    )?);
                }
                QueryForm::CombineResults => output.group_ids.push(encode_result_id(
                    crate::identity::IdentityDomain::ResultArtifact,
                    None,
                    fixed_id_at(batch, "group_key", row)?,
                )?),
                QueryForm::SummarizeFacts => {
                    let group_index = batch.schema().index_of("group_key").map_err(|_| {
                        phase_error(
                            "QUERY_OUTPUT_SCHEMA_MISMATCH",
                            "response_verification",
                            "group_key",
                            "summary group key is absent",
                        )
                    })?;
                    let groups = batch
                        .column(group_index)
                        .as_any()
                        .downcast_ref::<StringArray>()
                        .ok_or_else(|| {
                            phase_error(
                                "QUERY_OUTPUT_SCHEMA_MISMATCH",
                                "response_verification",
                                "group_key",
                                "summary group key is not Utf8",
                            )
                        })?;
                    let (raw, public) = graph_result_identity(
                        form,
                        &[groups.value(row).as_bytes(), row.to_string().as_bytes()],
                    );
                    let _ = raw;
                    output.group_ids.push(public);
                }
                QueryForm::RetrieveSourceContext => {
                    let file = fixed_id_at(batch, "source_file_id", row)?;
                    let start_index = batch.schema().index_of("span_start").map_err(|_| {
                        phase_error(
                            "QUERY_OUTPUT_SCHEMA_MISMATCH",
                            "response_verification",
                            "span_start",
                            "source context start is absent",
                        )
                    })?;
                    let end_index = batch.schema().index_of("span_end").map_err(|_| {
                        phase_error(
                            "QUERY_OUTPUT_SCHEMA_MISMATCH",
                            "response_verification",
                            "span_end",
                            "source context end is absent",
                        )
                    })?;
                    let starts = batch
                        .column(start_index)
                        .as_any()
                        .downcast_ref::<arrow_array::Int64Array>()
                        .ok_or_else(|| {
                            phase_error(
                                "QUERY_OUTPUT_SCHEMA_MISMATCH",
                                "response_verification",
                                "span_start",
                                "span start is not Int64",
                            )
                        })?;
                    let ends = batch
                        .column(end_index)
                        .as_any()
                        .downcast_ref::<arrow_array::Int64Array>()
                        .ok_or_else(|| {
                            phase_error(
                                "QUERY_OUTPUT_SCHEMA_MISMATCH",
                                "response_verification",
                                "span_end",
                                "span end is not Int64",
                            )
                        })?;
                    let bytes = [
                        file.as_slice(),
                        starts.value(row).to_be_bytes().as_slice(),
                        ends.value(row).to_be_bytes().as_slice(),
                    ]
                    .concat();
                    let mut fingerprint = crate::identity::semantic_fingerprint(
                        crate::identity::SemanticFingerprintDomain::QueryResultValueV1,
                    );
                    fingerprint.update(&bytes);
                    let digest = fingerprint.finalize();
                    let mut raw = [0_u8; 16];
                    raw.copy_from_slice(&digest[..16]);
                    output.source_context_ids.push(encode_result_id(
                        crate::identity::IdentityDomain::SourceContext,
                        None,
                        raw,
                    )?);
                }
                QueryForm::FollowRelationships | QueryForm::FindPaths | QueryForm::MatchPattern => {
                    return Err(phase_error(
                        "QUERY_FORM_COMPILER_MISMATCH",
                        "response_verification",
                        "",
                        "graph form reached relational response verification",
                    ));
                }
            }
        }
    }
    output.entity_ids.sort();
    output.entity_ids.dedup();
    output.fact_ids.sort();
    output.fact_ids.dedup();
    output.group_ids.sort();
    output.group_ids.dedup();
    output.source_context_ids.sort();
    output.source_context_ids.dedup();
    Ok(output)
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
        capability_summaries: manifest
            .body
            .contexts
            .records
            .iter()
            .map(|record| {
                let (capability_state, reason_code, diagnostic_id) = match freshness {
                    FreshnessState::Current => ("CURRENT", "NOT_APPLICABLE", "NOT_APPLICABLE"),
                    FreshnessState::PotentiallyStale => (
                        "UNKNOWN",
                        "CURRENT_FACTS_UNAVAILABLE",
                        "diagnostic:potentially-stale",
                    ),
                    FreshnessState::Unavailable => (
                        "UNKNOWN",
                        "CURRENT_FACTS_UNAVAILABLE",
                        "diagnostic:source-unavailable",
                    ),
                };
                BTreeMap::from([
                    (
                        "capability_code".to_owned(),
                        "ANALYSIS_CONTEXT_FACTS".to_owned(),
                    ),
                    (
                        "analysis_context_id".to_owned(),
                        record.analysis_context_id.clone(),
                    ),
                    (
                        "capability_partition_fingerprint".to_owned(),
                        record.capability_partition_digest.clone(),
                    ),
                    ("capability_state".to_owned(), capability_state.to_owned()),
                    ("reason_code".to_owned(), reason_code.to_owned()),
                    ("diagnostic_id".to_owned(), diagnostic_id.to_owned()),
                ])
            })
            .collect(),
        diagnostic_references: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eight_form_request() -> Vec<u8> {
        canonicalize_value(&serde_json::json!({
            "specification": SPECIFICATION,
            "version": VERSION,
            "semantic_request_id": "wp75-eight-form",
            "workspace_id": "workspace:00000000000000000000000000000000",
            "freshness_policy": "best_available_snapshot",
            "queries": [
                {"query_id":"entities","request":"find code entities","label":null,"looking_for":"syntax nodes","return":{"limit":{"maximum_results":10}}},
                {"query_id":"properties","request":"retrieve facts about code","label":null,"about":[{"results_of":"entities","select":"entities"}],"facts":["callable contracts"],"return":{"limit":{"maximum_results":10}}},
                {"query_id":"relations","request":"follow code relationships","label":null,"starting_from":[{"results_of":"entities","select":"entities"}],"relationship":"call targets","return":{"limit":{"maximum_results":10}}},
                {"query_id":"paths","request":"find connecting fact paths","label":null,"starting_from":[{"results_of":"entities","select":"entities"}],"ending_at":["matching destination entities"],"through":["control flow"],"path_policy":"one shortest witness path","maximum_length":4,"return":{"limit":{"maximum_results":10}}},
                {"query_id":"patterns","request":"match a code fact pattern","label":null,"bindings":[{"name":"source","looking_for":"syntax nodes","within":{"results_of":"entities","select":"entities"}}],"relationships":[],"return":{"limit":{"maximum_results":10}}},
                {"query_id":"combined","request":"combine result sets","label":null,"inputs":[{"results_of":"properties","select":"facts"},{"results_of":"relations","select":"facts"}],"combination":"union by fact identity","return":{"limit":{"maximum_results":10}}},
                {"query_id":"summary","request":"summarize objective facts","label":null,"input":[{"results_of":"combined","select":"groups"}],"summaries":["graph metrics"],"return":{"limit":{"maximum_results":10}}},
                {"query_id":"context","request":"retrieve source and syntax context","label":null,"for":[{"results_of":"paths","select":"paths"}],"context":["source location"],"return":{"limit":{"maximum_results":10}}}
            ],
            "response_projection": {"canonical_semantic_identity":true,"coverage":true},
            "cost_budget": {"maximum_rows":80}
        }))
        .unwrap()
    }

    fn graph_plan(typed: &TypedSemanticRequest, form: QueryForm) -> GraphOperatorPlan {
        let mut block = typed
            .blocks
            .iter()
            .find(|block| block.form == form)
            .unwrap()
            .clone();
        let query = typed
            .request
            .queries
            .iter()
            .find(|query| query.form() == form)
            .unwrap();
        let compiler = ontology_compiler().unwrap();
        block.resolved_phrases =
            resolve_query_phrases(&compiler, query, &block.source_pointer).unwrap();
        graph_operator_plan(&compiler, &block, query).unwrap()
    }

    fn graph_edges() -> Vec<GraphEdge> {
        vec![
            GraphEdge {
                fact_id: [0x11; 16],
                source_id: [0x01; 16],
                target_id: [0x02; 16],
                relation_family_code: 2,
                relation_kind_code: 10,
                distance: Some(1),
                directness_code: 10,
                certainty_code: 10,
                resolution_code: 10,
            },
            GraphEdge {
                fact_id: [0x12; 16],
                source_id: [0x02; 16],
                target_id: [0x03; 16],
                relation_family_code: 2,
                relation_kind_code: 10,
                distance: Some(1),
                directness_code: 10,
                certainty_code: 10,
                resolution_code: 10,
            },
        ]
    }

    fn parallel_graph_edges() -> Vec<GraphEdge> {
        let mut edges = graph_edges();
        edges.push(GraphEdge {
            fact_id: [0x13; 16],
            source_id: [0x01; 16],
            target_id: [0x02; 16],
            relation_family_code: 2,
            relation_kind_code: 10,
            distance: Some(1),
            directness_code: 10,
            certainty_code: 20,
            resolution_code: 20,
        });
        edges
    }

    fn reference_simple_path_count(
        edges: &[GraphEdge],
        current: [u8; 16],
        target: [u8; 16],
        maximum_depth: usize,
        visited: &mut BTreeSet<[u8; 16]>,
    ) -> usize {
        if current == target {
            return 1;
        }
        if maximum_depth == 0 {
            return 0;
        }
        let mut count = 0;
        for edge in edges.iter().filter(|edge| edge.source_id == current) {
            if visited.insert(edge.target_id) {
                count += reference_simple_path_count(
                    edges,
                    edge.target_id,
                    target,
                    maximum_depth - 1,
                    visited,
                );
                visited.remove(&edge.target_id);
            }
        }
        count
    }

    fn request() -> Vec<u8> {
        br#"{
          "specification":"composable semantic CPG fact query",
          "version":"1.3",
          "semantic_request_id":"gate-b",
          "workspace_id":"workspace:00000000000000000000000000000000",
          "freshness_policy":"best_available_snapshot",
          "queries":[
            {"query_id":"q1","request":"find code entities","label":null,"looking_for":"syntax nodes","return":{"limit":{"maximum_results":10}}},
            {"query_id":"q2","request":"retrieve facts about code","label":null,"about":[{"results_of":"q1","select":"entities"}],"facts":["callable contracts"],"return":{"limit":{"maximum_results":10}}},
            {"query_id":"q3","request":"follow code relationships","label":null,"starting_from":[{"results_of":"q1","select":"entities"}],"relationship":"call targets","return":{"limit":{"maximum_results":10}}}
          ],
          "response_projection":null,
          "cost_budget":{"maximum_rows":30}
        }"#
        .to_vec()
    }

    fn relational_request() -> Vec<u8> {
        canonicalize_value(&serde_json::json!({
            "specification": SPECIFICATION,
            "version": VERSION,
            "semantic_request_id": "wp02-relational",
            "workspace_id": "workspace:00000000000000000000000000000000",
            "freshness_policy": "best_available_snapshot",
            "queries": [
                {"query_id":"entities-a","request":"find code entities","label":null,"looking_for":"syntax nodes","return":{"limit":{"maximum_results":10}}},
                {"query_id":"entities-b","request":"find code entities","label":null,"looking_for":"semantic symbols","return":{"limit":{"maximum_results":10}}},
                {"query_id":"facts-a","request":"retrieve facts about code","label":null,"about":[{"results_of":"entities-a","select":"entities"}],"facts":["callable contracts"],"return":{"limit":{"maximum_results":10}}},
                {"query_id":"facts-b","request":"retrieve facts about code","label":null,"about":[{"results_of":"entities-b","select":"entities"}],"facts":["callable contracts"],"return":{"limit":{"maximum_results":10}}},
                {"query_id":"combined","request":"combine result sets","label":null,"inputs":[{"results_of":"facts-a","select":"facts"},{"results_of":"facts-b","select":"facts"}],"combination":"union by fact identity","identity":"fact identity","preserve_origin":"all origins","return":{"limit":{"maximum_results":10}}},
                {"query_id":"summary","request":"summarize objective facts","label":null,"input":[{"results_of":"combined","select":"groups"}],"summaries":["graph metrics"],"include_support":"fact identities","return":{"limit":{"maximum_results":10}}},
                {"query_id":"context","request":"retrieve source and syntax context","label":null,"for":[{"results_of":"facts-a","select":"facts"}],"context":["source location","exact span"],"text_handling":"omit text","return":{"limit":{"maximum_results":10}}}
            ],
            "response_projection": {"canonical_semantic_identity":true,"coverage":true},
            "cost_budget": {"maximum_rows":70}
        }))
        .unwrap()
    }

    #[test]
    fn wp38_behavioral_acceptance() {
        let validated = validate_request(&request()).unwrap();
        assert_eq!(validated.request.queries.len(), 3);
        assert!(validated.request_digest.starts_with("b3:"));
    }

    #[test]
    fn semantic_query_relational_policy_and_absence() {
        let typed = validate_request(&relational_request()).unwrap();
        assert_eq!(
            typed
                .blocks
                .iter()
                .map(|block| block.form)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                QueryForm::FindEntities,
                QueryForm::RetrieveFacts,
                QueryForm::CombineResults,
                QueryForm::SummarizeFacts,
                QueryForm::RetrieveSourceContext,
            ])
        );

        let mut unknown: serde_json::Value = serde_json::from_slice(&relational_request()).unwrap();
        unknown["queries"][0]["looking_for"] = serde_json::json!("guessed entities");
        let unknown = validate_request(&canonicalize_value(&unknown).unwrap()).unwrap();
        let block = &unknown.blocks[0];
        let query = unknown
            .request
            .queries
            .iter()
            .find(|query| query.query_id() == block.block_id)
            .unwrap();
        let compiler = ontology_compiler().unwrap();
        let error = resolve_query_phrases(&compiler, query, &block.source_pointer).unwrap_err();
        assert!(error.to_string().contains("semantic_binding"));
        assert!(error.to_string().contains("/queries/0/looking_for"));

        let mut mixed: serde_json::Value = serde_json::from_slice(&relational_request()).unwrap();
        mixed["queries"][4]["inputs"][0]["select"] = serde_json::json!("entities");
        mixed["queries"][4]["inputs"][0]["results_of"] = serde_json::json!("entities-a");
        let mixed = validate_request(&canonicalize_value(&mixed).unwrap()).unwrap();
        let block = mixed
            .blocks
            .iter()
            .find(|block| block.form == QueryForm::CombineResults)
            .unwrap();
        let query = mixed
            .request
            .queries
            .iter()
            .find(|query| query.form() == QueryForm::CombineResults)
            .unwrap();
        let error =
            compile_combine_template(&ontology_compiler().unwrap(), block, query).unwrap_err();
        assert!(error.to_string().contains("QUERY_IDENTITY_DOMAIN_MISMATCH"));

        let mut uncovered: serde_json::Value =
            serde_json::from_slice(&relational_request()).unwrap();
        uncovered["queries"][0]["where"] = serde_json::json!(["there are no callers"]);
        let error = validate_request(&canonicalize_value(&uncovered).unwrap()).unwrap_err();
        assert!(error.to_string().contains("structural_policy"));
    }

    #[test]
    fn semantic_query_relational_operational_gate() {
        let typed = validate_request(&relational_request()).unwrap();
        assert!(require_registered_executors(&typed).is_ok());
        let registered = QUERY_FORM_VALUES
            .iter()
            .filter_map(|entry| QueryForm::try_from(entry.code).ok())
            .filter(|form| form.executor_registered())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            registered,
            BTreeSet::from([
                QueryForm::FindEntities,
                QueryForm::RetrieveFacts,
                QueryForm::FollowRelationships,
                QueryForm::FindPaths,
                QueryForm::MatchPattern,
                QueryForm::CombineResults,
                QueryForm::SummarizeFacts,
                QueryForm::RetrieveSourceContext,
            ])
        );
        assert!(typed.blocks.iter().all(|block| {
            !block.canonical_order.is_empty()
                && block.cancellation_required
                && block.maximum_memory_bytes > 0
        }));
    }

    #[test]
    fn qry_v13_form_contract_conformance() {
        let parsed = parse_request(&eight_form_request()).unwrap();
        assert_eq!(parsed.request.queries.len(), 8);
        assert_eq!(
            parsed
                .request
                .queries
                .iter()
                .map(SemanticQueryClause::form)
                .collect::<BTreeSet<_>>(),
            QUERY_FORM_VALUES
                .iter()
                .map(|entry| QueryForm::try_from(entry.code).unwrap())
                .collect()
        );
        let mut wrong_variant: serde_json::Value =
            serde_json::from_slice(&eight_form_request()).unwrap();
        wrong_variant["queries"][3]["facts"] = serde_json::json!(["not a path field"]);
        assert!(parse_request(&canonicalize_value(&wrong_variant).unwrap()).is_err());
        let mut missing_required: serde_json::Value =
            serde_json::from_slice(&eight_form_request()).unwrap();
        missing_required["queries"][7]
            .as_object_mut()
            .unwrap()
            .remove("context");
        assert!(parse_request(&canonicalize_value(&missing_required).unwrap()).is_err());
    }

    #[test]
    fn query_form_projection_parity() {
        assert_eq!(QUERY_FORM_CONTRACTS.len(), QUERY_FORM_VALUES.len());
        for (contract, registry) in QUERY_FORM_CONTRACTS.iter().zip(QUERY_FORM_VALUES) {
            assert_eq!(contract.code, registry.code);
            assert_eq!(contract.slug, registry.slug);
            assert!(contract.owner_section >= 13 && contract.owner_section <= 20);
            assert!(!contract.node_kind.is_empty());
            assert!(!contract.canonical_order.is_empty());
        }
        assert_eq!(QUERY_FORM_CONTRACT_ID, "codefabric.query.form-contract");
        assert_eq!(QUERY_FORM_CONTRACT_VERSION, "1.0");
        assert!(QUERY_FORM_CONTRACT_DIGEST.starts_with("b3:"));
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../contracts/schema/cpg-semantic-query-request.schema.json"
        ))
        .unwrap();
        let schema_slugs = schema["properties"]["queries"]["items"]["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|variant| variant["properties"]["request"]["const"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            schema_slugs,
            QUERY_FORM_VALUES
                .iter()
                .map(|entry| entry.slug)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn qry_v13_connecting_path_schema_falsification() {
        let parsed = parse_request(&eight_form_request()).unwrap();
        let SemanticQueryClause::FindPaths {
            starting_from,
            ending_at,
            through,
            path_policy,
            maximum_length,
            ..
        } = &parsed.request.queries[3]
        else {
            panic!("path fixture changed variant");
        };
        assert_eq!(starting_from.len(), 1);
        assert_eq!(ending_at.len(), 1);
        assert_eq!(
            through.iter().map(String::as_str).collect::<Vec<_>>(),
            ["control flow"]
        );
        assert_eq!(path_policy, "one shortest witness path");
        assert_eq!(*maximum_length, Some(4));
        for retired in [
            "retrieve facts",
            "follow relationships",
            "find paths",
            "summarize facts",
            "fetch source context",
        ] {
            let changed = String::from_utf8(eight_form_request())
                .unwrap()
                .replace("find connecting fact paths", retired);
            assert!(
                parse_request(changed.as_bytes()).is_err(),
                "accepted {retired}"
            );
        }
    }

    #[test]
    fn query_form_contract_operational_gate() {
        let typed = validate_request(&eight_form_request()).unwrap();
        require_registered_executors(&typed).unwrap();
        assert_eq!(
            typed
                .blocks
                .iter()
                .filter(|block| block.form.executor_registered())
                .count(),
            8
        );
        assert!(
            typed
                .blocks
                .iter()
                .filter(|block| matches!(
                    block.form,
                    QueryForm::FollowRelationships | QueryForm::FindPaths | QueryForm::MatchPattern
                ))
                .all(|block| block.form.executor_registered())
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
            "\"looking_for\":\"syntax nodes\"",
            "\"looking_for\":\"call targets\"",
        );
        let incompatible = validate_request(incompatible_phrase.as_bytes()).unwrap();
        let block = &incompatible.blocks[0];
        let query = incompatible
            .request
            .queries
            .iter()
            .find(|query| query.query_id() == block.block_id)
            .unwrap();
        assert!(
            resolve_query_phrases(&ontology_compiler().unwrap(), query, &block.source_pointer)
                .is_err()
        );
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
    fn odf_phrase_binding_dual_path_parity() {
        let compiler = ontology_compiler().expect("ontology compiler");
        let operation = compiler
            .phrases
            .iter()
            .find(|(_, operation)| operation.canonical_text == "certainty is exact")
            .map(|(_, operation)| operation)
            .expect("compiled phrase operation");
        let expected = operation
            .operand_codes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(expected, BTreeSet::from([10, 20, 30, 50]));

        let mut value: serde_json::Value = serde_json::from_slice(&request()).unwrap();
        value["queries"][0]["where"] = serde_json::json!([operation.canonical_text]);
        value["queries"][2]["where"] = serde_json::json!([operation.canonical_text]);
        let typed = validate_request(&canonicalize_value(&value).unwrap()).unwrap();
        let relational = typed
            .request
            .queries
            .iter()
            .find(|query| query.form() == QueryForm::FindEntities)
            .unwrap();
        let predicates = condition_predicates(&compiler, relational, "entity").unwrap();
        assert_eq!(predicates.len(), 1);
        let rendered = predicates[0].to_string();
        for code in &expected {
            assert!(rendered.contains(&code.to_string()), "{rendered}");
        }

        let graph = typed
            .request
            .queries
            .iter()
            .find(|query| query.form() == QueryForm::FollowRelationships)
            .unwrap();
        assert_eq!(
            graph_certainty_codes(&compiler, graph)
                .unwrap()
                .into_iter()
                .collect::<BTreeSet<_>>(),
            expected
        );
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
                .map(SemanticQueryClause::form)
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
                r#""looking_for":"syntax nodes""#,
                r#""looking_for":"syntax nodes","where":["entities whose semantic kind is function"]"#,
            )
            .replace(
                "\"response_projection\":null",
                "\"response_projection\":{\"canonical_semantic_identity\":true,\"coverage\":true}",
            );
        let typed = type_request(parse_request(filtered.as_bytes()).unwrap()).unwrap();
        assert_eq!(typed.blocks.len(), 3);
        assert_eq!(typed.blocks[0].form, QueryForm::FindEntities);
        assert_eq!(typed.blocks[0].output_role, ResultRole::Entities);
        assert_eq!(
            typed.blocks[0].canonical_order,
            [
                "source_file_path",
                "span_start",
                "semantic_kind",
                "qualified_name",
                "entity_id"
            ]
        );
        let SemanticQueryClause::FindEntities {
            where_conditions, ..
        } = &typed.request.queries[0]
        else {
            panic!("first query changed variant");
        };
        assert_eq!(
            where_conditions
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["entities whose semantic kind is function"]
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
        let typed = validate_request(&request()).unwrap();
        assert_eq!(typed.execution_order, ["q1", "q2", "q3"]);
        assert_eq!(typed.blocks[0].output_role, ResultRole::Entities);
        assert_eq!(typed.blocks[0].fan_out, 2);
        assert_eq!(typed.blocks[1].input_roles, [ResultRole::Entities]);
        assert_eq!(typed.blocks[1].source_pointer, "/queries/1");
        assert!(typed.blocks.iter().all(|block| {
            block.cancellation_required
                && block.maximum_memory_bytes > 0
                && !block.canonical_order.is_empty()
                && !block.coverage_effects.is_empty()
        }));
    }

    #[test]
    fn wp62_negative_zero_state() {
        let evaluative = String::from_utf8(request())
            .unwrap()
            .replace("\"label\":null", "\"label\":\"safe to refactor\"");
        let error = validate_request(evaluative.as_bytes()).unwrap_err();
        assert!(error.to_string().contains("evaluative intent"));

        let mut cycle: serde_json::Value = serde_json::from_slice(&request()).unwrap();
        cycle["queries"][0]["within"] = serde_json::json!([
            {"results_of":"q2","select":"facts"}
        ]);
        let cycle_error = validate_request(&canonicalize_value(&cycle).unwrap())
            .unwrap_err()
            .to_string();
        assert!(cycle_error.contains("cycle"));
        assert!(cycle_error.contains("q1"));
        assert!(cycle_error.contains("q2"));

        let mut mismatch: serde_json::Value = serde_json::from_slice(&request()).unwrap();
        mismatch["queries"][1]["about"] = serde_json::json!([
            {"results_of":"q1","select":"facts"}
        ]);
        assert!(
            validate_request(&canonicalize_value(&mismatch).unwrap())
                .unwrap_err()
                .to_string()
                .contains("incompatible result role")
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One differential corpus keeps all graph-form witness contracts visible together.
    fn qry_v13_graph_forms_conformance() {
        let typed = validate_request(&eight_form_request()).unwrap();
        assert_eq!(typed.blocks.len(), QUERY_FORM_VALUES.len());
        assert!(
            typed
                .blocks
                .iter()
                .all(|block| block.form.currently_supported())
        );

        let cancellation = crate::cancellation::Cancellation::with_check_interval(1);
        let path_plan = graph_plan(&typed, QueryForm::FindPaths);
        let graph = QueryGraph::build(graph_edges(), path_plan.maximum_memory_bytes).unwrap();
        let path_input = GraphInputs {
            starting: SeedSet {
                entities: BTreeSet::from([[1; 16]]),
                ..SeedSet::default()
            },
            ending: SeedSet {
                entities: BTreeSet::from([[3; 16]]),
                ..SeedSet::default()
            },
            ..GraphInputs::default()
        };
        let paths = execute_graph_operator(&path_plan, &path_input, &graph, &cancellation).unwrap();
        assert_eq!(paths.completeness, CompletenessState::Complete);
        assert_eq!(paths.values.path_ids.len(), 1);
        assert_eq!(paths.batches[0].num_rows(), 1);

        let pattern_plan = graph_plan(&typed, QueryForm::MatchPattern);
        let pattern_input = GraphInputs {
            binding_candidates: BTreeMap::from([(
                "source".to_owned(),
                SeedSet {
                    entities: BTreeSet::from([[2; 16]]),
                    ..SeedSet::default()
                },
            )]),
            ..GraphInputs::default()
        };
        let patterns =
            execute_graph_operator(&pattern_plan, &pattern_input, &graph, &cancellation).unwrap();
        assert_eq!(patterns.values.group_ids.len(), 1);

        let relational = typed
            .blocks
            .iter()
            .find(|block| block.form == QueryForm::CombineResults)
            .unwrap();
        let query = typed
            .request
            .queries
            .iter()
            .find(|query| query.form() == QueryForm::CombineResults)
            .unwrap();
        assert!(graph_operator_plan(&ontology_compiler().unwrap(), relational, query).is_err());

        let mut follow_plan = graph_plan(&typed, QueryForm::FollowRelationships);
        follow_plan.semantics = GraphSemantics::Follow {
            direction: GraphDirection::Outgoing,
            distance: GraphDistance::Direct,
            stop_when: Vec::new(),
        };
        follow_plan.maximum_depth = 1;
        let parallel =
            QueryGraph::build(parallel_graph_edges(), follow_plan.maximum_memory_bytes).unwrap();
        let follow = execute_graph_operator(
            &follow_plan,
            &GraphInputs {
                starting: SeedSet {
                    entities: BTreeSet::from([[1; 16]]),
                    ..SeedSet::default()
                },
                ..GraphInputs::default()
            },
            &parallel,
            &cancellation,
        )
        .unwrap();
        assert_eq!(follow.values.fact_ids.len(), 2);
        assert_eq!(follow.coverage["parallel_edges"], 1);

        follow_plan.semantics = GraphSemantics::Follow {
            direction: GraphDirection::Incoming,
            distance: GraphDistance::Direct,
            stop_when: Vec::new(),
        };
        let incoming = execute_graph_operator(
            &follow_plan,
            &GraphInputs {
                starting: SeedSet {
                    entities: BTreeSet::from([[2; 16]]),
                    ..SeedSet::default()
                },
                ..GraphInputs::default()
            },
            &parallel,
            &cancellation,
        )
        .unwrap();
        assert_eq!(incoming.values.fact_ids.len(), 2);

        let mut all_shortest = path_plan.clone();
        all_shortest.semantics = GraphSemantics::Paths {
            direction: GraphDirection::Outgoing,
            path_policy: PathPolicy::AllShortest,
            through_codes: vec![10],
        };
        let all_paths =
            execute_graph_operator(&all_shortest, &path_input, &parallel, &cancellation).unwrap();
        let expected = reference_simple_path_count(
            &parallel_graph_edges(),
            [1; 16],
            [3; 16],
            2,
            &mut BTreeSet::from([[1; 16]]),
        );
        assert_eq!(all_paths.values.path_ids.len(), expected);
        assert_eq!(expected, 2);
        let fact_witnesses = all_paths.batches[0]
            .column_by_name("ordered_fact_ids")
            .unwrap()
            .as_any()
            .downcast_ref::<ListArray>()
            .unwrap();
        assert!((0..fact_witnesses.len()).all(|row| fact_witnesses.value_length(row) == 2));
        assert_eq!(fact_witnesses.value_type(), DataType::FixedSizeBinary(16));

        let mut conjunctive = pattern_plan;
        conjunctive.semantics = GraphSemantics::Pattern {
            bindings: ["source", "middle", "target"]
                .into_iter()
                .map(|name| PatternBinding {
                    name: name.to_owned(),
                    looking_for: "syntax nodes".to_owned(),
                    within: None,
                    where_conditions: Vec::new(),
                })
                .collect(),
            relationships: vec![
                PatternRelationship {
                    from: "source".to_owned(),
                    to: "middle".to_owned(),
                    relationship: "call targets".to_owned(),
                    direction: Some("outgoing".to_owned()),
                    distance: Some("one relationship step".to_owned()),
                },
                PatternRelationship {
                    from: "middle".to_owned(),
                    to: "target".to_owned(),
                    relationship: "call targets".to_owned(),
                    direction: Some("outgoing".to_owned()),
                    distance: Some("one relationship step".to_owned()),
                },
            ],
        };
        let candidates = GraphInputs {
            binding_candidates: BTreeMap::from([
                (
                    "source".to_owned(),
                    SeedSet {
                        entities: BTreeSet::from([[1; 16]]),
                        ..SeedSet::default()
                    },
                ),
                (
                    "middle".to_owned(),
                    SeedSet {
                        entities: BTreeSet::from([[2; 16]]),
                        ..SeedSet::default()
                    },
                ),
                (
                    "target".to_owned(),
                    SeedSet {
                        entities: BTreeSet::from([[3; 16]]),
                        ..SeedSet::default()
                    },
                ),
            ]),
            ..GraphInputs::default()
        };
        let matched =
            execute_graph_operator(&conjunctive, &candidates, &parallel, &cancellation).unwrap();
        assert_eq!(matched.values.group_ids.len(), 1);
    }

    #[test]
    fn semantic_query_mixed_dag_contract() {
        let typed = validate_request(&eight_form_request()).unwrap();
        let forms = typed
            .blocks
            .iter()
            .map(|block| block.form)
            .collect::<BTreeSet<_>>();
        assert_eq!(forms.len(), QUERY_FORM_VALUES.len());
        for block in &typed.blocks {
            let query = typed
                .request
                .queries
                .iter()
                .find(|query| query.query_id() == block.block_id)
                .unwrap();
            assert_eq!(block.output_role, block.form.output_role());
            assert_eq!(block.canonical_order, canonical_order(block.form));
            assert!(block.cancellation_required);
            if matches!(
                block.form,
                QueryForm::FindEntities
                    | QueryForm::RetrieveFacts
                    | QueryForm::CombineResults
                    | QueryForm::SummarizeFacts
                    | QueryForm::RetrieveSourceContext
            ) {
                assert!(graph_operator_plan(&ontology_compiler().unwrap(), block, query).is_err());
            } else {
                let compiler = ontology_compiler().unwrap();
                let mut bound_block = block.clone();
                bound_block.resolved_phrases =
                    resolve_query_phrases(&compiler, query, &bound_block.source_pointer).unwrap();
                let plan = graph_operator_plan(&compiler, &bound_block, query).unwrap();
                assert_eq!(plan.form, block.form);
                assert_eq!(plan.output_role, block.output_role);
                assert_eq!(plan.canonical_order, block.canonical_order);
                assert!(plan.output_schema.fields().len() >= 6);
                assert!(plan.cancellation_required);
                assert!(plan.maximum_memory_bytes > 0);
            }
        }
        require_registered_executors(&typed).unwrap();
        assert!(
            typed
                .blocks
                .iter()
                .all(|block| block.form.executor_registered())
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One adversarial corpus covers topology, bounds, cancellation, and identity failures.
    fn semantic_query_graph_adversarial_conformance() {
        let mut cycle: serde_json::Value = serde_json::from_slice(&eight_form_request()).unwrap();
        cycle["queries"][0]["within"] = serde_json::json!([
            {"results_of":"properties","select":"facts"}
        ]);
        assert!(
            validate_request(&canonicalize_value(&cycle).unwrap())
                .unwrap_err()
                .to_string()
                .contains("cycle")
        );

        let mut mismatch: serde_json::Value =
            serde_json::from_slice(&eight_form_request()).unwrap();
        mismatch["queries"][3]["starting_from"] = serde_json::json!([
            {"results_of":"properties","select":"facts"}
        ]);
        assert!(
            validate_request(&canonicalize_value(&mismatch).unwrap())
                .unwrap_err()
                .to_string()
                .contains("incompatible result role")
        );

        let typed = validate_request(&eight_form_request()).unwrap();
        let cancelled = crate::cancellation::Cancellation::with_check_interval(1);
        cancelled.cancel();
        let pattern_plan = graph_plan(&typed, QueryForm::MatchPattern);
        let graph = QueryGraph::build(graph_edges(), pattern_plan.maximum_memory_bytes).unwrap();
        assert!(
            execute_graph_operator(&pattern_plan, &GraphInputs::default(), &graph, &cancelled,)
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );

        let cancellation = crate::cancellation::Cancellation::with_check_interval(1);
        let empty_graph = QueryGraph::build(Vec::new(), pattern_plan.maximum_memory_bytes).unwrap();
        let empty = execute_graph_operator(
            &pattern_plan,
            &GraphInputs::default(),
            &empty_graph,
            &cancellation,
        )
        .unwrap();
        assert_eq!(empty.completeness, CompletenessState::Indeterminate);
        assert_eq!(empty.coverage["negative_proof_available"], 0);

        let mut bounded = graph_plan(&typed, QueryForm::FindPaths);
        bounded.maximum_depth = 1;
        let bounded_graph = QueryGraph::build(graph_edges(), bounded.maximum_memory_bytes).unwrap();
        let overflow = execute_graph_operator(
            &bounded,
            &GraphInputs {
                starting: SeedSet {
                    entities: BTreeSet::from([[1; 16]]),
                    ..SeedSet::default()
                },
                ending: SeedSet {
                    entities: BTreeSet::from([[3; 16]]),
                    ..SeedSet::default()
                },
                ..GraphInputs::default()
            },
            &bounded_graph,
            &cancellation,
        )
        .unwrap();
        assert_eq!(overflow.completeness, CompletenessState::Indeterminate);
        assert_eq!(overflow.coverage["depth_bound_reached"], 1);

        bounded.maximum_memory_bytes = 1;
        assert!(
            QueryGraph::build(graph_edges(), bounded.maximum_memory_bytes)
                .unwrap_err()
                .to_string()
                .contains("memory bound")
        );

        let mut conflicting = graph_edges();
        let mut duplicate = conflicting[0];
        duplicate.target_id = [9; 16];
        conflicting.push(duplicate);
        assert!(
            QueryGraph::build(conflicting, bounded_graph.edges.len() * 4096)
                .unwrap_err()
                .to_string()
                .contains("conflicting relation rows")
        );

        let missing = execute_graph_operator(
            &bounded,
            &GraphInputs {
                starting: SeedSet {
                    entities: BTreeSet::from([[8; 16]]),
                    ..SeedSet::default()
                },
                ending: SeedSet {
                    entities: BTreeSet::from([[9; 16]]),
                    ..SeedSet::default()
                },
                ..GraphInputs::default()
            },
            &bounded_graph,
            &cancellation,
        )
        .unwrap();
        assert!(missing.values.path_ids.is_empty());
        assert_eq!(missing.completeness, CompletenessState::Indeterminate);

        let mut unbounded: serde_json::Value =
            serde_json::from_slice(&eight_form_request()).unwrap();
        unbounded["queries"][3]["path_policy"] =
            serde_json::json!("all simple paths up to the maximum length");
        unbounded["queries"][3]
            .as_object_mut()
            .unwrap()
            .remove("maximum_length");
        let unbounded = validate_request(&canonicalize_value(&unbounded).unwrap()).unwrap();
        let block = unbounded
            .blocks
            .iter()
            .find(|block| block.form == QueryForm::FindPaths)
            .unwrap();
        let query = unbounded
            .request
            .queries
            .iter()
            .find(|query| query.form() == QueryForm::FindPaths)
            .unwrap();
        assert!(
            graph_operator_plan(&ontology_compiler().unwrap(), block, query)
                .unwrap_err()
                .to_string()
                .contains("requires an explicit maximum_length")
        );
    }

    #[test]
    fn semantic_query_graph_operational_gate() {
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
        let input = GraphInputs {
            starting: SeedSet {
                entities: BTreeSet::from([[1; 16]]),
                ..SeedSet::default()
            },
            ending: SeedSet {
                entities: BTreeSet::from([[3; 16]]),
                ..SeedSet::default()
            },
            ..GraphInputs::default()
        };
        let cancellation = crate::cancellation::Cancellation::with_check_interval(1);
        let first_graph = QueryGraph::build(graph_edges(), plan.maximum_memory_bytes).unwrap();
        let first = execute_graph_operator(&plan, &input, &first_graph, &cancellation).unwrap();
        let second_graph = QueryGraph::build(
            graph_edges().into_iter().rev().collect(),
            plan.maximum_memory_bytes,
        )
        .unwrap();
        let second = execute_graph_operator(&plan, &input, &second_graph, &cancellation).unwrap();
        assert_eq!(first.values.path_ids, second.values.path_ids);
        assert_eq!(
            crate::fabric::batch_checksum(&first.batches[0]).unwrap(),
            crate::fabric::batch_checksum(&second.batches[0]).unwrap()
        );
    }
}
