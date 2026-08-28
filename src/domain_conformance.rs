//! Application-owned logical-plan ID-domain conformance.

use arrow_schema::Field;
use datafusion::common::config::ConfigOptions;
use datafusion::common::tree_node::{TreeNode, TreeNodeRecursion};
use datafusion::common::{DataFusionError, ExprSchema, Result};
use datafusion::logical_expr::{Expr, LogicalPlan, Operator};
use datafusion::optimizer::analyzer::AnalyzerRule;

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

fn validate_expression(plan: &LogicalPlan, expression: &Expr) -> Result<()> {
    match expression {
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
        _ => {}
    }
    Ok(())
}

fn expression_domain<'a>(plan: &'a LogicalPlan, expression: &'a Expr) -> Result<Option<&'a str>> {
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
        _ => Ok(None),
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
