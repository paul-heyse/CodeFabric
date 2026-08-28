//! Application-owned logical-plan ID-domain conformance.

use arrow_schema::Field;
use datafusion::common::config::ConfigOptions;
use datafusion::common::tree_node::{TreeNode, TreeNodeRecursion};
use datafusion::common::{DataFusionError, ExprSchema, Result};
use datafusion::logical_expr::{Expr, LogicalPlan, Operator};
use datafusion::optimizer::analyzer::AnalyzerRule;

/// Generated-state projection used by the total analyzer transition lattice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainState {
    Untracked,
    Exact(String),
    Predicate,
}

/// Effect class assigned to every pinned expression form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainEffect {
    Preserve,
    Compare,
    Predicate,
    Aggregate,
    Window,
    Subquery,
    Set,
    Cast,
    Function,
    RejectUnresolved,
}

/// Compile-time census for the pinned DataFusion 55 expression enum.
pub const DATAFUSION_EXPR_VARIANT_CENSUS: &[&str] = &[
    "Alias",
    "Column",
    "ScalarVariable",
    "Literal",
    "BinaryExpr",
    "Like",
    "SimilarTo",
    "Not",
    "IsNotNull",
    "IsNull",
    "IsTrue",
    "IsFalse",
    "IsUnknown",
    "IsNotTrue",
    "IsNotFalse",
    "IsNotUnknown",
    "Negative",
    "Between",
    "Case",
    "Cast",
    "TryCast",
    "ScalarFunction",
    "AggregateFunction",
    "WindowFunction",
    "InList",
    "Exists",
    "InSubquery",
    "SetComparison",
    "ScalarSubquery",
    "Wildcard",
    "GroupingSet",
    "Placeholder",
    "OuterReferenceColumn",
    "Unnest",
    "HigherOrderFunction",
    "Lambda",
    "LambdaVariable",
];

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
#[derive(Debug, Default)]
pub struct DomainConformanceRule;

impl DomainConformanceRule {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl AnalyzerRule for DomainConformanceRule {
    fn analyze(&self, plan: LogicalPlan, _config: &ConfigOptions) -> Result<LogicalPlan> {
        plan.apply(|node| {
            validate_plan_variant(node)?;
            validate_set_alignment(node)?;
            for expression in node.expressions() {
                expression.apply(|nested| {
                    validate_expression(node, nested)?;
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
pub(crate) fn analyze_governed_plan(plan: LogicalPlan) -> Result<LogicalPlan> {
    DomainConformanceRule::new().analyze(plan, &ConfigOptions::default())
}

#[allow(clippy::too_many_lines)] // Exhaustive DataFusion Expr coverage is intentionally centralized.
fn validate_expression(plan: &LogicalPlan, expression: &Expr) -> Result<()> {
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
            let left = expression_domain(plan, &binary.left)?;
            let right = expression_domain(plan, &binary.right)?;
            if left.is_some() || right.is_some() {
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
                require_same_domain(left, right, "binary comparison")?;
            }
        }
        Expr::InList(in_list) => {
            let value_domain = expression_domain(plan, &in_list.expr)?;
            if value_domain.is_some() {
                for candidate in &in_list.list {
                    require_same_domain(
                        value_domain,
                        expression_domain(plan, candidate)?,
                        "IN-list member",
                    )?;
                }
            }
        }
        Expr::Cast(cast) => {
            if let Some(domain) = expression_domain(plan, &cast.expr)? {
                return domain_error(format!("explicit cast erases logical ID domain {domain}"));
            }
        }
        Expr::TryCast(cast) => {
            if let Some(domain) = expression_domain(plan, &cast.expr)? {
                return domain_error(format!("try-cast erases logical ID domain {domain}"));
            }
        }
        Expr::Like(like) | Expr::SimilarTo(like) => {
            reject_domain_arguments(
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
            let _ = expression_domain(plan, value)?;
        }
        Expr::Negative(value) => {
            reject_domain_arguments(plan, [value.as_ref()], "numeric negation")?;
        }
        Expr::Between(between) => {
            let domain = expression_domain(plan, &between.expr)?;
            require_same_domain(
                domain,
                expression_domain(plan, &between.low)?,
                "BETWEEN low",
            )?;
            require_same_domain(
                domain,
                expression_domain(plan, &between.high)?,
                "BETWEEN high",
            )?;
        }
        Expr::Case(case) => {
            let mut values = Vec::new();
            if let Some(value) = &case.expr {
                values.push(value.as_ref());
            }
            for (when, then) in &case.when_then_expr {
                values.push(when.as_ref());
                values.push(then.as_ref());
            }
            if let Some(value) = &case.else_expr {
                values.push(value.as_ref());
            }
            reject_domain_arguments(plan, values, "CASE")?;
        }
        Expr::ScalarFunction(function) => {
            reject_domain_arguments(plan, function.args.iter(), "scalar function")?;
        }
        Expr::AggregateFunction(function) => {
            let has_domain_argument = function
                .params
                .args
                .iter()
                .map(|argument| expression_domain(plan, argument))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .any(|domain| domain.is_some());
            if has_domain_argument && function.func.name() != "count" {
                return domain_error(format!(
                    "aggregate function {} has no generated ID-domain transition",
                    function.func.name()
                ));
            }
            if let Some(filter) = &function.params.filter {
                let _ = expression_domain(plan, filter)?;
            }
            reject_domain_arguments(
                plan,
                function.params.order_by.iter().map(|sort| &sort.expr),
                "aggregate ordering",
            )?;
        }
        Expr::WindowFunction(function) => {
            reject_domain_arguments(plan, function.params.args.iter(), "window function")?;
            reject_domain_arguments(
                plan,
                function.params.partition_by.iter(),
                "window partition",
            )?;
            reject_domain_arguments(
                plan,
                function.params.order_by.iter().map(|sort| &sort.expr),
                "window ordering",
            )?;
        }
        Expr::InSubquery(subquery) => {
            if expression_domain(plan, &subquery.expr)?.is_some() {
                return domain_error(
                    "ID-domain IN-subquery requires an explicit generated binding",
                );
            }
        }
        Expr::SetComparison(comparison) => {
            if expression_domain(plan, &comparison.expr)?.is_some() {
                return domain_error(
                    "ID-domain set comparison requires an explicit generated binding",
                );
            }
        }
        Expr::Wildcard { .. } => {
            return domain_error("unresolved wildcard reached the application analyzer");
        }
        Expr::GroupingSet(grouping) => {
            reject_domain_arguments(plan, grouping.distinct_expr(), "grouping set")?;
        }
        Expr::Placeholder(_) => {
            return domain_error("unresolved prepared-statement placeholder reached analyzer");
        }
        Expr::Unnest(unnest) => {
            reject_domain_arguments(plan, [unnest.expr.as_ref()], "unnest")?;
        }
        Expr::HigherOrderFunction(function) => {
            reject_domain_arguments(plan, function.args.iter(), "higher-order function")?;
        }
        Expr::Lambda(lambda) => {
            reject_domain_arguments(plan, [lambda.body.as_ref()], "lambda")?;
        }
    }
    Ok(())
}

fn reject_domain_arguments<'a>(
    plan: &'a LogicalPlan,
    expressions: impl IntoIterator<Item = &'a Expr>,
    operation: &str,
) -> Result<()> {
    for expression in expressions {
        if let Some(domain) = expression_domain(plan, expression)? {
            return domain_error(format!(
                "{operation} has no generated transition for ID domain {domain}"
            ));
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

fn validate_set_alignment(plan: &LogicalPlan) -> Result<()> {
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
    use datafusion::logical_expr::{LogicalPlan, LogicalPlanBuilder, col};
    use datafusion::optimizer::analyzer::AnalyzerRule;

    use super::DomainConformanceRule;
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

    fn analyze(plan: LogicalPlan) -> datafusion::common::Result<LogicalPlan> {
        DomainConformanceRule::new().analyze(plan, &ConfigOptions::default())
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
