//! Artifact-bound WP38 execution for the eight released query forms and child authorization.
//!
//! The WP33 JSONL is read at compile time and decoded with the production strict JSON boundary.
//! Every causal/rejection case is applied to the declared input role and JSON pointer before the
//! resulting typed Arrow relations enter the real relational compiler and authorized child.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arrow_array::{ArrayRef, Int64Array, RecordBatch, StringArray, UInt32Array};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use datafusion::common::TableReference;
use datafusion::datasource::MemTable;
use datafusion::prelude::SessionContext;
use serde_json::{Map, Value, json};

use super::*;
use crate::contracts::jcs;
use crate::fabric::child_session::resource_governance::{
    EpochResourceCoordinator, EpochResourcePolicy, test_lifecycle_work_class_policies,
};
use crate::fabric::child_session::{
    AuthorizedChildSession, ChildRegistryAllowlist, ChildResourceLimits, ChildSessionError,
    ChildSessionPins, ChildSessionPolicy, ChildTableGrant, ChildTableScan,
};
use crate::fabric::epoch_runtime::{
    FABRIC_CATALOG, FabricEpochId, FabricEpochRuntimeConfig, FabricSchemaRole,
};
use crate::fabric::explicit_unknown::{
    ExplicitUnknownFactInput, UnknownCoverageState, materialize_explicit_unknown_fact,
};
use crate::fabric::graph_program::{
    GraphRelationInput, GraphResourceBounds, OrderedPathBounds, OrderedPathEdge,
    PathResultSlotIdentityInput, ReachabilityBindings, bounded_shortest_path_witness,
    compile_bounded_reachability, issue_path_result_slot_identity,
    validate_bounded_ordered_path_policy,
};
use crate::fabric::programmatic_epoch::{ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder};
use crate::fabric::programmatic_ingress_port::ApplicationOwnedSemanticIngressPort;
use crate::fabric::programmatic_query_backend::ProgrammaticSemanticIngressPort;
use crate::fabric::programmatic_schema::{ProgrammaticRelationId, ProviderInput};
use crate::fabric::source_context::{
    SourceAccessGrant, SourceContextContent, SourceContextLimitState,
    SourceContextMaterializationError, SourceContextMaterializationInput, SourceSpanIdentity,
    materialize_authorized_source_context,
};
use crate::identity::{
    AccessScopeIdentityInput, AuthorizedSourceRangeIdentityInput, CanonicalPublicIdentity,
    issue_access_scope_identity,
};
use crate::relational_program::{AggregateOperator, FieldId, JoinKind, RelationId, ScalarOperator};
use crate::schema_contract::{
    FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SchemaContract,
};
use crate::semantic_query_contract::parse_request;

const EXPECTATIONS: &str =
    include_str!("../contracts/acceptance/relational-fabric-v3/expectations.jsonl");
const FIXTURES: &str =
    include_str!("../contracts/acceptance/relational-fabric-v3/negative-fixtures.jsonl");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    Positive,
    Causal,
    Negative,
}

impl Case {
    const fn fixture_kind(self) -> Option<&'static str> {
        match self {
            Self::Positive => None,
            Self::Causal => Some("causal"),
            Self::Negative => Some("negative"),
        }
    }
}

fn object<'a>(value: &'a Value, context: &str) -> &'a Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{context} must be an object"))
}

fn array<'a>(value: &'a Value, context: &str) -> &'a [Value] {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{context} must be an array"))
}

fn string<'a>(value: &'a Value, context: &str) -> &'a str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{context} must be a string"))
}

fn usize_value(value: &Value, context: &str) -> usize {
    usize::try_from(
        value
            .as_u64()
            .unwrap_or_else(|| panic!("{context} must be an unsigned integer")),
    )
    .unwrap_or_else(|_| panic!("{context} exceeds usize"))
}

fn strict_jsonl(source: &str, context: &str) -> Vec<Value> {
    assert!(!source.is_empty(), "{context} must be non-empty");
    source
        .lines()
        .enumerate()
        .map(|(index, line)| {
            assert!(!line.is_empty(), "{context} row {} is blank", index + 1);
            jcs::decode_strict(line.as_bytes())
                .unwrap_or_else(|error| panic!("strict {context} row {}: {error}", index + 1))
        })
        .collect()
}

fn expectation(claim_id: &str) -> Value {
    let rows = strict_jsonl(EXPECTATIONS, "WP33 expectations");
    let matches = rows
        .into_iter()
        .filter(|row| row["claim_id"] == claim_id)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one frozen {claim_id} expectation");
    matches.into_iter().next().expect("one expectation")
}

fn fixture(claim_id: &str, kind: &str) -> Value {
    let rows = strict_jsonl(FIXTURES, "WP33 fixtures");
    let matches = rows
        .into_iter()
        .filter(|row| row["claim_id"] == claim_id && row["kind"] == kind)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one frozen {claim_id} {kind} fixture");
    matches.into_iter().next().expect("one fixture")
}

fn base_inputs(claim: &Value) -> Value {
    let universe = &claim["complete_input_universe"];
    assert_eq!(universe["closed"], true, "input universe must be closed");
    universe["inputs"].clone()
}

fn apply_fixture_mutation(claim: &Value, fixture: &Value) -> Value {
    assert_eq!(fixture["claim_id"], claim["claim_id"]);
    assert_eq!(fixture["semantic"], true);
    assert_eq!(fixture["integrity_only"], false);
    assert_eq!(fixture["imports"], json!([]));
    let mutation = object(&fixture["mutation"], "fixture mutation");
    let input_role = string(&mutation["input_role"], "fixture input role");
    let pointer = string(&mutation["json_pointer"], "fixture JSON pointer");
    let mut inputs = base_inputs(claim);
    if input_role == "$input_universe" {
        assert!(
            pointer.is_empty(),
            "atomic input-universe mutation is root-only"
        );
        let before = object(&mutation["before"], "atomic mutation before");
        let after = object(&mutation["after"], "atomic mutation after");
        assert_eq!(
            before.keys().collect::<Vec<_>>(),
            after.keys().collect::<Vec<_>>()
        );
        for (role_name, before_value) in before {
            assert_eq!(
                &inputs[role_name], before_value,
                "atomic input role {role_name} drifted"
            );
            inputs[role_name] = after[role_name].clone();
        }
        return inputs;
    }
    let role = inputs
        .get_mut(input_role)
        .unwrap_or_else(|| panic!("fixture input role {input_role} is absent"));
    if pointer.is_empty() {
        assert_eq!(
            *role, mutation["before"],
            "whole-role mutation before drifted"
        );
        *role = mutation["after"].clone();
    } else {
        let current = role
            .pointer(pointer)
            .unwrap_or_else(|| panic!("fixture pointer {pointer} is absent"));
        assert_eq!(current, &mutation["before"], "fixture before value drifted");
        let target = role
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("fixture mutable pointer {pointer} is absent"));
        *target = mutation["after"].clone();
    }
    inputs
}

fn case_inputs(claim: &Value, case: Case) -> (Value, Option<Value>) {
    match case.fixture_kind() {
        None => (base_inputs(claim), None),
        Some(kind) => {
            let fixture = fixture(string(&claim["claim_id"], "claim id"), kind);
            (apply_fixture_mutation(claim, &fixture), Some(fixture))
        }
    }
}

fn positive_response(claim: &Value) -> &Value {
    &claim["decoded_expectation"]["rows"][0][0]
}

fn expected_case<'a>(claim: &'a Value, fixture: Option<&'a Value>) -> &'a Value {
    fixture.map_or_else(|| positive_response(claim), |row| &row["expected_decoded"])
}

#[derive(Clone, Debug)]
enum Cell {
    Text(String),
    Int64(i64),
}

#[derive(Clone, Debug)]
struct TestField {
    name: String,
    id: FieldId,
    data_type: DataType,
}

impl TestField {
    fn text(claim: &str, relation: &str, name: &str) -> Self {
        Self {
            name: name.to_owned(),
            id: field(format!("wp38.{claim}.{relation}.{name}")),
            data_type: DataType::Utf8,
        }
    }

    fn int64(claim: &str, relation: &str, name: &str) -> Self {
        Self {
            name: name.to_owned(),
            id: field(format!("wp38.{claim}.{relation}.{name}")),
            data_type: DataType::Int64,
        }
    }
}

#[derive(Clone, Debug)]
struct TestRelation {
    id: RelationId,
    table_name: String,
    fields: Vec<TestField>,
    rows: Vec<Vec<Cell>>,
}

impl TestRelation {
    fn new(claim: &str, name: &str, fields: Vec<TestField>, rows: Vec<Vec<Cell>>) -> Self {
        assert!(rows.iter().all(|row| row.len() == fields.len()));
        Self {
            id: relation(name),
            table_name: format!("wp38_{}_{}", claim, name.replace('.', "_")),
            fields,
            rows,
        }
    }

    fn field(&self, name: &str) -> FieldId {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| field.id.clone())
            .unwrap_or_else(|| panic!("{} lacks field {name}", self.id.as_str()))
    }

    fn field_ids(&self) -> Vec<FieldId> {
        self.fields.iter().map(|field| field.id.clone()).collect()
    }

    fn schema(&self) -> SchemaRef {
        let fields = self
            .fields
            .iter()
            .map(|field| {
                Field::new(&field.name, field.data_type.clone(), false).with_metadata(
                    HashMap::from([(
                        FIELD_ID_METADATA_KEY.to_owned(),
                        field.id.as_str().to_owned(),
                    )]),
                )
            })
            .collect::<Vec<_>>();
        Arc::new(Schema::new_with_metadata(
            fields,
            HashMap::from([(
                RELATION_ID_METADATA_KEY.to_owned(),
                self.id.as_str().to_owned(),
            )]),
        ))
    }

    fn provider_input(&self) -> ProviderInput {
        let schema = self.schema();
        let columns = self
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| match field.data_type {
                DataType::Utf8 => Arc::new(StringArray::from(
                    self.rows
                        .iter()
                        .map(|row| match &row[index] {
                            Cell::Text(value) => value.as_str(),
                            Cell::Int64(_) => panic!("text column received integer cell"),
                        })
                        .collect::<Vec<_>>(),
                )) as ArrayRef,
                DataType::Int64 => Arc::new(Int64Array::from(
                    self.rows
                        .iter()
                        .map(|row| match row[index] {
                            Cell::Int64(value) => value,
                            Cell::Text(_) => panic!("integer column received text cell"),
                        })
                        .collect::<Vec<_>>(),
                )) as ArrayRef,
                ref other => panic!("unsupported test data type {other:?}"),
            })
            .collect::<Vec<_>>();
        let batch = RecordBatch::try_new(Arc::clone(&schema), columns)
            .expect("artifact-derived typed Arrow batch");
        let table_reference = TableReference::full(
            FABRIC_CATALOG,
            FabricSchemaRole::Fact.as_str(),
            self.table_name.clone(),
        );
        let contract = Arc::new(
            SchemaContract::try_new(
                format!("provider:wp38:{}:v1", self.id.as_str()),
                table_reference.clone(),
                Arc::clone(&schema),
                Arc::clone(&schema),
                (0..self.fields.len())
                    .map(|index| FieldIndexMapping::direct(index, index))
                    .collect(),
            )
            .expect("artifact relation contract"),
        );
        let provider = Arc::new(
            MemTable::try_new(schema, vec![vec![batch]]).expect("artifact MemTable provider"),
        );
        ProviderInput::new(
            ProgrammaticRelationId::new(self.id.as_str()),
            table_reference,
            contract,
            provider,
        )
    }
}

fn relation(value: impl Into<String>) -> RelationId {
    RelationId::new(value).expect("bounded evidence relation identity")
}

fn field(value: impl Into<String>) -> FieldId {
    FieldId::new(value).expect("bounded evidence field identity")
}

fn text(value: impl Into<String>) -> Cell {
    Cell::Text(value.into())
}

fn int64(value: i64) -> Cell {
    Cell::Int64(value)
}

fn encode(value: &Value) -> String {
    String::from_utf8(jcs::canonicalize_value(value).expect("canonical evidence row"))
        .expect("canonical JSON is UTF-8")
}

fn semantic_limits() -> SemanticRequestLimits {
    SemanticRequestLimits::try_new(8, 8, 8, 8, 32, 32, 1_024)
        .expect("bounded semantic evidence limits")
}

fn ingress_limits() -> EpochBoundSemanticIngressLimits {
    EpochBoundSemanticIngressLimits::try_new(semantic_limits(), 256, 256, 256, 256, 64)
        .expect("bounded WP38 ingress limits")
}

fn validate_v2_ingress(inputs: &Value) -> Result<(), String> {
    let envelope = &inputs["request_envelope"];
    let canonical = string(&envelope["canonical_json"], "canonical request JSON");
    let decoded = jcs::decode_strict(canonical.as_bytes())
        .map_err(|error| format!("strict v2 request decode failed: {error}"))?;
    if decoded != envelope["decoded"] {
        return Err("canonical v2 request and frozen decoded request disagree".to_owned());
    }
    let parsed = parse_request(canonical.as_bytes())
        .map_err(|error| format!("v2 request parse failed: {error}"))?;
    let port = ApplicationOwnedSemanticIngressPort::try_released_v2_0([0x38; 32], ingress_limits())
        .map_err(|error| format!("v2 ingress construction failed: {error}"))?;
    port.validate_request(&parsed)
        .map_err(|error| format!("v2 ingress rejected request: {error}"))
}

fn runtime_closure() -> ProducerClosureProof {
    ProducerClosureProof {
        proof_pin: [5; 32],
        application_authority_id: Arc::from("authority.application"),
        families: vec![ProducerFamilyClosureRow {
            family_id: Arc::from("family.core"),
            disposition: ProducerFamilyDisposition::RuntimeProducer(RuntimeProducerProof {
                producer_id: Arc::from("producer.wp38.artifact"),
                authority_id: Arc::from("authority.application"),
                algorithm_release: Arc::from("algorithm.wp38.v1"),
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

#[derive(Clone, Debug)]
struct CompiledPlan {
    form: ReleasedSemanticForm,
    output_role: Arc<str>,
    output_relation: RelationId,
    output_fields: Vec<FieldId>,
    operators: Vec<ProgramOperatorRow>,
    clauses: Vec<ProgramClauseBindingRow>,
    request_clauses: Vec<SemanticRequestClauseRow>,
    explicit_limit: Option<usize>,
    semantic_class: SemanticQueryClass,
}

impl CompiledPlan {
    fn request(&self, semantic_request_id: &str, query_id: &str) -> SemanticRequestRelations {
        SemanticRequestRelations {
            semantic_request_id: Arc::from(semantic_request_id.to_owned()),
            program_catalog_pin: [1; 32],
            source_pin: [2; 32],
            policy_pin: [3; 32],
            producer_closure_proof_pin: [5; 32],
            blocks: vec![SemanticRequestBlockRow {
                query_id: Arc::from(query_id.to_owned()),
                form: self.form,
                output_role_id: Arc::clone(&self.output_role),
                explicit_result_limit: self.explicit_limit,
            }],
            clauses: self.request_clauses.clone(),
            dependencies: Vec::new(),
            limits: semantic_limits(),
        }
    }

    fn catalog(&self, relations: &[TestRelation]) -> SemanticQueryProgramCatalog {
        SemanticQueryProgramCatalog {
            program_catalog_pin: [1; 32],
            program_release_pin: [4; 32],
            authority: SemanticQueryAuthority::ApplicationOwned(Arc::from("authority.application")),
            semantic_class: self.semantic_class.clone(),
            forms: vec![ProgramFormBindingRow {
                form_label: Arc::from(self.form.label()),
                output_role_id: Arc::clone(&self.output_role),
                root_node_id: Arc::clone(
                    &self.operators.last().expect("non-empty program").node_id,
                ),
                output_relation_id: self.output_relation.clone(),
                output_fields: self.output_fields.clone(),
            }],
            input_roles: Vec::new(),
            clauses: self.clauses.clone(),
            operators: self.operators.clone(),
            relation_schemas: relations
                .iter()
                .map(|relation| ProgramRelationSchemaRow {
                    relation_id: relation.id.clone(),
                    fields: relation.field_ids(),
                })
                .collect(),
            required_fact_families: vec![ProgramRequiredFactFamilyRow {
                form_label: Arc::from(self.form.label()),
                output_role_id: Arc::clone(&self.output_role),
                family_id: Arc::from("family.core"),
            }],
        }
    }
}

fn single_plan(
    form: ReleasedSemanticForm,
    relation: &TestRelation,
    query_id: &str,
    filters: &[(&str, &str)],
    sort_fields: &[&str],
    explicit_limit: Option<usize>,
    semantic_class: SemanticQueryClass,
) -> CompiledPlan {
    let role: Arc<str> = Arc::from("result");
    let input_fields = relation.field_ids();
    let mut operators = vec![ProgramOperatorRow {
        form_label: Arc::from(form.label()),
        output_role_id: Arc::clone(&role),
        node_id: Arc::from("node.input"),
        ordinal: 0,
        input_node_ids: Vec::new(),
        operator: ProgramRelationalOperator::Input {
            relation_id: relation.id.clone(),
        },
        output_fields: input_fields.clone(),
    }];
    let mut clauses = Vec::new();
    let mut request_clauses = Vec::new();
    let mut current: Arc<str> = Arc::from("node.input");
    if !filters.is_empty() {
        let filter_node: Arc<str> = Arc::from("node.filter");
        operators.push(ProgramOperatorRow {
            form_label: Arc::from(form.label()),
            output_role_id: Arc::clone(&role),
            node_id: Arc::clone(&filter_node),
            ordinal: 1,
            input_node_ids: vec![Arc::clone(&current)],
            operator: ProgramRelationalOperator::Filter,
            output_fields: input_fields.clone(),
        });
        for (index, (field_name, value)) in filters.iter().enumerate() {
            let clause_id: Arc<str> = Arc::from(format!("filter.{index}"));
            clauses.push(ProgramClauseBindingRow {
                form_label: Arc::from(form.label()),
                output_role_id: Arc::clone(&role),
                clause_id: Arc::clone(&clause_id),
                operator_node_id: Arc::clone(&filter_node),
                input_field_id: relation.field(field_name),
                scalar_operator: ScalarOperator::Equal,
                value_kind: SemanticValueKind::Text,
                required: true,
            });
            request_clauses.push(SemanticRequestClauseRow {
                query_id: Arc::from(query_id.to_owned()),
                clause_id,
                value: SemanticClauseValue::Text(Arc::from((*value).to_owned())),
            });
        }
        current = filter_node;
    }
    if !sort_fields.is_empty() {
        let ordinal = u32::try_from(operators.len()).expect("small operator count");
        let sort_node: Arc<str> = Arc::from("node.sort");
        operators.push(ProgramOperatorRow {
            form_label: Arc::from(form.label()),
            output_role_id: Arc::clone(&role),
            node_id: Arc::clone(&sort_node),
            ordinal,
            input_node_ids: vec![Arc::clone(&current)],
            operator: ProgramRelationalOperator::Sort {
                fields: sort_fields
                    .iter()
                    .map(|name| ProgramSortField {
                        input_field_id: relation.field(name),
                        ascending: true,
                        nulls_first: false,
                    })
                    .collect(),
            },
            output_fields: input_fields.clone(),
        });
        current = sort_node;
    }
    if explicit_limit.is_some() {
        let ordinal = u32::try_from(operators.len()).expect("small operator count");
        operators.push(ProgramOperatorRow {
            form_label: Arc::from(form.label()),
            output_role_id: Arc::clone(&role),
            node_id: Arc::from("node.limit"),
            ordinal,
            input_node_ids: vec![current],
            operator: ProgramRelationalOperator::Limit { skip: 0 },
            output_fields: input_fields.clone(),
        });
    }
    CompiledPlan {
        form,
        output_role: role,
        output_relation: relation.id.clone(),
        output_fields: input_fields,
        operators,
        clauses,
        request_clauses,
        explicit_limit,
        semantic_class,
    }
}

fn binary_join_plan(
    form: ReleasedSemanticForm,
    left: &TestRelation,
    right: &TestRelation,
    kind: JoinKind,
    left_key: &str,
    right_key: &str,
    sort_fields: &[&str],
) -> CompiledPlan {
    let role: Arc<str> = Arc::from("result");
    let left_fields = left.field_ids();
    let mut operators = vec![
        ProgramOperatorRow {
            form_label: Arc::from(form.label()),
            output_role_id: Arc::clone(&role),
            node_id: Arc::from("node.left"),
            ordinal: 0,
            input_node_ids: Vec::new(),
            operator: ProgramRelationalOperator::Input {
                relation_id: left.id.clone(),
            },
            output_fields: left_fields.clone(),
        },
        ProgramOperatorRow {
            form_label: Arc::from(form.label()),
            output_role_id: Arc::clone(&role),
            node_id: Arc::from("node.right"),
            ordinal: 1,
            input_node_ids: Vec::new(),
            operator: ProgramRelationalOperator::Input {
                relation_id: right.id.clone(),
            },
            output_fields: right.field_ids(),
        },
        ProgramOperatorRow {
            form_label: Arc::from(form.label()),
            output_role_id: Arc::clone(&role),
            node_id: Arc::from("node.join"),
            ordinal: 2,
            input_node_ids: vec![Arc::from("node.left"), Arc::from("node.right")],
            operator: ProgramRelationalOperator::Join {
                kind,
                predicates: vec![ProgramJoinPredicate {
                    left_field_id: left.field(left_key),
                    right_field_id: right.field(right_key),
                    scalar_operator: ScalarOperator::Equal,
                }],
            },
            output_fields: left_fields.clone(),
        },
    ];
    if !sort_fields.is_empty() {
        operators.push(ProgramOperatorRow {
            form_label: Arc::from(form.label()),
            output_role_id: Arc::clone(&role),
            node_id: Arc::from("node.sort"),
            ordinal: 3,
            input_node_ids: vec![Arc::from("node.join")],
            operator: ProgramRelationalOperator::Sort {
                fields: sort_fields
                    .iter()
                    .map(|name| ProgramSortField {
                        input_field_id: left.field(name),
                        ascending: true,
                        nulls_first: false,
                    })
                    .collect(),
            },
            output_fields: left_fields.clone(),
        });
    }
    CompiledPlan {
        form,
        output_role: role,
        output_relation: left.id.clone(),
        output_fields: left_fields,
        operators,
        clauses: Vec::new(),
        request_clauses: Vec::new(),
        explicit_limit: None,
        semantic_class: SemanticQueryClass::Fact(Arc::from("semantic.fact")),
    }
}

fn aggregate_plan(relation: &TestRelation, group_field: &str, count_field: &str) -> CompiledPlan {
    let form = ReleasedSemanticForm::SummarizeObjectiveFacts;
    let role: Arc<str> = Arc::from("result");
    let output_fields = vec![relation.field(group_field), relation.field(count_field)];
    CompiledPlan {
        form,
        output_role: Arc::clone(&role),
        output_relation: relation.id.clone(),
        output_fields: output_fields.clone(),
        operators: vec![
            ProgramOperatorRow {
                form_label: Arc::from(form.label()),
                output_role_id: Arc::clone(&role),
                node_id: Arc::from("node.input"),
                ordinal: 0,
                input_node_ids: Vec::new(),
                operator: ProgramRelationalOperator::Input {
                    relation_id: relation.id.clone(),
                },
                output_fields: relation.field_ids(),
            },
            ProgramOperatorRow {
                form_label: Arc::from(form.label()),
                output_role_id: Arc::clone(&role),
                node_id: Arc::from("node.aggregate"),
                ordinal: 1,
                input_node_ids: vec![Arc::from("node.input")],
                operator: ProgramRelationalOperator::Aggregate {
                    group_by: vec![ProgramGroupField {
                        input_field_id: relation.field(group_field),
                        output_field_id: relation.field(group_field),
                    }],
                    aggregates: vec![ProgramAggregateField {
                        input_field_id: relation.field(count_field),
                        output_field_id: relation.field(count_field),
                        aggregate_operator: AggregateOperator::Count,
                    }],
                },
                output_fields: output_fields.clone(),
            },
            ProgramOperatorRow {
                form_label: Arc::from(form.label()),
                output_role_id: Arc::clone(&role),
                node_id: Arc::from("node.sort"),
                ordinal: 2,
                input_node_ids: vec![Arc::from("node.aggregate")],
                operator: ProgramRelationalOperator::Sort {
                    fields: vec![ProgramSortField {
                        input_field_id: relation.field(group_field),
                        ascending: true,
                        nulls_first: false,
                    }],
                },
                output_fields,
            },
        ],
        clauses: Vec::new(),
        request_clauses: Vec::new(),
        explicit_limit: None,
        semantic_class: SemanticQueryClass::Fact(Arc::from("semantic.fact")),
    }
}

fn child_resources() -> ChildResourceLimits {
    ChildResourceLimits::try_new(16 * 1024 * 1024, 32 * 1024 * 1024, 4, 2, 1_024, 1)
        .expect("bounded WP38 child resources")
}

fn resource_coordinator(epoch: &ProgrammaticFabricEpoch) -> EpochResourceCoordinator {
    let policy = EpochResourcePolicy::try_new(
        child_resources(),
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
    .expect("bounded WP38 epoch resource policy");
    EpochResourceCoordinator::try_new(*epoch.identity(), [0x33; 32], policy)
        .expect("WP38 resource coordinator")
}

async fn sealed_epoch(seed: u8, relations: &[TestRelation]) -> ProgrammaticFabricEpoch {
    let mut builder = ProgrammaticFabricEpochBuilder::try_new(
        FabricEpochId::from_bytes([seed; 16]),
        FabricEpochRuntimeConfig::default(),
    )
    .expect("fresh WP38 programmatic epoch");
    for relation in relations {
        builder
            .register_provider(relation.provider_input())
            .expect("register artifact-derived relation");
    }
    builder
        .seal_for_test()
        .await
        .expect("seal artifact-derived epoch")
}

async fn authorized_child(
    epoch: &ProgrammaticFabricEpoch,
    granted_relations: &[&TestRelation],
) -> Result<AuthorizedChildSession, ChildSessionError> {
    authorized_child_with_max_rows(epoch, granted_relations, 1_024).await
}

async fn authorized_child_with_max_rows(
    epoch: &ProgrammaticFabricEpoch,
    granted_relations: &[&TestRelation],
    max_output_rows: usize,
) -> Result<AuthorizedChildSession, ChildSessionError> {
    authorized_child_with_access_scope_and_max_rows(
        epoch,
        granted_relations,
        [0x11; 32],
        max_output_rows,
    )
    .await
}

async fn authorized_child_with_access_scope_and_max_rows(
    epoch: &ProgrammaticFabricEpoch,
    granted_relations: &[&TestRelation],
    access_scope: [u8; 32],
    max_output_rows: usize,
) -> Result<AuthorizedChildSession, ChildSessionError> {
    let resources = resource_coordinator(epoch);
    let policy = ChildSessionPolicy::try_new(
        ChildSessionPins::try_new(*epoch.identity(), access_scope, [0x22; 32], [0x33; 32])
            .expect("exact child pins"),
        granted_relations
            .iter()
            .map(|relation| {
                ChildTableGrant::try_new(ProgrammaticRelationId::new(relation.id.as_str()))
                    .expect("artifact relation grant")
            })
            .collect::<Vec<_>>(),
        child_resources(),
        max_output_rows,
        ChildRegistryAllowlist::default(),
    )
    .expect("artifact child policy");
    epoch.authorized_child_session(policy, &resources).await
}

#[derive(Debug)]
enum Execution {
    Rows(Vec<BTreeMap<String, Value>>),
    Rejected(String),
    Unavailable(Vec<SemanticCompilationIssue>),
}

async fn execute_plan(
    claim_id: &str,
    query_id: &str,
    relations: Vec<TestRelation>,
    plan: CompiledPlan,
    denied_relations: &[&str],
) -> Execution {
    execute_plan_with_closure(
        claim_id,
        query_id,
        relations,
        plan,
        denied_relations,
        runtime_closure(),
    )
    .await
}

async fn execute_plan_with_closure(
    claim_id: &str,
    query_id: &str,
    relations: Vec<TestRelation>,
    plan: CompiledPlan,
    denied_relations: &[&str],
    closure: ProducerClosureProof,
) -> Execution {
    let request = plan.request(claim_id, query_id);
    let catalog = plan.catalog(&relations);
    let compiled = match compile_relational_semantic_request(&request, &catalog, &closure) {
        Ok(compiled) => compiled,
        Err(error) => return Execution::Rejected(error.to_string()),
    };
    let block = compiled.blocks().first().expect("one compiled query block");
    let Some(output) = block.output() else {
        return Execution::Unavailable(block.issues().to_vec());
    };
    let seed = claim_id
        .as_bytes()
        .iter()
        .fold(0_u8, |state, byte| state.wrapping_add(*byte));
    let epoch = sealed_epoch(seed, &relations).await;
    let granted = relations
        .iter()
        .filter(|relation| !denied_relations.contains(&relation.id.as_str()))
        .collect::<Vec<_>>();
    let child = match authorized_child(&epoch, &granted).await {
        Ok(child) => child,
        Err(error) => return Execution::Rejected(error.to_string()),
    };
    let result = match child.execute_relational_program(output.program()).await {
        Ok(result) => result,
        Err(error) => return Execution::Rejected(error.to_string()),
    };
    let schema = result.schema();
    let mut rows = Vec::new();
    for batch in result.batches() {
        for row_index in 0..batch.num_rows() {
            let mut row = BTreeMap::new();
            for (column_index, field) in schema.fields().iter().enumerate() {
                let array = batch.column(column_index);
                let value = match field.data_type() {
                    DataType::Utf8 => Value::String(
                        array
                            .as_any()
                            .downcast_ref::<StringArray>()
                            .expect("UTF-8 result array")
                            .value(row_index)
                            .to_owned(),
                    ),
                    DataType::Int64 => Value::from(
                        array
                            .as_any()
                            .downcast_ref::<Int64Array>()
                            .expect("Int64 result array")
                            .value(row_index),
                    ),
                    other => panic!("unexpected result type {other:?}"),
                };
                row.insert(field.name().clone(), value);
            }
            rows.push(row);
        }
    }
    Execution::Rows(rows)
}

fn rows(execution: Execution) -> Vec<BTreeMap<String, Value>> {
    match execution {
        Execution::Rows(rows) => rows,
        Execution::Rejected(error) => panic!("production execution rejected: {error}"),
        Execution::Unavailable(issues) => panic!("production execution unavailable: {issues:?}"),
    }
}

fn row_text<'a>(row: &'a BTreeMap<String, Value>, name: &str) -> &'a str {
    string(
        row.get(name)
            .unwrap_or_else(|| panic!("observed row lacks {name}")),
        name,
    )
}

fn row_usize(row: &BTreeMap<String, Value>, name: &str) -> usize {
    usize::try_from(
        row.get(name)
            .and_then(Value::as_i64)
            .unwrap_or_else(|| panic!("observed row lacks integer {name}")),
    )
    .unwrap_or_else(|_| panic!("observed {name} is negative or outside usize"))
}

fn canonical_row(row: &BTreeMap<String, Value>, name: &str) -> Value {
    jcs::decode_strict(row_text(row, name).as_bytes()).expect("strict observed canonical row")
}

fn query(claim_inputs: &Value) -> &Value {
    &claim_inputs["request_envelope"]["decoded"]["queries"][0]
}

fn public_provenance(inputs: &Value) -> Value {
    let pinned = &inputs["pinned_epoch"];
    json!({
        "epoch_id": pinned["fabric_epoch_id"],
        "snapshot_id": pinned["snapshot_id"],
        "query_program_release": inputs["program_binding"]["query_program_release"],
        "producer_closure_id": inputs["program_binding"]["producer_closure_id"],
        "policy_release": pinned["policy_release"],
        "expectation_issuance": pinned["expectation_issuance"],
    })
}

fn assert_positive_response_common(inputs: &Value, actual_query: &Value, expected: &Value) {
    assert_eq!(actual_query, &expected["query_results"][0]);
    assert_eq!(
        expected["specification"],
        "composable semantic CPG fact query response"
    );
    assert_eq!(
        expected["version"],
        inputs["request_envelope"]["decoded"]["version"]
    );
    assert_eq!(
        expected["semantic_request_id"],
        inputs["request_envelope"]["decoded"]["semantic_request_id"]
    );
    assert_eq!(
        expected["snapshot"],
        inputs["pinned_epoch"]["public_snapshot_projection"]
    );
    assert_eq!(expected["query_results"].as_array().map(Vec::len), Some(1));
    assert_eq!(expected["errors"], json!([]));
}

fn expected_fault<'a>(fixture: &'a Value) -> &'a Value {
    &fixture["expected_decoded"]
}

fn assert_fault(actual: Value, fixture: &Value) {
    assert_eq!(actual, *expected_fault(fixture));
}

fn execution_rows(execution: Execution) -> Result<Vec<BTreeMap<String, Value>>, String> {
    match execution {
        Execution::Rows(rows) => Ok(rows),
        Execution::Rejected(error) => Err(error),
        Execution::Unavailable(issues) => Err(format!("unavailable: {issues:?}")),
    }
}

fn entity_rows(claim_slug: &str, admitted: &Value) -> TestRelation {
    let fields = vec![
        TestField::text(claim_slug, "entity", "entity_id"),
        TestField::text(claim_slug, "entity", "semantic_kind"),
        TestField::text(claim_slug, "entity", "representation"),
        TestField::text(claim_slug, "entity", "record_json"),
    ];
    let rows = array(&admitted["entity_rows"], "entity rows")
        .iter()
        .map(|row| {
            vec![
                text(string(&row["entity_id"], "entity id")),
                text(string(&row["semantic_kind"], "entity semantic kind")),
                text(string(&row["representation"], "entity representation")),
                text(encode(row)),
            ]
        })
        .collect();
    TestRelation::new(claim_slug, "canonical.entity", fields, rows)
}

fn phrase_semantics(phrase: &str) -> Option<(&'static str, &'static str)> {
    match phrase {
        "function syntax" => Some(("function_syntax", "syntax_occurrence")),
        "function" => Some(("function_declaration", "semantic_entity")),
        _ => None,
    }
}

async fn claim_004(case: Case) {
    const CLAIM: &str = "RFV3-CLAIM-004";
    let claim = expectation(CLAIM);
    let (inputs, fixture) = case_inputs(&claim, case);
    let ingress = validate_v2_ingress(&inputs);
    if case == Case::Negative {
        assert!(
            ingress
                .expect_err("evaluative v2 request must fail at programmatic ingress")
                .contains("evaluative intent")
        );
    } else {
        ingress.expect("released v2 request must pass programmatic ingress");
    }
    let request = query(&inputs);
    let phrase = string(&request["looking_for"], "looking_for");
    let relation = entity_rows("004", &inputs["admitted_relations"]);
    let semantic_class = if phrase_semantics(phrase).is_some() {
        SemanticQueryClass::Fact(Arc::from("semantic.fact"))
    } else {
        SemanticQueryClass::Judgment(Arc::from(phrase.to_owned()))
    };
    let filters = phrase_semantics(phrase)
        .map(|(kind, representation)| {
            vec![("semantic_kind", kind), ("representation", representation)]
        })
        .unwrap_or_default();
    let plan = single_plan(
        ReleasedSemanticForm::FindCodeEntities,
        &relation,
        string(&request["query_id"], "query id"),
        &filters,
        &["entity_id"],
        None,
        semantic_class,
    );
    let execution = execute_plan(
        CLAIM,
        string(&request["query_id"], "query id"),
        vec![relation],
        plan,
        &[],
    )
    .await;

    if case == Case::Negative {
        let error = execution_rows(execution).expect_err("judgment request must reject");
        assert!(error.contains("evaluative or judgment semantics"));
        let fixture = fixture.as_ref().expect("negative fixture");
        let actual = json!({
            "execution_state": "FAILED",
            "availability_state": "UNAVAILABLE",
            "completeness_state": "UNAVAILABLE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "READY",
            "resolved_semantics": {
                "rejected_phrase": phrase,
                "fact_equivalent_rewrites": [
                    "find function declarations",
                    "retrieve static source/semantic facts"
                ]
            },
            "query_result": {
                "query_id": request["query_id"],
                "result_role": "entities",
                "entity_ids": [],
                "coverage": {"state": "NOT_APPLICABLE", "reason": "excluded domain"},
                "errors": [{
                    "code": "NOT_OBJECTIVE_FACT_REQUEST",
                    "layer": "semantic_resolution",
                    "retryable": false,
                    "safe_message": "Runtime observation and coverage are outside the present-state fact substrate.",
                    "field": "looking_for",
                    "semantic_phrase": phrase,
                    "candidate_interpretations": [
                        "find function declarations",
                        "retrieve static source/semantic facts"
                    ],
                    "failed_dependency_query_id": null,
                    "diagnostic_id": null
                }],
                "notices": []
            },
            "errors": [{
                "code": "NOT_OBJECTIVE_FACT_REQUEST",
                "layer": "semantic_resolution",
                "retryable": false,
                "safe_message": "Runtime observation and coverage are outside the present-state fact substrate.",
                "field": "looking_for",
                "semantic_phrase": phrase,
                "candidate_interpretations": [
                    "find function declarations",
                    "retrieve static source/semantic facts"
                ],
                "failed_dependency_query_id": null,
                "diagnostic_id": null
            }]
        });
        assert_fault(actual, fixture);
        return;
    }

    let observed = rows(execution);
    let ids = observed
        .iter()
        .map(|row| Value::String(row_text(row, "entity_id").to_owned()))
        .collect::<Vec<_>>();
    let records = observed
        .iter()
        .map(|row| canonical_row(row, "record_json"))
        .collect::<Vec<_>>();
    let (kind, representation) = phrase_semantics(phrase).expect("objective phrase");
    if case == Case::Causal {
        let entities = records
            .iter()
            .cloned()
            .map(|mut record| {
                record
                    .as_object_mut()
                    .expect("entity record object")
                    .remove("alias");
                record
            })
            .collect::<Vec<_>>();
        let coverage = &inputs["producer_coverage"];
        let completed_inputs = array(&coverage["covered_entity_ids"], "covered entity ids").len();
        let actual = json!({
            "execution_state": "COMPLETE",
            "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE",
            "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED",
            "dependency_state": "READY",
            "resolved_semantics": {
                "looking_for": phrase,
                "semantic_kind": kind,
                "representation": representation
            },
            "query_result": {
                "query_id": request["query_id"],
                "result_role": "entities",
                "entity_ids": ids,
                "entities": entities,
                "coverage": {"state": coverage["state"], "family": coverage["family"],
                    "scope": coverage["scope"], "completed_inputs": completed_inputs},
                "errors": [],
                "notices": []
            },
            "errors": []
        });
        assert_fault(actual, fixture.as_ref().expect("causal fixture"));
        return;
    }

    let actual_query = json!({
        "query_id": request["query_id"],
        "request": request["request"],
        "execution_state": "COMPLETE",
        "availability_state": "AVAILABLE",
        "completeness_state": "COMPLETE",
        "freshness_state": "CURRENT",
        "limit_state": "NOT_APPLIED",
        "dependency_state": "READY",
        "resolved_semantics": {
            "looking_for": phrase,
            "semantic_kind": kind,
            "representation": representation
        },
        "result_role": "entities",
        "entity_ids": ids,
        "fact_ids": [],
        "path_ids": [],
        "group_ids": [],
        "source_context_ids": [],
        "bindings": [],
        "coverage": {
            "state": "COMPLETE",
            "scope": inputs["producer_coverage"]["scope"],
            "family": "entity_kind",
            "completed_inputs": array(&inputs["admitted_relations"]["entity_rows"], "entity rows").len()
        },
        "provenance": public_provenance(&inputs),
        "errors": [],
        "notices": []
    });
    let expected = positive_response(&claim);
    assert_positive_response_common(&inputs, &actual_query, expected);
    let actual_entities = records
        .into_iter()
        .map(|mut record| {
            record
                .as_object_mut()
                .expect("entity record object")
                .remove("alias");
            (string(&record["entity_id"], "entity id").to_owned(), record)
        })
        .collect::<Map<_, _>>();
    assert_eq!(Value::Object(actual_entities), expected["entities"]);
}

fn unknown_fact_record(inputs: &Value, coverage: &Value) -> Value {
    let fact = &inputs["admitted_relations"]["fact_rows"][0];
    let property_kind_registry = &inputs["admitted_relations"]["property_kind_registry"];
    assert_eq!(property_kind_registry["relation_id"], "input.property_kind");
    assert_eq!(property_kind_registry["closed_universe"], true);
    let mut property_kind_allocations = BTreeMap::new();
    let mut property_code_allocations = BTreeMap::new();
    for row in array(
        &property_kind_registry["rows"],
        "property kind registry rows",
    ) {
        let name = string(&row["property_kind"], "property kind");
        let code = u16::try_from(
            row["property_kind_code"]
                .as_u64()
                .expect("property kind code"),
        )
        .expect("property kind code fits u16");
        assert_ne!(code, 0, "property kind zero is not allocated");
        assert!(
            property_kind_allocations.insert(name, code).is_none(),
            "duplicate property kind allocation"
        );
        assert!(
            property_code_allocations.insert(code, name).is_none(),
            "duplicate property code allocation"
        );
    }
    let property_kind_code = *property_kind_allocations
        .get("UNKNOWN_EFFECT")
        .expect("UNKNOWN_EFFECT property kind registry row");
    let canonical_value = string(&coverage["family"], "unknown canonical family value");
    let coverage_state = match string(&coverage["state"], "coverage state") {
        "UNAVAILABLE" => UnknownCoverageState::Unavailable,
        "PARTIAL" => UnknownCoverageState::Partial,
        other => panic!("complete coverage cannot materialize an unknown fact: {other}"),
    };
    let provenance = &fact["direct_provenance"];
    let unknown = materialize_explicit_unknown_fact(ExplicitUnknownFactInput {
        workspace_id: Arc::from(string(&fact["workspace_id"], "workspace id")),
        analysis_context_id: Arc::from(string(&fact["analysis_context_id"], "analysis context id")),
        subject_id: Arc::from(string(&coverage["subject"], "coverage subject")),
        requested_family: Arc::from(string(&coverage["family"], "coverage family")),
        property_kind_code,
        canonical_value: Arc::from(canonical_value),
        source_file_id: Arc::from(string(
            &coverage["source_identity"]["file_id"],
            "unknown source file identity",
        )),
        source_content_digest: Arc::from(string(
            &coverage["source_identity"]["content_digest"],
            "unknown source content digest",
        )),
        producer_closure_id: Arc::from(string(
            &inputs["program_binding"]["producer_closure_id"],
            "producer closure identity",
        )),
        policy_identity: Arc::from(string(
            &inputs["pinned_epoch"]["policy_release"],
            "policy identity",
        )),
        reason: Arc::from(
            coverage
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("partial provider coverage"),
        ),
        coverage_state,
        producer_release: Arc::from("r1"),
        source_generation: provenance["source_generation"]
            .as_u64()
            .expect("source generation"),
        input_set_id: Arc::from(string(&provenance["input_set_id"], "input set id")),
        support_ids: Arc::from(
            array(&provenance["support_ids"], "support ids")
                .iter()
                .map(|value| Arc::<str>::from(string(value, "support id")))
                .collect::<Vec<_>>(),
        ),
    })
    .expect("production explicit-unknown materialization");
    json!({
        "fact_id": unknown.fact_id,
        "fact_form": "property",
        "fact_kind": "unknown",
        "fact_class": "semantic",
        "workspace_id": unknown.workspace_id,
        "analysis_context_id": unknown.analysis_context_id,
        "owner_id": unknown.subject_id,
        "statement": {
            "subject": unknown.subject_id,
            "predicate": "UNKNOWN_EFFECT",
            "object": unknown.requested_family
        },
        "certainty": "unresolved",
        "resolution": unknown.coverage_state.resolution(),
        "directness": "direct",
        "producer": {"producer_id": unknown.producer_id, "release": unknown.producer_release},
        "direct_provenance": {
            "source_generation": unknown.source_generation,
            "input_set_id": unknown.input_set_id,
            "support_ids": unknown.support_ids,
            "coverage": {
                "state": unknown.coverage_state.wire_name(),
                "reason": unknown.reason,
                "retryable": false
            }
        },
        "property_kind_code": unknown.property_kind_code,
        "identity_recipe": unknown.identity_recipe
    })
}

fn retrieve_fact_input_set_identity(inputs: &Value) -> CanonicalPublicIdentity {
    let admitted = &inputs["admitted_relations"];
    let fact_rows = array(&admitted["fact_rows"], "retrieve-facts input rows");
    let coverage_rows = array(&admitted["coverage_rows"], "retrieve-facts coverage rows");
    assert!(!fact_rows.is_empty(), "retrieve-facts input set is empty");
    assert!(
        !coverage_rows.is_empty(),
        "retrieve-facts coverage is empty"
    );
    let coverage_state = if coverage_rows.iter().all(|row| row["state"] == "COMPLETE") {
        ObjectiveInputCoverageState::Complete
    } else if coverage_rows
        .iter()
        .any(|row| row["state"] == "UNAVAILABLE")
    {
        ObjectiveInputCoverageState::Indeterminate
    } else {
        assert!(
            coverage_rows.iter().any(|row| row["state"] == "PARTIAL"),
            "retrieve-facts coverage state is outside its closed vocabulary"
        );
        ObjectiveInputCoverageState::Partial
    };
    let mut producer_identities = fact_rows
        .iter()
        .map(|row| {
            Arc::<str>::from(string(
                &row["producer"]["producer_id"],
                "retrieve-facts producer identity",
            ))
        })
        .collect::<Vec<_>>();
    producer_identities.extend(
        coverage_rows
            .iter()
            .filter(|row| row["state"] != "COMPLETE")
            .map(|row| Arc::<str>::from(format!("coverage:{}", row["family"].as_str().unwrap()))),
    );
    producer_identities.sort();
    producer_identities.dedup();
    let identity = issue_objective_input_set_identity(&ObjectiveInputSetIdentityInput {
        workspace_id: Arc::from(string(
            &fact_rows[0]["workspace_id"],
            "retrieve-facts workspace",
        )),
        analysis_context_ids: fact_rows
            .iter()
            .map(|row| {
                Arc::<str>::from(string(
                    &row["analysis_context_id"],
                    "retrieve-facts analysis context",
                ))
            })
            .collect::<Vec<_>>()
            .into(),
        fact_ids: fact_rows
            .iter()
            .map(|row| Arc::<str>::from(string(&row["fact_id"], "retrieve-facts fact")))
            .collect::<Vec<_>>()
            .into(),
        producer_identities: producer_identities.into(),
        policy_identity: Arc::from(string(
            &inputs["pinned_epoch"]["policy_release"],
            "retrieve-facts policy",
        )),
        coverage_state,
    })
    .expect("production retrieve-facts objective input-set identity");
    assert_eq!(
        identity.recipe_evidence(),
        admitted["input_set_identity"],
        "retrieve-facts input-set recipe drifted"
    );
    assert!(
        fact_rows
            .iter()
            .all(|row| { row["direct_provenance"]["input_set_id"] == identity.public_id.as_str() })
    );
    identity
}

async fn claim_005(case: Case) {
    const CLAIM: &str = "RFV3-CLAIM-005";
    let claim = expectation(CLAIM);
    let (inputs, fixture) = case_inputs(&claim, case);
    validate_v2_ingress(&inputs).expect("released v2 request must pass programmatic ingress");
    let request = query(&inputs);
    let admitted = &inputs["admitted_relations"];
    let fields = vec![
        TestField::text("005", "facts", "fact_id"),
        TestField::text("005", "facts", "family"),
        TestField::text("005", "facts", "coverage_state"),
        TestField::text("005", "facts", "record_json"),
    ];
    let mut relation_rows = array(&admitted["fact_rows"], "fact rows")
        .iter()
        .map(|fact| {
            vec![
                text(string(&fact["fact_id"], "fact id")),
                text(string(&fact["fact_kind"], "fact kind")),
                text("COMPLETE"),
                text(encode(fact)),
            ]
        })
        .collect::<Vec<_>>();
    let incomplete = array(&admitted["coverage_rows"], "coverage rows")
        .iter()
        .find(|row| row["state"] != "COMPLETE")
        .expect("explicit incomplete requested family");
    let input_set_identity = retrieve_fact_input_set_identity(&inputs);
    let unknown_record = unknown_fact_record(&inputs, incomplete);
    let unknown_identity = string(&unknown_record["fact_id"], "issued unknown fact identity");
    relation_rows.push(vec![
        text(unknown_identity),
        text(string(&incomplete["family"], "coverage family")),
        text(string(&incomplete["state"], "coverage state")),
        text(encode(&unknown_record)),
    ]);
    let relation = TestRelation::new("005", "canonical.fact", fields, relation_rows);
    let plan = single_plan(
        ReleasedSemanticForm::RetrieveFactsAboutCode,
        &relation,
        string(&request["query_id"], "query id"),
        &[],
        &["fact_id"],
        None,
        SemanticQueryClass::Fact(Arc::from("semantic.fact")),
    );
    let observed = rows(
        execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![relation],
            plan,
            &[],
        )
        .await,
    );
    let ids = observed
        .iter()
        .map(|row| Value::String(row_text(row, "fact_id").to_owned()))
        .collect::<Vec<_>>();
    let records = observed
        .iter()
        .map(|row| canonical_row(row, "record_json"))
        .collect::<Vec<_>>();
    let expected = expected_case(&claim, fixture.as_ref());
    let expected_unknown = if case == Case::Positive {
        string(
            &positive_response(&claim)["query_results"][0]["coverage"]["unknown_fact_id"],
            "expected unknown fact identity",
        )
    } else {
        string(
            &expected["query_result"]["identity_contract"]["unknown_fact_id"],
            "expected unknown fact identity contract",
        )
    };
    assert_eq!(unknown_identity, expected_unknown);
    let identity_recipe = unknown_record["identity_recipe"].clone();
    let source_file_id = incomplete["source_identity"]["file_id"].clone();

    let coverage_states = array(&admitted["coverage_rows"], "coverage rows")
        .iter()
        .map(|row| {
            (
                string(&row["family"], "coverage family").to_owned(),
                row["state"].clone(),
            )
        })
        .collect::<Map<_, _>>();
    let type_record = records
        .iter()
        .find(|record| record["fact_kind"] == "type")
        .expect("executed type fact");
    if case == Case::Causal {
        let actual = json!({
            "execution_state": "COMPLETE", "availability_state": "PARTIAL",
            "completeness_state": "INDETERMINATE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": {
                "requested_families": inputs["producer_coverage"]["requested_families"],
                "type_return": type_record["statement"]["object"]["return"]
            },
            "query_result": {"query_id": request["query_id"], "result_role": "facts",
                "fact_ids": ids, "coverage": {"state": "INDETERMINATE", "families": coverage_states},
                "errors": [], "notices": [],
                "identity_contract": {
                    "known_type_fact_id": type_record["fact_id"],
                    "known_type_identity_recipe": type_record["identity_recipe"],
                    "unknown_fact_id": unknown_identity,
                    "identity_recipe": identity_recipe,
                    "input_set_id": input_set_identity.public_id.as_str(),
                    "input_set_identity_recipe": input_set_identity.recipe_evidence(),
                    "source_file_id": source_file_id}}, "errors": []
        });
        assert_eq!(actual, *expected);
        return;
    }
    if case == Case::Negative {
        let actual = json!({
            "execution_state": "COMPLETE", "availability_state": "PARTIAL",
            "completeness_state": "INDETERMINATE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": {"requested_families": inputs["producer_coverage"]["requested_families"]},
            "query_result": {"query_id": request["query_id"], "result_role": "facts",
                "fact_ids": ids,
                "coverage": {"state": "INDETERMINATE", "families": coverage_states,
                    "unknown_reason": "partial provider coverage"},
                "errors": [], "notices": [],
                "identity_contract": {
                    "known_type_fact_id": type_record["fact_id"],
                    "known_type_identity_recipe": type_record["identity_recipe"],
                    "unknown_fact_id": unknown_identity,
                    "identity_recipe": identity_recipe,
                    "input_set_id": input_set_identity.public_id.as_str(),
                    "input_set_identity_recipe": input_set_identity.recipe_evidence(),
                    "source_file_id": source_file_id}}, "errors": []
        });
        assert_eq!(actual, *expected);
        return;
    }

    let available = array(&admitted["coverage_rows"], "coverage rows")
        .iter()
        .filter(|row| row["state"] == "COMPLETE")
        .map(|row| row["family"].clone())
        .collect::<Vec<_>>();
    let unavailable = array(&admitted["coverage_rows"], "coverage rows")
        .iter()
        .filter(|row| row["state"] != "COMPLETE")
        .map(|row| row["family"].clone())
        .collect::<Vec<_>>();
    let actual_query = json!({
        "query_id": request["query_id"], "request": request["request"],
        "execution_state": "COMPLETE", "availability_state": "PARTIAL",
        "completeness_state": "INDETERMINATE", "freshness_state": "CURRENT",
        "limit_state": "NOT_APPLIED", "dependency_state": "READY",
        "resolved_semantics": {"about": request["about"], "requested_families": request["facts"],
            "available_families": available, "unavailable_families": unavailable},
        "result_role": "facts", "entity_ids": [], "fact_ids": ids, "path_ids": [],
        "group_ids": [], "source_context_ids": [], "bindings": [],
        "coverage": {"state": "INDETERMINATE", "families": coverage_states,
            "unknown_fact_id": unknown_identity},
        "provenance": public_provenance(&inputs), "errors": [], "notices": []
    });
    let expected_response = positive_response(&claim);
    assert_eq!(actual_query, expected_response["query_results"][0]);
    let actual_facts = records
        .into_iter()
        .map(|mut record| {
            object(&record, "executed fact record");
            record
                .as_object_mut()
                .expect("executed fact record object")
                .remove("alias");
            (string(&record["fact_id"], "fact id").to_owned(), record)
        })
        .collect::<Map<_, _>>();
    assert_eq!(Value::Object(actual_facts), expected_response["facts"]);
}

fn edge_relation(claim_slug: &str, name: &str, edge_rows: &Value) -> TestRelation {
    let fields = vec![
        TestField::text(claim_slug, name, "fact_id"),
        TestField::text(claim_slug, name, "subject"),
        TestField::text(claim_slug, name, "target"),
        TestField::text(claim_slug, name, "fact_kind"),
        TestField::text(claim_slug, name, "record_json"),
    ];
    let rows = array(edge_rows, "edge rows")
        .iter()
        .map(|edge| {
            vec![
                text(string(&edge["fact_id"], "edge fact id")),
                text(string(&edge["statement"]["subject"], "edge subject")),
                text(string(&edge["statement"]["object"], "edge target")),
                text(string(&edge["fact_kind"], "edge kind")),
                text(encode(edge)),
            ]
        })
        .collect();
    TestRelation::new(claim_slug, name, fields, rows)
}

async fn claim_006(case: Case) {
    const CLAIM: &str = "RFV3-CLAIM-006";
    let claim = expectation(CLAIM);
    let (inputs, fixture) = case_inputs(&claim, case);
    validate_v2_ingress(&inputs).expect("released v2 request must pass programmatic ingress");
    let request = query(&inputs);
    let admitted = &inputs["admitted_relations"];
    let relation = edge_relation("006", "canonical.call_fact", &admitted["call_edges"]);
    let start = string(&request["starting_from"][0], "starting entity");
    let kind = string(&request["relationship"], "relationship");
    let plan = single_plan(
        ReleasedSemanticForm::FollowCodeRelationships,
        &relation,
        string(&request["query_id"], "query id"),
        &[("subject", start), ("fact_kind", kind)],
        &["target", "fact_id"],
        None,
        SemanticQueryClass::Fact(Arc::from("semantic.fact")),
    );
    let observed = rows(
        execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![relation],
            plan,
            &[],
        )
        .await,
    );
    let ids = observed
        .iter()
        .map(|row| Value::String(row_text(row, "fact_id").to_owned()))
        .collect::<Vec<_>>();
    let facts = observed
        .iter()
        .map(|row| canonical_row(row, "record_json"))
        .collect::<Vec<_>>();
    let coverage_state = string(&inputs["producer_coverage"]["state"], "coverage state");
    if case == Case::Causal {
        let before =
            &fixture.as_ref().expect("causal fixture")["mutation"]["before"]["admitted_relations"];
        let before_ids = array(&before["call_edges"], "before edges")
            .iter()
            .map(|row| row["fact_id"].clone())
            .collect::<Vec<_>>();
        let added_fact = facts
            .iter()
            .find(|fact| !before_ids.contains(&fact["fact_id"]))
            .expect("causal added fact");
        let target = string(&added_fact["statement"]["object"], "added target");
        let actual = json!({
            "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": {"starting_from": request["starting_from"],
                "relationship": kind, "direction": request["direction"],
                "distance": request["distance"]},
            "query_result": {"query_id": request["query_id"], "result_role": "facts",
                "fact_ids": ids, "added_fact": added_fact,
                "coverage": {"state": coverage_state, "owner": start,
                    "analysis_context_id": inputs["producer_coverage"]["analysis_context_id"],
                    "distance": request["distance"], "completed_family": kind},
                "errors": [], "notices": [],
                "added_entity": admitted["entity_dictionary"][target]},
            "errors": []
        });
        assert_fault(actual, fixture.as_ref().expect("causal fixture"));
        return;
    }
    if case == Case::Negative {
        assert_eq!(
            json!(ids),
            inputs["producer_coverage"]["covered_fact_ids"],
            "the executed relationship rows must equal the partial-coverage membership"
        );
        let fact_records = facts
            .iter()
            .cloned()
            .map(|record| (string(&record["fact_id"], "fact id").to_owned(), record))
            .collect::<Map<_, _>>();
        let remainders = inputs["producer_coverage"]["remainders"].clone();
        let actual = json!({
            "execution_state": "COMPLETE", "availability_state": "PARTIAL",
            "completeness_state": "PARTIAL", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": {"starting_from": request["starting_from"],
                "relationship": kind, "direction": request["direction"],
                "distance": request["distance"]},
            "query_result": {"query_id": request["query_id"], "result_role": "facts",
                "fact_ids": ids, "facts": fact_records, "remainders": remainders,
                "coverage": {"state": coverage_state, "owner": start,
                    "analysis_context_id": inputs["producer_coverage"]["analysis_context_id"],
                    "completed_family": kind, "distance": request["distance"],
                    "covered_fact_ids": inputs["producer_coverage"]["covered_fact_ids"],
                    "remainders": inputs["producer_coverage"]["remainders"]},
                "errors": [], "notices": []}, "errors": []
        });
        assert_fault(actual, fixture.as_ref().expect("negative fixture"));
        return;
    }
    let actual_query = json!({
        "query_id": request["query_id"], "request": request["request"],
        "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
        "completeness_state": "COMPLETE", "freshness_state": "CURRENT",
        "limit_state": "NOT_APPLIED", "dependency_state": "READY",
        "resolved_semantics": {"starting_from": request["starting_from"],
            "relationship": kind, "direction": request["direction"], "distance": request["distance"]},
        "result_role": "facts", "entity_ids": [], "fact_ids": ids,
        "path_ids": [], "group_ids": [], "source_context_ids": [], "bindings": [],
        "coverage": {"state": coverage_state, "owner": start,
            "analysis_context_id": inputs["producer_coverage"]["analysis_context_id"],
            "distance": request["distance"], "completed_family": kind},
        "provenance": public_provenance(&inputs), "errors": [], "notices": []
    });
    let expected = positive_response(&claim);
    assert_positive_response_common(&inputs, &actual_query, expected);
    let actual_facts = facts
        .into_iter()
        .map(|record| (string(&record["fact_id"], "fact id").to_owned(), record))
        .collect::<Map<_, _>>();
    assert_eq!(Value::Object(actual_facts), expected["facts"]);
    assert_eq!(admitted["entity_dictionary"], expected["entities"]);
}

async fn reachability_depth(
    edges: &[BTreeMap<String, Value>],
    source: &str,
    target: &str,
    maximum_depth: u16,
) -> Option<u32> {
    let edge_schema = Arc::new(Schema::new(vec![
        Field::new("from", DataType::Utf8, false),
        Field::new("to", DataType::Utf8, false),
    ]));
    let batch = RecordBatch::try_new(
        Arc::clone(&edge_schema),
        vec![
            Arc::new(StringArray::from_iter_values(
                edges.iter().map(|edge| row_text(edge, "subject")),
            )),
            Arc::new(StringArray::from_iter_values(
                edges.iter().map(|edge| row_text(edge, "target")),
            )),
        ],
    )
    .expect("authorized path edge batch");
    let context = SessionContext::new();
    context
        .register_table(
            "wp38_path_edges",
            Arc::new(
                MemTable::try_new(Arc::clone(&edge_schema), vec![vec![batch]])
                    .expect("path edge MemTable"),
            ),
        )
        .expect("register path edge relation");
    let input = context
        .table("wp38_path_edges")
        .await
        .expect("resolve path edge relation")
        .into_unoptimized_plan();
    let output_schema = Arc::new(Schema::new(vec![
        Field::new("reachable_from", DataType::Utf8, false),
        Field::new("reachable_to", DataType::Utf8, false),
        Field::new("minimum_depth", DataType::UInt32, false),
    ]));
    let bindings = ReachabilityBindings::try_new(
        "wp38.shortest-path-reachability",
        relation("canonical.call_fact"),
        edge_schema,
        field("from"),
        field("to"),
        relation("query.path_reachability"),
        output_schema,
        field("reachable_from"),
        field("reachable_to"),
        field("minimum_depth"),
        "codefabric.graph.datafusion-55.recursive.v1",
    )
    .expect("typed path reachability bindings");
    let bounds = GraphResourceBounds::try_new(maximum_depth, 128, 32, 1024 * 1024)
        .expect("bounded path reachability");
    let compiled = compile_bounded_reachability(
        GraphRelationInput::new(relation("canonical.call_fact"), input),
        &bindings,
        bounds,
    )
    .expect("native DataFusion reachability plan");
    let executed = compiled
        .execute(&context)
        .await
        .expect("native DataFusion reachability execution");
    executed.batches().iter().find_map(|batch| {
        let sources = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("reachability sources");
        let targets = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("reachability targets");
        let depths = batch
            .column(2)
            .as_any()
            .downcast_ref::<UInt32Array>()
            .expect("reachability depths");
        (0..batch.num_rows())
            .find(|index| sources.value(*index) == source && targets.value(*index) == target)
            .map(|index| depths.value(index))
    })
}

async fn claim_007(case: Case) {
    const CLAIM: &str = "RFV3-CLAIM-007";
    let claim = expectation(CLAIM);
    let (inputs, fixture) = case_inputs(&claim, case);
    validate_v2_ingress(&inputs).expect("released v2 request must pass programmatic ingress");
    let request = query(&inputs);
    let policy = string(&request["path_policy"], "path policy");
    if case == Case::Negative {
        let error = validate_bounded_ordered_path_policy(policy)
            .expect_err("unbounded all-path policy must reject");
        assert!(error.to_string().contains("not a released finite policy"));
        let actual = json!({
            "execution_state": "FAILED", "availability_state": "UNAVAILABLE",
            "completeness_state": "UNAVAILABLE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": {"rejected_path_policy": policy,
                "bounded_alternatives": ["shortest", "all shortest", "simple with explicit bound"]},
            "query_result": {"query_id": request["query_id"], "result_role": "paths",
                "path_ids": [], "coverage": {"state": "NOT_APPLICABLE"},
                "errors": [{"code": "UNBOUNDED_QUERY", "layer": "resource_policy",
                    "retryable": false,
                    "safe_message": "Unrestricted all-path enumeration requires an explicit finite bound.",
                    "field": "path_policy", "semantic_phrase": policy,
                    "candidate_interpretations": ["shortest", "all shortest", "simple with explicit bound"],
                    "failed_dependency_query_id": null, "diagnostic_id": null}], "notices": [],
            },
            "errors": [{"code": "UNBOUNDED_QUERY", "layer": "resource_policy",
                "retryable": false,
                "safe_message": "Unrestricted all-path enumeration requires an explicit finite bound.",
                "field": "path_policy", "semantic_phrase": policy,
                "candidate_interpretations": ["shortest", "all shortest", "simple with explicit bound"],
                "failed_dependency_query_id": null, "diagnostic_id": null}]
        });
        assert_fault(actual, fixture.as_ref().expect("negative fixture"));
        return;
    }
    validate_bounded_ordered_path_policy(policy).expect("released shortest policy");
    let admitted = &inputs["admitted_relations"];
    let relation = edge_relation("007", "canonical.call_fact", &admitted["edges"]);
    let plan = single_plan(
        ReleasedSemanticForm::FindConnectingFactPaths,
        &relation,
        string(&request["query_id"], "query id"),
        &[],
        &["fact_id"],
        None,
        SemanticQueryClass::Fact(Arc::from("semantic.fact")),
    );
    let observed = rows(
        execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![relation],
            plan,
            &[],
        )
        .await,
    );
    let source = string(&request["from"][0], "path source");
    let target = string(&request["to"][0], "path target");
    let maximum_length = u16::try_from(usize_value(
        &inputs["resource_limits"]["max_path_length"],
        "maximum path length",
    ))
    .expect("small path bound");
    let depth = reachability_depth(&observed, source, target, maximum_length)
        .await
        .expect("target reachable through DataFusion graph program");
    let path_edges = observed
        .iter()
        .map(|row| {
            OrderedPathEdge::try_new(
                row_text(row, "fact_id"),
                row_text(row, "subject"),
                row_text(row, "target"),
            )
            .expect("canonical ordered path edge")
        })
        .collect::<Vec<_>>();
    let witness = bounded_shortest_path_witness(
        &path_edges,
        source,
        target,
        OrderedPathBounds::try_new(maximum_length, 128, 128).expect("ordered path bounds"),
    )
    .expect("bounded ordered path execution")
    .expect("shortest witness exists");
    assert_eq!(u32::try_from(witness.length()).expect("path depth"), depth);
    let ordered_entity_ids = witness
        .ordered_entity_ids()
        .iter()
        .map(|id| Value::String(id.to_string()))
        .collect::<Vec<_>>();
    let ordered_fact_ids = witness
        .ordered_fact_ids()
        .iter()
        .map(|id| Value::String(id.to_string()))
        .collect::<Vec<_>>();
    let first_edge = array(&admitted["edges"], "path edges")
        .first()
        .expect("path identity context edge");
    let path_identity = issue_path_result_slot_identity(&PathResultSlotIdentityInput {
        workspace_id: Arc::from(string(&first_edge["workspace_id"], "path workspace")),
        analysis_context_id: Arc::from(string(
            &first_edge["analysis_context_id"],
            "path analysis context",
        )),
        fabric_epoch_id: Arc::from(string(
            &inputs["pinned_epoch"]["fabric_epoch_id"],
            "fabric epoch identity",
        )),
        policy_identity: Arc::from(string(
            &inputs["pinned_epoch"]["policy_release"],
            "path policy identity",
        )),
        ordered_entity_ids: Arc::from(witness.ordered_entity_ids().to_vec()),
        ordered_fact_ids: Arc::from(witness.ordered_fact_ids().to_vec()),
    })
    .expect("production witness-bound path identity");
    let path_id = path_identity.public_id.as_str();
    let path_identity_recipe = path_identity.recipe_evidence();
    let expected = expected_case(&claim, fixture.as_ref());
    let expected_path_id = if case == Case::Positive {
        string(
            &positive_response(&claim)["query_results"][0]["path_ids"][0],
            "path ID",
        )
    } else {
        string(&expected["query_result"]["path_ids"][0], "fault path ID")
    };
    assert_eq!(path_id, expected_path_id);
    let mut path_record = json!({
        "path_id": path_id,
        "ordered_entity_ids": ordered_entity_ids,
        "ordered_fact_ids": ordered_fact_ids,
        "length": witness.length(),
        "path_policy": policy,
        "certainty_summary": "exact",
        "identity_recipe": path_identity_recipe
    });
    if case == Case::Causal {
        let mut causal_path_record = path_record.clone();
        causal_path_record
            .as_object_mut()
            .expect("causal path record object")
            .remove("identity_recipe");
        let actual = json!({
            "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": {"from": request["from"], "to": request["to"],
                "relationship_families": request["using"], "path_policy": policy,
                "maximum_path_length": inputs["resource_limits"]["max_path_length"]},
            "query_result": {"query_id": request["query_id"], "result_role": "paths",
                "path_ids": [path_id], "paths": [causal_path_record],
                "coverage": {"state": "COMPLETE",
                    "searched_fact_count": array(&admitted["edges"], "path edges").len()},
                "errors": [], "notices": [],
                "identity_contract": {"path_id": path_id,
                    "identity_recipe": path_identity_recipe, "witness_bound": true}},
            "errors": []
        });
        assert_eq!(actual, *expected);
        return;
    }
    path_record["supporting_provenance"] = json!({
        "coverage_state": inputs["producer_coverage"]["state"],
        "analysis_context_id": inputs["producer_coverage"]["analysis_context_id"],
        "producer_releases": ["fixture-provider:r1"]
    });
    let actual_query = json!({
        "query_id": request["query_id"], "request": request["request"],
        "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
        "completeness_state": "COMPLETE", "freshness_state": "CURRENT",
        "limit_state": "NOT_APPLIED", "dependency_state": "READY",
        "resolved_semantics": {"from": request["from"], "to": request["to"],
            "relationship_families": request["using"], "path_policy": policy,
            "maximum_path_length": inputs["resource_limits"]["max_path_length"]},
        "result_role": "paths", "entity_ids": [], "fact_ids": [],
        "path_ids": [path_id], "group_ids": [], "source_context_ids": [], "bindings": [],
        "coverage": {"state": "COMPLETE", "graph_projection": "calls@context:source",
            "searched_entity_count": object(&admitted["entity_dictionary"], "entity dictionary").len(),
            "searched_fact_count": array(&admitted["edges"], "path edges").len()},
        "provenance": public_provenance(&inputs), "errors": [], "notices": []
    });
    let positive = positive_response(&claim);
    assert_eq!(actual_query, positive["query_results"][0]);
    // A shortest-path result is proved against the complete searched graph projection,
    // not only by replaying its winning witness. Preserve every programmatically
    // observed admitted fact in the canonical response dictionary so an independently
    // decoded response can verify both the witness and the absence of a shorter route.
    let searched_facts = observed
        .iter()
        .map(|row| {
            let record = canonical_row(row, "record_json");
            let fact_id = string(&record["fact_id"], "observed path fact id").to_owned();
            (fact_id, record)
        })
        .collect::<Map<_, _>>();
    assert_eq!(Value::Object(searched_facts), positive["facts"]);
    assert_eq!(admitted["entity_dictionary"], positive["entities"]);
    let actual_paths = Value::Object(Map::from_iter([(path_id.to_owned(), path_record)]));
    assert_eq!(actual_paths, positive["paths"]);
}

fn pattern_entity_relation(admitted: &Value) -> TestRelation {
    let fields = vec![
        TestField::text("008", "entity", "entity_id"),
        TestField::text("008", "entity", "semantic_kind"),
        TestField::text("008", "entity", "name"),
        TestField::text("008", "entity", "module_id"),
        TestField::text("008", "entity", "record_json"),
    ];
    let rows = array(&admitted["entities"]["rows"], "pattern entities")
        .iter()
        .map(|entity| {
            let qualified = string(&entity["qualified_name"], "qualified name");
            let name = qualified
                .rsplit('.')
                .next()
                .expect("qualified name segment");
            vec![
                text(string(&entity["entity_id"], "entity id")),
                text(string(&entity["semantic_kind"], "semantic kind")),
                text(name),
                text(
                    entity
                        .get("module_id")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                ),
                text(encode(entity)),
            ]
        })
        .collect();
    TestRelation::new("008", "canonical.entity", fields, rows)
}

fn pattern_candidate_relation(binding: &str, rows: &[BTreeMap<String, Value>]) -> TestRelation {
    let relation_name = format!("query.pattern_{binding}_candidate");
    let fields = vec![
        TestField::text("008", &relation_name, "entity_id"),
        TestField::text("008", &relation_name, "semantic_kind"),
        TestField::text("008", &relation_name, "name"),
        TestField::text("008", &relation_name, "module_id"),
        TestField::text("008", &relation_name, "record_json"),
    ];
    TestRelation::new(
        "008",
        &relation_name,
        fields,
        rows.iter()
            .map(|row| {
                vec![
                    text(row_text(row, "entity_id")),
                    text(row_text(row, "semantic_kind")),
                    text(row_text(row, "name")),
                    text(row_text(row, "module_id")),
                    text(row_text(row, "record_json")),
                ]
            })
            .collect(),
    )
}

fn pattern_call_relation(admitted: &Value) -> TestRelation {
    let relation_name = "canonical.call_fact";
    let fields = vec![
        TestField::text("008", relation_name, "fact_id"),
        TestField::text("008", relation_name, "subject"),
        TestField::text("008", relation_name, "target"),
        TestField::text("008", relation_name, "fact_kind"),
        TestField::text("008", relation_name, "analysis_context_id"),
        TestField::text("008", relation_name, "subject_module_id"),
        TestField::text("008", relation_name, "record_json"),
    ];
    let dictionary = object(&admitted["entity_dictionary"], "pattern entity dictionary");
    let rows = array(&admitted["call_edges"], "pattern call edges")
        .iter()
        .map(|edge| {
            let subject = string(&edge["statement"]["subject"], "edge subject");
            let subject_record = dictionary
                .get(subject)
                .unwrap_or_else(|| panic!("pattern edge subject {subject} lacks an entity record"));
            vec![
                text(string(&edge["fact_id"], "edge fact id")),
                text(subject),
                text(string(&edge["statement"]["object"], "edge target")),
                text(string(&edge["fact_kind"], "edge kind")),
                text(string(
                    &edge["analysis_context_id"],
                    "edge analysis context",
                )),
                text(string(&subject_record["module_id"], "edge subject module")),
                text(encode(edge)),
            ]
        })
        .collect();
    TestRelation::new("008", relation_name, fields, rows)
}

fn observed_pattern_call_relation(name: &str, rows: &[BTreeMap<String, Value>]) -> TestRelation {
    let fields = vec![
        TestField::text("008", name, "fact_id"),
        TestField::text("008", name, "subject"),
        TestField::text("008", name, "target"),
        TestField::text("008", name, "fact_kind"),
        TestField::text("008", name, "analysis_context_id"),
        TestField::text("008", name, "subject_module_id"),
        TestField::text("008", name, "record_json"),
    ];
    TestRelation::new(
        "008",
        name,
        fields,
        rows.iter()
            .map(|row| {
                vec![
                    text(row_text(row, "fact_id")),
                    text(row_text(row, "subject")),
                    text(row_text(row, "target")),
                    text(row_text(row, "fact_kind")),
                    text(row_text(row, "analysis_context_id")),
                    text(row_text(row, "subject_module_id")),
                    text(row_text(row, "record_json")),
                ]
            })
            .collect(),
    )
}

fn pattern_resolved_semantics(pattern: &Value, coverage: &Value) -> Value {
    let typed_bindings = array(&pattern["nodes"], "pattern nodes")
        .iter()
        .map(|node| {
            (
                string(&node["binding"], "pattern binding").to_owned(),
                node["semantic_kind"].clone(),
            )
        })
        .collect::<Map<_, _>>();
    json!({
        "pattern_id": "pattern:typed-edge-no-outgoing-call-v1",
        "typed_bindings": typed_bindings,
        "positive_fact_count": array(&pattern["facts"], "pattern facts").len(),
        "scoped_negation_universe": coverage["negative_proof_universe_id"],
    })
}

fn pattern_result_coverage(coverage: &Value, outcome: Option<&str>) -> Value {
    let mut result = json!({
        "state": coverage["state"],
        "owner_scope": coverage["owner_scope"],
        "analysis_context_id": coverage["analysis_context_id"],
        "family": coverage["family"],
        "covered_subject_ids": coverage["covered_subject_ids"],
        "covered_fact_ids": coverage["covered_fact_ids"],
        "negative_proof_universe_id": coverage["negative_proof_universe_id"],
    });
    if let Some(outcome) = outcome {
        result["outcome"] = Value::String(outcome.to_owned());
    } else {
        result["remainders"] = coverage["remainders"].clone();
    }
    result
}

fn pattern_binding(
    f_row: &BTreeMap<String, Value>,
    g_row: &BTreeMap<String, Value>,
    supporting_rows: &[&BTreeMap<String, Value>],
    negation: &Value,
    coverage: &Value,
    binding_state: &str,
) -> Value {
    let f_record = canonical_row(f_row, "record_json");
    let g_record = canonical_row(g_row, "record_json");
    let negation_state = if binding_state == "MATCH" {
        "PROVED_ABSENT"
    } else {
        "INDETERMINATE"
    };
    json!({
        "matched_branch": "primary",
        "binding_state": binding_state,
        "bindings": {
            "f": {
                "binding_type": "entity:function",
                "entity_id": f_record["entity_id"],
                "semantic_kind": f_record["semantic_kind"],
            },
            "g": {
                "binding_type": "entity:function",
                "entity_id": g_record["entity_id"],
                "semantic_kind": g_record["semantic_kind"],
            },
        },
        "supporting_fact_ids": supporting_rows
            .iter()
            .map(|row| row["fact_id"].clone())
            .collect::<Vec<_>>(),
        "scoped_negation": [{
            "subject_binding": negation["subject_binding"],
            "subject_entity_id": f_record["entity_id"],
            "relationship": negation["relationship"],
            "direction": negation["direction"],
            "owner_scope": negation["owner_scope"],
            "analysis_context_id": negation["analysis_context_id"],
            "coverage_witness": coverage["negative_proof_universe_id"],
            "state": negation_state,
        }],
    })
}

async fn claim_008(case: Case) {
    const CLAIM: &str = "RFV3-CLAIM-008";
    let claim = expectation(CLAIM);
    let (inputs, fixture) = case_inputs(&claim, case);
    validate_v2_ingress(&inputs).expect("released v2 pattern request must pass ingress");
    let request = query(&inputs);
    let admitted = &inputs["admitted_relations"];
    let pattern = &request["pattern"];
    assert_eq!(inputs["program_binding"]["pattern_contract"], *pattern);
    let nodes = array(&pattern["nodes"], "pattern nodes");
    let facts = array(&pattern["facts"], "pattern facts");
    let negations = array(&pattern["scoped_negation"], "pattern scoped negation");
    assert_eq!(nodes.len(), 2, "Claim 008 exercises two typed nodes");
    assert_eq!(facts.len(), 1, "Claim 008 exercises one positive edge");
    assert_eq!(
        negations.len(),
        1,
        "Claim 008 exercises one scoped negation"
    );
    assert_eq!(pattern["alternatives"], json!([]));
    let f_node = nodes
        .iter()
        .find(|node| node["binding"] == "f")
        .expect("typed f node");
    let g_node = nodes
        .iter()
        .find(|node| node["binding"] == "g")
        .expect("typed g node");
    let positive_fact = &facts[0];
    let negation = &negations[0];
    assert_eq!(positive_fact["subject_binding"], "g");
    assert_eq!(positive_fact["object_binding"], "f");
    assert_eq!(positive_fact["direction"], "outgoing");
    assert_eq!(negation["subject_binding"], "f");
    assert_eq!(negation["direction"], "outgoing");
    assert_eq!(negation["required_coverage"], "COMPLETE");

    let coverage = &inputs["producer_coverage"];
    assert_eq!(coverage["family"], negation["relationship"]);
    assert_eq!(coverage["owner_scope"], negation["owner_scope"]);
    assert_eq!(
        coverage["analysis_context_id"],
        negation["analysis_context_id"]
    );
    assert_eq!(
        coverage["state"],
        if case == Case::Negative {
            "PARTIAL"
        } else {
            "COMPLETE"
        }
    );
    let mut covered_subject_ids = array(&admitted["entities"]["rows"], "pattern entities")
        .iter()
        .filter(|entity| {
            entity["module_id"] == negation["owner_scope"]
                && entity["analysis_context_id"] == negation["analysis_context_id"]
                && entity["semantic_kind"] == f_node["semantic_kind"]
        })
        .map(|entity| entity["entity_id"].clone())
        .collect::<Vec<_>>();
    covered_subject_ids.sort_by(|left, right| {
        string(left, "covered subject").cmp(string(right, "covered subject"))
    });
    assert_eq!(json!(covered_subject_ids), coverage["covered_subject_ids"]);
    let mut covered_fact_ids = array(&admitted["call_edges"], "pattern call edges")
        .iter()
        .filter(|edge| {
            edge["analysis_context_id"] == negation["analysis_context_id"]
                && edge["statement"]["predicate"] == negation["relationship"]
                && array(&coverage["covered_subject_ids"], "covered subjects")
                    .contains(&edge["statement"]["subject"])
        })
        .map(|edge| edge["fact_id"].clone())
        .collect::<Vec<_>>();
    covered_fact_ids
        .sort_by(|left, right| string(left, "covered fact").cmp(string(right, "covered fact")));
    assert_eq!(json!(covered_fact_ids), coverage["covered_fact_ids"]);

    let entity_input = pattern_entity_relation(admitted);
    let f_rows = rows(
        execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![entity_input.clone()],
            single_plan(
                ReleasedSemanticForm::MatchCodeFactPattern,
                &entity_input,
                string(&request["query_id"], "query id"),
                &[
                    (
                        "semantic_kind",
                        string(&f_node["semantic_kind"], "f semantic kind"),
                    ),
                    ("name", string(&f_node["name"], "f name")),
                    ("module_id", string(&f_node["module_id"], "f module")),
                ],
                &["entity_id"],
                None,
                SemanticQueryClass::Fact(Arc::from("semantic.fact")),
            ),
            &[],
        )
        .await,
    );
    let g_rows = rows(
        execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![entity_input.clone()],
            single_plan(
                ReleasedSemanticForm::MatchCodeFactPattern,
                &entity_input,
                string(&request["query_id"], "query id"),
                &[
                    (
                        "semantic_kind",
                        string(&g_node["semantic_kind"], "g semantic kind"),
                    ),
                    ("name", string(&g_node["name"], "g name")),
                    ("module_id", string(&g_node["module_id"], "g module")),
                ],
                &["entity_id"],
                None,
                SemanticQueryClass::Fact(Arc::from("semantic.fact")),
            ),
            &[],
        )
        .await,
    );
    assert_eq!(f_rows.len(), 1, "exactly one typed f candidate");
    assert_eq!(g_rows.len(), 1, "exactly one typed g candidate");
    let f_candidates = pattern_candidate_relation("f", &f_rows);
    let g_id = row_text(&g_rows[0], "entity_id");

    // The coverage scope is a semantic input to negation. Execute its owner/family/context
    // restriction before either the positive edge join or the left-anti absence check.
    let calls = pattern_call_relation(admitted);
    let scoped_call_rows = rows(
        execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![calls.clone()],
            single_plan(
                ReleasedSemanticForm::MatchCodeFactPattern,
                &calls,
                string(&request["query_id"], "query id"),
                &[
                    (
                        "fact_kind",
                        string(&positive_fact["relationship"], "positive fact kind"),
                    ),
                    (
                        "analysis_context_id",
                        string(&coverage["analysis_context_id"], "coverage context"),
                    ),
                    (
                        "subject_module_id",
                        string(&coverage["owner_scope"], "coverage owner scope"),
                    ),
                ],
                &["subject", "target", "fact_id"],
                None,
                SemanticQueryClass::Fact(Arc::from("semantic.fact")),
            ),
            &[],
        )
        .await,
    );
    let mut executed_covered_fact_ids = scoped_call_rows
        .iter()
        .map(|row| row["fact_id"].clone())
        .collect::<Vec<_>>();
    executed_covered_fact_ids.sort_by(|left, right| {
        string(left, "executed covered fact").cmp(string(right, "executed covered fact"))
    });
    assert_eq!(
        executed_covered_fact_ids,
        array(&coverage["covered_fact_ids"], "covered fact ids")
    );
    let scoped_calls = observed_pattern_call_relation("query.scoped_call_fact", &scoped_call_rows);
    let g_call_rows = rows(
        execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![scoped_calls.clone()],
            single_plan(
                ReleasedSemanticForm::MatchCodeFactPattern,
                &scoped_calls,
                string(&request["query_id"], "query id"),
                &[("subject", g_id)],
                &["target", "fact_id"],
                None,
                SemanticQueryClass::Fact(Arc::from("semantic.fact")),
            ),
            &[],
        )
        .await,
    );
    assert!(
        !g_call_rows.is_empty(),
        "the typed g candidate must have an admitted outgoing fact"
    );
    let g_calls = observed_pattern_call_relation("query.g_call_fact", &g_call_rows);
    let positive_edge_plan = binary_join_plan(
        ReleasedSemanticForm::MatchCodeFactPattern,
        &f_candidates,
        &g_calls,
        JoinKind::LeftSemi,
        "entity_id",
        "target",
        &["entity_id"],
    );
    let bound_f_rows = rows(
        execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![f_candidates, g_calls],
            positive_edge_plan,
            &[],
        )
        .await,
    );
    assert_eq!(
        bound_f_rows.len(),
        1,
        "the positive g-calls-f typed edge must bind f"
    );
    let f_id = row_text(&bound_f_rows[0], "entity_id");
    let mut supporting_rows = g_call_rows
        .iter()
        .filter(|row| row_text(row, "target") == f_id)
        .collect::<Vec<_>>();
    supporting_rows.sort_by_key(|row| row_text(row, "fact_id"));
    assert!(
        !supporting_rows.is_empty(),
        "the typed binding must retain its supporting fact"
    );
    let resolved_semantics = pattern_resolved_semantics(pattern, coverage);

    if case == Case::Negative {
        // Positive typed bindings remain executable. Partial producer coverage only makes the
        // scoped absence clause indeterminate; it does not erase known nodes or supporting facts.
        let binding = pattern_binding(
            &bound_f_rows[0],
            &g_rows[0],
            &supporting_rows,
            negation,
            coverage,
            "INDETERMINATE",
        );
        let f_record = canonical_row(&bound_f_rows[0], "record_json");
        let g_record = canonical_row(&g_rows[0], "record_json");
        let mut entity_ids = vec![f_record["entity_id"].clone(), g_record["entity_id"].clone()];
        entity_ids.sort_by(|left, right| {
            string(left, "pattern entity").cmp(string(right, "pattern entity"))
        });
        let entity_records = Map::from_iter([
            (
                string(&f_record["entity_id"], "f entity id").to_owned(),
                f_record,
            ),
            (
                string(&g_record["entity_id"], "g entity id").to_owned(),
                g_record,
            ),
        ]);
        let facts = supporting_rows
            .iter()
            .map(|row| {
                (
                    row_text(row, "fact_id").to_owned(),
                    canonical_row(row, "record_json"),
                )
            })
            .collect::<Map<_, _>>();
        let error = json!({
            "code": "NEGATIVE_PROOF_INDETERMINATE", "layer": "coverage",
            "safe_message": "Scoped negation requires complete owner, family, context, subject, and fact coverage.",
            "field": "pattern.scoped_negation", "semantic_phrase": null,
            "candidate_interpretations": [], "retryable": true,
            "failed_dependency_query_id": null, "diagnostic_id": null
        });
        let actual = json!({
            "execution_state": "COMPLETE", "availability_state": "PARTIAL",
            "completeness_state": "INDETERMINATE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": resolved_semantics,
            "query_result": {"query_id": request["query_id"], "result_role": "pattern_bindings",
                "bindings": [binding], "entity_ids": entity_ids,
                "fact_ids": facts.keys().cloned().collect::<Vec<_>>(),
                "entity_records": entity_records, "facts": facts,
                "remainders": coverage["remainders"],
                "coverage": pattern_result_coverage(coverage, None),
                "errors": [error.clone()], "notices": []},
            "errors": [error]
        });
        assert_fault(actual, fixture.as_ref().expect("negative fixture"));
        return;
    }

    let bound_f = pattern_candidate_relation("bound_f", &bound_f_rows);
    let anti_rows = rows(
        execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![bound_f.clone(), scoped_calls.clone()],
            binary_join_plan(
                ReleasedSemanticForm::MatchCodeFactPattern,
                &bound_f,
                &scoped_calls,
                JoinKind::LeftAnti,
                "entity_id",
                "subject",
                &["entity_id"],
            ),
            &[],
        )
        .await,
    );
    if case == Case::Causal {
        assert!(
            anti_rows.is_empty(),
            "the added f outgoing edge must invalidate scoped negation"
        );
        let actual = json!({
            "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": resolved_semantics,
            "query_result": {"query_id": request["query_id"], "result_role": "pattern_bindings",
                "bindings": [], "entity_ids": [], "fact_ids": [],
                "evaluated_fact_ids": coverage["covered_fact_ids"],
                "coverage": pattern_result_coverage(coverage, Some("NO_MATCH_AFTER_FILTERS")),
                "errors": [], "notices": []}, "errors": []
        });
        assert_fault(actual, fixture.as_ref().expect("causal fixture"));
        return;
    }

    assert_eq!(anti_rows.len(), 1, "complete scoped negation must retain f");
    let binding = pattern_binding(
        &anti_rows[0],
        &g_rows[0],
        &supporting_rows,
        negation,
        coverage,
        "MATCH",
    );
    let f_record = canonical_row(&anti_rows[0], "record_json");
    let g_record = canonical_row(&g_rows[0], "record_json");
    let mut entity_ids = vec![f_record["entity_id"].clone(), g_record["entity_id"].clone()];
    entity_ids
        .sort_by(|left, right| string(left, "pattern entity").cmp(string(right, "pattern entity")));
    let fact_ids = supporting_rows
        .iter()
        .map(|row| row["fact_id"].clone())
        .collect::<Vec<_>>();
    let actual_query = json!({
        "query_id": request["query_id"], "request": request["request"],
        "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
        "completeness_state": "COMPLETE", "freshness_state": "CURRENT",
        "limit_state": "NOT_APPLIED", "dependency_state": "READY",
        "resolved_semantics": resolved_semantics,
        "result_role": "pattern_bindings", "entity_ids": entity_ids, "fact_ids": fact_ids,
        "path_ids": [], "group_ids": [], "source_context_ids": [],
        "bindings": [binding],
        "coverage": pattern_result_coverage(coverage, Some("MATCH")),
        "provenance": public_provenance(&inputs),
        "errors": [], "notices": []
    });
    let expected = positive_response(&claim);
    assert_positive_response_common(&inputs, &actual_query, expected);
    assert_eq!(admitted["entity_dictionary"], expected["entities"]);
    let executed_facts = scoped_call_rows
        .iter()
        .map(|row| {
            (
                row_text(row, "fact_id").to_owned(),
                canonical_row(row, "record_json"),
            )
        })
        .collect::<Map<_, _>>();
    assert_eq!(Value::Object(executed_facts), expected["facts"]);
}

fn producer_result_compatibility(query_id: &str, value: &Value) -> SemanticResultCompatibility {
    SemanticResultCompatibility {
        query_id: Arc::from(query_id.to_owned()),
        workspace_id: Arc::from(string(&value["workspace_id"], "upstream workspace")),
        analysis_context_id: Arc::from(string(
            &value["analysis_context_id"],
            "upstream analysis context",
        )),
        representation_layer: Arc::from(string(
            &value["representation_layer"],
            "upstream representation",
        )),
        certainty_class: Arc::from(string(&value["certainty_class"], "upstream certainty")),
        semantic_role: Arc::from(string(&value["semantic_role"], "upstream role")),
    }
}

fn upstream_entity_relation(
    claim_slug: &str,
    relation_id: &str,
    input: &Value,
    dictionary: &Value,
) -> TestRelation {
    let fields = vec![
        TestField::text(claim_slug, relation_id, "entity_id"),
        TestField::text(claim_slug, relation_id, "record_json"),
    ];
    let rows = array(&input["rows"], "upstream entity rows")
        .iter()
        .map(|identity| {
            let identity = string(identity, "upstream entity identity");
            vec![text(identity), text(encode(&dictionary[identity]))]
        })
        .collect();
    TestRelation::new(claim_slug, relation_id, fields, rows)
}

async fn claim_009(case: Case) {
    const CLAIM: &str = "RFV3-CLAIM-009";
    let claim = expectation(CLAIM);
    let (inputs, fixture) = case_inputs(&claim, case);
    validate_v2_ingress(&inputs).expect("released v2 combination request must pass ingress");
    let queries = array(
        &inputs["request_envelope"]["decoded"]["queries"],
        "combination query blocks",
    );
    let request = queries
        .iter()
        .find(|block| block["request"] == "combine result sets")
        .expect("one combine-result-sets block");
    let selections = array(&request["inputs"], "combine prior-result selections");
    assert_eq!(
        queries
            .iter()
            .map(|block| string(&block["query_id"], "topological query id"))
            .collect::<Vec<_>>(),
        ["left", "right", "q-combine"],
        "producer blocks must precede their combination consumer"
    );
    let producer_query_ids = selections
        .iter()
        .map(|selection| string(&selection["results_of"], "producer query id"))
        .collect::<Vec<_>>();
    assert_eq!(producer_query_ids, ["left", "right"]);
    assert!(
        selections
            .iter()
            .all(|selection| selection["select"] == "entities"),
        "combination inputs must select typed entity results"
    );
    assert_eq!(
        inputs["access_scope"]["input_results"],
        json!(producer_query_ids),
        "authorization must name the same upstream results as the typed DAG"
    );

    let admitted = &inputs["admitted_relations"];
    assert!(
        admitted.get("producer_results").is_none(),
        "preauthored producer outputs are circular authority"
    );
    let producer_inputs = object(&admitted["producer_inputs"], "producer base inputs");
    let missing_producers = producer_query_ids
        .iter()
        .copied()
        .filter(|query_id| !producer_inputs.contains_key(*query_id))
        .collect::<Vec<_>>();

    if case == Case::Negative {
        assert_eq!(
            missing_producers,
            ["right"],
            "negative fixture must remove the independently derived right producer"
        );
        let fault = json!({
            "code": "DANGLING_RESULT_REFERENCE", "layer": "binding",
            "retryable": false,
            "safe_message": "The referenced prior query result is absent.",
            "field": "inputs", "semantic_phrase": null, "candidate_interpretations": [],
            "failed_dependency_query_id": missing_producers[0], "diagnostic_id": null
        });
        let actual = json!({
            "execution_state": "NOT_EXECUTED_DEPENDENCY", "availability_state": "UNAVAILABLE",
            "completeness_state": "UNAVAILABLE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "FAILED_DEPENDENCY",
            "resolved_semantics": {"operation": request["operation"],
                "dangling_result_reference": missing_producers[0]},
            "query_result": {"query_id": request["query_id"], "result_role": "entities",
                "entity_ids": [], "coverage": {"state": "NOT_APPLICABLE"},
                "errors": [fault.clone()], "notices": []}, "errors": [fault]
        });
        assert_fault(actual, fixture.as_ref().expect("negative fixture"));
        return;
    }
    assert!(
        missing_producers.is_empty(),
        "every results_of dependency must have an independent producer input"
    );

    let dictionary = &admitted["entity_dictionary"];
    let provenance = public_provenance(&inputs);
    let mut producer_results = Map::new();
    let mut result_relations = Vec::new();
    let mut compatibilities = Vec::new();
    for query_id in &producer_query_ids {
        let block = queries
            .iter()
            .find(|candidate| candidate["query_id"] == **query_id)
            .unwrap_or_else(|| panic!("producer block {query_id} is absent"));
        assert_eq!(block["request"], "find code entities");
        assert_eq!(block["looking_for"], "function declarations");
        let producer_input = &producer_inputs[*query_id];
        let relation_id = string(
            &producer_input["relation_id"],
            "producer base relation identity",
        );
        assert_eq!(
            block["where"],
            json!([{"relation": relation_id, "predicate": "member"}]),
            "producer block must read its independent admitted base relation"
        );
        assert_eq!(
            block["within"],
            json!([producer_input["workspace_id"].clone()]),
            "producer block workspace must match its base relation"
        );

        let base_relation =
            upstream_entity_relation("009", relation_id, producer_input, dictionary);
        let maximum_results = usize_value(
            &block["return"]["limit"]["maximum_results"],
            "producer result limit",
        );
        let producer_plan = single_plan(
            ReleasedSemanticForm::FindCodeEntities,
            &base_relation,
            query_id,
            &[],
            &["entity_id"],
            Some(maximum_results),
            SemanticQueryClass::Fact(Arc::from("semantic.fact")),
        );
        let observed =
            rows(execute_plan(CLAIM, query_id, vec![base_relation], producer_plan, &[]).await);
        let entity_ids = observed
            .iter()
            .map(|row| Value::String(row_text(row, "entity_id").to_owned()))
            .collect::<Vec<_>>();
        assert!(
            entity_ids.len() <= maximum_results,
            "producer output must honor its declared result limit"
        );
        let result_relation_name = format!("query.result.{query_id}");
        let result_relation = TestRelation::new(
            "009",
            &result_relation_name,
            vec![
                TestField::text("009", &result_relation_name, "entity_id"),
                TestField::text("009", &result_relation_name, "record_json"),
            ],
            observed
                .iter()
                .map(|row| {
                    vec![
                        text(row_text(row, "entity_id")),
                        text(row_text(row, "record_json")),
                    ]
                })
                .collect(),
        );
        let compatibility = producer_result_compatibility(query_id, producer_input);
        let result = json!({
            "query_id": query_id, "request": block["request"],
            "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": {"looking_for": block["looking_for"],
                "producer_input_relation": relation_id,
                "compatibility_dimensions": {
                    "workspace_id": producer_input["workspace_id"],
                    "analysis_context_id": producer_input["analysis_context_id"],
                    "representation_layer": producer_input["representation_layer"],
                    "certainty_class": producer_input["certainty_class"],
                    "semantic_role": producer_input["semantic_role"]}},
            "result_role": "entities", "entity_ids": entity_ids, "fact_ids": [],
            "path_ids": [], "group_ids": [], "source_context_ids": [], "bindings": [],
            "coverage": {"state": "COMPLETE", "producer_query_id": query_id,
                "producer_input_relation": relation_id,
                "completed_entities": observed.len()},
            "provenance": provenance, "errors": [], "notices": []
        });
        assert!(
            producer_results
                .insert((*query_id).to_owned(), result)
                .is_none(),
            "producer query identities must be unique"
        );
        result_relations.push(result_relation);
        compatibilities.push(compatibility);
    }
    let [left, right] = result_relations.as_slice() else {
        panic!("intersection proof requires exactly two producer results")
    };
    let [left_compatibility, right_compatibility] = compatibilities.as_slice() else {
        panic!("intersection proof requires exactly two compatibility envelopes")
    };
    validate_semantic_result_compatibility(left_compatibility, right_compatibility)
        .expect("compatible independently produced results");

    let expected_edges = producer_query_ids
        .iter()
        .map(|query_id| {
            json!({"producer_query_id": query_id, "consumer_query_id": request["query_id"],
                "selection": "entities"})
        })
        .collect::<Vec<_>>();
    assert_eq!(
        admitted["dependency_edges"],
        Value::Array(expected_edges),
        "typed dependency edges must close the requested producer DAG"
    );
    let plan = binary_join_plan(
        ReleasedSemanticForm::CombineResultSets,
        left,
        right,
        JoinKind::LeftSemi,
        "entity_id",
        "entity_id",
        &["entity_id"],
    );
    let observed = rows(
        execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            result_relations,
            plan,
            &[],
        )
        .await,
    );
    let entity_ids = observed
        .iter()
        .map(|row| Value::String(row_text(row, "entity_id").to_owned()))
        .collect::<Vec<_>>();
    let records = observed
        .iter()
        .map(|row| canonical_row(row, "record_json"))
        .collect::<Vec<_>>();
    if case == Case::Causal {
        let entity_records = records
            .into_iter()
            .map(|record| (string(&record["entity_id"], "entity id").to_owned(), record))
            .collect::<Map<_, _>>();
        let actual = json!({
            "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "SATISFIED",
            "resolved_semantics": {"operation": request["operation"],
                "producer_query_ids": producer_query_ids, "compatibility": "equal"},
            "query_result": {"query_id": request["query_id"], "result_role": "entities",
                "entity_ids": entity_ids, "upstream_query_ids": producer_query_ids,
                "producer_results": producer_results,
                "coverage": {"state": "COMPLETE"},
                "errors": [], "notices": [], "entity_records": entity_records}, "errors": []
        });
        assert_fault(actual, fixture.as_ref().expect("causal fixture"));
        return;
    }

    let compatibility_dimensions = &inputs["program_binding"]["compatibility_dimensions"];
    assert_eq!(
        compatibility_dimensions,
        &json!([
            "workspace_id",
            "analysis_context_id",
            "representation_layer",
            "certainty_class",
            "semantic_role"
        ])
    );
    let actual_query = json!({
        "query_id": request["query_id"], "request": request["request"],
        "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
        "completeness_state": "COMPLETE", "freshness_state": "CURRENT",
        "limit_state": "NOT_APPLIED", "dependency_state": "READY",
        "resolved_semantics": {"operation": request["operation"], "inputs": request["inputs"],
            "compatibility_dimensions": {"workspace_id": left_compatibility.workspace_id,
                "analysis_context_id": left_compatibility.analysis_context_id,
                "representation_layer": left_compatibility.representation_layer,
                "certainty_class": left_compatibility.certainty_class,
                "semantic_role": left_compatibility.semantic_role}},
        "result_role": "entities", "entity_ids": entity_ids, "fact_ids": [], "path_ids": [],
        "group_ids": [], "source_context_ids": [], "bindings": [],
        "coverage": {"state": "COMPLETE", "upstream_results": producer_query_ids,
            "producer_result_identities_preserved": true},
        "provenance": public_provenance(&inputs), "errors": [], "notices": []
    });
    let expected = positive_response(&claim);
    let actual_results = producer_query_ids
        .iter()
        .map(|query_id| producer_results[*query_id].clone())
        .chain(std::iter::once(actual_query))
        .collect::<Vec<_>>();
    assert_eq!(Value::Array(actual_results), expected["query_results"]);
    assert_eq!(
        expected["specification"],
        "composable semantic CPG fact query response"
    );
    assert_eq!(
        expected["version"],
        inputs["request_envelope"]["decoded"]["version"]
    );
    assert_eq!(
        expected["semantic_request_id"],
        inputs["request_envelope"]["decoded"]["semantic_request_id"]
    );
    assert_eq!(
        expected["snapshot"],
        inputs["pinned_epoch"]["public_snapshot_projection"]
    );
    assert_eq!(expected["successful_query_count"], 3);
    assert_eq!(expected["failed_query_count"], 0);
    assert_eq!(expected["not_executed_dependency_count"], 0);
    assert_eq!(expected["errors"], json!([]));
    assert_eq!(admitted["entity_dictionary"], expected["entities"]);
}

fn syntax_fact_relation(admitted: &Value) -> TestRelation {
    let fields = vec![
        TestField::text("010", "syntax", "native_kind"),
        TestField::int64("010", "syntax", "count_seed"),
    ];
    let rows = array(&admitted["syntax_rows"], "syntax rows")
        .iter()
        .map(|fact| {
            vec![
                text(string(&fact["statement"]["object"], "native kind")),
                int64(1),
            ]
        })
        .collect();
    TestRelation::new("010", "canonical.syntax_fact", fields, rows)
}

fn objective_input_set_identity(inputs: &Value) -> CanonicalPublicIdentity {
    let admitted = &inputs["admitted_relations"];
    let syntax_rows = array(&admitted["syntax_rows"], "summary syntax rows");
    let first = syntax_rows
        .first()
        .expect("objective input-set membership must be non-empty");
    let workspace_id = string(&first["workspace_id"], "summary workspace");
    assert!(
        syntax_rows
            .iter()
            .all(|row| row["workspace_id"] == workspace_id),
        "objective input-set membership must belong to one workspace"
    );

    let coverage_state = match string(&admitted["coverage_state"], "summary coverage state") {
        "complete" => ObjectiveInputCoverageState::Complete,
        "partial" => ObjectiveInputCoverageState::Partial,
        "indeterminate" => ObjectiveInputCoverageState::Indeterminate,
        "unavailable" => ObjectiveInputCoverageState::Unavailable,
        other => panic!("unsupported objective input-set coverage state {other}"),
    };
    let identity = issue_objective_input_set_identity(&ObjectiveInputSetIdentityInput {
        workspace_id: Arc::from(workspace_id),
        analysis_context_ids: syntax_rows
            .iter()
            .map(|row| {
                Arc::<str>::from(string(
                    &row["analysis_context_id"],
                    "summary analysis context",
                ))
            })
            .collect::<Vec<_>>()
            .into(),
        fact_ids: syntax_rows
            .iter()
            .map(|row| Arc::<str>::from(string(&row["fact_id"], "summary fact identity")))
            .collect::<Vec<_>>()
            .into(),
        producer_identities: syntax_rows
            .iter()
            .map(|row| {
                Arc::<str>::from(string(
                    &row["producer"]["producer_id"],
                    "summary producer identity",
                ))
            })
            .collect::<Vec<_>>()
            .into(),
        policy_identity: Arc::from(string(
            &inputs["pinned_epoch"]["policy_release"],
            "summary policy identity",
        )),
        coverage_state,
    })
    .expect("production objective input-set identity");

    assert_eq!(
        identity.recipe_evidence(),
        admitted["input_set_identity"],
        "frozen domain-19 evidence must equal the identity derived from admitted membership"
    );
    assert!(
        syntax_rows.iter().all(|row| {
            string(
                &row["direct_provenance"]["input_set_id"],
                "summary fact input-set identity",
            ) == identity.public_id
        }),
        "each admitted fact must retain the derived objective input-set identity"
    );
    identity
}

fn objective_group_identity(
    inputs: &Value,
    request: &Value,
    input_set_id: &str,
    kind: &str,
) -> CanonicalPublicIdentity {
    let admitted = &inputs["admitted_relations"];
    let first = array(&admitted["syntax_rows"], "summary syntax rows")
        .first()
        .expect("summary identity input row");
    issue_objective_group_identity(&ObjectiveGroupIdentityInput {
        workspace_id: Arc::from(string(&first["workspace_id"], "summary workspace")),
        analysis_context_id: Arc::from(string(
            &first["analysis_context_id"],
            "summary analysis context",
        )),
        input_set_id: Arc::from(input_set_id),
        grouping_dimensions: Arc::from(
            array(&request["group_by"], "summary grouping dimensions")
                .iter()
                .map(|value| Arc::<str>::from(string(value, "summary grouping dimension")))
                .collect::<Vec<_>>(),
        ),
        canonical_group_key: BTreeMap::from([(
            Arc::from("native_kind"),
            ObjectiveGroupScalar::Text(Arc::from(kind)),
        )]),
        aggregate_function: Arc::from("count"),
        measure: Arc::from(string(&request["measure"], "summary measure")),
        producer_id: Arc::from(string(
            &first["producer"]["producer_id"],
            "summary producer",
        )),
    })
    .expect("production objective-group identity")
}

fn support_fact_ids(admitted: &Value, kind: &str) -> Vec<Value> {
    let mut ids = array(&admitted["syntax_rows"], "syntax rows")
        .iter()
        .filter(|fact| fact["statement"]["object"] == kind)
        .map(|fact| fact["fact_id"].clone())
        .collect::<Vec<_>>();
    ids.sort_by(|left, right| string(left, "fact id").cmp(string(right, "fact id")));
    ids
}

async fn claim_010(case: Case) {
    const CLAIM: &str = "RFV3-CLAIM-010";
    let claim = expectation(CLAIM);
    let (inputs, fixture) = case_inputs(&claim, case);
    let request = query(&inputs);
    let ingress = validate_v2_ingress(&inputs);
    let relation = syntax_fact_relation(&inputs["admitted_relations"]);
    let mut plan = aggregate_plan(&relation, "native_kind", "count_seed");
    if case == Case::Negative {
        assert!(
            ingress
                .expect_err("evaluative measure must fail at v2 ingress")
                .contains("evaluative intent")
        );
        plan.semantic_class = SemanticQueryClass::Judgment(Arc::from(string(
            &request["measure"],
            "rejected measure",
        )));
        let rejected = execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![relation],
            plan,
            &[],
        )
        .await;
        assert!(
            execution_rows(rejected)
                .expect_err("judgment aggregate must reject")
                .contains("evaluative or judgment semantics")
        );
        let measure = &request["measure"];
        let fault = json!({
            "code": "NOT_OBJECTIVE_FACT_REQUEST", "layer": "semantic_resolution",
            "retryable": false,
            "safe_message": "Evaluative risk labels are not objective fact summaries.",
            "field": "measure", "semantic_phrase": measure,
            "candidate_interpretations": ["count grouped by native_kind"],
            "failed_dependency_query_id": null, "diagnostic_id": null
        });
        let actual = json!({
            "execution_state": "FAILED", "availability_state": "UNAVAILABLE",
            "completeness_state": "UNAVAILABLE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": {"rejected_measure": measure,
                "fact_equivalent_rewrite": "count grouped by native_kind"},
            "query_result": {"query_id": request["query_id"], "result_role": "groups",
                "group_ids": [], "coverage": {"state": "NOT_APPLICABLE"},
                "errors": [fault.clone()], "notices": []}, "errors": [fault]
        });
        assert_fault(actual, fixture.as_ref().expect("negative fixture"));
        return;
    }
    ingress.expect("objective v2 summary must pass ingress");

    let observed = rows(
        execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![relation],
            plan,
            &[],
        )
        .await,
    );
    let admitted = &inputs["admitted_relations"];
    let expected_positive = positive_response(&claim);
    let input_set_identity = objective_input_set_identity(&inputs);
    let input_set_id = input_set_identity.public_id.as_str();
    let group_ids = observed
        .iter()
        .map(|row| {
            Value::String(
                objective_group_identity(
                    &inputs,
                    request,
                    input_set_id,
                    row_text(row, "native_kind"),
                )
                .public_id,
            )
        })
        .collect::<Vec<_>>();

    if case == Case::Causal {
        let groups = observed
            .iter()
            .map(|row| {
                let kind = row_text(row, "native_kind");
                let identity = objective_group_identity(&inputs, request, input_set_id, kind);
                json!({
                    "group_id": identity.public_id,
                    "group_key": {"native_kind": kind},
                    "objective_value": {"measure": request["measure"],
                        "value": row["count_seed"]},
                    "input_set_id": input_set_id,
                    "grouping": request["group_by"],
                    "aggregation": request["measure"],
                    "support_fact_ids": support_fact_ids(admitted, kind),
                    "producer_id": admitted["producer_id"], "precision": admitted["precision"],
                    "completeness": "COMPLETE",
                    "identity_recipe": identity.recipe_evidence()
                })
            })
            .collect::<Vec<_>>();
        let group_ids_by_native_kind = observed
            .iter()
            .map(|row| {
                let kind = row_text(row, "native_kind");
                (
                    kind.to_owned(),
                    Value::String(
                        objective_group_identity(&inputs, request, input_set_id, kind).public_id,
                    ),
                )
            })
            .collect::<Map<_, _>>();
        let fixture = fixture.as_ref().expect("causal fixture");
        let previous_fact_ids = array(
            &fixture["mutation"]["before"]["syntax_rows"],
            "previous summary syntax rows",
        )
        .iter()
        .map(|row| string(&row["fact_id"], "previous summary fact identity"))
        .collect::<std::collections::BTreeSet<_>>();
        let changed_rows = array(&admitted["syntax_rows"], "summary syntax rows")
            .iter()
            .filter(|row| {
                !previous_fact_ids.contains(string(&row["fact_id"], "summary fact identity"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            changed_rows.len(),
            1,
            "causal summary mutation must admit exactly one changed fact"
        );
        let changed_fact = changed_rows[0];
        let actual = json!({
            "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": {"measure": request["measure"], "group_by": request["group_by"],
                "input_set_id": input_set_id},
            "query_result": {"query_id": request["query_id"], "result_role": "groups",
                "group_ids": group_ids, "groups": groups,
                "coverage": {"state": "COMPLETE",
                    "input_set_id": input_set_id,
                    "input_count": array(&admitted["syntax_rows"], "syntax rows").len(),
                    "group_count": observed.len()}, "errors": [], "notices": [],
                "identity_contract": {
                    "changed_fact_id": changed_fact["fact_id"],
                    "changed_fact_identity_recipe": changed_fact["identity_recipe"],
                    "input_set_id": input_set_id,
                    "input_set_identity_recipe": input_set_identity.recipe_evidence(),
                    "group_ids_by_native_kind": group_ids_by_native_kind}}, "errors": []
        });
        assert_eq!(actual, *expected_case(&claim, Some(fixture)));
        return;
    }

    let groups = observed
        .iter()
        .map(|row| {
            let kind = row_text(row, "native_kind");
            let identity = objective_group_identity(&inputs, request, input_set_id, kind);
            (
                identity.public_id.clone(),
                json!({
                    "group_id": identity.public_id, "group_key": {"native_kind": kind},
                    "objective_value": {"measure": request["measure"],
                        "value": row["count_seed"]},
                    "input_set_id": input_set_id, "grouping": request["group_by"],
                    "aggregation": request["measure"], "producer_id": admitted["producer_id"],
                    "precision": admitted["precision"], "completeness": "COMPLETE",
                    "support_fact_ids": support_fact_ids(admitted, kind),
                    "identity_recipe": identity.recipe_evidence()
                }),
            )
        })
        .collect::<Map<_, _>>();
    let actual_query = json!({
        "query_id": request["query_id"], "request": request["request"],
        "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
        "completeness_state": "COMPLETE", "freshness_state": "CURRENT",
        "limit_state": "NOT_APPLIED", "dependency_state": "READY",
        "resolved_semantics": {"measure": request["measure"], "group_by": request["group_by"],
            "input_set_id": input_set_id, "objective_only": true},
        "result_role": "groups", "entity_ids": [], "fact_ids": [], "path_ids": [],
        "group_ids": group_ids, "source_context_ids": [], "bindings": [],
        "coverage": {"state": "COMPLETE", "input_set_id": input_set_id,
            "input_count": array(&admitted["syntax_rows"], "syntax rows").len(),
            "group_count": observed.len()}, "provenance": public_provenance(&inputs),
        "errors": [], "notices": []
    });
    assert_eq!(actual_query, expected_positive["query_results"][0]);
    assert_eq!(Value::Object(groups), expected_positive["groups"]);
    let facts = array(&admitted["syntax_rows"], "syntax rows")
        .iter()
        .map(|fact| (string(&fact["fact_id"], "fact id").to_owned(), fact.clone()))
        .collect::<Map<_, _>>();
    assert_eq!(Value::Object(facts), expected_positive["facts"]);
    assert_eq!(admitted["entity_dictionary"], expected_positive["entities"]);
}

fn source_context_input_relation(inputs: &Value) -> TestRelation {
    let admitted = &inputs["admitted_relations"];
    let span = &admitted["entity_span"];
    let source = &admitted["source_bytes"];
    let fields = vec![
        TestField::text("011", "source", "entity_id"),
        TestField::text("011", "source", "workspace_id"),
        TestField::text("011", "source", "source_file_id"),
        TestField::text("011", "source", "content_digest"),
        TestField::text("011", "source", "byte_safe_path"),
        TestField::int64("011", "source", "start_byte"),
        TestField::int64("011", "source", "end_byte"),
        TestField::int64("011", "source", "source_generation"),
        TestField::text("011", "source", "encoding"),
        TestField::text("011", "source", "source_value"),
        TestField::int64("011", "source", "byte_length"),
    ];
    let numeric = |value: &Value, name: &str| {
        i64::try_from(
            value
                .as_u64()
                .unwrap_or_else(|| panic!("{name} must be unsigned")),
        )
        .unwrap_or_else(|_| panic!("{name} exceeds i64"))
    };
    TestRelation::new(
        "011",
        "canonical.entity_span",
        fields,
        vec![vec![
            text(string(&span["entity_id"], "source entity id")),
            text(string(&span["workspace_id"], "source workspace id")),
            text(string(&span["source_file_id"], "source file id")),
            text(string(&span["content_digest"], "source content digest")),
            text(string(&span["byte_safe_path"], "byte-safe path")),
            int64(numeric(&span["start_byte"], "start byte")),
            int64(numeric(&span["end_byte"], "end byte")),
            int64(numeric(&span["source_generation"], "source generation")),
            text(string(&source["encoding"], "source encoding")),
            text(string(&source["value"], "source value")),
            int64(numeric(&source["byte_length"], "source byte length")),
        ]],
    )
}

fn source_reference(row: &BTreeMap<String, Value>) -> Value {
    json!({
        "workspace_id": row["workspace_id"],
        "source_file_id": row["source_file_id"],
        "content_digest": row["content_digest"],
        "byte_safe_path": row["byte_safe_path"],
        "start_byte": row["start_byte"],
        "end_byte": row["end_byte"],
        "source_generation": row["source_generation"]
    })
}

fn source_content_json(content: &SourceContextContent) -> Value {
    match content {
        SourceContextContent::Text(text) => json!({"variant": "text", "text": text.as_ref()}),
        SourceContextContent::Bytes(bytes) => json!({
            "variant": "bytes",
            "bytes": bytes.iter().copied().map(Value::from).collect::<Vec<_>>()
        }),
    }
}

fn materialize_source_context(
    inputs: &Value,
    row: &BTreeMap<String, Value>,
) -> Result<
    crate::fabric::source_context::MaterializedSourceContext,
    SourceContextMaterializationError,
> {
    let access = &inputs["access_scope"];
    let source_access = access["source_access"]
        .as_bool()
        .expect("source access boolean");
    let ranges = array(&access["authorized_ranges"], "authorized source ranges");
    let (authorized_start_byte, authorized_end_byte) = if source_access {
        assert_eq!(ranges.len(), 1, "one exact authorized source range");
        let authorized = array(&ranges[0], "authorized source range");
        assert_eq!(
            string(&authorized[0], "authorized source file"),
            row_text(row, "source_file_id"),
            "source grant must bind the selected canonical file"
        );
        (
            usize_value(&authorized[1], "authorized start"),
            usize_value(&authorized[2], "authorized end"),
        )
    } else {
        assert!(
            ranges.is_empty(),
            "denied source scope grants no byte range"
        );
        (0, 0)
    };
    let request = query(inputs);
    materialize_authorized_source_context(SourceContextMaterializationInput {
        span: SourceSpanIdentity {
            entity_id: Arc::from(row_text(row, "entity_id")),
            workspace_id: Arc::from(row_text(row, "workspace_id")),
            source_file_id: Arc::from(row_text(row, "source_file_id")),
            content_digest: Arc::from(row_text(row, "content_digest")),
            byte_safe_path: Arc::from(row_text(row, "byte_safe_path")),
            start_byte: row_usize(row, "start_byte"),
            end_byte: row_usize(row, "end_byte"),
            source_generation: u64::try_from(row_usize(row, "source_generation"))
                .expect("source generation fits u64"),
        },
        grant: SourceAccessGrant {
            source_access,
            workspace_id: Arc::from(string(&access["workspace"], "access workspace")),
            authorized_start_byte,
            authorized_end_byte,
            authorization_scope: Arc::from(string(
                &access["scope_id"],
                "source disclosure scope identity",
            )),
        },
        analysis_context_id: Arc::from(string(
            &inputs["admitted_relations"]["entity_dictionary"][row_text(row, "entity_id")]["analysis_context_id"],
            "source analysis context identity",
        )),
        snapshot_id: Arc::from(string(
            &inputs["pinned_epoch"]["snapshot_id"],
            "source snapshot identity",
        )),
        context_kind: Arc::from(string(&request["context"], "source context kind")),
        policy_identity: Arc::from(string(
            &inputs["pinned_epoch"]["policy_release"],
            "source context policy identity",
        )),
        source_bytes: row_text(row, "source_value").as_bytes(),
        declared_byte_length: row_usize(row, "byte_length"),
        explicit_source_byte_limit: usize_value(
            &inputs["resource_limits"]["max_source_bytes"],
            "explicit source byte limit",
        ),
        hard_output_byte_limit: usize_value(
            &inputs["resource_limits"]["max_output_bytes"],
            "hard source output limit",
        ),
    })
}

async fn claim_011(case: Case) {
    const CLAIM: &str = "RFV3-CLAIM-011";
    let claim = expectation(CLAIM);
    let (inputs, fixture) = case_inputs(&claim, case);
    validate_v2_ingress(&inputs).expect("released v2 source-context request must pass ingress");
    let request = query(&inputs);
    let relation = source_context_input_relation(&inputs);
    let relation_id = relation.id.as_str().to_owned();
    let plan = single_plan(
        ReleasedSemanticForm::RetrieveSourceAndSyntaxContext,
        &relation,
        string(&request["query_id"], "query id"),
        &[("entity_id", string(&request["about"][0], "source target"))],
        &[],
        None,
        SemanticQueryClass::Fact(Arc::from("semantic.fact")),
    );

    if case == Case::Negative {
        let denied = execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![relation],
            plan,
            &[relation_id.as_str()],
        )
        .await;
        execution_rows(denied).expect_err("ungranted source relation must reject physically");
        let denied_row = BTreeMap::from([
            (
                "entity_id".to_owned(),
                inputs["admitted_relations"]["entity_span"]["entity_id"].clone(),
            ),
            (
                "workspace_id".to_owned(),
                inputs["admitted_relations"]["entity_span"]["workspace_id"].clone(),
            ),
            (
                "source_file_id".to_owned(),
                inputs["admitted_relations"]["entity_span"]["source_file_id"].clone(),
            ),
            (
                "content_digest".to_owned(),
                inputs["admitted_relations"]["entity_span"]["content_digest"].clone(),
            ),
            (
                "byte_safe_path".to_owned(),
                inputs["admitted_relations"]["entity_span"]["byte_safe_path"].clone(),
            ),
            (
                "start_byte".to_owned(),
                inputs["admitted_relations"]["entity_span"]["start_byte"].clone(),
            ),
            (
                "end_byte".to_owned(),
                inputs["admitted_relations"]["entity_span"]["end_byte"].clone(),
            ),
            (
                "source_generation".to_owned(),
                inputs["admitted_relations"]["entity_span"]["source_generation"].clone(),
            ),
            (
                "source_value".to_owned(),
                inputs["admitted_relations"]["source_bytes"]["value"].clone(),
            ),
            (
                "byte_length".to_owned(),
                inputs["admitted_relations"]["source_bytes"]["byte_length"].clone(),
            ),
        ]);
        assert_eq!(
            materialize_source_context(&inputs, &denied_row),
            Err(SourceContextMaterializationError::SourceAccessDenied)
        );
        let fault = json!({
            "code": "SOURCE_ACCESS_DENIED", "layer": "authorization", "retryable": false,
            "safe_message": "Source disclosure is not authorized for this request.",
            "field": "source_access", "semantic_phrase": null, "candidate_interpretations": [],
            "failed_dependency_query_id": null, "diagnostic_id": null
        });
        let actual = json!({
            "execution_state": "FAILED", "availability_state": "UNAVAILABLE",
            "completeness_state": "UNAVAILABLE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": {"about": request["about"], "context": request["context"]},
            "query_result": {"query_id": request["query_id"], "result_role": "source_contexts",
                "source_context_ids": [], "source_bytes_disclosed": 0, "text_disclosed": false,
                "coverage": {"state": "NOT_APPLICABLE",
                    "reason": "source authorization denied"}, "errors": [fault.clone()],
                "notices": []}, "errors": [fault]
        });
        assert_fault(actual, fixture.as_ref().expect("negative fixture"));
        return;
    }

    let observed = rows(
        execute_plan(
            CLAIM,
            string(&request["query_id"], "query id"),
            vec![relation],
            plan,
            &[],
        )
        .await,
    );
    assert_eq!(observed.len(), 1, "one requested source span");
    let row = &observed[0];
    let materialized = materialize_source_context(&inputs, row)
        .expect("authorized source context materialization");
    let source_context_id = materialized.source_context_id.to_string();
    let source_context_identity_recipe = materialized.identity_recipe.clone();
    let hard_limit = usize_value(
        &inputs["resource_limits"]["max_output_bytes"],
        "hard source output limit",
    );
    assert!(materialized.returned_bytes < hard_limit);
    let expected_context_id = string(
        if case == Case::Causal {
            &expected_case(&claim, fixture.as_ref())["query_result"]["source_context_ids"][0]
        } else {
            &positive_response(&claim)["query_results"][0]["source_context_ids"][0]
        },
        "expected source context identity",
    );
    assert_eq!(materialized.source_context_id.as_ref(), expected_context_id);
    assert_eq!(source_context_id, expected_context_id);

    if case == Case::Causal {
        assert_eq!(
            materialized.limit_state,
            SourceContextLimitState::NotApplied
        );
        let actual = json!({
            "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
            "completeness_state": "COMPLETE", "freshness_state": "CURRENT",
            "limit_state": "NOT_APPLIED", "dependency_state": "READY",
            "resolved_semantics": {"context": request["context"],
                "explicit_source_byte_limit": materialized.explicit_source_byte_limit},
            "query_result": {"query_id": request["query_id"], "result_role": "source_contexts",
                "source_context_ids": [source_context_id],
                "source_contexts": [{"source_context_id": source_context_id,
                    "content": source_content_json(&materialized.content),
                    "returned_bytes": materialized.returned_bytes,
                    "omitted_bytes": materialized.omitted_bytes, "complete": materialized.complete}],
                "coverage": {"state": "COMPLETE",
                    "authorized_span_bytes": materialized.returned_bytes + materialized.omitted_bytes,
                    "returned_bytes": materialized.returned_bytes,
                    "omitted_bytes": materialized.omitted_bytes}, "errors": [], "notices": [],
                "identity_contract": {"source_context_id": source_context_id,
                    "identity_recipe": source_context_identity_recipe,
                    "delivered_bytes_bound": true}},
            "errors": []
        });
        assert_eq!(actual, *expected_case(&claim, fixture.as_ref()));
        return;
    }

    assert_eq!(
        materialized.limit_state,
        SourceContextLimitState::ExplicitLimitReached
    );
    let source_reference = source_reference(row);
    let context_record = json!({
        "source_context_id": source_context_id,
        "entity_id": materialized.span.entity_id.as_ref(),
        "context_kind": request["context"], "source_reference": source_reference,
        "content": source_content_json(&materialized.content),
        "returned_bytes": materialized.returned_bytes, "omitted_bytes": materialized.omitted_bytes,
        "complete": materialized.complete,
        "authorization_scope": materialized.authorization_scope.as_ref(),
        "limit": {"kind": "explicit", "state": "EXPLICIT_LIMIT_REACHED",
            "maximum_source_bytes": materialized.explicit_source_byte_limit},
        "identity_recipe": source_context_identity_recipe
    });
    let actual_query = json!({
        "query_id": request["query_id"], "request": request["request"],
        "execution_state": "COMPLETE", "availability_state": "AVAILABLE",
        "completeness_state": "PARTIAL", "freshness_state": "CURRENT",
        "limit_state": "EXPLICIT_LIMIT_REACHED", "dependency_state": "READY",
        "resolved_semantics": {"about": request["about"], "context": request["context"],
            "text_handling": "lossless UTF-8 else bytes",
            "explicit_source_byte_limit": materialized.explicit_source_byte_limit},
        "result_role": "source_contexts", "entity_ids": [], "fact_ids": [], "path_ids": [],
        "group_ids": [], "source_context_ids": [source_context_id], "bindings": [],
        "coverage": {"state": "PARTIAL", "reason": "EXPLICIT_LIMIT_REACHED",
            "authorized_span_bytes": materialized.returned_bytes + materialized.omitted_bytes,
            "returned_bytes": materialized.returned_bytes,
            "omitted_bytes": materialized.omitted_bytes},
        "provenance": public_provenance(&inputs), "errors": [], "notices": []
    });
    let expected = positive_response(&claim);
    assert_eq!(actual_query, expected["query_results"][0]);
    let contexts = Value::Object(Map::from_iter([(
        source_context_id.to_owned(),
        context_record,
    )]));
    assert_eq!(contexts, expected["source_contexts"]);
    assert_eq!(
        inputs["admitted_relations"]["entity_dictionary"],
        expected["entities"]
    );
    assert_eq!(expected["facts"], json!({}));
}

fn authorization_relations(inputs: &Value) -> Vec<TestRelation> {
    let entity_rows = array(&inputs["provider_rows"]["rows"], "authorized entity rows")
        .iter()
        .map(|row| {
            let row = array(row, "authorized entity row");
            vec![
                text(string(&row[0], "authorized entity id")),
                text(string(&row[1], "authorized entity kind")),
            ]
        })
        .collect();
    vec![
        TestRelation::new(
            "014",
            "public.entity",
            vec![
                TestField::text("014", "entity", "entity_id"),
                TestField::text("014", "entity", "kind"),
            ],
            entity_rows,
        ),
        TestRelation::new(
            "014",
            "public.location",
            vec![
                TestField::text("014", "location", "entity_id"),
                TestField::text("014", "location", "source_file_id"),
                TestField::int64("014", "location", "start_byte"),
                TestField::int64("014", "location", "end_byte"),
            ],
            Vec::new(),
        ),
        TestRelation::new(
            "014",
            "internal.source_secret",
            vec![
                TestField::text("014", "secret", "entity_id"),
                TestField::text("014", "secret", "secret"),
            ],
            Vec::new(),
        ),
    ]
}

fn access_scope_string_set(scope: &Value, field: &str) -> Vec<String> {
    array(&scope[field], field)
        .iter()
        .map(|value| string(value, field).to_owned())
        .collect()
}

fn derive_access_scope_identity(scope: &Value, policy_identity: &str) -> CanonicalPublicIdentity {
    let allowed_columns = object(&scope["allowed_columns"], "allowed columns")
        .iter()
        .map(|(relation, columns)| {
            (
                relation.clone(),
                array(columns, "allowed relation columns")
                    .iter()
                    .map(|column| string(column, "allowed column").to_owned())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let authorized_ranges = array(&scope["authorized_ranges"], "authorized ranges")
        .iter()
        .map(|range| {
            let range = array(range, "authorized source range");
            assert_eq!(range.len(), 3, "authorized source range arity");
            AuthorizedSourceRangeIdentityInput {
                source_file_id: string(&range[0], "authorized source file").to_owned(),
                start_byte: range[1]
                    .as_u64()
                    .expect("authorized source start must be unsigned"),
                end_byte: range[2]
                    .as_u64()
                    .expect("authorized source end must be unsigned"),
            }
        })
        .collect::<Vec<_>>();
    issue_access_scope_identity(&AccessScopeIdentityInput {
        workspace_id: string(&scope["workspace"], "access workspace").to_owned(),
        policy_identity: policy_identity.to_owned(),
        principal_id: string(&scope["principal_id"], "access principal").to_owned(),
        agent_id: string(&scope["agent_id"], "access agent").to_owned(),
        credential_digest: string(&scope["credential_digest"], "credential digest").to_owned(),
        role: string(&scope["role"], "access role").to_owned(),
        operation: string(&scope["operation"], "access operation").to_owned(),
        allowed_relations: access_scope_string_set(scope, "allowed_relations"),
        allowed_columns,
        allowed_functions: access_scope_string_set(scope, "allowed_functions"),
        allowed_extensions: access_scope_string_set(scope, "allowed_extensions"),
        allowed_variables: access_scope_string_set(scope, "allowed_variables"),
        allowed_object_stores: access_scope_string_set(scope, "allowed_object_stores"),
        allowed_metadata: access_scope_string_set(scope, "allowed_metadata"),
        row_policies: access_scope_string_set(scope, "row_policies"),
        execution_posture: access_scope_string_set(scope, "execution_posture"),
        source_access: scope["source_access"]
            .as_bool()
            .expect("source access must be boolean"),
        source_file_ids: access_scope_string_set(scope, "source_file_ids"),
        authorized_ranges,
    })
    .expect("derive complete CBEF access-scope identity")
}

fn relation_refs_for_ids<'a>(relations: &'a [TestRelation], ids: &Value) -> Vec<&'a TestRelation> {
    array(ids, "allowed relation identities")
        .iter()
        .map(|identity| {
            let identity = string(identity, "allowed relation identity");
            relations
                .iter()
                .find(|relation| relation.id.as_str() == identity)
                .unwrap_or_else(|| panic!("epoch lacks allowed relation {identity}"))
        })
        .collect()
}

fn child_query_rows(result: &crate::fabric::child_session::ChildQueryResult) -> Vec<Vec<Value>> {
    let schema = result.schema();
    let mut rows = Vec::new();
    for batch in result.batches() {
        for row_index in 0..batch.num_rows() {
            let mut row = Vec::with_capacity(batch.num_columns());
            for (column_index, field) in schema.fields().iter().enumerate() {
                let array = batch.column(column_index);
                let value = match field.data_type() {
                    DataType::Utf8 => Value::String(
                        array
                            .as_any()
                            .downcast_ref::<StringArray>()
                            .expect("authorized UTF-8 array")
                            .value(row_index)
                            .to_owned(),
                    ),
                    DataType::Int64 => Value::from(
                        array
                            .as_any()
                            .downcast_ref::<Int64Array>()
                            .expect("authorized Int64 array")
                            .value(row_index),
                    ),
                    other => panic!("unexpected authorized result type {other:?}"),
                };
                row.push(value);
            }
            rows.push(row);
        }
    }
    rows
}

async fn claim_014(case: Case) {
    const CLAIM: &str = "RFV3-CLAIM-014";
    let claim = expectation(CLAIM);
    let (inputs, fixture) = case_inputs(&claim, case);
    let access_scope = &inputs["access_scope"];
    let policy_identity = string(
        &inputs["authorization_policy"]["policy_id"],
        "authorization policy identity",
    );
    let derived_scope = derive_access_scope_identity(access_scope, policy_identity);
    let supplied_scope_id = string(&access_scope["scope_id"], "supplied access-scope identity");
    if case == Case::Negative {
        assert_eq!(
            access_scope["identity_recipe"]["output_id"], supplied_scope_id,
            "negative case must retain the previously issued identity recipe"
        );
        assert_ne!(
            supplied_scope_id, derived_scope.public_id,
            "a mutated column grant must not reuse its previous access-scope identity"
        );
        assert_fault(
            json!({
                "error": "ACCESS_SCOPE_IDENTITY_MISMATCH",
                "supplied_scope_id": supplied_scope_id,
                "derived_scope_id": derived_scope.public_id.as_str()
            }),
            fixture.as_ref().expect("negative fixture"),
        );
        return;
    }
    assert_eq!(supplied_scope_id, derived_scope.public_id);
    assert_eq!(
        access_scope["identity_recipe"],
        derived_scope.recipe_evidence()
    );
    let relations = authorization_relations(&inputs);
    let catalog_relations = object(
        &inputs["epoch_provider_catalog"]["relations"],
        "epoch provider catalog",
    )
    .keys()
    .map(String::as_str)
    .collect::<Vec<_>>();
    assert_eq!(
        catalog_relations,
        relations
            .iter()
            .map(|relation| relation.id.as_str())
            .collect::<Vec<_>>()
    );
    let epoch = sealed_epoch(0x14, &relations).await;
    let max_rows = usize_value(
        &inputs["resource_policy"]["max_rows"],
        "authorized max rows",
    );
    let allowed = relation_refs_for_ids(&relations, &inputs["access_scope"]["allowed_relations"]);
    let child = authorized_child_with_access_scope_and_max_rows(
        &epoch,
        &allowed,
        derived_scope.full_digest,
        max_rows,
    )
    .await
    .expect("construct reduced authorized child catalog");
    assert_eq!(child.pins().access_scope(), &derived_scope.full_digest);
    let visible = child
        .allowed_tables()
        .map(|relation| Value::String(relation.as_str().to_owned()))
        .collect::<Vec<_>>();

    let over_limit = child
        .scan(
            &ChildTableScan::all(ProgrammaticRelationId::new("public.entity"))
                .with_limit(max_rows + 1),
        )
        .await
        .expect_err("child resource policy must reject an oversized result request");
    assert!(matches!(
        over_limit,
        ChildSessionError::OutputRowLimitExceeded { .. }
    ));

    if case == Case::Causal {
        let fixture = fixture.as_ref().expect("causal fixture");
        let previous_scope = &fixture["mutation"]["before"];
        let previous_identity = derive_access_scope_identity(previous_scope, policy_identity);
        assert_eq!(previous_scope["scope_id"], previous_identity.public_id);
        assert_eq!(
            previous_scope["identity_recipe"],
            previous_identity.recipe_evidence()
        );
        let previous_allowed =
            relation_refs_for_ids(&relations, &previous_scope["allowed_relations"]);
        let previous = authorized_child_with_access_scope_and_max_rows(
            &epoch,
            &previous_allowed,
            previous_identity.full_digest,
            max_rows,
        )
        .await
        .expect("rebuild previous reduced child catalog");
        assert_eq!(
            previous.pins().access_scope(),
            &previous_identity.full_digest
        );
        let previous_visible = previous
            .allowed_tables()
            .map(|relation| Value::String(relation.as_str().to_owned()))
            .collect::<Vec<_>>();
        let bound_providers = inputs["bound_plan"]["providers"].clone();
        let entity = child
            .scan(
                &ChildTableScan::all(ProgrammaticRelationId::new("public.entity"))
                    .with_projection(vec![0, 1])
                    .with_limit(max_rows),
            )
            .await
            .expect("unchanged bound plan executes in rebuilt child");
        assert_eq!(
            json!(child_query_rows(&entity)),
            inputs["provider_rows"]["rows"]
        );
        assert_fault(
            json!({
                "scope_id": derived_scope.public_id.as_str(),
                "previous_scope_id": previous_identity.public_id.as_str(),
                "visible_relations": visible,
                "rebuilt_installed_relations": visible,
                "previous_installed_relations": previous_visible,
                "bound_plan_providers_unchanged": bound_providers
            }),
            fixture,
        );
        return;
    }

    assert_eq!(
        json!(visible),
        inputs["child_catalog_bindings"]["installed_relations"]
    );
    let provider = string(&inputs["bound_plan"]["providers"][0], "bound provider");
    let projection = array(&inputs["bound_plan"]["projection"], "bound projection");
    let entity_relation = relations
        .iter()
        .find(|relation| relation.id.as_str() == provider)
        .expect("bound provider relation");
    let projection_indices = projection
        .iter()
        .map(|field| {
            let field = string(field, "bound projection field");
            entity_relation
                .fields
                .iter()
                .position(|candidate| candidate.name == field)
                .unwrap_or_else(|| panic!("bound projection lacks {field}"))
        })
        .collect::<Vec<_>>();
    let result = child
        .scan(
            &ChildTableScan::all(ProgrammaticRelationId::new(provider))
                .with_projection(projection_indices)
                .with_limit(max_rows),
        )
        .await
        .expect("authorized bound plan scan");
    assert!(!result.truncated());
    let expected = &claim["decoded_expectation"];
    assert_eq!(
        json!({
            "terminal": "pass", "relation": provider, "columns": projection,
            "rows": child_query_rows(&result),
            "coverage": "one allowed provider recursively verified; denied provider absent from resolution and diagnostics"
        }),
        *expected
    );
}

#[tokio::test]
async fn wp38_claim_004_positive_production_execution() {
    claim_004(Case::Positive).await;
}

#[tokio::test]
async fn wp38_claim_004_causal_production_execution() {
    claim_004(Case::Causal).await;
}

#[tokio::test]
async fn wp38_claim_004_negative_production_execution() {
    claim_004(Case::Negative).await;
}

#[tokio::test]
async fn wp38_claim_005_positive_production_execution() {
    claim_005(Case::Positive).await;
}

#[tokio::test]
async fn wp38_claim_005_causal_production_execution() {
    claim_005(Case::Causal).await;
}

#[tokio::test]
async fn wp38_claim_005_negative_production_execution() {
    claim_005(Case::Negative).await;
}

#[tokio::test]
async fn wp38_claim_006_positive_production_execution() {
    claim_006(Case::Positive).await;
}

#[tokio::test]
async fn wp38_claim_006_causal_production_execution() {
    claim_006(Case::Causal).await;
}

#[tokio::test]
async fn wp38_claim_006_negative_production_execution() {
    claim_006(Case::Negative).await;
}

#[tokio::test]
async fn wp38_claim_007_positive_production_execution() {
    claim_007(Case::Positive).await;
}

#[tokio::test]
async fn wp38_claim_007_causal_production_execution() {
    claim_007(Case::Causal).await;
}

#[tokio::test]
async fn wp38_claim_007_negative_production_execution() {
    claim_007(Case::Negative).await;
}

#[tokio::test]
async fn wp38_claim_008_positive_production_execution() {
    claim_008(Case::Positive).await;
}

#[tokio::test]
async fn wp38_claim_008_causal_production_execution() {
    claim_008(Case::Causal).await;
}

#[tokio::test]
async fn wp38_claim_008_negative_production_execution() {
    claim_008(Case::Negative).await;
}

#[tokio::test]
async fn wp38_claim_009_positive_production_execution() {
    claim_009(Case::Positive).await;
}

#[tokio::test]
async fn wp38_claim_009_causal_production_execution() {
    claim_009(Case::Causal).await;
}

#[tokio::test]
async fn wp38_claim_009_negative_production_execution() {
    claim_009(Case::Negative).await;
}

#[tokio::test]
async fn wp38_claim_010_positive_production_execution() {
    claim_010(Case::Positive).await;
}

#[tokio::test]
async fn wp38_claim_010_causal_production_execution() {
    claim_010(Case::Causal).await;
}

#[tokio::test]
async fn wp38_claim_010_negative_production_execution() {
    claim_010(Case::Negative).await;
}

#[tokio::test]
async fn wp38_claim_011_positive_production_execution() {
    claim_011(Case::Positive).await;
}

#[tokio::test]
async fn wp38_claim_011_causal_production_execution() {
    claim_011(Case::Causal).await;
}

#[tokio::test]
async fn wp38_claim_011_negative_production_execution() {
    claim_011(Case::Negative).await;
}

#[tokio::test]
async fn wp38_claim_014_positive_production_execution() {
    claim_014(Case::Positive).await;
}

#[tokio::test]
async fn wp38_claim_014_causal_production_execution() {
    claim_014(Case::Causal).await;
}

#[tokio::test]
async fn wp38_claim_014_negative_production_execution() {
    claim_014(Case::Negative).await;
}
