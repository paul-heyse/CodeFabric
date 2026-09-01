//! Artifact-bound WP38 execution for provider, transformation, and analysis claims.
//!
//! Every test decodes the independently reviewed WP33 row, supplies its exact values through the
//! owning production constructor, executes the real owner, and compares observed decoded values.

use std::collections::HashMap;
use std::sync::Arc;

use arrow_array::{Array as _, ArrayRef, RecordBatch, StringArray, UInt32Array};
use arrow_schema::{DataType, Field, Schema};
use datafusion::catalog::MemorySchemaProvider;
use datafusion::common::TableReference;
use datafusion::datasource::MemTable;
use datafusion::logical_expr::{LogicalPlan, LogicalPlanBuilder, lit};
use datafusion::prelude::{SessionContext, col};
use serde_json::{Map, Value, json};

use crate::fabric::command::EpochId;
use crate::fabric::programmatic_schema::{
    ObservationFixedPointPolicy, ProgrammaticFieldId, ProgrammaticRelationId,
    ProgrammaticSchemaAssembly, ProgrammaticSchemaError, ProgrammaticTransformation,
    ProgrammaticTransformationContract, ProgrammaticTransformationId, ProviderInput,
    TransformationDeterminismPolicy, TransformationFieldIdentity, TransformationInputs,
    TransformationNullPlacement, TransformationOrderingKey, TransformationOrderingPolicy,
    TransformationOutput, TransformationPlanError, TransformationProvenance,
    TransformationProvenanceIdentity, TransformationRecursionPolicy, TransformationReleaseIdentity,
    TransformationResourceClass, TransformationSemanticVersion, TransformationSortDirection,
};
use crate::schema_contract::{
    FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SchemaContract,
};

const EXPECTATIONS: &str =
    include_str!("../contracts/acceptance/relational-fabric-v3/expectations.jsonl");
const FIXTURES: &str =
    include_str!("../contracts/acceptance/relational-fabric-v3/negative-fixtures.jsonl");

fn object<'a>(value: &'a Value, context: &str) -> &'a Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{context} must be an object"))
}

fn array<'a>(value: &'a Value, context: &str) -> &'a [Value] {
    value
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("{context} must be an array"))
}

fn string<'a>(value: &'a Value, context: &str) -> &'a str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{context} must be a string"))
}

fn claim(claim_id: &str) -> Value {
    EXPECTATIONS
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("valid WP33 expectation JSONL"))
        .find(|row| row["claim_id"] == claim_id)
        .unwrap_or_else(|| panic!("frozen expectation {claim_id}"))
}

fn fixture(fixture_id: &str) -> Value {
    FIXTURES
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("valid WP33 fixture JSONL"))
        .find(|row| row["fixture_id"] == fixture_id)
        .unwrap_or_else(|| panic!("frozen fixture {fixture_id}"))
}

fn small_cardinal(value: usize) -> &'static str {
    const CARDINALS: [&str; 21] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ];
    CARDINALS
        .get(value)
        .copied()
        .expect("WP38 evidence fixtures use bounded small cardinalities")
}

#[derive(Debug)]
struct Claim002Transformation {
    contract: ProgrammaticTransformationContract,
    output: TransformationOutput,
    dependencies: Vec<ProgrammaticRelationId>,
    filter_column: Arc<str>,
    filter_value: Arc<str>,
}

impl ProgrammaticTransformation for Claim002Transformation {
    fn contract(&self) -> &ProgrammaticTransformationContract {
        &self.contract
    }

    fn output(&self) -> &TransformationOutput {
        &self.output
    }

    fn dependencies(&self) -> &[ProgrammaticRelationId] {
        &self.dependencies
    }

    fn build(&self, inputs: &TransformationInputs) -> Result<LogicalPlan, TransformationPlanError> {
        let input = inputs.plan(&self.dependencies[0])?;
        Ok(LogicalPlanBuilder::from(input)
            .filter(col(self.filter_column.as_ref()).eq(lit(self.filter_value.to_string())))?
            .project([col("node_id").alias("subject"), col("ordinal")])?
            .sort([
                col("ordinal").sort(true, false),
                col("subject").sort(true, false),
            ])?
            .build()?)
    }
}

fn claim_002_input_rows(input: &Value) -> (Vec<String>, Vec<String>, Vec<u32>) {
    let rows = array(&input["rows"], "Claim 002 admitted rows");
    let mut node_ids = Vec::with_capacity(rows.len());
    let mut native_kinds = Vec::with_capacity(rows.len());
    let mut ordinals = Vec::with_capacity(rows.len());
    for row in rows {
        let fields = array(row, "Claim 002 admitted row");
        assert_eq!(fields.len(), 3);
        node_ids.push(string(&fields[0], "Claim 002 node id").to_owned());
        native_kinds.push(string(&fields[1], "Claim 002 native kind").to_owned());
        ordinals.push(
            u32::try_from(
                fields[2]
                    .as_u64()
                    .expect("Claim 002 ordinal must be unsigned"),
            )
            .expect("Claim 002 ordinal must fit u32"),
        );
    }
    (node_ids, native_kinds, ordinals)
}

fn claim_002_candidate_state() -> datafusion::execution::context::SessionState {
    let context = SessionContext::new();
    context
        .catalog("datafusion")
        .expect("default DataFusion catalog")
        .register_schema("system", Arc::new(MemorySchemaProvider::new()))
        .expect("WP38 observation schema");
    context.state()
}

fn claim_002_provider(input: &Value) -> ProviderInput {
    let relation_id = string(&input["relation"], "Claim 002 input relation");
    let (node_ids, native_kinds, ordinals) = claim_002_input_rows(input);
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("node_id", DataType::Utf8, false).with_metadata(HashMap::from([(
                FIELD_ID_METADATA_KEY.to_owned(),
                "native.syntax_node.node_id".to_owned(),
            )])),
            Field::new("native_kind", DataType::Utf8, false).with_metadata(HashMap::from([(
                FIELD_ID_METADATA_KEY.to_owned(),
                "native.syntax_node.native_kind".to_owned(),
            )])),
            Field::new("ordinal", DataType::UInt32, false).with_metadata(HashMap::from([(
                FIELD_ID_METADATA_KEY.to_owned(),
                "native.syntax_node.ordinal".to_owned(),
            )])),
        ],
        HashMap::from([(RELATION_ID_METADATA_KEY.to_owned(), relation_id.to_owned())]),
    ));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(StringArray::from(node_ids)) as ArrayRef,
            Arc::new(StringArray::from(native_kinds)) as ArrayRef,
            Arc::new(UInt32Array::from(ordinals)) as ArrayRef,
        ],
    )
    .expect("typed Claim 002 provider batch");
    let table_reference = TableReference::full("datafusion", "public", "wp38_native_syntax_node");
    let contract = Arc::new(
        SchemaContract::try_new(
            "wp38:claim-002:provider",
            table_reference.clone(),
            Arc::clone(&schema),
            Arc::clone(&schema),
            vec![
                FieldIndexMapping::direct(0, 0),
                FieldIndexMapping::direct(1, 1),
                FieldIndexMapping::direct(2, 2),
            ],
        )
        .expect("Claim 002 provider schema contract"),
    );
    ProviderInput::new(
        ProgrammaticRelationId::new(relation_id),
        table_reference,
        contract,
        Arc::new(
            MemTable::try_new(Arc::clone(&schema), vec![vec![batch]])
                .expect("Claim 002 provider table"),
        ),
    )
}

fn claim_002_transformation(definition: &Value) -> Claim002Transformation {
    let definition = object(definition, "Claim 002 transformation definition");
    let operations = array(
        &definition["plan_building_function"]["operations"],
        "Claim 002 operations",
    );
    let predicate = &operations[0]["predicate"];
    let output = &definition["output_schema_assertion"];
    let output_relation = string(&output["relation_id"], "Claim 002 output relation");
    let fields = array(&output["fields"], "Claim 002 output fields");
    let output_schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new(
                string(&fields[0]["name"], "Claim 002 subject field"),
                DataType::Utf8,
                false,
            )
            .with_metadata(HashMap::from([(
                FIELD_ID_METADATA_KEY.to_owned(),
                "canonical.function_syntax.subject".to_owned(),
            )])),
            Field::new(
                string(&fields[1]["name"], "Claim 002 ordinal field"),
                DataType::UInt32,
                false,
            )
            .with_metadata(HashMap::from([(
                FIELD_ID_METADATA_KEY.to_owned(),
                "canonical.function_syntax.ordinal".to_owned(),
            )])),
        ],
        HashMap::from([(
            RELATION_ID_METADATA_KEY.to_owned(),
            output_relation.to_owned(),
        )]),
    ));
    Claim002Transformation {
        contract: ProgrammaticTransformationContract::new(
            ProgrammaticTransformationId::new(string(
                &definition["semantic_id"],
                "Claim 002 transformation id",
            )),
            TransformationSemanticVersion::new(1, 0, 0),
            TransformationResourceClass::BoundedInMemory {
                max_rows: 3,
                max_memory_bytes: 1 << 20,
            },
            TransformationDeterminismPolicy::DeterministicSequence,
            TransformationOrderingPolicy::ByOutputFields(Arc::from([
                TransformationOrderingKey::new(
                    ProgrammaticFieldId::new("canonical.function_syntax.ordinal"),
                    TransformationSortDirection::Ascending,
                    TransformationNullPlacement::Last,
                ),
                TransformationOrderingKey::new(
                    ProgrammaticFieldId::new("canonical.function_syntax.subject"),
                    TransformationSortDirection::Ascending,
                    TransformationNullPlacement::Last,
                ),
            ])),
            TransformationRecursionPolicy::Forbidden,
            TransformationProvenance::new(
                TransformationProvenanceIdentity::from_bytes([0x22; 32]),
                TransformationReleaseIdentity::from_bytes([0x23; 32]),
            ),
        ),
        output: TransformationOutput::new(
            ProgrammaticRelationId::new(output_relation),
            TableReference::full("datafusion", "public", "wp38_function_syntax"),
            [
                TransformationFieldIdentity::new(ProgrammaticFieldId::new(
                    "canonical.function_syntax.subject",
                )),
                TransformationFieldIdentity::new(ProgrammaticFieldId::new(
                    "canonical.function_syntax.ordinal",
                )),
            ],
        )
        .with_schema_assertion(output_schema),
        dependencies: vec![ProgrammaticRelationId::new(string(
            &definition["plan_building_function"]["root_input_relation_id"],
            "Claim 002 root input relation",
        ))],
        filter_column: Arc::from(string(
            &predicate["left"]["name"],
            "Claim 002 filter column",
        )),
        filter_value: Arc::from(string(
            &predicate["right"]["value"],
            "Claim 002 filter value",
        )),
    }
}

async fn execute_claim_002(
    input: &Value,
    definition: &Value,
) -> Result<Value, ProgrammaticSchemaError> {
    let input_count = array(&input["rows"], "Claim 002 input rows").len();
    let transformation = claim_002_transformation(definition);
    let output_id = transformation.output().relation_id().clone();
    let mut assembly = ProgrammaticSchemaAssembly::with_observation_policy(
        claim_002_candidate_state(),
        ObservationFixedPointPolicy::production(),
    );
    assembly.register_provider(claim_002_provider(input))?;
    assembly.add_transformation(Arc::new(transformation))?;
    let sealed = assembly.seal(EpochId::from_bytes([0x22; 16])).await?;
    let binding = sealed
        .relation(&output_id)
        .expect("Claim 002 output binding");
    let batches = sealed
        .session()
        .table(binding.table_reference.clone())
        .await?
        .collect()
        .await?;
    let mut rows = Vec::new();
    for batch in batches {
        let subjects = batch
            .column_by_name("subject")
            .expect("Claim 002 subject column")
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("Claim 002 subject strings");
        let ordinals = batch
            .column_by_name("ordinal")
            .expect("Claim 002 ordinal column")
            .as_any()
            .downcast_ref::<UInt32Array>()
            .expect("Claim 002 ordinal values");
        for row in 0..batch.num_rows() {
            rows.push(json!([subjects.value(row), ordinals.value(row)]));
        }
    }
    Ok(json!({
        "terminal": "pass",
        "relation": output_id.as_str(),
        "columns": ["subject", "ordinal"],
        "rows": rows,
        "coverage": format!(
            "all {} admitted input rows consumed",
            small_cardinal(input_count)
        ),
    }))
}

#[tokio::test]
async fn wp38_claim_002_positive_executes_frozen_typed_datafusion_transformation() {
    let claim = claim("RFV3-CLAIM-002");
    let inputs = &claim["complete_input_universe"]["inputs"];
    let observed = execute_claim_002(
        &inputs["admitted_input_relation"],
        &inputs["transformation_definition"],
    )
    .await
    .expect("frozen Claim 002 production transformation");
    assert_eq!(observed, claim["decoded_expectation"]);
}

#[tokio::test]
async fn wp38_claim_002_causal_fixture_changes_real_datafusion_rows() {
    let claim = claim("RFV3-CLAIM-002");
    let causal = fixture("RFV3-FIX-002-C");
    let inputs = &claim["complete_input_universe"]["inputs"];
    let mut definition = inputs["transformation_definition"].clone();
    let mutation = &causal["mutation"];
    assert_eq!(mutation["input_role"], "transformation_definition");
    assert_eq!(
        definition["plan_building_function"]["operations"][0]["predicate"]["right"]["value"],
        mutation["before"]
    );
    definition["plan_building_function"]["operations"][0]["predicate"]["right"]["value"] =
        mutation["after"].clone();
    let baseline = execute_claim_002(
        &inputs["admitted_input_relation"],
        &inputs["transformation_definition"],
    )
    .await
    .expect("baseline Claim 002 transformation");
    let changed = execute_claim_002(&inputs["admitted_input_relation"], &definition)
        .await
        .expect("mutated Claim 002 transformation");
    assert_ne!(baseline["rows"], changed["rows"]);
    assert_eq!(changed["relation"], causal["expected_decoded"]["relation"]);
    assert_eq!(changed["rows"], causal["expected_decoded"]["rows"]);
    assert_eq!(causal["expected_terminal"], "changed");
}

#[tokio::test]
async fn wp38_claim_002_negative_fixture_rejects_undeclared_typed_column() {
    let claim = claim("RFV3-CLAIM-002");
    let negative = fixture("RFV3-FIX-002-N");
    let inputs = &claim["complete_input_universe"]["inputs"];
    let mut definition = inputs["transformation_definition"].clone();
    let mutation = &negative["mutation"];
    assert_eq!(mutation["input_role"], "transformation_definition");
    assert_eq!(
        definition["plan_building_function"]["operations"][0]["predicate"]["left"]["name"],
        mutation["before"]
    );
    definition["plan_building_function"]["operations"][0]["predicate"]["left"]["name"] =
        mutation["after"].clone();
    let error = execute_claim_002(&inputs["admitted_input_relation"], &definition)
        .await
        .expect_err("undeclared Claim 002 column must fail before execution");
    let error_text = match &error {
        ProgrammaticSchemaError::TransformationBuild {
            source: TransformationPlanError::DataFusion(source),
            ..
        } => source.to_string(),
        other => panic!("unexpected Claim 002 production rejection: {other:?}"),
    };
    let rejected_column = string(&mutation["after"], "Claim 002 rejected column");
    assert!(
        error_text.contains(rejected_column),
        "real DataFusion/schema error did not identify {rejected_column}: {error_text}"
    );
    assert_eq!(negative["expected_terminal"], "reject");
    assert_eq!(negative["expected_decoded"]["column"], rejected_column);
    assert_eq!(
        negative["expected_decoded"]["error"],
        "TRANSFORMATION_INPUT_COLUMN_UNDECLARED"
    );
}
