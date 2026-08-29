//! Typed normalized ontology-program graph and generic DataFusion lowering.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Not as _;
use std::sync::Arc;

use arrow_array::{Array as _, BooleanArray, RecordBatch, StringArray, UInt16Array};
use arrow_schema::DataType;
use datafusion::catalog::TableProvider;
use datafusion::common::{Column, TableReference};
use datafusion::datasource::{MemTable, provider_as_source};
use datafusion::functions_aggregate::expr_fn::count;
use datafusion::logical_expr::expr_fn::cast;
use datafusion::logical_expr::{Expr, JoinType, LogicalPlan, LogicalPlanBuilder, lit};
use datafusion::scalar::ScalarValue;

use crate::ontology_executor::OntologyProgramCompileError;
use crate::ontology_program::OntologyProgramPackage;

/// One executable program root and its acceptance contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramContract {
    pub program_id: String,
    pub rule_id: String,
    pub root_node_id: String,
    pub execution_phase: String,
    pub calculation_id: String,
    pub policy_id: String,
    pub expected_result_contract: String,
    pub diagnostic_code: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScanNode {
    relation_ref: String,
    relation_alias: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PlanNode {
    Scan(ScanNode),
    Filter {
        predicate_expr_id: String,
    },
    Project,
    Join {
        join_type: String,
        condition_expr_id: String,
    },
    Aggregate,
    Set {
        set_operation: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExpressionNode {
    Column {
        relation_alias: String,
        column_name: String,
    },
    Literal {
        logical_type: String,
        value: Option<String>,
        is_null: bool,
    },
    Binary {
        operator: String,
    },
    Call {
        function_name: String,
    },
    Case,
    Cast {
        target_type: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PlanEdge {
    child_node_id: String,
    input_ordinal: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExpressionEdge {
    child_expr_id: String,
    role: String,
    operand_ordinal: u16,
    output_alias: Option<String>,
}

/// Complete, validated normalized program graph.
#[derive(Clone, Debug)]
pub struct OntologyRelationalProgram {
    programs: BTreeMap<String, ProgramContract>,
    nodes: BTreeMap<String, PlanNode>,
    expressions: BTreeMap<String, ExpressionNode>,
    plan_edges: BTreeMap<String, Vec<PlanEdge>>,
    expression_edges: BTreeMap<String, Vec<ExpressionEdge>>,
}

fn one_batch<'a>(
    package: &'a OntologyProgramPackage,
    relation: &str,
) -> Result<&'a RecordBatch, OntologyProgramCompileError> {
    let member = package
        .members
        .get(relation)
        .ok_or_else(|| OntologyProgramCompileError::Decode(format!("missing {relation}")))?;
    if member.batches.len() != 1 {
        return Err(OntologyProgramCompileError::Decode(format!(
            "{relation} must have one canonical batch"
        )));
    }
    Ok(&member.batches[0])
}

fn utf8<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a StringArray, OntologyProgramCompileError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .ok_or_else(|| OntologyProgramCompileError::Decode(format!("{name} is not Utf8")))
}

fn uint16<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a UInt16Array, OntologyProgramCompileError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<UInt16Array>())
        .ok_or_else(|| OntologyProgramCompileError::Decode(format!("{name} is not UInt16")))
}

fn insert_unique<T>(
    values: &mut BTreeMap<String, T>,
    id: String,
    value: T,
    family: &str,
) -> Result<(), OntologyProgramCompileError> {
    if id.is_empty() || values.insert(id.clone(), value).is_some() {
        return Err(OntologyProgramCompileError::Decode(format!(
            "duplicate or empty {family} id {id:?}"
        )));
    }
    Ok(())
}

impl OntologyRelationalProgram {
    /// Decode and structurally validate all typed plan and expression relations.
    pub fn decode(package: &OntologyProgramPackage) -> Result<Self, OntologyProgramCompileError> {
        let contract = one_batch(package, "program.program_contract")?;
        let program_ids = utf8(contract, "program_id")?;
        let rule_ids = utf8(contract, "rule_id")?;
        let roots = utf8(contract, "root_node_id")?;
        let phases = utf8(contract, "execution_phase")?;
        let calculations = utf8(contract, "calculation_id")?;
        let policies = utf8(contract, "policy_id")?;
        let expected = utf8(contract, "expected_result_contract")?;
        let diagnostics = utf8(contract, "diagnostic_code")?;
        let mut programs = BTreeMap::new();
        for row in 0..contract.num_rows() {
            let program = ProgramContract {
                program_id: program_ids.value(row).to_owned(),
                rule_id: rule_ids.value(row).to_owned(),
                root_node_id: roots.value(row).to_owned(),
                execution_phase: phases.value(row).to_owned(),
                calculation_id: calculations.value(row).to_owned(),
                policy_id: policies.value(row).to_owned(),
                expected_result_contract: expected.value(row).to_owned(),
                diagnostic_code: diagnostics.value(row).to_owned(),
            };
            if program.rule_id.is_empty()
                || program.calculation_id.is_empty()
                || program.policy_id.is_empty()
                || program.expected_result_contract.is_empty()
                || program.diagnostic_code.is_empty()
                || (program.execution_phase != "semantic_analysis"
                    && program.root_node_id.is_empty())
            {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "{} has an incomplete program contract",
                    program.program_id
                )));
            }
            insert_unique(
                &mut programs,
                program.program_id.clone(),
                program,
                "program",
            )?;
        }

        let mut nodes = BTreeMap::new();
        let scans = one_batch(package, "program.scan_node")?;
        let ids = utf8(scans, "node_id")?;
        let relations = utf8(scans, "relation_ref")?;
        let aliases = utf8(scans, "relation_alias")?;
        for row in 0..scans.num_rows() {
            insert_unique(
                &mut nodes,
                ids.value(row).to_owned(),
                PlanNode::Scan(ScanNode {
                    relation_ref: relations.value(row).to_owned(),
                    relation_alias: aliases.value(row).to_owned(),
                }),
                "plan node",
            )?;
        }
        let filters = one_batch(package, "program.filter_node")?;
        let ids = utf8(filters, "node_id")?;
        let predicates = utf8(filters, "predicate_expr_id")?;
        for row in 0..filters.num_rows() {
            insert_unique(
                &mut nodes,
                ids.value(row).to_owned(),
                PlanNode::Filter {
                    predicate_expr_id: predicates.value(row).to_owned(),
                },
                "plan node",
            )?;
        }
        for (relation, node) in [
            ("program.project_node", PlanNode::Project),
            ("program.aggregate_node", PlanNode::Aggregate),
        ] {
            let batch = one_batch(package, relation)?;
            let ids = utf8(batch, "node_id")?;
            for row in 0..batch.num_rows() {
                insert_unique(
                    &mut nodes,
                    ids.value(row).to_owned(),
                    node.clone(),
                    "plan node",
                )?;
            }
        }
        let joins = one_batch(package, "program.join_node")?;
        let ids = utf8(joins, "node_id")?;
        let types = utf8(joins, "join_type")?;
        let conditions = utf8(joins, "condition_expr_id")?;
        for row in 0..joins.num_rows() {
            insert_unique(
                &mut nodes,
                ids.value(row).to_owned(),
                PlanNode::Join {
                    join_type: types.value(row).to_owned(),
                    condition_expr_id: conditions.value(row).to_owned(),
                },
                "plan node",
            )?;
        }
        let sets = one_batch(package, "program.set_node")?;
        let ids = utf8(sets, "node_id")?;
        let operations = utf8(sets, "set_operation")?;
        for row in 0..sets.num_rows() {
            insert_unique(
                &mut nodes,
                ids.value(row).to_owned(),
                PlanNode::Set {
                    set_operation: operations.value(row).to_owned(),
                },
                "plan node",
            )?;
        }

        let mut expressions = BTreeMap::new();
        let columns = one_batch(package, "program.column_expr")?;
        let ids = utf8(columns, "expr_id")?;
        let aliases = utf8(columns, "relation_alias")?;
        let names = utf8(columns, "column_name")?;
        for row in 0..columns.num_rows() {
            insert_unique(
                &mut expressions,
                ids.value(row).to_owned(),
                ExpressionNode::Column {
                    relation_alias: aliases.value(row).to_owned(),
                    column_name: names.value(row).to_owned(),
                },
                "expression",
            )?;
        }
        let literals = one_batch(package, "program.literal_expr")?;
        let ids = utf8(literals, "expr_id")?;
        let types = utf8(literals, "logical_type")?;
        let values = utf8(literals, "value")?;
        let nulls = literals
            .column_by_name("is_null")
            .and_then(|column| column.as_any().downcast_ref::<BooleanArray>())
            .ok_or_else(|| OntologyProgramCompileError::Decode("is_null is not Boolean".into()))?;
        for row in 0..literals.num_rows() {
            let is_null = nulls.value(row);
            if is_null != values.is_null(row) {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "literal {} has inconsistent null encoding",
                    ids.value(row)
                )));
            }
            insert_unique(
                &mut expressions,
                ids.value(row).to_owned(),
                ExpressionNode::Literal {
                    logical_type: types.value(row).to_owned(),
                    value: (!is_null).then(|| values.value(row).to_owned()),
                    is_null,
                },
                "expression",
            )?;
        }
        for (relation, value_column, constructor) in [
            (
                "program.binary_expr",
                "operator",
                ExpressionNode::Binary {
                    operator: String::new(),
                },
            ),
            (
                "program.call_expr",
                "function_name",
                ExpressionNode::Call {
                    function_name: String::new(),
                },
            ),
            (
                "program.cast_expr",
                "target_type",
                ExpressionNode::Cast {
                    target_type: String::new(),
                },
            ),
        ] {
            let batch = one_batch(package, relation)?;
            let ids = utf8(batch, "expr_id")?;
            let values = utf8(batch, value_column)?;
            for row in 0..batch.num_rows() {
                let value = values.value(row).to_owned();
                let expression = match &constructor {
                    ExpressionNode::Binary { .. } => ExpressionNode::Binary { operator: value },
                    ExpressionNode::Call { .. } => ExpressionNode::Call {
                        function_name: value,
                    },
                    ExpressionNode::Cast { .. } => ExpressionNode::Cast { target_type: value },
                    _ => unreachable!("closed constructor census"),
                };
                insert_unique(
                    &mut expressions,
                    ids.value(row).to_owned(),
                    expression,
                    "expression",
                )?;
            }
        }
        let cases = one_batch(package, "program.case_expr")?;
        let ids = utf8(cases, "expr_id")?;
        for row in 0..cases.num_rows() {
            insert_unique(
                &mut expressions,
                ids.value(row).to_owned(),
                ExpressionNode::Case,
                "expression",
            )?;
        }

        let plan_edge_batch = one_batch(package, "program.plan_edge")?;
        let parents = utf8(plan_edge_batch, "parent_node_id")?;
        let children = utf8(plan_edge_batch, "child_node_id")?;
        let ordinals = uint16(plan_edge_batch, "input_ordinal")?;
        let mut plan_edges: BTreeMap<String, Vec<PlanEdge>> = BTreeMap::new();
        for row in 0..plan_edge_batch.num_rows() {
            let edges = plan_edges.entry(parents.value(row).to_owned()).or_default();
            if usize::from(ordinals.value(row)) != edges.len() {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "{} has non-contiguous plan edges",
                    parents.value(row)
                )));
            }
            edges.push(PlanEdge {
                child_node_id: children.value(row).to_owned(),
                input_ordinal: ordinals.value(row),
            });
        }
        let expression_edge_batch = one_batch(package, "program.expression_edge")?;
        let parents = utf8(expression_edge_batch, "parent_id")?;
        let children = utf8(expression_edge_batch, "child_expr_id")?;
        let roles = utf8(expression_edge_batch, "role")?;
        let ordinals = uint16(expression_edge_batch, "operand_ordinal")?;
        let aliases = utf8(expression_edge_batch, "output_alias")?;
        let mut expression_edges: BTreeMap<String, Vec<ExpressionEdge>> = BTreeMap::new();
        for row in 0..expression_edge_batch.num_rows() {
            let edges = expression_edges
                .entry(parents.value(row).to_owned())
                .or_default();
            let role = roles.value(row);
            let expected_ordinal = edges.iter().filter(|edge| edge.role == role).count();
            if usize::from(ordinals.value(row)) != expected_ordinal {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "{}:{role} has non-contiguous expression edges",
                    parents.value(row)
                )));
            }
            edges.push(ExpressionEdge {
                child_expr_id: children.value(row).to_owned(),
                role: role.to_owned(),
                operand_ordinal: ordinals.value(row),
                output_alias: (!aliases.is_null(row)).then(|| aliases.value(row).to_owned()),
            });
        }

        let graph = Self {
            programs,
            nodes,
            expressions,
            plan_edges,
            expression_edges,
        };
        graph.validate_graph()?;
        Ok(graph)
    }

    fn validate_graph(&self) -> Result<(), OntologyProgramCompileError> {
        for (parent, edges) in &self.plan_edges {
            if !self.nodes.contains_key(parent)
                || edges
                    .iter()
                    .any(|edge| !self.nodes.contains_key(&edge.child_node_id))
            {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "dangling plan edge below {parent}"
                )));
            }
        }
        for (parent, edges) in &self.expression_edges {
            if !self.nodes.contains_key(parent) && !self.expressions.contains_key(parent) {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "expression edge has unknown parent {parent}"
                )));
            }
            if edges
                .iter()
                .any(|edge| !self.expressions.contains_key(&edge.child_expr_id))
            {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "dangling expression below {parent}"
                )));
            }
        }
        for program in self.programs.values() {
            if program.execution_phase != "semantic_analysis"
                && !self.nodes.contains_key(&program.root_node_id)
            {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "{} has unknown root {}",
                    program.program_id, program.root_node_id
                )));
            }
        }
        let mut visited = BTreeSet::new();
        let mut active = BTreeSet::new();
        for program in self
            .programs
            .values()
            .filter(|program| !program.root_node_id.is_empty())
        {
            self.validate_plan_acyclic(&program.root_node_id, &mut visited, &mut active)?;
        }
        Ok(())
    }

    fn validate_plan_acyclic(
        &self,
        node_id: &str,
        visited: &mut BTreeSet<String>,
        active: &mut BTreeSet<String>,
    ) -> Result<(), OntologyProgramCompileError> {
        if visited.contains(node_id) {
            return Ok(());
        }
        if !active.insert(node_id.to_owned()) {
            return Err(OntologyProgramCompileError::Decode(format!(
                "cycle at plan node {node_id}"
            )));
        }
        if let Some(edges) = self.plan_edges.get(node_id) {
            for edge in edges {
                self.validate_plan_acyclic(&edge.child_node_id, visited, active)?;
            }
        }
        active.remove(node_id);
        visited.insert(node_id.to_owned());
        Ok(())
    }

    #[must_use]
    pub fn programs(&self) -> &BTreeMap<String, ProgramContract> {
        &self.programs
    }

    fn ordered_plan_children(&self, node_id: &str) -> Vec<&PlanEdge> {
        let mut edges = self
            .plan_edges
            .get(node_id)
            .map_or_else(Vec::new, |edges| edges.iter().collect());
        edges.sort_by_key(|edge| edge.input_ordinal);
        edges
    }

    fn expression_children(&self, parent_id: &str, role: &str) -> Vec<&ExpressionEdge> {
        let mut edges = self
            .expression_edges
            .get(parent_id)
            .map_or_else(Vec::new, |edges| {
                edges.iter().filter(|edge| edge.role == role).collect()
            });
        edges.sort_by_key(|edge| edge.operand_ordinal);
        edges
    }

    fn compile_expression(&self, expr_id: &str) -> Result<Expr, OntologyProgramCompileError> {
        let expression = self.expressions.get(expr_id).ok_or_else(|| {
            OntologyProgramCompileError::Decode(format!("unknown expression {expr_id}"))
        })?;
        match expression {
            ExpressionNode::Column {
                relation_alias,
                column_name,
            } => Ok(Expr::Column(if relation_alias.is_empty() {
                Column::new_unqualified(column_name)
            } else {
                Column::new(
                    Some(TableReference::bare(relation_alias.clone())),
                    column_name,
                )
            })),
            ExpressionNode::Literal {
                logical_type,
                value,
                is_null,
            } => {
                if *is_null {
                    return Ok(lit(ScalarValue::Null));
                }
                let value = value.as_deref().ok_or_else(|| {
                    OntologyProgramCompileError::Decode(format!(
                        "non-null literal {expr_id} lacks value"
                    ))
                })?;
                match logical_type.as_str() {
                    "utf8" => Ok(lit(value.to_owned())),
                    "boolean" => value
                        .parse::<bool>()
                        .map(lit)
                        .map_err(|error| OntologyProgramCompileError::Decode(error.to_string())),
                    "int16" => value
                        .parse::<i16>()
                        .map(lit)
                        .map_err(|error| OntologyProgramCompileError::Decode(error.to_string())),
                    "int32" => value
                        .parse::<i32>()
                        .map(lit)
                        .map_err(|error| OntologyProgramCompileError::Decode(error.to_string())),
                    "int64" => value
                        .parse::<i64>()
                        .map(lit)
                        .map_err(|error| OntologyProgramCompileError::Decode(error.to_string())),
                    logical_type => Err(OntologyProgramCompileError::Unsupported(format!(
                        "literal type {logical_type}"
                    ))),
                }
            }
            ExpressionNode::Binary { operator } => {
                let children = self.expression_children(expr_id, "left");
                let right = self.expression_children(expr_id, "right");
                if children.len() != 1 || right.len() != 1 {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "binary expression {expr_id} does not have left/right operands"
                    )));
                }
                let left = self.compile_expression(&children[0].child_expr_id)?;
                let right = self.compile_expression(&right[0].child_expr_id)?;
                match operator.as_str() {
                    "eq" => Ok(left.eq(right)),
                    "neq" => Ok(left.not_eq(right)),
                    "gt" => Ok(left.gt(right)),
                    "gte" => Ok(left.gt_eq(right)),
                    "lt" => Ok(left.lt(right)),
                    "lte" => Ok(left.lt_eq(right)),
                    "and" => Ok(left.and(right)),
                    "or" => Ok(left.or(right)),
                    operator => Err(OntologyProgramCompileError::Unsupported(format!(
                        "binary operator {operator}"
                    ))),
                }
            }
            ExpressionNode::Call { function_name } => {
                let arguments = self
                    .expression_children(expr_id, "argument")
                    .into_iter()
                    .map(|edge| self.compile_expression(&edge.child_expr_id))
                    .collect::<Result<Vec<_>, _>>()?;
                match (function_name.as_str(), arguments.as_slice()) {
                    ("is_null", [argument]) => Ok(argument.clone().is_null()),
                    ("is_not_null", [argument]) => Ok(argument.clone().is_not_null()),
                    ("not", [argument]) => Ok(argument.clone().not()),
                    ("is_true", [argument]) => Ok(argument.clone().is_true()),
                    ("count", [argument]) => Ok(count(argument.clone())),
                    (function, _) => Err(OntologyProgramCompileError::Unsupported(format!(
                        "built-in call {function}/{}",
                        arguments.len()
                    ))),
                }
            }
            ExpressionNode::Cast { target_type } => {
                let arguments = self.expression_children(expr_id, "argument");
                if arguments.len() != 1 {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "cast {expr_id} does not have one argument"
                    )));
                }
                let expression = self.compile_expression(&arguments[0].child_expr_id)?;
                let data_type = match target_type.as_str() {
                    "int16" => DataType::Int16,
                    "int32" => DataType::Int32,
                    "int64" => DataType::Int64,
                    "utf8" => DataType::Utf8,
                    target => {
                        return Err(OntologyProgramCompileError::Unsupported(format!(
                            "cast target {target}"
                        )));
                    }
                };
                Ok(cast(expression, data_type))
            }
            ExpressionNode::Case => Err(OntologyProgramCompileError::Unsupported(
                "case expression lacks a released current-profile row".into(),
            )),
        }
    }

    fn compile_node(
        &self,
        node_id: &str,
        providers: &BTreeMap<String, Arc<dyn TableProvider>>,
    ) -> Result<LogicalPlan, OntologyProgramCompileError> {
        let node = self.nodes.get(node_id).ok_or_else(|| {
            OntologyProgramCompileError::Decode(format!("unknown plan node {node_id}"))
        })?;
        let children = self.ordered_plan_children(node_id);
        match node {
            PlanNode::Scan(scan) => {
                if !children.is_empty() || scan.relation_alias.is_empty() {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "scan {node_id} has child edges or empty alias"
                    )));
                }
                let provider = providers.get(&scan.relation_ref).ok_or_else(|| {
                    OntologyProgramCompileError::Unsupported(format!(
                        "unbound relation {}",
                        scan.relation_ref
                    ))
                })?;
                LogicalPlanBuilder::scan(
                    scan.relation_alias.clone(),
                    provider_as_source(Arc::clone(provider)),
                    None,
                )
                .and_then(LogicalPlanBuilder::build)
                .map_err(|error| OntologyProgramCompileError::Decode(error.to_string()))
            }
            PlanNode::Filter { predicate_expr_id } => {
                if children.len() != 1 {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "filter {node_id} does not have one input"
                    )));
                }
                let input = self.compile_node(&children[0].child_node_id, providers)?;
                LogicalPlanBuilder::from(input)
                    .filter(self.compile_expression(predicate_expr_id)?)
                    .and_then(LogicalPlanBuilder::build)
                    .map_err(|error| OntologyProgramCompileError::Decode(error.to_string()))
            }
            PlanNode::Project => {
                if children.len() != 1 {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "project {node_id} does not have one input"
                    )));
                }
                let input = self.compile_node(&children[0].child_node_id, providers)?;
                let projections = self
                    .expression_children(node_id, "projection")
                    .into_iter()
                    .map(|edge| {
                        let expression = self.compile_expression(&edge.child_expr_id)?;
                        edge.output_alias
                            .as_ref()
                            .map_or(Ok(expression.clone()), |alias| Ok(expression.alias(alias)))
                    })
                    .collect::<Result<Vec<_>, OntologyProgramCompileError>>()?;
                if projections.is_empty() {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "project {node_id} has no expressions"
                    )));
                }
                LogicalPlanBuilder::from(input)
                    .project(projections)
                    .and_then(LogicalPlanBuilder::build)
                    .map_err(|error| OntologyProgramCompileError::Decode(error.to_string()))
            }
            PlanNode::Join {
                join_type,
                condition_expr_id,
            } => {
                if children.len() != 2 {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "join {node_id} does not have two inputs"
                    )));
                }
                let left = self.compile_node(&children[0].child_node_id, providers)?;
                let right = self.compile_node(&children[1].child_node_id, providers)?;
                let join_type = match join_type.as_str() {
                    "inner" => JoinType::Inner,
                    "left" => JoinType::Left,
                    "left_semi" => JoinType::LeftSemi,
                    "left_anti" => JoinType::LeftAnti,
                    kind => {
                        return Err(OntologyProgramCompileError::Unsupported(format!(
                            "join type {kind}"
                        )));
                    }
                };
                LogicalPlanBuilder::from(left)
                    .join_on(
                        right,
                        join_type,
                        [self.compile_expression(condition_expr_id)?],
                    )
                    .and_then(LogicalPlanBuilder::build)
                    .map_err(|error| OntologyProgramCompileError::Decode(error.to_string()))
            }
            PlanNode::Aggregate => {
                if children.len() != 1 {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "aggregate {node_id} does not have one input"
                    )));
                }
                let input = self.compile_node(&children[0].child_node_id, providers)?;
                let groups = self
                    .expression_children(node_id, "group")
                    .into_iter()
                    .map(|edge| {
                        let expression = self.compile_expression(&edge.child_expr_id)?;
                        Ok(edge
                            .output_alias
                            .as_ref()
                            .map_or(expression.clone(), |alias| expression.alias(alias)))
                    })
                    .collect::<Result<Vec<_>, OntologyProgramCompileError>>()?;
                let aggregates = self
                    .expression_children(node_id, "aggregate")
                    .into_iter()
                    .map(|edge| {
                        let expression = self.compile_expression(&edge.child_expr_id)?;
                        Ok(edge
                            .output_alias
                            .as_ref()
                            .map_or(expression.clone(), |alias| expression.alias(alias)))
                    })
                    .collect::<Result<Vec<_>, OntologyProgramCompileError>>()?;
                if aggregates.is_empty() {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "aggregate {node_id} has no aggregate expressions"
                    )));
                }
                LogicalPlanBuilder::from(input)
                    .aggregate(groups, aggregates)
                    .and_then(LogicalPlanBuilder::build)
                    .map_err(|error| OntologyProgramCompileError::Decode(error.to_string()))
            }
            PlanNode::Set { set_operation } => {
                if children.len() < 2 || set_operation != "union_all" {
                    return Err(OntologyProgramCompileError::Unsupported(format!(
                        "set node {node_id}:{set_operation}/{}",
                        children.len()
                    )));
                }
                let mut children = children.into_iter();
                let first = children.next().expect("two children validated");
                let mut builder =
                    LogicalPlanBuilder::from(self.compile_node(&first.child_node_id, providers)?);
                for child in children {
                    builder = builder
                        .union(self.compile_node(&child.child_node_id, providers)?)
                        .map_err(|error| OntologyProgramCompileError::Decode(error.to_string()))?;
                }
                builder
                    .build()
                    .map_err(|error| OntologyProgramCompileError::Decode(error.to_string()))
            }
        }
    }

    /// Compile one named program solely from normalized graph rows and bound providers.
    pub fn compile(
        &self,
        program_id: &str,
        providers: &BTreeMap<String, Arc<dyn TableProvider>>,
    ) -> Result<LogicalPlan, OntologyProgramCompileError> {
        let program = self.programs.get(program_id).ok_or_else(|| {
            OntologyProgramCompileError::Unsupported(format!("unknown program {program_id}"))
        })?;
        if program.execution_phase == "semantic_analysis" {
            return Err(OntologyProgramCompileError::Unsupported(format!(
                "{} is enforced by the semantic analyzer",
                program.program_id
            )));
        }
        self.compile_node(&program.root_node_id, providers)
    }
}

/// Bind candidate Arrow batches behind ordinary DataFusion `TableProvider`s.
pub fn candidate_batch_providers(
    batches: &BTreeMap<i16, RecordBatch>,
) -> Result<BTreeMap<String, Arc<dyn TableProvider>>, OntologyProgramCompileError> {
    batches
        .iter()
        .map(|(table_code, batch)| {
            let provider: Arc<dyn TableProvider> = Arc::new(
                MemTable::try_new(batch.schema(), vec![vec![batch.clone()]])
                    .map_err(|error| OntologyProgramCompileError::Decode(error.to_string()))?,
            );
            Ok((format!("table:{table_code}"), provider))
        })
        .collect()
}
