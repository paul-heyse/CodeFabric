//! Normalizes authored schema/rule contracts into typed relational-program batches.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, RecordBatch, StringArray, UInt16Array};
use arrow_schema::{DataType, Field, Schema};

use super::{SchemaContractIr, SchemaDriverError, SemanticAuthority, ontology_artifact_error};

#[derive(Clone, Debug)]
struct ProgramRow {
    program_id: String,
    rule_id: String,
    root_node_id: String,
    execution_phase: &'static str,
    calculation_id: String,
    policy_id: String,
    expected_result_contract: String,
    diagnostic_code: String,
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
    join_type: &'static str,
    condition_expr_id: String,
}

#[derive(Clone, Debug)]
struct SetRow {
    node_id: String,
    set_operation: &'static str,
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
    logical_type: &'static str,
    value: Option<String>,
    is_null: bool,
}

#[derive(Clone, Debug)]
struct BinaryRow {
    expr_id: String,
    operator: &'static str,
}

#[derive(Clone, Debug)]
struct CallRow {
    expr_id: String,
    function_name: &'static str,
}

#[derive(Clone, Debug)]
struct CastRow {
    expr_id: String,
    target_type: &'static str,
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
    role: &'static str,
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
        let id = format!("{prefix}:{:05}", self.serial);
        self.serial += 1;
        id
    }

    fn scan(&mut self, table_code: i16, alias: &str) -> String {
        let node_id = self.id("plan.scan");
        self.scans.push(ScanRow {
            node_id: node_id.clone(),
            relation_ref: format!("table:{table_code}"),
            relation_alias: alias.to_owned(),
        });
        node_id
    }

    fn column(&mut self, alias: &str, name: &str) -> String {
        let expr_id = self.id("expr.column");
        self.columns.push(ColumnRow {
            expr_id: expr_id.clone(),
            relation_alias: alias.to_owned(),
            column_name: name.to_owned(),
        });
        expr_id
    }

    fn literal(&mut self, logical_type: &'static str, value: impl Into<String>) -> String {
        let expr_id = self.id("expr.literal");
        self.literals.push(LiteralRow {
            expr_id: expr_id.clone(),
            logical_type,
            value: Some(value.into()),
            is_null: false,
        });
        expr_id
    }

    fn binary(&mut self, operator: &'static str, left: String, right: String) -> String {
        let expr_id = self.id("expr.binary");
        self.binaries.push(BinaryRow {
            expr_id: expr_id.clone(),
            operator,
        });
        self.expression_edge(&expr_id, left, "left", 0, None);
        self.expression_edge(&expr_id, right, "right", 0, None);
        expr_id
    }

    fn call(&mut self, function_name: &'static str, arguments: Vec<String>) -> String {
        let expr_id = self.id("expr.call");
        self.calls.push(CallRow {
            expr_id: expr_id.clone(),
            function_name,
        });
        for (ordinal, argument) in arguments.into_iter().enumerate() {
            self.expression_edge(
                &expr_id,
                argument,
                "argument",
                u16::try_from(ordinal).expect("program expression ordinal"),
                None,
            );
        }
        expr_id
    }

    fn cast(&mut self, argument: String, target_type: &'static str) -> String {
        let expr_id = self.id("expr.cast");
        self.casts.push(CastRow {
            expr_id: expr_id.clone(),
            target_type,
        });
        self.expression_edge(&expr_id, argument, "argument", 0, None);
        expr_id
    }

    fn filter(&mut self, child: String, predicate: String) -> String {
        let node_id = self.id("plan.filter");
        self.filters.push(FilterRow {
            node_id: node_id.clone(),
            predicate_expr_id: predicate,
        });
        self.plan_edge(&node_id, child, 0);
        node_id
    }

    fn join(
        &mut self,
        left: String,
        right: String,
        join_type: &'static str,
        condition: String,
    ) -> String {
        let node_id = self.id("plan.join");
        self.joins.push(JoinRow {
            node_id: node_id.clone(),
            join_type,
            condition_expr_id: condition,
        });
        self.plan_edge(&node_id, left, 0);
        self.plan_edge(&node_id, right, 1);
        node_id
    }

    fn aggregate(
        &mut self,
        child: String,
        groups: Vec<(String, String)>,
        aggregates: Vec<(String, String)>,
    ) -> String {
        let node_id = self.id("plan.aggregate");
        self.aggregates.push(node_id.clone());
        self.plan_edge(&node_id, child, 0);
        for (ordinal, (expression, alias)) in groups.into_iter().enumerate() {
            self.expression_edge(
                &node_id,
                expression,
                "group",
                u16::try_from(ordinal).expect("group ordinal"),
                Some(alias),
            );
        }
        for (ordinal, (expression, alias)) in aggregates.into_iter().enumerate() {
            self.expression_edge(
                &node_id,
                expression,
                "aggregate",
                u16::try_from(ordinal).expect("aggregate ordinal"),
                Some(alias),
            );
        }
        node_id
    }

    fn violation_projection(
        &mut self,
        child: String,
        program_id: &str,
        rule_id: &str,
        diagnostic_code: &str,
        subject_table_id: &str,
    ) -> String {
        let node_id = self.id("plan.project");
        self.projects.push(node_id.clone());
        self.plan_edge(&node_id, child, 0);
        for (ordinal, (alias, value)) in [
            ("program_id", program_id),
            ("rule_id", rule_id),
            ("diagnostic_code", diagnostic_code),
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

    fn plan_edge(&mut self, parent: &str, child: String, ordinal: u16) {
        self.plan_edges.push(PlanEdgeRow {
            parent_node_id: parent.to_owned(),
            child_node_id: child,
            input_ordinal: ordinal,
        });
    }

    fn expression_edge(
        &mut self,
        parent: &str,
        child: String,
        role: &'static str,
        ordinal: u16,
        output_alias: Option<String>,
    ) {
        self.expression_edges.push(ExpressionEdgeRow {
            parent_id: parent.to_owned(),
            child_expr_id: child,
            role,
            operand_ordinal: ordinal,
            output_alias,
        });
    }

    fn register_program(
        &mut self,
        rule: &super::OntologyRuleContract,
        suffix: &str,
        root: String,
        subject_table: &str,
    ) -> String {
        let program_id = format!("{}:{suffix}", rule.rule_id);
        let projected = self.violation_projection(
            root,
            &program_id,
            &rule.rule_id,
            &rule.diagnostic_code,
            subject_table,
        );
        self.programs.push(ProgramRow {
            program_id: program_id.clone(),
            rule_id: rule.rule_id.clone(),
            root_node_id: projected.clone(),
            execution_phase: "candidate_validation",
            calculation_id: rule.calculation_id.clone(),
            policy_id: rule.policy_id.clone(),
            expected_result_contract: rule.output_contract.clone(),
            diagnostic_code: rule.diagnostic_code.clone(),
        });
        projected
    }
}

macro_rules! graph_binary {
    ($graph:expr, $operator:expr, $left:expr, $right:expr $(,)?) => {{
        let left = $left;
        let right = $right;
        ($graph).binary($operator, left, right)
    }};
}

macro_rules! graph_call {
    ($graph:expr, $function:expr, $arguments:expr $(,)?) => {{
        let arguments = $arguments;
        ($graph).call($function, arguments)
    }};
}

macro_rules! graph_cast {
    ($graph:expr, $argument:expr, $target:expr $(,)?) => {{
        let argument = $argument;
        ($graph).cast(argument, $target)
    }};
}

fn and_all(graph: &mut Graph, mut expressions: Vec<String>) -> String {
    let first = expressions.remove(0);
    expressions.into_iter().fold(first, |left, right| {
        graph_binary!(graph, "and", left, right)
    })
}

fn or_all(graph: &mut Graph, mut expressions: Vec<String>) -> String {
    let first = expressions.remove(0);
    expressions
        .into_iter()
        .fold(first, |left, right| graph_binary!(graph, "or", left, right))
}

fn rule<'a>(ir: &'a SchemaContractIr, id: &str) -> &'a super::OntologyRuleContract {
    ir.ontology_rule_contracts
        .iter()
        .find(|rule| rule.rule_id == id)
        .expect("validated ontology rule census")
}

fn table_code(ir: &SchemaContractIr, name: &str) -> i16 {
    ir.tables
        .iter()
        .find(|table| table.name == name)
        .map(|table| table.table_code)
        .expect("validated table reference")
}

fn governed_dimension<'a>(
    ir: &'a SchemaContractIr,
    semantic_type: &str,
) -> Option<(i16, &'static str, Option<&'a str>)> {
    let binding = ir
        .semantic_type_bindings
        .iter()
        .find(|binding| binding.semantic_type == semantic_type)?;
    match binding.authority {
        SemanticAuthority::EnumRegistry => Some((11, "code", binding.domain.as_deref())),
        SemanticAuthority::OntologyEntityRegistry => Some((
            if binding.domain.as_deref() == Some("ENTITY_FAMILY") {
                13
            } else {
                12
            },
            "code",
            None,
        )),
        SemanticAuthority::OntologyRelationRegistry => Some((
            if binding.domain.as_deref() == Some("RELATION_FAMILY") {
                15
            } else {
                14
            },
            "code",
            None,
        )),
        SemanticAuthority::OntologyPropertyRegistry => Some((16, "code", None)),
        SemanticAuthority::OntologyFactRegistry => Some((17, "code", None)),
        _ => None,
    }
}

fn relation_membership_program(
    graph: &mut Graph,
    ir: &SchemaContractIr,
    side: &str,
    entity_id_column: &str,
    entity_family_column: &str,
    predicate: &str,
) -> String {
    let relation_code = table_code(ir, "relation");
    let entity_code = table_code(ir, "entity");
    let edge_code = table_code(ir, "ontology_edge");
    let term_code = table_code(ir, "ontology_term");

    let relations = graph.scan(relation_code, &format!("membership_{side}_relation"));
    let entities = graph.scan(entity_code, &format!("membership_{side}_entity"));
    let relation_entity = graph_binary!(
        graph,
        "eq",
        graph.column(&format!("membership_{side}_relation"), entity_id_column),
        graph.column(&format!("membership_{side}_entity"), "entity_id"),
    );
    let candidates = graph.join(relations, entities, "inner", relation_entity);

    let edges = graph.scan(edge_code, &format!("membership_{side}_edge"));
    let edge_predicate = graph_binary!(
        graph,
        "eq",
        graph.column(&format!("membership_{side}_edge"), "predicate_term_id"),
        graph.literal("utf8", predicate),
    );
    let edges = graph.filter(edges, edge_predicate);
    let relation_terms = graph.scan(term_code, &format!("membership_{side}_relation_term"));
    let relation_term_predicate = graph_binary!(
        graph,
        "eq",
        graph.column(&format!("membership_{side}_relation_term"), "semantic_type",),
        graph.literal("utf8", "ontology:relation-kind"),
    );
    let relation_terms = graph.filter(relation_terms, relation_term_predicate);
    let edge_to_relation_term = graph_binary!(
        graph,
        "eq",
        graph.column(&format!("membership_{side}_edge"), "subject_term_id"),
        graph.column(&format!("membership_{side}_relation_term"), "term_id",),
    );
    let allowed = graph.join(edges, relation_terms, "inner", edge_to_relation_term);
    let family_terms = graph.scan(term_code, &format!("membership_{side}_family_term"));
    let family_term_predicate = graph_binary!(
        graph,
        "eq",
        graph.column(&format!("membership_{side}_family_term"), "semantic_type",),
        graph.literal("utf8", "ontology:entity-family"),
    );
    let family_terms = graph.filter(family_terms, family_term_predicate);
    let edge_to_family_term = graph_binary!(
        graph,
        "eq",
        graph.column(&format!("membership_{side}_edge"), "object_term_id"),
        graph.column(&format!("membership_{side}_family_term"), "term_id"),
    );
    let allowed = graph.join(allowed, family_terms, "inner", edge_to_family_term);

    let relation_kind_match = graph_binary!(
        graph,
        "eq",
        graph_cast!(
            graph,
            graph.column(&format!("membership_{side}_relation"), "relation_kind_code"),
            "int64",
        ),
        graph.column(&format!("membership_{side}_relation_term"), "code_int64",),
    );
    let family_match = graph_binary!(
        graph,
        "eq",
        graph_cast!(
            graph,
            graph.column(&format!("membership_{side}_entity"), entity_family_column,),
            "int64",
        ),
        graph.column(&format!("membership_{side}_family_term"), "code_int64",),
    );
    let condition = graph_binary!(graph, "and", relation_kind_match, family_match);
    graph.join(candidates, allowed, "left_anti", condition)
}

fn build_graph(ir: &SchemaContractIr) -> Graph {
    let mut graph = Graph::default();
    let mut roots = Vec::new();

    let foreign_key_rule = rule(ir, "ontology.fk.v1");
    for source in &ir.tables {
        if source.publication_pin_role != super::PublicationPinRole::PinnedData {
            continue;
        }
        for column in &source.columns {
            let Some(target) = column.foreign_key.as_deref() else {
                continue;
            };
            let (target_table, target_column) = target
                .split_once('.')
                .expect("validated foreign-key contract");
            let target_code = table_code(ir, target_table);
            let suffix = format!("{}:{}", source.table_code, column.name);
            let left_alias = format!("fk_source_{}_{}", source.table_code, column.name);
            let right_alias = format!("fk_target_{target_code}_{target_column}");
            let source_scan = graph.scan(source.table_code, &left_alias);
            let not_null = graph_call!(
                graph,
                "is_not_null",
                vec![graph.column(&left_alias, &column.name)],
            );
            let source_scan = graph.filter(source_scan, not_null);
            let target_scan = graph.scan(target_code, &right_alias);
            let condition = graph_binary!(
                graph,
                "eq",
                graph.column(&left_alias, &column.name),
                graph.column(&right_alias, target_column),
            );
            let invalid = graph.join(source_scan, target_scan, "left_anti", condition);
            roots.push(graph.register_program(foreign_key_rule, &suffix, invalid, &source.name));
        }
    }

    let primary_key_rule = rule(ir, "ontology.primary-key.v1");
    for table in ir
        .tables
        .iter()
        .filter(|table| table.publication_pin_role == super::PublicationPinRole::PinnedData)
    {
        let alias = format!("primary_key_{}", table.table_code);
        let source = graph.scan(table.table_code, &alias);
        let groups = table
            .primary_key
            .iter()
            .map(|column| (graph.column(&alias, column), column.clone()))
            .collect();
        let one = graph.literal("int64", "1");
        let count = graph_call!(graph, "count", vec![one]);
        let aggregate = graph.aggregate(source, groups, vec![(count, "row_count".into())]);
        let predicate = graph_binary!(
            graph,
            "gt",
            graph.column("", "row_count"),
            graph.literal("int64", "1"),
        );
        let invalid = graph.filter(aggregate, predicate);
        roots.push(graph.register_program(
            primary_key_rule,
            &table.table_code.to_string(),
            invalid,
            &table.name,
        ));
    }

    let governed_code_rule = rule(ir, "ontology.governed-code.v1");
    for source in &ir.tables {
        if source.publication_pin_role != super::PublicationPinRole::PinnedData {
            continue;
        }
        for column in &source.columns {
            let Some(semantic_type) = column.semantic_type.as_deref() else {
                continue;
            };
            let Some((dimension_code, dimension_column, domain)) =
                governed_dimension(ir, semantic_type)
            else {
                continue;
            };
            let suffix = format!("{}:{}", source.table_code, column.name);
            let left_alias = format!("code_source_{}_{}", source.table_code, column.name);
            let right_alias = format!("code_dimension_{dimension_code}_{}", column.name);
            let source_scan = graph.scan(source.table_code, &left_alias);
            let not_null = graph_call!(
                graph,
                "is_not_null",
                vec![graph.column(&left_alias, &column.name)],
            );
            let source_scan = graph.filter(source_scan, not_null);
            let mut target_scan = graph.scan(dimension_code, &right_alias);
            if let Some(domain) = domain {
                let domain_predicate = graph_binary!(
                    graph,
                    "eq",
                    graph.column(&right_alias, "domain"),
                    graph.literal("utf8", domain),
                );
                target_scan = graph.filter(target_scan, domain_predicate);
            }
            let left = graph_cast!(graph, graph.column(&left_alias, &column.name), "int64");
            let right = graph_cast!(graph, graph.column(&right_alias, dimension_column), "int64");
            let condition = graph_binary!(graph, "eq", left, right);
            let invalid = graph.join(source_scan, target_scan, "left_anti", condition);
            roots.push(graph.register_program(governed_code_rule, &suffix, invalid, &source.name));
        }
    }

    let membership_rule = rule(ir, "ontology.membership.v1");
    let edge_code = table_code(ir, "ontology_edge");
    let term_code = table_code(ir, "ontology_term");
    for endpoint in ["subject_term_id", "predicate_term_id", "object_term_id"] {
        let edge_alias = format!("membership_edge_{endpoint}");
        let term_alias = format!("membership_term_{endpoint}");
        let edges = graph.scan(edge_code, &edge_alias);
        let terms = graph.scan(term_code, &term_alias);
        let condition = graph_binary!(
            graph,
            "eq",
            graph.column(&edge_alias, endpoint),
            graph.column(&term_alias, "term_id"),
        );
        let invalid = graph.join(edges, terms, "left_anti", condition);
        roots.push(graph.register_program(membership_rule, endpoint, invalid, "ontology_edge"));
    }
    for (side, id, family, predicate) in [
        (
            "source",
            "source_id",
            "entity_family_code",
            "allows_subject_family",
        ),
        (
            "target",
            "target_id",
            "entity_family_code",
            "allows_object_family",
        ),
    ] {
        let invalid = relation_membership_program(&mut graph, ir, side, id, family, predicate);
        roots.push(graph.register_program(membership_rule, side, invalid, "relation"));
    }

    let relation_code = table_code(ir, "relation");
    let relation_kind_code = table_code(ir, "relation_kind");
    let entity_code = table_code(ir, "entity");

    let family_rule = rule(ir, "ontology.relation-family.v1");
    let relations = graph.scan(relation_code, "family_relation");
    let kinds = graph.scan(relation_kind_code, "family_kind");
    let kind_match = graph_binary!(
        graph,
        "eq",
        graph.column("family_relation", "relation_kind_code"),
        graph.column("family_kind", "code"),
    );
    let joined = graph.join(relations, kinds, "inner", kind_match);
    let mismatch = graph_binary!(
        graph,
        "neq",
        graph.column("family_relation", "relation_family_code"),
        graph.column("family_kind", "family_code"),
    );
    let invalid = graph.filter(joined, mismatch);
    roots.push(graph.register_program(family_rule, "relation", invalid, "relation"));

    let cardinality_rule = rule(ir, "ontology.relation-cardinality.v1");
    for (suffix, cardinalities, group_column) in [
        ("source", vec!["many-to-one", "one-to-one"], "source_id"),
        (
            "target",
            vec!["one-to-many", "one-to-many-ordered"],
            "target_id",
        ),
        ("one-to-one-target", vec!["one-to-one"], "target_id"),
    ] {
        let relation_alias = format!("cardinality_relation_{suffix}");
        let kind_alias = format!("cardinality_kind_{suffix}");
        let relations = graph.scan(relation_code, &relation_alias);
        let kinds = graph.scan(relation_kind_code, &kind_alias);
        let choices = cardinalities
            .into_iter()
            .map(|value| {
                graph_binary!(
                    graph,
                    "eq",
                    graph.column(&kind_alias, "cardinality"),
                    graph.literal("utf8", value),
                )
            })
            .collect::<Vec<_>>();
        let cardinality_predicate = or_all(&mut graph, choices);
        let kinds = graph.filter(kinds, cardinality_predicate);
        let condition = graph_binary!(
            graph,
            "eq",
            graph.column(&relation_alias, "relation_kind_code"),
            graph.column(&kind_alias, "code"),
        );
        let joined = graph.join(relations, kinds, "inner", condition);
        let groups = vec![
            (
                graph.column(&relation_alias, "relation_kind_code"),
                "relation_kind_code".into(),
            ),
            (
                graph.column(&relation_alias, group_column),
                "group_identity".into(),
            ),
        ];
        let one = graph.literal("int64", "1");
        let count = graph_call!(graph, "count", vec![one]);
        let aggregate = graph.aggregate(joined, groups, vec![(count, "relation_count".into())]);
        let predicate = graph_binary!(
            graph,
            "gt",
            graph.column("", "relation_count"),
            graph.literal("int64", "1"),
        );
        let invalid = graph.filter(aggregate, predicate);
        roots.push(graph.register_program(cardinality_rule, suffix, invalid, "relation"));
    }

    let owner_rule = rule(ir, "ontology.relation-owner.v1");
    let relations = graph.scan(relation_code, "owner_relation");
    let entities = graph.scan(entity_code, "owner_source_entity");
    let source_match = graph_binary!(
        graph,
        "eq",
        graph.column("owner_relation", "source_id"),
        graph.column("owner_source_entity", "entity_id"),
    );
    let joined = graph.join(relations, entities, "inner", source_match);
    let mismatch = graph_binary!(
        graph,
        "neq",
        graph.column("owner_relation", "owner_id"),
        graph.column("owner_source_entity", "owner_id"),
    );
    let invalid = graph.filter(joined, mismatch);
    roots.push(graph.register_program(owner_rule, "source-owner", invalid, "relation"));

    let self_edge_rule = rule(ir, "ontology.relation-self-edge.v1");
    let relations = graph.scan(relation_code, "self_edge_relation");
    let is_self = graph_binary!(
        graph,
        "eq",
        graph.column("self_edge_relation", "source_id"),
        graph.column("self_edge_relation", "target_id"),
    );
    let relations = graph.filter(relations, is_self);
    let kinds = graph.scan(relation_kind_code, "self_edge_kind");
    let forbidden = graph_binary!(
        graph,
        "eq",
        graph.column("self_edge_kind", "self_edge_policy"),
        graph.literal("utf8", "forbidden"),
    );
    let kinds = graph.filter(kinds, forbidden);
    let condition = graph_binary!(
        graph,
        "eq",
        graph.column("self_edge_relation", "relation_kind_code"),
        graph.column("self_edge_kind", "code"),
    );
    let invalid = graph.join(relations, kinds, "inner", condition);
    roots.push(graph.register_program(self_edge_rule, "relation", invalid, "relation"));

    let property_rule = rule(ir, "ontology.property-one-of.v1");
    let property_code = table_code(ir, "property_fact");
    let properties = graph.scan(property_code, "property_one_of");
    let value_columns = [
        "value_entity_id",
        "value_bool",
        "value_int64",
        "value_float64",
        "value_text",
        "value_bytes",
        "value_type_id",
    ];
    let none = value_columns
        .iter()
        .map(|name| {
            graph_call!(
                graph,
                "is_null",
                vec![graph.column("property_one_of", name)]
            )
        })
        .collect::<Vec<_>>();
    let mut pairs = Vec::new();
    for (index, left) in value_columns.iter().enumerate() {
        for right in &value_columns[index + 1..] {
            let left = graph_call!(
                graph,
                "is_not_null",
                vec![graph.column("property_one_of", left)],
            );
            let right = graph_call!(
                graph,
                "is_not_null",
                vec![graph.column("property_one_of", right)],
            );
            pairs.push(graph_binary!(graph, "and", left, right));
        }
    }
    let mut tag_mismatches = Vec::new();
    for (index, name) in value_columns.iter().enumerate() {
        let code = ((index + 1) * 10).to_string();
        let equal = graph_binary!(
            graph,
            "eq",
            graph.column("property_one_of", "value_kind_code"),
            graph.literal("int16", &code),
        );
        let missing = graph_call!(
            graph,
            "is_null",
            vec![graph.column("property_one_of", name)]
        );
        let first = graph_binary!(graph, "and", equal, missing);
        let unequal = graph_binary!(
            graph,
            "neq",
            graph.column("property_one_of", "value_kind_code"),
            graph.literal("int16", &code),
        );
        let present = graph_call!(
            graph,
            "is_not_null",
            vec![graph.column("property_one_of", name)],
        );
        let second = graph_binary!(graph, "and", unequal, present);
        tag_mismatches.push(graph_binary!(graph, "or", first, second));
    }
    let none = and_all(&mut graph, none);
    let multiple = or_all(&mut graph, pairs);
    let tag_mismatch = or_all(&mut graph, tag_mismatches);
    let malformed = graph_binary!(graph, "or", none, multiple);
    let malformed = graph_binary!(graph, "or", malformed, tag_mismatch);
    let invalid = graph.filter(properties, malformed);
    roots.push(graph.register_program(property_rule, "property-fact", invalid, "property_fact"));

    let span_rule = rule(ir, "ontology.source-span.v1");
    for table in ir
        .tables
        .iter()
        .filter(|table| table.publication_pin_role == super::PublicationPinRole::PinnedData)
    {
        if !table
            .columns
            .iter()
            .any(|column| column.name == "start_byte")
            || !table.columns.iter().any(|column| column.name == "end_byte")
        {
            continue;
        }
        let alias = format!("span_{}", table.table_code);
        let source = graph.scan(table.table_code, &alias);
        let start_null = graph_call!(graph, "is_null", vec![graph.column(&alias, "start_byte")]);
        let end_null = graph_call!(graph, "is_null", vec![graph.column(&alias, "end_byte")]);
        let mismatched_null = graph_binary!(graph, "neq", start_null, end_null);
        let negative = graph_binary!(
            graph,
            "lt",
            graph.column(&alias, "start_byte"),
            graph.literal("int64", "0"),
        );
        let reversed = graph_binary!(
            graph,
            "lt",
            graph.column(&alias, "end_byte"),
            graph.column(&alias, "start_byte"),
        );
        let invalid_expr = graph_binary!(graph, "or", mismatched_null, negative);
        let invalid_expr = graph_binary!(graph, "or", invalid_expr, reversed);
        let invalid = graph.filter(source, invalid_expr);
        roots.push(graph.register_program(
            span_rule,
            &table.table_code.to_string(),
            invalid,
            &table.name,
        ));
    }

    let id_rule = rule(ir, "ontology.id-domain.v1");
    graph.programs.push(ProgramRow {
        program_id: "ontology.id-domain.v1:analyzer".into(),
        rule_id: id_rule.rule_id.clone(),
        root_node_id: String::new(),
        execution_phase: "semantic_analysis",
        calculation_id: id_rule.calculation_id.clone(),
        policy_id: id_rule.policy_id.clone(),
        expected_result_contract: id_rule.output_contract.clone(),
        diagnostic_code: id_rule.diagnostic_code.clone(),
    });

    graph
}

fn batch(fields: Vec<Field>, columns: Vec<ArrayRef>) -> Result<RecordBatch, SchemaDriverError> {
    RecordBatch::try_new(Arc::new(Schema::new(fields)), columns)
        .map_err(|error| ontology_artifact_error(error.to_string()))
}

fn strings(values: impl IntoIterator<Item = String>) -> ArrayRef {
    Arc::new(StringArray::from_iter_values(values))
}

fn static_strings<'a>(values: impl IntoIterator<Item = &'a str>) -> ArrayRef {
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
    let graph = build_graph(ir);
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
            ],
            vec![
                strings(graph.programs.iter().map(|row| row.program_id.clone())),
                strings(graph.programs.iter().map(|row| row.rule_id.clone())),
                strings(graph.programs.iter().map(|row| row.root_node_id.clone())),
                static_strings(graph.programs.iter().map(|row| row.execution_phase)),
                strings(graph.programs.iter().map(|row| row.calculation_id.clone())),
                strings(graph.programs.iter().map(|row| row.policy_id.clone())),
                strings(
                    graph
                        .programs
                        .iter()
                        .map(|row| row.expected_result_contract.clone()),
                ),
                strings(graph.programs.iter().map(|row| row.diagnostic_code.clone())),
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
                static_strings(graph.joins.iter().map(|row| row.join_type)),
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
                static_strings(graph.sets.iter().map(|row| row.set_operation)),
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
                static_strings(graph.literals.iter().map(|row| row.logical_type)),
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
                static_strings(graph.binaries.iter().map(|row| row.operator)),
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
                static_strings(graph.calls.iter().map(|row| row.function_name)),
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
                static_strings(graph.casts.iter().map(|row| row.target_type)),
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
                static_strings(graph.expression_edges.iter().map(|row| row.role)),
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
