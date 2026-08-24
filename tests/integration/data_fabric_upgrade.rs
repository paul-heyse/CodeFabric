use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use arrow_array::builder::{ListBuilder, StringBuilder};
use arrow_array::{
    ArrayRef, BinaryArray, BooleanArray, Float64Array, Int32Array, RecordBatch, StringArray,
    TimestampMicrosecondArray,
};
use arrow_schema::{DataType, Field, Schema, TimeUnit};
use arrow_select::concat::concat_batches;
use codefabric::fabric::{EmptySnapshotOverlay, SnapshotOverlayProviderFactory, batch_checksum};
use datafusion::prelude::{SessionConfig, SessionContext};
use deltalake::DeltaTableBuilder;
use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
use deltalake::kernel::{Transaction, transaction::CommitProperties};
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;
use serde_json::{Value, json};
use tempfile::TempDir;
use url::Url;

const FIXTURE_ROOT: &str = "tests/fixtures/data_fabric_upgrade/old_stack";
const MANIFEST_FILE: &str = "manifest.json";
const DELTA_DIR: &str = "delta_table";
const IPC_FILE: &str = "extractor_arrow58.ipc";
const BENCHMARK_COMPARATOR: &str = "tests/fixtures/data_fabric_upgrade/benchmark_comparator.json";

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_ROOT)
}

fn fixture_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Binary, false),
        Field::new("label", DataType::Utf8, true),
        Field::new("score", DataType::Float64, true),
        Field::new(
            "occurred_at",
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            true,
        ),
        Field::new(
            "tags",
            DataType::List(Arc::new(Field::new("element", DataType::Utf8, true))),
            true,
        ),
        Field::new("active", DataType::Boolean, false),
    ]))
}

fn tags(values: &[Option<Vec<Option<&str>>>]) -> ArrayRef {
    let mut builder = ListBuilder::new(StringBuilder::new()).with_field(Arc::new(Field::new(
        "element",
        DataType::Utf8,
        true,
    )));
    for value in values {
        match value {
            Some(items) => {
                for item in items {
                    builder.values().append_option(*item);
                }
                builder.append(true);
            }
            None => builder.append(false),
        }
    }
    Arc::new(builder.finish())
}

fn first_batch() -> RecordBatch {
    RecordBatch::try_new(
        fixture_schema(),
        vec![
            Arc::new(BinaryArray::from(vec![
                Some([0_u8, 1, 2, 3].as_slice()),
                Some([255_u8, 0, 127].as_slice()),
            ])),
            Arc::new(StringArray::from(vec![Some("alpha"), None])),
            Arc::new(Float64Array::from(vec![Some(1.5), Some(-0.0)])),
            Arc::new(
                TimestampMicrosecondArray::from(vec![Some(1_700_000_000_000_000_i64), None])
                    .with_timezone("UTC"),
            ),
            tags(&[
                Some(vec![Some("syntax"), None, Some("rust")]),
                Some(Vec::new()),
            ]),
            Arc::new(BooleanArray::from(vec![true, false])),
        ],
    )
    .expect("valid first fixture batch")
}

fn second_batch() -> RecordBatch {
    RecordBatch::try_new(
        fixture_schema(),
        vec![
            Arc::new(BinaryArray::from(vec![Some(
                [16_u8, 32, 48, 64].as_slice(),
            )])),
            Arc::new(StringArray::from(vec![Some("omega")])),
            Arc::new(Float64Array::from(vec![None])),
            Arc::new(
                TimestampMicrosecondArray::from(vec![Some(1_800_000_000_000_000_i64)])
                    .with_timezone("UTC"),
            ),
            tags(&[None]),
            Arc::new(BooleanArray::from(vec![true])),
        ],
    )
    .expect("valid second fixture batch")
}

fn ordered_batch() -> RecordBatch {
    concat_batches(&fixture_schema(), &[first_batch(), second_batch()])
        .expect("fixture batches concatenate")
}

fn reversed_batch() -> RecordBatch {
    let ordered = ordered_batch();
    let indices = Int32Array::from(vec![2, 1, 0]);
    let columns = ordered
        .columns()
        .iter()
        .map(|column| arrow_select::take::take(column.as_ref(), &indices, None).expect("take"))
        .collect();
    RecordBatch::try_new(fixture_schema(), columns).expect("reversed fixture batch")
}

fn empty_batch() -> RecordBatch {
    RecordBatch::new_empty(fixture_schema())
}

fn projected_primary_key(batch: &RecordBatch) -> RecordBatch {
    RecordBatch::try_new(
        Arc::new(Schema::new(vec![Field::new("id", DataType::Binary, false)])),
        vec![Arc::clone(batch.column(0))],
    )
    .expect("primary-key projection")
}

fn digest(value: &[u8]) -> String {
    format!("b3:{}", blake3::hash(value).to_hex())
}

fn batch_digest(batch: &RecordBatch) -> String {
    format!(
        "b3:{}",
        blake3::Hash::from_bytes(batch_checksum(batch).expect("batch checksum")).to_hex()
    )
}

fn stack_identity() -> Value {
    let lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"));
    let delta_source = lock
        .lines()
        .find(|line| line.contains("delta-rs.git?rev="))
        .and_then(|line| line.split("rev=").nth(1))
        .and_then(|value| value.split('#').next())
        .unwrap_or("unknown");
    json!({
        "arrow": arrow::ARROW_VERSION,
        "datafusion": datafusion::DATAFUSION_VERSION,
        "delta_revision": delta_source,
    })
}

fn commit_properties(version: i64) -> CommitProperties {
    CommitProperties::default()
        .with_max_retries(0)
        .with_metadata([
            ("codefabric_fixture".to_owned(), json!("wp01-old-stack")),
            (
                "operation_id".to_owned(),
                json!(format!("fixture-{version}")),
            ),
            ("application_version".to_owned(), json!(version)),
        ])
        .with_application_transaction(Transaction::new(
            "codefabric/wp01/data-fabric-upgrade",
            version,
        ))
}

fn write_fixture_payload(root: &Path) {
    let mut ipc = Vec::new();
    {
        let mut writer = StreamWriter::try_new(&mut ipc, &fixture_schema())
            .expect("create Arrow IPC stream writer");
        writer.write(&first_batch()).expect("write first IPC batch");
        writer
            .write(&second_batch())
            .expect("write second IPC batch");
        writer.finish().expect("finish Arrow IPC fixture");
    }
    fs::write(root.join(IPC_FILE), &ipc).expect("write Arrow IPC fixture");

    let rows = ordered_batch();
    let manifest = json!({
        "fixture_contract": "codefabric-data-fabric-upgrade-v1",
        "producer": stack_identity(),
        "delta": {
            "versions": [0, 1, 2],
            "row_counts": {"0": 0, "1": 2, "2": 3},
            "min_reader_version": 1,
            "min_writer_version": 2,
            "reader_features": [],
            "writer_features": [],
            "table_features": {
                "change_data_feed": false,
                "deletion_vectors": false,
                "type_widening": false
            },
            "application_id": "codefabric/wp01/data-fabric-upgrade",
            "application_versions": [0, 1, 2]
        },
        "checksums": {
            "empty_batch": batch_digest(&empty_batch()),
            "batch": batch_digest(&rows),
            "primary_key": batch_digest(&projected_primary_key(&rows)),
            "provider_content": batch_digest(&rows),
            "overlay": format!("b3:{}", blake3::Hash::from_bytes(EmptySnapshotOverlay.checksum()).to_hex()),
            "query_result": batch_digest(&rows),
            "ipc_bytes": digest(&ipc)
        },
        "schema": {
            "fields": ["id", "label", "score", "occurred_at", "tags", "active"],
            "nullable": [false, true, true, true, true, false],
            "rows": 3
        }
    });
    fs::write(
        root.join(MANIFEST_FILE),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&manifest).expect("manifest JSON")
        ),
    )
    .expect("write fixture manifest");
}

async fn generate_fixture(root: &Path) {
    fs::create_dir_all(root).expect("create fixture root");
    let delta_path = root.join(DELTA_DIR);
    fs::create_dir_all(&delta_path).expect("create Delta fixture root");
    let location = Url::from_directory_path(&delta_path)
        .expect("fixture path is a file URL")
        .to_string();
    let kernel: deltalake::kernel::StructType = fixture_schema()
        .as_ref()
        .try_into_kernel()
        .expect("Arrow fixture schema converts to Delta");
    let table = CreateBuilder::new()
        .with_location(location)
        .with_table_name("codefabric_wp01_old_stack")
        .with_comment("CodeFabric pre-upgrade compatibility fixture")
        .with_save_mode(SaveMode::ErrorIfExists)
        .with_columns(kernel.fields().cloned())
        .with_configuration(HashMap::from([
            (
                "delta.enableChangeDataFeed".to_owned(),
                Some("false".to_owned()),
            ),
            (
                "delta.enableDeletionVectors".to_owned(),
                Some("false".to_owned()),
            ),
            (
                "delta.enableTypeWidening".to_owned(),
                Some("false".to_owned()),
            ),
            ("delta.checkpointInterval".to_owned(), Some("10".to_owned())),
        ]))
        .with_raise_if_key_not_exists(false)
        .with_commit_properties(commit_properties(0))
        .await
        .expect("create old-stack Delta fixture");
    let table = table
        .write([first_batch()])
        .with_save_mode(SaveMode::Append)
        .with_commit_properties(commit_properties(1))
        .await
        .expect("write Delta fixture version 1");
    let table = table
        .write([second_batch()])
        .with_save_mode(SaveMode::Append)
        .with_commit_properties(commit_properties(2))
        .await
        .expect("write Delta fixture version 2");
    assert_eq!(table.version(), Some(2));

    write_fixture_payload(root);
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create fixture copy destination");
    for entry in fs::read_dir(source).expect("read fixture tree") {
        let entry = entry.expect("fixture tree entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("fixture entry type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy fixture file");
        }
    }
}

fn manifest(root: &Path) -> Value {
    serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).expect("read fixture manifest"))
        .expect("strict fixture manifest JSON")
}

async fn query_version(root: &Path, version: u64) -> RecordBatch {
    let location =
        Url::from_directory_path(root.join(DELTA_DIR)).expect("fixture path is a file URL");
    let table = DeltaTableBuilder::from_url(location)
        .expect("create exact-version Delta builder")
        .with_version(version)
        .load()
        .await
        .expect("open exact Delta fixture version");
    assert_eq!(table.version(), Some(version));
    let config = SessionConfig::new().set_bool(
        "datafusion.execution.parquet.schema_force_view_types",
        false,
    );
    let context = SessionContext::new_with_config(config);
    let provider = table
        .table_provider()
        .with_session(Arc::new(context.state()))
        .await
        .expect("construct Delta fixture provider");
    context
        .register_table("fixture", provider)
        .expect("register Delta fixture provider");
    let batches = context
        .sql("SELECT id, label, score, occurred_at, tags, active FROM fixture ORDER BY id")
        .await
        .expect("plan fixture query")
        .collect()
        .await
        .expect("execute fixture query");
    if batches.is_empty() {
        return empty_batch();
    }
    concat_batches(&batches[0].schema(), &batches).expect("concatenate fixture query output")
}

fn assert_protocol_baseline(root: &Path, expected: &Value) {
    let mut log = String::new();
    let log_root = root.join(DELTA_DIR).join("_delta_log");
    let mut json_files = fs::read_dir(log_root)
        .expect("read Delta log")
        .map(|entry| entry.expect("Delta log entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    json_files.sort();
    for path in json_files {
        log.push_str(&fs::read_to_string(path).expect("read Delta JSON action log"));
    }
    assert!(log.contains("\"minReaderVersion\":1"));
    assert!(log.contains("\"minWriterVersion\":2"));
    assert!(!log.contains("readerFeatures"));
    assert!(!log.contains("writerFeatures"));
    assert!(log.contains("delta.enableChangeDataFeed\":\"false"));
    assert!(log.contains("delta.enableDeletionVectors\":\"false"));
    assert!(log.contains("delta.enableTypeWidening\":\"false"));
    assert!(log.contains("codefabric/wp01/data-fabric-upgrade"));
    assert!(log.contains("\"version\":2"));
    assert_eq!(expected["delta"]["reader_features"], json!([]));
    assert_eq!(expected["delta"]["writer_features"], json!([]));
}

async fn validate_fixture(root: &Path, require_old_identity: bool) {
    let expected = manifest(root);
    if require_old_identity {
        assert_eq!(expected["producer"]["arrow"], "58.4.0");
        assert_eq!(expected["producer"]["datafusion"], "54.1.0");
        assert_eq!(
            expected["producer"]["delta_revision"],
            "9f9223197469897ef05ae4369eb4fd1390174e65"
        );
    }
    for (version, rows) in [(0_u64, 0_usize), (1, 2), (2, 3)] {
        assert_eq!(query_version(root, version).await.num_rows(), rows);
    }
    let rows = query_version(root, 2).await;
    assert_eq!(batch_digest(&rows), expected["checksums"]["query_result"]);
    assert_eq!(
        batch_digest(&rows),
        expected["checksums"]["provider_content"]
    );
    assert_eq!(batch_digest(&rows), expected["checksums"]["batch"]);
    assert_eq!(
        batch_digest(&projected_primary_key(&rows)),
        expected["checksums"]["primary_key"]
    );
    assert_protocol_baseline(root, &expected);
}

fn decode_ipc(root: &Path) -> RecordBatch {
    let bytes = fs::read(root.join(IPC_FILE)).expect("read extractor Arrow IPC fixture");
    let batches = StreamReader::try_new(Cursor::new(bytes), None)
        .expect("open extractor Arrow IPC stream")
        .collect::<Result<Vec<_>, _>>()
        .expect("decode extractor Arrow IPC batches");
    concat_batches(&batches[0].schema(), &batches).expect("concatenate decoded IPC batches")
}

#[tokio::test]
async fn data_fabric_54_arrow58_delta9f_persisted_baseline() {
    let temporary = TempDir::new().expect("fixture copy temporary directory");
    copy_tree(&fixture_root(), temporary.path());
    validate_fixture(temporary.path(), true).await;
}

#[test]
fn arrow58_codefabric_batch_checksum_kat() {
    let expected = manifest(&fixture_root());
    let ordered = ordered_batch();
    assert_eq!(batch_digest(&ordered), expected["checksums"]["batch"]);
    assert_eq!(
        batch_digest(&reversed_batch()),
        expected["checksums"]["batch"]
    );
    assert_eq!(
        batch_digest(&projected_primary_key(&ordered)),
        expected["checksums"]["primary_key"]
    );
    assert_eq!(
        batch_digest(&empty_batch()),
        expected["checksums"]["empty_batch"]
    );
}

#[test]
fn extractor_arrow58_ipc_baseline() {
    let expected = manifest(&fixture_root());
    let batch = decode_ipc(&fixture_root());
    assert_eq!(batch.num_rows(), 3);
    assert_eq!(batch.schema().fields().len(), 6);
    assert_eq!(
        batch
            .schema()
            .fields()
            .iter()
            .map(|field| field.is_nullable())
            .collect::<Vec<_>>(),
        vec![false, true, true, true, true, false]
    );
    assert_eq!(batch_digest(&batch), expected["checksums"]["batch"]);
    assert_eq!(
        digest(&fs::read(fixture_root().join(IPC_FILE)).expect("read IPC fixture")),
        expected["checksums"]["ipc_bytes"]
    );
}

#[tokio::test]
async fn data_fabric_cross_revision_fixture_mode() {
    let mode = std::env::var("CODEFABRIC_CROSS_REVISION_MODE")
        .expect("CODEFABRIC_CROSS_REVISION_MODE must be produce or consume");
    let root = PathBuf::from(
        std::env::var_os("CODEFABRIC_CROSS_REVISION_FIXTURE")
            .expect("CODEFABRIC_CROSS_REVISION_FIXTURE must name an isolated namespace"),
    );
    match mode.as_str() {
        "produce" => {
            assert!(!root.exists(), "producer refuses to overwrite a namespace");
            generate_fixture(&root).await;
        }
        "consume" => validate_fixture(&root, false).await,
        other => panic!("unknown cross-revision mode {other}"),
    }
}

#[tokio::test]
#[ignore = "one-time fixture generation; normal acceptance gates are read-only"]
async fn generate_old_stack_fixture_candidate() {
    let output = PathBuf::from(
        std::env::var_os("CODEFABRIC_FIXTURE_CANDIDATE")
            .expect("CODEFABRIC_FIXTURE_CANDIDATE is required"),
    );
    assert!(
        !output.exists(),
        "fixture generator refuses to overwrite bytes"
    );
    generate_fixture(&output).await;
}

fn benchmark_report() -> Value {
    let mut samples_micros = Vec::new();
    let mut checksum = String::new();
    for _ in 0..15 {
        let started = Instant::now();
        for _ in 0..128 {
            checksum = batch_digest(&reversed_batch());
        }
        samples_micros.push(
            u64::try_from(started.elapsed().as_micros()).expect("benchmark duration fits u64"),
        );
    }
    let rss_kib = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0);
    json!({
        "contract": "codefabric-data-fabric-upgrade-benchmark-v1",
        "stack": stack_identity(),
        "environment": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "rustc": option_env!("RUSTC").unwrap_or("rustc")
        },
        "workload": {
            "name": "arrow-batch-checksum-128x",
            "samples_micros": samples_micros,
            "correctness_checksum": checksum,
            "observed_rss_bytes": rss_kib.saturating_mul(1024),
            "resource_ceiling_bytes": 1_073_741_824_u64
        }
    })
}

fn median(values: &[u64]) -> u64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

fn benchmark_samples(report: &Value) -> Vec<u64> {
    report["workload"]["samples_micros"]
        .as_array()
        .expect("benchmark samples array")
        .iter()
        .map(|value| value.as_u64().expect("benchmark sample is u64"))
        .collect()
}

fn deterministic_bootstrap_regression_interval(baseline: &[u64], target: &[u64]) -> (i64, i64) {
    let mut seed = 0x04f1_bbcd_u64;
    let mut regressions = Vec::with_capacity(2_000);
    for _ in 0..2_000 {
        let mut baseline_sample = Vec::with_capacity(baseline.len());
        let mut target_sample = Vec::with_capacity(target.len());
        for _ in 0..baseline.len() {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let sample_len = u64::try_from(baseline.len()).expect("sample length fits u64");
            let index = usize::try_from(seed % sample_len).expect("sample index fits usize");
            baseline_sample.push(baseline[index]);
        }
        for _ in 0..target.len() {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let sample_len = u64::try_from(target.len()).expect("sample length fits u64");
            let index = usize::try_from(seed % sample_len).expect("sample index fits usize");
            target_sample.push(target[index]);
        }
        let baseline_median = median(&baseline_sample);
        let target_median = median(&target_sample);
        regressions.push(
            (i128::from(target_median) - i128::from(baseline_median))
                .saturating_mul(10_000)
                .checked_div(i128::from(baseline_median.max(1)))
                .and_then(|value| i64::try_from(value).ok())
                .expect("bootstrap regression basis points fit i64"),
        );
    }
    regressions.sort_unstable();
    (regressions[50], regressions[1_949])
}

fn compare_benchmark_reports(baseline: &Value, target: &Value, comparator: &Value) {
    assert_eq!(baseline["contract"], target["contract"]);
    assert_eq!(
        baseline["workload"]["correctness_checksum"],
        target["workload"]["correctness_checksum"]
    );
    for report in [baseline, target] {
        assert!(
            report["workload"]["observed_rss_bytes"]
                .as_u64()
                .expect("RSS")
                <= report["workload"]["resource_ceiling_bytes"]
                    .as_u64()
                    .expect("resource ceiling")
        );
    }
    let baseline_samples = benchmark_samples(baseline);
    let target_samples = benchmark_samples(target);
    let baseline_median = median(&baseline_samples);
    let target_median = median(&target_samples);
    let regression_bps = (u128::from(target_median).saturating_mul(10_000)
        / u128::from(baseline_median.max(1)))
    .saturating_sub(10_000);
    let allowed_bps = u128::from(
        comparator["median_regression_limit_basis_points"]
            .as_u64()
            .expect("median comparator limit"),
    );
    let (low_bps, _high_bps) =
        deterministic_bootstrap_regression_interval(&baseline_samples, &target_samples);
    assert!(
        regression_bps <= allowed_bps || low_bps <= 0,
        "target median regressed {regression_bps} bps and the 95% bootstrap interval excludes parity ({low_bps} bps lower bound)"
    );
}

#[test]
fn data_fabric_benchmark_emit_mode() {
    let output = PathBuf::from(
        std::env::var_os("CODEFABRIC_BENCHMARK_REPORT")
            .expect("CODEFABRIC_BENCHMARK_REPORT is required"),
    );
    fs::write(
        output,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&benchmark_report()).expect("benchmark JSON")
        ),
    )
    .expect("write benchmark report");
}

#[test]
fn data_fabric_benchmark_compare_mode() {
    let baseline = manifest_from_env("CODEFABRIC_BENCHMARK_BASELINE");
    let target = manifest_from_env("CODEFABRIC_BENCHMARK_TARGET");
    let comparator: Value = serde_json::from_slice(
        &fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(BENCHMARK_COMPARATOR))
            .expect("read benchmark comparator"),
    )
    .expect("benchmark comparator JSON");
    compare_benchmark_reports(&baseline, &target, &comparator);
}

fn manifest_from_env(name: &str) -> Value {
    let path =
        PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required")));
    serde_json::from_slice(&fs::read(path).expect("read benchmark report"))
        .expect("benchmark report JSON")
}

#[tokio::test]
async fn wp01_behavioral_old_stack_fixture() {
    let temporary = TempDir::new().expect("fixture copy temporary directory");
    copy_tree(&fixture_root(), temporary.path());
    validate_fixture(temporary.path(), true).await;
    arrow58_codefabric_batch_checksum_kat();
    extractor_arrow58_ipc_baseline();
}

#[test]
fn wp01_structural_single_test_target() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let top_level = fs::read_dir(tests)
        .expect("read tests directory")
        .map(|entry| entry.expect("test entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    assert_eq!(
        top_level,
        vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/integration.rs")]
    );
}

#[test]
fn wp01_negative_protocol_feature_baseline() {
    assert_protocol_baseline(&fixture_root(), &manifest(&fixture_root()));
}

#[test]
fn wp01_operational_benchmark_self_compare() {
    let report = benchmark_report();
    let comparator: Value = serde_json::from_slice(
        &fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(BENCHMARK_COMPARATOR))
            .expect("read benchmark comparator"),
    )
    .expect("benchmark comparator JSON");
    compare_benchmark_reports(&report, &report, &comparator);
}
