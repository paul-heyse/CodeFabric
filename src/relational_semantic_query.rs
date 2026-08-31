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
use crate::relational_program::{
    AggregateExpression, AggregateOperator, FieldId, JoinKind, NamedAggregateExpression,
    NamedExpression, RelationId, RelationalExpression, RelationalProgram, ScalarExpression,
    ScalarOperator, SortExpression, UnionKind,
};

pub const RELATIONAL_SEMANTIC_QUERY_COMPILER_RELEASE: &str =
    "codefabric.relational-semantic-query.datafusion-55.v1";

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
    pub program_compiler_release_pin: [u8; 32],
    pub authority: SemanticQueryAuthority,
    pub semantic_class: SemanticQueryClass,
    pub forms: Vec<ProgramFormBindingRow>,
    pub input_roles: Vec<ProgramInputRoleBindingRow>,
    pub clauses: Vec<ProgramClauseBindingRow>,
    pub operators: Vec<ProgramOperatorRow>,
    pub relation_schemas: Vec<ProgramRelationSchemaRow>,
    pub required_fact_families: Vec<ProgramRequiredFactFamilyRow>,
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
    ProgramCatalog([u8; 32]),
    SourcePin([u8; 32]),
    PolicyPin([u8; 32]),
    ProducerClosureProof([u8; 32]),
    CompilerRelease([u8; 32]),
    Authority(Arc<str>),
    SemanticClass(Arc<str>),
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
    OperatorNode {
        node_id: Arc<str>,
        binding_pin: [u8; 32],
    },
    Clause {
        clause_id: Arc<str>,
        binding_pin: [u8; 32],
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
    pub compiler_release: &'static str,
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
    validate_pin(
        "program_compiler_release_pin",
        catalog.program_compiler_release_pin,
    )?;
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
        SemanticCompilerDependency::CompilerRelease(program_catalog.program_compiler_release_pin),
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
            compiler_release: RELATIONAL_SEMANTIC_QUERY_COMPILER_RELEASE,
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
                detail: "compiled predecessor relation differs from its catalog role".to_owned(),
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
        RELATIONAL_SEMANTIC_QUERY_COMPILER_RELEASE.as_bytes(),
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

#[cfg(test)]
mod tests {
    use super::*;

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
            program_compiler_release_pin: [4; 32],
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
            program_compiler_release_pin: [4; 32],
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
            program_compiler_release_pin: [4; 32],
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
}
