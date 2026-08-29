//! Application-owned logical-plan ID-domain conformance.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::{Array as _, BooleanArray, StringArray};
use arrow_schema::Field;
use datafusion::common::config::ConfigOptions;
use datafusion::common::tree_node::{TreeNode, TreeNodeRecursion};
use datafusion::common::{DataFusionError, ExprSchema, Result};
use datafusion::logical_expr::{Expr, LogicalPlan, Operator};
use datafusion::optimizer::analyzer::AnalyzerRule;

/// Generated-state projection used by the total analyzer transition lattice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainState {
    Domain(String),
    Neutral,
    Bottom,
    Opaque,
}

/// Effect class assigned to every pinned expression form.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DomainEffect {
    None,
    Preserve,
    ConsumeSameDomain,
    Produce,
    ExplicitErase,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DomainStateKind {
    Domain,
    Neutral,
    Bottom,
    Opaque,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DomainTransition {
    output: DomainStateKind,
    allowed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DomainOperation {
    effect: DomainEffect,
    domain_functions: BTreeSet<String>,
}

/// Immutable semantic policy decoded from the ontology program package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainOperationPolicy {
    identity: String,
    version: String,
    operations: BTreeMap<String, DomainOperation>,
    transitions: BTreeMap<(DomainStateKind, DomainEffect), DomainTransition>,
    comparison_domain_pairs: BTreeSet<(String, String)>,
}

impl DomainOperationPolicy {
    /// Decode and validate the complete policy artifact selected by one package.
    ///
    /// # Errors
    ///
    /// Rejects missing or malformed policy members, duplicate or incomplete expression and
    /// transition coverage, inconsistent versions, or invalid comparison-domain pairs.
    #[allow(clippy::too_many_lines)] // Validation is intentionally one auditable package boundary.
    pub fn from_package(package: &crate::ontology_program::OntologyProgramPackage) -> Result<Self> {
        let operation_member = package
            .members
            .get("program.domain_operation_policy")
            .ok_or_else(|| {
                DataFusionError::Plan("package has no domain operation policy".into())
            })?;
        let transition_member = package
            .members
            .get("program.domain_transition_policy")
            .ok_or_else(|| {
                DataFusionError::Plan("package has no domain transition policy".into())
            })?;
        let comparison_member = package
            .members
            .get("program.domain_comparison_policy")
            .ok_or_else(|| {
                DataFusionError::Plan("package has no domain comparison policy".into())
            })?;
        let operation = single_batch(operation_member)?;
        let transition = single_batch(transition_member)?;
        let comparison = single_batch(comparison_member)?;
        let operation_versions = strings(operation, "policy_version")?;
        let variants = strings(operation, "expression_variant")?;
        let effects = strings(operation, "effect")?;
        let functions = strings(operation, "domain_functions")?;
        let mut operations = BTreeMap::new();
        let mut versions = BTreeSet::new();
        for row in 0..operation.num_rows() {
            versions.insert(operation_versions.value(row).to_owned());
            let entry = DomainOperation {
                effect: parse_effect(effects.value(row))?,
                domain_functions: functions
                    .value(row)
                    .split(',')
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
                    .collect(),
            };
            if operations
                .insert(variants.value(row).to_owned(), entry)
                .is_some()
            {
                return domain_error("duplicate expression variant in domain policy");
            }
        }
        if operations
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != DATAFUSION_EXPR_VARIANT_CENSUS.iter().copied().collect()
        {
            return domain_error("domain policy does not cover the pinned expression census");
        }
        let transition_versions = strings(transition, "policy_version")?;
        let input_states = strings(transition, "input_state")?;
        let transition_effects = strings(transition, "effect")?;
        let output_states = strings(transition, "output_state")?;
        let allowed = transition
            .column_by_name("allowed")
            .and_then(|column| column.as_any().downcast_ref::<BooleanArray>())
            .ok_or_else(|| DataFusionError::Plan("domain policy allowed is not Boolean".into()))?;
        let mut transitions = BTreeMap::new();
        for row in 0..transition.num_rows() {
            versions.insert(transition_versions.value(row).to_owned());
            if transitions
                .insert(
                    (
                        parse_state(input_states.value(row))?,
                        parse_effect(transition_effects.value(row))?,
                    ),
                    DomainTransition {
                        output: parse_state(output_states.value(row))?,
                        allowed: allowed.value(row),
                    },
                )
                .is_some()
            {
                return domain_error("duplicate domain transition");
            }
        }
        if transitions.len() != 20 {
            return domain_error("domain transition lattice is not total");
        }
        let comparison_versions = strings(comparison, "policy_version")?;
        let left = strings(comparison, "left_domain")?;
        let right = strings(comparison, "right_domain")?;
        let mut comparison_domain_pairs = BTreeSet::new();
        for row in 0..comparison.num_rows() {
            versions.insert(comparison_versions.value(row).to_owned());
            let mut pair = [left.value(row).to_owned(), right.value(row).to_owned()];
            pair.sort();
            if pair[0] == pair[1]
                || !comparison_domain_pairs.insert((pair[0].clone(), pair[1].clone()))
            {
                return domain_error("invalid comparison-domain policy pair");
            }
        }
        if versions.len() != 1 {
            return domain_error("domain policy members select different versions");
        }
        let version = versions
            .pop_first()
            .ok_or_else(|| DataFusionError::Plan("domain policy has no version".into()))?;
        let identity = crate::integrity::framed_digest(
            [
                b"domain-operation-policy.v1".as_slice(),
                operation_member.member_identity.as_bytes(),
                transition_member.member_identity.as_bytes(),
                comparison_member.member_identity.as_bytes(),
            ]
            .concat()
            .as_slice(),
        );
        Ok(Self {
            identity,
            version,
            operations,
            transitions,
            comparison_domain_pairs,
        })
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Result-authority policy identity bound to this exact generated artifact.
    #[must_use]
    pub fn result_policy_identity(&self) -> String {
        let mut bytes = Vec::new();
        for part in [b"candidate-policy.v1".as_slice(), self.identity.as_bytes()] {
            bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
            bytes.extend_from_slice(part);
        }
        crate::integrity::framed_digest(&bytes)
    }

    fn operation(&self, variant: &str) -> Result<&DomainOperation> {
        self.operations.get(variant).ok_or_else(|| {
            DataFusionError::Plan(format!("domain policy has no transition for {variant}"))
        })
    }

    fn transition(&self, state: DomainStateKind, effect: DomainEffect) -> Result<DomainTransition> {
        self.transitions
            .get(&(state, effect))
            .copied()
            .ok_or_else(|| {
                DataFusionError::Plan("domain policy transition lattice is incomplete".into())
            })
    }

    pub(crate) fn function_allowed(&self, variant: &str, function: &str) -> Result<bool> {
        Ok(self.operation(variant)?.domain_functions.contains(function))
    }

    fn comparison_allowed(&self, left: &str, right: &str) -> bool {
        if left == right {
            return true;
        }
        let mut pair = [left.to_owned(), right.to_owned()];
        pair.sort();
        self.comparison_domain_pairs
            .contains(&(pair[0].clone(), pair[1].clone()))
    }
}

fn single_batch(
    member: &crate::ontology_program::OntologyProgramMember,
) -> Result<&arrow_array::RecordBatch> {
    if member.batches.len() != 1 {
        return domain_error("domain policy member is not one canonical batch");
    }
    Ok(&member.batches[0])
}

fn strings<'a>(batch: &'a arrow_array::RecordBatch, name: &str) -> Result<&'a StringArray> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .ok_or_else(|| DataFusionError::Plan(format!("domain policy {name} is not Utf8")))
}

fn parse_effect(value: &str) -> Result<DomainEffect> {
    if value == "NONE" {
        Ok(DomainEffect::None)
    } else if value == "PRESERVE" {
        Ok(DomainEffect::Preserve)
    } else if value == "CONSUME_SAME_DOMAIN" {
        Ok(DomainEffect::ConsumeSameDomain)
    } else if value == "PRODUCE" {
        Ok(DomainEffect::Produce)
    } else if value == "EXPLICIT_ERASE" {
        Ok(DomainEffect::ExplicitErase)
    } else {
        domain_error(format!("unknown domain effect {value}"))
    }
}

fn parse_state(value: &str) -> Result<DomainStateKind> {
    if value == "DOMAIN" {
        Ok(DomainStateKind::Domain)
    } else if value == "NEUTRAL" {
        Ok(DomainStateKind::Neutral)
    } else if value == "BOTTOM" {
        Ok(DomainStateKind::Bottom)
    } else if value == "OPAQUE" {
        Ok(DomainStateKind::Opaque)
    } else {
        domain_error(format!("unknown domain state {value}"))
    }
}

/// Compile-time census for the pinned DataFusion 55 expression enum.
pub use crate::ontology_contract::DATAFUSION_EXPR_VARIANT_CENSUS;

/// Compile-time census for the pinned DataFusion 55 logical-plan enum.
pub const DATAFUSION_LOGICAL_PLAN_VARIANT_CENSUS: &[&str] = &[
    "Projection",
    "Filter",
    "Window",
    "Aggregate",
    "Sort",
    "Join",
    "Repartition",
    "Union",
    "TableScan",
    "EmptyRelation",
    "Subquery",
    "SubqueryAlias",
    "Limit",
    "Statement",
    "Values",
    "Explain",
    "Analyze",
    "Extension",
    "Distinct",
    "Dml",
    "Ddl",
    "Copy",
    "DescribeTable",
    "Unnest",
    "RecursiveQuery",
];

/// Single idempotent analyzer installed in every serving session.
///
/// The rule does not rewrite valid plans. It rejects domain mismatches across every plan
/// constructor that reaches DataFusion analysis, so binder diagnostics can never become a
/// separate policy authority.
#[derive(Debug)]
pub struct DomainConformanceRule {
    policy: Arc<DomainOperationPolicy>,
}

impl DomainConformanceRule {
    #[must_use]
    pub fn new(policy: Arc<DomainOperationPolicy>) -> Self {
        Self { policy }
    }
}

impl AnalyzerRule for DomainConformanceRule {
    fn analyze(&self, plan: LogicalPlan, _config: &ConfigOptions) -> Result<LogicalPlan> {
        plan.apply_with_subqueries(|node| {
            validate_plan_variant(node)?;
            validate_set_alignment(&self.policy, node)?;
            for field in node.schema().fields() {
                let _ = field_state(field.as_ref())?;
                crate::schema_registry::validate_logical_extension_field(field.as_ref())
                    .map_err(|error| DataFusionError::Plan(error.to_string()))?;
            }
            for expression in node.expressions() {
                expression.apply(|nested| {
                    validate_expression(&self.policy, node, nested)?;
                    Ok(TreeNodeRecursion::Continue)
                })?;
            }
            Ok(TreeNodeRecursion::Continue)
        })?;
        Ok(plan)
    }

    fn name(&self) -> &'static str {
        "codefabric_domain_conformance"
    }
}

/// Apply the application-owned analyzer independently of a session.
///
/// The sealed session uses this second pass to prove that the final application rule is
/// idempotent after DataFusion's built-in analyzer chain has completed.
pub(crate) fn analyze_governed_plan(
    plan: LogicalPlan,
    policy: Arc<DomainOperationPolicy>,
) -> Result<LogicalPlan> {
    DomainConformanceRule::new(policy).analyze(plan, &ConfigOptions::default())
}

#[allow(clippy::too_many_lines)] // Exhaustive DataFusion Expr coverage is intentionally centralized.
fn validate_expression(
    policy: &DomainOperationPolicy,
    plan: &LogicalPlan,
    expression: &Expr,
) -> Result<()> {
    let variant = expression_variant(expression);
    let effect = policy.operation(variant)?.effect;
    #[allow(deprecated)]
    match expression {
        Expr::Alias(_)
        | Expr::Column(_)
        | Expr::ScalarVariable(_, _)
        | Expr::Literal(_, _)
        | Expr::OuterReferenceColumn(_, _)
        | Expr::LambdaVariable(_)
        | Expr::Exists(_)
        | Expr::ScalarSubquery(_) => {}
        Expr::BinaryExpr(binary) => {
            let left = expression_state(policy, plan, &binary.left)?;
            let right = expression_state(policy, plan, &binary.right)?;
            if is_domain_bearing(&left) || is_domain_bearing(&right) {
                if !matches!(
                    binary.op,
                    Operator::Eq
                        | Operator::NotEq
                        | Operator::Lt
                        | Operator::LtEq
                        | Operator::Gt
                        | Operator::GtEq
                        | Operator::IsDistinctFrom
                        | Operator::IsNotDistinctFrom
                ) {
                    return domain_error(format!(
                        "operator {} is not defined for logical ID domains",
                        binary.op
                    ));
                }
                require_compatible_states(policy, left, right, "binary comparison")?;
            }
        }
        Expr::InList(in_list) => {
            let value_state = expression_state(policy, plan, &in_list.expr)?;
            for candidate in &in_list.list {
                require_compatible_states(
                    policy,
                    value_state.clone(),
                    expression_state(policy, plan, candidate)?,
                    "IN-list member",
                )?;
            }
        }
        Expr::Cast(cast) => {
            if let DomainState::Domain(domain) = expression_state(policy, plan, &cast.expr)? {
                return domain_error(format!("explicit cast erases logical ID domain {domain}"));
            }
        }
        Expr::TryCast(cast) => {
            if let DomainState::Domain(domain) = expression_state(policy, plan, &cast.expr)? {
                return domain_error(format!("try-cast erases logical ID domain {domain}"));
            }
        }
        Expr::Like(like) | Expr::SimilarTo(like) => {
            reject_domain_arguments(
                policy,
                plan,
                [like.expr.as_ref(), like.pattern.as_ref()],
                "pattern operation",
            )?;
        }
        Expr::Not(value)
        | Expr::IsNotNull(value)
        | Expr::IsNull(value)
        | Expr::IsTrue(value)
        | Expr::IsFalse(value)
        | Expr::IsUnknown(value)
        | Expr::IsNotTrue(value)
        | Expr::IsNotFalse(value)
        | Expr::IsNotUnknown(value) => {
            ensure_not_opaque(
                expression_state(policy, plan, value)?,
                "boolean/null predicate",
            )?;
        }
        Expr::Negative(value) => {
            reject_domain_arguments(policy, plan, [value.as_ref()], "numeric negation")?;
        }
        Expr::Between(between) => {
            let state = expression_state(policy, plan, &between.expr)?;
            require_compatible_states(
                policy,
                state.clone(),
                expression_state(policy, plan, &between.low)?,
                "BETWEEN low",
            )?;
            require_compatible_states(
                policy,
                state,
                expression_state(policy, plan, &between.high)?,
                "BETWEEN high",
            )?;
        }
        Expr::Case(case) => {
            if let Some(value) = &case.expr {
                let state = expression_state(policy, plan, value)?;
                for (when, _) in &case.when_then_expr {
                    require_compatible_states(
                        policy,
                        state.clone(),
                        expression_state(policy, plan, when)?,
                        "simple CASE match",
                    )?;
                }
            } else {
                for (when, _) in &case.when_then_expr {
                    match expression_state(policy, plan, when)? {
                        DomainState::Neutral | DomainState::Bottom => {}
                        state => {
                            return domain_error(format!(
                                "searched CASE predicate is not neutral: {state:?}"
                            ));
                        }
                    }
                }
            }
            ensure_not_opaque(expression_state(policy, plan, expression)?, "CASE result")?;
        }
        Expr::ScalarFunction(function) => {
            ensure_not_opaque(
                expression_state(policy, plan, expression)?,
                &format!("scalar function {}", function.func.name()),
            )?;
        }
        Expr::AggregateFunction(function) => {
            let has_domain_argument = function
                .params
                .args
                .iter()
                .map(|argument| expression_state(policy, plan, argument))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .any(|state| is_domain_bearing(&state));
            if has_domain_argument
                && !policy.function_allowed("AggregateFunction", function.func.name())?
            {
                return domain_error(format!(
                    "aggregate function {} has no generated ID-domain transition",
                    function.func.name()
                ));
            }
            if let Some(filter) = &function.params.filter {
                ensure_not_opaque(expression_state(policy, plan, filter)?, "aggregate filter")?;
            }
            for sort in &function.params.order_by {
                ensure_not_opaque(
                    expression_state(policy, plan, &sort.expr)?,
                    "aggregate ordering",
                )?;
            }
        }
        Expr::WindowFunction(function) => {
            ensure_not_opaque(
                expression_state(policy, plan, expression)?,
                &format!("window function {}", function.fun.name()),
            )?;
            for partition in &function.params.partition_by {
                ensure_not_opaque(
                    expression_state(policy, plan, partition)?,
                    "window partition",
                )?;
            }
            for sort in &function.params.order_by {
                ensure_not_opaque(
                    expression_state(policy, plan, &sort.expr)?,
                    "window ordering",
                )?;
            }
        }
        Expr::InSubquery(subquery) => {
            require_compatible_states(
                policy,
                expression_state(policy, plan, &subquery.expr)?,
                subquery_output_state(&subquery.subquery.subquery)?,
                "IN-subquery",
            )?;
        }
        Expr::SetComparison(comparison) => {
            require_compatible_states(
                policy,
                expression_state(policy, plan, &comparison.expr)?,
                subquery_output_state(&comparison.subquery.subquery)?,
                "set comparison",
            )?;
        }
        Expr::Wildcard { .. } => {
            return domain_error("unresolved wildcard reached the application analyzer");
        }
        Expr::GroupingSet(grouping) => {
            for value in grouping.distinct_expr() {
                ensure_not_opaque(expression_state(policy, plan, value)?, "grouping set")?;
            }
        }
        Expr::Placeholder(_) => {
            return domain_error("unresolved prepared-statement placeholder reached analyzer");
        }
        Expr::Unnest(unnest) => {
            reject_domain_arguments(policy, plan, [unnest.expr.as_ref()], "unnest")?;
        }
        Expr::HigherOrderFunction(function) => {
            reject_domain_arguments(policy, plan, function.args.iter(), "higher-order function")?;
        }
        Expr::Lambda(lambda) => {
            reject_domain_arguments(policy, plan, [lambda.body.as_ref()], "lambda")?;
        }
    }
    let state = expression_state(policy, plan, expression)?;
    let state_kind = match &state {
        DomainState::Domain(_) => DomainStateKind::Domain,
        DomainState::Neutral => DomainStateKind::Neutral,
        DomainState::Bottom => DomainStateKind::Bottom,
        DomainState::Opaque => DomainStateKind::Opaque,
    };
    let transition = policy.transition(state_kind, effect)?;
    if state_kind == DomainStateKind::Opaque && !transition.allowed {
        return domain_error(format!(
            "{variant} reaches forbidden OPAQUE/{effect:?} transition to {:?}",
            transition.output
        ));
    }
    match effect {
        DomainEffect::ConsumeSameDomain | DomainEffect::Produce => {
            ensure_not_opaque(state, "generated domain-effect transition")?;
        }
        DomainEffect::None | DomainEffect::Preserve | DomainEffect::ExplicitErase => {}
    }
    Ok(())
}

#[allow(deprecated)]
const fn expression_variant(expression: &Expr) -> &'static str {
    match expression {
        Expr::Alias(_) => "Alias",
        Expr::Column(_) => "Column",
        Expr::ScalarVariable(_, _) => "ScalarVariable",
        Expr::Literal(_, _) => "Literal",
        Expr::BinaryExpr(_) => "BinaryExpr",
        Expr::Like(_) => "Like",
        Expr::SimilarTo(_) => "SimilarTo",
        Expr::Not(_) => "Not",
        Expr::IsNotNull(_) => "IsNotNull",
        Expr::IsNull(_) => "IsNull",
        Expr::IsTrue(_) => "IsTrue",
        Expr::IsFalse(_) => "IsFalse",
        Expr::IsUnknown(_) => "IsUnknown",
        Expr::IsNotTrue(_) => "IsNotTrue",
        Expr::IsNotFalse(_) => "IsNotFalse",
        Expr::IsNotUnknown(_) => "IsNotUnknown",
        Expr::Negative(_) => "Negative",
        Expr::Between(_) => "Between",
        Expr::Case(_) => "Case",
        Expr::Cast(_) => "Cast",
        Expr::TryCast(_) => "TryCast",
        Expr::ScalarFunction(_) => "ScalarFunction",
        Expr::AggregateFunction(_) => "AggregateFunction",
        Expr::WindowFunction(_) => "WindowFunction",
        Expr::InList(_) => "InList",
        Expr::Exists(_) => "Exists",
        Expr::InSubquery(_) => "InSubquery",
        Expr::SetComparison(_) => "SetComparison",
        Expr::ScalarSubquery(_) => "ScalarSubquery",
        Expr::Wildcard { .. } => "Wildcard",
        Expr::GroupingSet(_) => "GroupingSet",
        Expr::Placeholder(_) => "Placeholder",
        Expr::OuterReferenceColumn(_, _) => "OuterReferenceColumn",
        Expr::Unnest(_) => "Unnest",
        Expr::HigherOrderFunction(_) => "HigherOrderFunction",
        Expr::Lambda(_) => "Lambda",
        Expr::LambdaVariable(_) => "LambdaVariable",
    }
}

#[allow(clippy::needless_pass_by_value)] // Callers produce a fresh lattice value for this terminal check.
fn ensure_not_opaque(state: DomainState, operation: &str) -> Result<()> {
    if state == DomainState::Opaque {
        return domain_error(format!(
            "{operation} produces an opaque value from domain-bearing input"
        ));
    }
    Ok(())
}

const fn is_domain_bearing(state: &DomainState) -> bool {
    matches!(state, DomainState::Domain(_) | DomainState::Opaque)
}

fn join_states(left: DomainState, right: DomainState) -> DomainState {
    match (left, right) {
        (DomainState::Opaque, _) | (_, DomainState::Opaque) => DomainState::Opaque,
        (DomainState::Bottom, state) | (state, DomainState::Bottom) => state,
        (DomainState::Neutral, DomainState::Neutral) => DomainState::Neutral,
        (DomainState::Domain(left), DomainState::Domain(right)) if left == right => {
            DomainState::Domain(left)
        }
        (DomainState::Neutral | DomainState::Domain(_), DomainState::Domain(_))
        | (DomainState::Domain(_), DomainState::Neutral) => DomainState::Opaque,
    }
}

#[allow(clippy::needless_pass_by_value)] // Owned states keep diagnostics available after the join.
fn require_compatible_states(
    policy: &DomainOperationPolicy,
    left: DomainState,
    right: DomainState,
    operation: &str,
) -> Result<()> {
    if let (DomainState::Domain(left_domain), DomainState::Domain(right_domain)) = (&left, &right)
        && policy.comparison_allowed(left_domain, right_domain)
    {
        return Ok(());
    }
    match join_states(left.clone(), right.clone()) {
        DomainState::Opaque => domain_error(format!(
            "{operation} combines incompatible states {left:?} and {right:?}"
        )),
        DomainState::Bottom | DomainState::Neutral | DomainState::Domain(_) => Ok(()),
    }
}

fn subquery_output_state(plan: &LogicalPlan) -> Result<DomainState> {
    let fields = plan.schema().fields();
    if fields.len() != 1 {
        return domain_error("scalar/set subquery must expose exactly one field");
    }
    field_state(fields[0].as_ref())
}

#[allow(clippy::too_many_lines)] // Exhaustive state algebra mirrors the pinned Expr census.
fn expression_state(
    policy: &DomainOperationPolicy,
    plan: &LogicalPlan,
    expression: &Expr,
) -> Result<DomainState> {
    #[allow(deprecated)]
    match expression {
        Expr::Alias(alias) => expression_state(policy, plan, &alias.expr),
        Expr::Column(_) | Expr::OuterReferenceColumn(_, _) => expression_domain(plan, expression)
            .map(|domain| {
                domain.map_or(DomainState::Neutral, |value| {
                    DomainState::Domain(value.into())
                })
            }),
        Expr::Literal(value, metadata) => {
            if value.is_null() {
                Ok(DomainState::Bottom)
            } else if let Some(name) = metadata.as_ref().and_then(|metadata| {
                metadata
                    .inner()
                    .get(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY)
            }) {
                extension_domain(name).map(|domain| {
                    domain.map_or(DomainState::Neutral, |value| {
                        DomainState::Domain(value.into())
                    })
                })
            } else {
                Ok(DomainState::Neutral)
            }
        }
        Expr::Cast(cast) => match expression_state(policy, plan, &cast.expr)? {
            DomainState::Domain(_) | DomainState::Opaque => Ok(DomainState::Opaque),
            DomainState::Bottom => Ok(DomainState::Bottom),
            DomainState::Neutral => Ok(DomainState::Neutral),
        },
        Expr::TryCast(cast) => match expression_state(policy, plan, &cast.expr)? {
            DomainState::Domain(_) | DomainState::Opaque => Ok(DomainState::Opaque),
            DomainState::Bottom => Ok(DomainState::Bottom),
            DomainState::Neutral => Ok(DomainState::Neutral),
        },
        Expr::Case(case) => {
            let mut state = DomainState::Bottom;
            for (_, then) in &case.when_then_expr {
                state = join_states(state, expression_state(policy, plan, then)?);
            }
            if let Some(otherwise) = &case.else_expr {
                state = join_states(state, expression_state(policy, plan, otherwise)?);
            }
            Ok(state)
        }
        Expr::ScalarFunction(function) => {
            let states = function
                .args
                .iter()
                .map(|argument| expression_state(policy, plan, argument))
                .collect::<Result<Vec<_>>>()?;
            if policy.function_allowed("ScalarFunction", function.func.name())? {
                Ok(states.into_iter().fold(DomainState::Bottom, join_states))
            } else if states
                .iter()
                .any(|state| matches!(state, DomainState::Domain(_) | DomainState::Opaque))
            {
                Ok(DomainState::Opaque)
            } else {
                Ok(DomainState::Neutral)
            }
        }
        Expr::AggregateFunction(function) => {
            let first = function
                .params
                .args
                .first()
                .map_or(Ok(DomainState::Neutral), |argument| {
                    expression_state(policy, plan, argument)
                })?;
            if function.func.name() == "count" {
                Ok(DomainState::Neutral)
            } else if policy.function_allowed("AggregateFunction", function.func.name())? {
                Ok(first)
            } else if matches!(first, DomainState::Domain(_) | DomainState::Opaque) {
                Ok(DomainState::Opaque)
            } else {
                Ok(DomainState::Neutral)
            }
        }
        Expr::WindowFunction(function) => {
            let first = function
                .params
                .args
                .first()
                .map_or(Ok(DomainState::Neutral), |argument| {
                    expression_state(policy, plan, argument)
                })?;
            if policy.function_allowed("WindowFunction", function.fun.name())? {
                Ok(first)
            } else if matches!(first, DomainState::Domain(_) | DomainState::Opaque) {
                Ok(DomainState::Opaque)
            } else {
                Ok(DomainState::Neutral)
            }
        }
        Expr::ScalarSubquery(subquery) => subquery_output_state(&subquery.subquery),
        Expr::BinaryExpr(_)
        | Expr::Like(_)
        | Expr::SimilarTo(_)
        | Expr::Not(_)
        | Expr::IsNotNull(_)
        | Expr::IsNull(_)
        | Expr::IsTrue(_)
        | Expr::IsFalse(_)
        | Expr::IsUnknown(_)
        | Expr::IsNotTrue(_)
        | Expr::IsNotFalse(_)
        | Expr::IsNotUnknown(_)
        | Expr::Negative(_)
        | Expr::Between(_)
        | Expr::InList(_)
        | Expr::Exists(_)
        | Expr::InSubquery(_)
        | Expr::SetComparison(_)
        | Expr::GroupingSet(_) => Ok(DomainState::Neutral),
        Expr::ScalarVariable(_, _)
        | Expr::Placeholder(_)
        | Expr::Unnest(_)
        | Expr::HigherOrderFunction(_)
        | Expr::Lambda(_)
        | Expr::LambdaVariable(_) => Ok(DomainState::Opaque),
        Expr::Wildcard { .. } => {
            domain_error("unresolved wildcard is forbidden after governed resolution")
        }
    }
}

fn reject_domain_arguments<'a>(
    policy: &DomainOperationPolicy,
    plan: &'a LogicalPlan,
    expressions: impl IntoIterator<Item = &'a Expr>,
    operation: &str,
) -> Result<()> {
    for expression in expressions {
        match expression_state(policy, plan, expression)? {
            DomainState::Domain(domain) => {
                return domain_error(format!(
                    "{operation} has no generated transition for ID domain {domain}"
                ));
            }
            DomainState::Opaque => {
                return domain_error(format!(
                    "{operation} has an opaque nested-domain transition"
                ));
            }
            DomainState::Neutral | DomainState::Bottom => {}
        }
    }
    Ok(())
}

fn expression_domain<'a>(plan: &'a LogicalPlan, expression: &'a Expr) -> Result<Option<&'a str>> {
    #[allow(deprecated)]
    match expression {
        Expr::Alias(alias) => expression_domain(plan, &alias.expr),
        Expr::Column(column) => {
            for input in plan.inputs() {
                if let Ok(field) = input.schema().field_from_column(column) {
                    return field_domain(field.as_ref());
                }
            }
            plan.schema()
                .field_from_column(column)
                .map_or(Ok(None), |field| field_domain(field.as_ref()))
        }
        Expr::Literal(_, metadata) => metadata.as_ref().map_or(Ok(None), |metadata| {
            metadata
                .inner()
                .get(arrow_schema::extension::EXTENSION_TYPE_NAME_KEY)
                .map_or(Ok(None), |name| extension_domain(name))
        }),
        Expr::Cast(cast) => expression_domain(plan, &cast.expr),
        Expr::TryCast(cast) => expression_domain(plan, &cast.expr),
        Expr::OuterReferenceColumn(field, _) => field_domain(field.as_ref()),
        Expr::ScalarVariable(_, _)
        | Expr::BinaryExpr(_)
        | Expr::Like(_)
        | Expr::SimilarTo(_)
        | Expr::Not(_)
        | Expr::IsNotNull(_)
        | Expr::IsNull(_)
        | Expr::IsTrue(_)
        | Expr::IsFalse(_)
        | Expr::IsUnknown(_)
        | Expr::IsNotTrue(_)
        | Expr::IsNotFalse(_)
        | Expr::IsNotUnknown(_)
        | Expr::Negative(_)
        | Expr::Between(_)
        | Expr::Case(_)
        | Expr::ScalarFunction(_)
        | Expr::AggregateFunction(_)
        | Expr::WindowFunction(_)
        | Expr::InList(_)
        | Expr::Exists(_)
        | Expr::InSubquery(_)
        | Expr::SetComparison(_)
        | Expr::ScalarSubquery(_)
        | Expr::GroupingSet(_)
        | Expr::Placeholder(_)
        | Expr::Unnest(_)
        | Expr::HigherOrderFunction(_)
        | Expr::Lambda(_)
        | Expr::LambdaVariable(_) => Ok(None),
        Expr::Wildcard { .. } => {
            domain_error("unresolved wildcard is forbidden after governed resolution")
        }
    }
}

fn validate_plan_variant(plan: &LogicalPlan) -> Result<()> {
    match plan {
        LogicalPlan::Projection(_)
        | LogicalPlan::Filter(_)
        | LogicalPlan::Window(_)
        | LogicalPlan::Aggregate(_)
        | LogicalPlan::Sort(_)
        | LogicalPlan::Join(_)
        | LogicalPlan::Repartition(_)
        | LogicalPlan::Union(_)
        | LogicalPlan::TableScan(_)
        | LogicalPlan::EmptyRelation(_)
        | LogicalPlan::Subquery(_)
        | LogicalPlan::SubqueryAlias(_)
        | LogicalPlan::Limit(_)
        | LogicalPlan::Values(_)
        | LogicalPlan::Distinct(_)
        | LogicalPlan::Unnest(_)
        | LogicalPlan::RecursiveQuery(_) => Ok(()),
        LogicalPlan::Statement(_) => domain_error("statement plan is not governed query ingress"),
        LogicalPlan::Explain(_) => domain_error("EXPLAIN is diagnostic-only and not gate ingress"),
        LogicalPlan::Analyze(_) => domain_error("ANALYZE/EXPLAIN ANALYZE is forbidden"),
        LogicalPlan::Extension(_) => domain_error("custom logical extension nodes are forbidden"),
        LogicalPlan::Dml(_) => domain_error("DML is forbidden in governed query ingress"),
        LogicalPlan::Ddl(_) => domain_error("DDL is forbidden in governed query ingress"),
        LogicalPlan::Copy(_) => domain_error("COPY is forbidden in governed query ingress"),
        LogicalPlan::DescribeTable(_) => {
            domain_error("DESCRIBE is forbidden in governed query ingress")
        }
    }
}

fn field_domain(field: &Field) -> Result<Option<&str>> {
    field
        .extension_type_name()
        .map_or(Ok(None), extension_domain)
}

fn field_state(field: &Field) -> Result<DomainState> {
    field_domain(field).map(|domain| {
        domain.map_or(DomainState::Neutral, |value| {
            DomainState::Domain(value.to_owned())
        })
    })
}

fn extension_domain(extension_name: &str) -> Result<Option<&str>> {
    if let Some(domain) = crate::schema_registry::id_domain_for_extension_name(extension_name) {
        return Ok(Some(domain));
    }
    if extension_name.starts_with("codefabric.")
        && extension_name != crate::schema_registry::Hash32Extension::NAME
    {
        return domain_error(format!(
            "unknown CodeFabric logical extension type {extension_name}"
        ));
    }
    Ok(None)
}

fn require_same_domain(left: Option<&str>, right: Option<&str>, operation: &str) -> Result<()> {
    match (left, right) {
        (Some(left), Some(right)) if left == right => Ok(()),
        (Some(left), Some(right)) => {
            domain_error(format!("{operation} crosses ID domains {left} and {right}"))
        }
        (Some(domain), None) | (None, Some(domain)) => domain_error(format!(
            "{operation} erases or omits the {domain} ID domain"
        )),
        (None, None) => Ok(()),
    }
}

fn validate_set_alignment(policy: &DomainOperationPolicy, plan: &LogicalPlan) -> Result<()> {
    let LogicalPlan::Union(union) = plan else {
        return Ok(());
    };
    let Some(authority) = union.inputs.first() else {
        return Ok(());
    };
    for input in union.inputs.iter().skip(1) {
        for (index, (left, right)) in authority
            .schema()
            .fields()
            .iter()
            .zip(input.schema().fields())
            .enumerate()
        {
            require_same_domain(
                field_domain(left.as_ref())?,
                field_domain(right.as_ref())?,
                &format!("set-operation column {index}"),
            )?;
            require_compatible_states(
                policy,
                field_state(left.as_ref())?,
                field_state(right.as_ref())?,
                &format!("set-operation column {index}"),
            )?;
        }
    }
    Ok(())
}

fn domain_error<T>(detail: impl Into<String>) -> Result<T> {
    Err(DataFusionError::Plan(format!(
        "ID_DOMAIN_MISMATCH:{}",
        detail.into()
    )))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow_array::RecordBatch;
    use arrow_schema::DataType;
    use datafusion::common::config::ConfigOptions;
    use datafusion::datasource::{MemTable, provider_as_source};
    use datafusion::logical_expr::expr_fn::cast;
    use datafusion::logical_expr::expr_fn::scalar_subquery;
    use datafusion::logical_expr::{Expr, LogicalPlan, LogicalPlanBuilder, col};
    use datafusion::optimizer::analyzer::AnalyzerRule;

    use super::{DomainConformanceRule, DomainEffect, DomainOperationPolicy, DomainStateKind};
    use crate::schema_registry::{DomainTypedLiteral, table_spec};

    fn workspace_plan() -> LogicalPlan {
        let schema = Arc::clone(&table_spec(1).expect("workspace table").arrow_schema);
        let provider = Arc::new(
            MemTable::try_new(
                Arc::clone(&schema),
                vec![vec![RecordBatch::new_empty(schema)]],
            )
            .expect("workspace probe provider"),
        );
        LogicalPlanBuilder::scan("workspace_probe", provider_as_source(provider), None)
            .expect("workspace scan")
            .build()
            .expect("workspace plan")
    }

    fn policy() -> Arc<DomainOperationPolicy> {
        let package = crate::ontology_program::build_ontology_program_package(
            &crate::ontology_program::OntologyPackagingProfile::default(),
        )
        .expect("generated ontology package");
        Arc::new(DomainOperationPolicy::from_package(&package).expect("domain policy"))
    }

    fn analyze(plan: LogicalPlan) -> datafusion::common::Result<LogicalPlan> {
        DomainConformanceRule::new(policy()).analyze(plan, &ConfigOptions::default())
    }

    #[test]
    fn odf_generated_domain_policy_total_truth_table() {
        let policy = policy();
        let states = [
            DomainStateKind::Domain,
            DomainStateKind::Neutral,
            DomainStateKind::Bottom,
            DomainStateKind::Opaque,
        ];
        let effects = [
            DomainEffect::None,
            DomainEffect::Preserve,
            DomainEffect::ConsumeSameDomain,
            DomainEffect::Produce,
            DomainEffect::ExplicitErase,
        ];
        let mut observed = 0;
        for state in states {
            for effect in effects {
                let transition = policy.transition(state, effect).expect("total transition");
                assert_eq!(
                    transition.allowed,
                    match state {
                        DomainStateKind::Opaque => false,
                        DomainStateKind::Domain =>
                            !matches!(effect, DomainEffect::Produce | DomainEffect::ExplicitErase),
                        DomainStateKind::Neutral | DomainStateKind::Bottom => true,
                    }
                );
                observed += 1;
            }
        }
        assert_eq!(observed, 20);
    }

    fn workspace_literal(value: u8) -> datafusion::logical_expr::Expr {
        DomainTypedLiteral::new("workspace", [value; 16])
            .expect("workspace domain")
            .into_expr()
    }

    fn repository_literal(value: u8) -> datafusion::logical_expr::Expr {
        DomainTypedLiteral::new("repository", [value; 16])
            .expect("repository domain")
            .into_expr()
    }

    fn domain_literal(domain: &str, value: u8) -> datafusion::logical_expr::Expr {
        DomainTypedLiteral::new(domain, [value; 16])
            .expect("registered domain")
            .into_expr()
    }

    #[test]
    fn odf_domain_conformant_plans_execute() {
        let plan = LogicalPlanBuilder::from(workspace_plan())
            .filter(col("workspace_id").eq(workspace_literal(1)))
            .expect("same-domain comparison")
            .build()
            .expect("same-domain plan");
        let once = analyze(plan).expect("same-domain plan accepted");
        let once_shape = once.display_indent_schema().to_string();
        let twice = analyze(once).expect("idempotent second analysis");
        assert_eq!(once_shape, twice.display_indent_schema().to_string());
    }

    #[test]
    fn odf_cross_domain_plan_rejection() {
        let comparison = LogicalPlanBuilder::from(workspace_plan())
            .filter(col("workspace_id").eq(col("repository_id")))
            .expect("comparison plan")
            .build()
            .expect("comparison plan build");
        let error = analyze(comparison).expect_err("cross-domain comparison must fail");
        assert!(error.to_string().contains("workspace"));
        assert!(error.to_string().contains("repository"));

        let in_list = LogicalPlanBuilder::from(workspace_plan())
            .filter(col("workspace_id").in_list(vec![repository_literal(2)], false))
            .expect("IN-list plan")
            .build()
            .expect("IN-list plan build");
        assert!(analyze(in_list).is_err());
    }

    #[test]
    fn odf_embedded_subquery_plan_is_domain_checked() {
        let subquery = LogicalPlanBuilder::from(workspace_plan())
            .filter(col("workspace_id").eq(col("repository_id")))
            .expect("cross-domain subquery filter")
            .project(vec![col("workspace_id")])
            .expect("scalar subquery projection")
            .build()
            .expect("scalar subquery plan");
        let outer = LogicalPlanBuilder::from(workspace_plan())
            .project(vec![scalar_subquery(Arc::new(subquery)).alias("nested_id")])
            .expect("outer scalar-subquery projection")
            .build()
            .expect("outer scalar-subquery plan");
        let error = analyze(outer).expect_err("nested cross-domain plan must fail");
        assert!(error.to_string().contains("workspace"));
        assert!(error.to_string().contains("repository"));

        let cross_domain_subquery = LogicalPlanBuilder::from(workspace_plan())
            .filter(col("workspace_id").eq(col("repository_id")))
            .expect("cross-domain set subquery filter")
            .project(vec![col("workspace_id")])
            .expect("set subquery projection")
            .build()
            .expect("set subquery plan");
        for nested in [
            datafusion::logical_expr::expr_fn::exists(Arc::new(cross_domain_subquery.clone())),
            datafusion::logical_expr::expr_fn::in_subquery(
                col("workspace_id"),
                Arc::new(cross_domain_subquery),
            ),
        ] {
            let outer = LogicalPlanBuilder::from(workspace_plan())
                .filter(nested)
                .expect("outer set-subquery filter")
                .build()
                .expect("outer set-subquery plan");
            let error = analyze(outer).expect_err("embedded set subquery must fail");
            assert!(error.to_string().contains("workspace"));
            assert!(error.to_string().contains("repository"));
        }

        let repository_field = Arc::new(
            crate::schema_registry::table_spec(1)
                .unwrap()
                .arrow_schema
                .field_with_name("repository_id")
                .unwrap()
                .clone(),
        );
        let correlated = LogicalPlanBuilder::from(workspace_plan())
            .alias("inner")
            .expect("correlated inner alias")
            .filter(col("inner.workspace_id").eq(Expr::OuterReferenceColumn(
                repository_field,
                datafusion::common::Column::from_qualified_name("outer.repository_id"),
            )))
            .expect("correlated cross-domain comparison")
            .project(vec![col("inner.workspace_id")])
            .expect("correlated projection")
            .build()
            .expect("correlated subquery");
        let outer = LogicalPlanBuilder::from(workspace_plan())
            .alias("outer")
            .expect("correlated outer alias")
            .filter(datafusion::logical_expr::expr_fn::exists(Arc::new(
                correlated,
            )))
            .expect("correlated exists")
            .build()
            .expect("correlated outer plan");
        let error = analyze(outer).expect_err("correlated cross-domain subquery must fail");
        assert!(error.to_string().contains("workspace"));
        assert!(error.to_string().contains("repository"));
    }

    #[test]
    fn odf_governed_type_entity_comparison_only() {
        let comparison = LogicalPlanBuilder::from(workspace_plan())
            .filter(domain_literal("type", 3).eq(domain_literal("entity", 3)))
            .expect("subdomain comparison plan")
            .build()
            .expect("subdomain comparison plan build");
        analyze(comparison).expect("type/entity ontology comparison");

        let unrelated = LogicalPlanBuilder::from(workspace_plan())
            .filter(domain_literal("type", 3).eq(repository_literal(3)))
            .expect("unrelated comparison plan")
            .build()
            .expect("unrelated comparison plan build");
        assert!(analyze(unrelated).is_err());
    }

    #[test]
    fn odf_all_plan_ingresses_domain_checked() {
        let cast_plan = LogicalPlanBuilder::from(workspace_plan())
            .project(vec![cast(
                col("workspace_id"),
                DataType::FixedSizeBinary(16),
            )])
            .expect("cast plan")
            .build()
            .expect("cast plan build");
        assert!(analyze(cast_plan).is_err());

        let workspace = LogicalPlanBuilder::from(workspace_plan())
            .project(vec![col("workspace_id").alias("id")])
            .expect("workspace projection")
            .build()
            .expect("workspace projection build");
        let repository = LogicalPlanBuilder::from(workspace_plan())
            .project(vec![col("repository_id").alias("id")])
            .expect("repository projection")
            .build()
            .expect("repository projection build");
        let union = LogicalPlanBuilder::from(workspace)
            .union(repository)
            .expect("union plan")
            .build()
            .expect("union plan build");
        assert!(analyze(union).is_err());
    }
}
