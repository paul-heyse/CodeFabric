//! Ephemeral cross-revision benchmark target.
//!
//! `scripts/data_fabric_revision_check.sh` copies this source into each isolated
//! revision worktree as a temporary integration target. It is never a permanent
//! Cargo test target, and intentionally uses only the API intersection shared by
//! the frozen WP01 stack and the target stack.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use arrow::ipc::writer::StreamWriter;
use arrow_array::{ArrayRef, BinaryArray, BooleanArray, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use datafusion::prelude::{SessionConfig, SessionContext};
use deltalake::DeltaTableBuilder;
use deltalake::kernel::engine::arrow_conversion::TryIntoKernel as _;
use deltalake::kernel::{Transaction, transaction::CommitProperties};
use deltalake::operations::create::CreateBuilder;
use deltalake::protocol::SaveMode;
use serde_json::{Value, json};
use tempfile::TempDir;
use url::Url;

const SAMPLES: usize = 15;
const RESOURCE_CEILING_BYTES: u64 = 1_073_741_824;

fn schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("owner_id", DataType::Binary, false),
        Field::new("label", DataType::Utf8, true),
        Field::new("active", DataType::Boolean, false),
    ]))
}

fn batch(start: i64, rows: usize) -> RecordBatch {
    let ids = (0..rows)
        .map(|offset| start + i64::try_from(offset).expect("row offset fits i64"))
        .collect::<Vec<_>>();
    let owners = (0..rows)
        .map(|offset| Some([u8::try_from(offset % 8).expect("owner byte") + 1; 16].to_vec()))
        .collect::<Vec<_>>();
    let labels = ids
        .iter()
        .map(|id| Some(format!("row-{id:04}")))
        .collect::<Vec<_>>();
    let active = ids.iter().map(|id| id % 2 == 0).collect::<Vec<_>>();
    RecordBatch::try_new(
        schema(),
        vec![
            Arc::new(Int64Array::from(ids)) as ArrayRef,
            Arc::new(BinaryArray::from_iter(owners)) as ArrayRef,
            Arc::new(StringArray::from(labels)) as ArrayRef,
            Arc::new(BooleanArray::from(active)) as ArrayRef,
        ],
    )
    .expect("benchmark batch")
}

fn commit_properties(application_version: i64) -> CommitProperties {
    CommitProperties::default()
        .with_max_retries(0)
        .with_metadata([
            (
                "benchmark_contract".to_owned(),
                json!("codefabric-fab112.6-v1"),
            ),
            ("application_version".to_owned(), json!(application_version)),
        ])
        .with_application_transaction(Transaction::new(
            "codefabric/data-fabric-revision-benchmark",
            application_version,
        ))
}

async fn create_fixture(root: &Path) -> deltalake::DeltaTable {
    fs::create_dir_all(root).expect("create benchmark table root");
    let location = Url::from_directory_path(root)
        .expect("benchmark path is a file URL")
        .to_string();
    let kernel: deltalake::kernel::StructType = schema()
        .as_ref()
        .try_into_kernel()
        .expect("benchmark schema converts to Delta");
    let table = CreateBuilder::new()
        .with_location(location)
        .with_table_name("codefabric_revision_benchmark")
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
        ]))
        .with_raise_if_key_not_exists(false)
        .with_commit_properties(commit_properties(0))
        .await
        .expect("create benchmark Delta table");
    let table = table
        .write([batch(0, 64)])
        .with_save_mode(SaveMode::Append)
        .with_commit_properties(commit_properties(1))
        .await
        .expect("write benchmark batch one");
    table
        .write([batch(64, 64)])
        .with_save_mode(SaveMode::Append)
        .with_commit_properties(commit_properties(2))
        .await
        .expect("write benchmark batch two")
}

fn session_config() -> SessionConfig {
    SessionConfig::new()
        .set_bool(
            "datafusion.execution.parquet.schema_force_view_types",
            false,
        )
        .set_bool("datafusion.execution.parquet.pushdown_filters", false)
        .set_bool("datafusion.execution.parquet.reorder_filters", false)
}

async fn context_for(table: &deltalake::DeltaTable) -> SessionContext {
    let context = SessionContext::new_with_config(session_config());
    let provider = table
        .table_provider()
        .with_session(Arc::new(context.state()))
        .await
        .expect("benchmark Delta provider");
    context
        .register_table("facts", provider)
        .expect("register benchmark table");
    context
}

async fn query(context: &SessionContext, sql: &str) -> Vec<RecordBatch> {
    context
        .sql(sql)
        .await
        .expect("plan benchmark query")
        .collect()
        .await
        .expect("execute benchmark query")
}

fn checksum(batches: &[RecordBatch]) -> String {
    let mut bytes = Vec::new();
    if let Some(first) = batches.first() {
        let mut writer = StreamWriter::try_new(&mut bytes, &first.schema())
            .expect("create benchmark checksum stream");
        for batch in batches {
            writer.write(batch).expect("write benchmark checksum batch");
        }
        writer.finish().expect("finish benchmark checksum stream");
    }
    format!("b3:{}", blake3::hash(&bytes).to_hex())
}

fn sample(value: &mut Vec<u64>, started: Instant) {
    value.push(u64::try_from(started.elapsed().as_micros()).expect("duration fits u64"));
}

fn rss_bytes() -> u64 {
    Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0)
        .saturating_mul(1024)
}

fn workload(samples_micros: Vec<u64>, correctness_checksum: String) -> Value {
    json!({
        "samples_micros": samples_micros,
        "correctness_checksum": correctness_checksum,
    })
}

async fn report() -> Value {
    let root = TempDir::new().expect("benchmark temporary directory");
    let table_root = root.path().join("shared");
    let table = create_fixture(&table_root).await;
    assert_eq!(table.version(), Some(2));
    let location = Url::from_directory_path(&table_root).expect("benchmark table URL");

    let mut activation = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let opened = DeltaTableBuilder::from_url(location.clone())
            .expect("activation builder")
            .load()
            .await
            .expect("activation load");
        sample(&mut activation, started);
        assert_eq!(opened.version(), Some(2));
    }

    let full_sql = "SELECT id, owner_id, label, active FROM facts ORDER BY id";
    let filtered_sql =
        "SELECT id, owner_id, label, active FROM facts WHERE active = true ORDER BY id";
    let mut first_query = Vec::with_capacity(SAMPLES);
    let mut first_checksum = String::new();
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let opened = DeltaTableBuilder::from_url(location.clone())
            .expect("first-query builder")
            .load()
            .await
            .expect("first-query load");
        let context = context_for(&opened).await;
        let rows = query(&context, filtered_sql).await;
        sample(&mut first_query, started);
        first_checksum = checksum(&rows);
    }

    let context = context_for(&table).await;
    let _ = query(&context, filtered_sql).await;
    let _ = query(&context, full_sql).await;
    let mut warmed_filtered = Vec::with_capacity(SAMPLES);
    let mut filtered_checksum = String::new();
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let rows = query(&context, filtered_sql).await;
        sample(&mut warmed_filtered, started);
        filtered_checksum = checksum(&rows);
    }
    let mut warmed_full = Vec::with_capacity(SAMPLES);
    let mut full_checksum = String::new();
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let rows = query(&context, full_sql).await;
        sample(&mut warmed_full, started);
        full_checksum = checksum(&rows);
    }

    let mut publication = Vec::with_capacity(SAMPLES);
    let mut publication_checksum = String::new();
    for index in 0..SAMPLES {
        let sample_root = root.path().join(format!("publication-{index}"));
        let sample_table = create_fixture(&sample_root).await;
        let started = Instant::now();
        let sample_table = sample_table
            .write([batch(128, 16)])
            .with_save_mode(SaveMode::Append)
            .with_commit_properties(commit_properties(3))
            .await
            .expect("benchmark owner replacement/publication append");
        sample(&mut publication, started);
        assert_eq!(sample_table.version(), Some(3));
        let context = context_for(&sample_table).await;
        publication_checksum = checksum(&query(&context, full_sql).await);
    }

    deltalake::checkpoints::create_checkpoint(&table, None)
        .await
        .expect("create benchmark checkpoint");
    let mut checkpoint_reopen = Vec::with_capacity(SAMPLES);
    let mut checkpoint_checksum = String::new();
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let reopened = DeltaTableBuilder::from_url(location.clone())
            .expect("checkpoint reopen builder")
            .with_version(2)
            .load()
            .await
            .expect("checkpoint reopen");
        sample(&mut checkpoint_reopen, started);
        let context = context_for(&reopened).await;
        checkpoint_checksum = checksum(&query(&context, full_sql).await);
    }

    let observed_rss_bytes = rss_bytes();
    json!({
        "contract": "codefabric-data-fabric-upgrade-benchmark-v2",
        "stack": {
            "arrow": arrow::ARROW_VERSION,
            "datafusion": datafusion::DATAFUSION_VERSION,
        },
        "environment": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
        },
        "observed_peak_rss_bytes": observed_rss_bytes,
        "resource_ceiling_bytes": RESOURCE_CEILING_BYTES,
        "workloads": {
            "activation": workload(activation, "delta-version:2".to_owned()),
            "first_filtered_query": workload(first_query, first_checksum),
            "warmed_filtered_query": workload(warmed_filtered, filtered_checksum),
            "warmed_full_query": workload(warmed_full, full_checksum),
            "owner_replacement_publication": workload(publication, publication_checksum),
            "checkpoint_reopen": workload(checkpoint_reopen, checkpoint_checksum),
        }
    })
}

#[tokio::test]
async fn data_fabric_revision_benchmark_emit() {
    let output = std::path::PathBuf::from(
        std::env::var_os("CODEFABRIC_BENCHMARK_REPORT")
            .expect("CODEFABRIC_BENCHMARK_REPORT is required"),
    );
    fs::write(
        output,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&report().await).expect("benchmark report JSON")
        ),
    )
    .expect("write benchmark report");
}
