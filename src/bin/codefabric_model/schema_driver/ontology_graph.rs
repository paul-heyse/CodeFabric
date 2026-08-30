//! Normalizes authored schema/rule contracts into typed relational-program batches.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, RecordBatch, StringArray, UInt16Array};
use arrow_schema::{DataType, Field, Schema};

use super::{
    OntologyExecutionPhase, OntologyProgramOperandContract, OntologyProgramOperandRole,
    OntologyProgramOperationContract, SchemaContractIr, SchemaDriverError, invalid,
    ontology_artifact_error,
};

#[derive(Clone, Debug)]
struct ProgramRow {
    program_id: String,
    rule_id: String,
    root_node_id: String,
    execution_phase: String,
    calculation_id: String,
    policy_id: String,
    expected_result_contract: String,
    diagnostic_code: String,
    rule_semantics_identity: String,
    subject_table_id: String,
}

#[derive(Clone, Debug)]
struct ScanRow {
    node_id: String,
    relation_ref: String,
    relation_alias: String,
}

#[derive(Clone, Debug)]
struct FilterRow {
    node_id: String,
    predicate_expr_id: String,
}

#[derive(Clone, Debug)]
struct JoinRow {
    node_id: String,
    join_type: String,
    condition_expr_id: String,
}

#[derive(Clone, Debug)]
struct SetRow {
    node_id: String,
    set_operation: String,
}

#[derive(Clone, Debug)]
struct ColumnRow {
    expr_id: String,
    relation_alias: String,
    column_name: String,
}

#[derive(Clone, Debug)]
struct LiteralRow {
    expr_id: String,
    logical_type: String,
    value: Option<String>,
    is_null: bool,
}

#[derive(Clone, Debug)]
struct BinaryRow {
    expr_id: String,
    operator: String,
}

#[derive(Clone, Debug)]
struct CallRow {
    expr_id: String,
    function_name: String,
}

#[derive(Clone, Debug)]
struct CastRow {
    expr_id: String,
    target_type: String,
}

#[derive(Clone, Debug)]
struct PlanEdgeRow {
    parent_node_id: String,
    child_node_id: String,
    input_ordinal: u16,
}

#[derive(Clone, Debug)]
struct ExpressionEdgeRow {
    parent_id: String,
    child_expr_id: String,
    role: String,
    operand_ordinal: u16,
    output_alias: Option<String>,
}

#[derive(Default)]
struct Graph {
    programs: Vec<ProgramRow>,
    scans: Vec<ScanRow>,
    filters: Vec<FilterRow>,
    projects: Vec<String>,
    joins: Vec<JoinRow>,
    aggregates: Vec<String>,
    sets: Vec<SetRow>,
    columns: Vec<ColumnRow>,
    literals: Vec<LiteralRow>,
    binaries: Vec<BinaryRow>,
    calls: Vec<CallRow>,
    casts: Vec<CastRow>,
    plan_edges: Vec<PlanEdgeRow>,
    expression_edges: Vec<ExpressionEdgeRow>,
    serial: usize,
}

impl Graph {
    fn id(&mut self, prefix: &str) -> String {
        let id = format!("generated.{prefix}:{:05}", self.serial);
        self.serial += 1;
        id
    }

    fn literal(&mut self, logical_type: &str, value: impl Into<String>) -> String {
        let expr_id = self.id("expr.literal");
        self.literals.push(LiteralRow {
            expr_id: expr_id.clone(),
            logical_type: logical_type.to_owned(),
            value: Some(value.into()),
            is_null: false,
        });
        expr_id
    }

    fn plan_edge(&mut self, parent: &str, child: impl Into<String>, ordinal: u16) {
        self.plan_edges.push(PlanEdgeRow {
            parent_node_id: parent.to_owned(),
            child_node_id: child.into(),
            input_ordinal: ordinal,
        });
    }

    fn expression_edge(
        &mut self,
        parent: &str,
        child: impl Into<String>,
        role: &str,
        ordinal: u16,
        output_alias: Option<String>,
    ) {
        self.expression_edges.push(ExpressionEdgeRow {
            parent_id: parent.to_owned(),
            child_expr_id: child.into(),
            role: role.to_owned(),
            operand_ordinal: ordinal,
            output_alias,
        });
    }

    fn violation_projection(
        &mut self,
        child: String,
        program_id: &str,
        rule_id: &str,
        diagnostic_code: &str,
        rule_semantics_identity: &str,
        subject_table_id: &str,
    ) -> String {
        let node_id = self.id("plan.project");
        self.projects.push(node_id.clone());
        self.plan_edge(&node_id, child, 0);
        for (ordinal, (alias, value)) in [
            ("program_id", program_id),
            ("rule_id", rule_id),
            ("diagnostic_code", diagnostic_code),
            ("rule_semantics_identity", rule_semantics_identity),
            ("subject_table_id", subject_table_id),
            ("detail_code", "relation-row"),
        ]
        .into_iter()
        .enumerate()
        {
            let expression = self.literal("utf8", value);
            self.expression_edge(
                &node_id,
                expression,
                "projection",
                u16::try_from(ordinal).expect("projection ordinal"),
                Some(alias.to_owned()),
            );
        }
        node_id
    }
}

fn operation_id(operation: &OntologyProgramOperationContract) -> &str {
    operation.operation_id()
}

fn is_plan_operation(operation: &OntologyProgramOperationContract) -> bool {
    matches!(
        operation,
        OntologyProgramOperationContract::Scan { .. }
            | OntologyProgramOperationContract::Filter { .. }
            | OntologyProgramOperationContract::Join { .. }
            | OntologyProgramOperationContract::Aggregate { .. }
            | OntologyProgramOperationContract::Set { .. }
    )
}

fn operation_record(operation: &OntologyProgramOperationContract) -> String {
    use codefabric::ontology_contract::ontology_semantics_record;
    match operation {
        OntologyProgramOperationContract::Scan {
            operation_id,
            relation_ref,
            relation_alias,
        } => ontology_semantics_record("SCAN", [operation_id, relation_ref, relation_alias]),
        OntologyProgramOperationContract::Filter { operation_id } => {
            ontology_semantics_record("FILTER", [operation_id.as_str()])
        }
        OntologyProgramOperationContract::Join {
            operation_id,
            join_type,
        } => ontology_semantics_record("JOIN", [operation_id, join_type]),
        OntologyProgramOperationContract::Aggregate { operation_id } => {
            ontology_semantics_record("AGGREGATE", [operation_id.as_str()])
        }
        OntologyProgramOperationContract::Set {
            operation_id,
            set_operation,
        } => ontology_semantics_record("SET", [operation_id, set_operation]),
        OntologyProgramOperationContract::Column {
            operation_id,
            relation_alias,
            column_name,
        } => ontology_semantics_record("COLUMN", [operation_id, relation_alias, column_name]),
        OntologyProgramOperationContract::Literal {
            operation_id,
            logical_type,
            value,
            is_null,
        } => {
            let null = is_null.to_string();
            ontology_semantics_record(
                "LITERAL",
                [
                    operation_id,
                    logical_type,
                    value.as_deref().unwrap_or(""),
                    &null,
                ],
            )
        }
        OntologyProgramOperationContract::Binary {
            operation_id,
            operator,
        } => ontology_semantics_record("BINARY", [operation_id, operator]),
        OntologyProgramOperationContract::Call {
            operation_id,
            function_name,
        } => ontology_semantics_record("CALL", [operation_id, function_name]),
        OntologyProgramOperationContract::Cast {
            operation_id,
            target_type,
        } => ontology_semantics_record("CAST", [operation_id, target_type]),
    }
}

fn operand_record(operand: &OntologyProgramOperandContract) -> String {
    let ordinal = operand.ordinal.to_string();
    codefabric::ontology_contract::ontology_semantics_record(
        "OPERAND",
        [
            operand.parent_operation_id.as_str(),
            operand.child_operation_id.as_str(),
            operand.role.as_str(),
            ordinal.as_str(),
            operand.output_alias.as_deref().unwrap_or(""),
        ],
    )
}

fn program_record(
    program: &super::OntologyProgramContract,
    rule: &super::OntologyRuleContract,
) -> String {
    codefabric::ontology_contract::ontology_semantics_record(
        "PROGRAM",
        [
            program.program_id.as_str(),
            program.root_operation_id.as_deref().unwrap_or(""),
            program.execution_phase.as_str(),
            program.subject_table_id.as_str(),
            rule.rule_id.as_str(),
            rule.calculation_id.as_str(),
            rule.policy_id.as_str(),
            rule.output_contract.as_str(),
            rule.diagnostic_code.as_str(),
        ],
    )
}

fn operands_by_parent<'a>(
    ir: &'a SchemaContractIr,
) -> BTreeMap<&'a str, Vec<&'a OntologyProgramOperandContract>> {
    let mut by_parent = BTreeMap::<&str, Vec<&OntologyProgramOperandContract>>::new();
    for operand in &ir.ontology_program_graph.operands {
        by_parent
            .entry(&operand.parent_operation_id)
            .or_default()
            .push(operand);
    }
    by_parent
}

fn role_operands<'a>(
    by_parent: &'a BTreeMap<&str, Vec<&OntologyProgramOperandContract>>,
    operation_id: &str,
    role: OntologyProgramOperandRole,
) -> Vec<&'a OntologyProgramOperandContract> {
    let mut operands = by_parent
        .get(operation_id)
        .map_or_else(Vec::new, |operands| {
            operands
                .iter()
                .copied()
                .filter(|operand| operand.role == role)
                .collect()
        });
    operands.sort_by_key(|operand| operand.ordinal);
    operands
}

fn visit_operations(
    operation_id: &str,
    by_parent: &BTreeMap<&str, Vec<&OntologyProgramOperandContract>>,
    visited: &mut BTreeSet<String>,
    active: &mut BTreeSet<String>,
    depth: usize,
) -> Result<(), SchemaDriverError> {
    if depth > 256 {
        return invalid(
            "$.ontology_program_graph",
            "authored operation graph exceeds depth bound",
        );
    }
    if visited.contains(operation_id) {
        return Ok(());
    }
    if !active.insert(operation_id.to_owned()) {
        return invalid(
            "$.ontology_program_graph",
            "authored operation graph contains a cycle",
        );
    }
    if let Some(operands) = by_parent.get(operation_id) {
        for operand in operands {
            visit_operations(
                &operand.child_operation_id,
                by_parent,
                visited,
                active,
                depth + 1,
            )?;
        }
    }
    active.remove(operation_id);
    visited.insert(operation_id.to_owned());
    Ok(())
}

fn validate_config(operation: &OntologyProgramOperationContract) -> bool {
    match operation {
        OntologyProgramOperationContract::Scan {
            relation_ref,
            relation_alias,
            ..
        } => !relation_ref.is_empty() && !relation_alias.is_empty(),
        OntologyProgramOperationContract::Filter { .. }
        | OntologyProgramOperationContract::Aggregate { .. } => true,
        OntologyProgramOperationContract::Join { join_type, .. } => {
            matches!(
                join_type.as_str(),
                "inner" | "left" | "left_semi" | "left_anti"
            )
        }
        OntologyProgramOperationContract::Set { set_operation, .. } => set_operation == "union_all",
        OntologyProgramOperationContract::Column { column_name, .. } => !column_name.is_empty(),
        OntologyProgramOperationContract::Literal {
            logical_type,
            value,
            is_null,
            ..
        } => {
            matches!(
                logical_type.as_str(),
                "utf8" | "boolean" | "int16" | "int32" | "int64"
            ) && (*is_null == value.is_none())
        }
        OntologyProgramOperationContract::Binary { operator, .. } => matches!(
            operator.as_str(),
            "eq" | "neq" | "gt" | "gte" | "lt" | "lte" | "and" | "or"
        ),
        OntologyProgramOperationContract::Call { function_name, .. } => matches!(
            function_name.as_str(),
            "is_null" | "is_not_null" | "not" | "is_true" | "count"
        ),
        OntologyProgramOperationContract::Cast { target_type, .. } => {
            matches!(target_type.as_str(), "int16" | "int32" | "int64" | "utf8")
        }
    }
}

fn expected_role_counts(
    operation: &OntologyProgramOperationContract,
) -> &'static [(OntologyProgramOperandRole, usize, Option<usize>)] {
    use OntologyProgramOperandRole as Role;
    match operation {
        OntologyProgramOperationContract::Scan { .. }
        | OntologyProgramOperationContract::Column { .. }
        | OntologyProgramOperationContract::Literal { .. } => &[],
        OntologyProgramOperationContract::Filter { .. } => {
            &[(Role::Input, 1, Some(1)), (Role::Predicate, 1, Some(1))]
        }
        OntologyProgramOperationContract::Join { .. } => {
            &[(Role::Input, 2, Some(2)), (Role::Condition, 1, Some(1))]
        }
        OntologyProgramOperationContract::Aggregate { .. } => &[
            (Role::Input, 1, Some(1)),
            (Role::Group, 0, None),
            (Role::Aggregate, 1, None),
        ],
        OntologyProgramOperationContract::Set { .. } => &[(Role::Input, 2, None)],
        OntologyProgramOperationContract::Binary { .. } => {
            &[(Role::Left, 1, Some(1)), (Role::Right, 1, Some(1))]
        }
        OntologyProgramOperationContract::Call { .. }
        | OntologyProgramOperationContract::Cast { .. } => &[(Role::Argument, 1, Some(1))],
    }
}

pub(super) fn validate_authored_graph(ir: &SchemaContractIr) -> Result<(), SchemaDriverError> {
    let graph = &ir.ontology_program_graph;
    if graph.programs.is_empty() || graph.operations.is_empty() {
        return invalid(
            "$.ontology_program_graph",
            "authored program and operation relations must be nonempty",
        );
    }
    if graph.operations.len() > 65_536 || graph.operands.len() > 262_144 {
        return invalid(
            "$.ontology_program_graph",
            "authored operation graph exceeds count bounds",
        );
    }
    let rule_ids = ir
        .ontology_rule_contracts
        .iter()
        .map(|rule| rule.rule_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut program_ids = BTreeSet::new();
    let mut referenced_rules = BTreeSet::new();
    for program in &graph.programs {
        if program.program_id.is_empty()
            || program.subject_table_id.is_empty()
            || !program_ids.insert(program.program_id.as_str())
            || !rule_ids.contains(program.rule_id.as_str())
        {
            return invalid(
                "$.ontology_program_graph.programs",
                "program is empty, duplicated, or references an unknown rule",
            );
        }
        referenced_rules.insert(program.rule_id.as_str());
        match (program.execution_phase, &program.root_operation_id) {
            (OntologyExecutionPhase::CandidateValidation, Some(root)) if !root.is_empty() => {}
            (OntologyExecutionPhase::SemanticAnalysis, None) => {}
            _ => {
                return invalid(
                    "$.ontology_program_graph.programs",
                    "execution phase and root operation are inconsistent",
                );
            }
        }
    }
    if referenced_rules != rule_ids {
        return invalid(
            "$.ontology_program_graph.programs",
            "every authored rule must own at least one program",
        );
    }

    let mut operations = BTreeMap::new();
    for operation in &graph.operations {
        let id = operation_id(operation);
        if id.is_empty()
            || id.starts_with("generated.")
            || operations.insert(id, operation).is_some()
            || !validate_config(operation)
        {
            return invalid(
                "$.ontology_program_graph.operations",
                "operation is empty, duplicated, reserved, or unsupported",
            );
        }
    }
    let by_parent = operands_by_parent(ir);
    let mut ordinals = BTreeMap::<(&str, OntologyProgramOperandRole), Vec<u16>>::new();
    for operand in &graph.operands {
        let output_role = matches!(
            operand.role,
            OntologyProgramOperandRole::Group | OntologyProgramOperandRole::Aggregate
        );
        if !operations.contains_key(operand.parent_operation_id.as_str())
            || !operations.contains_key(operand.child_operation_id.as_str())
            || output_role
                != operand
                    .output_alias
                    .as_deref()
                    .is_some_and(|alias| !alias.is_empty())
        {
            return invalid(
                "$.ontology_program_graph.operands",
                "operand is dangling or violates its exact output-alias contract",
            );
        }
        ordinals
            .entry((operand.parent_operation_id.as_str(), operand.role))
            .or_default()
            .push(operand.ordinal);
    }
    for values in ordinals.values_mut() {
        values.sort_unstable();
        if values
            .iter()
            .enumerate()
            .any(|(index, ordinal)| usize::from(*ordinal) != index)
        {
            return invalid(
                "$.ontology_program_graph.operands",
                "operand ordinals are not contiguous within their role",
            );
        }
    }
    for (id, operation) in &operations {
        let expected = expected_role_counts(operation);
        let actual = by_parent.get(id).map_or(&[][..], Vec::as_slice);
        if actual
            .iter()
            .any(|operand| !expected.iter().any(|(role, _, _)| *role == operand.role))
        {
            return invalid(
                "$.ontology_program_graph.operands",
                "operation has an unsupported operand role",
            );
        }
        for (role, minimum, maximum) in expected {
            let count = actual
                .iter()
                .filter(|operand| operand.role == *role)
                .count();
            if count < *minimum || maximum.is_some_and(|maximum| count > maximum) {
                return invalid(
                    "$.ontology_program_graph.operands",
                    "operation operand arity differs from its typed contract",
                );
            }
        }
        for operand in actual {
            let child = operations[operand.child_operation_id.as_str()];
            let categories_match = match operand.role {
                OntologyProgramOperandRole::Input => {
                    is_plan_operation(operation) && is_plan_operation(child)
                }
                OntologyProgramOperandRole::Predicate
                | OntologyProgramOperandRole::Condition
                | OntologyProgramOperandRole::Left
                | OntologyProgramOperandRole::Right
                | OntologyProgramOperandRole::Argument
                | OntologyProgramOperandRole::Group
                | OntologyProgramOperandRole::Aggregate => !is_plan_operation(child),
            };
            if !categories_match {
                return invalid(
                    "$.ontology_program_graph.operands",
                    "operand connects incompatible operation categories",
                );
            }
        }
    }

    let mut all_visited = BTreeSet::new();
    let mut owners = BTreeMap::<String, &str>::new();
    for program in graph
        .programs
        .iter()
        .filter(|program| program.execution_phase == OntologyExecutionPhase::CandidateValidation)
    {
        let root = program
            .root_operation_id
            .as_deref()
            .expect("phase/root validated");
        if !operations
            .get(root)
            .is_some_and(|operation| is_plan_operation(operation))
        {
            return invalid(
                "$.ontology_program_graph.programs",
                "candidate program root is absent or not a plan operation",
            );
        }
        let mut visited = BTreeSet::new();
        visit_operations(root, &by_parent, &mut visited, &mut BTreeSet::new(), 0)?;
        for operation_id in visited {
            if let Some(owner) = owners.insert(operation_id.clone(), &program.rule_id)
                && owner != program.rule_id
            {
                return invalid(
                    "$.ontology_program_graph",
                    "an authored operation is shared by different rules",
                );
            }
            all_visited.insert(operation_id);
        }
    }
    if all_visited.len() != operations.len() {
        return invalid(
            "$.ontology_program_graph.operations",
            "authored operation relation contains unreachable rows",
        );
    }
    Ok(())
}

pub(super) fn rule_semantics_identity(
    ir: &SchemaContractIr,
    rule: &super::OntologyRuleContract,
) -> Result<String, SchemaDriverError> {
    let by_parent = operands_by_parent(ir);
    let operations = ir
        .ontology_program_graph
        .operations
        .iter()
        .map(|operation| (operation_id(operation), operation))
        .collect::<BTreeMap<_, _>>();
    let programs = ir
        .ontology_program_graph
        .programs
        .iter()
        .filter(|program| program.rule_id == rule.rule_id)
        .collect::<Vec<_>>();
    let mut reachable = BTreeSet::new();
    for program in &programs {
        if let Some(root) = &program.root_operation_id {
            visit_operations(root, &by_parent, &mut reachable, &mut BTreeSet::new(), 0)?;
        }
    }
    let mut records = programs
        .iter()
        .map(|program| program_record(program, rule))
        .collect::<Vec<_>>();
    records.extend(
        reachable
            .iter()
            .map(|operation_id| operation_record(operations[operation_id.as_str()])),
    );
    records.extend(
        ir.ontology_program_graph
            .operands
            .iter()
            .filter(|operand| reachable.contains(&operand.parent_operation_id))
            .map(operand_record),
    );
    Ok(codefabric::ontology_contract::rule_semantics_identity(
        records.iter().map(String::as_str),
    ))
}

fn build_graph(ir: &SchemaContractIr) -> Result<Graph, SchemaDriverError> {
    validate_authored_graph(ir)?;
    let by_parent = operands_by_parent(ir);
    let mut graph = Graph {
        serial: 1_000_000,
        ..Graph::default()
    };
    for operation in &ir.ontology_program_graph.operations {
        match operation {
            OntologyProgramOperationContract::Scan {
                operation_id,
                relation_ref,
                relation_alias,
            } => graph.scans.push(ScanRow {
                node_id: operation_id.clone(),
                relation_ref: relation_ref.clone(),
                relation_alias: relation_alias.clone(),
            }),
            OntologyProgramOperationContract::Filter { operation_id } => {
                let predicate = role_operands(
                    &by_parent,
                    operation_id,
                    OntologyProgramOperandRole::Predicate,
                );
                graph.filters.push(FilterRow {
                    node_id: operation_id.clone(),
                    predicate_expr_id: predicate[0].child_operation_id.clone(),
                });
            }
            OntologyProgramOperationContract::Join {
                operation_id,
                join_type,
            } => {
                let condition = role_operands(
                    &by_parent,
                    operation_id,
                    OntologyProgramOperandRole::Condition,
                );
                graph.joins.push(JoinRow {
                    node_id: operation_id.clone(),
                    join_type: join_type.clone(),
                    condition_expr_id: condition[0].child_operation_id.clone(),
                });
            }
            OntologyProgramOperationContract::Aggregate { operation_id } => {
                graph.aggregates.push(operation_id.clone());
            }
            OntologyProgramOperationContract::Set {
                operation_id,
                set_operation,
            } => graph.sets.push(SetRow {
                node_id: operation_id.clone(),
                set_operation: set_operation.clone(),
            }),
            OntologyProgramOperationContract::Column {
                operation_id,
                relation_alias,
                column_name,
            } => graph.columns.push(ColumnRow {
                expr_id: operation_id.clone(),
                relation_alias: relation_alias.clone(),
                column_name: column_name.clone(),
            }),
            OntologyProgramOperationContract::Literal {
                operation_id,
                logical_type,
                value,
                is_null,
            } => graph.literals.push(LiteralRow {
                expr_id: operation_id.clone(),
                logical_type: logical_type.clone(),
                value: value.clone(),
                is_null: *is_null,
            }),
            OntologyProgramOperationContract::Binary {
                operation_id,
                operator,
            } => graph.binaries.push(BinaryRow {
                expr_id: operation_id.clone(),
                operator: operator.clone(),
            }),
            OntologyProgramOperationContract::Call {
                operation_id,
                function_name,
            } => graph.calls.push(CallRow {
                expr_id: operation_id.clone(),
                function_name: function_name.clone(),
            }),
            OntologyProgramOperationContract::Cast {
                operation_id,
                target_type,
            } => graph.casts.push(CastRow {
                expr_id: operation_id.clone(),
                target_type: target_type.clone(),
            }),
        }
    }
    for operand in &ir.ontology_program_graph.operands {
        match operand.role {
            OntologyProgramOperandRole::Input => graph.plan_edge(
                &operand.parent_operation_id,
                operand.child_operation_id.clone(),
                operand.ordinal,
            ),
            OntologyProgramOperandRole::Predicate | OntologyProgramOperandRole::Condition => {}
            role => graph.expression_edge(
                &operand.parent_operation_id,
                operand.child_operation_id.clone(),
                role.as_str(),
                operand.ordinal,
                operand.output_alias.clone(),
            ),
        }
    }
    let rules = ir
        .ontology_rule_contracts
        .iter()
        .map(|rule| (rule.rule_id.as_str(), rule))
        .collect::<BTreeMap<_, _>>();
    for program in &ir.ontology_program_graph.programs {
        let rule = rules[program.rule_id.as_str()];
        let semantics_identity = rule_semantics_identity(ir, rule)?;
        let root_node_id = if let Some(root) = &program.root_operation_id {
            graph.violation_projection(
                root.clone(),
                &program.program_id,
                &rule.rule_id,
                &rule.diagnostic_code,
                &semantics_identity,
                &program.subject_table_id,
            )
        } else {
            String::new()
        };
        graph.programs.push(ProgramRow {
            program_id: program.program_id.clone(),
            rule_id: rule.rule_id.clone(),
            root_node_id,
            execution_phase: program.execution_phase.as_str().to_owned(),
            calculation_id: rule.calculation_id.clone(),
            policy_id: rule.policy_id.clone(),
            expected_result_contract: rule.output_contract.clone(),
            diagnostic_code: rule.diagnostic_code.clone(),
            rule_semantics_identity: semantics_identity,
            subject_table_id: program.subject_table_id.clone(),
        });
    }
    Ok(graph)
}

fn batch(fields: Vec<Field>, columns: Vec<ArrayRef>) -> Result<RecordBatch, SchemaDriverError> {
    RecordBatch::try_new(Arc::new(Schema::new(fields)), columns)
        .map_err(|error| ontology_artifact_error(error.to_string()))
}

fn strings(values: impl IntoIterator<Item = String>) -> ArrayRef {
    Arc::new(StringArray::from_iter_values(values))
}

fn empty_ids(name: &str) -> Result<RecordBatch, SchemaDriverError> {
    batch(
        vec![Field::new(name, DataType::Utf8, false)],
        vec![Arc::new(StringArray::from(Vec::<&str>::new()))],
    )
}

pub(super) fn program_graph_batches(
    ir: &SchemaContractIr,
) -> Result<BTreeMap<String, RecordBatch>, SchemaDriverError> {
    let graph = build_graph(ir)?;
    let mut batches = BTreeMap::new();
    batches.insert(
        "program.program_contract".into(),
        batch(
            vec![
                Field::new("program_id", DataType::Utf8, false),
                Field::new("rule_id", DataType::Utf8, false),
                Field::new("root_node_id", DataType::Utf8, false),
                Field::new("execution_phase", DataType::Utf8, false),
                Field::new("calculation_id", DataType::Utf8, false),
                Field::new("policy_id", DataType::Utf8, false),
                Field::new("expected_result_contract", DataType::Utf8, false),
                Field::new("diagnostic_code", DataType::Utf8, false),
                Field::new("rule_semantics_identity", DataType::Utf8, false),
                Field::new("subject_table_id", DataType::Utf8, false),
            ],
            vec![
                strings(graph.programs.iter().map(|row| row.program_id.clone())),
                strings(graph.programs.iter().map(|row| row.rule_id.clone())),
                strings(graph.programs.iter().map(|row| row.root_node_id.clone())),
                strings(graph.programs.iter().map(|row| row.execution_phase.clone())),
                strings(graph.programs.iter().map(|row| row.calculation_id.clone())),
                strings(graph.programs.iter().map(|row| row.policy_id.clone())),
                strings(
                    graph
                        .programs
                        .iter()
                        .map(|row| row.expected_result_contract.clone()),
                ),
                strings(graph.programs.iter().map(|row| row.diagnostic_code.clone())),
                strings(
                    graph
                        .programs
                        .iter()
                        .map(|row| row.rule_semantics_identity.clone()),
                ),
                strings(
                    graph
                        .programs
                        .iter()
                        .map(|row| row.subject_table_id.clone()),
                ),
            ],
        )?,
    );
    batches.insert(
        "program.scan_node".into(),
        batch(
            vec![
                Field::new("node_id", DataType::Utf8, false),
                Field::new("relation_ref", DataType::Utf8, false),
                Field::new("relation_alias", DataType::Utf8, false),
            ],
            vec![
                strings(graph.scans.iter().map(|row| row.node_id.clone())),
                strings(graph.scans.iter().map(|row| row.relation_ref.clone())),
                strings(graph.scans.iter().map(|row| row.relation_alias.clone())),
            ],
        )?,
    );
    batches.insert(
        "program.filter_node".into(),
        batch(
            vec![
                Field::new("node_id", DataType::Utf8, false),
                Field::new("predicate_expr_id", DataType::Utf8, false),
            ],
            vec![
                strings(graph.filters.iter().map(|row| row.node_id.clone())),
                strings(
                    graph
                        .filters
                        .iter()
                        .map(|row| row.predicate_expr_id.clone()),
                ),
            ],
        )?,
    );
    batches.insert(
        "program.project_node".into(),
        batch(
            vec![Field::new("node_id", DataType::Utf8, false)],
            vec![strings(graph.projects.iter().cloned())],
        )?,
    );
    batches.insert(
        "program.join_node".into(),
        batch(
            vec![
                Field::new("node_id", DataType::Utf8, false),
                Field::new("join_type", DataType::Utf8, false),
                Field::new("condition_expr_id", DataType::Utf8, false),
            ],
            vec![
                strings(graph.joins.iter().map(|row| row.node_id.clone())),
                strings(graph.joins.iter().map(|row| row.join_type.clone())),
                strings(graph.joins.iter().map(|row| row.condition_expr_id.clone())),
            ],
        )?,
    );
    batches.insert(
        "program.aggregate_node".into(),
        batch(
            vec![Field::new("node_id", DataType::Utf8, false)],
            vec![strings(graph.aggregates.iter().cloned())],
        )?,
    );
    batches.insert(
        "program.set_node".into(),
        batch(
            vec![
                Field::new("node_id", DataType::Utf8, false),
                Field::new("set_operation", DataType::Utf8, false),
            ],
            vec![
                strings(graph.sets.iter().map(|row| row.node_id.clone())),
                strings(graph.sets.iter().map(|row| row.set_operation.clone())),
            ],
        )?,
    );
    batches.insert(
        "program.column_expr".into(),
        batch(
            vec![
                Field::new("expr_id", DataType::Utf8, false),
                Field::new("relation_alias", DataType::Utf8, false),
                Field::new("column_name", DataType::Utf8, false),
            ],
            vec![
                strings(graph.columns.iter().map(|row| row.expr_id.clone())),
                strings(graph.columns.iter().map(|row| row.relation_alias.clone())),
                strings(graph.columns.iter().map(|row| row.column_name.clone())),
            ],
        )?,
    );
    batches.insert(
        "program.literal_expr".into(),
        batch(
            vec![
                Field::new("expr_id", DataType::Utf8, false),
                Field::new("logical_type", DataType::Utf8, false),
                Field::new("value", DataType::Utf8, true),
                Field::new("is_null", DataType::Boolean, false),
            ],
            vec![
                strings(graph.literals.iter().map(|row| row.expr_id.clone())),
                strings(graph.literals.iter().map(|row| row.logical_type.clone())),
                Arc::new(StringArray::from(
                    graph
                        .literals
                        .iter()
                        .map(|row| row.value.as_deref())
                        .collect::<Vec<_>>(),
                )),
                Arc::new(BooleanArray::from(
                    graph
                        .literals
                        .iter()
                        .map(|row| row.is_null)
                        .collect::<Vec<_>>(),
                )),
            ],
        )?,
    );
    batches.insert(
        "program.binary_expr".into(),
        batch(
            vec![
                Field::new("expr_id", DataType::Utf8, false),
                Field::new("operator", DataType::Utf8, false),
            ],
            vec![
                strings(graph.binaries.iter().map(|row| row.expr_id.clone())),
                strings(graph.binaries.iter().map(|row| row.operator.clone())),
            ],
        )?,
    );
    batches.insert(
        "program.call_expr".into(),
        batch(
            vec![
                Field::new("expr_id", DataType::Utf8, false),
                Field::new("function_name", DataType::Utf8, false),
            ],
            vec![
                strings(graph.calls.iter().map(|row| row.expr_id.clone())),
                strings(graph.calls.iter().map(|row| row.function_name.clone())),
            ],
        )?,
    );
    batches.insert("program.case_expr".into(), empty_ids("expr_id")?);
    batches.insert(
        "program.cast_expr".into(),
        batch(
            vec![
                Field::new("expr_id", DataType::Utf8, false),
                Field::new("target_type", DataType::Utf8, false),
            ],
            vec![
                strings(graph.casts.iter().map(|row| row.expr_id.clone())),
                strings(graph.casts.iter().map(|row| row.target_type.clone())),
            ],
        )?,
    );
    batches.insert(
        "program.plan_edge".into(),
        batch(
            vec![
                Field::new("parent_node_id", DataType::Utf8, false),
                Field::new("child_node_id", DataType::Utf8, false),
                Field::new("input_ordinal", DataType::UInt16, false),
            ],
            vec![
                strings(
                    graph
                        .plan_edges
                        .iter()
                        .map(|row| row.parent_node_id.clone()),
                ),
                strings(graph.plan_edges.iter().map(|row| row.child_node_id.clone())),
                Arc::new(UInt16Array::from_iter_values(
                    graph.plan_edges.iter().map(|row| row.input_ordinal),
                )),
            ],
        )?,
    );
    batches.insert(
        "program.expression_edge".into(),
        batch(
            vec![
                Field::new("parent_id", DataType::Utf8, false),
                Field::new("child_expr_id", DataType::Utf8, false),
                Field::new("role", DataType::Utf8, false),
                Field::new("operand_ordinal", DataType::UInt16, false),
                Field::new("output_alias", DataType::Utf8, true),
            ],
            vec![
                strings(
                    graph
                        .expression_edges
                        .iter()
                        .map(|row| row.parent_id.clone()),
                ),
                strings(
                    graph
                        .expression_edges
                        .iter()
                        .map(|row| row.child_expr_id.clone()),
                ),
                strings(graph.expression_edges.iter().map(|row| row.role.clone())),
                Arc::new(UInt16Array::from_iter_values(
                    graph.expression_edges.iter().map(|row| row.operand_ordinal),
                )),
                Arc::new(StringArray::from(
                    graph
                        .expression_edges
                        .iter()
                        .map(|row| row.output_alias.as_deref())
                        .collect::<Vec<_>>(),
                )),
            ],
        )?,
    );
    Ok(batches)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use arrow_array::{Array as _, StringArray};

    use super::{
        OntologyExecutionPhase, OntologyProgramOperandContract, OntologyProgramOperandRole,
        OntologyProgramOperationContract, SchemaContractIr, program_graph_batches,
    };

    fn source_ir() -> SchemaContractIr {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/contracts/schema/schema-contract-ir.json"
        )))
        .expect("schema contract IR")
    }

    fn utf8_values(batch: &arrow_array::RecordBatch, column: &str) -> Vec<Option<String>> {
        let values = batch
            .column_by_name(column)
            .and_then(|column| column.as_any().downcast_ref::<StringArray>())
            .expect("Utf8 column");
        (0..values.len())
            .map(|row| (!values.is_null(row)).then(|| values.value(row).to_owned()))
            .collect()
    }

    fn batch_identity(batch: &arrow_array::RecordBatch) -> String {
        let mut bytes = Vec::new();
        {
            let mut writer =
                arrow_ipc::writer::StreamWriter::try_new(&mut bytes, batch.schema().as_ref())
                    .expect("batch writer");
            writer.write(batch).expect("batch rows");
            writer.finish().expect("batch finish");
        }
        format!("b3:{}", blake3::hash(&bytes).to_hex())
    }

    fn operation_kind(operation: &OntologyProgramOperationContract) -> &'static str {
        match operation {
            OntologyProgramOperationContract::Scan { .. } => "SCAN",
            OntologyProgramOperationContract::Filter { .. } => "FILTER",
            OntologyProgramOperationContract::Join { .. } => "JOIN",
            OntologyProgramOperationContract::Aggregate { .. } => "AGGREGATE",
            OntologyProgramOperationContract::Set { .. } => "SET",
            OntologyProgramOperationContract::Column { .. } => "COLUMN",
            OntologyProgramOperationContract::Literal { .. } => "LITERAL",
            OntologyProgramOperationContract::Binary { .. } => "BINARY",
            OntologyProgramOperationContract::Call { .. } => "CALL",
            OntologyProgramOperationContract::Cast { .. } => "CAST",
        }
    }

    fn operation_relation(kind: &str) -> &'static str {
        match kind {
            "SCAN" => "program.scan_node",
            "FILTER" => "program.filter_node",
            "JOIN" => "program.join_node",
            "AGGREGATE" => "program.aggregate_node",
            "SET" => "program.set_node",
            "COLUMN" => "program.column_expr",
            "LITERAL" => "program.literal_expr",
            "BINARY" => "program.binary_expr",
            "CALL" => "program.call_expr",
            "CAST" => "program.cast_expr",
            _ => panic!("unknown operation kind {kind}"),
        }
    }

    fn rename_operation(ir: &mut SchemaContractIr, operation_index: usize) {
        let old = ir.ontology_program_graph.operations[operation_index]
            .operation_id()
            .to_owned();
        let new = format!("{old}.causal-mutant");
        match &mut ir.ontology_program_graph.operations[operation_index] {
            OntologyProgramOperationContract::Scan { operation_id, .. }
            | OntologyProgramOperationContract::Filter { operation_id }
            | OntologyProgramOperationContract::Join { operation_id, .. }
            | OntologyProgramOperationContract::Aggregate { operation_id }
            | OntologyProgramOperationContract::Set { operation_id, .. }
            | OntologyProgramOperationContract::Column { operation_id, .. }
            | OntologyProgramOperationContract::Literal { operation_id, .. }
            | OntologyProgramOperationContract::Binary { operation_id, .. }
            | OntologyProgramOperationContract::Call { operation_id, .. }
            | OntologyProgramOperationContract::Cast { operation_id, .. } => {
                *operation_id = new.clone()
            }
        }
        for program in &mut ir.ontology_program_graph.programs {
            if program.root_operation_id.as_deref() == Some(old.as_str()) {
                program.root_operation_id = Some(new.clone());
            }
        }
        for operand in &mut ir.ontology_program_graph.operands {
            if operand.parent_operation_id == old {
                operand.parent_operation_id.clone_from(&new);
            }
            if operand.child_operation_id == old {
                operand.child_operation_id.clone_from(&new);
            }
        }
    }

    fn mutate_operation(ir: &mut SchemaContractIr, operation_index: usize) {
        if matches!(
            &ir.ontology_program_graph.operations[operation_index],
            OntologyProgramOperationContract::Filter { .. }
                | OntologyProgramOperationContract::Aggregate { .. }
                | OntologyProgramOperationContract::Set { .. }
        ) {
            rename_operation(ir, operation_index);
            return;
        }
        match &mut ir.ontology_program_graph.operations[operation_index] {
            OntologyProgramOperationContract::Scan { relation_alias, .. } => {
                relation_alias.push_str("_causal_mutant");
            }
            OntologyProgramOperationContract::Filter { .. }
            | OntologyProgramOperationContract::Aggregate { .. }
            | OntologyProgramOperationContract::Set { .. } => {
                unreachable!("identity-only operations were handled before the mutable borrow");
            }
            OntologyProgramOperationContract::Join { join_type, .. } => {
                *join_type = if join_type == "inner" {
                    "left"
                } else {
                    "inner"
                }
                .into();
            }
            OntologyProgramOperationContract::Column { column_name, .. } => {
                column_name.push_str("_causal_mutant");
            }
            OntologyProgramOperationContract::Literal {
                logical_type,
                value,
                is_null,
                ..
            } => {
                let replacement = match logical_type.as_str() {
                    "utf8" => format!("{}_causal_mutant", value.as_deref().unwrap_or("null")),
                    "boolean" if value.as_deref() == Some("true") => "false".into(),
                    "boolean" => "true".into(),
                    "int16" | "int32" | "int64" if value.as_deref() == Some("1") => "2".into(),
                    "int16" | "int32" | "int64" => "1".into(),
                    other => panic!("unknown literal type {other}"),
                };
                *value = Some(replacement);
                *is_null = false;
            }
            OntologyProgramOperationContract::Binary { operator, .. } => {
                *operator = match operator.as_str() {
                    "eq" => "neq",
                    "neq" => "eq",
                    "gt" => "gte",
                    "gte" => "gt",
                    "lt" => "lte",
                    "lte" => "lt",
                    "and" => "or",
                    "or" => "and",
                    other => panic!("unknown binary operator {other}"),
                }
                .into();
            }
            OntologyProgramOperationContract::Call { function_name, .. } => {
                *function_name = match function_name.as_str() {
                    "is_null" => "is_not_null",
                    "is_not_null" | "not" | "is_true" | "count" => "is_null",
                    other => panic!("unknown call {other}"),
                }
                .into();
            }
            OntologyProgramOperationContract::Cast { target_type, .. } => {
                *target_type = if target_type == "int64" {
                    "utf8"
                } else {
                    "int64"
                }
                .into();
            }
        }
    }

    fn illegal_replacement_role(role: OntologyProgramOperandRole) -> OntologyProgramOperandRole {
        match role {
            OntologyProgramOperandRole::Input
            | OntologyProgramOperandRole::Predicate
            | OntologyProgramOperandRole::Condition => OntologyProgramOperandRole::Left,
            OntologyProgramOperandRole::Left
            | OntologyProgramOperandRole::Right
            | OntologyProgramOperandRole::Argument
            | OntologyProgramOperandRole::Group
            | OntologyProgramOperandRole::Aggregate => OntologyProgramOperandRole::Predicate,
        }
    }

    fn add_set_coverage_program(ir: &mut SchemaContractIr) {
        let mut rule = ir.ontology_rule_contracts[0].clone();
        rule.rule_id = "ontology.causal-set-coverage.v1".into();
        rule.calculation_id = "calculation.causal-set-coverage.v1".into();
        rule.policy_id = "policy.causal-set-coverage.v1".into();
        rule.diagnostic_code = "PUBLICATION_CAUSAL_SET_COVERAGE".into();
        ir.ontology_rule_contracts.push(rule);
        ir.ontology_program_graph
            .programs
            .push(super::super::OntologyProgramContract {
                program_id: "ontology.causal-set-coverage.v1:1".into(),
                rule_id: "ontology.causal-set-coverage.v1".into(),
                root_operation_id: Some("causal.set".into()),
                execution_phase: OntologyExecutionPhase::CandidateValidation,
                subject_table_id: "source_file".into(),
            });
        ir.ontology_program_graph.operations.extend([
            OntologyProgramOperationContract::Set {
                operation_id: "causal.set".into(),
                set_operation: "union_all".into(),
            },
            OntologyProgramOperationContract::Scan {
                operation_id: "causal.set.scan.0".into(),
                relation_ref: "table:1".into(),
                relation_alias: "causal_set_left".into(),
            },
            OntologyProgramOperationContract::Scan {
                operation_id: "causal.set.scan.1".into(),
                relation_ref: "table:1".into(),
                relation_alias: "causal_set_right".into(),
            },
        ]);
        ir.ontology_program_graph.operands.extend([
            OntologyProgramOperandContract {
                parent_operation_id: "causal.set".into(),
                child_operation_id: "causal.set.scan.0".into(),
                role: OntologyProgramOperandRole::Input,
                ordinal: 0,
                output_alias: None,
            },
            OntologyProgramOperandContract {
                parent_operation_id: "causal.set".into(),
                child_operation_id: "causal.set.scan.1".into(),
                role: OntologyProgramOperandRole::Input,
                ordinal: 1,
                output_alias: None,
            },
        ]);
    }

    #[cfg(feature = "data-fabric")]
    fn add_executable_filter_rule(ir: &mut SchemaContractIr, literal_value: &str) {
        let mut rule = ir.ontology_rule_contracts[0].clone();
        rule.rule_id = "ontology.additive-executable.v1".into();
        rule.calculation_id = "calculation.additive-executable.v1".into();
        rule.policy_id = "policy.additive-executable.v1".into();
        rule.diagnostic_code = "PUBLICATION_ADDITIVE_EXECUTABLE".into();
        ir.ontology_rule_contracts.push(rule);
        ir.ontology_program_graph
            .programs
            .push(super::super::OntologyProgramContract {
                program_id: "ontology.additive-executable.v1:1".into(),
                rule_id: "ontology.additive-executable.v1".into(),
                root_operation_id: Some("additive.filter".into()),
                execution_phase: OntologyExecutionPhase::CandidateValidation,
                subject_table_id: "source_file".into(),
            });
        ir.ontology_program_graph.operations.extend([
            OntologyProgramOperationContract::Filter {
                operation_id: "additive.filter".into(),
            },
            OntologyProgramOperationContract::Scan {
                operation_id: "additive.scan".into(),
                relation_ref: "table:32767".into(),
                relation_alias: "additive_source".into(),
            },
            OntologyProgramOperationContract::Binary {
                operation_id: "additive.eq".into(),
                operator: "eq".into(),
            },
            OntologyProgramOperationContract::Column {
                operation_id: "additive.flag".into(),
                relation_alias: "additive_source".into(),
                column_name: "flag".into(),
            },
            OntologyProgramOperationContract::Literal {
                operation_id: "additive.expected".into(),
                logical_type: "boolean".into(),
                value: Some(literal_value.into()),
                is_null: false,
            },
        ]);
        ir.ontology_program_graph.operands.extend([
            OntologyProgramOperandContract {
                parent_operation_id: "additive.filter".into(),
                child_operation_id: "additive.scan".into(),
                role: OntologyProgramOperandRole::Input,
                ordinal: 0,
                output_alias: None,
            },
            OntologyProgramOperandContract {
                parent_operation_id: "additive.filter".into(),
                child_operation_id: "additive.eq".into(),
                role: OntologyProgramOperandRole::Predicate,
                ordinal: 0,
                output_alias: None,
            },
            OntologyProgramOperandContract {
                parent_operation_id: "additive.eq".into(),
                child_operation_id: "additive.flag".into(),
                role: OntologyProgramOperandRole::Left,
                ordinal: 0,
                output_alias: None,
            },
            OntologyProgramOperandContract {
                parent_operation_id: "additive.eq".into(),
                child_operation_id: "additive.expected".into(),
                role: OntologyProgramOperandRole::Right,
                ordinal: 0,
                output_alias: None,
            },
        ]);
    }

    #[cfg(feature = "data-fabric")]
    fn relational_package(
        batches: BTreeMap<String, arrow_array::RecordBatch>,
    ) -> codefabric::ontology_program::OntologyProgramPackage {
        use codefabric::ontology_program::{
            OntologyProgramManifest, OntologyProgramMember, OntologyProgramPackage,
        };

        let members = batches
            .into_iter()
            .map(|(relation_id, batch)| {
                let member = OntologyProgramMember {
                    relation_id: relation_id.clone(),
                    schema: batch.schema(),
                    batches: vec![batch],
                    ipc_bytes: Vec::new(),
                    member_identity: format!("test:{relation_id}"),
                };
                (relation_id, member)
            })
            .collect::<BTreeMap<_, _>>();
        let member_identities = members
            .iter()
            .map(|(relation, member)| (relation.clone(), member.member_identity.clone()))
            .collect();
        OntologyProgramPackage {
            manifest: OntologyProgramManifest {
                package_version: "source-causality-test.v1".into(),
                bootstrap_schema_identity: "test:bootstrap".into(),
                authored_content_identity: "test:authored".into(),
                logical_program_identity: "test:logical".into(),
                packaging_profile_id: "test:memory".into(),
                member_identities,
                package_identity: "test:package".into(),
            },
            members,
        }
    }

    #[cfg(feature = "data-fabric")]
    async fn execute_rows(
        plan: datafusion::logical_expr::LogicalPlan,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let context = datafusion::prelude::SessionContext::new();
        let batches = context.execute_logical_plan(plan).await?.collect().await?;
        Ok(batches.iter().map(arrow_array::RecordBatch::num_rows).sum())
    }

    #[cfg(feature = "data-fabric")]
    fn schema_faithful_providers()
    -> BTreeMap<String, std::sync::Arc<dyn datafusion::catalog::TableProvider>> {
        let batches = codefabric::schema_registry::table_specs()
            .iter()
            .map(|spec| {
                (
                    spec.table_code,
                    arrow_array::RecordBatch::new_empty(std::sync::Arc::clone(&spec.arrow_schema)),
                )
            })
            .collect::<BTreeMap<_, _>>();
        codefabric::ontology_relational_program::candidate_batch_providers(&batches)
            .expect("schema-faithful source causality providers")
    }

    #[cfg(feature = "data-fabric")]
    fn compiled_candidate_plans(
        program: &codefabric::ontology_relational_program::OntologyRelationalProgram,
        providers: &BTreeMap<String, std::sync::Arc<dyn datafusion::catalog::TableProvider>>,
    ) -> Result<BTreeMap<String, String>, codefabric::ontology_executor::OntologyProgramCompileError>
    {
        program
            .programs()
            .values()
            .filter(|contract| contract.execution_phase == "candidate_validation")
            .map(|contract| {
                Ok((
                    contract.program_id.clone(),
                    program
                        .compile(&contract.program_id, providers)?
                        .display_indent()
                        .to_string(),
                ))
            })
            .collect()
    }

    #[test]
    fn authored_graph_rows_causally_regenerate_plan_diagnostic_and_additive_rule() {
        let mut base = source_ir();
        add_set_coverage_program(&mut base);
        base.validate().expect("base authored graph");
        let base_batches = program_graph_batches(&base).expect("base normalized graph");

        let first_operation_by_kind = base
            .ontology_program_graph
            .operations
            .iter()
            .enumerate()
            .fold(BTreeMap::new(), |mut kinds, (index, operation)| {
                kinds.entry(operation_kind(operation)).or_insert(index);
                kinds
            });
        assert_eq!(
            first_operation_by_kind
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "SCAN",
                "FILTER",
                "JOIN",
                "AGGREGATE",
                "SET",
                "COLUMN",
                "LITERAL",
                "BINARY",
                "CALL",
                "CAST",
            ])
        );
        for (kind, operation_index) in first_operation_by_kind {
            let mut mutant = base.clone();
            mutate_operation(&mut mutant, operation_index);
            mutant
                .validate()
                .unwrap_or_else(|error| panic!("valid {kind} source mutant: {error}"));
            let mutant_batches = program_graph_batches(&mutant)
                .unwrap_or_else(|error| panic!("regenerate {kind} mutant: {error}"));
            let relation = operation_relation(kind);
            assert_ne!(
                batch_identity(&base_batches[relation]),
                batch_identity(&mutant_batches[relation]),
                "{kind} source row did not causally change its normalized relation"
            );
            assert_ne!(
                utf8_values(
                    &base_batches["program.program_contract"],
                    "rule_semantics_identity"
                ),
                utf8_values(
                    &mutant_batches["program.program_contract"],
                    "rule_semantics_identity"
                ),
                "{kind} source row did not causally change rule semantics authority"
            );
        }

        let first_operand_by_role = base
            .ontology_program_graph
            .operands
            .iter()
            .enumerate()
            .fold(BTreeMap::new(), |mut roles, (index, operand)| {
                roles.entry(operand.role).or_insert(index);
                roles
            });
        assert_eq!(
            first_operand_by_role
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                OntologyProgramOperandRole::Input,
                OntologyProgramOperandRole::Predicate,
                OntologyProgramOperandRole::Condition,
                OntologyProgramOperandRole::Left,
                OntologyProgramOperandRole::Right,
                OntologyProgramOperandRole::Argument,
                OntologyProgramOperandRole::Group,
                OntologyProgramOperandRole::Aggregate,
            ])
        );
        for (role, operand_index) in first_operand_by_role {
            let mut mutant = base.clone();
            mutant.ontology_program_graph.operands[operand_index].role =
                illegal_replacement_role(role);
            let error = program_graph_batches(&mutant).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("ontology_program_graph.operands"),
                "{} role mutant did not produce a typed graph diagnostic: {error}",
                role.as_str()
            );
        }

        let mut operand_mutant = base.clone();
        let binary_parent = operand_mutant
            .ontology_program_graph
            .operands
            .iter()
            .find(|operand| operand.role == OntologyProgramOperandRole::Left)
            .expect("left operand")
            .parent_operation_id
            .clone();
        let [left, right] = operand_mutant
            .ontology_program_graph
            .operands
            .iter_mut()
            .filter(|operand| operand.parent_operation_id == binary_parent)
            .filter(|operand| {
                matches!(
                    operand.role,
                    OntologyProgramOperandRole::Left | OntologyProgramOperandRole::Right
                )
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("binary operands");
        std::mem::swap(&mut left.child_operation_id, &mut right.child_operation_id);
        operand_mutant.validate().expect("valid operand mutant");
        let operand_batches = program_graph_batches(&operand_mutant).expect("operand regeneration");
        assert_ne!(
            utf8_values(&base_batches["program.expression_edge"], "child_expr_id"),
            utf8_values(&operand_batches["program.expression_edge"], "child_expr_id")
        );

        let mut diagnostic_mutant = base.clone();
        diagnostic_mutant.ontology_rule_contracts[0].diagnostic_code =
            "PUBLICATION_REFERENTIAL_INTEGRITY_MUTANT".into();
        diagnostic_mutant
            .validate()
            .expect("valid diagnostic mutant");
        let diagnostic_batches =
            program_graph_batches(&diagnostic_mutant).expect("diagnostic regeneration");
        assert_ne!(
            utf8_values(&base_batches["program.program_contract"], "diagnostic_code"),
            utf8_values(
                &diagnostic_batches["program.program_contract"],
                "diagnostic_code"
            )
        );

        let mut additive = base.clone();
        let mut rule = additive.ontology_rule_contracts[0].clone();
        rule.rule_id = "ontology.additive-scan.v1".into();
        rule.calculation_id = "calculation.additive-scan.v1".into();
        rule.policy_id = "policy.additive-scan.v1".into();
        rule.diagnostic_code = "PUBLICATION_ADDITIVE_SCAN".into();
        additive.ontology_rule_contracts.push(rule);
        additive
            .ontology_program_graph
            .programs
            .push(super::super::OntologyProgramContract {
                program_id: "ontology.additive-scan.v1:1".into(),
                rule_id: "ontology.additive-scan.v1".into(),
                root_operation_id: Some("additive.scan".into()),
                execution_phase: OntologyExecutionPhase::CandidateValidation,
                subject_table_id: "source_file".into(),
            });
        additive
            .ontology_program_graph
            .operations
            .push(OntologyProgramOperationContract::Scan {
                operation_id: "additive.scan".into(),
                relation_ref: "table:1".into(),
                relation_alias: "additive_source".into(),
            });
        additive.validate().expect("data-only additive rule");
        let additive_batches = program_graph_batches(&additive).expect("additive regeneration");
        assert_eq!(
            additive_batches["program.program_contract"].num_rows(),
            base_batches["program.program_contract"].num_rows() + 1
        );
    }

    #[cfg(feature = "data-fabric")]
    #[tokio::test]
    async fn authored_additive_rule_decodes_lowers_executes_and_changes_result() {
        use std::sync::Arc;

        use arrow_array::{ArrayRef, BooleanArray, RecordBatch};
        use arrow_schema::{DataType, Field, Schema};
        use codefabric::ontology_relational_program::OntologyRelationalProgram;
        use datafusion::catalog::TableProvider;
        use datafusion::datasource::MemTable;

        let schema = Arc::new(Schema::new(vec![Field::new(
            "flag",
            DataType::Boolean,
            false,
        )]));
        let values: ArrayRef = Arc::new(BooleanArray::from(vec![true, false, false]));
        let candidate =
            RecordBatch::try_new(Arc::clone(&schema), vec![values]).expect("candidate batch");
        let provider: Arc<dyn TableProvider> =
            Arc::new(MemTable::try_new(schema, vec![vec![candidate]]).expect("candidate provider"));
        let providers = BTreeMap::from([("table:32767".to_owned(), provider)]);

        let mut authored_true = source_ir();
        add_executable_filter_rule(&mut authored_true, "true");
        authored_true.validate().expect("valid additive true rule");
        let generated_true = program_graph_batches(&authored_true).expect("regenerate true rule");
        let decoded_true = OntologyRelationalProgram::decode(&relational_package(generated_true))
            .expect("decode additive true rule");
        assert!(
            decoded_true
                .programs()
                .contains_key("ontology.additive-executable.v1:1"),
            "newly authored rule was not decoded without a Rust dispatch edit"
        );
        let true_plan = decoded_true
            .compile("ontology.additive-executable.v1:1", &providers)
            .expect("lower additive true rule");
        let true_plan_text = true_plan.display_indent().to_string();
        assert_eq!(execute_rows(true_plan).await.expect("execute true rule"), 1);

        let mut authored_false = source_ir();
        add_executable_filter_rule(&mut authored_false, "false");
        authored_false
            .validate()
            .expect("valid additive false rule");
        let generated_false =
            program_graph_batches(&authored_false).expect("regenerate false rule");
        let decoded_false = OntologyRelationalProgram::decode(&relational_package(generated_false))
            .expect("decode additive false rule");
        let false_plan = decoded_false
            .compile("ontology.additive-executable.v1:1", &providers)
            .expect("lower additive false rule");
        assert_ne!(
            true_plan_text,
            false_plan.display_indent().to_string(),
            "source literal mutation did not change the lowered DataFusion plan"
        );
        assert_eq!(
            execute_rows(false_plan).await.expect("execute false rule"),
            2,
            "source literal mutation did not change the semantic result"
        );
    }

    #[cfg(feature = "data-fabric")]
    #[test]
    fn authored_operation_matrix_changes_runtime_plan_or_typed_rejection() {
        use codefabric::ontology_relational_program::OntologyRelationalProgram;

        let mut base = source_ir();
        add_set_coverage_program(&mut base);
        base.validate().expect("operation-matrix source graph");
        let providers = schema_faithful_providers();
        let base_package = relational_package(
            program_graph_batches(&base).expect("operation-matrix base regeneration"),
        );
        let base_program =
            OntologyRelationalProgram::decode(&base_package).expect("operation-matrix base decode");
        let base_plans =
            compiled_candidate_plans(&base_program, &providers).expect("operation-matrix plans");

        let first_operation_by_kind = base
            .ontology_program_graph
            .operations
            .iter()
            .enumerate()
            .fold(BTreeMap::new(), |mut kinds, (index, operation)| {
                kinds.entry(operation_kind(operation)).or_insert(index);
                kinds
            });
        assert_eq!(first_operation_by_kind.len(), 10);
        let mut changed_plans = 0;
        let mut typed_rejections = 0;
        for (kind, operation_index) in first_operation_by_kind {
            let mut mutant = base.clone();
            mutate_operation(&mut mutant, operation_index);
            mutant
                .validate()
                .unwrap_or_else(|error| panic!("valid {kind} runtime source mutant: {error}"));
            let package = relational_package(
                program_graph_batches(&mutant)
                    .unwrap_or_else(|error| panic!("regenerate {kind} runtime mutant: {error}")),
            );
            let runtime_result = OntologyRelationalProgram::decode(&package)
                .and_then(|program| compiled_candidate_plans(&program, &providers));
            match runtime_result {
                Ok(mutant_plans) => {
                    assert_ne!(
                        base_plans, mutant_plans,
                        "{kind} source mutation did not change a lowered DataFusion plan"
                    );
                    changed_plans += 1;
                }
                Err(error) => {
                    assert!(
                        error.to_string().starts_with("ONTOLOGY_PROGRAM_"),
                        "{kind} source mutation did not fail with a typed program diagnostic: {error}"
                    );
                    typed_rejections += 1;
                }
            }
        }
        assert_eq!(changed_plans + typed_rejections, 10);
        assert!(changed_plans > 0);
    }
}
