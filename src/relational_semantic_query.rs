//! Programmatic compilation of the released semantic request envelope into relational programs.
//!
//! The eight released form labels are compatibility values only. One generic compiler joins
//! request rows to catalog-carried form, role, clause, operator, schema, and fact-family rows. It
//! never selects an executor from a form label, exposes a physical catalog name, or falls back
//! from unavailable semantics to syntax or names.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::num::NonZeroUsize;
use std::sync::Arc;

use datafusion::common::ScalarValue;

use crate::fabric::arrow_result_resource::{
    ArrowResultResourceError, ResultCompleteness, ResultCoverage, ResultUnknownCause,
};
use crate::fabric::relational_query_runtime::SelectedQueryOutput;
use crate::identity::{
    CanonicalPublicIdentity, IdentityDomain, decode_public_id, decode_public_id_any_kind,
    derive_public_recipe_identity,
};
use crate::identity_recipes::{self as recipes, RecipeValue};
use crate::relational_program::{
    AggregateExpression, AggregateOperator, FieldId, JoinKind, NamedAggregateExpression,
    NamedExpression, RelationId, RelationalExpression, RelationalProgram, ScalarExpression,
    ScalarOperator, SortExpression, UnionKind,
};

pub const RELATIONAL_SEMANTIC_QUERY_PROGRAM_RELEASE: &str =
    "codefabric.relational-semantic-query.datafusion-55.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectiveInputCoverageState {
    Complete,
    Partial,
    Indeterminate,
    Unavailable,
}

impl ObjectiveInputCoverageState {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Indeterminate => "indeterminate",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectiveInputSetIdentityInput {
    pub workspace_id: Arc<str>,
    pub analysis_context_ids: Arc<[Arc<str>]>,
    pub fact_ids: Arc<[Arc<str>]>,
    pub producer_identities: Arc<[Arc<str>]>,
    pub policy_identity: Arc<str>,
    pub coverage_state: ObjectiveInputCoverageState,
}

/// Issue the exact unordered objective-input membership as CBEF-v1 domain 19.
///
/// # Errors
///
/// Rejects empty membership, malformed canonical IDs, invalid producer/policy values, or a
/// recipe mismatch.
pub fn issue_objective_input_set_identity(
    input: &ObjectiveInputSetIdentityInput,
) -> Result<CanonicalPublicIdentity, ObjectiveGroupIdentityError> {
    if input.analysis_context_ids.is_empty()
        || input.fact_ids.is_empty()
        || input.producer_identities.is_empty()
        || !valid_identity_text(&input.policy_identity)
        || input
            .producer_identities
            .iter()
            .any(|value| !valid_identity_text(value))
    {
        return Err(ObjectiveGroupIdentityError::InvalidPreimage);
    }
    let workspace_id = decode_public_id(IdentityDomain::Workspace, None, &input.workspace_id)
        .map_err(|_| ObjectiveGroupIdentityError::InvalidPreimage)?;
    let mut contexts = input
        .analysis_context_ids
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    contexts.sort();
    contexts.dedup();
    let context_values = contexts
        .iter()
        .map(|value| {
            decode_public_id(IdentityDomain::AnalysisContext, None, value)
                .map(RecipeValue::Id)
                .map_err(|_| ObjectiveGroupIdentityError::InvalidPreimage)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut facts = input.fact_ids.iter().cloned().collect::<Vec<_>>();
    facts.sort();
    facts.dedup();
    let fact_values = facts
        .iter()
        .map(|value| {
            decode_public_id_any_kind(IdentityDomain::RelationFact, value)
                .map(RecipeValue::Id)
                .map_err(|_| ObjectiveGroupIdentityError::InvalidPreimage)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut producers = input
        .producer_identities
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    producers.sort();
    producers.dedup();
    let coverage = input.coverage_state.wire_name();
    let record = recipes::objective_input_set(recipes::ObjectiveInputSetFields {
        workspace_id: RecipeValue::Id(workspace_id),
        analysis_context_ids: RecipeValue::Set(context_values),
        fact_ids: RecipeValue::Set(fact_values),
        producer_identities: RecipeValue::Set(
            producers
                .iter()
                .map(|value| RecipeValue::Utf8(value.to_string()))
                .collect(),
        ),
        policy_identity: RecipeValue::Utf8(input.policy_identity.to_string()),
        coverage_state: RecipeValue::Utf8(coverage.to_owned()),
    })
    .map_err(|_| ObjectiveGroupIdentityError::CanonicalIdentity)?;
    derive_public_recipe_identity(
        record,
        vec![
            ("workspace_id", serde_json::json!(input.workspace_id)),
            ("analysis_context_ids", serde_json::json!(contexts)),
            ("fact_ids", serde_json::json!(facts)),
            ("producer_identities", serde_json::json!(producers)),
            ("policy_identity", serde_json::json!(input.policy_identity)),
            ("coverage_state", serde_json::json!(coverage)),
        ],
        &[
            "fact ordering",
            "support ids",
            "mutable coverage counters",
            "diagnostic evidence",
        ],
    )
    .map_err(|_| ObjectiveGroupIdentityError::CanonicalIdentity)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectiveGroupScalar {
    Text(Arc<str>),
    Unsigned(u64),
    Signed(i64),
    Boolean(bool),
}

impl ObjectiveGroupScalar {
    fn recipe_value(&self) -> RecipeValue {
        match self {
            Self::Text(value) => {
                RecipeValue::TaggedUnion(2, Box::new(RecipeValue::Utf8(value.to_string())))
            }
            Self::Unsigned(value) => RecipeValue::TaggedUnion(
                4,
                Box::new(RecipeValue::Unsigned(value.to_be_bytes().to_vec())),
            ),
            Self::Signed(value) => RecipeValue::TaggedUnion(
                5,
                Box::new(RecipeValue::Signed(value.to_be_bytes().to_vec())),
            ),
            Self::Boolean(value) => {
                RecipeValue::TaggedUnion(6, Box::new(RecipeValue::Boolean(*value)))
            }
        }
    }

    fn evidence(&self) -> serde_json::Value {
        match self {
            Self::Text(value) => serde_json::json!({
                "variant": 2, "member_type": "UTF8", "value": value,
            }),
            Self::Unsigned(value) => serde_json::json!({
                "variant": 4, "member_type": "UNSIGNED", "value": value,
            }),
            Self::Signed(value) => serde_json::json!({
                "variant": 5, "member_type": "SIGNED", "value": value,
            }),
            Self::Boolean(value) => serde_json::json!({
                "variant": 6, "member_type": "BOOLEAN", "value": value,
            }),
        }
    }
}

/// Immutable grouping inputs for one objective summary result identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectiveGroupIdentityInput {
    pub workspace_id: Arc<str>,
    pub analysis_context_id: Arc<str>,
    pub input_set_id: Arc<str>,
    pub grouping_dimensions: Arc<[Arc<str>]>,
    pub canonical_group_key: BTreeMap<Arc<str>, ObjectiveGroupScalar>,
    pub aggregate_function: Arc<str>,
    pub measure: Arc<str>,
    pub producer_id: Arc<str>,
}

/// Fail-closed objective-group identity issuance error.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ObjectiveGroupIdentityError {
    #[error("objective-group identity preimage contains an invalid bounded value")]
    InvalidPreimage,
    #[error("objective-group canonical identity derivation failed")]
    CanonicalIdentity,
}

/// Issue the canonical objective-group identity before projecting mutable group observations.
///
/// # Errors
///
/// Rejects empty, padded, or unbounded inputs and malformed typed CBEF fields.
pub fn issue_objective_group_identity(
    input: &ObjectiveGroupIdentityInput,
) -> Result<CanonicalPublicIdentity, ObjectiveGroupIdentityError> {
    let values = [
        input.workspace_id.as_ref(),
        input.analysis_context_id.as_ref(),
        input.input_set_id.as_ref(),
    ]
    .into_iter()
    .chain(input.grouping_dimensions.iter().map(AsRef::as_ref))
    .chain(
        input
            .canonical_group_key
            .iter()
            .map(|(key, _)| key.as_ref()),
    )
    .chain([
        input.aggregate_function.as_ref(),
        input.measure.as_ref(),
        input.producer_id.as_ref(),
    ]);
    if values.into_iter().any(|value| !valid_identity_text(value))
        || input.grouping_dimensions.is_empty()
        || input.canonical_group_key.is_empty()
        || input.canonical_group_key.values().any(
            |value| matches!(value, ObjectiveGroupScalar::Text(text) if !valid_identity_text(text)),
        )
    {
        return Err(ObjectiveGroupIdentityError::InvalidPreimage);
    }
    let aggregate = input.aggregate_function.to_ascii_lowercase();
    if !input.aggregate_function.is_ascii()
        || !matches!(
            aggregate.as_str(),
            "count" | "count_distinct" | "sum" | "average" | "minimum" | "maximum"
        )
    {
        return Err(ObjectiveGroupIdentityError::InvalidPreimage);
    }
    let workspace_id = decode_public_id(IdentityDomain::Workspace, None, &input.workspace_id)
        .map_err(|_| ObjectiveGroupIdentityError::InvalidPreimage)?;
    let analysis_context_id = decode_public_id(
        IdentityDomain::AnalysisContext,
        None,
        &input.analysis_context_id,
    )
    .map_err(|_| ObjectiveGroupIdentityError::InvalidPreimage)?;
    let input_set_id =
        decode_public_id(IdentityDomain::ObjectiveInputSet, None, &input.input_set_id)
            .map_err(|_| ObjectiveGroupIdentityError::InvalidPreimage)?;
    let record = recipes::objective_group(recipes::ObjectiveGroupFields {
        workspace_id: RecipeValue::Id(workspace_id),
        analysis_context_id: RecipeValue::Id(analysis_context_id),
        input_set_id: RecipeValue::Id(input_set_id),
        grouping_dimensions: RecipeValue::OrderedList(
            input
                .grouping_dimensions
                .iter()
                .map(|value| RecipeValue::Utf8(value.to_string()))
                .collect(),
        ),
        canonical_group_key: RecipeValue::Map(
            input
                .canonical_group_key
                .iter()
                .map(|(key, value)| (RecipeValue::Utf8(key.to_string()), value.recipe_value()))
                .collect(),
        ),
        aggregate_function: RecipeValue::Utf8(aggregate.clone()),
        measure: RecipeValue::Utf8(input.measure.to_string()),
        producer_identity: RecipeValue::Utf8(input.producer_id.to_string()),
    })
    .map_err(|_| ObjectiveGroupIdentityError::CanonicalIdentity)?;
    let group_key_evidence = input
        .canonical_group_key
        .iter()
        .map(|(key, value)| (key.to_string(), value.evidence()))
        .collect::<BTreeMap<_, _>>();
    derive_public_recipe_identity(
        record,
        vec![
            ("workspace_id", serde_json::json!(input.workspace_id)),
            (
                "analysis_context_id",
                serde_json::json!(input.analysis_context_id),
            ),
            ("input_set_id", serde_json::json!(input.input_set_id)),
            (
                "grouping_dimensions",
                serde_json::json!(input.grouping_dimensions),
            ),
            ("canonical_group_key", serde_json::json!(group_key_evidence)),
            ("aggregate_function", serde_json::json!(aggregate)),
            ("measure", serde_json::json!(input.measure)),
            ("producer_identity", serde_json::json!(input.producer_id)),
        ],
        &[
            "support_fact_ids",
            "group members",
            "member count",
            "objective count value",
            "mutable coverage counters",
        ],
    )
    .map_err(|_| ObjectiveGroupIdentityError::CanonicalIdentity)
}

fn valid_identity_text(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= 512
        && !value.chars().any(char::is_control)
}

/// Released request-form labels. These are external envelope values, not executor identities.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReleasedSemanticForm {
    FindCodeEntities,
    RetrieveFactsAboutCode,
    FollowCodeRelationships,
    FindConnectingFactPaths,
    MatchCodeFactPattern,
    CombineResultSets,
    SummarizeObjectiveFacts,
    RetrieveSourceAndSyntaxContext,
}

impl ReleasedSemanticForm {
    pub const ALL: [Self; 8] = [
        Self::FindCodeEntities,
        Self::RetrieveFactsAboutCode,
        Self::FollowCodeRelationships,
        Self::FindConnectingFactPaths,
        Self::MatchCodeFactPattern,
        Self::CombineResultSets,
        Self::SummarizeObjectiveFacts,
        Self::RetrieveSourceAndSyntaxContext,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FindCodeEntities => "find code entities",
            Self::RetrieveFactsAboutCode => "retrieve facts about code",
            Self::FollowCodeRelationships => "follow code relationships",
            Self::FindConnectingFactPaths => "find connecting fact paths",
            Self::MatchCodeFactPattern => "match a code fact pattern",
            Self::CombineResultSets => "combine result sets",
            Self::SummarizeObjectiveFacts => "summarize objective facts",
            Self::RetrieveSourceAndSyntaxContext => "retrieve source and syntax context",
        }
    }
}

/// Typed semantic literal supplied by a request-clause relation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticClauseValue {
    Boolean(bool),
    Int64(i64),
    UInt64(u64),
    Text(Arc<str>),
}

impl SemanticClauseValue {
    fn kind(&self) -> SemanticValueKind {
        match self {
            Self::Boolean(_) => SemanticValueKind::Boolean,
            Self::Int64(_) => SemanticValueKind::Int64,
            Self::UInt64(_) => SemanticValueKind::UInt64,
            Self::Text(_) => SemanticValueKind::Text,
        }
    }

    fn scalar(&self) -> ScalarValue {
        match self {
            Self::Boolean(value) => ScalarValue::Boolean(Some(*value)),
            Self::Int64(value) => ScalarValue::Int64(Some(*value)),
            Self::UInt64(value) => ScalarValue::UInt64(Some(*value)),
            Self::Text(value) => ScalarValue::Utf8(Some(value.to_string())),
        }
    }
}

/// Program-catalog clause value kind.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticValueKind {
    Boolean,
    Int64,
    UInt64,
    Text,
}

/// One request-block relation row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticRequestBlockRow {
    pub query_id: Arc<str>,
    pub form: ReleasedSemanticForm,
    pub output_role_id: Arc<str>,
    pub explicit_result_limit: Option<usize>,
}

/// One typed clause-value relation row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticRequestClauseRow {
    pub query_id: Arc<str>,
    pub clause_id: Arc<str>,
    pub value: SemanticClauseValue,
}

/// One prior-result composition edge. Roles remain semantic catalog identities.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticRequestDependencyRow {
    pub producer_query_id: Arc<str>,
    pub producer_role_id: Arc<str>,
    pub consumer_query_id: Arc<str>,
    pub consumer_role_id: Arc<str>,
}

/// Non-zero request compiler bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticRequestLimits {
    max_blocks: NonZeroUsize,
    max_dependencies: NonZeroUsize,
    max_fanout: NonZeroUsize,
    max_fanin: NonZeroUsize,
    max_operator_nodes_per_block: NonZeroUsize,
    max_fields_per_node: NonZeroUsize,
    max_explicit_result_rows: NonZeroUsize,
}

impl SemanticRequestLimits {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        max_blocks: usize,
        max_dependencies: usize,
        max_fanout: usize,
        max_fanin: usize,
        max_operator_nodes_per_block: usize,
        max_fields_per_node: usize,
        max_explicit_result_rows: usize,
    ) -> Result<Self, RelationalSemanticQueryError> {
        Ok(Self {
            max_blocks: nonzero(max_blocks, "max_blocks")?,
            max_dependencies: nonzero(max_dependencies, "max_dependencies")?,
            max_fanout: nonzero(max_fanout, "max_fanout")?,
            max_fanin: nonzero(max_fanin, "max_fanin")?,
            max_operator_nodes_per_block: nonzero(
                max_operator_nodes_per_block,
                "max_operator_nodes_per_block",
            )?,
            max_fields_per_node: nonzero(max_fields_per_node, "max_fields_per_node")?,
            max_explicit_result_rows: nonzero(
                max_explicit_result_rows,
                "max_explicit_result_rows",
            )?,
        })
    }

    #[must_use]
    pub const fn max_blocks(self) -> usize {
        self.max_blocks.get()
    }

    #[must_use]
    pub const fn max_dependencies(self) -> usize {
        self.max_dependencies.get()
    }

    #[must_use]
    pub const fn max_fanout(self) -> usize {
        self.max_fanout.get()
    }

    #[must_use]
    pub const fn max_fanin(self) -> usize {
        self.max_fanin.get()
    }

    #[must_use]
    pub const fn max_operator_nodes_per_block(self) -> usize {
        self.max_operator_nodes_per_block.get()
    }

    #[must_use]
    pub const fn max_fields_per_node(self) -> usize {
        self.max_fields_per_node.get()
    }

    #[must_use]
    pub const fn max_explicit_result_rows(self) -> usize {
        self.max_explicit_result_rows.get()
    }
}

/// Exact epoch/proof/policy pins and typed request relations.
#[derive(Clone, Debug)]
pub struct SemanticRequestRelations {
    pub semantic_request_id: Arc<str>,
    pub program_catalog_pin: [u8; 32],
    pub source_pin: [u8; 32],
    pub policy_pin: [u8; 32],
    pub producer_closure_proof_pin: [u8; 32],
    pub blocks: Vec<SemanticRequestBlockRow>,
    pub clauses: Vec<SemanticRequestClauseRow>,
    pub dependencies: Vec<SemanticRequestDependencyRow>,
    pub limits: SemanticRequestLimits,
}

/// Program authority is exhaustive so provider ownership can be rejected before lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticQueryAuthority {
    ApplicationOwned(Arc<str>),
    ProviderNative(Arc<str>),
}

/// Query compilation is restricted to objective fact semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticQueryClass {
    Fact(Arc<str>),
    Judgment(Arc<str>),
}

/// One form/output-role binding selected by catalog data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramFormBindingRow {
    pub form_label: Arc<str>,
    pub output_role_id: Arc<str>,
    pub root_node_id: Arc<str>,
    pub output_relation_id: RelationId,
    pub output_fields: Vec<FieldId>,
}

/// One semantic consumer role bound to the relation shape expected by a catalog program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramInputRoleBindingRow {
    pub form_label: Arc<str>,
    pub role_id: Arc<str>,
    pub input_relation_id: RelationId,
}

/// One request clause bound to a generic filter node and input field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramClauseBindingRow {
    pub form_label: Arc<str>,
    pub output_role_id: Arc<str>,
    pub clause_id: Arc<str>,
    pub operator_node_id: Arc<str>,
    pub input_field_id: FieldId,
    pub scalar_operator: ScalarOperator,
    pub value_kind: SemanticValueKind,
    pub required: bool,
}

/// One catalog-declared projection field mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramProjectionField {
    pub input_field_id: FieldId,
    pub output_field_id: FieldId,
}

/// One catalog-declared join predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramJoinPredicate {
    pub left_field_id: FieldId,
    pub right_field_id: FieldId,
    pub scalar_operator: ScalarOperator,
}

/// One catalog-declared grouping field mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramGroupField {
    pub input_field_id: FieldId,
    pub output_field_id: FieldId,
}

/// One catalog-declared aggregate output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramAggregateField {
    pub input_field_id: FieldId,
    pub output_field_id: FieldId,
    pub aggregate_operator: AggregateOperator,
}

/// One catalog-declared sort key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramSortField {
    pub input_field_id: FieldId,
    pub ascending: bool,
    pub nulls_first: bool,
}

/// Closed generic operator algebra carried by catalog rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgramRelationalOperator {
    Input {
        relation_id: RelationId,
    },
    Projection {
        fields: Vec<ProgramProjectionField>,
    },
    Filter,
    Join {
        kind: JoinKind,
        predicates: Vec<ProgramJoinPredicate>,
    },
    Union {
        kind: UnionKind,
    },
    Aggregate {
        group_by: Vec<ProgramGroupField>,
        aggregates: Vec<ProgramAggregateField>,
    },
    Sort {
        fields: Vec<ProgramSortField>,
    },
    Limit {
        skip: usize,
    },
}

/// One versioned catalog operator node. Dependencies refer only to node IDs in the same binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramOperatorRow {
    pub form_label: Arc<str>,
    pub output_role_id: Arc<str>,
    pub node_id: Arc<str>,
    pub ordinal: u32,
    pub input_node_ids: Vec<Arc<str>>,
    pub operator: ProgramRelationalOperator,
    pub output_fields: Vec<FieldId>,
}

/// Exact catalog relation field order used for pre-plan schema validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRelationSchemaRow {
    pub relation_id: RelationId,
    pub fields: Vec<FieldId>,
}

/// One accepted fact-family requirement for one form/output role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRequiredFactFamilyRow {
    pub form_label: Arc<str>,
    pub output_role_id: Arc<str>,
    pub family_id: Arc<str>,
}

/// Complete typed semantic-query program catalog.
#[derive(Clone, Debug)]
pub struct SemanticQueryProgramCatalog {
    pub program_catalog_pin: [u8; 32],
    pub program_release_pin: [u8; 32],
    pub authority: SemanticQueryAuthority,
    pub semantic_class: SemanticQueryClass,
    pub forms: Vec<ProgramFormBindingRow>,
    pub input_roles: Vec<ProgramInputRoleBindingRow>,
    pub clauses: Vec<ProgramClauseBindingRow>,
    pub operators: Vec<ProgramOperatorRow>,
    pub relation_schemas: Vec<ProgramRelationSchemaRow>,
    pub required_fact_families: Vec<ProgramRequiredFactFamilyRow>,
}

/// Explicit bounds for the epoch-bound semantic ingress relation families.
///
/// The existing compiler bounds remain embedded rather than duplicated. Additional relation
/// families receive independent bounds so a small block limit cannot conceal an unbounded input,
/// selection, return, or scope relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochBoundSemanticIngressLimits {
    compiler: SemanticRequestLimits,
    max_selection_rows: NonZeroUsize,
    max_return_rows: NonZeroUsize,
    max_scope_rows: NonZeroUsize,
    max_request_input_rows: NonZeroUsize,
    max_fields_per_request_input_row: NonZeroUsize,
}

impl EpochBoundSemanticIngressLimits {
    /// Construct a non-zero bound set.
    ///
    /// # Errors
    ///
    /// Rejects a zero relation-family bound.
    pub fn try_new(
        compiler: SemanticRequestLimits,
        max_selection_rows: usize,
        max_return_rows: usize,
        max_scope_rows: usize,
        max_request_input_rows: usize,
        max_fields_per_request_input_row: usize,
    ) -> Result<Self, EpochBoundSemanticIngressError> {
        Ok(Self {
            compiler,
            max_selection_rows: epoch_nonzero(max_selection_rows, "max_selection_rows")?,
            max_return_rows: epoch_nonzero(max_return_rows, "max_return_rows")?,
            max_scope_rows: epoch_nonzero(max_scope_rows, "max_scope_rows")?,
            max_request_input_rows: epoch_nonzero(
                max_request_input_rows,
                "max_request_input_rows",
            )?,
            max_fields_per_request_input_row: epoch_nonzero(
                max_fields_per_request_input_row,
                "max_fields_per_request_input_row",
            )?,
        })
    }

    #[must_use]
    pub const fn compiler(self) -> SemanticRequestLimits {
        self.compiler
    }

    #[must_use]
    pub const fn max_selection_rows(self) -> usize {
        self.max_selection_rows.get()
    }

    #[must_use]
    pub const fn max_return_rows(self) -> usize {
        self.max_return_rows.get()
    }

    #[must_use]
    pub const fn max_scope_rows(self) -> usize {
        self.max_scope_rows.get()
    }

    #[must_use]
    pub const fn max_request_input_rows(self) -> usize {
        self.max_request_input_rows.get()
    }

    #[must_use]
    pub const fn max_fields_per_request_input_row(self) -> usize {
        self.max_fields_per_request_input_row.get()
    }
}

/// One explicitly selected, pinned program for a request block.
///
/// `compatibility_form` records released envelope meaning. The validator resolves this row only by
/// `program_binding_id`, then verifies the compatibility observation, so the form cannot select an
/// executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundBlockBindingRow {
    pub query_id: Arc<str>,
    pub compatibility_form: ReleasedSemanticForm,
    pub program_binding_id: Arc<str>,
    pub program_binding_pin: [u8; 32],
    pub output_role_id: Arc<str>,
    pub explicit_result_limit: Option<usize>,
}

/// One repeatable, typed semantic selection value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundSelectionRow {
    pub query_id: Arc<str>,
    pub selection_id: Arc<str>,
    pub ordinal: u32,
    pub value: SemanticClauseValue,
}

/// One repeatable, typed return/projection directive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundReturnRow {
    pub query_id: Arc<str>,
    pub return_id: Arc<str>,
    pub ordinal: u32,
    pub value: SemanticClauseValue,
}

/// One request-global scope value normalized under the pinned policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundScopeRow {
    pub scope_id: Arc<str>,
    pub ordinal: u32,
    pub value: SemanticClauseValue,
}

/// One typed value in a request-owned relational tuple.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundRequestInputFieldValue {
    pub field_id: FieldId,
    pub value: SemanticClauseValue,
}

/// One tuple in a request-owned relation.
///
/// Lists become consecutive tuple ordinals and records become fields. No JSON/string encoding is
/// needed to preserve cardinality or structure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundRequestInputRow {
    pub query_id: Arc<str>,
    pub input_id: Arc<str>,
    pub row_id: Arc<str>,
    pub ordinal: u32,
    pub fields: Vec<EpochBoundRequestInputFieldValue>,
}

/// One prior-result edge bound to an explicit consumer slot.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EpochBoundDependencyRow {
    pub producer_query_id: Arc<str>,
    pub producer_role_id: Arc<str>,
    pub consumer_query_id: Arc<str>,
    pub consumer_slot_id: Arc<str>,
    pub consumer_role_id: Arc<str>,
    pub ordinal: u32,
}

/// Complete normalized semantic ingress after epoch/program/policy resolution.
///
/// There is intentionally no conversion from the legacy `ValidatedSemanticRequest`. The
/// epoch-bound semantic program must emit these rows and exact pins directly.
#[derive(Clone, Debug)]
pub struct EpochBoundSemanticIngress {
    pub semantic_request_id: Arc<str>,
    pub request_content_pin: [u8; 32],
    pub fabric_epoch_pin: [u8; 32],
    pub program_catalog_pin: [u8; 32],
    pub source_pin: [u8; 32],
    pub policy_pin: [u8; 32],
    pub producer_closure_proof_pin: [u8; 32],
    pub limits_pin: [u8; 32],
    pub limits: EpochBoundSemanticIngressLimits,
    pub blocks: Vec<EpochBoundBlockBindingRow>,
    pub selections: Vec<EpochBoundSelectionRow>,
    pub returns: Vec<EpochBoundReturnRow>,
    pub scopes: Vec<EpochBoundScopeRow>,
    pub request_inputs: Vec<EpochBoundRequestInputRow>,
    pub dependencies: Vec<EpochBoundDependencyRow>,
    pub dependency_order: Vec<Arc<str>>,
}

/// Catalog-side exact program selection record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundProgramBindingRow {
    pub program_binding_id: Arc<str>,
    pub program_binding_pin: [u8; 32],
    pub compatibility_form: ReleasedSemanticForm,
    pub output_role_id: Arc<str>,
    pub execution_program_pin: [u8; 32],
}

/// Catalog-side semantic role and cardinality for one consumer slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundConsumerSlotBindingRow {
    pub program_binding_id: Arc<str>,
    pub consumer_slot_id: Arc<str>,
    pub consumer_role_id: Arc<str>,
    pub minimum_edges: usize,
    pub maximum_edges: usize,
}

/// Catalog-side selection value contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundSelectionBindingRow {
    pub program_binding_id: Arc<str>,
    pub selection_id: Arc<str>,
    pub value_kind: SemanticValueKind,
    pub minimum_values: usize,
    pub maximum_values: usize,
}

/// Catalog-side return value contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundReturnBindingRow {
    pub program_binding_id: Arc<str>,
    pub return_id: Arc<str>,
    pub value_kind: SemanticValueKind,
    pub minimum_values: usize,
    pub maximum_values: usize,
}

/// Catalog-side scope value contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundScopeBindingRow {
    pub scope_id: Arc<str>,
    pub value_kind: SemanticValueKind,
    pub minimum_values: usize,
    pub maximum_values: usize,
}

/// One field contract for a request-owned relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundRequestInputField {
    pub field_id: FieldId,
    pub value_kind: SemanticValueKind,
    pub required: bool,
}

/// Catalog-side request-owned relation contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundRequestInputBindingRow {
    pub program_binding_id: Arc<str>,
    pub input_id: Arc<str>,
    pub input_relation_id: RelationId,
    pub fields: Vec<EpochBoundRequestInputField>,
    pub minimum_rows: usize,
    pub maximum_rows: usize,
}

/// Authorized epoch projection of all metadata needed to validate semantic ingress.
#[derive(Clone, Debug)]
pub struct EpochBoundSemanticIngressCatalog {
    pub fabric_epoch_pin: [u8; 32],
    pub program_catalog_pin: [u8; 32],
    pub source_pin: [u8; 32],
    pub policy_pin: [u8; 32],
    pub producer_closure_proof_pin: [u8; 32],
    pub limits_pin: [u8; 32],
    pub program_bindings: Vec<EpochBoundProgramBindingRow>,
    pub consumer_slots: Vec<EpochBoundConsumerSlotBindingRow>,
    pub selections: Vec<EpochBoundSelectionBindingRow>,
    pub returns: Vec<EpochBoundReturnBindingRow>,
    pub scopes: Vec<EpochBoundScopeBindingRow>,
    pub request_inputs: Vec<EpochBoundRequestInputBindingRow>,
}

/// Exact row-consumption proof produced by successful ingress validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochBoundIngressConsumption {
    pub blocks: usize,
    pub selections: usize,
    pub returns: usize,
    pub scopes: usize,
    pub request_input_rows: usize,
    pub request_input_fields: usize,
    pub dependencies: usize,
}

/// Materialization-ready request-owned relation after catalog resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedEpochBoundRequestInput {
    pub query_id: Arc<str>,
    pub input_id: Arc<str>,
    pub relation_id: RelationId,
    pub fields: Vec<EpochBoundRequestInputField>,
    pub rows: Vec<EpochBoundRequestInputRow>,
}

/// Successfully validated, canonicalized epoch-bound semantic ingress.
#[derive(Clone, Debug)]
pub struct ValidatedEpochBoundSemanticIngress {
    ingress: EpochBoundSemanticIngress,
    execution_programs: BTreeMap<Arc<str>, [u8; 32]>,
    request_inputs: Vec<ValidatedEpochBoundRequestInput>,
    consumption: EpochBoundIngressConsumption,
}

impl ValidatedEpochBoundSemanticIngress {
    #[must_use]
    pub const fn ingress(&self) -> &EpochBoundSemanticIngress {
        &self.ingress
    }

    #[must_use]
    pub const fn execution_programs(&self) -> &BTreeMap<Arc<str>, [u8; 32]> {
        &self.execution_programs
    }

    #[must_use]
    pub fn request_inputs(&self) -> &[ValidatedEpochBoundRequestInput] {
        &self.request_inputs
    }

    #[must_use]
    pub const fn consumption(&self) -> EpochBoundIngressConsumption {
        self.consumption
    }

    #[must_use]
    pub fn into_inner(self) -> EpochBoundSemanticIngress {
        self.ingress
    }
}

/// Fail-closed epoch-bound ingress validation failures.
#[derive(Debug, thiserror::Error)]
pub enum EpochBoundSemanticIngressError {
    #[error("resource bound {0} must be non-zero")]
    ZeroBound(&'static str),
    #[error("invalid {kind} identity {value:?}")]
    InvalidIdentity { kind: &'static str, value: String },
    #[error("required pin {0} is absent")]
    MissingPin(&'static str),
    #[error("ingress pin {0} differs from the authorized epoch projection")]
    PinMismatch(&'static str),
    #[error("duplicate {kind} key {key}")]
    Duplicate { kind: &'static str, key: String },
    #[error("ingress exceeds {limit}: observed {observed}, maximum {maximum}")]
    Limit {
        limit: &'static str,
        observed: usize,
        maximum: usize,
    },
    #[error("query {query_id} has no program binding {program_binding_id}")]
    MissingProgramBinding {
        query_id: String,
        program_binding_id: String,
    },
    #[error("query {query_id} program binding {program_binding_id} disagrees with {detail}")]
    ProgramBindingMismatch {
        query_id: String,
        program_binding_id: String,
        detail: &'static str,
    },
    #[error("query {query_id} has no {family} binding {binding_id}")]
    MissingBinding {
        query_id: String,
        family: &'static str,
        binding_id: String,
    },
    #[error("scope binding {0} is not declared by the authorized catalog")]
    MissingScopeBinding(String),
    #[error(
        "{family} binding {binding_id} has cardinality {observed}; expected {minimum}..={maximum}"
    )]
    Cardinality {
        family: &'static str,
        binding_id: String,
        observed: usize,
        minimum: usize,
        maximum: usize,
    },
    #[error("{family} binding {binding_id} has non-contiguous or repeated ordinals")]
    Ordinal {
        family: &'static str,
        binding_id: String,
    },
    #[error("request input {input_id} row {row_id} has an invalid field contract")]
    RequestInputField { input_id: String, row_id: String },
    #[error("dependency references unknown query {0}")]
    UnknownDependencyQuery(String),
    #[error("dependency for query {query_id} has an invalid consumer slot {slot_id}")]
    ConsumerSlot { query_id: String, slot_id: String },
    #[error("query dependency graph contains a cycle")]
    DependencyCycle,
    #[error("declared dependency order differs from the proved topology")]
    DependencyOrderMismatch,
}

pub const EPOCH_BOUND_SEMANTIC_QUERY_PROGRAM_RELEASE: &str =
    "codefabric.epoch-bound-semantic-query.datafusion-55.v1";

/// One exact execution program selected by `program_binding_id` and execution pin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundExecutionProgramRow {
    pub program_binding_id: Arc<str>,
    pub execution_program_pin: [u8; 32],
    pub root_node_id: Arc<str>,
    pub output_relation_id: RelationId,
    pub output_fields: Vec<FieldId>,
}

/// One operator node in an epoch-bound execution program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundExecutionOperatorRow {
    pub program_binding_id: Arc<str>,
    pub execution_program_pin: [u8; 32],
    pub node_id: Arc<str>,
    pub ordinal: u32,
    pub input_node_ids: Vec<Arc<str>>,
    pub operator: ProgramRelationalOperator,
    pub output_fields: Vec<FieldId>,
}

/// How repeated producers bound to one consumer slot become one relation input.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EpochBoundConsumerComposition {
    Single,
    Union(UnionKind),
}

/// Execution mapping from a semantic consumer slot to a program input relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundExecutionConsumerSlotRow {
    pub program_binding_id: Arc<str>,
    pub execution_program_pin: [u8; 32],
    pub consumer_slot_id: Arc<str>,
    pub consumer_role_id: Arc<str>,
    pub input_relation_id: RelationId,
    pub composition: EpochBoundConsumerComposition,
}

/// Predicate fold for repeated values of one selection binding.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EpochBoundSelectionFold {
    All,
    Any,
}

/// Data-carried lowering of one selection into a filter node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundExecutionSelectionRow {
    pub program_binding_id: Arc<str>,
    pub execution_program_pin: [u8; 32],
    pub selection_id: Arc<str>,
    pub operator_node_id: Arc<str>,
    pub input_field_id: FieldId,
    pub scalar_operator: ScalarOperator,
    pub fold: EpochBoundSelectionFold,
}

/// Exact return value and operator/field realization inside one selected program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundExecutionReturnRow {
    pub program_binding_id: Arc<str>,
    pub execution_program_pin: [u8; 32],
    pub return_id: Arc<str>,
    pub value: SemanticClauseValue,
    pub realization_node_id: Arc<str>,
    pub realization_field_ids: Vec<FieldId>,
    pub realization_pin: [u8; 32],
}

/// Exact producer-family requirement for one selected program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundExecutionRequiredFamilyRow {
    pub program_binding_id: Arc<str>,
    pub execution_program_pin: [u8; 32],
    pub family_id: Arc<str>,
}

/// Runtime handoff declaration for a request-owned relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundExecutionRequestInputRow {
    pub program_binding_id: Arc<str>,
    pub execution_program_pin: [u8; 32],
    pub input_id: Arc<str>,
    pub input_relation_id: RelationId,
    pub fields: Vec<EpochBoundRequestInputField>,
    pub handoff_pin: [u8; 32],
}

/// Runtime handoff declaration from normalized scope rows to child-authorization input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundExecutionScopeRow {
    pub scope_id: Arc<str>,
    pub authorization_input_id: Arc<str>,
    pub handoff_pin: [u8; 32],
}

/// Program-ID-keyed execution catalog for direct epoch-bound lowering.
#[derive(Clone, Debug)]
pub struct EpochBoundSemanticExecutionCatalog {
    pub fabric_epoch_pin: [u8; 32],
    pub program_catalog_pin: [u8; 32],
    pub source_pin: [u8; 32],
    pub policy_pin: [u8; 32],
    pub producer_closure_proof_pin: [u8; 32],
    pub execution_catalog_pin: [u8; 32],
    pub program_release_pin: [u8; 32],
    pub authority: SemanticQueryAuthority,
    pub semantic_class: SemanticQueryClass,
    pub programs: Vec<EpochBoundExecutionProgramRow>,
    pub operators: Vec<EpochBoundExecutionOperatorRow>,
    pub relation_schemas: Vec<ProgramRelationSchemaRow>,
    pub consumer_slots: Vec<EpochBoundExecutionConsumerSlotRow>,
    pub selections: Vec<EpochBoundExecutionSelectionRow>,
    pub returns: Vec<EpochBoundExecutionReturnRow>,
    pub required_fact_families: Vec<EpochBoundExecutionRequiredFamilyRow>,
    pub request_inputs: Vec<EpochBoundExecutionRequestInputRow>,
    pub scopes: Vec<EpochBoundExecutionScopeRow>,
}

/// One request-owned relation that must be installed as a query-local `RelationInput`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledEpochBoundRequestInputHandoff {
    pub query_id: Arc<str>,
    pub program_binding_id: Arc<str>,
    pub execution_program_pin: [u8; 32],
    pub input_id: Arc<str>,
    pub relation_id: RelationId,
    pub fields: Vec<EpochBoundRequestInputField>,
    pub rows: Vec<EpochBoundRequestInputRow>,
    pub handoff_pin: [u8; 32],
    pub content_pin: [u8; 32],
}

/// One normalized scope relation that must enter child authorization under a declared input ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledEpochBoundScopeHandoff {
    pub scope_id: Arc<str>,
    pub authorization_input_id: Arc<str>,
    pub rows: Vec<EpochBoundScopeRow>,
    pub handoff_pin: [u8; 32],
    pub content_pin: [u8; 32],
}

/// Complete non-discardable runtime handoff accompanying direct compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochBoundSemanticRuntimeHandoff {
    pub fabric_epoch_pin: [u8; 32],
    pub program_catalog_pin: [u8; 32],
    pub policy_pin: [u8; 32],
    pub request_inputs: Vec<CompiledEpochBoundRequestInputHandoff>,
    pub scopes: Vec<CompiledEpochBoundScopeHandoff>,
}

impl EpochBoundSemanticRuntimeHandoff {
    #[must_use]
    pub fn requires_query_local_binding(&self) -> bool {
        !self.request_inputs.is_empty() || !self.scopes.is_empty()
    }
}

/// Direct compiler result. The runtime handoff cannot be discarded through an `into_compiled`
/// convenience: callers receive both parts together.
#[derive(Clone, Debug)]
pub struct CompiledEpochBoundSemanticRequest {
    compiled: CompiledSemanticRequest,
    handoff: EpochBoundSemanticRuntimeHandoff,
}

impl CompiledEpochBoundSemanticRequest {
    #[must_use]
    pub const fn compiled(&self) -> &CompiledSemanticRequest {
        &self.compiled
    }

    #[must_use]
    pub const fn handoff(&self) -> &EpochBoundSemanticRuntimeHandoff {
        &self.handoff
    }

    #[must_use]
    pub fn into_parts(self) -> (CompiledSemanticRequest, EpochBoundSemanticRuntimeHandoff) {
        (self.compiled, self.handoff)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EpochBoundSemanticCompileError {
    #[error("required execution pin {0} is absent")]
    MissingPin(&'static str),
    #[error("execution catalog pin {0} differs from validated ingress")]
    PinMismatch(&'static str),
    #[error("duplicate execution {kind} key {key}")]
    Duplicate { kind: &'static str, key: String },
    #[error("program binding {0} is absent from the execution catalog")]
    MissingProgram(String),
    #[error("program binding {program_binding_id} has an execution pin mismatch")]
    ExecutionProgramPinMismatch { program_binding_id: String },
    #[error("invalid execution node {node}: {detail}")]
    InvalidNode { node: String, detail: String },
    #[error("execution relation/schema mismatch for {program_binding_id}: {detail}")]
    OutputSchema {
        program_binding_id: String,
        detail: String,
    },
    #[error("query {query_id} has no execution mapping for {family} {binding_id}")]
    MissingBinding {
        query_id: String,
        family: &'static str,
        binding_id: String,
    },
    #[error(
        "request-owned relation {input_id} for query {query_id} is not executable without its declared handoff"
    )]
    RequestInputHandoff { query_id: String, input_id: String },
    #[error("producer closure is invalid for family {family}: {detail}")]
    ProducerClosure { family: String, detail: String },
    #[error("request dependency {0} was not compiled before its consumer")]
    DependencyUnavailable(String),
    #[error("integer or resource bound overflow while compiling epoch-bound ingress")]
    BoundOverflow,
    #[error(transparent)]
    ResultCoverage(#[from] ArrowResultResourceError),
}

/// Application-owned runtime producer proof for one accepted family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProducerProof {
    pub producer_id: Arc<str>,
    pub authority_id: Arc<str>,
    pub algorithm_release: Arc<str>,
    pub precision_id: Arc<str>,
    pub input_pin: [u8; 32],
    pub invalidation_pin: [u8; 32],
    pub materialization_pin: [u8; 32],
    pub requested_units: u64,
    pub completed_units: u64,
    pub remainder_units: u64,
    pub unknown_units: u64,
    pub completeness_proof_pin: [u8; 32],
    pub producer_proof_pin: [u8; 32],
}

/// Explicit unsupported remainder for one accepted family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedFamilyRemainder {
    pub remainder_id: Arc<str>,
    pub authority_id: Arc<str>,
    pub reason_id: Arc<str>,
    pub proof_pin: [u8; 32],
}

/// Exactly one closure disposition per family; multiple/zero rows remain validation failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProducerFamilyDisposition {
    RuntimeProducer(RuntimeProducerProof),
    UnsupportedRemainder(UnsupportedFamilyRemainder),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerFamilyClosureRow {
    pub family_id: Arc<str>,
    pub disposition: ProducerFamilyDisposition,
}

/// Executed producer-closure proof projected into application DTOs.
#[derive(Clone, Debug)]
pub struct ProducerClosureProof {
    pub proof_pin: [u8; 32],
    pub application_authority_id: Arc<str>,
    pub families: Vec<ProducerFamilyClosureRow>,
}

/// Explicit non-plan outcome for a request block.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticBlockDisposition {
    Compiled,
    UnsupportedRemainder,
    UnknownProducerClosure,
    NotExecutedDependency,
}

/// Data-carried reason attached to a non-compiled block.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticCompilationIssue {
    pub code: &'static str,
    pub subject_id: Arc<str>,
    pub related_id: Option<Arc<str>>,
}

/// One block's deterministic compiler result.
#[derive(Clone, Debug)]
pub struct CompiledSemanticBlock {
    query_id: Arc<str>,
    form: ReleasedSemanticForm,
    disposition: SemanticBlockDisposition,
    output: Option<SelectedQueryOutput>,
    issues: Vec<SemanticCompilationIssue>,
}

impl CompiledSemanticBlock {
    #[must_use]
    pub const fn query_id(&self) -> &Arc<str> {
        &self.query_id
    }

    #[must_use]
    pub const fn form(&self) -> ReleasedSemanticForm {
        self.form
    }

    #[must_use]
    pub const fn disposition(&self) -> SemanticBlockDisposition {
        self.disposition
    }

    #[must_use]
    pub const fn output(&self) -> Option<&SelectedQueryOutput> {
        self.output.as_ref()
    }

    #[must_use]
    pub fn issues(&self) -> &[SemanticCompilationIssue] {
        &self.issues
    }
}

/// Native relational operators causally selected from catalog rows.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticCompilerOperator {
    Input,
    Projection,
    Filter,
    Join(JoinKind),
    Union(UnionKind),
    Aggregate,
    Sort,
    Limit,
}

/// Exact data dependency observed by successful request compilation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticCompilerDependency {
    EpochBoundRequest([u8; 32]),
    FabricEpoch([u8; 32]),
    ExecutionCatalog([u8; 32]),
    Limits([u8; 32]),
    ProgramCatalog([u8; 32]),
    SourcePin([u8; 32]),
    PolicyPin([u8; 32]),
    ProducerClosureProof([u8; 32]),
    ProgramRelease([u8; 32]),
    Authority(Arc<str>),
    SemanticClass(Arc<str>),
    ProgramBinding {
        program_binding_id: Arc<str>,
        execution_program_pin: [u8; 32],
    },
    FormRole {
        form_label: Arc<str>,
        role_id: Arc<str>,
        binding_pin: [u8; 32],
    },
    RequestBlock {
        query_id: Arc<str>,
        content_pin: [u8; 32],
    },
    CompositionEdge([u8; 32]),
    ConsumerSlot {
        consumer_slot_id: Arc<str>,
        binding_pin: [u8; 32],
    },
    OperatorNode {
        node_id: Arc<str>,
        binding_pin: [u8; 32],
    },
    Clause {
        clause_id: Arc<str>,
        binding_pin: [u8; 32],
    },
    EpochBoundSelection {
        selection_id: Arc<str>,
        binding_pin: [u8; 32],
    },
    ReturnRealization {
        return_id: Arc<str>,
        realization_pin: [u8; 32],
    },
    ScopeHandoff {
        scope_id: Arc<str>,
        handoff_pin: [u8; 32],
        content_pin: [u8; 32],
    },
    RequestInputHandoff {
        input_id: Arc<str>,
        handoff_pin: [u8; 32],
        content_pin: [u8; 32],
    },
    Relation(RelationId),
    Field(FieldId),
    FactFamily(Arc<str>),
    Producer(Arc<str>),
    UnsupportedRemainder(Arc<str>),
}

/// Causal proof of the generic compiler choices used for this request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticCompilerObservation {
    pub program_release: &'static str,
    pub compiler_proof_pin: [u8; 32],
    pub dependencies: BTreeSet<SemanticCompilerDependency>,
    pub operators: BTreeSet<SemanticCompilerOperator>,
    pub dependency_order: Vec<Arc<str>>,
    pub limits: SemanticRequestLimits,
}

/// Complete canonical request compilation.
#[derive(Clone, Debug)]
pub struct CompiledSemanticRequest {
    blocks: Vec<CompiledSemanticBlock>,
    observation: SemanticCompilerObservation,
}

impl CompiledSemanticRequest {
    #[must_use]
    pub fn blocks(&self) -> &[CompiledSemanticBlock] {
        &self.blocks
    }

    #[must_use]
    pub const fn observation(&self) -> &SemanticCompilerObservation {
        &self.observation
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RelationalSemanticQueryError {
    #[error("resource bound {0} must be non-zero")]
    ZeroBound(&'static str),
    #[error("invalid {kind} identity {value:?}")]
    InvalidIdentity { kind: &'static str, value: String },
    #[error("required pin {0} is absent")]
    MissingPin(&'static str),
    #[error("request catalog epoch pin differs from the supplied catalog")]
    ProgramCatalogMismatch,
    #[error("request producer-closure proof pin differs from the supplied closure")]
    ProducerClosurePinMismatch,
    #[error("provider-native authority cannot compile semantic facts: {0}")]
    ProviderNativeAuthority(String),
    #[error("evaluative or judgment semantics are not queryable facts: {0}")]
    JudgmentSemanticClass(String),
    #[error("duplicate {kind} key {key}")]
    DuplicateProgramBinding { kind: &'static str, key: String },
    #[error("request exceeds {limit}: observed {observed}, maximum {maximum}")]
    RequestLimit {
        limit: &'static str,
        observed: usize,
        maximum: usize,
    },
    #[error("query {query_id} has no catalog binding for form {form:?} and role {role}")]
    MissingFormBinding {
        query_id: String,
        form: ReleasedSemanticForm,
        role: String,
    },
    #[error("query {query_id} has an invalid or missing clause {clause_id}")]
    ClauseBinding { query_id: String, clause_id: String },
    #[error("composition references unknown query {0}")]
    UnknownDependencyQuery(String),
    #[error("composition role {role} is not bound for query {query_id}")]
    UnknownCompositionRole { query_id: String, role: String },
    #[error("query dependency graph contains a cycle")]
    QueryDependencyCycle,
    #[error("operator graph for {form}/{role} contains a cycle")]
    OperatorCycle { form: String, role: String },
    #[error("invalid operator node {node}: {detail}")]
    InvalidOperatorNode { node: String, detail: String },
    #[error("output relation/schema mismatch for {form}/{role}: {detail}")]
    OutputSchema {
        form: String,
        role: String,
        detail: String,
    },
    #[error("producer closure is invalid for family {family}: {detail}")]
    ProducerClosure { family: String, detail: String },
    #[error(transparent)]
    ResultCoverage(#[from] ArrowResultResourceError),
}

fn nonzero(value: usize, name: &'static str) -> Result<NonZeroUsize, RelationalSemanticQueryError> {
    NonZeroUsize::new(value).ok_or(RelationalSemanticQueryError::ZeroBound(name))
}

fn validate_identity(kind: &'static str, value: &str) -> Result<(), RelationalSemanticQueryError> {
    if value.is_empty()
        || value.len() > 1_024
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err(RelationalSemanticQueryError::InvalidIdentity {
            kind,
            value: value.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn validate_pin(kind: &'static str, value: [u8; 32]) -> Result<(), RelationalSemanticQueryError> {
    if value == [0; 32] {
        Err(RelationalSemanticQueryError::MissingPin(kind))
    } else {
        Ok(())
    }
}

type FormRoleKey = (Arc<str>, Arc<str>);
type NodeKey = (Arc<str>, Arc<str>, Arc<str>);

struct ValidatedProgramCatalog<'a> {
    forms: BTreeMap<FormRoleKey, &'a ProgramFormBindingRow>,
    input_roles: BTreeMap<(Arc<str>, Arc<str>), &'a ProgramInputRoleBindingRow>,
    clauses: BTreeMap<(Arc<str>, Arc<str>, Arc<str>), &'a ProgramClauseBindingRow>,
    operators: BTreeMap<NodeKey, &'a ProgramOperatorRow>,
    schemas: BTreeMap<RelationId, &'a ProgramRelationSchemaRow>,
    required_families: BTreeMap<FormRoleKey, BTreeSet<Arc<str>>>,
    authority_id: Arc<str>,
    semantic_class_id: Arc<str>,
}

struct ValidatedRequest<'a> {
    blocks: BTreeMap<Arc<str>, &'a SemanticRequestBlockRow>,
    clauses: BTreeMap<(Arc<str>, Arc<str>), &'a SemanticRequestClauseRow>,
    incoming: BTreeMap<Arc<str>, Vec<&'a SemanticRequestDependencyRow>>,
    order: Vec<Arc<str>>,
}

enum ClosureStatus<'a> {
    Runtime(&'a RuntimeProducerProof),
    Remainder(&'a UnsupportedFamilyRemainder),
}

fn validate_program_catalog<'a>(
    catalog: &'a SemanticQueryProgramCatalog,
    limits: SemanticRequestLimits,
) -> Result<ValidatedProgramCatalog<'a>, RelationalSemanticQueryError> {
    validate_pin("program_catalog_pin", catalog.program_catalog_pin)?;
    validate_pin("program_release_pin", catalog.program_release_pin)?;
    let authority_id = match &catalog.authority {
        SemanticQueryAuthority::ApplicationOwned(value) => {
            validate_identity("application authority", value)?;
            Arc::clone(value)
        }
        SemanticQueryAuthority::ProviderNative(value) => {
            return Err(RelationalSemanticQueryError::ProviderNativeAuthority(
                value.to_string(),
            ));
        }
    };
    let semantic_class_id = match &catalog.semantic_class {
        SemanticQueryClass::Fact(value) => {
            validate_identity("fact semantic class", value)?;
            Arc::clone(value)
        }
        SemanticQueryClass::Judgment(value) => {
            return Err(RelationalSemanticQueryError::JudgmentSemanticClass(
                value.to_string(),
            ));
        }
    };

    let mut schemas = BTreeMap::new();
    for row in &catalog.relation_schemas {
        validate_fields("relation schema", &row.fields, limits)?;
        if schemas.insert(row.relation_id.clone(), row).is_some() {
            return Err(duplicate("relation schema", row.relation_id.as_str()));
        }
    }

    let mut forms = BTreeMap::new();
    for row in &catalog.forms {
        for (kind, value) in [
            ("form label", row.form_label.as_ref()),
            ("output role", row.output_role_id.as_ref()),
            ("root node", row.root_node_id.as_ref()),
        ] {
            validate_identity(kind, value)?;
        }
        validate_fields("form output", &row.output_fields, limits)?;
        let key = (Arc::clone(&row.form_label), Arc::clone(&row.output_role_id));
        if forms.insert(key, row).is_some() {
            return Err(duplicate(
                "form/output role",
                &format!("{}/{}", row.form_label, row.output_role_id),
            ));
        }
    }

    let mut input_roles = BTreeMap::new();
    for row in &catalog.input_roles {
        validate_identity("form label", &row.form_label)?;
        validate_identity("input role", &row.role_id)?;
        let key = (Arc::clone(&row.form_label), Arc::clone(&row.role_id));
        if input_roles.insert(key, row).is_some() {
            return Err(duplicate(
                "form/input role",
                &format!("{}/{}", row.form_label, row.role_id),
            ));
        }
        if !schemas.contains_key(&row.input_relation_id) {
            return Err(RelationalSemanticQueryError::OutputSchema {
                form: row.form_label.to_string(),
                role: row.role_id.to_string(),
                detail: format!(
                    "input role references undeclared relation {}",
                    row.input_relation_id.as_str()
                ),
            });
        }
    }

    let mut operators = BTreeMap::new();
    for row in &catalog.operators {
        for (kind, value) in [
            ("form label", row.form_label.as_ref()),
            ("output role", row.output_role_id.as_ref()),
            ("operator node", row.node_id.as_ref()),
        ] {
            validate_identity(kind, value)?;
        }
        validate_fields("operator output", &row.output_fields, limits)?;
        let mut inputs = BTreeSet::new();
        for input in &row.input_node_ids {
            validate_identity("operator input node", input)?;
            if !inputs.insert(input.as_ref()) {
                return Err(invalid_node(&row.node_id, "input node is repeated"));
            }
        }
        validate_operator_arity(row)?;
        let key = (
            Arc::clone(&row.form_label),
            Arc::clone(&row.output_role_id),
            Arc::clone(&row.node_id),
        );
        if operators.insert(key, row).is_some() {
            return Err(duplicate(
                "operator node",
                &format!("{}/{}/{}", row.form_label, row.output_role_id, row.node_id),
            ));
        }
    }

    let mut clauses = BTreeMap::new();
    for row in &catalog.clauses {
        for (kind, value) in [
            ("form label", row.form_label.as_ref()),
            ("output role", row.output_role_id.as_ref()),
            ("clause", row.clause_id.as_ref()),
            ("clause operator node", row.operator_node_id.as_ref()),
        ] {
            validate_identity(kind, value)?;
        }
        if !is_binary_predicate(row.scalar_operator) {
            return Err(invalid_node(
                &row.operator_node_id,
                "clause binding requires a binary predicate operator",
            ));
        }
        let key = (
            Arc::clone(&row.form_label),
            Arc::clone(&row.output_role_id),
            Arc::clone(&row.clause_id),
        );
        if clauses.insert(key, row).is_some() {
            return Err(duplicate(
                "clause binding",
                &format!(
                    "{}/{}/{}",
                    row.form_label, row.output_role_id, row.clause_id
                ),
            ));
        }
    }

    let mut required_families: BTreeMap<FormRoleKey, BTreeSet<Arc<str>>> = BTreeMap::new();
    for row in &catalog.required_fact_families {
        validate_identity("form label", &row.form_label)?;
        validate_identity("output role", &row.output_role_id)?;
        validate_identity("fact family", &row.family_id)?;
        let inserted = required_families
            .entry((Arc::clone(&row.form_label), Arc::clone(&row.output_role_id)))
            .or_default()
            .insert(Arc::clone(&row.family_id));
        if !inserted {
            return Err(duplicate(
                "required fact family",
                &format!(
                    "{}/{}/{}",
                    row.form_label, row.output_role_id, row.family_id
                ),
            ));
        }
    }

    let validated = ValidatedProgramCatalog {
        forms,
        input_roles,
        clauses,
        operators,
        schemas,
        required_families,
        authority_id,
        semantic_class_id,
    };
    for binding in validated.forms.values() {
        validate_form_program(binding, &validated, limits)?;
    }
    Ok(validated)
}

fn validate_fields(
    context: &'static str,
    fields: &[FieldId],
    limits: SemanticRequestLimits,
) -> Result<(), RelationalSemanticQueryError> {
    if fields.is_empty() {
        return Err(RelationalSemanticQueryError::InvalidOperatorNode {
            node: context.to_owned(),
            detail: "field contract is empty".to_owned(),
        });
    }
    if fields.len() > limits.max_fields_per_node() {
        return Err(RelationalSemanticQueryError::RequestLimit {
            limit: "max_fields_per_node",
            observed: fields.len(),
            maximum: limits.max_fields_per_node(),
        });
    }
    let mut unique = BTreeSet::new();
    for field in fields {
        if !unique.insert(field) {
            return Err(RelationalSemanticQueryError::InvalidOperatorNode {
                node: context.to_owned(),
                detail: format!("field {} is repeated", field.as_str()),
            });
        }
    }
    Ok(())
}

fn validate_operator_arity(row: &ProgramOperatorRow) -> Result<(), RelationalSemanticQueryError> {
    let valid = match &row.operator {
        ProgramRelationalOperator::Input { .. } => row.input_node_ids.is_empty(),
        ProgramRelationalOperator::Projection { .. }
        | ProgramRelationalOperator::Filter
        | ProgramRelationalOperator::Aggregate { .. }
        | ProgramRelationalOperator::Sort { .. }
        | ProgramRelationalOperator::Limit { .. } => row.input_node_ids.len() == 1,
        ProgramRelationalOperator::Join { .. } => row.input_node_ids.len() == 2,
        ProgramRelationalOperator::Union { .. } => !row.input_node_ids.is_empty(),
    };
    if valid {
        Ok(())
    } else {
        Err(invalid_node(
            &row.node_id,
            "operator input arity is invalid",
        ))
    }
}

fn validate_form_program(
    binding: &ProgramFormBindingRow,
    catalog: &ValidatedProgramCatalog<'_>,
    limits: SemanticRequestLimits,
) -> Result<(), RelationalSemanticQueryError> {
    let prefix = (binding.form_label.as_ref(), binding.output_role_id.as_ref());
    let mut nodes = catalog
        .operators
        .iter()
        .filter(|((form, role, _), _)| form.as_ref() == prefix.0 && role.as_ref() == prefix.1)
        .map(|((_, _, node), row)| (Arc::clone(node), *row))
        .collect::<BTreeMap<_, _>>();
    if nodes.is_empty() || !nodes.contains_key(binding.root_node_id.as_ref()) {
        return Err(RelationalSemanticQueryError::OutputSchema {
            form: binding.form_label.to_string(),
            role: binding.output_role_id.to_string(),
            detail: "root operator node is missing".to_owned(),
        });
    }
    if nodes.len() > limits.max_operator_nodes_per_block() {
        return Err(RelationalSemanticQueryError::RequestLimit {
            limit: "max_operator_nodes_per_block",
            observed: nodes.len(),
            maximum: limits.max_operator_nodes_per_block(),
        });
    }
    let mut ordinals = BTreeSet::new();
    for row in nodes.values() {
        if !ordinals.insert(row.ordinal) {
            return Err(invalid_node(&row.node_id, "operator ordinal is repeated"));
        }
        for input in &row.input_node_ids {
            let dependency = nodes
                .get(input.as_ref())
                .ok_or_else(|| invalid_node(&row.node_id, "input node is unresolved"))?;
            if dependency.ordinal >= row.ordinal {
                return Err(invalid_node(
                    &row.node_id,
                    "input must precede its consumer ordinal",
                ));
            }
        }
        validate_node_fields(row, &nodes, catalog)?;
    }
    let reachable = reachable_operator_nodes(&nodes, &binding.root_node_id)?;
    if reachable.len() != nodes.len() {
        return Err(invalid_node(
            &binding.root_node_id,
            "binding contains nodes that do not contribute to the root",
        ));
    }
    let root = nodes
        .remove(binding.root_node_id.as_ref())
        .expect("root presence checked");
    if root.output_fields != binding.output_fields {
        return Err(output_schema_error(
            binding,
            "root fields differ from form output fields",
        ));
    }
    let schema = catalog
        .schemas
        .get(&binding.output_relation_id)
        .ok_or_else(|| output_schema_error(binding, "output relation schema is absent"))?;
    if schema.fields != binding.output_fields {
        return Err(output_schema_error(
            binding,
            "catalog relation schema differs from form output fields",
        ));
    }

    for clause in catalog.clauses.values().filter(|row| {
        row.form_label == binding.form_label && row.output_role_id == binding.output_role_id
    }) {
        let node = nodes
            .get(clause.operator_node_id.as_ref())
            .or_else(|| (clause.operator_node_id == root.node_id).then_some(&root))
            .ok_or_else(|| invalid_node(&clause.operator_node_id, "clause node is unresolved"))?;
        if !matches!(node.operator, ProgramRelationalOperator::Filter) {
            return Err(invalid_node(
                &clause.operator_node_id,
                "clause is not attached to a filter node",
            ));
        }
        let input = &nodes[&node.input_node_ids[0]];
        if !input.output_fields.contains(&clause.input_field_id) {
            return Err(invalid_node(
                &clause.operator_node_id,
                "clause field is absent from filter input",
            ));
        }
    }
    Ok(())
}

fn validate_node_fields(
    row: &ProgramOperatorRow,
    nodes: &BTreeMap<Arc<str>, &ProgramOperatorRow>,
    catalog: &ValidatedProgramCatalog<'_>,
) -> Result<(), RelationalSemanticQueryError> {
    let input = |ordinal: usize| -> &ProgramOperatorRow { nodes[&row.input_node_ids[ordinal]] };
    match &row.operator {
        ProgramRelationalOperator::Input { relation_id } => {
            let schema = catalog
                .schemas
                .get(relation_id)
                .ok_or_else(|| invalid_node(&row.node_id, "input relation schema is unresolved"))?;
            if schema.fields != row.output_fields {
                return Err(invalid_node(
                    &row.node_id,
                    "input node fields differ from relation schema",
                ));
            }
        }
        ProgramRelationalOperator::Projection { fields } => {
            let source = input(0);
            let outputs = fields
                .iter()
                .map(|field| field.output_field_id.clone())
                .collect::<Vec<_>>();
            if outputs != row.output_fields
                || fields
                    .iter()
                    .any(|field| !source.output_fields.contains(&field.input_field_id))
            {
                return Err(invalid_node(
                    &row.node_id,
                    "projection field mapping disagrees with input/output contracts",
                ));
            }
        }
        ProgramRelationalOperator::Filter | ProgramRelationalOperator::Sort { .. } => {
            if input(0).output_fields != row.output_fields {
                return Err(invalid_node(
                    &row.node_id,
                    "row-preserving operator changed its field contract",
                ));
            }
            if let ProgramRelationalOperator::Sort { fields } = &row.operator
                && fields
                    .iter()
                    .any(|field| !row.output_fields.contains(&field.input_field_id))
            {
                return Err(invalid_node(&row.node_id, "sort field is out of scope"));
            }
        }
        ProgramRelationalOperator::Limit { .. } => {
            if input(0).output_fields != row.output_fields {
                return Err(invalid_node(
                    &row.node_id,
                    "limit changed its field contract",
                ));
            }
        }
        ProgramRelationalOperator::Join { predicates, .. } => {
            let left = input(0);
            let right = input(1);
            if predicates.iter().any(|predicate| {
                !left.output_fields.contains(&predicate.left_field_id)
                    || !right.output_fields.contains(&predicate.right_field_id)
                    || !is_binary_predicate(predicate.scalar_operator)
            }) {
                return Err(invalid_node(
                    &row.node_id,
                    "join predicate is invalid or out of scope",
                ));
            }
        }
        ProgramRelationalOperator::Union { .. } => {
            if row
                .input_node_ids
                .iter()
                .any(|node| nodes[node].output_fields != row.output_fields)
            {
                return Err(invalid_node(
                    &row.node_id,
                    "union inputs do not have the declared common schema",
                ));
            }
        }
        ProgramRelationalOperator::Aggregate {
            group_by,
            aggregates,
        } => {
            let source = input(0);
            let outputs = group_by
                .iter()
                .map(|field| field.output_field_id.clone())
                .chain(aggregates.iter().map(|field| field.output_field_id.clone()))
                .collect::<Vec<_>>();
            if outputs != row.output_fields
                || group_by
                    .iter()
                    .any(|field| !source.output_fields.contains(&field.input_field_id))
                || aggregates
                    .iter()
                    .any(|field| !source.output_fields.contains(&field.input_field_id))
            {
                return Err(invalid_node(
                    &row.node_id,
                    "aggregate field mapping disagrees with input/output contracts",
                ));
            }
        }
    }
    Ok(())
}

fn reachable_operator_nodes(
    nodes: &BTreeMap<Arc<str>, &ProgramOperatorRow>,
    root: &Arc<str>,
) -> Result<BTreeSet<Arc<str>>, RelationalSemanticQueryError> {
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::from([Arc::clone(root)]);
    while let Some(node_id) = queue.pop_front() {
        if !reachable.insert(Arc::clone(&node_id)) {
            continue;
        }
        let node = nodes.get(node_id.as_ref()).ok_or_else(|| {
            RelationalSemanticQueryError::OperatorCycle {
                form: "unresolved".to_owned(),
                role: node_id.to_string(),
            }
        })?;
        queue.extend(node.input_node_ids.iter().cloned());
    }
    Ok(reachable)
}

fn is_binary_predicate(operator: ScalarOperator) -> bool {
    matches!(
        operator,
        ScalarOperator::Equal
            | ScalarOperator::NotEqual
            | ScalarOperator::LessThan
            | ScalarOperator::LessThanOrEqual
            | ScalarOperator::GreaterThan
            | ScalarOperator::GreaterThanOrEqual
    )
}

fn duplicate(kind: &'static str, key: &str) -> RelationalSemanticQueryError {
    RelationalSemanticQueryError::DuplicateProgramBinding {
        kind,
        key: key.to_owned(),
    }
}

fn invalid_node(node: &str, detail: &str) -> RelationalSemanticQueryError {
    RelationalSemanticQueryError::InvalidOperatorNode {
        node: node.to_owned(),
        detail: detail.to_owned(),
    }
}

fn output_schema_error(
    binding: &ProgramFormBindingRow,
    detail: &str,
) -> RelationalSemanticQueryError {
    RelationalSemanticQueryError::OutputSchema {
        form: binding.form_label.to_string(),
        role: binding.output_role_id.to_string(),
        detail: detail.to_owned(),
    }
}

fn validate_request<'a>(
    request: &'a SemanticRequestRelations,
    catalog: &ValidatedProgramCatalog<'_>,
) -> Result<ValidatedRequest<'a>, RelationalSemanticQueryError> {
    validate_identity("semantic request", &request.semantic_request_id)?;
    for (kind, pin) in [
        ("program_catalog_pin", request.program_catalog_pin),
        ("source_pin", request.source_pin),
        ("policy_pin", request.policy_pin),
        (
            "producer_closure_proof_pin",
            request.producer_closure_proof_pin,
        ),
    ] {
        validate_pin(kind, pin)?;
    }
    if request.blocks.is_empty() || request.blocks.len() > request.limits.max_blocks() {
        return Err(RelationalSemanticQueryError::RequestLimit {
            limit: "max_blocks",
            observed: request.blocks.len(),
            maximum: request.limits.max_blocks(),
        });
    }
    if request.dependencies.len() > request.limits.max_dependencies() {
        return Err(RelationalSemanticQueryError::RequestLimit {
            limit: "max_dependencies",
            observed: request.dependencies.len(),
            maximum: request.limits.max_dependencies(),
        });
    }

    let mut blocks = BTreeMap::new();
    for block in &request.blocks {
        validate_identity("query", &block.query_id)?;
        validate_identity("output role", &block.output_role_id)?;
        if block
            .explicit_result_limit
            .is_some_and(|limit| limit == 0 || limit > request.limits.max_explicit_result_rows())
        {
            return Err(RelationalSemanticQueryError::RequestLimit {
                limit: "max_explicit_result_rows",
                observed: block.explicit_result_limit.unwrap_or_default(),
                maximum: request.limits.max_explicit_result_rows(),
            });
        }
        if blocks.insert(Arc::clone(&block.query_id), block).is_some() {
            return Err(duplicate("request block", &block.query_id));
        }
        let key = (
            Arc::from(block.form.label()),
            Arc::clone(&block.output_role_id),
        );
        if !catalog.forms.contains_key(&key) {
            return Err(RelationalSemanticQueryError::MissingFormBinding {
                query_id: block.query_id.to_string(),
                form: block.form,
                role: block.output_role_id.to_string(),
            });
        }
    }

    let mut clauses = BTreeMap::new();
    for clause in &request.clauses {
        validate_identity("query", &clause.query_id)?;
        validate_identity("clause", &clause.clause_id)?;
        let block = blocks.get(clause.query_id.as_ref()).ok_or_else(|| {
            RelationalSemanticQueryError::ClauseBinding {
                query_id: clause.query_id.to_string(),
                clause_id: clause.clause_id.to_string(),
            }
        })?;
        let catalog_key = (
            Arc::from(block.form.label()),
            Arc::clone(&block.output_role_id),
            Arc::clone(&clause.clause_id),
        );
        let binding = catalog.clauses.get(&catalog_key).ok_or_else(|| {
            RelationalSemanticQueryError::ClauseBinding {
                query_id: clause.query_id.to_string(),
                clause_id: clause.clause_id.to_string(),
            }
        })?;
        if binding.value_kind != clause.value.kind() {
            return Err(RelationalSemanticQueryError::ClauseBinding {
                query_id: clause.query_id.to_string(),
                clause_id: clause.clause_id.to_string(),
            });
        }
        if clauses
            .insert(
                (Arc::clone(&clause.query_id), Arc::clone(&clause.clause_id)),
                clause,
            )
            .is_some()
        {
            return Err(duplicate(
                "request clause",
                &format!("{}/{}", clause.query_id, clause.clause_id),
            ));
        }
    }
    for block in blocks.values() {
        let form = block.form.label();
        for binding in catalog.clauses.values().filter(|binding| {
            binding.form_label.as_ref() == form
                && binding.output_role_id == block.output_role_id
                && binding.required
        }) {
            if !clauses.contains_key(&(Arc::clone(&block.query_id), Arc::clone(&binding.clause_id)))
            {
                return Err(RelationalSemanticQueryError::ClauseBinding {
                    query_id: block.query_id.to_string(),
                    clause_id: binding.clause_id.to_string(),
                });
            }
        }
    }

    let mut incoming: BTreeMap<Arc<str>, Vec<&SemanticRequestDependencyRow>> = blocks
        .keys()
        .map(|query| (Arc::clone(query), Vec::new()))
        .collect();
    let mut outgoing_counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut incoming_counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut edges = BTreeSet::new();
    for edge in &request.dependencies {
        for (kind, value) in [
            ("producer query", edge.producer_query_id.as_ref()),
            ("producer role", edge.producer_role_id.as_ref()),
            ("consumer query", edge.consumer_query_id.as_ref()),
            ("consumer role", edge.consumer_role_id.as_ref()),
        ] {
            validate_identity(kind, value)?;
        }
        let producer = blocks.get(edge.producer_query_id.as_ref()).ok_or_else(|| {
            RelationalSemanticQueryError::UnknownDependencyQuery(edge.producer_query_id.to_string())
        })?;
        let consumer = blocks.get(edge.consumer_query_id.as_ref()).ok_or_else(|| {
            RelationalSemanticQueryError::UnknownDependencyQuery(edge.consumer_query_id.to_string())
        })?;
        if producer.query_id == consumer.query_id {
            return Err(RelationalSemanticQueryError::QueryDependencyCycle);
        }
        if edge.producer_role_id != producer.output_role_id {
            return Err(RelationalSemanticQueryError::UnknownCompositionRole {
                query_id: producer.query_id.to_string(),
                role: edge.producer_role_id.to_string(),
            });
        }
        let consumer_role = (
            Arc::from(consumer.form.label()),
            Arc::clone(&edge.consumer_role_id),
        );
        let Some(consumer_role_binding) = catalog.input_roles.get(&consumer_role) else {
            return Err(RelationalSemanticQueryError::UnknownCompositionRole {
                query_id: consumer.query_id.to_string(),
                role: edge.consumer_role_id.to_string(),
            });
        };
        let consumes_role_relation = catalog.operators.values().any(|operator| {
            operator.form_label.as_ref() == consumer.form.label()
                && operator.output_role_id == consumer.output_role_id
                && matches!(
                    &operator.operator,
                    ProgramRelationalOperator::Input { relation_id }
                        if relation_id == &consumer_role_binding.input_relation_id
                )
        });
        if !consumes_role_relation {
            return Err(RelationalSemanticQueryError::UnknownCompositionRole {
                query_id: consumer.query_id.to_string(),
                role: edge.consumer_role_id.to_string(),
            });
        }
        let producer_key = (
            Arc::from(producer.form.label()),
            Arc::clone(&edge.producer_role_id),
        );
        let producer_binding = catalog.forms[&producer_key];
        let consumer_schema = catalog.schemas[&consumer_role_binding.input_relation_id];
        if producer_binding.output_fields != consumer_schema.fields {
            return Err(RelationalSemanticQueryError::OutputSchema {
                form: consumer.form.label().to_owned(),
                role: edge.consumer_role_id.to_string(),
                detail: "producer output fields differ from consumer role schema".to_owned(),
            });
        }
        if !edges.insert(edge.clone()) {
            return Err(duplicate(
                "composition edge",
                &format!(
                    "{}:{}->{}:{}",
                    edge.producer_query_id,
                    edge.producer_role_id,
                    edge.consumer_query_id,
                    edge.consumer_role_id
                ),
            ));
        }
        *outgoing_counts
            .entry(edge.producer_query_id.as_ref())
            .or_default() += 1;
        *incoming_counts
            .entry(edge.consumer_query_id.as_ref())
            .or_default() += 1;
        incoming
            .get_mut(edge.consumer_query_id.as_ref())
            .expect("consumer exists")
            .push(edge);
    }
    if let Some((_, observed)) = outgoing_counts
        .iter()
        .find(|(_, observed)| **observed > request.limits.max_fanout())
    {
        return Err(RelationalSemanticQueryError::RequestLimit {
            limit: "max_fanout",
            observed: *observed,
            maximum: request.limits.max_fanout(),
        });
    }
    if let Some((_, observed)) = incoming_counts
        .iter()
        .find(|(_, observed)| **observed > request.limits.max_fanin())
    {
        return Err(RelationalSemanticQueryError::RequestLimit {
            limit: "max_fanin",
            observed: *observed,
            maximum: request.limits.max_fanin(),
        });
    }
    for edges in incoming.values_mut() {
        edges.sort();
        let mut roles = BTreeSet::new();
        for edge in edges.iter() {
            if !roles.insert(edge.consumer_role_id.as_ref()) {
                return Err(RelationalSemanticQueryError::UnknownCompositionRole {
                    query_id: edge.consumer_query_id.to_string(),
                    role: edge.consumer_role_id.to_string(),
                });
            }
        }
    }
    let order = request_dependency_order(&blocks, &request.dependencies)?;
    Ok(ValidatedRequest {
        blocks,
        clauses,
        incoming,
        order,
    })
}

fn request_dependency_order(
    blocks: &BTreeMap<Arc<str>, &SemanticRequestBlockRow>,
    dependencies: &[SemanticRequestDependencyRow],
) -> Result<Vec<Arc<str>>, RelationalSemanticQueryError> {
    let mut degree: BTreeMap<Arc<str>, usize> =
        blocks.keys().map(|id| (Arc::clone(id), 0)).collect();
    let mut outgoing: BTreeMap<Arc<str>, BTreeSet<Arc<str>>> = BTreeMap::new();
    for edge in dependencies {
        *degree
            .get_mut(edge.consumer_query_id.as_ref())
            .expect("validated consumer") += 1;
        outgoing
            .entry(Arc::clone(&edge.producer_query_id))
            .or_default()
            .insert(Arc::clone(&edge.consumer_query_id));
    }
    let mut ready = degree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| Arc::clone(id))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(blocks.len());
    while let Some(next) = ready.pop_first() {
        order.push(Arc::clone(&next));
        for consumer in outgoing.get(next.as_ref()).into_iter().flatten() {
            let degree = degree
                .get_mut(consumer.as_ref())
                .expect("validated consumer");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(Arc::clone(consumer));
            }
        }
    }
    if order.len() != blocks.len() {
        Err(RelationalSemanticQueryError::QueryDependencyCycle)
    } else {
        Ok(order)
    }
}

fn validate_closure<'a>(
    proof: &'a ProducerClosureProof,
    request: &SemanticRequestRelations,
) -> Result<BTreeMap<Arc<str>, ClosureStatus<'a>>, RelationalSemanticQueryError> {
    validate_pin("producer closure proof", proof.proof_pin)?;
    validate_identity(
        "producer closure authority",
        &proof.application_authority_id,
    )?;
    if proof.proof_pin != request.producer_closure_proof_pin {
        return Err(RelationalSemanticQueryError::ProducerClosurePinMismatch);
    }
    let mut families = BTreeMap::new();
    for row in &proof.families {
        validate_identity("producer family", &row.family_id)?;
        let status = match &row.disposition {
            ProducerFamilyDisposition::RuntimeProducer(runtime) => {
                for (kind, value) in [
                    ("producer", runtime.producer_id.as_ref()),
                    ("producer authority", runtime.authority_id.as_ref()),
                    ("producer algorithm", runtime.algorithm_release.as_ref()),
                    ("producer precision", runtime.precision_id.as_ref()),
                ] {
                    validate_identity(kind, value)?;
                }
                if runtime.authority_id != proof.application_authority_id {
                    return Err(closure_error(
                        &row.family_id,
                        "runtime producer authority is not application-owned",
                    ));
                }
                for (kind, pin) in [
                    ("producer input", runtime.input_pin),
                    ("producer invalidation", runtime.invalidation_pin),
                    ("producer materialization", runtime.materialization_pin),
                    ("producer completeness", runtime.completeness_proof_pin),
                    ("producer proof", runtime.producer_proof_pin),
                ] {
                    validate_pin(kind, pin)?;
                }
                if runtime.requested_units != runtime.completed_units
                    || runtime.remainder_units != 0
                    || runtime.unknown_units != 0
                {
                    return Err(closure_error(
                        &row.family_id,
                        "runtime producer completeness census is not exact",
                    ));
                }
                ClosureStatus::Runtime(runtime)
            }
            ProducerFamilyDisposition::UnsupportedRemainder(remainder) => {
                for (kind, value) in [
                    ("remainder", remainder.remainder_id.as_ref()),
                    ("remainder authority", remainder.authority_id.as_ref()),
                    ("remainder reason", remainder.reason_id.as_ref()),
                ] {
                    validate_identity(kind, value)?;
                }
                if remainder.authority_id != proof.application_authority_id {
                    return Err(closure_error(
                        &row.family_id,
                        "unsupported remainder authority is not application-owned",
                    ));
                }
                validate_pin("unsupported remainder proof", remainder.proof_pin)?;
                ClosureStatus::Remainder(remainder)
            }
        };
        if families
            .insert(Arc::clone(&row.family_id), status)
            .is_some()
        {
            return Err(closure_error(
                &row.family_id,
                "family has multiple producer-or-remainder dispositions",
            ));
        }
    }
    Ok(families)
}

fn closure_error(family: &str, detail: &str) -> RelationalSemanticQueryError {
    RelationalSemanticQueryError::ProducerClosure {
        family: family.to_owned(),
        detail: detail.to_owned(),
    }
}

struct CompiledRoot {
    relation_id: RelationId,
    expression: RelationalExpression,
}

/// Compile one pinned semantic request through a single data-driven relational algorithm.
///
/// Output relation/schema closure is validated from catalog rows before any `RelationalProgram`
/// is built. The existing relational runtime subsequently revalidates the selected output
/// against the live programmatic epoch bindings and compiles it to a native DataFusion plan.
pub fn compile_relational_semantic_request(
    request: &SemanticRequestRelations,
    program_catalog: &SemanticQueryProgramCatalog,
    producer_closure: &ProducerClosureProof,
) -> Result<CompiledSemanticRequest, RelationalSemanticQueryError> {
    if request.program_catalog_pin != program_catalog.program_catalog_pin {
        return Err(RelationalSemanticQueryError::ProgramCatalogMismatch);
    }
    let catalog = validate_program_catalog(program_catalog, request.limits)?;
    let request_rows = validate_request(request, &catalog)?;
    let closure = validate_closure(producer_closure, request)?;

    let mut dependencies = BTreeSet::from([
        SemanticCompilerDependency::ProgramCatalog(request.program_catalog_pin),
        SemanticCompilerDependency::SourcePin(request.source_pin),
        SemanticCompilerDependency::PolicyPin(request.policy_pin),
        SemanticCompilerDependency::ProducerClosureProof(request.producer_closure_proof_pin),
        SemanticCompilerDependency::ProgramRelease(program_catalog.program_release_pin),
        SemanticCompilerDependency::Authority(Arc::clone(&catalog.authority_id)),
        SemanticCompilerDependency::SemanticClass(Arc::clone(&catalog.semantic_class_id)),
    ]);
    for block in request_rows.blocks.values() {
        dependencies.insert(SemanticCompilerDependency::RequestBlock {
            query_id: Arc::clone(&block.query_id),
            content_pin: debug_pin(b"request-block", block),
        });
    }
    for clause in request_rows.clauses.values() {
        dependencies.insert(SemanticCompilerDependency::Clause {
            clause_id: Arc::clone(&clause.clause_id),
            binding_pin: debug_pin(b"request-clause", clause),
        });
    }
    for edge in &request.dependencies {
        dependencies.insert(SemanticCompilerDependency::CompositionEdge(debug_pin(
            b"composition-edge",
            edge,
        )));
    }

    let mut operators = BTreeSet::new();
    let mut compiled_roots: BTreeMap<Arc<str>, CompiledRoot> = BTreeMap::new();
    let mut dispositions: BTreeMap<Arc<str>, SemanticBlockDisposition> = BTreeMap::new();
    let mut blocks = Vec::with_capacity(request_rows.blocks.len());

    for query_id in &request_rows.order {
        let block = request_rows.blocks[query_id.as_ref()];
        let form_label: Arc<str> = Arc::from(block.form.label());
        let key = (Arc::clone(&form_label), Arc::clone(&block.output_role_id));
        let binding = catalog.forms[&key];
        dependencies.insert(SemanticCompilerDependency::FormRole {
            form_label: Arc::clone(&form_label),
            role_id: Arc::clone(&block.output_role_id),
            binding_pin: debug_pin(b"catalog-form-binding", binding),
        });
        dependencies.insert(SemanticCompilerDependency::Relation(
            binding.output_relation_id.clone(),
        ));
        dependencies.extend(
            binding
                .output_fields
                .iter()
                .cloned()
                .map(SemanticCompilerDependency::Field),
        );

        let mut issues = Vec::new();
        let mut own_disposition = SemanticBlockDisposition::Compiled;
        for family in catalog.required_families.get(&key).into_iter().flatten() {
            dependencies.insert(SemanticCompilerDependency::FactFamily(Arc::clone(family)));
            match closure.get(family.as_ref()) {
                Some(ClosureStatus::Runtime(runtime)) => {
                    dependencies.insert(SemanticCompilerDependency::Producer(Arc::clone(
                        &runtime.producer_id,
                    )));
                }
                Some(ClosureStatus::Remainder(remainder)) => {
                    dependencies.insert(SemanticCompilerDependency::UnsupportedRemainder(
                        Arc::clone(&remainder.remainder_id),
                    ));
                    own_disposition = SemanticBlockDisposition::UnsupportedRemainder;
                    issues.push(SemanticCompilationIssue {
                        code: "UNSUPPORTED_FACT_FAMILY",
                        subject_id: Arc::clone(family),
                        related_id: Some(Arc::clone(&remainder.reason_id)),
                    });
                }
                None => {
                    if own_disposition != SemanticBlockDisposition::UnsupportedRemainder {
                        own_disposition = SemanticBlockDisposition::UnknownProducerClosure;
                    }
                    issues.push(SemanticCompilationIssue {
                        code: "PRODUCER_CLOSURE_UNKNOWN",
                        subject_id: Arc::clone(family),
                        related_id: None,
                    });
                }
            }
        }

        let failed_dependencies = request_rows
            .incoming
            .get(query_id.as_ref())
            .into_iter()
            .flatten()
            .filter(|edge| {
                dispositions
                    .get(edge.producer_query_id.as_ref())
                    .is_some_and(|state| *state != SemanticBlockDisposition::Compiled)
            })
            .map(|edge| Arc::clone(&edge.producer_query_id))
            .collect::<BTreeSet<_>>();
        if own_disposition == SemanticBlockDisposition::Compiled && !failed_dependencies.is_empty()
        {
            own_disposition = SemanticBlockDisposition::NotExecutedDependency;
            issues.extend(failed_dependencies.into_iter().map(|dependency| {
                SemanticCompilationIssue {
                    code: "NOT_EXECUTED_DEPENDENCY",
                    subject_id: Arc::clone(query_id),
                    related_id: Some(dependency),
                }
            }));
        }

        issues.sort();
        let output = if own_disposition == SemanticBlockDisposition::Compiled {
            let composition_inputs = composition_inputs(
                block,
                request_rows.incoming.get(query_id.as_ref()),
                &request_rows,
                &catalog,
                &compiled_roots,
            )?;
            let program = lower_block(
                block,
                binding,
                &request_rows,
                &catalog,
                &composition_inputs,
                &mut dependencies,
                &mut operators,
            )?;
            compiled_roots.insert(
                Arc::clone(query_id),
                CompiledRoot {
                    relation_id: binding.output_relation_id.clone(),
                    expression: program.root.clone(),
                },
            );
            Some(SelectedQueryOutput::new(
                binding.output_relation_id.clone(),
                program,
                Some(output_coverage(block)?),
            ))
        } else {
            None
        };
        dispositions.insert(Arc::clone(query_id), own_disposition);
        blocks.push(CompiledSemanticBlock {
            query_id: Arc::clone(query_id),
            form: block.form,
            disposition: own_disposition,
            output,
            issues,
        });
    }

    let compiler_proof_pin = compiler_proof_pin(
        request,
        &dependencies,
        &operators,
        &request_rows.order,
        &blocks,
    );
    Ok(CompiledSemanticRequest {
        blocks,
        observation: SemanticCompilerObservation {
            program_release: RELATIONAL_SEMANTIC_QUERY_PROGRAM_RELEASE,
            compiler_proof_pin,
            dependencies,
            operators,
            dependency_order: request_rows.order,
            limits: request.limits,
        },
    })
}

fn output_coverage(
    block: &SemanticRequestBlockRow,
) -> Result<ResultCoverage, RelationalSemanticQueryError> {
    if block.explicit_result_limit.is_some() {
        Ok(ResultCoverage::try_new(
            ResultCompleteness::Unknown,
            1,
            0,
            1,
            Some(ResultUnknownCause::try_new(
                "EXPLICIT_LIMIT_REACHED_UNKNOWN",
            )?),
        )?)
    } else {
        Ok(ResultCoverage::complete(1))
    }
}

fn composition_inputs(
    block: &SemanticRequestBlockRow,
    incoming: Option<&Vec<&SemanticRequestDependencyRow>>,
    request: &ValidatedRequest<'_>,
    catalog: &ValidatedProgramCatalog<'_>,
    roots: &BTreeMap<Arc<str>, CompiledRoot>,
) -> Result<BTreeMap<RelationId, RelationalExpression>, RelationalSemanticQueryError> {
    let mut inputs = BTreeMap::new();
    for edge in incoming.into_iter().flatten() {
        let producer_block = request.blocks[edge.producer_query_id.as_ref()];
        let producer_key = (
            Arc::from(producer_block.form.label()),
            Arc::clone(&edge.producer_role_id),
        );
        let producer_binding = catalog.forms[&producer_key];
        let producer = roots
            .get(edge.producer_query_id.as_ref())
            .expect("compiled dependency order proved available producer");
        if producer.relation_id != producer_binding.output_relation_id {
            return Err(RelationalSemanticQueryError::OutputSchema {
                form: producer_block.form.label().to_owned(),
                role: edge.producer_role_id.to_string(),
                detail: "compiled upstream relation differs from its catalog role".to_owned(),
            });
        }
        let consumer_role_key = (
            Arc::from(block.form.label()),
            Arc::clone(&edge.consumer_role_id),
        );
        let relation = catalog.input_roles[&consumer_role_key]
            .input_relation_id
            .clone();
        if inputs
            .insert(relation, producer.expression.clone())
            .is_some()
        {
            return Err(RelationalSemanticQueryError::UnknownCompositionRole {
                query_id: block.query_id.to_string(),
                role: edge.consumer_role_id.to_string(),
            });
        }
    }
    Ok(inputs)
}

#[allow(clippy::too_many_arguments)]
fn lower_block(
    block: &SemanticRequestBlockRow,
    binding: &ProgramFormBindingRow,
    request: &ValidatedRequest<'_>,
    catalog: &ValidatedProgramCatalog<'_>,
    composition_inputs: &BTreeMap<RelationId, RelationalExpression>,
    dependencies: &mut BTreeSet<SemanticCompilerDependency>,
    selections: &mut BTreeSet<SemanticCompilerOperator>,
) -> Result<RelationalProgram, RelationalSemanticQueryError> {
    let mut rows = catalog
        .operators
        .values()
        .filter(|row| {
            row.form_label == binding.form_label && row.output_role_id == binding.output_role_id
        })
        .copied()
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.ordinal
            .cmp(&right.ordinal)
            .then_with(|| left.node_id.cmp(&right.node_id))
    });
    let mut expressions = BTreeMap::new();
    for row in rows {
        dependencies.insert(SemanticCompilerDependency::OperatorNode {
            node_id: Arc::clone(&row.node_id),
            binding_pin: debug_pin(b"catalog-operator", row),
        });
        dependencies.extend(
            row.output_fields
                .iter()
                .cloned()
                .map(SemanticCompilerDependency::Field),
        );
        let inputs =
            row.input_node_ids
                .iter()
                .map(|node| {
                    expressions.get(node.as_ref()).cloned().ok_or_else(|| {
                        invalid_node(&row.node_id, "lowering dependency is unavailable")
                    })
                })
                .collect::<Result<Vec<RelationalExpression>, _>>()?;
        let expression = match &row.operator {
            ProgramRelationalOperator::Input { relation_id } => {
                selections.insert(SemanticCompilerOperator::Input);
                dependencies.insert(SemanticCompilerDependency::Relation(relation_id.clone()));
                composition_inputs
                    .get(relation_id)
                    .cloned()
                    .unwrap_or_else(|| RelationalExpression::Input(relation_id.clone()))
            }
            ProgramRelationalOperator::Projection { fields } => {
                selections.insert(SemanticCompilerOperator::Projection);
                dependencies.extend(fields.iter().flat_map(|field| {
                    [
                        SemanticCompilerDependency::Field(field.input_field_id.clone()),
                        SemanticCompilerDependency::Field(field.output_field_id.clone()),
                    ]
                }));
                RelationalExpression::Projection {
                    input: Box::new(inputs[0].clone()),
                    expressions: fields
                        .iter()
                        .map(|field| NamedExpression {
                            field_id: field.output_field_id.clone(),
                            expression: ScalarExpression::Field(field.input_field_id.clone()),
                        })
                        .collect(),
                }
            }
            ProgramRelationalOperator::Filter => {
                let mut predicates = catalog
                    .clauses
                    .values()
                    .filter(|clause| {
                        clause.form_label == binding.form_label
                            && clause.output_role_id == binding.output_role_id
                            && clause.operator_node_id == row.node_id
                    })
                    .filter_map(|clause| {
                        request
                            .clauses
                            .get(&(Arc::clone(&block.query_id), Arc::clone(&clause.clause_id)))
                            .map(|value| (clause, *value))
                    })
                    .collect::<Vec<_>>();
                predicates.sort_by(|(left, _), (right, _)| left.clause_id.cmp(&right.clause_id));
                if predicates.is_empty() {
                    inputs[0].clone()
                } else {
                    selections.insert(SemanticCompilerOperator::Filter);
                    let mut lowered = predicates
                        .into_iter()
                        .map(|(clause, value)| {
                            dependencies.insert(SemanticCompilerDependency::Clause {
                                clause_id: Arc::clone(&clause.clause_id),
                                binding_pin: debug_pin(b"catalog-clause", clause),
                            });
                            dependencies.insert(SemanticCompilerDependency::Field(
                                clause.input_field_id.clone(),
                            ));
                            ScalarExpression::Call {
                                operator: clause.scalar_operator,
                                arguments: vec![
                                    ScalarExpression::Field(clause.input_field_id.clone()),
                                    ScalarExpression::Literal(value.value.scalar()),
                                ],
                            }
                        })
                        .collect::<VecDeque<_>>();
                    let first = lowered.pop_front().expect("non-empty predicates");
                    let predicate =
                        lowered
                            .into_iter()
                            .fold(first, |left, right| ScalarExpression::Call {
                                operator: ScalarOperator::And,
                                arguments: vec![left, right],
                            });
                    RelationalExpression::Filter {
                        input: Box::new(inputs[0].clone()),
                        predicate,
                    }
                }
            }
            ProgramRelationalOperator::Join { kind, predicates } => {
                selections.insert(SemanticCompilerOperator::Join(*kind));
                let predicates = predicates
                    .iter()
                    .map(|predicate| ScalarExpression::Call {
                        operator: predicate.scalar_operator,
                        arguments: vec![
                            ScalarExpression::Field(predicate.left_field_id.clone()),
                            ScalarExpression::Field(predicate.right_field_id.clone()),
                        ],
                    })
                    .collect();
                RelationalExpression::Join {
                    left: Box::new(inputs[0].clone()),
                    right: Box::new(inputs[1].clone()),
                    kind: *kind,
                    predicates,
                }
            }
            ProgramRelationalOperator::Union { kind } => {
                selections.insert(SemanticCompilerOperator::Union(*kind));
                RelationalExpression::Union {
                    inputs,
                    kind: *kind,
                }
            }
            ProgramRelationalOperator::Aggregate {
                group_by,
                aggregates,
            } => {
                selections.insert(SemanticCompilerOperator::Aggregate);
                RelationalExpression::Aggregate {
                    input: Box::new(inputs[0].clone()),
                    group_by: group_by
                        .iter()
                        .map(|field| NamedExpression {
                            field_id: field.output_field_id.clone(),
                            expression: ScalarExpression::Field(field.input_field_id.clone()),
                        })
                        .collect(),
                    aggregates: aggregates
                        .iter()
                        .map(|field| NamedAggregateExpression {
                            field_id: field.output_field_id.clone(),
                            expression: AggregateExpression {
                                operator: field.aggregate_operator,
                                argument: ScalarExpression::Field(field.input_field_id.clone()),
                            },
                        })
                        .collect(),
                }
            }
            ProgramRelationalOperator::Sort { fields } => {
                selections.insert(SemanticCompilerOperator::Sort);
                RelationalExpression::Sort {
                    input: Box::new(inputs[0].clone()),
                    expressions: fields
                        .iter()
                        .map(|field| SortExpression {
                            expression: ScalarExpression::Field(field.input_field_id.clone()),
                            ascending: field.ascending,
                            nulls_first: field.nulls_first,
                        })
                        .collect(),
                }
            }
            ProgramRelationalOperator::Limit { skip } => {
                if let Some(fetch) = block.explicit_result_limit {
                    selections.insert(SemanticCompilerOperator::Limit);
                    RelationalExpression::Limit {
                        input: Box::new(inputs[0].clone()),
                        skip: *skip,
                        fetch: Some(fetch),
                    }
                } else {
                    inputs[0].clone()
                }
            }
        };
        expressions.insert(Arc::clone(&row.node_id), expression);
    }
    let root = expressions
        .remove(binding.root_node_id.as_ref())
        .expect("validated root was lowered");
    Ok(RelationalProgram {
        root,
        output_fields: binding.output_fields.clone(),
    })
}

fn debug_pin<T: std::fmt::Debug>(domain: &[u8], value: &T) -> [u8; 32] {
    let rendered = format!("{value:?}");
    let mut hasher = blake3::Hasher::new();
    hash_part(&mut hasher, domain);
    hash_part(&mut hasher, rendered.as_bytes());
    *hasher.finalize().as_bytes()
}

fn compiler_proof_pin(
    request: &SemanticRequestRelations,
    dependencies: &BTreeSet<SemanticCompilerDependency>,
    operators: &BTreeSet<SemanticCompilerOperator>,
    order: &[Arc<str>],
    blocks: &[CompiledSemanticBlock],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hash_part(
        &mut hasher,
        b"codefabric.relational-semantic-query.compiler-proof.v1",
    );
    hash_part(
        &mut hasher,
        RELATIONAL_SEMANTIC_QUERY_PROGRAM_RELEASE.as_bytes(),
    );
    hash_part(&mut hasher, request.semantic_request_id.as_bytes());
    for dependency in dependencies {
        hash_part(&mut hasher, format!("{dependency:?}").as_bytes());
    }
    for operator in operators {
        hash_part(&mut hasher, format!("{operator:?}").as_bytes());
    }
    for query_id in order {
        hash_part(&mut hasher, query_id.as_bytes());
    }
    for block in blocks {
        hash_part(&mut hasher, block.query_id.as_bytes());
        hash_part(&mut hasher, format!("{:?}", block.disposition).as_bytes());
        for issue in &block.issues {
            hash_part(&mut hasher, format!("{issue:?}").as_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

fn hash_part(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value);
}

struct ValidatedEpochBoundIngressCatalog<'a> {
    programs: BTreeMap<Arc<str>, &'a EpochBoundProgramBindingRow>,
    consumer_slots: BTreeMap<(Arc<str>, Arc<str>), &'a EpochBoundConsumerSlotBindingRow>,
    selections: BTreeMap<(Arc<str>, Arc<str>), &'a EpochBoundSelectionBindingRow>,
    returns: BTreeMap<(Arc<str>, Arc<str>), &'a EpochBoundReturnBindingRow>,
    scopes: BTreeMap<Arc<str>, &'a EpochBoundScopeBindingRow>,
    request_inputs: BTreeMap<(Arc<str>, Arc<str>), &'a EpochBoundRequestInputBindingRow>,
}

fn epoch_nonzero(
    value: usize,
    name: &'static str,
) -> Result<NonZeroUsize, EpochBoundSemanticIngressError> {
    NonZeroUsize::new(value).ok_or(EpochBoundSemanticIngressError::ZeroBound(name))
}

fn validate_epoch_identity(
    kind: &'static str,
    value: &str,
) -> Result<(), EpochBoundSemanticIngressError> {
    if value.is_empty()
        || value.len() > 1_024
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err(EpochBoundSemanticIngressError::InvalidIdentity {
            kind,
            value: value.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn validate_epoch_pin(
    kind: &'static str,
    value: [u8; 32],
) -> Result<(), EpochBoundSemanticIngressError> {
    if value == [0; 32] {
        Err(EpochBoundSemanticIngressError::MissingPin(kind))
    } else {
        Ok(())
    }
}

fn validate_epoch_cardinality_declaration(
    family: &'static str,
    binding_id: &str,
    minimum: usize,
    maximum: usize,
    policy_maximum: usize,
) -> Result<(), EpochBoundSemanticIngressError> {
    if maximum == 0 || minimum > maximum || maximum > policy_maximum {
        return Err(EpochBoundSemanticIngressError::Cardinality {
            family,
            binding_id: binding_id.to_owned(),
            observed: maximum,
            minimum,
            maximum: policy_maximum,
        });
    }
    Ok(())
}

fn validate_epoch_cardinality(
    family: &'static str,
    binding_id: &str,
    observed: usize,
    minimum: usize,
    maximum: usize,
) -> Result<(), EpochBoundSemanticIngressError> {
    if observed < minimum || observed > maximum {
        return Err(EpochBoundSemanticIngressError::Cardinality {
            family,
            binding_id: binding_id.to_owned(),
            observed,
            minimum,
            maximum,
        });
    }
    Ok(())
}

fn validate_epoch_ordinals(
    family: &'static str,
    binding_id: &str,
    ordinals: &[u32],
) -> Result<(), EpochBoundSemanticIngressError> {
    if ordinals
        .iter()
        .copied()
        .enumerate()
        .any(|(index, ordinal)| usize::try_from(ordinal).map_or(true, |ordinal| ordinal != index))
    {
        return Err(EpochBoundSemanticIngressError::Ordinal {
            family,
            binding_id: binding_id.to_owned(),
        });
    }
    Ok(())
}

fn epoch_duplicate(kind: &'static str, key: impl Into<String>) -> EpochBoundSemanticIngressError {
    EpochBoundSemanticIngressError::Duplicate {
        kind,
        key: key.into(),
    }
}

/// Return the canonical pin for an exact ingress limit set.
#[must_use]
pub fn epoch_bound_semantic_ingress_limits_pin(
    limits: EpochBoundSemanticIngressLimits,
) -> [u8; 32] {
    let compiler = limits.compiler();
    let values = [
        compiler.max_blocks(),
        compiler.max_dependencies(),
        compiler.max_fanout(),
        compiler.max_fanin(),
        compiler.max_operator_nodes_per_block(),
        compiler.max_fields_per_node(),
        compiler.max_explicit_result_rows(),
        limits.max_selection_rows(),
        limits.max_return_rows(),
        limits.max_scope_rows(),
        limits.max_request_input_rows(),
        limits.max_fields_per_request_input_row(),
    ];
    let mut hasher = blake3::Hasher::new();
    hash_part(
        &mut hasher,
        b"codefabric.epoch-bound-semantic-ingress-limits.v1",
    );
    for value in values {
        hash_part(&mut hasher, &value.to_be_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn validate_epoch_bound_ingress_catalog<'a>(
    catalog: &'a EpochBoundSemanticIngressCatalog,
    limits: EpochBoundSemanticIngressLimits,
) -> Result<ValidatedEpochBoundIngressCatalog<'a>, EpochBoundSemanticIngressError> {
    for (kind, pin) in [
        ("catalog.fabric_epoch_pin", catalog.fabric_epoch_pin),
        ("catalog.program_catalog_pin", catalog.program_catalog_pin),
        ("catalog.source_pin", catalog.source_pin),
        ("catalog.policy_pin", catalog.policy_pin),
        (
            "catalog.producer_closure_proof_pin",
            catalog.producer_closure_proof_pin,
        ),
        ("catalog.limits_pin", catalog.limits_pin),
    ] {
        validate_epoch_pin(kind, pin)?;
    }

    let mut programs = BTreeMap::new();
    for row in &catalog.program_bindings {
        validate_epoch_identity("program binding", &row.program_binding_id)?;
        validate_epoch_identity("output role", &row.output_role_id)?;
        validate_epoch_pin("program_binding_pin", row.program_binding_pin)?;
        validate_epoch_pin("execution_program_pin", row.execution_program_pin)?;
        if programs
            .insert(Arc::clone(&row.program_binding_id), row)
            .is_some()
        {
            return Err(epoch_duplicate(
                "program binding",
                row.program_binding_id.to_string(),
            ));
        }
    }

    let compiler_limits = limits.compiler();
    let mut consumer_slots = BTreeMap::new();
    for row in &catalog.consumer_slots {
        validate_epoch_identity("program binding", &row.program_binding_id)?;
        validate_epoch_identity("consumer slot", &row.consumer_slot_id)?;
        validate_epoch_identity("consumer role", &row.consumer_role_id)?;
        if !programs.contains_key(row.program_binding_id.as_ref()) {
            return Err(epoch_duplicate(
                "consumer-slot program binding",
                row.program_binding_id.to_string(),
            ));
        }
        validate_epoch_cardinality_declaration(
            "dependency",
            &row.consumer_slot_id,
            row.minimum_edges,
            row.maximum_edges,
            compiler_limits.max_fanin(),
        )?;
        let key = (
            Arc::clone(&row.program_binding_id),
            Arc::clone(&row.consumer_slot_id),
        );
        if consumer_slots.insert(key, row).is_some() {
            return Err(epoch_duplicate(
                "consumer slot",
                format!("{}/{}", row.program_binding_id, row.consumer_slot_id),
            ));
        }
    }

    let mut selections = BTreeMap::new();
    for row in &catalog.selections {
        validate_epoch_identity("program binding", &row.program_binding_id)?;
        validate_epoch_identity("selection", &row.selection_id)?;
        if !programs.contains_key(row.program_binding_id.as_ref()) {
            return Err(epoch_duplicate(
                "selection program binding",
                row.program_binding_id.to_string(),
            ));
        }
        validate_epoch_cardinality_declaration(
            "selection",
            &row.selection_id,
            row.minimum_values,
            row.maximum_values,
            limits.max_selection_rows(),
        )?;
        let key = (
            Arc::clone(&row.program_binding_id),
            Arc::clone(&row.selection_id),
        );
        if selections.insert(key, row).is_some() {
            return Err(epoch_duplicate(
                "selection binding",
                format!("{}/{}", row.program_binding_id, row.selection_id),
            ));
        }
    }

    let mut returns = BTreeMap::new();
    for row in &catalog.returns {
        validate_epoch_identity("program binding", &row.program_binding_id)?;
        validate_epoch_identity("return", &row.return_id)?;
        if !programs.contains_key(row.program_binding_id.as_ref()) {
            return Err(epoch_duplicate(
                "return program binding",
                row.program_binding_id.to_string(),
            ));
        }
        validate_epoch_cardinality_declaration(
            "return",
            &row.return_id,
            row.minimum_values,
            row.maximum_values,
            limits.max_return_rows(),
        )?;
        let key = (
            Arc::clone(&row.program_binding_id),
            Arc::clone(&row.return_id),
        );
        if returns.insert(key, row).is_some() {
            return Err(epoch_duplicate(
                "return binding",
                format!("{}/{}", row.program_binding_id, row.return_id),
            ));
        }
    }

    let mut scopes = BTreeMap::new();
    for row in &catalog.scopes {
        validate_epoch_identity("scope", &row.scope_id)?;
        validate_epoch_cardinality_declaration(
            "scope",
            &row.scope_id,
            row.minimum_values,
            row.maximum_values,
            limits.max_scope_rows(),
        )?;
        if scopes.insert(Arc::clone(&row.scope_id), row).is_some() {
            return Err(epoch_duplicate("scope binding", row.scope_id.to_string()));
        }
    }

    let mut request_inputs = BTreeMap::new();
    let mut program_relations = BTreeSet::new();
    for row in &catalog.request_inputs {
        validate_epoch_identity("program binding", &row.program_binding_id)?;
        validate_epoch_identity("request input", &row.input_id)?;
        if !programs.contains_key(row.program_binding_id.as_ref()) {
            return Err(epoch_duplicate(
                "request-input program binding",
                row.program_binding_id.to_string(),
            ));
        }
        validate_epoch_cardinality_declaration(
            "request input",
            &row.input_id,
            row.minimum_rows,
            row.maximum_rows,
            limits.max_request_input_rows(),
        )?;
        if row.fields.is_empty() || row.fields.len() > limits.max_fields_per_request_input_row() {
            return Err(EpochBoundSemanticIngressError::Limit {
                limit: "max_fields_per_request_input_row",
                observed: row.fields.len(),
                maximum: limits.max_fields_per_request_input_row(),
            });
        }
        let mut field_ids = BTreeSet::new();
        for field in &row.fields {
            if !field_ids.insert(field.field_id.clone()) {
                return Err(epoch_duplicate(
                    "request input field",
                    field.field_id.as_str().to_owned(),
                ));
            }
        }
        if !program_relations.insert((
            Arc::clone(&row.program_binding_id),
            row.input_relation_id.clone(),
        )) {
            return Err(epoch_duplicate(
                "request input relation",
                format!(
                    "{}/{}",
                    row.program_binding_id,
                    row.input_relation_id.as_str()
                ),
            ));
        }
        let key = (
            Arc::clone(&row.program_binding_id),
            Arc::clone(&row.input_id),
        );
        if request_inputs.insert(key, row).is_some() {
            return Err(epoch_duplicate(
                "request input binding",
                format!("{}/{}", row.program_binding_id, row.input_id),
            ));
        }
    }

    Ok(ValidatedEpochBoundIngressCatalog {
        programs,
        consumer_slots,
        selections,
        returns,
        scopes,
        request_inputs,
    })
}

fn validate_epoch_limit(
    limit: &'static str,
    observed: usize,
    maximum: usize,
) -> Result<(), EpochBoundSemanticIngressError> {
    if observed > maximum {
        return Err(EpochBoundSemanticIngressError::Limit {
            limit,
            observed,
            maximum,
        });
    }
    Ok(())
}

fn validate_epoch_authority_pins(
    ingress: &EpochBoundSemanticIngress,
    catalog: &EpochBoundSemanticIngressCatalog,
) -> Result<(), EpochBoundSemanticIngressError> {
    for (kind, ingress_pin, catalog_pin) in [
        (
            "fabric_epoch_pin",
            ingress.fabric_epoch_pin,
            catalog.fabric_epoch_pin,
        ),
        (
            "program_catalog_pin",
            ingress.program_catalog_pin,
            catalog.program_catalog_pin,
        ),
        ("source_pin", ingress.source_pin, catalog.source_pin),
        ("policy_pin", ingress.policy_pin, catalog.policy_pin),
        (
            "producer_closure_proof_pin",
            ingress.producer_closure_proof_pin,
            catalog.producer_closure_proof_pin,
        ),
        ("limits_pin", ingress.limits_pin, catalog.limits_pin),
    ] {
        validate_epoch_pin(kind, ingress_pin)?;
        if ingress_pin != catalog_pin {
            return Err(EpochBoundSemanticIngressError::PinMismatch(kind));
        }
    }
    validate_epoch_pin("request_content_pin", ingress.request_content_pin)?;
    if ingress.limits_pin != epoch_bound_semantic_ingress_limits_pin(ingress.limits) {
        return Err(EpochBoundSemanticIngressError::PinMismatch(
            "limits_content",
        ));
    }
    Ok(())
}

fn epoch_bound_dependency_order(
    blocks: &BTreeMap<Arc<str>, &EpochBoundBlockBindingRow>,
    edges: &BTreeSet<(Arc<str>, Arc<str>)>,
) -> Result<Vec<Arc<str>>, EpochBoundSemanticIngressError> {
    let mut indegree = blocks
        .keys()
        .map(|query_id| (Arc::clone(query_id), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = blocks
        .keys()
        .map(|query_id| (Arc::clone(query_id), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (producer, consumer) in edges {
        if outgoing
            .get_mut(producer.as_ref())
            .expect("validated producer exists")
            .insert(Arc::clone(consumer))
        {
            *indegree
                .get_mut(consumer.as_ref())
                .expect("validated consumer exists") += 1;
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(query_id, _)| Arc::clone(query_id))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(blocks.len());
    while let Some(query_id) = ready.pop_first() {
        order.push(Arc::clone(&query_id));
        for consumer in &outgoing[query_id.as_ref()] {
            let degree = indegree
                .get_mut(consumer.as_ref())
                .expect("validated consumer exists");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(Arc::clone(consumer));
            }
        }
    }
    if order.len() != blocks.len() {
        return Err(EpochBoundSemanticIngressError::DependencyCycle);
    }
    Ok(order)
}

/// Validate and canonicalize one complete epoch-bound semantic ingress product.
///
/// Program selection is driven exclusively by the explicit `program_binding_id` and exact pin in
/// each block. Released form labels are checked only after that lookup. Every other ingress row is
/// joined to exactly one unique catalog binding, cardinality and ordinals are proved, and the
/// declared dependency order must equal the deterministic order derived from consumer-slot edges.
///
/// # Errors
///
/// Rejects missing or mismatched authority pins, incomplete/extra/duplicate relation rows,
/// incompatible typed values, invalid request-owned tuple schemas, bound violations, unknown
/// consumer slots, or divergent/cyclic topology.
#[allow(clippy::too_many_lines)]
pub fn validate_epoch_bound_semantic_ingress(
    mut ingress: EpochBoundSemanticIngress,
    catalog: &EpochBoundSemanticIngressCatalog,
) -> Result<ValidatedEpochBoundSemanticIngress, EpochBoundSemanticIngressError> {
    validate_epoch_identity("semantic request", &ingress.semantic_request_id)?;
    validate_epoch_authority_pins(&ingress, catalog)?;
    let validated_catalog = validate_epoch_bound_ingress_catalog(catalog, ingress.limits)?;
    let compiler_limits = ingress.limits.compiler();

    if ingress.blocks.is_empty() {
        return Err(EpochBoundSemanticIngressError::Limit {
            limit: "max_blocks",
            observed: 0,
            maximum: compiler_limits.max_blocks(),
        });
    }
    for (limit, observed, maximum) in [
        (
            "max_blocks",
            ingress.blocks.len(),
            compiler_limits.max_blocks(),
        ),
        (
            "max_dependencies",
            ingress.dependencies.len(),
            compiler_limits.max_dependencies(),
        ),
        (
            "max_selection_rows",
            ingress.selections.len(),
            ingress.limits.max_selection_rows(),
        ),
        (
            "max_return_rows",
            ingress.returns.len(),
            ingress.limits.max_return_rows(),
        ),
        (
            "max_scope_rows",
            ingress.scopes.len(),
            ingress.limits.max_scope_rows(),
        ),
        (
            "max_request_input_rows",
            ingress.request_inputs.len(),
            ingress.limits.max_request_input_rows(),
        ),
    ] {
        validate_epoch_limit(limit, observed, maximum)?;
    }

    let mut blocks = BTreeMap::new();
    let mut execution_programs = BTreeMap::new();
    for block in &ingress.blocks {
        for (kind, value) in [
            ("query", block.query_id.as_ref()),
            ("program binding", block.program_binding_id.as_ref()),
            ("output role", block.output_role_id.as_ref()),
        ] {
            validate_epoch_identity(kind, value)?;
        }
        validate_epoch_pin("program_binding_pin", block.program_binding_pin)?;
        if block
            .explicit_result_limit
            .is_some_and(|limit| limit == 0 || limit > compiler_limits.max_explicit_result_rows())
        {
            return Err(EpochBoundSemanticIngressError::Limit {
                limit: "max_explicit_result_rows",
                observed: block.explicit_result_limit.unwrap_or_default(),
                maximum: compiler_limits.max_explicit_result_rows(),
            });
        }
        let binding = validated_catalog
            .programs
            .get(block.program_binding_id.as_ref())
            .ok_or_else(|| EpochBoundSemanticIngressError::MissingProgramBinding {
                query_id: block.query_id.to_string(),
                program_binding_id: block.program_binding_id.to_string(),
            })?;
        for (matches, detail) in [
            (
                block.program_binding_pin == binding.program_binding_pin,
                "program binding pin",
            ),
            (
                block.compatibility_form == binding.compatibility_form,
                "compatibility form observation",
            ),
            (
                block.output_role_id == binding.output_role_id,
                "semantic output role",
            ),
        ] {
            if !matches {
                return Err(EpochBoundSemanticIngressError::ProgramBindingMismatch {
                    query_id: block.query_id.to_string(),
                    program_binding_id: block.program_binding_id.to_string(),
                    detail,
                });
            }
        }
        if blocks.insert(Arc::clone(&block.query_id), block).is_some() {
            return Err(epoch_duplicate("request block", block.query_id.to_string()));
        }
        execution_programs.insert(Arc::clone(&block.query_id), binding.execution_program_pin);
    }

    let mut selection_keys = BTreeSet::new();
    let mut selection_groups = BTreeMap::<(Arc<str>, Arc<str>), Vec<u32>>::new();
    for row in &ingress.selections {
        validate_epoch_identity("selection query", &row.query_id)?;
        validate_epoch_identity("selection", &row.selection_id)?;
        let block = blocks.get(row.query_id.as_ref()).ok_or_else(|| {
            EpochBoundSemanticIngressError::MissingBinding {
                query_id: row.query_id.to_string(),
                family: "selection",
                binding_id: row.selection_id.to_string(),
            }
        })?;
        let key = (
            Arc::clone(&block.program_binding_id),
            Arc::clone(&row.selection_id),
        );
        let binding = validated_catalog.selections.get(&key).ok_or_else(|| {
            EpochBoundSemanticIngressError::MissingBinding {
                query_id: row.query_id.to_string(),
                family: "selection",
                binding_id: row.selection_id.to_string(),
            }
        })?;
        if row.value.kind() != binding.value_kind {
            return Err(EpochBoundSemanticIngressError::MissingBinding {
                query_id: row.query_id.to_string(),
                family: "selection value kind",
                binding_id: row.selection_id.to_string(),
            });
        }
        if !selection_keys.insert((
            Arc::clone(&row.query_id),
            Arc::clone(&row.selection_id),
            row.ordinal,
        )) {
            return Err(epoch_duplicate(
                "selection row",
                format!("{}/{}/{}", row.query_id, row.selection_id, row.ordinal),
            ));
        }
        selection_groups
            .entry((Arc::clone(&row.query_id), Arc::clone(&row.selection_id)))
            .or_default()
            .push(row.ordinal);
    }
    for block in blocks.values() {
        for ((program_binding_id, selection_id), binding) in &validated_catalog.selections {
            if program_binding_id != &block.program_binding_id {
                continue;
            }
            let group_key = (Arc::clone(&block.query_id), Arc::clone(selection_id));
            let mut ordinals = selection_groups.remove(&group_key).unwrap_or_default();
            ordinals.sort_unstable();
            validate_epoch_cardinality(
                "selection",
                selection_id,
                ordinals.len(),
                binding.minimum_values,
                binding.maximum_values,
            )?;
            validate_epoch_ordinals("selection", selection_id, &ordinals)?;
        }
    }
    debug_assert!(selection_groups.is_empty());

    let mut return_keys = BTreeSet::new();
    let mut return_groups = BTreeMap::<(Arc<str>, Arc<str>), Vec<u32>>::new();
    for row in &ingress.returns {
        validate_epoch_identity("return query", &row.query_id)?;
        validate_epoch_identity("return", &row.return_id)?;
        let block = blocks.get(row.query_id.as_ref()).ok_or_else(|| {
            EpochBoundSemanticIngressError::MissingBinding {
                query_id: row.query_id.to_string(),
                family: "return",
                binding_id: row.return_id.to_string(),
            }
        })?;
        let key = (
            Arc::clone(&block.program_binding_id),
            Arc::clone(&row.return_id),
        );
        let binding = validated_catalog.returns.get(&key).ok_or_else(|| {
            EpochBoundSemanticIngressError::MissingBinding {
                query_id: row.query_id.to_string(),
                family: "return",
                binding_id: row.return_id.to_string(),
            }
        })?;
        if row.value.kind() != binding.value_kind {
            return Err(EpochBoundSemanticIngressError::MissingBinding {
                query_id: row.query_id.to_string(),
                family: "return value kind",
                binding_id: row.return_id.to_string(),
            });
        }
        if !return_keys.insert((
            Arc::clone(&row.query_id),
            Arc::clone(&row.return_id),
            row.ordinal,
        )) {
            return Err(epoch_duplicate(
                "return row",
                format!("{}/{}/{}", row.query_id, row.return_id, row.ordinal),
            ));
        }
        return_groups
            .entry((Arc::clone(&row.query_id), Arc::clone(&row.return_id)))
            .or_default()
            .push(row.ordinal);
    }
    for block in blocks.values() {
        for ((program_binding_id, return_id), binding) in &validated_catalog.returns {
            if program_binding_id != &block.program_binding_id {
                continue;
            }
            let group_key = (Arc::clone(&block.query_id), Arc::clone(return_id));
            let mut ordinals = return_groups.remove(&group_key).unwrap_or_default();
            ordinals.sort_unstable();
            validate_epoch_cardinality(
                "return",
                return_id,
                ordinals.len(),
                binding.minimum_values,
                binding.maximum_values,
            )?;
            validate_epoch_ordinals("return", return_id, &ordinals)?;
        }
    }
    debug_assert!(return_groups.is_empty());

    let mut scope_keys = BTreeSet::new();
    let mut scope_groups = BTreeMap::<Arc<str>, Vec<u32>>::new();
    for row in &ingress.scopes {
        validate_epoch_identity("scope", &row.scope_id)?;
        let binding = validated_catalog
            .scopes
            .get(row.scope_id.as_ref())
            .ok_or_else(|| {
                EpochBoundSemanticIngressError::MissingScopeBinding(row.scope_id.to_string())
            })?;
        if row.value.kind() != binding.value_kind {
            return Err(EpochBoundSemanticIngressError::MissingScopeBinding(
                row.scope_id.to_string(),
            ));
        }
        if !scope_keys.insert((Arc::clone(&row.scope_id), row.ordinal)) {
            return Err(epoch_duplicate(
                "scope row",
                format!("{}/{}", row.scope_id, row.ordinal),
            ));
        }
        scope_groups
            .entry(Arc::clone(&row.scope_id))
            .or_default()
            .push(row.ordinal);
    }
    for (scope_id, binding) in &validated_catalog.scopes {
        let mut ordinals = scope_groups.remove(scope_id).unwrap_or_default();
        ordinals.sort_unstable();
        validate_epoch_cardinality(
            "scope",
            scope_id,
            ordinals.len(),
            binding.minimum_values,
            binding.maximum_values,
        )?;
        validate_epoch_ordinals("scope", scope_id, &ordinals)?;
    }
    debug_assert!(scope_groups.is_empty());

    let mut request_input_keys = BTreeSet::new();
    let mut request_input_row_ids = BTreeSet::new();
    let mut request_input_groups = BTreeMap::<(Arc<str>, Arc<str>), Vec<u32>>::new();
    let mut request_input_field_count = 0_usize;
    for row in &ingress.request_inputs {
        validate_epoch_identity("request-input query", &row.query_id)?;
        validate_epoch_identity("request input", &row.input_id)?;
        validate_epoch_identity("request input row", &row.row_id)?;
        let block = blocks.get(row.query_id.as_ref()).ok_or_else(|| {
            EpochBoundSemanticIngressError::MissingBinding {
                query_id: row.query_id.to_string(),
                family: "request input",
                binding_id: row.input_id.to_string(),
            }
        })?;
        let key = (
            Arc::clone(&block.program_binding_id),
            Arc::clone(&row.input_id),
        );
        let binding = validated_catalog.request_inputs.get(&key).ok_or_else(|| {
            EpochBoundSemanticIngressError::MissingBinding {
                query_id: row.query_id.to_string(),
                family: "request input",
                binding_id: row.input_id.to_string(),
            }
        })?;
        if row.fields.is_empty()
            || row.fields.len() > ingress.limits.max_fields_per_request_input_row()
        {
            return Err(EpochBoundSemanticIngressError::Limit {
                limit: "max_fields_per_request_input_row",
                observed: row.fields.len(),
                maximum: ingress.limits.max_fields_per_request_input_row(),
            });
        }
        let mut observed_fields = BTreeSet::new();
        for field in &row.fields {
            if !observed_fields.insert(field.field_id.clone()) {
                return Err(EpochBoundSemanticIngressError::RequestInputField {
                    input_id: row.input_id.to_string(),
                    row_id: row.row_id.to_string(),
                });
            }
            let Some(field_binding) = binding
                .fields
                .iter()
                .find(|candidate| candidate.field_id == field.field_id)
            else {
                return Err(EpochBoundSemanticIngressError::RequestInputField {
                    input_id: row.input_id.to_string(),
                    row_id: row.row_id.to_string(),
                });
            };
            if field.value.kind() != field_binding.value_kind {
                return Err(EpochBoundSemanticIngressError::RequestInputField {
                    input_id: row.input_id.to_string(),
                    row_id: row.row_id.to_string(),
                });
            }
        }
        if binding
            .fields
            .iter()
            .any(|field| field.required && !observed_fields.contains(&field.field_id))
        {
            return Err(EpochBoundSemanticIngressError::RequestInputField {
                input_id: row.input_id.to_string(),
                row_id: row.row_id.to_string(),
            });
        }
        if !request_input_keys.insert((
            Arc::clone(&row.query_id),
            Arc::clone(&row.input_id),
            row.ordinal,
        )) || !request_input_row_ids.insert((
            Arc::clone(&row.query_id),
            Arc::clone(&row.input_id),
            Arc::clone(&row.row_id),
        )) {
            return Err(epoch_duplicate(
                "request input row",
                format!("{}/{}/{}", row.query_id, row.input_id, row.row_id),
            ));
        }
        request_input_field_count = request_input_field_count
            .checked_add(row.fields.len())
            .ok_or(EpochBoundSemanticIngressError::Limit {
                limit: "request_input_field_count",
                observed: usize::MAX,
                maximum: usize::MAX - 1,
            })?;
        request_input_groups
            .entry((Arc::clone(&row.query_id), Arc::clone(&row.input_id)))
            .or_default()
            .push(row.ordinal);
    }
    for block in blocks.values() {
        for ((program_binding_id, input_id), binding) in &validated_catalog.request_inputs {
            if program_binding_id != &block.program_binding_id {
                continue;
            }
            let group_key = (Arc::clone(&block.query_id), Arc::clone(input_id));
            let mut ordinals = request_input_groups.remove(&group_key).unwrap_or_default();
            ordinals.sort_unstable();
            validate_epoch_cardinality(
                "request input",
                input_id,
                ordinals.len(),
                binding.minimum_rows,
                binding.maximum_rows,
            )?;
            validate_epoch_ordinals("request input", input_id, &ordinals)?;
        }
    }
    debug_assert!(request_input_groups.is_empty());

    let mut dependency_keys = BTreeSet::new();
    let mut dependency_groups = BTreeMap::<(Arc<str>, Arc<str>), Vec<u32>>::new();
    let mut graph_edges = BTreeSet::new();
    let mut fanin = BTreeMap::<Arc<str>, usize>::new();
    let mut fanout = BTreeMap::<Arc<str>, usize>::new();
    for edge in &ingress.dependencies {
        for (kind, value) in [
            ("producer query", edge.producer_query_id.as_ref()),
            ("producer role", edge.producer_role_id.as_ref()),
            ("consumer query", edge.consumer_query_id.as_ref()),
            ("consumer slot", edge.consumer_slot_id.as_ref()),
            ("consumer role", edge.consumer_role_id.as_ref()),
        ] {
            validate_epoch_identity(kind, value)?;
        }
        let producer = blocks.get(edge.producer_query_id.as_ref()).ok_or_else(|| {
            EpochBoundSemanticIngressError::UnknownDependencyQuery(
                edge.producer_query_id.to_string(),
            )
        })?;
        let consumer = blocks.get(edge.consumer_query_id.as_ref()).ok_or_else(|| {
            EpochBoundSemanticIngressError::UnknownDependencyQuery(
                edge.consumer_query_id.to_string(),
            )
        })?;
        if producer.query_id == consumer.query_id {
            return Err(EpochBoundSemanticIngressError::DependencyCycle);
        }
        if producer.output_role_id != edge.producer_role_id {
            return Err(EpochBoundSemanticIngressError::ConsumerSlot {
                query_id: edge.producer_query_id.to_string(),
                slot_id: edge.producer_role_id.to_string(),
            });
        }
        let slot_key = (
            Arc::clone(&consumer.program_binding_id),
            Arc::clone(&edge.consumer_slot_id),
        );
        let slot = validated_catalog
            .consumer_slots
            .get(&slot_key)
            .ok_or_else(|| EpochBoundSemanticIngressError::ConsumerSlot {
                query_id: edge.consumer_query_id.to_string(),
                slot_id: edge.consumer_slot_id.to_string(),
            })?;
        if slot.consumer_role_id != edge.consumer_role_id {
            return Err(EpochBoundSemanticIngressError::ConsumerSlot {
                query_id: edge.consumer_query_id.to_string(),
                slot_id: edge.consumer_slot_id.to_string(),
            });
        }
        if !dependency_keys.insert((
            Arc::clone(&edge.consumer_query_id),
            Arc::clone(&edge.consumer_slot_id),
            edge.ordinal,
        )) {
            return Err(epoch_duplicate(
                "dependency consumer slot row",
                format!(
                    "{}/{}/{}",
                    edge.consumer_query_id, edge.consumer_slot_id, edge.ordinal
                ),
            ));
        }
        dependency_groups
            .entry((
                Arc::clone(&edge.consumer_query_id),
                Arc::clone(&edge.consumer_slot_id),
            ))
            .or_default()
            .push(edge.ordinal);
        graph_edges.insert((
            Arc::clone(&edge.producer_query_id),
            Arc::clone(&edge.consumer_query_id),
        ));
        *fanin
            .entry(Arc::clone(&edge.consumer_query_id))
            .or_default() += 1;
        *fanout
            .entry(Arc::clone(&edge.producer_query_id))
            .or_default() += 1;
    }
    for block in blocks.values() {
        for ((program_binding_id, slot_id), binding) in &validated_catalog.consumer_slots {
            if program_binding_id != &block.program_binding_id {
                continue;
            }
            let group_key = (Arc::clone(&block.query_id), Arc::clone(slot_id));
            let mut ordinals = dependency_groups.remove(&group_key).unwrap_or_default();
            ordinals.sort_unstable();
            validate_epoch_cardinality(
                "dependency",
                slot_id,
                ordinals.len(),
                binding.minimum_edges,
                binding.maximum_edges,
            )?;
            validate_epoch_ordinals("dependency", slot_id, &ordinals)?;
        }
    }
    debug_assert!(dependency_groups.is_empty());
    if let Some(observed) = fanin
        .values()
        .copied()
        .find(|observed| *observed > compiler_limits.max_fanin())
    {
        return Err(EpochBoundSemanticIngressError::Limit {
            limit: "max_fanin",
            observed,
            maximum: compiler_limits.max_fanin(),
        });
    }
    if let Some(observed) = fanout
        .values()
        .copied()
        .find(|observed| *observed > compiler_limits.max_fanout())
    {
        return Err(EpochBoundSemanticIngressError::Limit {
            limit: "max_fanout",
            observed,
            maximum: compiler_limits.max_fanout(),
        });
    }
    let dependency_order = epoch_bound_dependency_order(&blocks, &graph_edges)?;
    if dependency_order != ingress.dependency_order {
        return Err(EpochBoundSemanticIngressError::DependencyOrderMismatch);
    }

    let mut validated_request_inputs = Vec::new();
    for block in blocks.values() {
        for ((program_binding_id, input_id), binding) in &validated_catalog.request_inputs {
            if program_binding_id != &block.program_binding_id {
                continue;
            }
            let mut rows = ingress
                .request_inputs
                .iter()
                .filter(|row| row.query_id == block.query_id && row.input_id == *input_id)
                .cloned()
                .collect::<Vec<_>>();
            for row in &mut rows {
                row.fields
                    .sort_by(|left, right| left.field_id.cmp(&right.field_id));
            }
            rows.sort_by_key(|row| row.ordinal);
            validated_request_inputs.push(ValidatedEpochBoundRequestInput {
                query_id: Arc::clone(&block.query_id),
                input_id: Arc::clone(input_id),
                relation_id: binding.input_relation_id.clone(),
                fields: binding.fields.clone(),
                rows,
            });
        }
    }
    validated_request_inputs.sort_by(|left, right| {
        left.query_id
            .cmp(&right.query_id)
            .then_with(|| left.input_id.cmp(&right.input_id))
    });

    let consumption = EpochBoundIngressConsumption {
        blocks: ingress.blocks.len(),
        selections: ingress.selections.len(),
        returns: ingress.returns.len(),
        scopes: ingress.scopes.len(),
        request_input_rows: ingress.request_inputs.len(),
        request_input_fields: request_input_field_count,
        dependencies: ingress.dependencies.len(),
    };
    ingress
        .blocks
        .sort_by(|left, right| left.query_id.cmp(&right.query_id));
    ingress.selections.sort_by(|left, right| {
        left.query_id
            .cmp(&right.query_id)
            .then_with(|| left.selection_id.cmp(&right.selection_id))
            .then_with(|| left.ordinal.cmp(&right.ordinal))
    });
    ingress.returns.sort_by(|left, right| {
        left.query_id
            .cmp(&right.query_id)
            .then_with(|| left.return_id.cmp(&right.return_id))
            .then_with(|| left.ordinal.cmp(&right.ordinal))
    });
    ingress.scopes.sort_by(|left, right| {
        left.scope_id
            .cmp(&right.scope_id)
            .then_with(|| left.ordinal.cmp(&right.ordinal))
    });
    for row in &mut ingress.request_inputs {
        row.fields
            .sort_by(|left, right| left.field_id.cmp(&right.field_id));
    }
    ingress.request_inputs.sort_by(|left, right| {
        left.query_id
            .cmp(&right.query_id)
            .then_with(|| left.input_id.cmp(&right.input_id))
            .then_with(|| left.ordinal.cmp(&right.ordinal))
    });
    ingress.dependencies.sort();

    Ok(ValidatedEpochBoundSemanticIngress {
        ingress,
        execution_programs,
        request_inputs: validated_request_inputs,
        consumption,
    })
}

type EpochExecutionNodeKey = (Arc<str>, Arc<str>);

struct ValidatedEpochExecutionCatalog<'a> {
    programs: BTreeMap<Arc<str>, &'a EpochBoundExecutionProgramRow>,
    operators: BTreeMap<EpochExecutionNodeKey, &'a EpochBoundExecutionOperatorRow>,
    schemas: BTreeMap<RelationId, &'a ProgramRelationSchemaRow>,
    consumer_slots: BTreeMap<(Arc<str>, Arc<str>), &'a EpochBoundExecutionConsumerSlotRow>,
    selections: BTreeMap<(Arc<str>, Arc<str>), &'a EpochBoundExecutionSelectionRow>,
    returns: BTreeMap<(Arc<str>, Arc<str>, SemanticClauseValue), &'a EpochBoundExecutionReturnRow>,
    required_families: BTreeMap<Arc<str>, BTreeSet<Arc<str>>>,
    request_inputs: BTreeMap<(Arc<str>, Arc<str>), &'a EpochBoundExecutionRequestInputRow>,
    scopes: BTreeMap<Arc<str>, &'a EpochBoundExecutionScopeRow>,
    authority_id: Arc<str>,
    semantic_class_id: Arc<str>,
}

fn epoch_compile_duplicate(
    kind: &'static str,
    key: impl Into<String>,
) -> EpochBoundSemanticCompileError {
    EpochBoundSemanticCompileError::Duplicate {
        kind,
        key: key.into(),
    }
}

fn epoch_compile_pin(
    kind: &'static str,
    pin: [u8; 32],
) -> Result<(), EpochBoundSemanticCompileError> {
    if pin == [0; 32] {
        Err(EpochBoundSemanticCompileError::MissingPin(kind))
    } else {
        Ok(())
    }
}

fn epoch_compile_fields(
    node: &str,
    fields: &[FieldId],
    maximum: usize,
) -> Result<(), EpochBoundSemanticCompileError> {
    if fields.is_empty() || fields.len() > maximum {
        return Err(EpochBoundSemanticCompileError::InvalidNode {
            node: node.to_owned(),
            detail: format!("field count {} is outside 1..={maximum}", fields.len()),
        });
    }
    let mut unique = BTreeSet::new();
    for field in fields {
        if !unique.insert(field) {
            return Err(EpochBoundSemanticCompileError::InvalidNode {
                node: node.to_owned(),
                detail: format!("field {} is repeated", field.as_str()),
            });
        }
    }
    Ok(())
}

fn epoch_execution_operator_arity(row: &EpochBoundExecutionOperatorRow) -> bool {
    match &row.operator {
        ProgramRelationalOperator::Input { .. } => row.input_node_ids.is_empty(),
        ProgramRelationalOperator::Projection { .. }
        | ProgramRelationalOperator::Filter
        | ProgramRelationalOperator::Aggregate { .. }
        | ProgramRelationalOperator::Sort { .. }
        | ProgramRelationalOperator::Limit { .. } => row.input_node_ids.len() == 1,
        ProgramRelationalOperator::Join { .. } => row.input_node_ids.len() == 2,
        ProgramRelationalOperator::Union { .. } => !row.input_node_ids.is_empty(),
    }
}

fn validate_epoch_execution_node_fields(
    row: &EpochBoundExecutionOperatorRow,
    nodes: &BTreeMap<Arc<str>, &EpochBoundExecutionOperatorRow>,
    schemas: &BTreeMap<RelationId, &ProgramRelationSchemaRow>,
) -> Result<(), EpochBoundSemanticCompileError> {
    let input =
        |ordinal: usize| -> &EpochBoundExecutionOperatorRow { nodes[&row.input_node_ids[ordinal]] };
    let invalid = |detail: &str| EpochBoundSemanticCompileError::InvalidNode {
        node: row.node_id.to_string(),
        detail: detail.to_owned(),
    };
    match &row.operator {
        ProgramRelationalOperator::Input { relation_id } => {
            let schema = schemas
                .get(relation_id)
                .ok_or_else(|| invalid("input relation schema is unresolved"))?;
            if schema.fields != row.output_fields {
                return Err(invalid("input fields differ from relation schema"));
            }
        }
        ProgramRelationalOperator::Projection { fields } => {
            let source = input(0);
            let outputs = fields
                .iter()
                .map(|field| field.output_field_id.clone())
                .collect::<Vec<_>>();
            if outputs != row.output_fields
                || fields
                    .iter()
                    .any(|field| !source.output_fields.contains(&field.input_field_id))
            {
                return Err(invalid(
                    "projection mapping disagrees with its field contracts",
                ));
            }
        }
        ProgramRelationalOperator::Filter | ProgramRelationalOperator::Limit { .. } => {
            if input(0).output_fields != row.output_fields {
                return Err(invalid(
                    "row-preserving operator changed its field contract",
                ));
            }
        }
        ProgramRelationalOperator::Sort { fields } => {
            if input(0).output_fields != row.output_fields
                || fields
                    .iter()
                    .any(|field| !row.output_fields.contains(&field.input_field_id))
            {
                return Err(invalid("sort mapping disagrees with its field contract"));
            }
        }
        ProgramRelationalOperator::Join { predicates, .. } => {
            let left = input(0);
            let right = input(1);
            if predicates.iter().any(|predicate| {
                !left.output_fields.contains(&predicate.left_field_id)
                    || !right.output_fields.contains(&predicate.right_field_id)
                    || !is_binary_predicate(predicate.scalar_operator)
            }) {
                return Err(invalid("join predicate is invalid or out of scope"));
            }
        }
        ProgramRelationalOperator::Union { .. } => {
            if row
                .input_node_ids
                .iter()
                .any(|node| nodes[node].output_fields != row.output_fields)
            {
                return Err(invalid("union inputs do not have one common schema"));
            }
        }
        ProgramRelationalOperator::Aggregate {
            group_by,
            aggregates,
        } => {
            let source = input(0);
            let outputs = group_by
                .iter()
                .map(|field| field.output_field_id.clone())
                .chain(aggregates.iter().map(|field| field.output_field_id.clone()))
                .collect::<Vec<_>>();
            if outputs != row.output_fields
                || group_by
                    .iter()
                    .any(|field| !source.output_fields.contains(&field.input_field_id))
                || aggregates
                    .iter()
                    .any(|field| !source.output_fields.contains(&field.input_field_id))
            {
                return Err(invalid(
                    "aggregate mapping disagrees with its field contracts",
                ));
            }
        }
    }
    Ok(())
}

fn validate_epoch_execution_program(
    program: &EpochBoundExecutionProgramRow,
    catalog: &ValidatedEpochExecutionCatalog<'_>,
    limits: SemanticRequestLimits,
) -> Result<(), EpochBoundSemanticCompileError> {
    let mut nodes = catalog
        .operators
        .iter()
        .filter(|((program_binding_id, _), _)| program_binding_id == &program.program_binding_id)
        .map(|((_, node_id), row)| (Arc::clone(node_id), *row))
        .collect::<BTreeMap<_, _>>();
    if nodes.is_empty() || !nodes.contains_key(program.root_node_id.as_ref()) {
        return Err(EpochBoundSemanticCompileError::OutputSchema {
            program_binding_id: program.program_binding_id.to_string(),
            detail: "root operator node is missing".to_owned(),
        });
    }
    if nodes.len() > limits.max_operator_nodes_per_block() {
        return Err(EpochBoundSemanticCompileError::InvalidNode {
            node: program.root_node_id.to_string(),
            detail: format!(
                "operator count {} exceeds {}",
                nodes.len(),
                limits.max_operator_nodes_per_block()
            ),
        });
    }
    let mut ordinals = BTreeSet::new();
    for row in nodes.values() {
        if !ordinals.insert(row.ordinal) {
            return Err(EpochBoundSemanticCompileError::InvalidNode {
                node: row.node_id.to_string(),
                detail: "operator ordinal is repeated".to_owned(),
            });
        }
        for input in &row.input_node_ids {
            let dependency = nodes.get(input.as_ref()).ok_or_else(|| {
                EpochBoundSemanticCompileError::InvalidNode {
                    node: row.node_id.to_string(),
                    detail: "operator input node is unresolved".to_owned(),
                }
            })?;
            if dependency.ordinal >= row.ordinal {
                return Err(EpochBoundSemanticCompileError::InvalidNode {
                    node: row.node_id.to_string(),
                    detail: "operator input must precede its consumer".to_owned(),
                });
            }
        }
        validate_epoch_execution_node_fields(row, &nodes, &catalog.schemas)?;
    }
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::from([Arc::clone(&program.root_node_id)]);
    while let Some(node_id) = queue.pop_front() {
        if !reachable.insert(Arc::clone(&node_id)) {
            continue;
        }
        let node = nodes.get(node_id.as_ref()).ok_or_else(|| {
            EpochBoundSemanticCompileError::InvalidNode {
                node: node_id.to_string(),
                detail: "reachable operator is unresolved".to_owned(),
            }
        })?;
        queue.extend(node.input_node_ids.iter().cloned());
    }
    if reachable.len() != nodes.len() {
        return Err(EpochBoundSemanticCompileError::InvalidNode {
            node: program.root_node_id.to_string(),
            detail: "program contains nodes that do not contribute to its root".to_owned(),
        });
    }
    let root = nodes
        .remove(program.root_node_id.as_ref())
        .expect("validated root exists");
    if root.output_fields != program.output_fields {
        return Err(EpochBoundSemanticCompileError::OutputSchema {
            program_binding_id: program.program_binding_id.to_string(),
            detail: "root fields differ from program output fields".to_owned(),
        });
    }
    let schema = catalog
        .schemas
        .get(&program.output_relation_id)
        .ok_or_else(|| EpochBoundSemanticCompileError::OutputSchema {
            program_binding_id: program.program_binding_id.to_string(),
            detail: "output relation schema is absent".to_owned(),
        })?;
    if schema.fields != program.output_fields {
        return Err(EpochBoundSemanticCompileError::OutputSchema {
            program_binding_id: program.program_binding_id.to_string(),
            detail: "output relation fields differ from program output fields".to_owned(),
        });
    }
    Ok(())
}

fn validate_epoch_execution_catalog<'a>(
    ingress: &ValidatedEpochBoundSemanticIngress,
    catalog: &'a EpochBoundSemanticExecutionCatalog,
) -> Result<ValidatedEpochExecutionCatalog<'a>, EpochBoundSemanticCompileError> {
    let request = ingress.ingress();
    for (kind, value, expected) in [
        (
            "fabric_epoch_pin",
            catalog.fabric_epoch_pin,
            request.fabric_epoch_pin,
        ),
        (
            "program_catalog_pin",
            catalog.program_catalog_pin,
            request.program_catalog_pin,
        ),
        ("source_pin", catalog.source_pin, request.source_pin),
        ("policy_pin", catalog.policy_pin, request.policy_pin),
        (
            "producer_closure_proof_pin",
            catalog.producer_closure_proof_pin,
            request.producer_closure_proof_pin,
        ),
    ] {
        epoch_compile_pin(kind, value)?;
        if value != expected {
            return Err(EpochBoundSemanticCompileError::PinMismatch(kind));
        }
    }
    epoch_compile_pin("execution_catalog_pin", catalog.execution_catalog_pin)?;
    epoch_compile_pin("program_release_pin", catalog.program_release_pin)?;
    let authority_id = match &catalog.authority {
        SemanticQueryAuthority::ApplicationOwned(value) => {
            validate_epoch_identity("execution authority", value)
                .map_err(|_| EpochBoundSemanticCompileError::MissingProgram(value.to_string()))?;
            Arc::clone(value)
        }
        SemanticQueryAuthority::ProviderNative(value) => {
            return Err(EpochBoundSemanticCompileError::MissingProgram(format!(
                "provider-native authority {value}"
            )));
        }
    };
    let semantic_class_id = match &catalog.semantic_class {
        SemanticQueryClass::Fact(value) => {
            validate_epoch_identity("semantic class", value)
                .map_err(|_| EpochBoundSemanticCompileError::MissingProgram(value.to_string()))?;
            Arc::clone(value)
        }
        SemanticQueryClass::Judgment(value) => {
            return Err(EpochBoundSemanticCompileError::MissingProgram(format!(
                "judgment semantic class {value}"
            )));
        }
    };

    let limits = request.limits.compiler();
    let mut schemas = BTreeMap::new();
    for row in &catalog.relation_schemas {
        epoch_compile_fields(
            row.relation_id.as_str(),
            &row.fields,
            limits.max_fields_per_node(),
        )?;
        if schemas.insert(row.relation_id.clone(), row).is_some() {
            return Err(epoch_compile_duplicate(
                "relation schema",
                row.relation_id.as_str().to_owned(),
            ));
        }
    }
    let mut programs = BTreeMap::new();
    for row in &catalog.programs {
        validate_epoch_identity("program binding", &row.program_binding_id).map_err(|_| {
            EpochBoundSemanticCompileError::MissingProgram(row.program_binding_id.to_string())
        })?;
        validate_epoch_identity("program root", &row.root_node_id).map_err(|_| {
            EpochBoundSemanticCompileError::MissingProgram(row.program_binding_id.to_string())
        })?;
        epoch_compile_pin("execution_program_pin", row.execution_program_pin)?;
        epoch_compile_fields(
            &row.root_node_id,
            &row.output_fields,
            limits.max_fields_per_node(),
        )?;
        if programs
            .insert(Arc::clone(&row.program_binding_id), row)
            .is_some()
        {
            return Err(epoch_compile_duplicate(
                "program",
                row.program_binding_id.to_string(),
            ));
        }
    }

    let mut operators = BTreeMap::new();
    for row in &catalog.operators {
        validate_epoch_identity("operator program", &row.program_binding_id).map_err(|_| {
            EpochBoundSemanticCompileError::MissingProgram(row.program_binding_id.to_string())
        })?;
        validate_epoch_identity("operator node", &row.node_id).map_err(|_| {
            EpochBoundSemanticCompileError::InvalidNode {
                node: row.node_id.to_string(),
                detail: "node identity is invalid".to_owned(),
            }
        })?;
        let program = programs
            .get(row.program_binding_id.as_ref())
            .ok_or_else(|| {
                EpochBoundSemanticCompileError::MissingProgram(row.program_binding_id.to_string())
            })?;
        if row.execution_program_pin != program.execution_program_pin {
            return Err(
                EpochBoundSemanticCompileError::ExecutionProgramPinMismatch {
                    program_binding_id: row.program_binding_id.to_string(),
                },
            );
        }
        epoch_compile_fields(
            &row.node_id,
            &row.output_fields,
            limits.max_fields_per_node(),
        )?;
        if !epoch_execution_operator_arity(row) {
            return Err(EpochBoundSemanticCompileError::InvalidNode {
                node: row.node_id.to_string(),
                detail: "operator input arity is invalid".to_owned(),
            });
        }
        let mut inputs = BTreeSet::new();
        for input in &row.input_node_ids {
            if !inputs.insert(input) {
                return Err(EpochBoundSemanticCompileError::InvalidNode {
                    node: row.node_id.to_string(),
                    detail: "operator input node is repeated".to_owned(),
                });
            }
        }
        let key = (
            Arc::clone(&row.program_binding_id),
            Arc::clone(&row.node_id),
        );
        if operators.insert(key, row).is_some() {
            return Err(epoch_compile_duplicate(
                "operator node",
                format!("{}/{}", row.program_binding_id, row.node_id),
            ));
        }
    }

    let mut consumer_slots = BTreeMap::new();
    for row in &catalog.consumer_slots {
        let program = programs
            .get(row.program_binding_id.as_ref())
            .ok_or_else(|| {
                EpochBoundSemanticCompileError::MissingProgram(row.program_binding_id.to_string())
            })?;
        if row.execution_program_pin != program.execution_program_pin {
            return Err(
                EpochBoundSemanticCompileError::ExecutionProgramPinMismatch {
                    program_binding_id: row.program_binding_id.to_string(),
                },
            );
        }
        validate_epoch_identity("consumer slot", &row.consumer_slot_id).map_err(|_| {
            EpochBoundSemanticCompileError::MissingBinding {
                query_id: row.program_binding_id.to_string(),
                family: "consumer slot",
                binding_id: row.consumer_slot_id.to_string(),
            }
        })?;
        validate_epoch_identity("consumer role", &row.consumer_role_id).map_err(|_| {
            EpochBoundSemanticCompileError::MissingBinding {
                query_id: row.program_binding_id.to_string(),
                family: "consumer role",
                binding_id: row.consumer_role_id.to_string(),
            }
        })?;
        if !schemas.contains_key(&row.input_relation_id) {
            return Err(EpochBoundSemanticCompileError::OutputSchema {
                program_binding_id: row.program_binding_id.to_string(),
                detail: format!(
                    "consumer slot relation {} has no schema",
                    row.input_relation_id.as_str()
                ),
            });
        }
        let key = (
            Arc::clone(&row.program_binding_id),
            Arc::clone(&row.consumer_slot_id),
        );
        if consumer_slots.insert(key, row).is_some() {
            return Err(epoch_compile_duplicate(
                "consumer slot",
                format!("{}/{}", row.program_binding_id, row.consumer_slot_id),
            ));
        }
    }

    let mut selections = BTreeMap::new();
    for row in &catalog.selections {
        let program = programs
            .get(row.program_binding_id.as_ref())
            .ok_or_else(|| {
                EpochBoundSemanticCompileError::MissingProgram(row.program_binding_id.to_string())
            })?;
        if row.execution_program_pin != program.execution_program_pin {
            return Err(
                EpochBoundSemanticCompileError::ExecutionProgramPinMismatch {
                    program_binding_id: row.program_binding_id.to_string(),
                },
            );
        }
        if !is_binary_predicate(row.scalar_operator) {
            return Err(EpochBoundSemanticCompileError::InvalidNode {
                node: row.operator_node_id.to_string(),
                detail: "selection lowering requires a binary scalar predicate".to_owned(),
            });
        }
        let key = (
            Arc::clone(&row.program_binding_id),
            Arc::clone(&row.selection_id),
        );
        if selections.insert(key, row).is_some() {
            return Err(epoch_compile_duplicate(
                "selection lowering",
                format!("{}/{}", row.program_binding_id, row.selection_id),
            ));
        }
    }

    let mut returns = BTreeMap::new();
    for row in &catalog.returns {
        let program = programs
            .get(row.program_binding_id.as_ref())
            .ok_or_else(|| {
                EpochBoundSemanticCompileError::MissingProgram(row.program_binding_id.to_string())
            })?;
        if row.execution_program_pin != program.execution_program_pin {
            return Err(
                EpochBoundSemanticCompileError::ExecutionProgramPinMismatch {
                    program_binding_id: row.program_binding_id.to_string(),
                },
            );
        }
        epoch_compile_pin("return realization pin", row.realization_pin)?;
        if row.realization_field_ids.is_empty() {
            return Err(EpochBoundSemanticCompileError::MissingBinding {
                query_id: row.program_binding_id.to_string(),
                family: "return realization fields",
                binding_id: row.return_id.to_string(),
            });
        }
        let key = (
            Arc::clone(&row.program_binding_id),
            Arc::clone(&row.return_id),
            row.value.clone(),
        );
        if returns.insert(key, row).is_some() {
            return Err(epoch_compile_duplicate(
                "return realization",
                format!(
                    "{}/{}:{:?}",
                    row.program_binding_id, row.return_id, row.value
                ),
            ));
        }
    }

    let mut required_families = BTreeMap::<Arc<str>, BTreeSet<Arc<str>>>::new();
    for row in &catalog.required_fact_families {
        let program = programs
            .get(row.program_binding_id.as_ref())
            .ok_or_else(|| {
                EpochBoundSemanticCompileError::MissingProgram(row.program_binding_id.to_string())
            })?;
        if row.execution_program_pin != program.execution_program_pin {
            return Err(
                EpochBoundSemanticCompileError::ExecutionProgramPinMismatch {
                    program_binding_id: row.program_binding_id.to_string(),
                },
            );
        }
        if !required_families
            .entry(Arc::clone(&row.program_binding_id))
            .or_default()
            .insert(Arc::clone(&row.family_id))
        {
            return Err(epoch_compile_duplicate(
                "required family",
                format!("{}/{}", row.program_binding_id, row.family_id),
            ));
        }
    }

    let mut request_inputs = BTreeMap::new();
    for row in &catalog.request_inputs {
        let program = programs
            .get(row.program_binding_id.as_ref())
            .ok_or_else(|| {
                EpochBoundSemanticCompileError::MissingProgram(row.program_binding_id.to_string())
            })?;
        if row.execution_program_pin != program.execution_program_pin {
            return Err(
                EpochBoundSemanticCompileError::ExecutionProgramPinMismatch {
                    program_binding_id: row.program_binding_id.to_string(),
                },
            );
        }
        epoch_compile_pin("request input handoff pin", row.handoff_pin)?;
        let schema = schemas.get(&row.input_relation_id).ok_or_else(|| {
            EpochBoundSemanticCompileError::OutputSchema {
                program_binding_id: row.program_binding_id.to_string(),
                detail: format!(
                    "request input relation {} has no schema",
                    row.input_relation_id.as_str()
                ),
            }
        })?;
        if row
            .fields
            .iter()
            .map(|field| &field.field_id)
            .ne(schema.fields.iter())
        {
            return Err(EpochBoundSemanticCompileError::OutputSchema {
                program_binding_id: row.program_binding_id.to_string(),
                detail: format!(
                    "request input {} fields differ from its schema",
                    row.input_id
                ),
            });
        }
        let key = (
            Arc::clone(&row.program_binding_id),
            Arc::clone(&row.input_id),
        );
        if request_inputs.insert(key, row).is_some() {
            return Err(epoch_compile_duplicate(
                "request input handoff",
                format!("{}/{}", row.program_binding_id, row.input_id),
            ));
        }
    }

    let mut scopes = BTreeMap::new();
    for row in &catalog.scopes {
        validate_epoch_identity("scope", &row.scope_id).map_err(|_| {
            EpochBoundSemanticCompileError::MissingBinding {
                query_id: "request".to_owned(),
                family: "scope",
                binding_id: row.scope_id.to_string(),
            }
        })?;
        validate_epoch_identity("authorization input", &row.authorization_input_id).map_err(
            |_| EpochBoundSemanticCompileError::MissingBinding {
                query_id: "request".to_owned(),
                family: "scope authorization input",
                binding_id: row.authorization_input_id.to_string(),
            },
        )?;
        epoch_compile_pin("scope handoff pin", row.handoff_pin)?;
        if scopes.insert(Arc::clone(&row.scope_id), row).is_some() {
            return Err(epoch_compile_duplicate(
                "scope handoff",
                row.scope_id.to_string(),
            ));
        }
    }

    let validated = ValidatedEpochExecutionCatalog {
        programs,
        operators,
        schemas,
        consumer_slots,
        selections,
        returns,
        required_families,
        request_inputs,
        scopes,
        authority_id,
        semantic_class_id,
    };
    for program in validated.programs.values() {
        validate_epoch_execution_program(program, &validated, limits)?;
    }
    for selection in validated.selections.values() {
        let node = validated
            .operators
            .get(&(
                Arc::clone(&selection.program_binding_id),
                Arc::clone(&selection.operator_node_id),
            ))
            .ok_or_else(|| EpochBoundSemanticCompileError::InvalidNode {
                node: selection.operator_node_id.to_string(),
                detail: "selection filter node is unresolved".to_owned(),
            })?;
        if !matches!(node.operator, ProgramRelationalOperator::Filter) {
            return Err(EpochBoundSemanticCompileError::InvalidNode {
                node: selection.operator_node_id.to_string(),
                detail: "selection is not attached to a filter node".to_owned(),
            });
        }
        let input = validated.operators[&(
            Arc::clone(&selection.program_binding_id),
            Arc::clone(&node.input_node_ids[0]),
        )];
        if !input.output_fields.contains(&selection.input_field_id) {
            return Err(EpochBoundSemanticCompileError::InvalidNode {
                node: selection.operator_node_id.to_string(),
                detail: "selection field is absent from filter input".to_owned(),
            });
        }
    }
    for realization in validated.returns.values() {
        let node = validated
            .operators
            .get(&(
                Arc::clone(&realization.program_binding_id),
                Arc::clone(&realization.realization_node_id),
            ))
            .ok_or_else(|| EpochBoundSemanticCompileError::InvalidNode {
                node: realization.realization_node_id.to_string(),
                detail: "return realization node is unresolved".to_owned(),
            })?;
        if realization
            .realization_field_ids
            .iter()
            .any(|field| !node.output_fields.contains(field))
        {
            return Err(EpochBoundSemanticCompileError::InvalidNode {
                node: realization.realization_node_id.to_string(),
                detail: "return realization field is absent from its node".to_owned(),
            });
        }
    }
    for slot in validated.consumer_slots.values() {
        let consumed = validated.operators.values().any(|operator| {
            operator.program_binding_id == slot.program_binding_id
                && matches!(
                    &operator.operator,
                    ProgramRelationalOperator::Input { relation_id }
                        if relation_id == &slot.input_relation_id
                )
        });
        if !consumed {
            return Err(EpochBoundSemanticCompileError::MissingBinding {
                query_id: slot.program_binding_id.to_string(),
                family: "consumer slot input relation",
                binding_id: slot.consumer_slot_id.to_string(),
            });
        }
    }
    for input in validated.request_inputs.values() {
        let consumed = validated.operators.values().any(|operator| {
            operator.program_binding_id == input.program_binding_id
                && matches!(
                    &operator.operator,
                    ProgramRelationalOperator::Input { relation_id }
                        if relation_id == &input.input_relation_id
                )
        });
        if !consumed {
            return Err(EpochBoundSemanticCompileError::RequestInputHandoff {
                query_id: input.program_binding_id.to_string(),
                input_id: input.input_id.to_string(),
            });
        }
    }
    Ok(validated)
}

fn validate_epoch_compile_closure<'a>(
    proof: &'a ProducerClosureProof,
    expected_pin: [u8; 32],
) -> Result<BTreeMap<Arc<str>, ClosureStatus<'a>>, EpochBoundSemanticCompileError> {
    epoch_compile_pin("producer closure proof", proof.proof_pin)?;
    if proof.proof_pin != expected_pin {
        return Err(EpochBoundSemanticCompileError::PinMismatch(
            "producer_closure_proof_pin",
        ));
    }
    validate_epoch_identity(
        "producer closure authority",
        &proof.application_authority_id,
    )
    .map_err(|_| EpochBoundSemanticCompileError::ProducerClosure {
        family: "authority".to_owned(),
        detail: "application authority identity is invalid".to_owned(),
    })?;
    let mut families = BTreeMap::new();
    for row in &proof.families {
        validate_epoch_identity("producer family", &row.family_id).map_err(|_| {
            EpochBoundSemanticCompileError::ProducerClosure {
                family: row.family_id.to_string(),
                detail: "family identity is invalid".to_owned(),
            }
        })?;
        let status = match &row.disposition {
            ProducerFamilyDisposition::RuntimeProducer(runtime) => {
                for (kind, value) in [
                    ("producer", runtime.producer_id.as_ref()),
                    ("producer authority", runtime.authority_id.as_ref()),
                    ("producer algorithm", runtime.algorithm_release.as_ref()),
                    ("producer precision", runtime.precision_id.as_ref()),
                ] {
                    validate_epoch_identity(kind, value).map_err(|_| {
                        EpochBoundSemanticCompileError::ProducerClosure {
                            family: row.family_id.to_string(),
                            detail: format!("{kind} identity is invalid"),
                        }
                    })?;
                }
                if runtime.authority_id != proof.application_authority_id {
                    return Err(EpochBoundSemanticCompileError::ProducerClosure {
                        family: row.family_id.to_string(),
                        detail: "runtime producer authority is not application-owned".to_owned(),
                    });
                }
                for (kind, pin) in [
                    ("producer input", runtime.input_pin),
                    ("producer invalidation", runtime.invalidation_pin),
                    ("producer materialization", runtime.materialization_pin),
                    ("producer completeness", runtime.completeness_proof_pin),
                    ("producer proof", runtime.producer_proof_pin),
                ] {
                    epoch_compile_pin(kind, pin)?;
                }
                if runtime.requested_units != runtime.completed_units
                    || runtime.remainder_units != 0
                    || runtime.unknown_units != 0
                {
                    return Err(EpochBoundSemanticCompileError::ProducerClosure {
                        family: row.family_id.to_string(),
                        detail: "runtime producer completeness census is not exact".to_owned(),
                    });
                }
                ClosureStatus::Runtime(runtime)
            }
            ProducerFamilyDisposition::UnsupportedRemainder(remainder) => {
                for (kind, value) in [
                    ("remainder", remainder.remainder_id.as_ref()),
                    ("remainder authority", remainder.authority_id.as_ref()),
                    ("remainder reason", remainder.reason_id.as_ref()),
                ] {
                    validate_epoch_identity(kind, value).map_err(|_| {
                        EpochBoundSemanticCompileError::ProducerClosure {
                            family: row.family_id.to_string(),
                            detail: format!("{kind} identity is invalid"),
                        }
                    })?;
                }
                if remainder.authority_id != proof.application_authority_id {
                    return Err(EpochBoundSemanticCompileError::ProducerClosure {
                        family: row.family_id.to_string(),
                        detail: "unsupported remainder authority is not application-owned"
                            .to_owned(),
                    });
                }
                epoch_compile_pin("unsupported remainder proof", remainder.proof_pin)?;
                ClosureStatus::Remainder(remainder)
            }
        };
        if families
            .insert(Arc::clone(&row.family_id), status)
            .is_some()
        {
            return Err(EpochBoundSemanticCompileError::ProducerClosure {
                family: row.family_id.to_string(),
                detail: "family has multiple closure dispositions".to_owned(),
            });
        }
    }
    Ok(families)
}

fn compile_epoch_runtime_handoff(
    ingress: &ValidatedEpochBoundSemanticIngress,
    catalog: &ValidatedEpochExecutionCatalog<'_>,
    blocks: &BTreeMap<Arc<str>, &EpochBoundBlockBindingRow>,
    dependencies: &mut BTreeSet<SemanticCompilerDependency>,
) -> Result<EpochBoundSemanticRuntimeHandoff, EpochBoundSemanticCompileError> {
    let mut request_inputs = Vec::new();
    let mut observed_inputs = BTreeSet::new();
    for input in ingress.request_inputs() {
        let block = blocks
            .get(input.query_id.as_ref())
            .expect("validated request input query exists");
        let key = (
            Arc::clone(&block.program_binding_id),
            Arc::clone(&input.input_id),
        );
        let binding = catalog.request_inputs.get(&key).ok_or_else(|| {
            EpochBoundSemanticCompileError::RequestInputHandoff {
                query_id: input.query_id.to_string(),
                input_id: input.input_id.to_string(),
            }
        })?;
        if binding.input_relation_id != input.relation_id || binding.fields != input.fields {
            return Err(EpochBoundSemanticCompileError::RequestInputHandoff {
                query_id: input.query_id.to_string(),
                input_id: input.input_id.to_string(),
            });
        }
        let content_pin = debug_pin(
            b"epoch-bound-request-input-handoff",
            &(input.relation_id.clone(), &input.fields, &input.rows),
        );
        dependencies.insert(SemanticCompilerDependency::RequestInputHandoff {
            input_id: Arc::clone(&input.input_id),
            handoff_pin: binding.handoff_pin,
            content_pin,
        });
        observed_inputs.insert(key);
        request_inputs.push(CompiledEpochBoundRequestInputHandoff {
            query_id: Arc::clone(&input.query_id),
            program_binding_id: Arc::clone(&block.program_binding_id),
            execution_program_pin: catalog.programs[block.program_binding_id.as_ref()]
                .execution_program_pin,
            input_id: Arc::clone(&input.input_id),
            relation_id: input.relation_id.clone(),
            fields: input.fields.clone(),
            rows: input.rows.clone(),
            handoff_pin: binding.handoff_pin,
            content_pin,
        });
    }
    for ((program_binding_id, input_id), _) in &catalog.request_inputs {
        let used_program = blocks
            .values()
            .any(|block| block.program_binding_id == *program_binding_id);
        if used_program
            && !observed_inputs.contains(&(Arc::clone(program_binding_id), Arc::clone(input_id)))
        {
            return Err(EpochBoundSemanticCompileError::RequestInputHandoff {
                query_id: program_binding_id.to_string(),
                input_id: input_id.to_string(),
            });
        }
    }
    request_inputs.sort_by(|left, right| {
        left.query_id
            .cmp(&right.query_id)
            .then_with(|| left.input_id.cmp(&right.input_id))
    });

    let mut scope_groups = BTreeMap::<Arc<str>, Vec<EpochBoundScopeRow>>::new();
    for row in &ingress.ingress().scopes {
        scope_groups
            .entry(Arc::clone(&row.scope_id))
            .or_default()
            .push(row.clone());
    }
    let mut scopes = Vec::new();
    for (scope_id, rows) in scope_groups {
        let binding = catalog.scopes.get(scope_id.as_ref()).ok_or_else(|| {
            EpochBoundSemanticCompileError::MissingBinding {
                query_id: ingress.ingress().semantic_request_id.to_string(),
                family: "scope runtime handoff",
                binding_id: scope_id.to_string(),
            }
        })?;
        let content_pin = debug_pin(b"epoch-bound-scope-handoff", &rows);
        dependencies.insert(SemanticCompilerDependency::ScopeHandoff {
            scope_id: Arc::clone(&scope_id),
            handoff_pin: binding.handoff_pin,
            content_pin,
        });
        scopes.push(CompiledEpochBoundScopeHandoff {
            scope_id,
            authorization_input_id: Arc::clone(&binding.authorization_input_id),
            rows,
            handoff_pin: binding.handoff_pin,
            content_pin,
        });
    }
    scopes.sort_by(|left, right| left.scope_id.cmp(&right.scope_id));
    Ok(EpochBoundSemanticRuntimeHandoff {
        fabric_epoch_pin: ingress.ingress().fabric_epoch_pin,
        program_catalog_pin: ingress.ingress().program_catalog_pin,
        policy_pin: ingress.ingress().policy_pin,
        request_inputs,
        scopes,
    })
}

struct EpochCompiledRoot {
    relation_id: RelationId,
    fields: Vec<FieldId>,
    expression: RelationalExpression,
}

fn epoch_composition_inputs(
    block: &EpochBoundBlockBindingRow,
    request: &EpochBoundSemanticIngress,
    blocks: &BTreeMap<Arc<str>, &EpochBoundBlockBindingRow>,
    catalog: &ValidatedEpochExecutionCatalog<'_>,
    roots: &BTreeMap<Arc<str>, EpochCompiledRoot>,
    dependencies: &mut BTreeSet<SemanticCompilerDependency>,
) -> Result<BTreeMap<RelationId, RelationalExpression>, EpochBoundSemanticCompileError> {
    let mut by_slot = BTreeMap::<Arc<str>, Vec<&EpochBoundDependencyRow>>::new();
    for edge in request
        .dependencies
        .iter()
        .filter(|edge| edge.consumer_query_id == block.query_id)
    {
        by_slot
            .entry(Arc::clone(&edge.consumer_slot_id))
            .or_default()
            .push(edge);
    }
    let mut inputs = BTreeMap::new();
    for (slot_id, mut edges) in by_slot {
        edges.sort_by_key(|edge| edge.ordinal);
        let binding = catalog
            .consumer_slots
            .get(&(Arc::clone(&block.program_binding_id), Arc::clone(&slot_id)))
            .ok_or_else(|| EpochBoundSemanticCompileError::MissingBinding {
                query_id: block.query_id.to_string(),
                family: "consumer slot execution",
                binding_id: slot_id.to_string(),
            })?;
        dependencies.insert(SemanticCompilerDependency::ConsumerSlot {
            consumer_slot_id: Arc::clone(&slot_id),
            binding_pin: debug_pin(b"epoch-bound-consumer-slot", binding),
        });
        let expected = &catalog.schemas[&binding.input_relation_id].fields;
        let mut expressions = Vec::with_capacity(edges.len());
        for edge in edges {
            let producer_block = blocks
                .get(edge.producer_query_id.as_ref())
                .expect("validated producer block exists");
            if producer_block.output_role_id != edge.producer_role_id
                || binding.consumer_role_id != edge.consumer_role_id
            {
                return Err(EpochBoundSemanticCompileError::MissingBinding {
                    query_id: block.query_id.to_string(),
                    family: "consumer slot role",
                    binding_id: slot_id.to_string(),
                });
            }
            let producer = roots.get(edge.producer_query_id.as_ref()).ok_or_else(|| {
                EpochBoundSemanticCompileError::DependencyUnavailable(
                    edge.producer_query_id.to_string(),
                )
            })?;
            if producer.relation_id != binding.input_relation_id || producer.fields != *expected {
                return Err(EpochBoundSemanticCompileError::OutputSchema {
                    program_binding_id: block.program_binding_id.to_string(),
                    detail: format!(
                        "producer {} relation/schema differs from consumer slot {}",
                        edge.producer_query_id, slot_id
                    ),
                });
            }
            expressions.push(producer.expression.clone());
            dependencies.insert(SemanticCompilerDependency::CompositionEdge(debug_pin(
                b"epoch-bound-composition-edge",
                edge,
            )));
        }
        let expression = match binding.composition {
            EpochBoundConsumerComposition::Single if expressions.len() == 1 => {
                expressions.pop().expect("one expression")
            }
            EpochBoundConsumerComposition::Single => {
                return Err(EpochBoundSemanticCompileError::MissingBinding {
                    query_id: block.query_id.to_string(),
                    family: "single consumer slot cardinality",
                    binding_id: slot_id.to_string(),
                });
            }
            EpochBoundConsumerComposition::Union(kind) => RelationalExpression::Union {
                inputs: expressions,
                kind,
            },
        };
        if inputs
            .insert(binding.input_relation_id.clone(), expression)
            .is_some()
        {
            return Err(epoch_compile_duplicate(
                "consumer input relation",
                binding.input_relation_id.as_str().to_owned(),
            ));
        }
    }
    Ok(inputs)
}

fn fold_epoch_predicates(
    mut predicates: VecDeque<ScalarExpression>,
    operator: ScalarOperator,
) -> Option<ScalarExpression> {
    let first = predicates.pop_front()?;
    Some(
        predicates
            .into_iter()
            .fold(first, |left, right| ScalarExpression::Call {
                operator,
                arguments: vec![left, right],
            }),
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn lower_epoch_execution_program(
    block: &EpochBoundBlockBindingRow,
    program: &EpochBoundExecutionProgramRow,
    request: &EpochBoundSemanticIngress,
    catalog: &ValidatedEpochExecutionCatalog<'_>,
    composition_inputs: &BTreeMap<RelationId, RelationalExpression>,
    dependencies: &mut BTreeSet<SemanticCompilerDependency>,
    selected_operators: &mut BTreeSet<SemanticCompilerOperator>,
) -> Result<RelationalProgram, EpochBoundSemanticCompileError> {
    let mut rows = catalog
        .operators
        .values()
        .filter(|row| row.program_binding_id == block.program_binding_id)
        .copied()
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.ordinal
            .cmp(&right.ordinal)
            .then_with(|| left.node_id.cmp(&right.node_id))
    });
    let mut expressions = BTreeMap::new();
    for row in rows {
        dependencies.insert(SemanticCompilerDependency::OperatorNode {
            node_id: Arc::clone(&row.node_id),
            binding_pin: debug_pin(b"epoch-bound-operator", row),
        });
        dependencies.extend(
            row.output_fields
                .iter()
                .cloned()
                .map(SemanticCompilerDependency::Field),
        );
        let inputs = row
            .input_node_ids
            .iter()
            .map(|node_id| {
                expressions.get(node_id.as_ref()).cloned().ok_or_else(|| {
                    EpochBoundSemanticCompileError::InvalidNode {
                        node: row.node_id.to_string(),
                        detail: "lowering dependency is unavailable".to_owned(),
                    }
                })
            })
            .collect::<Result<Vec<RelationalExpression>, _>>()?;
        let expression = match &row.operator {
            ProgramRelationalOperator::Input { relation_id } => {
                selected_operators.insert(SemanticCompilerOperator::Input);
                dependencies.insert(SemanticCompilerDependency::Relation(relation_id.clone()));
                composition_inputs
                    .get(relation_id)
                    .cloned()
                    .unwrap_or_else(|| RelationalExpression::Input(relation_id.clone()))
            }
            ProgramRelationalOperator::Projection { fields } => {
                selected_operators.insert(SemanticCompilerOperator::Projection);
                RelationalExpression::Projection {
                    input: Box::new(inputs[0].clone()),
                    expressions: fields
                        .iter()
                        .map(|field| NamedExpression {
                            field_id: field.output_field_id.clone(),
                            expression: ScalarExpression::Field(field.input_field_id.clone()),
                        })
                        .collect(),
                }
            }
            ProgramRelationalOperator::Filter => {
                let mut bindings = catalog
                    .selections
                    .values()
                    .filter(|binding| {
                        binding.program_binding_id == block.program_binding_id
                            && binding.operator_node_id == row.node_id
                    })
                    .copied()
                    .collect::<Vec<_>>();
                bindings.sort_by(|left, right| left.selection_id.cmp(&right.selection_id));
                let mut selection_predicates = VecDeque::new();
                for binding in bindings {
                    let values = request
                        .selections
                        .iter()
                        .filter(|selection| {
                            selection.query_id == block.query_id
                                && selection.selection_id == binding.selection_id
                        })
                        .collect::<Vec<_>>();
                    let mut value_predicates = VecDeque::new();
                    for value in values {
                        dependencies.insert(SemanticCompilerDependency::EpochBoundSelection {
                            selection_id: Arc::clone(&binding.selection_id),
                            binding_pin: debug_pin(
                                b"epoch-bound-selection-lowering",
                                &(binding, value),
                            ),
                        });
                        value_predicates.push_back(ScalarExpression::Call {
                            operator: binding.scalar_operator,
                            arguments: vec![
                                ScalarExpression::Field(binding.input_field_id.clone()),
                                ScalarExpression::Literal(value.value.scalar()),
                            ],
                        });
                    }
                    let fold = match binding.fold {
                        EpochBoundSelectionFold::All => ScalarOperator::And,
                        EpochBoundSelectionFold::Any => ScalarOperator::Or,
                    };
                    if let Some(predicate) = fold_epoch_predicates(value_predicates, fold) {
                        selection_predicates.push_back(predicate);
                    }
                }
                if let Some(predicate) =
                    fold_epoch_predicates(selection_predicates, ScalarOperator::And)
                {
                    selected_operators.insert(SemanticCompilerOperator::Filter);
                    RelationalExpression::Filter {
                        input: Box::new(inputs[0].clone()),
                        predicate,
                    }
                } else {
                    inputs[0].clone()
                }
            }
            ProgramRelationalOperator::Join { kind, predicates } => {
                selected_operators.insert(SemanticCompilerOperator::Join(*kind));
                RelationalExpression::Join {
                    left: Box::new(inputs[0].clone()),
                    right: Box::new(inputs[1].clone()),
                    kind: *kind,
                    predicates: predicates
                        .iter()
                        .map(|predicate| ScalarExpression::Call {
                            operator: predicate.scalar_operator,
                            arguments: vec![
                                ScalarExpression::Field(predicate.left_field_id.clone()),
                                ScalarExpression::Field(predicate.right_field_id.clone()),
                            ],
                        })
                        .collect(),
                }
            }
            ProgramRelationalOperator::Union { kind } => {
                selected_operators.insert(SemanticCompilerOperator::Union(*kind));
                RelationalExpression::Union {
                    inputs,
                    kind: *kind,
                }
            }
            ProgramRelationalOperator::Aggregate {
                group_by,
                aggregates,
            } => {
                selected_operators.insert(SemanticCompilerOperator::Aggregate);
                RelationalExpression::Aggregate {
                    input: Box::new(inputs[0].clone()),
                    group_by: group_by
                        .iter()
                        .map(|field| NamedExpression {
                            field_id: field.output_field_id.clone(),
                            expression: ScalarExpression::Field(field.input_field_id.clone()),
                        })
                        .collect(),
                    aggregates: aggregates
                        .iter()
                        .map(|field| NamedAggregateExpression {
                            field_id: field.output_field_id.clone(),
                            expression: AggregateExpression {
                                operator: field.aggregate_operator,
                                argument: ScalarExpression::Field(field.input_field_id.clone()),
                            },
                        })
                        .collect(),
                }
            }
            ProgramRelationalOperator::Sort { fields } => {
                selected_operators.insert(SemanticCompilerOperator::Sort);
                RelationalExpression::Sort {
                    input: Box::new(inputs[0].clone()),
                    expressions: fields
                        .iter()
                        .map(|field| SortExpression {
                            expression: ScalarExpression::Field(field.input_field_id.clone()),
                            ascending: field.ascending,
                            nulls_first: field.nulls_first,
                        })
                        .collect(),
                }
            }
            ProgramRelationalOperator::Limit { skip } => {
                if let Some(fetch) = block.explicit_result_limit {
                    selected_operators.insert(SemanticCompilerOperator::Limit);
                    RelationalExpression::Limit {
                        input: Box::new(inputs[0].clone()),
                        skip: *skip,
                        fetch: Some(fetch),
                    }
                } else {
                    inputs[0].clone()
                }
            }
        };
        expressions.insert(Arc::clone(&row.node_id), expression);
    }
    let root = expressions
        .remove(program.root_node_id.as_ref())
        .expect("validated execution root was lowered");
    Ok(RelationalProgram {
        root,
        output_fields: program.output_fields.clone(),
    })
}

fn epoch_bound_output_coverage(
    block: &EpochBoundBlockBindingRow,
) -> Result<ResultCoverage, EpochBoundSemanticCompileError> {
    if block.explicit_result_limit.is_some() {
        Ok(ResultCoverage::try_new(
            ResultCompleteness::Unknown,
            1,
            0,
            1,
            Some(ResultUnknownCause::try_new(
                "EXPLICIT_LIMIT_REACHED_UNKNOWN",
            )?),
        )?)
    } else {
        Ok(ResultCoverage::complete(1))
    }
}

fn epoch_bound_compiler_proof_pin(
    request: &EpochBoundSemanticIngress,
    dependencies: &BTreeSet<SemanticCompilerDependency>,
    operators: &BTreeSet<SemanticCompilerOperator>,
    blocks: &[CompiledSemanticBlock],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hash_part(
        &mut hasher,
        b"codefabric.epoch-bound-semantic-query.compiler-proof.v1",
    );
    hash_part(
        &mut hasher,
        EPOCH_BOUND_SEMANTIC_QUERY_PROGRAM_RELEASE.as_bytes(),
    );
    hash_part(&mut hasher, request.semantic_request_id.as_bytes());
    hash_part(&mut hasher, &request.request_content_pin);
    hash_part(&mut hasher, &request.fabric_epoch_pin);
    hash_part(&mut hasher, &request.program_catalog_pin);
    hash_part(&mut hasher, &request.policy_pin);
    for dependency in dependencies {
        hash_part(&mut hasher, format!("{dependency:?}").as_bytes());
    }
    for operator in operators {
        hash_part(&mut hasher, format!("{operator:?}").as_bytes());
    }
    for query_id in &request.dependency_order {
        hash_part(&mut hasher, query_id.as_bytes());
    }
    for block in blocks {
        hash_part(&mut hasher, block.query_id.as_bytes());
        hash_part(&mut hasher, format!("{:?}", block.disposition).as_bytes());
        for issue in &block.issues {
            hash_part(&mut hasher, format!("{issue:?}").as_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

/// Compile one validated epoch-bound semantic ingress directly through the exact execution
/// programs selected by `program_binding_id` and execution pin.
///
/// Released form labels are copied into the compatibility result envelope only. They are never
/// consulted to select a program, operator graph, relation, or field. Request-owned relations and
/// normalized scopes remain attached as an explicit runtime handoff; callers cannot obtain an
/// owned compiled request without also receiving that handoff.
///
/// # Errors
///
/// Fails closed when any epoch/catalog/program pin, operator/schema contract, selection or return
/// realization, producer closure, consumer slot, or runtime handoff is absent or inconsistent.
#[allow(clippy::too_many_lines)]
pub fn compile_epoch_bound_semantic_request(
    ingress: &ValidatedEpochBoundSemanticIngress,
    execution_catalog: &EpochBoundSemanticExecutionCatalog,
    producer_closure: &ProducerClosureProof,
) -> Result<CompiledEpochBoundSemanticRequest, EpochBoundSemanticCompileError> {
    let request = ingress.ingress();
    let catalog = validate_epoch_execution_catalog(ingress, execution_catalog)?;
    let closure =
        validate_epoch_compile_closure(producer_closure, request.producer_closure_proof_pin)?;
    let blocks = request
        .blocks
        .iter()
        .map(|block| (Arc::clone(&block.query_id), block))
        .collect::<BTreeMap<_, _>>();

    let mut dependencies = BTreeSet::from([
        SemanticCompilerDependency::EpochBoundRequest(request.request_content_pin),
        SemanticCompilerDependency::FabricEpoch(request.fabric_epoch_pin),
        SemanticCompilerDependency::ExecutionCatalog(execution_catalog.execution_catalog_pin),
        SemanticCompilerDependency::Limits(request.limits_pin),
        SemanticCompilerDependency::ProgramCatalog(request.program_catalog_pin),
        SemanticCompilerDependency::SourcePin(request.source_pin),
        SemanticCompilerDependency::PolicyPin(request.policy_pin),
        SemanticCompilerDependency::ProducerClosureProof(request.producer_closure_proof_pin),
        SemanticCompilerDependency::ProgramRelease(execution_catalog.program_release_pin),
        SemanticCompilerDependency::Authority(Arc::clone(&catalog.authority_id)),
        SemanticCompilerDependency::SemanticClass(Arc::clone(&catalog.semantic_class_id)),
    ]);

    for block in blocks.values() {
        let program = catalog
            .programs
            .get(block.program_binding_id.as_ref())
            .ok_or_else(|| {
                EpochBoundSemanticCompileError::MissingProgram(block.program_binding_id.to_string())
            })?;
        let selected_pin = ingress
            .execution_programs()
            .get(block.query_id.as_ref())
            .expect("validated ingress selected one execution program per block");
        if program.execution_program_pin != *selected_pin {
            return Err(
                EpochBoundSemanticCompileError::ExecutionProgramPinMismatch {
                    program_binding_id: block.program_binding_id.to_string(),
                },
            );
        }
        let has_limit_node = catalog.operators.values().any(|operator| {
            operator.program_binding_id == block.program_binding_id
                && matches!(operator.operator, ProgramRelationalOperator::Limit { .. })
        });
        if block.explicit_result_limit.is_some() && !has_limit_node {
            return Err(EpochBoundSemanticCompileError::MissingBinding {
                query_id: block.query_id.to_string(),
                family: "explicit result limit operator",
                binding_id: block.program_binding_id.to_string(),
            });
        }
        dependencies.insert(SemanticCompilerDependency::RequestBlock {
            query_id: Arc::clone(&block.query_id),
            content_pin: debug_pin(b"epoch-bound-request-block", block),
        });
        dependencies.insert(SemanticCompilerDependency::ProgramBinding {
            program_binding_id: Arc::clone(&block.program_binding_id),
            execution_program_pin: program.execution_program_pin,
        });
        dependencies.insert(SemanticCompilerDependency::Relation(
            program.output_relation_id.clone(),
        ));
        dependencies.extend(
            program
                .output_fields
                .iter()
                .cloned()
                .map(SemanticCompilerDependency::Field),
        );

        for selection in request
            .selections
            .iter()
            .filter(|selection| selection.query_id == block.query_id)
        {
            if !catalog.selections.contains_key(&(
                Arc::clone(&block.program_binding_id),
                Arc::clone(&selection.selection_id),
            )) {
                return Err(EpochBoundSemanticCompileError::MissingBinding {
                    query_id: block.query_id.to_string(),
                    family: "selection lowering",
                    binding_id: selection.selection_id.to_string(),
                });
            }
        }

        let mut observed_return = false;
        let mut realized_fields = BTreeSet::new();
        for return_row in request
            .returns
            .iter()
            .filter(|return_row| return_row.query_id == block.query_id)
        {
            observed_return = true;
            let realization = catalog
                .returns
                .get(&(
                    Arc::clone(&block.program_binding_id),
                    Arc::clone(&return_row.return_id),
                    return_row.value.clone(),
                ))
                .ok_or_else(|| EpochBoundSemanticCompileError::MissingBinding {
                    query_id: block.query_id.to_string(),
                    family: "return realization",
                    binding_id: return_row.return_id.to_string(),
                })?;
            dependencies.insert(SemanticCompilerDependency::ReturnRealization {
                return_id: Arc::clone(&return_row.return_id),
                realization_pin: realization.realization_pin,
            });
            for field in &realization.realization_field_ids {
                realized_fields.insert(field.clone());
                dependencies.insert(SemanticCompilerDependency::Field(field.clone()));
            }
        }
        if observed_return
            && realized_fields
                != program
                    .output_fields
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
        {
            return Err(EpochBoundSemanticCompileError::OutputSchema {
                program_binding_id: block.program_binding_id.to_string(),
                detail: "selected return realizations do not cover the exact program output"
                    .to_owned(),
            });
        }
    }

    for edge in &request.dependencies {
        dependencies.insert(SemanticCompilerDependency::CompositionEdge(debug_pin(
            b"epoch-bound-composition-edge",
            edge,
        )));
    }
    let handoff = compile_epoch_runtime_handoff(ingress, &catalog, &blocks, &mut dependencies)?;

    let mut operators = BTreeSet::new();
    let mut compiled_roots = BTreeMap::<Arc<str>, EpochCompiledRoot>::new();
    let mut dispositions = BTreeMap::<Arc<str>, SemanticBlockDisposition>::new();
    let mut compiled_blocks = Vec::with_capacity(blocks.len());

    for query_id in &request.dependency_order {
        let block = blocks[query_id.as_ref()];
        let program = catalog.programs[block.program_binding_id.as_ref()];
        let mut disposition = SemanticBlockDisposition::Compiled;
        let mut issues = Vec::new();

        for family in catalog
            .required_families
            .get(block.program_binding_id.as_ref())
            .into_iter()
            .flatten()
        {
            dependencies.insert(SemanticCompilerDependency::FactFamily(Arc::clone(family)));
            match closure.get(family.as_ref()) {
                Some(ClosureStatus::Runtime(runtime)) => {
                    dependencies.insert(SemanticCompilerDependency::Producer(Arc::clone(
                        &runtime.producer_id,
                    )));
                }
                Some(ClosureStatus::Remainder(remainder)) => {
                    dependencies.insert(SemanticCompilerDependency::UnsupportedRemainder(
                        Arc::clone(&remainder.remainder_id),
                    ));
                    disposition = SemanticBlockDisposition::UnsupportedRemainder;
                    issues.push(SemanticCompilationIssue {
                        code: "UNSUPPORTED_FACT_FAMILY",
                        subject_id: Arc::clone(family),
                        related_id: Some(Arc::clone(&remainder.reason_id)),
                    });
                }
                None => {
                    if disposition != SemanticBlockDisposition::UnsupportedRemainder {
                        disposition = SemanticBlockDisposition::UnknownProducerClosure;
                    }
                    issues.push(SemanticCompilationIssue {
                        code: "PRODUCER_CLOSURE_UNKNOWN",
                        subject_id: Arc::clone(family),
                        related_id: None,
                    });
                }
            }
        }

        let failed_dependencies = request
            .dependencies
            .iter()
            .filter(|edge| edge.consumer_query_id.as_ref() == query_id.as_ref())
            .filter_map(|edge| {
                dispositions
                    .get(edge.producer_query_id.as_ref())
                    .filter(|state| **state != SemanticBlockDisposition::Compiled)
                    .map(|_| Arc::clone(&edge.producer_query_id))
            })
            .collect::<BTreeSet<_>>();
        if disposition == SemanticBlockDisposition::Compiled && !failed_dependencies.is_empty() {
            disposition = SemanticBlockDisposition::NotExecutedDependency;
            issues.extend(failed_dependencies.into_iter().map(|dependency| {
                SemanticCompilationIssue {
                    code: "NOT_EXECUTED_DEPENDENCY",
                    subject_id: Arc::clone(query_id),
                    related_id: Some(dependency),
                }
            }));
        }

        issues.sort();
        let output = if disposition == SemanticBlockDisposition::Compiled {
            let composition_inputs = epoch_composition_inputs(
                block,
                request,
                &blocks,
                &catalog,
                &compiled_roots,
                &mut dependencies,
            )?;
            let relational_program = lower_epoch_execution_program(
                block,
                program,
                request,
                &catalog,
                &composition_inputs,
                &mut dependencies,
                &mut operators,
            )?;
            compiled_roots.insert(
                Arc::clone(query_id),
                EpochCompiledRoot {
                    relation_id: program.output_relation_id.clone(),
                    fields: program.output_fields.clone(),
                    expression: relational_program.root.clone(),
                },
            );
            Some(SelectedQueryOutput::new(
                program.output_relation_id.clone(),
                relational_program,
                Some(epoch_bound_output_coverage(block)?),
            ))
        } else {
            None
        };
        dispositions.insert(Arc::clone(query_id), disposition);
        compiled_blocks.push(CompiledSemanticBlock {
            query_id: Arc::clone(query_id),
            form: block.compatibility_form,
            disposition,
            output,
            issues,
        });
    }

    let compiler_proof_pin =
        epoch_bound_compiler_proof_pin(request, &dependencies, &operators, &compiled_blocks);
    Ok(CompiledEpochBoundSemanticRequest {
        compiled: CompiledSemanticRequest {
            blocks: compiled_blocks,
            observation: SemanticCompilerObservation {
                program_release: EPOCH_BOUND_SEMANTIC_QUERY_PROGRAM_RELEASE,
                compiler_proof_pin,
                dependencies,
                operators,
                dependency_order: request.dependency_order.clone(),
                limits: request.limits.compiler(),
            },
        },
        handoff,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objective_input_set_and_typed_group_known_answers() {
        let input_set = issue_objective_input_set_identity(&ObjectiveInputSetIdentityInput {
            workspace_id: Arc::from(format!("workspace:{}", "00".repeat(16))),
            analysis_context_ids: vec![
                Arc::from(format!("context:{}", "12".repeat(16))),
                Arc::from(format!("context:{}", "11".repeat(16))),
            ]
            .into(),
            fact_ids: vec![
                Arc::from(format!("fact:native-kind:{}", "56".repeat(16))),
                Arc::from(format!("fact:native-kind:{}", "55".repeat(16))),
            ]
            .into(),
            producer_identities: vec![Arc::from("producer:b"), Arc::from("producer:a")].into(),
            policy_identity: Arc::from("policy:r1"),
            coverage_state: ObjectiveInputCoverageState::Partial,
        })
        .expect("domain-19 objective input-set KAT");
        assert_eq!(
            input_set.public_id,
            "input-set:e29e16fcd6993bde6c552af6233536ba"
        );
        assert_eq!(
            input_set.recipe_evidence()["digest"]["full_digest_hex"],
            "e29e16fcd6993bde6c552af6233536ba2fc3879fefc8e5dc1c003832639617fc"
        );

        let mut key = BTreeMap::new();
        key.insert(Arc::from("enabled"), ObjectiveGroupScalar::Boolean(true));
        key.insert(
            Arc::from("kind"),
            ObjectiveGroupScalar::Text(Arc::from("function")),
        );
        key.insert(Arc::from("ordinal"), ObjectiveGroupScalar::Unsigned(42));
        key.insert(Arc::from("offset"), ObjectiveGroupScalar::Signed(-7));
        let group_input = ObjectiveGroupIdentityInput {
            workspace_id: Arc::from(format!("workspace:{}", "00".repeat(16))),
            analysis_context_id: Arc::from(format!("context:{}", "11".repeat(16))),
            input_set_id: Arc::from(format!("input-set:{}", "88".repeat(16))),
            grouping_dimensions: vec![Arc::from("kind"), Arc::from("visibility")].into(),
            canonical_group_key: key,
            aggregate_function: Arc::from("COUNT"),
            measure: Arc::from("fact_id"),
            producer_id: Arc::from("producer:objective-r1"),
        };
        let group =
            issue_objective_group_identity(&group_input).expect("domain-20 objective-group KAT");
        assert_eq!(group.public_id, "group:1a442bd5da6abe7a2c0d264008ba85d7");
        let group_evidence = group.recipe_evidence();
        assert_eq!(
            group_evidence["digest"]["full_digest_hex"],
            "1a442bd5da6abe7a2c0d264008ba85d726d790df5d4d22a3c3a7435b8e4e4a9c"
        );
        assert_eq!(
            group_evidence["fields"][4]["value"]["enabled"]["member_type"],
            "BOOLEAN"
        );
        assert_eq!(
            group_evidence["fields"][4]["value"]["ordinal"]["variant"],
            4
        );

        let mut changed = group_input;
        changed
            .canonical_group_key
            .insert(Arc::from("enabled"), ObjectiveGroupScalar::Boolean(false));
        assert_ne!(
            issue_objective_group_identity(&changed).unwrap().public_id,
            group.public_id
        );
    }

    fn relation(value: impl Into<String>) -> RelationId {
        RelationId::new(value.into()).expect("test relation")
    }

    fn field(value: impl Into<String>) -> FieldId {
        FieldId::new(value.into()).expect("test field")
    }

    fn limits() -> SemanticRequestLimits {
        SemanticRequestLimits::try_new(32, 64, 8, 8, 32, 16, 10_000).expect("limits")
    }

    fn runtime_closure() -> ProducerClosureProof {
        ProducerClosureProof {
            proof_pin: [5; 32],
            application_authority_id: Arc::from("authority.application"),
            families: vec![ProducerFamilyClosureRow {
                family_id: Arc::from("family.core"),
                disposition: ProducerFamilyDisposition::RuntimeProducer(RuntimeProducerProof {
                    producer_id: Arc::from("producer.core"),
                    authority_id: Arc::from("authority.application"),
                    algorithm_release: Arc::from("algorithm.v1"),
                    precision_id: Arc::from("precision.exact"),
                    input_pin: [6; 32],
                    invalidation_pin: [7; 32],
                    materialization_pin: [8; 32],
                    requested_units: 1,
                    completed_units: 1,
                    remainder_units: 0,
                    unknown_units: 0,
                    completeness_proof_pin: [9; 32],
                    producer_proof_pin: [10; 32],
                }),
            }],
        }
    }

    fn request(blocks: Vec<SemanticRequestBlockRow>) -> SemanticRequestRelations {
        SemanticRequestRelations {
            semantic_request_id: Arc::from("request"),
            program_catalog_pin: [1; 32],
            source_pin: [2; 32],
            policy_pin: [3; 32],
            producer_closure_proof_pin: [5; 32],
            blocks,
            clauses: Vec::new(),
            dependencies: Vec::new(),
            limits: limits(),
        }
    }

    fn block(index: usize, form: ReleasedSemanticForm) -> SemanticRequestBlockRow {
        SemanticRequestBlockRow {
            query_id: Arc::from(format!("query-{index:02}")),
            form,
            output_role_id: Arc::from("result"),
            explicit_result_limit: None,
        }
    }

    fn eight_form_program_catalog() -> SemanticQueryProgramCatalog {
        let mut forms = Vec::new();
        let mut operators = Vec::new();
        let mut schemas = Vec::new();
        let mut required = Vec::new();
        for (index, form) in ReleasedSemanticForm::ALL.into_iter().enumerate() {
            let relation_id = relation(format!("relation.{index}"));
            let field_id = field(format!("field.{index}"));
            let node_id: Arc<str> = Arc::from(format!("node.{index}"));
            forms.push(ProgramFormBindingRow {
                form_label: Arc::from(form.label()),
                output_role_id: Arc::from("result"),
                root_node_id: Arc::clone(&node_id),
                output_relation_id: relation_id.clone(),
                output_fields: vec![field_id.clone()],
            });
            operators.push(ProgramOperatorRow {
                form_label: Arc::from(form.label()),
                output_role_id: Arc::from("result"),
                node_id,
                ordinal: 0,
                input_node_ids: Vec::new(),
                operator: ProgramRelationalOperator::Input {
                    relation_id: relation_id.clone(),
                },
                output_fields: vec![field_id.clone()],
            });
            schemas.push(ProgramRelationSchemaRow {
                relation_id,
                fields: vec![field_id],
            });
            required.push(ProgramRequiredFactFamilyRow {
                form_label: Arc::from(form.label()),
                output_role_id: Arc::from("result"),
                family_id: Arc::from("family.core"),
            });
        }
        SemanticQueryProgramCatalog {
            program_catalog_pin: [1; 32],
            program_release_pin: [4; 32],
            authority: SemanticQueryAuthority::ApplicationOwned(Arc::from("authority.application")),
            semantic_class: SemanticQueryClass::Fact(Arc::from("semantic.fact")),
            forms,
            input_roles: Vec::new(),
            clauses: Vec::new(),
            operators,
            relation_schemas: schemas,
            required_fact_families: required,
        }
    }

    #[test]
    fn all_eight_released_forms_compile_from_typed_program_rows() {
        let blocks = ReleasedSemanticForm::ALL
            .into_iter()
            .enumerate()
            .map(|(index, form)| block(index, form))
            .collect();
        let compiled = compile_relational_semantic_request(
            &request(blocks),
            &eight_form_program_catalog(),
            &runtime_closure(),
        )
        .expect("all forms compile");
        assert_eq!(compiled.blocks().len(), 8);
        assert!(compiled.blocks().iter().all(|block| {
            block.disposition() == SemanticBlockDisposition::Compiled && block.output().is_some()
        }));
        assert_eq!(
            compiled.observation().operators,
            BTreeSet::from([SemanticCompilerOperator::Input])
        );
    }

    #[tokio::test]
    async fn all_eight_epoch_bound_forms_execute_through_one_real_authorized_child() {
        use std::collections::HashMap;

        use arrow_array::{Int64Array, RecordBatch};
        use arrow_schema::{DataType, Field, Schema};
        use datafusion::common::TableReference;
        use datafusion::datasource::MemTable;

        use crate::fabric::child_session::resource_governance::{
            EpochResourceCoordinator, EpochResourcePolicy, test_lifecycle_work_class_policies,
        };
        use crate::fabric::child_session::{
            ChildRegistryAllowlist, ChildResourceLimits, ChildSessionPins, ChildSessionPolicy,
            ChildTableGrant,
        };
        use crate::fabric::epoch_runtime::{
            FABRIC_CATALOG, FabricEpochId, FabricEpochRuntimeConfig, FabricSchemaRole,
        };
        use crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder;
        use crate::fabric::programmatic_schema::{ProgrammaticRelationId, ProviderInput};
        use crate::schema_contract::{
            FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SchemaContract,
        };

        let ingress_limits = epoch_ingress_limits();
        let mut program_bindings = Vec::new();
        let mut blocks = Vec::new();
        let mut programs = Vec::new();
        let mut operators = Vec::new();
        let mut relation_schemas = Vec::new();
        let mut required_fact_families = Vec::new();
        let mut dependency_order = Vec::new();
        for (index, form) in ReleasedSemanticForm::ALL.into_iter().enumerate() {
            let index_pin = u8::try_from(index + 40).expect("small form index");
            let query_id: Arc<str> = Arc::from(format!("query-{index:02}"));
            let program_binding_id: Arc<str> = Arc::from(format!("program.{index}"));
            let role_id: Arc<str> = Arc::from(format!("role.{index}"));
            let input_node_id: Arc<str> = Arc::from(format!("node.{index}.input"));
            let root_node_id: Arc<str> = Arc::from(format!("node.{index}.limit"));
            let relation_id = relation(format!("relation.{index}"));
            let field_id = field(format!("field.{index}"));
            let execution_program_pin = [index_pin; 32];
            program_bindings.push(EpochBoundProgramBindingRow {
                program_binding_id: Arc::clone(&program_binding_id),
                program_binding_pin: [index_pin.wrapping_add(20); 32],
                compatibility_form: form,
                output_role_id: Arc::clone(&role_id),
                execution_program_pin,
            });
            blocks.push(EpochBoundBlockBindingRow {
                query_id: Arc::clone(&query_id),
                compatibility_form: form,
                program_binding_id: Arc::clone(&program_binding_id),
                program_binding_pin: [index_pin.wrapping_add(20); 32],
                output_role_id: role_id,
                explicit_result_limit: Some(128),
            });
            programs.push(EpochBoundExecutionProgramRow {
                program_binding_id: Arc::clone(&program_binding_id),
                execution_program_pin,
                root_node_id: Arc::clone(&root_node_id),
                output_relation_id: relation_id.clone(),
                output_fields: vec![field_id.clone()],
            });
            operators.push(EpochBoundExecutionOperatorRow {
                program_binding_id: Arc::clone(&program_binding_id),
                execution_program_pin,
                node_id: Arc::clone(&input_node_id),
                ordinal: 0,
                input_node_ids: Vec::new(),
                operator: ProgramRelationalOperator::Input {
                    relation_id: relation_id.clone(),
                },
                output_fields: vec![field_id.clone()],
            });
            operators.push(EpochBoundExecutionOperatorRow {
                program_binding_id: Arc::clone(&program_binding_id),
                execution_program_pin,
                node_id: root_node_id,
                ordinal: 1,
                input_node_ids: vec![input_node_id],
                operator: ProgramRelationalOperator::Limit { skip: 0 },
                output_fields: vec![field_id.clone()],
            });
            relation_schemas.push(ProgramRelationSchemaRow {
                relation_id,
                fields: vec![field_id],
            });
            required_fact_families.push(EpochBoundExecutionRequiredFamilyRow {
                program_binding_id,
                execution_program_pin,
                family_id: Arc::from("family.core"),
            });
            dependency_order.push(query_id);
        }
        let ingress_catalog = EpochBoundSemanticIngressCatalog {
            fabric_epoch_pin: [12; 32],
            program_catalog_pin: [13; 32],
            source_pin: [14; 32],
            policy_pin: [15; 32],
            producer_closure_proof_pin: [16; 32],
            limits_pin: epoch_bound_semantic_ingress_limits_pin(ingress_limits),
            program_bindings,
            consumer_slots: Vec::new(),
            selections: Vec::new(),
            returns: Vec::new(),
            scopes: Vec::new(),
            request_inputs: Vec::new(),
        };
        let ingress = EpochBoundSemanticIngress {
            semantic_request_id: Arc::from("request.all-eight.epoch-bound"),
            request_content_pin: [11; 32],
            fabric_epoch_pin: [12; 32],
            program_catalog_pin: [13; 32],
            source_pin: [14; 32],
            policy_pin: [15; 32],
            producer_closure_proof_pin: [16; 32],
            limits_pin: epoch_bound_semantic_ingress_limits_pin(ingress_limits),
            limits: ingress_limits,
            blocks,
            selections: Vec::new(),
            returns: Vec::new(),
            scopes: Vec::new(),
            request_inputs: Vec::new(),
            dependencies: Vec::new(),
            dependency_order,
        };
        let validated = validate_epoch_bound_semantic_ingress(ingress, &ingress_catalog)
            .expect("all eight bindings consume the exact epoch catalog");
        let execution_catalog = EpochBoundSemanticExecutionCatalog {
            fabric_epoch_pin: [12; 32],
            program_catalog_pin: [13; 32],
            source_pin: [14; 32],
            policy_pin: [15; 32],
            producer_closure_proof_pin: [16; 32],
            execution_catalog_pin: [24; 32],
            program_release_pin: [25; 32],
            authority: SemanticQueryAuthority::ApplicationOwned(Arc::from("authority.application")),
            semantic_class: SemanticQueryClass::Fact(Arc::from("semantic.fact")),
            programs,
            operators,
            relation_schemas,
            consumer_slots: Vec::new(),
            selections: Vec::new(),
            returns: Vec::new(),
            required_fact_families,
            request_inputs: Vec::new(),
            scopes: Vec::new(),
        };
        let compiled = compile_epoch_bound_semantic_request(
            &validated,
            &execution_catalog,
            &epoch_runtime_closure(),
        )
        .expect("all eight epoch-bound forms compile");

        let epoch_id = FabricEpochId::from_bytes([0x8e; 16]);
        let runtime_config = FabricEpochRuntimeConfig::try_new(
            64 * 1024 * 1024,
            256 * 1024 * 1024,
            8,
            8,
            1_024,
            1,
            true,
        )
        .expect("explicit bounded runtime configuration");
        let mut builder = ProgrammaticFabricEpochBuilder::try_new(epoch_id, runtime_config)
            .expect("fresh programmatic epoch");
        let mut grants = Vec::new();
        for index in 0..ReleasedSemanticForm::ALL.len() {
            let relation_name = format!("relation.{index}");
            let field_identity = format!("field.{index}");
            let field =
                Field::new("value", DataType::Int64, false).with_metadata(HashMap::from([(
                    FIELD_ID_METADATA_KEY.to_owned(),
                    field_identity,
                )]));
            let schema = Arc::new(Schema::new(vec![field]).with_metadata(HashMap::from([(
                RELATION_ID_METADATA_KEY.to_owned(),
                relation_name.clone(),
            )])));
            let batch = RecordBatch::try_new(
                Arc::clone(&schema),
                vec![Arc::new(Int64Array::from(vec![
                    i64::try_from(index).expect("small form index"),
                    i64::try_from(index + 100).expect("small form index"),
                ]))],
            )
            .expect("typed form rows");
            let provider = Arc::new(
                MemTable::try_new(Arc::clone(&schema), vec![vec![batch]]).expect("form provider"),
            );
            let relation_id = ProgrammaticRelationId::new(relation_name.as_str());
            let table_reference = TableReference::full(
                FABRIC_CATALOG,
                FabricSchemaRole::Fact.as_str(),
                format!("semantic_form_{index}"),
            );
            let contract = Arc::new(
                SchemaContract::try_new(
                    format!("provider:semantic-form:{index}"),
                    table_reference.clone(),
                    Arc::clone(&schema),
                    schema,
                    vec![FieldIndexMapping::direct(0, 0)],
                )
                .expect("exact form contract"),
            );
            builder
                .register_provider(ProviderInput::new(
                    relation_id.clone(),
                    table_reference,
                    contract,
                    provider,
                ))
                .expect("register form relation");
            grants.push(ChildTableGrant::try_new(relation_id).expect("form grant"));
        }
        let epoch = builder
            .seal_for_test()
            .await
            .expect("seal exact programmatic epoch");

        let child_resources =
            ChildResourceLimits::try_new(8 * 1024 * 1024, 32 * 1024 * 1024, 4, 2, 128, 1)
                .expect("bounded child resources");
        let resource_policy = EpochResourcePolicy::try_new(
            child_resources.clone(),
            test_lifecycle_work_class_policies(),
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
        .expect("bounded epoch resources");
        let resources = EpochResourceCoordinator::try_new(epoch_id, [0x33; 32], resource_policy)
            .expect("epoch resource authority");
        let policy = ChildSessionPolicy::try_new(
            ChildSessionPins::try_new(epoch_id, [0x11; 32], [0x22; 32], [0x33; 32])
                .expect("exact child pins"),
            grants,
            child_resources,
            128,
            ChildRegistryAllowlist::default(),
        )
        .expect("authorized all-form child policy");
        let child = epoch
            .authorized_child_session(policy, &resources)
            .await
            .expect("fresh authorized child");

        assert_eq!(
            compiled.compiled().blocks().len(),
            ReleasedSemanticForm::ALL.len()
        );
        for compiled_block in compiled.compiled().blocks() {
            let output = compiled_block
                .output()
                .expect("compiled form has a selected output");
            let result = child
                .execute_relational_program(output.program())
                .await
                .expect("form executes only through the reduced child");
            assert_eq!(result.row_count(), 2, "{:?}", compiled_block.form());
        }
    }

    fn composition_program_catalog() -> SemanticQueryProgramCatalog {
        let source_relation = relation("relation.source");
        let producer_relation = relation("relation.producer");
        let consumer_relation = relation("relation.consumer");
        let source_field = field("field.source");
        let producer_field = field("field.producer");
        let consumer_field = field("field.consumer");
        let producer_form = ReleasedSemanticForm::FindCodeEntities;
        let consumer_form = ReleasedSemanticForm::RetrieveFactsAboutCode;
        SemanticQueryProgramCatalog {
            program_catalog_pin: [1; 32],
            program_release_pin: [4; 32],
            authority: SemanticQueryAuthority::ApplicationOwned(Arc::from("authority.application")),
            semantic_class: SemanticQueryClass::Fact(Arc::from("semantic.fact")),
            forms: vec![
                ProgramFormBindingRow {
                    form_label: Arc::from(producer_form.label()),
                    output_role_id: Arc::from("result"),
                    root_node_id: Arc::from("producer-project"),
                    output_relation_id: producer_relation.clone(),
                    output_fields: vec![producer_field.clone()],
                },
                ProgramFormBindingRow {
                    form_label: Arc::from(consumer_form.label()),
                    output_role_id: Arc::from("result"),
                    root_node_id: Arc::from("consumer-project"),
                    output_relation_id: consumer_relation.clone(),
                    output_fields: vec![consumer_field.clone()],
                },
            ],
            input_roles: vec![ProgramInputRoleBindingRow {
                form_label: Arc::from(consumer_form.label()),
                role_id: Arc::from("source-results"),
                input_relation_id: producer_relation.clone(),
            }],
            clauses: Vec::new(),
            operators: vec![
                ProgramOperatorRow {
                    form_label: Arc::from(producer_form.label()),
                    output_role_id: Arc::from("result"),
                    node_id: Arc::from("producer-input"),
                    ordinal: 0,
                    input_node_ids: Vec::new(),
                    operator: ProgramRelationalOperator::Input {
                        relation_id: source_relation.clone(),
                    },
                    output_fields: vec![source_field.clone()],
                },
                ProgramOperatorRow {
                    form_label: Arc::from(producer_form.label()),
                    output_role_id: Arc::from("result"),
                    node_id: Arc::from("producer-project"),
                    ordinal: 1,
                    input_node_ids: vec![Arc::from("producer-input")],
                    operator: ProgramRelationalOperator::Projection {
                        fields: vec![ProgramProjectionField {
                            input_field_id: source_field.clone(),
                            output_field_id: producer_field.clone(),
                        }],
                    },
                    output_fields: vec![producer_field.clone()],
                },
                ProgramOperatorRow {
                    form_label: Arc::from(consumer_form.label()),
                    output_role_id: Arc::from("result"),
                    node_id: Arc::from("consumer-input"),
                    ordinal: 0,
                    input_node_ids: Vec::new(),
                    operator: ProgramRelationalOperator::Input {
                        relation_id: producer_relation.clone(),
                    },
                    output_fields: vec![producer_field.clone()],
                },
                ProgramOperatorRow {
                    form_label: Arc::from(consumer_form.label()),
                    output_role_id: Arc::from("result"),
                    node_id: Arc::from("consumer-project"),
                    ordinal: 1,
                    input_node_ids: vec![Arc::from("consumer-input")],
                    operator: ProgramRelationalOperator::Projection {
                        fields: vec![ProgramProjectionField {
                            input_field_id: producer_field.clone(),
                            output_field_id: consumer_field.clone(),
                        }],
                    },
                    output_fields: vec![consumer_field.clone()],
                },
            ],
            relation_schemas: vec![
                ProgramRelationSchemaRow {
                    relation_id: source_relation,
                    fields: vec![source_field],
                },
                ProgramRelationSchemaRow {
                    relation_id: producer_relation,
                    fields: vec![producer_field],
                },
                ProgramRelationSchemaRow {
                    relation_id: consumer_relation,
                    fields: vec![consumer_field],
                },
            ],
            required_fact_families: vec![
                ProgramRequiredFactFamilyRow {
                    form_label: Arc::from(producer_form.label()),
                    output_role_id: Arc::from("result"),
                    family_id: Arc::from("family.core"),
                },
                ProgramRequiredFactFamilyRow {
                    form_label: Arc::from(consumer_form.label()),
                    output_role_id: Arc::from("result"),
                    family_id: Arc::from("family.core"),
                },
            ],
        }
    }

    fn composition_request() -> SemanticRequestRelations {
        let mut request = request(vec![
            block(0, ReleasedSemanticForm::FindCodeEntities),
            block(1, ReleasedSemanticForm::RetrieveFactsAboutCode),
        ]);
        request.dependencies.push(SemanticRequestDependencyRow {
            producer_query_id: Arc::from("query-00"),
            producer_role_id: Arc::from("result"),
            consumer_query_id: Arc::from("query-01"),
            consumer_role_id: Arc::from("source-results"),
        });
        request
    }

    fn uniform_composition_program_catalog(
        forms: &[ReleasedSemanticForm],
    ) -> SemanticQueryProgramCatalog {
        let relation_id = relation("relation.uniform");
        let field_id = field("field.uniform");
        let mut catalog = SemanticQueryProgramCatalog {
            program_catalog_pin: [1; 32],
            program_release_pin: [4; 32],
            authority: SemanticQueryAuthority::ApplicationOwned(Arc::from("authority.application")),
            semantic_class: SemanticQueryClass::Fact(Arc::from("semantic.fact")),
            forms: Vec::new(),
            input_roles: Vec::new(),
            clauses: Vec::new(),
            operators: Vec::new(),
            relation_schemas: vec![ProgramRelationSchemaRow {
                relation_id: relation_id.clone(),
                fields: vec![field_id.clone()],
            }],
            required_fact_families: Vec::new(),
        };
        for (index, form) in forms.iter().copied().enumerate() {
            let node_id: Arc<str> = Arc::from(format!("uniform-node-{index}"));
            catalog.forms.push(ProgramFormBindingRow {
                form_label: Arc::from(form.label()),
                output_role_id: Arc::from("result"),
                root_node_id: Arc::clone(&node_id),
                output_relation_id: relation_id.clone(),
                output_fields: vec![field_id.clone()],
            });
            catalog.operators.push(ProgramOperatorRow {
                form_label: Arc::from(form.label()),
                output_role_id: Arc::from("result"),
                node_id,
                ordinal: 0,
                input_node_ids: Vec::new(),
                operator: ProgramRelationalOperator::Input {
                    relation_id: relation_id.clone(),
                },
                output_fields: vec![field_id.clone()],
            });
            catalog
                .required_fact_families
                .push(ProgramRequiredFactFamilyRow {
                    form_label: Arc::from(form.label()),
                    output_role_id: Arc::from("result"),
                    family_id: Arc::from("family.core"),
                });
            if index > 0 {
                catalog.input_roles.push(ProgramInputRoleBindingRow {
                    form_label: Arc::from(form.label()),
                    role_id: Arc::from("source"),
                    input_relation_id: relation_id.clone(),
                });
            }
        }
        catalog
    }

    #[test]
    fn composition_inlines_the_program_bound_predecessor_program() {
        let compiled = compile_relational_semantic_request(
            &composition_request(),
            &composition_program_catalog(),
            &runtime_closure(),
        )
        .expect("composition");
        let consumer = compiled.blocks()[1].output().expect("consumer output");
        let RelationalExpression::Projection { input, .. } = &consumer.program().root else {
            panic!("consumer root must come from its catalog projection");
        };
        assert!(matches!(
            input.as_ref(),
            RelationalExpression::Projection { .. }
        ));
        assert_eq!(
            compiled.observation().dependency_order,
            vec![Arc::from("query-00"), Arc::from("query-01")]
        );
    }

    #[test]
    fn dependency_cycle_and_fanout_are_rejected_before_lowering() {
        let catalog = composition_program_catalog();
        let mut cyclic = composition_request();
        cyclic.dependencies.push(SemanticRequestDependencyRow {
            producer_query_id: Arc::from("query-01"),
            producer_role_id: Arc::from("result"),
            consumer_query_id: Arc::from("query-00"),
            consumer_role_id: Arc::from("source-results"),
        });
        assert!(matches!(
            compile_relational_semantic_request(&cyclic, &catalog, &runtime_closure()),
            Err(RelationalSemanticQueryError::UnknownCompositionRole { .. })
                | Err(RelationalSemanticQueryError::QueryDependencyCycle)
        ));

        let fanout_forms = [
            ReleasedSemanticForm::FindCodeEntities,
            ReleasedSemanticForm::RetrieveFactsAboutCode,
            ReleasedSemanticForm::FollowCodeRelationships,
        ];
        let fanout_catalog = uniform_composition_program_catalog(&fanout_forms);
        let mut fanout = request(vec![
            block(0, ReleasedSemanticForm::FindCodeEntities),
            block(1, ReleasedSemanticForm::RetrieveFactsAboutCode),
            block(2, ReleasedSemanticForm::FollowCodeRelationships),
        ]);
        fanout.limits = SemanticRequestLimits::try_new(8, 8, 1, 8, 32, 16, 100).unwrap();
        for consumer in ["query-01", "query-02"] {
            fanout.dependencies.push(SemanticRequestDependencyRow {
                producer_query_id: Arc::from("query-00"),
                producer_role_id: Arc::from("result"),
                consumer_query_id: Arc::from(consumer),
                consumer_role_id: Arc::from("source"),
            });
        }
        assert!(matches!(
            compile_relational_semantic_request(&fanout, &fanout_catalog, &runtime_closure()),
            Err(RelationalSemanticQueryError::RequestLimit {
                limit: "max_fanout",
                ..
            })
        ));
        // Exercise the topology independently with otherwise valid typed block identities.
        let block_a = SemanticRequestBlockRow {
            query_id: Arc::from("a"),
            form: ReleasedSemanticForm::FindCodeEntities,
            output_role_id: Arc::from("result"),
            explicit_result_limit: None,
        };
        let block_b = SemanticRequestBlockRow {
            query_id: Arc::from("b"),
            form: ReleasedSemanticForm::RetrieveFactsAboutCode,
            output_role_id: Arc::from("result"),
            explicit_result_limit: None,
        };
        let blocks = BTreeMap::from([(Arc::from("a"), &block_a), (Arc::from("b"), &block_b)]);
        let edges = vec![
            SemanticRequestDependencyRow {
                producer_query_id: Arc::from("a"),
                producer_role_id: Arc::from("result"),
                consumer_query_id: Arc::from("b"),
                consumer_role_id: Arc::from("in"),
            },
            SemanticRequestDependencyRow {
                producer_query_id: Arc::from("b"),
                producer_role_id: Arc::from("result"),
                consumer_query_id: Arc::from("a"),
                consumer_role_id: Arc::from("in"),
            },
        ];
        assert!(matches!(
            request_dependency_order(&blocks, &edges),
            Err(RelationalSemanticQueryError::QueryDependencyCycle)
        ));
    }

    #[test]
    fn invalid_closure_and_explicit_remainder_do_not_fallback() {
        let request = request(vec![block(0, ReleasedSemanticForm::FindCodeEntities)]);
        let catalog = eight_form_program_catalog();
        let mut invalid = runtime_closure();
        let ProducerFamilyDisposition::RuntimeProducer(runtime) =
            &mut invalid.families[0].disposition
        else {
            unreachable!()
        };
        runtime.completed_units = 0;
        runtime.unknown_units = 1;
        assert!(matches!(
            compile_relational_semantic_request(&request, &catalog, &invalid),
            Err(RelationalSemanticQueryError::ProducerClosure { .. })
        ));

        let remainder = ProducerClosureProof {
            proof_pin: [5; 32],
            application_authority_id: Arc::from("authority.application"),
            families: vec![ProducerFamilyClosureRow {
                family_id: Arc::from("family.core"),
                disposition: ProducerFamilyDisposition::UnsupportedRemainder(
                    UnsupportedFamilyRemainder {
                        remainder_id: Arc::from("remainder.core"),
                        authority_id: Arc::from("authority.application"),
                        reason_id: Arc::from("NOT_AVAILABLE"),
                        proof_pin: [11; 32],
                    },
                ),
            }],
        };
        let compiled = compile_relational_semantic_request(&request, &catalog, &remainder)
            .expect("remainder is data");
        assert_eq!(
            compiled.blocks()[0].disposition(),
            SemanticBlockDisposition::UnsupportedRemainder
        );
        assert!(compiled.blocks()[0].output().is_none());
        assert_eq!(
            compiled.blocks()[0].issues()[0].code,
            "UNSUPPORTED_FACT_FAMILY"
        );
    }

    #[test]
    fn permutation_preserves_canonical_program_proof() {
        let blocks = ReleasedSemanticForm::ALL
            .into_iter()
            .enumerate()
            .map(|(index, form)| block(index, form))
            .collect::<Vec<_>>();
        let request_a = request(blocks.clone());
        let mut request_b = request(blocks.into_iter().rev().collect());
        request_b.clauses.reverse();
        let catalog_a = eight_form_program_catalog();
        let mut catalog_b = eight_form_program_catalog();
        catalog_b.forms.reverse();
        catalog_b.operators.reverse();
        catalog_b.relation_schemas.reverse();
        catalog_b.required_fact_families.reverse();
        let first = compile_relational_semantic_request(&request_a, &catalog_a, &runtime_closure())
            .expect("first");
        let second =
            compile_relational_semantic_request(&request_b, &catalog_b, &runtime_closure())
                .expect("second");
        assert_eq!(
            first.observation().compiler_proof_pin,
            second.observation().compiler_proof_pin
        );
        assert_eq!(
            first.observation().dependency_order,
            second.observation().dependency_order
        );
    }

    #[test]
    fn program_operator_rows_not_form_labels_select_the_executor_algebra() {
        let mut catalog = eight_form_program_catalog();
        let form = ReleasedSemanticForm::FindCodeEntities;
        let binding = catalog
            .forms
            .iter_mut()
            .find(|binding| binding.form_label.as_ref() == form.label())
            .unwrap();
        let relation_id = binding.output_relation_id.clone();
        let fields = binding.output_fields.clone();
        binding.root_node_id = Arc::from("catalog-limit");
        catalog.operators.push(ProgramOperatorRow {
            form_label: Arc::from(form.label()),
            output_role_id: Arc::from("result"),
            node_id: Arc::from("catalog-limit"),
            ordinal: 1,
            input_node_ids: vec![Arc::from("node.0")],
            operator: ProgramRelationalOperator::Limit { skip: 0 },
            output_fields: fields,
        });
        let mut request = request(vec![block(0, form)]);
        request.blocks[0].explicit_result_limit = Some(7);
        let compiled = compile_relational_semantic_request(&request, &catalog, &runtime_closure())
            .expect("catalog-selected limit");
        let output = compiled.blocks()[0].output().unwrap();
        assert_eq!(output.relation_id(), &relation_id);
        assert_eq!(
            output.coverage().expect("coverage").state(),
            ResultCompleteness::Unknown
        );
        assert!(matches!(
            &output.program().root,
            RelationalExpression::Limit { fetch: Some(7), .. }
        ));
        assert!(
            compiled
                .observation()
                .operators
                .contains(&SemanticCompilerOperator::Limit)
        );
    }

    #[test]
    fn provider_authority_and_judgment_are_rejected() {
        let request = request(vec![block(0, ReleasedSemanticForm::FindCodeEntities)]);
        let mut catalog = eight_form_program_catalog();
        catalog.authority = SemanticQueryAuthority::ProviderNative(Arc::from("provider"));
        assert!(matches!(
            compile_relational_semantic_request(&request, &catalog, &runtime_closure()),
            Err(RelationalSemanticQueryError::ProviderNativeAuthority(_))
        ));
        let mut catalog = eight_form_program_catalog();
        catalog.semantic_class = SemanticQueryClass::Judgment(Arc::from("risk"));
        assert!(matches!(
            compile_relational_semantic_request(&request, &catalog, &runtime_closure()),
            Err(RelationalSemanticQueryError::JudgmentSemanticClass(_))
        ));
    }

    fn epoch_ingress_limits() -> EpochBoundSemanticIngressLimits {
        EpochBoundSemanticIngressLimits::try_new(limits(), 32, 32, 16, 32, 8)
            .expect("epoch ingress limits")
    }

    fn epoch_ingress_catalog() -> EpochBoundSemanticIngressCatalog {
        let limits_pin = epoch_bound_semantic_ingress_limits_pin(epoch_ingress_limits());
        EpochBoundSemanticIngressCatalog {
            fabric_epoch_pin: [12; 32],
            program_catalog_pin: [13; 32],
            source_pin: [14; 32],
            policy_pin: [15; 32],
            producer_closure_proof_pin: [16; 32],
            limits_pin,
            program_bindings: vec![
                EpochBoundProgramBindingRow {
                    program_binding_id: Arc::from("program.entities"),
                    program_binding_pin: [17; 32],
                    compatibility_form: ReleasedSemanticForm::FindCodeEntities,
                    output_role_id: Arc::from("role.entities"),
                    execution_program_pin: [21; 32],
                },
                // The same released form deliberately has a second program. Successful validation
                // must remain unambiguous because lookup is by explicit program binding ID.
                EpochBoundProgramBindingRow {
                    program_binding_id: Arc::from("program.entities.alternate"),
                    program_binding_pin: [18; 32],
                    compatibility_form: ReleasedSemanticForm::FindCodeEntities,
                    output_role_id: Arc::from("role.entities.alternate"),
                    execution_program_pin: [23; 32],
                },
                EpochBoundProgramBindingRow {
                    program_binding_id: Arc::from("program.facts"),
                    program_binding_pin: [19; 32],
                    compatibility_form: ReleasedSemanticForm::RetrieveFactsAboutCode,
                    output_role_id: Arc::from("role.facts"),
                    execution_program_pin: [22; 32],
                },
            ],
            consumer_slots: vec![EpochBoundConsumerSlotBindingRow {
                program_binding_id: Arc::from("program.facts"),
                consumer_slot_id: Arc::from("slot.about"),
                consumer_role_id: Arc::from("role.entities"),
                minimum_edges: 1,
                maximum_edges: 1,
            }],
            selections: vec![EpochBoundSelectionBindingRow {
                program_binding_id: Arc::from("program.entities"),
                selection_id: Arc::from("selection.semantic-kind"),
                value_kind: SemanticValueKind::Text,
                minimum_values: 2,
                maximum_values: 2,
            }],
            returns: vec![
                EpochBoundReturnBindingRow {
                    program_binding_id: Arc::from("program.entities"),
                    return_id: Arc::from("return.include"),
                    value_kind: SemanticValueKind::Text,
                    minimum_values: 1,
                    maximum_values: 2,
                },
                EpochBoundReturnBindingRow {
                    program_binding_id: Arc::from("program.facts"),
                    return_id: Arc::from("return.include"),
                    value_kind: SemanticValueKind::Text,
                    minimum_values: 1,
                    maximum_values: 1,
                },
            ],
            scopes: vec![EpochBoundScopeBindingRow {
                scope_id: Arc::from("scope.workspace"),
                value_kind: SemanticValueKind::Text,
                minimum_values: 1,
                maximum_values: 1,
            }],
            request_inputs: vec![EpochBoundRequestInputBindingRow {
                program_binding_id: Arc::from("program.entities"),
                input_id: Arc::from("input.within"),
                input_relation_id: relation("request.within"),
                fields: vec![
                    EpochBoundRequestInputField {
                        field_id: field("request.within.entity-id"),
                        value_kind: SemanticValueKind::Text,
                        required: true,
                    },
                    EpochBoundRequestInputField {
                        field_id: field("request.within.representation"),
                        value_kind: SemanticValueKind::Text,
                        required: false,
                    },
                ],
                minimum_rows: 2,
                maximum_rows: 2,
            }],
        }
    }

    fn epoch_ingress() -> EpochBoundSemanticIngress {
        let limits = epoch_ingress_limits();
        EpochBoundSemanticIngress {
            semantic_request_id: Arc::from("request.epoch-bound"),
            request_content_pin: [11; 32],
            fabric_epoch_pin: [12; 32],
            program_catalog_pin: [13; 32],
            source_pin: [14; 32],
            policy_pin: [15; 32],
            producer_closure_proof_pin: [16; 32],
            limits_pin: epoch_bound_semantic_ingress_limits_pin(limits),
            limits,
            blocks: vec![
                EpochBoundBlockBindingRow {
                    query_id: Arc::from("query-facts"),
                    compatibility_form: ReleasedSemanticForm::RetrieveFactsAboutCode,
                    program_binding_id: Arc::from("program.facts"),
                    program_binding_pin: [19; 32],
                    output_role_id: Arc::from("role.facts"),
                    explicit_result_limit: Some(100),
                },
                EpochBoundBlockBindingRow {
                    query_id: Arc::from("query-entities"),
                    compatibility_form: ReleasedSemanticForm::FindCodeEntities,
                    program_binding_id: Arc::from("program.entities"),
                    program_binding_pin: [17; 32],
                    output_role_id: Arc::from("role.entities"),
                    explicit_result_limit: Some(100),
                },
            ],
            // Deliberately permuted: successful validation canonicalizes relation rows.
            selections: vec![
                EpochBoundSelectionRow {
                    query_id: Arc::from("query-entities"),
                    selection_id: Arc::from("selection.semantic-kind"),
                    ordinal: 1,
                    value: SemanticClauseValue::Text(Arc::from("function")),
                },
                EpochBoundSelectionRow {
                    query_id: Arc::from("query-entities"),
                    selection_id: Arc::from("selection.semantic-kind"),
                    ordinal: 0,
                    value: SemanticClauseValue::Text(Arc::from("module")),
                },
            ],
            returns: vec![
                EpochBoundReturnRow {
                    query_id: Arc::from("query-facts"),
                    return_id: Arc::from("return.include"),
                    ordinal: 0,
                    value: SemanticClauseValue::Text(Arc::from("fact.identity")),
                },
                EpochBoundReturnRow {
                    query_id: Arc::from("query-entities"),
                    return_id: Arc::from("return.include"),
                    ordinal: 0,
                    value: SemanticClauseValue::Text(Arc::from("entity.identity")),
                },
            ],
            scopes: vec![EpochBoundScopeRow {
                scope_id: Arc::from("scope.workspace"),
                ordinal: 0,
                value: SemanticClauseValue::Text(Arc::from(
                    "workspace:0123456789abcdef0123456789abcdef",
                )),
            }],
            request_inputs: vec![
                EpochBoundRequestInputRow {
                    query_id: Arc::from("query-entities"),
                    input_id: Arc::from("input.within"),
                    row_id: Arc::from("within-1"),
                    ordinal: 1,
                    fields: vec![EpochBoundRequestInputFieldValue {
                        field_id: field("request.within.entity-id"),
                        value: SemanticClauseValue::Text(Arc::from("entity:second")),
                    }],
                },
                EpochBoundRequestInputRow {
                    query_id: Arc::from("query-entities"),
                    input_id: Arc::from("input.within"),
                    row_id: Arc::from("within-0"),
                    ordinal: 0,
                    fields: vec![
                        EpochBoundRequestInputFieldValue {
                            field_id: field("request.within.representation"),
                            value: SemanticClauseValue::Text(Arc::from("semantic")),
                        },
                        EpochBoundRequestInputFieldValue {
                            field_id: field("request.within.entity-id"),
                            value: SemanticClauseValue::Text(Arc::from("entity:first")),
                        },
                    ],
                },
            ],
            dependencies: vec![EpochBoundDependencyRow {
                producer_query_id: Arc::from("query-entities"),
                producer_role_id: Arc::from("role.entities"),
                consumer_query_id: Arc::from("query-facts"),
                consumer_slot_id: Arc::from("slot.about"),
                consumer_role_id: Arc::from("role.entities"),
                ordinal: 0,
            }],
            dependency_order: vec![Arc::from("query-entities"), Arc::from("query-facts")],
        }
    }

    fn epoch_runtime_closure() -> ProducerClosureProof {
        let mut proof = runtime_closure();
        proof.proof_pin = [16; 32];
        proof
    }

    fn epoch_execution_catalog() -> EpochBoundSemanticExecutionCatalog {
        let within_relation = relation("request.within");
        let within_entity = field("request.within.entity-id");
        let within_representation = field("request.within.representation");
        let entities_relation = relation("result.entities");
        let entity_identity = field("result.entities.entity-id");
        let facts_relation = relation("result.facts");
        let fact_identity = field("result.facts.fact-id");
        EpochBoundSemanticExecutionCatalog {
            fabric_epoch_pin: [12; 32],
            program_catalog_pin: [13; 32],
            source_pin: [14; 32],
            policy_pin: [15; 32],
            producer_closure_proof_pin: [16; 32],
            execution_catalog_pin: [24; 32],
            program_release_pin: [25; 32],
            authority: SemanticQueryAuthority::ApplicationOwned(Arc::from("authority.application")),
            semantic_class: SemanticQueryClass::Fact(Arc::from("semantic.fact")),
            programs: vec![
                EpochBoundExecutionProgramRow {
                    program_binding_id: Arc::from("program.entities"),
                    execution_program_pin: [21; 32],
                    root_node_id: Arc::from("entities.limit"),
                    output_relation_id: entities_relation.clone(),
                    output_fields: vec![entity_identity.clone()],
                },
                EpochBoundExecutionProgramRow {
                    program_binding_id: Arc::from("program.facts"),
                    execution_program_pin: [22; 32],
                    root_node_id: Arc::from("facts.limit"),
                    output_relation_id: facts_relation.clone(),
                    output_fields: vec![fact_identity.clone()],
                },
            ],
            operators: vec![
                EpochBoundExecutionOperatorRow {
                    program_binding_id: Arc::from("program.entities"),
                    execution_program_pin: [21; 32],
                    node_id: Arc::from("entities.input"),
                    ordinal: 0,
                    input_node_ids: Vec::new(),
                    operator: ProgramRelationalOperator::Input {
                        relation_id: within_relation.clone(),
                    },
                    output_fields: vec![within_entity.clone(), within_representation.clone()],
                },
                EpochBoundExecutionOperatorRow {
                    program_binding_id: Arc::from("program.entities"),
                    execution_program_pin: [21; 32],
                    node_id: Arc::from("entities.filter"),
                    ordinal: 1,
                    input_node_ids: vec![Arc::from("entities.input")],
                    operator: ProgramRelationalOperator::Filter,
                    output_fields: vec![within_entity.clone(), within_representation.clone()],
                },
                EpochBoundExecutionOperatorRow {
                    program_binding_id: Arc::from("program.entities"),
                    execution_program_pin: [21; 32],
                    node_id: Arc::from("entities.project"),
                    ordinal: 2,
                    input_node_ids: vec![Arc::from("entities.filter")],
                    operator: ProgramRelationalOperator::Projection {
                        fields: vec![ProgramProjectionField {
                            input_field_id: within_entity.clone(),
                            output_field_id: entity_identity.clone(),
                        }],
                    },
                    output_fields: vec![entity_identity.clone()],
                },
                EpochBoundExecutionOperatorRow {
                    program_binding_id: Arc::from("program.entities"),
                    execution_program_pin: [21; 32],
                    node_id: Arc::from("entities.limit"),
                    ordinal: 3,
                    input_node_ids: vec![Arc::from("entities.project")],
                    operator: ProgramRelationalOperator::Limit { skip: 0 },
                    output_fields: vec![entity_identity.clone()],
                },
                EpochBoundExecutionOperatorRow {
                    program_binding_id: Arc::from("program.facts"),
                    execution_program_pin: [22; 32],
                    node_id: Arc::from("facts.input"),
                    ordinal: 0,
                    input_node_ids: Vec::new(),
                    operator: ProgramRelationalOperator::Input {
                        relation_id: entities_relation.clone(),
                    },
                    output_fields: vec![entity_identity.clone()],
                },
                EpochBoundExecutionOperatorRow {
                    program_binding_id: Arc::from("program.facts"),
                    execution_program_pin: [22; 32],
                    node_id: Arc::from("facts.project"),
                    ordinal: 1,
                    input_node_ids: vec![Arc::from("facts.input")],
                    operator: ProgramRelationalOperator::Projection {
                        fields: vec![ProgramProjectionField {
                            input_field_id: entity_identity.clone(),
                            output_field_id: fact_identity.clone(),
                        }],
                    },
                    output_fields: vec![fact_identity.clone()],
                },
                EpochBoundExecutionOperatorRow {
                    program_binding_id: Arc::from("program.facts"),
                    execution_program_pin: [22; 32],
                    node_id: Arc::from("facts.limit"),
                    ordinal: 2,
                    input_node_ids: vec![Arc::from("facts.project")],
                    operator: ProgramRelationalOperator::Limit { skip: 0 },
                    output_fields: vec![fact_identity.clone()],
                },
            ],
            relation_schemas: vec![
                ProgramRelationSchemaRow {
                    relation_id: within_relation.clone(),
                    fields: vec![within_entity.clone(), within_representation.clone()],
                },
                ProgramRelationSchemaRow {
                    relation_id: entities_relation.clone(),
                    fields: vec![entity_identity.clone()],
                },
                ProgramRelationSchemaRow {
                    relation_id: facts_relation,
                    fields: vec![fact_identity.clone()],
                },
            ],
            consumer_slots: vec![EpochBoundExecutionConsumerSlotRow {
                program_binding_id: Arc::from("program.facts"),
                execution_program_pin: [22; 32],
                consumer_slot_id: Arc::from("slot.about"),
                consumer_role_id: Arc::from("role.entities"),
                input_relation_id: entities_relation,
                composition: EpochBoundConsumerComposition::Single,
            }],
            selections: vec![EpochBoundExecutionSelectionRow {
                program_binding_id: Arc::from("program.entities"),
                execution_program_pin: [21; 32],
                selection_id: Arc::from("selection.semantic-kind"),
                operator_node_id: Arc::from("entities.filter"),
                input_field_id: within_representation,
                scalar_operator: ScalarOperator::Equal,
                fold: EpochBoundSelectionFold::Any,
            }],
            returns: vec![
                EpochBoundExecutionReturnRow {
                    program_binding_id: Arc::from("program.entities"),
                    execution_program_pin: [21; 32],
                    return_id: Arc::from("return.include"),
                    value: SemanticClauseValue::Text(Arc::from("entity.identity")),
                    realization_node_id: Arc::from("entities.project"),
                    realization_field_ids: vec![entity_identity],
                    realization_pin: [26; 32],
                },
                EpochBoundExecutionReturnRow {
                    program_binding_id: Arc::from("program.facts"),
                    execution_program_pin: [22; 32],
                    return_id: Arc::from("return.include"),
                    value: SemanticClauseValue::Text(Arc::from("fact.identity")),
                    realization_node_id: Arc::from("facts.project"),
                    realization_field_ids: vec![fact_identity],
                    realization_pin: [27; 32],
                },
            ],
            required_fact_families: vec![
                EpochBoundExecutionRequiredFamilyRow {
                    program_binding_id: Arc::from("program.entities"),
                    execution_program_pin: [21; 32],
                    family_id: Arc::from("family.core"),
                },
                EpochBoundExecutionRequiredFamilyRow {
                    program_binding_id: Arc::from("program.facts"),
                    execution_program_pin: [22; 32],
                    family_id: Arc::from("family.core"),
                },
            ],
            request_inputs: vec![EpochBoundExecutionRequestInputRow {
                program_binding_id: Arc::from("program.entities"),
                execution_program_pin: [21; 32],
                input_id: Arc::from("input.within"),
                input_relation_id: within_relation,
                fields: vec![
                    EpochBoundRequestInputField {
                        field_id: within_entity,
                        value_kind: SemanticValueKind::Text,
                        required: true,
                    },
                    EpochBoundRequestInputField {
                        field_id: field("request.within.representation"),
                        value_kind: SemanticValueKind::Text,
                        required: false,
                    },
                ],
                handoff_pin: [31; 32],
            }],
            scopes: vec![EpochBoundExecutionScopeRow {
                scope_id: Arc::from("scope.workspace"),
                authorization_input_id: Arc::from("authorization.workspace"),
                handoff_pin: [32; 32],
            }],
        }
    }

    fn validated_epoch_ingress() -> ValidatedEpochBoundSemanticIngress {
        validate_epoch_bound_semantic_ingress(epoch_ingress(), &epoch_ingress_catalog())
            .expect("validated epoch ingress")
    }

    #[test]
    fn epoch_bound_ingress_consumes_every_typed_relation_row_once() {
        let validated =
            validate_epoch_bound_semantic_ingress(epoch_ingress(), &epoch_ingress_catalog())
                .expect("complete epoch-bound ingress");
        assert_eq!(
            validated.consumption(),
            EpochBoundIngressConsumption {
                blocks: 2,
                selections: 2,
                returns: 2,
                scopes: 1,
                request_input_rows: 2,
                request_input_fields: 3,
                dependencies: 1,
            }
        );
        assert_eq!(validated.execution_programs()["query-entities"], [21; 32]);
        assert_eq!(validated.request_inputs().len(), 1);
        assert_eq!(
            validated.request_inputs()[0]
                .rows
                .iter()
                .map(|row| row.ordinal)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(
            validated
                .ingress()
                .blocks
                .iter()
                .map(|block| block.query_id.as_ref())
                .collect::<Vec<_>>(),
            ["query-entities", "query-facts"]
        );
    }

    #[test]
    fn epoch_bound_direct_compiler_lowers_exact_programs_returns_and_handoffs() {
        let compiled = compile_epoch_bound_semantic_request(
            &validated_epoch_ingress(),
            &epoch_execution_catalog(),
            &epoch_runtime_closure(),
        )
        .expect("direct epoch-bound compilation");

        assert_eq!(compiled.compiled().blocks().len(), 2);
        let entities = &compiled.compiled().blocks()[0];
        assert_eq!(entities.query_id().as_ref(), "query-entities");
        assert_eq!(entities.disposition(), SemanticBlockDisposition::Compiled);
        let entities_output = entities.output().expect("entities output");
        assert_eq!(entities_output.relation_id(), &relation("result.entities"));
        let RelationalExpression::Limit {
            input,
            fetch: Some(100),
            ..
        } = &entities_output.program().root
        else {
            panic!("explicit result limit must be realized by the selected program");
        };
        let RelationalExpression::Projection { input, .. } = input.as_ref() else {
            panic!("entity program must retain its catalog projection root");
        };
        let RelationalExpression::Filter { predicate, .. } = input.as_ref() else {
            panic!("entity selection must lower at the catalog filter node");
        };
        assert!(matches!(
            predicate,
            ScalarExpression::Call {
                operator: ScalarOperator::Or,
                arguments,
            } if arguments.len() == 2
        ));

        let facts = &compiled.compiled().blocks()[1];
        let facts_output = facts.output().expect("facts output");
        let RelationalExpression::Limit {
            input,
            fetch: Some(100),
            ..
        } = &facts_output.program().root
        else {
            panic!("facts result limit must be realized by the selected program");
        };
        let RelationalExpression::Projection { input, .. } = input.as_ref() else {
            panic!("facts program must retain its catalog projection root");
        };
        let RelationalExpression::Limit {
            input,
            fetch: Some(100),
            ..
        } = input.as_ref()
        else {
            panic!("consumer input must retain the complete bounded producer program");
        };
        let RelationalExpression::Projection { input, .. } = input.as_ref() else {
            panic!("composed producer must retain its catalog projection root");
        };
        assert!(matches!(
            input.as_ref(),
            RelationalExpression::Filter { .. }
        ));

        assert!(compiled.handoff().requires_query_local_binding());
        assert_eq!(compiled.handoff().request_inputs.len(), 1);
        assert_eq!(compiled.handoff().request_inputs[0].rows.len(), 2);
        assert_eq!(
            compiled.handoff().request_inputs[0]
                .program_binding_id
                .as_ref(),
            "program.entities"
        );
        assert_eq!(
            compiled.handoff().request_inputs[0].execution_program_pin,
            [21; 32]
        );
        assert_eq!(compiled.handoff().scopes.len(), 1);
        let dependencies = &compiled.compiled().observation().dependencies;
        assert!(
            dependencies.contains(&SemanticCompilerDependency::ProgramBinding {
                program_binding_id: Arc::from("program.entities"),
                execution_program_pin: [21; 32],
            })
        );
        assert!(
            dependencies.contains(&SemanticCompilerDependency::ReturnRealization {
                return_id: Arc::from("return.include"),
                realization_pin: [26; 32],
            })
        );
        assert!(
            dependencies.contains(&SemanticCompilerDependency::ReturnRealization {
                return_id: Arc::from("return.include"),
                realization_pin: [27; 32],
            })
        );
        assert!(
            dependencies.iter().all(|dependency| !matches!(
                dependency,
                SemanticCompilerDependency::FormRole { .. }
            ))
        );
    }

    #[test]
    fn epoch_bound_direct_compiler_fails_closed_on_missing_execution_contracts() {
        let validated = validated_epoch_ingress();

        let mut missing_return = epoch_execution_catalog();
        missing_return
            .returns
            .retain(|row| row.program_binding_id.as_ref() != "program.entities");
        assert!(matches!(
            compile_epoch_bound_semantic_request(
                &validated,
                &missing_return,
                &epoch_runtime_closure(),
            ),
            Err(EpochBoundSemanticCompileError::MissingBinding {
                family: "return realization",
                ..
            })
        ));

        let mut missing_handoff = epoch_execution_catalog();
        missing_handoff.request_inputs.clear();
        assert!(matches!(
            compile_epoch_bound_semantic_request(
                &validated,
                &missing_handoff,
                &epoch_runtime_closure(),
            ),
            Err(EpochBoundSemanticCompileError::RequestInputHandoff { .. })
        ));

        let mut invalid_selection = epoch_execution_catalog();
        invalid_selection.selections[0].input_field_id = field("selection.not-in-filter");
        assert!(matches!(
            compile_epoch_bound_semantic_request(
                &validated,
                &invalid_selection,
                &epoch_runtime_closure(),
            ),
            Err(EpochBoundSemanticCompileError::InvalidNode { .. })
        ));

        let mut mismatched_program = epoch_execution_catalog();
        mismatched_program.programs[0].execution_program_pin = [99; 32];
        assert!(matches!(
            compile_epoch_bound_semantic_request(
                &validated,
                &mismatched_program,
                &epoch_runtime_closure(),
            ),
            Err(EpochBoundSemanticCompileError::ExecutionProgramPinMismatch { .. })
        ));
    }

    #[test]
    fn epoch_bound_direct_compiler_preserves_unknown_producer_closure() {
        let mut closure = epoch_runtime_closure();
        closure.families.clear();
        let compiled = compile_epoch_bound_semantic_request(
            &validated_epoch_ingress(),
            &epoch_execution_catalog(),
            &closure,
        )
        .expect("unknown producer closure remains a typed block outcome");
        assert!(compiled.compiled().blocks().iter().all(|block| {
            block.disposition() == SemanticBlockDisposition::UnknownProducerClosure
                && block.output().is_none()
                && block
                    .issues()
                    .iter()
                    .any(|issue| issue.code == "PRODUCER_CLOSURE_UNKNOWN")
        }));
    }

    #[test]
    fn epoch_bound_ingress_program_id_not_compatibility_form_selects_execution() {
        let catalog = epoch_ingress_catalog();
        let validated = validate_epoch_bound_semantic_ingress(epoch_ingress(), &catalog)
            .expect("explicit program binding is unambiguous");
        assert_eq!(validated.execution_programs()["query-entities"], [21; 32]);

        let mut wrong_binding = epoch_ingress();
        let entities = wrong_binding
            .blocks
            .iter_mut()
            .find(|block| block.query_id.as_ref() == "query-entities")
            .unwrap();
        entities.program_binding_id = Arc::from("program.entities.alternate");
        assert!(matches!(
            validate_epoch_bound_semantic_ingress(wrong_binding, &catalog),
            Err(EpochBoundSemanticIngressError::ProgramBindingMismatch {
                detail: "program binding pin",
                ..
            })
        ));
    }

    #[test]
    fn epoch_bound_ingress_rejects_unconsumed_missing_and_malformed_rows() {
        let catalog = epoch_ingress_catalog();

        let mut undeclared = epoch_ingress();
        undeclared.selections.push(EpochBoundSelectionRow {
            query_id: Arc::from("query-entities"),
            selection_id: Arc::from("selection.undeclared"),
            ordinal: 0,
            value: SemanticClauseValue::Text(Arc::from("value")),
        });
        assert!(matches!(
            validate_epoch_bound_semantic_ingress(undeclared, &catalog),
            Err(EpochBoundSemanticIngressError::MissingBinding {
                family: "selection",
                ..
            })
        ));

        let mut missing = epoch_ingress();
        missing
            .returns
            .retain(|row| row.query_id.as_ref() != "query-facts");
        assert!(matches!(
            validate_epoch_bound_semantic_ingress(missing, &catalog),
            Err(EpochBoundSemanticIngressError::Cardinality {
                family: "return",
                observed: 0,
                ..
            })
        ));

        let mut malformed_input = epoch_ingress();
        malformed_input.request_inputs[0].fields.clear();
        assert!(matches!(
            validate_epoch_bound_semantic_ingress(malformed_input, &catalog),
            Err(EpochBoundSemanticIngressError::Limit {
                limit: "max_fields_per_request_input_row",
                ..
            })
        ));

        let mut wrong_order = epoch_ingress();
        wrong_order.dependency_order.reverse();
        assert!(matches!(
            validate_epoch_bound_semantic_ingress(wrong_order, &catalog),
            Err(EpochBoundSemanticIngressError::DependencyOrderMismatch)
        ));

        let mut wrong_policy = epoch_ingress();
        wrong_policy.policy_pin = [99; 32];
        assert!(matches!(
            validate_epoch_bound_semantic_ingress(wrong_policy, &catalog),
            Err(EpochBoundSemanticIngressError::PinMismatch("policy_pin"))
        ));
    }
}

/// Compatibility dimensions carried by one upstream semantic result relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticResultCompatibility {
    pub query_id: Arc<str>,
    pub workspace_id: Arc<str>,
    pub analysis_context_id: Arc<str>,
    pub representation_layer: Arc<str>,
    pub certainty_class: Arc<str>,
    pub semantic_role: Arc<str>,
}

/// One exact incompatibility detected before a set-composition plan is built.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("upstream result {query_id} differs on {dimension}: {left} != {right}")]
pub struct SemanticResultCompatibilityError {
    pub query_id: Arc<str>,
    pub dimension: &'static str,
    pub left: Arc<str>,
    pub right: Arc<str>,
}

/// Validate every semantic dimension required before composing prior-result relations.
///
/// # Errors
///
/// Returns the first fixed-order incompatible dimension; callers must reject before physical
/// planning rather than coercing workspaces, contexts, representations, certainty, or roles.
pub fn validate_semantic_result_compatibility(
    left: &SemanticResultCompatibility,
    right: &SemanticResultCompatibility,
) -> Result<(), SemanticResultCompatibilityError> {
    for (dimension, left_value, right_value) in [
        ("workspace_id", &left.workspace_id, &right.workspace_id),
        (
            "analysis_context_id",
            &left.analysis_context_id,
            &right.analysis_context_id,
        ),
        (
            "representation_layer",
            &left.representation_layer,
            &right.representation_layer,
        ),
        (
            "certainty_class",
            &left.certainty_class,
            &right.certainty_class,
        ),
        ("semantic_role", &left.semantic_role, &right.semantic_role),
    ] {
        if left_value != right_value {
            return Err(SemanticResultCompatibilityError {
                query_id: Arc::clone(&right.query_id),
                dimension,
                left: Arc::clone(left_value),
                right: Arc::clone(right_value),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_query_evidence_tests.rs"]
mod production_query_evidence_tests;
