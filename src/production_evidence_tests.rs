//! WP38 first-principles execution of the frozen clean/incremental claim.
//!
//! The expected source images and fault programs are read from the independently authored WP33
//! artifacts. The observed values come only from the production Python provider lane, Arrow
//! batches, and DataFusion logical execution. No test-local evaluator constructs expected rows.

use std::path::Path;
use std::sync::Arc;

use crate::cancellation::Cancellation;
use crate::fabric::{ResultChecksumV2, result_checksum_v2};
use crate::identity::{
    CbefField, CbefRecord, CbefValue, IdentityDomain, StringNormalization, decode_b3_digest,
    decode_public_id, derive_identity, encode_public_id,
};
use crate::provider_native_syntax::{
    ExactPythonSyntaxRunner, NativeSyntaxRelation, ProviderNativeSourceImage, PythonModuleInput,
    PythonSyntaxRunPins, SyntaxProviderRunPin,
};
use crate::provider_types::ProviderText;
use crate::tree_sitter_adapter::TreeSitterEdit;
use arrow_array::RecordBatch;
use datafusion::datasource::MemTable;
use datafusion::prelude::{SessionContext, col};
use serde_json::{Map, Value};

const EXPECTATIONS: &str =
    include_str!("../contracts/acceptance/relational-fabric-v3/expectations.jsonl");
const FIXTURES: &str =
    include_str!("../contracts/acceptance/relational-fabric-v3/negative-fixtures.jsonl");
const CLAIM_ID: &str = "RFV3-CLAIM-018";
const COMMON_PROVIDER_COLUMNS: usize = 8;
const MAXIMUM_CHECKSUM_ENCODING_BYTES: usize = 16 * 1024 * 1024;

fn object<'a>(value: &'a Value, context: &str) -> &'a Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{context} must be an object"))
}

fn string<'a>(value: &'a Value, context: &str) -> &'a str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{context} must be a string"))
}

fn expectation() -> Value {
    EXPECTATIONS
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("valid WP33 expectation JSONL"))
        .find(|row| row["claim_id"] == CLAIM_ID)
        .expect("frozen Claim 018 expectation")
}

fn fixture(kind: &str) -> Value {
    FIXTURES
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("valid WP33 fixture JSONL"))
        .find(|row| row["claim_id"] == CLAIM_ID && row["kind"] == kind)
        .unwrap_or_else(|| panic!("frozen Claim 018 {kind} fixture"))
}

fn source_value<'a>(claim: &'a Value, generation: &str) -> &'a Value {
    &claim["complete_input_universe"]["inputs"]["source_images"][generation]
}

fn provider_text(text: &str) -> ProviderText {
    let offsets = text
        .char_indices()
        .map(|(offset, _)| u64::try_from(offset).expect("source byte offset"))
        .chain(std::iter::once(
            u64::try_from(text.len()).expect("source byte length"),
        ))
        .collect::<Vec<_>>();
    ProviderText {
        text: Arc::from(text),
        original_byte_offsets: offsets.into(),
    }
}

fn source_image(value: &Value, generation: u64) -> ProviderNativeSourceImage {
    let row = object(value, "source image");
    assert_eq!(row["language"], "python");
    let text = string(&row["bytes_utf8"], "source bytes");
    let expected_digest = decode_b3_digest(string(&row["content_digest"], "source digest"))
        .expect("framed source BLAKE3 digest");
    assert_eq!(
        crate::integrity::digest_bytes(text.as_bytes()),
        expected_digest,
        "accepted source digest does not bind the delivered source bytes"
    );
    assert_eq!(
        row["source_generation"],
        format!("g{generation}"),
        "numeric provider generation differs from the accepted generation"
    );
    ProviderNativeSourceImage::new(
        decode_hex_16(string(&row["file_id"], "source file id")),
        generation,
        Arc::from(text.as_bytes()),
        expected_digest,
        provider_text(text),
    )
    .expect("exact immutable provider source image")
}

fn decode_hex_16(value: &str) -> [u8; 16] {
    assert_eq!(value.len(), 32, "16-byte identity must be lowercase hex");
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')),
        "16-byte identity must be lowercase hex"
    );
    let mut decoded = [0; 16];
    for (index, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        decoded[index] = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16)
            .expect("validated lowercase hexadecimal byte");
    }
    decoded
}

fn pins(seed: u8, value: &Value) -> PythonSyntaxRunPins {
    let row = object(value, "source image");
    let workspace_id = decode_public_id(
        IdentityDomain::Workspace,
        None,
        string(&row["workspace_id"], "source workspace id"),
    )
    .expect("accepted workspace public identity");
    let semantic_environment_id = decode_b3_digest(string(
        &row["semantic_environment_digest"],
        "semantic environment digest",
    ))
    .expect("accepted semantic-environment digest");
    let context = derive_identity(&CbefRecord {
        domain: IdentityDomain::AnalysisContext,
        fields: vec![
            CbefField {
                tag: 1,
                value: CbefValue::Id(workspace_id),
            },
            CbefField {
                tag: 2,
                value: CbefValue::Utf8 {
                    value: string(&row["language"], "source language").to_owned(),
                    normalization: StringNormalization::AsciiLower,
                },
            },
            CbefField {
                tag: 3,
                value: CbefValue::Digest(semantic_environment_id),
            },
        ],
    })
    .expect("derive accepted analysis-context identity");
    assert_eq!(
        encode_public_id(IdentityDomain::AnalysisContext, None, context.id)
            .expect("encode analysis-context identity"),
        string(&row["analysis_context_id"], "analysis-context public id"),
        "accepted analysis-context public identity differs from its typed recipe"
    );
    PythonSyntaxRunPins {
        tree_sitter: SyntaxProviderRunPin {
            provider_run_id: [seed; 16],
            analysis_context_id: context.full_digest,
            semantic_environment_id,
        },
        ruff: SyntaxProviderRunPin {
            provider_run_id: [seed.wrapping_add(1); 16],
            analysis_context_id: context.full_digest,
            semantic_environment_id,
        },
    }
}

fn encompassing_edit(before: &str, after: &str) -> TreeSitterEdit {
    let before_bytes = before.as_bytes();
    let after_bytes = after.as_bytes();
    let mut prefix = before_bytes
        .iter()
        .zip(after_bytes)
        .take_while(|(left, right)| left == right)
        .count();
    while !before.is_char_boundary(prefix) || !after.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let maximum_suffix = before_bytes.len().min(after_bytes.len()) - prefix;
    let mut suffix = (0..maximum_suffix)
        .take_while(|offset| {
            before_bytes[before_bytes.len() - 1 - offset]
                == after_bytes[after_bytes.len() - 1 - offset]
        })
        .count();
    while !before.is_char_boundary(before.len() - suffix)
        || !after.is_char_boundary(after.len() - suffix)
    {
        suffix -= 1;
    }
    TreeSitterEdit {
        start_byte: prefix,
        old_end_byte: before.len() - suffix,
        new_end_byte: after.len() - suffix,
    }
}

fn run_full(
    source: &ProviderNativeSourceImage,
    source_value: &Value,
    seed: u8,
) -> crate::provider_native_syntax::ProviderNativeSyntaxRun {
    ExactPythonSyntaxRunner::new()
        .expect("exact production Python providers")
        .run_full(
            source.source_generation,
            source,
            pins(seed, source_value),
            PythonModuleInput {
                module_name: "wp38.equivalence",
                module_path: Path::new("wp38/equivalence.py"),
            },
            &Cancellation::default(),
        )
        .expect("complete clean production provider route")
}

fn run_incremental(
    base: &ProviderNativeSourceImage,
    base_value: &Value,
    target: &ProviderNativeSourceImage,
    target_value: &Value,
) -> crate::provider_native_syntax::ProviderNativeSyntaxRun {
    let mut runner = ExactPythonSyntaxRunner::new().expect("exact production Python providers");
    runner
        .run_full(
            base.source_generation,
            base,
            pins(0x41, base_value),
            PythonModuleInput {
                module_name: "wp38.equivalence",
                module_path: Path::new("wp38/equivalence.py"),
            },
            &Cancellation::default(),
        )
        .expect("complete incremental base provider route");
    runner
        .run_incremental(
            target.source_generation,
            target,
            encompassing_edit(&base.provider_text.text, &target.provider_text.text),
            pins(0x43, target_value),
            PythonModuleInput {
                module_name: "wp38.equivalence",
                module_path: Path::new("wp38/equivalence.py"),
            },
            &Cancellation::default(),
        )
        .expect("complete incremental production provider route")
}

async fn execute_projection(
    batches: Vec<RecordBatch>,
    columns: &[String],
    distinct: bool,
) -> ResultChecksumV2 {
    let input_schema = batches
        .first()
        .expect("projection requires one schema-carrying batch")
        .schema();
    assert!(batches.iter().all(|batch| batch.schema() == input_schema));
    let context = SessionContext::new();
    context
        .register_table(
            "wp38_input",
            Arc::new(
                MemTable::try_new(Arc::clone(&input_schema), vec![batches])
                    .expect("schema-compatible Arrow route batches"),
            ),
        )
        .expect("register WP38 route relation");
    let column_names = columns.iter().map(String::as_str).collect::<Vec<_>>();
    let mut frame = context
        .table("wp38_input")
        .await
        .expect("resolve WP38 route relation")
        .select_columns(&column_names)
        .expect("typed DataFusion projection");
    if distinct {
        frame = frame.distinct().expect("transparent DataFusion distinct");
        frame = frame
            .sort(
                column_names
                    .iter()
                    .map(|name| col(*name).sort(true, true))
                    .collect(),
            )
            .expect("deterministic DataFusion ordering");
    }
    let plan = frame
        .into_optimized_plan()
        .expect("optimized WP38 logical plan");
    let output_schema = Arc::new(plan.schema().as_arrow().clone());
    let output = context
        .execute_logical_plan(plan)
        .await
        .expect("physical WP38 plan")
        .collect()
        .await
        .expect("execute WP38 relation projection");
    result_checksum_v2(
        output_schema.as_ref(),
        &output,
        MAXIMUM_CHECKSUM_ENCODING_BYTES,
    )
    .expect("bounded canonical Arrow result checksum")
}

fn semantic_columns(batch: &RecordBatch) -> Vec<String> {
    let fields = batch.schema();
    let common = fields
        .fields()
        .iter()
        .take(COMMON_PROVIDER_COLUMNS)
        .map(|field| field.name().as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        common,
        [
            "provider_run_id",
            "provider_id",
            "provider_release",
            "analysis_context_id",
            "semantic_environment_id",
            "file_id",
            "content_digest",
            "source_generation",
        ]
    );
    fields
        .fields()
        .iter()
        .skip(COMMON_PROVIDER_COLUMNS)
        .map(|field| field.name().clone())
        .collect()
}

#[tokio::test]
async fn wp38_claim_018_clean_incremental_equivalence_executes_successor_arrow_datafusion() {
    let claim = expectation();
    let base_value = source_value(&claim, "generation_g1");
    let target_value = source_value(&claim, "generation_g2");
    let base = source_image(base_value, 1);
    let target = source_image(target_value, 2);
    let clean = run_full(&target, target_value, 0x51);
    let incremental = run_incremental(&base, base_value, &target, target_value);

    assert!(
        incremental
            .relation(NativeSyntaxRelation::TreeSitterChangedRange)
            .num_rows()
            > 0,
        "the incremental route must execute a real changed-range parse"
    );
    for relation in NativeSyntaxRelation::ALL {
        if relation == NativeSyntaxRelation::TreeSitterChangedRange {
            continue;
        }
        let clean_batch = clean.relation(relation);
        let incremental_batch = incremental.relation(relation);
        assert_eq!(clean_batch.schema(), incremental_batch.schema());
        let columns = semantic_columns(clean_batch);
        let clean_checksum = execute_projection(vec![clean_batch.clone()], &columns, false).await;
        let incremental_checksum =
            execute_projection(vec![incremental_batch.clone()], &columns, false).await;
        assert_eq!(
            clean_checksum,
            incremental_checksum,
            "clean and incremental semantic Arrow rows differ for {}",
            relation.as_str()
        );
    }
}

#[tokio::test]
async fn wp38_claim_018_causal_source_change_is_discriminated_by_successor_execution() {
    let claim = expectation();
    let causal = fixture("causal");
    assert_eq!(causal["fixture_id"], "RFV3-FIX-018-C");
    assert_eq!(causal["mutation"]["input_role"], "source_images");
    let target_value = source_value(&claim, "generation_g2");
    let changed_value = &causal["mutation"]["after"];
    let target = source_image(target_value, 2);
    let changed = source_image(changed_value, 2);
    let target_run = run_full(&target, target_value, 0x61);
    let changed_run = run_full(&changed, changed_value, 0x63);
    let columns = [
        "name",
        "binding_kind",
        "target_form",
        "start_byte",
        "end_byte",
    ]
    .map(str::to_owned);
    let target_checksum = execute_projection(
        vec![
            target_run
                .relation(NativeSyntaxRelation::RuffBinding)
                .clone(),
        ],
        &columns,
        true,
    )
    .await;
    let changed_checksum = execute_projection(
        vec![
            changed_run
                .relation(NativeSyntaxRelation::RuffBinding)
                .clone(),
        ],
        &columns,
        true,
    )
    .await;
    assert_ne!(
        target_checksum, changed_checksum,
        "the independently issued e3-to-e4 source fault survived production execution"
    );
}

#[tokio::test]
async fn wp38_claim_018_missing_delete_fault_is_rejected_by_successor_execution() {
    let claim = expectation();
    let negative = fixture("negative");
    assert_eq!(negative["fixture_id"], "RFV3-FIX-018-N");
    assert_eq!(
        negative["mutation"]["json_pointer"],
        "/incremental/operations/2/enabled"
    );
    assert_eq!(negative["mutation"]["before"], true);
    assert_eq!(negative["mutation"]["after"], false);

    let base_value = source_value(&claim, "generation_g1");
    let target_value = source_value(&claim, "generation_g2");
    let base = source_image(base_value, 1);
    let target = source_image(target_value, 2);
    let base_run = run_full(&base, base_value, 0x71);
    let clean_run = run_full(&target, target_value, 0x73);
    let incremental_run = run_incremental(&base, base_value, &target, target_value);
    let columns = [
        "name",
        "binding_kind",
        "target_form",
        "start_byte",
        "end_byte",
    ]
    .map(str::to_owned);
    let clean_checksum = execute_projection(
        vec![
            clean_run
                .relation(NativeSyntaxRelation::RuffBinding)
                .clone(),
        ],
        &columns,
        true,
    )
    .await;
    let incremental_checksum = execute_projection(
        vec![
            incremental_run
                .relation(NativeSyntaxRelation::RuffBinding)
                .clone(),
        ],
        &columns,
        true,
    )
    .await;
    assert_eq!(clean_checksum, incremental_checksum);

    let stale_without_deletes = execute_projection(
        vec![
            base_run.relation(NativeSyntaxRelation::RuffBinding).clone(),
            incremental_run
                .relation(NativeSyntaxRelation::RuffBinding)
                .clone(),
        ],
        &columns,
        true,
    )
    .await;
    assert_ne!(
        clean_checksum, stale_without_deletes,
        "skipping typed deletes retained stale-current semantic rows without detection"
    );
}
