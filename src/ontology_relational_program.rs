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

const MAX_PROGRAM_NODES: usize = 65_536;
const MAX_PROGRAM_EXPRESSIONS: usize = 262_144;
const MAX_PROGRAM_GRAPH_DEPTH: usize = 256;

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
    pub rule_semantics_identity: String,
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
    id: &str,
    value: T,
    family: &str,
) -> Result<(), OntologyProgramCompileError> {
    if id.is_empty() || values.insert(id.to_owned(), value).is_some() {
        return Err(OntologyProgramCompileError::Decode(format!(
            "duplicate or empty {family} id {id:?}"
        )));
    }
    Ok(())
}

impl OntologyRelationalProgram {
    /// Decode and structurally validate all typed plan and expression relations.
    ///
    /// # Errors
    ///
    /// Rejects missing, duplicate, malformed, or cross-relation-inconsistent program rows.
    #[allow(clippy::too_many_lines)] // Complete normalized relation decoding stays one closed census.
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
        let rule_semantics_identities = utf8(contract, "rule_semantics_identity")?;
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
                rule_semantics_identity: rule_semantics_identities.value(row).to_owned(),
            };
            if program.rule_id.is_empty()
                || program.calculation_id.is_empty()
                || program.policy_id.is_empty()
                || program.expected_result_contract.is_empty()
                || program.diagnostic_code.is_empty()
                || program.rule_semantics_identity.is_empty()
            {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "{} has an incomplete program contract",
                    program.program_id
                )));
            }
            match program.execution_phase.as_str() {
                "candidate_validation" if !program.root_node_id.is_empty() => {}
                "semantic_analysis" if program.root_node_id.is_empty() => {}
                "candidate_validation" | "semantic_analysis" => {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "{} has a root incompatible with execution phase {}",
                        program.program_id, program.execution_phase
                    )));
                }
                phase => {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "{} has unknown execution phase {phase}",
                        program.program_id
                    )));
                }
            }
            let program_id = program.program_id.clone();
            insert_unique(&mut programs, &program_id, program, "program")?;
        }

        let mut nodes = BTreeMap::new();
        let scans = one_batch(package, "program.scan_node")?;
        let ids = utf8(scans, "node_id")?;
        let relations = utf8(scans, "relation_ref")?;
        let aliases = utf8(scans, "relation_alias")?;
        for row in 0..scans.num_rows() {
            insert_unique(
                &mut nodes,
                ids.value(row),
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
                ids.value(row),
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
                insert_unique(&mut nodes, ids.value(row), node.clone(), "plan node")?;
            }
        }
        let joins = one_batch(package, "program.join_node")?;
        let ids = utf8(joins, "node_id")?;
        let types = utf8(joins, "join_type")?;
        let conditions = utf8(joins, "condition_expr_id")?;
        for row in 0..joins.num_rows() {
            insert_unique(
                &mut nodes,
                ids.value(row),
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
                ids.value(row),
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
                ids.value(row),
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
                ids.value(row),
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
                insert_unique(&mut expressions, ids.value(row), expression, "expression")?;
            }
        }
        let cases = one_batch(package, "program.case_expr")?;
        let ids = utf8(cases, "expr_id")?;
        for row in 0..cases.num_rows() {
            insert_unique(
                &mut expressions,
                ids.value(row),
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
        if self.nodes.len() > MAX_PROGRAM_NODES || self.expressions.len() > MAX_PROGRAM_EXPRESSIONS
        {
            return Err(OntologyProgramCompileError::Decode(format!(
                "program graph exceeds node/expression bounds {MAX_PROGRAM_NODES}/{MAX_PROGRAM_EXPRESSIONS}"
            )));
        }
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
            if program.execution_phase == "candidate_validation"
                && !self.nodes.contains_key(&program.root_node_id)
            {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "{} has unknown root {}",
                    program.program_id, program.root_node_id
                )));
            }
        }
        let mut visited_nodes = BTreeSet::new();
        let mut active_nodes = BTreeSet::new();
        for program in self
            .programs
            .values()
            .filter(|program| !program.root_node_id.is_empty())
        {
            self.validate_plan_acyclic(
                &program.root_node_id,
                &mut visited_nodes,
                &mut active_nodes,
                0,
            )?;
        }
        let all_nodes = self.nodes.keys().cloned().collect::<BTreeSet<_>>();
        if visited_nodes != all_nodes {
            return Err(OntologyProgramCompileError::Decode(format!(
                "unreachable plan nodes: {:?}",
                all_nodes.difference(&visited_nodes).collect::<Vec<_>>()
            )));
        }

        let mut expression_roots = BTreeSet::new();
        for (node_id, node) in &self.nodes {
            self.validate_plan_node_shape(node_id, node, &mut expression_roots)?;
        }
        let mut visited_expressions = BTreeSet::new();
        let mut active_expressions = BTreeSet::new();
        for expr_id in expression_roots {
            self.validate_expression_acyclic(
                &expr_id,
                &mut visited_expressions,
                &mut active_expressions,
                0,
            )?;
        }
        let all_expressions = self.expressions.keys().cloned().collect::<BTreeSet<_>>();
        if visited_expressions != all_expressions {
            return Err(OntologyProgramCompileError::Decode(format!(
                "unreachable expressions: {:?}",
                all_expressions
                    .difference(&visited_expressions)
                    .collect::<Vec<_>>()
            )));
        }
        Ok(())
    }

    fn validate_plan_node_shape(
        &self,
        node_id: &str,
        node: &PlanNode,
        expression_roots: &mut BTreeSet<String>,
    ) -> Result<(), OntologyProgramCompileError> {
        let plan_child_count = self.plan_edges.get(node_id).map_or(0, Vec::len);
        let expression_edges = self
            .expression_edges
            .get(node_id)
            .map_or(&[][..], Vec::as_slice);
        let allowed_roles: &[&str] = match node {
            PlanNode::Scan(_) => {
                if plan_child_count != 0 {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "scan {node_id} has plan children"
                    )));
                }
                &[]
            }
            PlanNode::Filter { predicate_expr_id } => {
                if plan_child_count != 1 {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "filter {node_id} does not have one input"
                    )));
                }
                expression_roots.insert(predicate_expr_id.clone());
                &[]
            }
            PlanNode::Project => {
                if plan_child_count != 1
                    || !expression_edges
                        .iter()
                        .any(|edge| edge.role == "projection")
                {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "project {node_id} lacks its input or projections"
                    )));
                }
                &["projection"]
            }
            PlanNode::Join {
                condition_expr_id, ..
            } => {
                if plan_child_count != 2 {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "join {node_id} does not have two inputs"
                    )));
                }
                expression_roots.insert(condition_expr_id.clone());
                &[]
            }
            PlanNode::Aggregate => {
                if plan_child_count != 1
                    || !expression_edges.iter().any(|edge| edge.role == "aggregate")
                {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "aggregate {node_id} lacks its input or aggregate expressions"
                    )));
                }
                &["group", "aggregate"]
            }
            PlanNode::Set { .. } => {
                if plan_child_count < 2 {
                    return Err(OntologyProgramCompileError::Decode(format!(
                        "set {node_id} has fewer than two inputs"
                    )));
                }
                &[]
            }
        };
        for edge in expression_edges {
            if !allowed_roles.contains(&edge.role.as_str()) {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "plan node {node_id} has unsupported expression role {}",
                    edge.role
                )));
            }
            if edge.output_alias.as_deref().is_some_and(str::is_empty) {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "plan node {node_id} has an empty output alias"
                )));
            }
            expression_roots.insert(edge.child_expr_id.clone());
        }
        Ok(())
    }

    fn validate_plan_acyclic(
        &self,
        node_id: &str,
        visited: &mut BTreeSet<String>,
        active: &mut BTreeSet<String>,
        depth: usize,
    ) -> Result<(), OntologyProgramCompileError> {
        if depth > MAX_PROGRAM_GRAPH_DEPTH {
            return Err(OntologyProgramCompileError::Decode(format!(
                "plan graph exceeds depth bound at {node_id}"
            )));
        }
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
                self.validate_plan_acyclic(&edge.child_node_id, visited, active, depth + 1)?;
            }
        }
        active.remove(node_id);
        visited.insert(node_id.to_owned());
        Ok(())
    }

    fn validate_expression_acyclic(
        &self,
        expr_id: &str,
        visited: &mut BTreeSet<String>,
        active: &mut BTreeSet<String>,
        depth: usize,
    ) -> Result<(), OntologyProgramCompileError> {
        if depth > MAX_PROGRAM_GRAPH_DEPTH {
            return Err(OntologyProgramCompileError::Decode(format!(
                "expression graph exceeds depth bound at {expr_id}"
            )));
        }
        if visited.contains(expr_id) {
            return Ok(());
        }
        if !active.insert(expr_id.to_owned()) {
            return Err(OntologyProgramCompileError::Decode(format!(
                "cycle at expression {expr_id}"
            )));
        }
        let expression = self.expressions.get(expr_id).ok_or_else(|| {
            OntologyProgramCompileError::Decode(format!("unknown expression {expr_id}"))
        })?;
        let edges = self
            .expression_edges
            .get(expr_id)
            .map_or(&[][..], Vec::as_slice);
        let expected_roles: &[(&str, usize)] = match expression {
            ExpressionNode::Column { .. } | ExpressionNode::Literal { .. } => &[],
            ExpressionNode::Binary { .. } => &[("left", 1), ("right", 1)],
            ExpressionNode::Call { function_name } => match function_name.as_str() {
                "is_null" | "is_not_null" | "not" | "is_true" | "count" => &[("argument", 1)],
                function => {
                    return Err(OntologyProgramCompileError::Unsupported(format!(
                        "built-in call {function}"
                    )));
                }
            },
            ExpressionNode::Cast { .. } => &[("argument", 1)],
            ExpressionNode::Case => {
                return Err(OntologyProgramCompileError::Unsupported(
                    "case expression lacks a released current-profile contract".into(),
                ));
            }
        };
        for (role, expected_count) in expected_roles {
            let count = edges.iter().filter(|edge| edge.role == *role).count();
            if count != *expected_count {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "expression {expr_id} requires {expected_count} {role} edge(s), found {count}"
                )));
            }
        }
        for edge in edges {
            if !expected_roles.iter().any(|(role, _)| *role == edge.role) {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "expression {expr_id} has unsupported role {}",
                    edge.role
                )));
            }
            if edge.output_alias.is_some() {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "expression {expr_id} has an illegal child output alias"
                )));
            }
            self.validate_expression_acyclic(&edge.child_expr_id, visited, active, depth + 1)?;
        }
        active.remove(expr_id);
        visited.insert(expr_id.to_owned());
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

    #[allow(clippy::too_many_lines)] // Exhaustive generated expression lowering mirrors the closed operator census.
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

    #[allow(clippy::too_many_lines)] // Exhaustive generated plan lowering mirrors the closed node census.
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
    ///
    /// # Errors
    ///
    /// Rejects an unknown or analyzer-owned program and reports malformed generated nodes,
    /// expressions, provider bindings, or DataFusion logical-plan construction.
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
///
/// Candidate binding already fixes table identity through the generated `table:<code>` provider
/// key. Schema-level metadata describes the durable source table, and combining distinct source
/// maps in an ordinary DataFusion join can make the DataFusion 55 projection-pushdown rule reject
/// an otherwise type-correct plan. The semantic execution plane therefore retains every Arrow
/// field (including field metadata and extension types) while normalizing table-level metadata at
/// this one boundary. Durable/Delta schema validation remains against the unmodified source schema.
///
/// # Errors
///
/// Returns a decode error when a normalized Arrow batch or DataFusion memory provider cannot be
/// constructed from the candidate arrays.
pub fn candidate_batch_providers(
    batches: &BTreeMap<i16, RecordBatch>,
) -> Result<BTreeMap<String, Arc<dyn TableProvider>>, OntologyProgramCompileError> {
    batches
        .iter()
        .map(|(table_code, batch)| {
            let execution_schema =
                Arc::new(arrow_schema::Schema::new(batch.schema().fields().clone()));
            let execution_batch =
                RecordBatch::try_new(execution_schema.clone(), batch.columns().to_vec())
                    .map_err(|error| OntologyProgramCompileError::Decode(error.to_string()))?;
            let provider: Arc<dyn TableProvider> = Arc::new(
                MemTable::try_new(execution_schema, vec![vec![execution_batch]])
                    .map_err(|error| OntologyProgramCompileError::Decode(error.to_string()))?,
            );
            Ok((format!("table:{table_code}"), provider))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Arc;

    use arrow_array::{Int64Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};

    use super::candidate_batch_providers;

    #[test]
    fn semantic_provider_normalizes_only_table_metadata() {
        let field = Field::new("value", DataType::Int64, false)
            .with_metadata(HashMap::from([("id-domain".into(), "entity".into())]));
        let schema = Arc::new(Schema::new_with_metadata(
            vec![field],
            HashMap::from([("table-name".into(), "entity".into())]),
        ));
        let batch = RecordBatch::try_new(schema, vec![Arc::new(Int64Array::from(vec![1]))])
            .expect("metadata-bearing batch");
        let providers =
            candidate_batch_providers(&BTreeMap::from([(100, batch)])).expect("semantic providers");
        let execution_schema = providers["table:100"].schema();
        assert!(execution_schema.metadata().is_empty());
        assert_eq!(
            execution_schema.field(0).metadata().get("id-domain"),
            Some(&"entity".to_owned())
        );
    }
}
