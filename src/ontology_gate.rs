//! Once-executed ontology gate action and separate diagnostic execution artifact.

use std::collections::BTreeMap;

use arrow_array::RecordBatch;
use datafusion::error::DataFusionError;
use datafusion::logical_expr::LogicalPlan;
use datafusion::physical_plan::metrics::MetricValue;
use datafusion::physical_plan::{ExecutionPlan, displayable, execute_stream};
use datafusion::prelude::SessionContext;
use futures::StreamExt as _;
use thiserror::Error;

use crate::fabric::{GateResultChecksumV1, ResultChecksumError, gate_result_checksum_v1};

/// Bounded resources for one gate terminal action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateResourceEnvelope {
    pub max_output_rows: usize,
    pub max_output_bytes: usize,
    pub max_output_batches: usize,
    pub max_checksum_encoding_bytes: usize,
}

impl GateResourceEnvelope {
    /// Stable identity included in receipt semantics.
    #[must_use]
    pub fn identity(&self) -> String {
        framed([
            b"ontology-gate-resource-envelope.v1".as_slice(),
            &self.max_output_rows.to_be_bytes(),
            &self.max_output_bytes.to_be_bytes(),
            &self.max_output_batches.to_be_bytes(),
            &self.max_checksum_encoding_bytes.to_be_bytes(),
        ])
    }
}

impl Default for GateResourceEnvelope {
    fn default() -> Self {
        Self {
            max_output_rows: 10_000,
            max_output_bytes: 16 * 1_024 * 1_024,
            max_output_batches: 128,
            max_checksum_encoding_bytes: 16 * 1_024 * 1_024,
        }
    }
}

/// Deterministic resource-contract failures.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GateResourceFailure {
    #[error("ONTOLOGY_GATE_ROW_LIMIT:{observed}>{limit}")]
    Rows { limit: usize, observed: usize },
    #[error("ONTOLOGY_GATE_BYTE_LIMIT:{observed}>{limit}")]
    Bytes { limit: usize, observed: usize },
    #[error("ONTOLOGY_GATE_BATCH_LIMIT:{observed}>{limit}")]
    Batches { limit: usize, observed: usize },
    #[error("ONTOLOGY_GATE_COUNTER_OVERFLOW:{0}")]
    Counter(&'static str),
}

/// Gate runner failure. No variant carries activation or pointer authority.
#[derive(Debug, Error)]
pub enum OntologyGateError {
    #[error(transparent)]
    DataFusion(#[from] DataFusionError),
    #[error(transparent)]
    Checksum(#[from] ResultChecksumError),
    #[error(transparent)]
    Resource(#[from] GateResourceFailure),
}

/// Diagnostic-only material collected from the exhausted physical-plan instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateExecutionArtifact {
    pub execution_id: String,
    pub candidate_id: String,
    pub action_id: String,
    pub terminal_action_count: u16,
    pub physical_plan_diagnostic: String,
    pub metrics: BTreeMap<String, u64>,
    pub artifact_identity: String,
}

/// Semantic gate evidence. Metrics and plan text are deliberately outside receipt identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateReceiptEvidence {
    pub execution_id: String,
    pub candidate_id: String,
    pub action_id: String,
    pub gate_checksum: GateResultChecksumV1,
    pub resource_contract_identity: String,
    pub diagnostic_artifact_identity: String,
    pub receipt_identity: String,
}

/// Complete result of the single terminal action.
#[derive(Clone, Debug)]
pub struct OntologyGateOutcome {
    pub batches: Vec<RecordBatch>,
    pub artifact: GateExecutionArtifact,
    pub receipt: GateReceiptEvidence,
}

fn framed(parts: impl IntoIterator<Item = impl AsRef<[u8]>>) -> String {
    let mut bytes = Vec::new();
    for part in parts {
        let part = part.as_ref();
        bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
        bytes.extend_from_slice(part);
    }
    crate::integrity::framed_digest(&bytes)
}

/// Recompute diagnostic artifact identity. Metrics are sorted by `BTreeMap`, but remain
/// diagnostic: this identity is never an input to [`gate_receipt_identity`].
#[must_use]
pub fn gate_artifact_identity(artifact: &GateExecutionArtifact) -> String {
    framed(
        [
            artifact.execution_id.as_bytes().to_vec(),
            artifact.candidate_id.as_bytes().to_vec(),
            artifact.action_id.as_bytes().to_vec(),
            artifact.terminal_action_count.to_be_bytes().to_vec(),
            artifact.physical_plan_diagnostic.as_bytes().to_vec(),
        ]
        .into_iter()
        .chain(
            artifact
                .metrics
                .iter()
                .map(|(name, value)| format!("{name}:{value}").into_bytes()),
        ),
    )
}

/// Compute receipt identity without metric names, metric values, plan text, or artifact digest.
#[must_use]
pub fn gate_receipt_identity(receipt: &GateReceiptEvidence) -> String {
    framed([
        b"ontology-gate-receipt.v1".as_slice(),
        receipt.execution_id.as_bytes(),
        receipt.candidate_id.as_bytes(),
        receipt.action_id.as_bytes(),
        receipt.gate_checksum.checksum.as_bytes(),
        receipt.resource_contract_identity.as_bytes(),
    ])
}

fn metric_map(plan: &dyn ExecutionPlan) -> BTreeMap<String, u64> {
    fn visit(plan: &dyn ExecutionPlan, output: &mut BTreeMap<String, u64>) {
        if let Some(metrics) = plan.metrics() {
            *output.entry("output_rows".into()).or_default() = output
                .get("output_rows")
                .copied()
                .unwrap_or_default()
                .saturating_add(metrics.output_rows().unwrap_or_default() as u64);
            *output.entry("spill_count".into()).or_default() = output
                .get("spill_count")
                .copied()
                .unwrap_or_default()
                .saturating_add(metrics.spill_count().unwrap_or_default() as u64);
            *output.entry("spilled_bytes".into()).or_default() = output
                .get("spilled_bytes")
                .copied()
                .unwrap_or_default()
                .saturating_add(metrics.spilled_bytes().unwrap_or_default() as u64);
            for metric in metrics.iter() {
                if let MetricValue::PruningMetrics {
                    pruning_metrics, ..
                } = metric.value()
                {
                    *output.entry("pruned_row_groups".into()).or_default() = output
                        .get("pruned_row_groups")
                        .copied()
                        .unwrap_or_default()
                        .saturating_add(pruning_metrics.pruned() as u64);
                }
            }
        }
        for child in plan.children() {
            visit(child.as_ref(), output);
        }
    }
    let mut output = BTreeMap::new();
    visit(plan, &mut output);
    output
}

/// Execute one logical plan exactly once, checksum its exhausted result, and only then collect
/// metrics from that same physical plan.
///
/// # Errors
///
/// Returns typed planning, stream, checksum, or deterministic resource failures. The function
/// has no durable store/pointer parameter, so every failure leaves activation authority unchanged.
pub async fn execute_ontology_gate_once(
    context: &SessionContext,
    plan: &LogicalPlan,
    execution_id: &str,
    candidate_id: &str,
    action_id: &str,
    limits: &GateResourceEnvelope,
) -> Result<OntologyGateOutcome, OntologyGateError> {
    let optimized = context.state().optimize(plan)?;
    let physical = context.state().create_physical_plan(&optimized).await?;
    let mut stream = execute_stream(physical.clone(), context.task_ctx())?;
    let mut batches = Vec::new();
    let mut rows = 0_usize;
    let mut bytes = 0_usize;
    while let Some(batch) = stream.next().await {
        let batch = batch?;
        rows = rows
            .checked_add(batch.num_rows())
            .ok_or(GateResourceFailure::Counter("rows"))?;
        bytes = bytes
            .checked_add(batch.get_array_memory_size())
            .ok_or(GateResourceFailure::Counter("bytes"))?;
        let batch_count = batches
            .len()
            .checked_add(1)
            .ok_or(GateResourceFailure::Counter("batches"))?;
        if rows > limits.max_output_rows {
            return Err(GateResourceFailure::Rows {
                limit: limits.max_output_rows,
                observed: rows,
            }
            .into());
        }
        if bytes > limits.max_output_bytes {
            return Err(GateResourceFailure::Bytes {
                limit: limits.max_output_bytes,
                observed: bytes,
            }
            .into());
        }
        if batch_count > limits.max_output_batches {
            return Err(GateResourceFailure::Batches {
                limit: limits.max_output_batches,
                observed: batch_count,
            }
            .into());
        }
        batches.push(batch);
    }
    drop(stream);
    let gate_checksum = gate_result_checksum_v1(
        physical.schema().as_ref(),
        &batches,
        limits.max_checksum_encoding_bytes,
    )?;
    let resource_contract_identity = limits.identity();
    let mut artifact = GateExecutionArtifact {
        execution_id: execution_id.into(),
        candidate_id: candidate_id.into(),
        action_id: action_id.into(),
        terminal_action_count: 1,
        physical_plan_diagnostic: displayable(physical.as_ref()).indent(true).to_string(),
        metrics: metric_map(physical.as_ref()),
        artifact_identity: String::new(),
    };
    artifact.artifact_identity = gate_artifact_identity(&artifact);
    let mut receipt = GateReceiptEvidence {
        execution_id: execution_id.into(),
        candidate_id: candidate_id.into(),
        action_id: action_id.into(),
        gate_checksum,
        resource_contract_identity,
        diagnostic_artifact_identity: artifact.artifact_identity.clone(),
        receipt_identity: String::new(),
    };
    receipt.receipt_identity = gate_receipt_identity(&receipt);
    Ok(OntologyGateOutcome {
        batches,
        artifact,
        receipt,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow_array::{
        Array as _, ArrayRef, Int64Array, MapArray, RecordBatch, StringArray, StructArray,
    };
    use arrow_buffer::OffsetBuffer;
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::logical_expr::{col, lit};
    use datafusion::prelude::SessionContext;

    use super::{
        GateResourceEnvelope, GateResourceFailure, OntologyGateError, execute_ontology_gate_once,
        gate_artifact_identity, gate_receipt_identity,
    };
    use crate::fabric::{ResultChecksumError, gate_result_checksum_v1};

    fn map_batch(keys: Vec<&str>) -> RecordBatch {
        let entry_fields = vec![
            Arc::new(Field::new("keys", DataType::Utf8, false)),
            Arc::new(Field::new("values", DataType::Int64, true)),
        ];
        let values = (0..keys.len())
            .map(|value| i64::try_from(value).expect("fixture value"))
            .collect::<Vec<_>>();
        let entries = StructArray::new(
            entry_fields.clone().into(),
            vec![
                Arc::new(StringArray::from(keys)) as ArrayRef,
                Arc::new(Int64Array::from(values)),
            ],
            None,
        );
        let entry = Arc::new(Field::new(
            "entries",
            DataType::Struct(entry_fields.into()),
            false,
        ));
        let map = MapArray::new(
            Arc::clone(&entry),
            OffsetBuffer::from_lengths([entries.len()]),
            entries,
            None,
            true,
        );
        RecordBatch::try_new(
            Arc::new(Schema::new(vec![Field::new(
                "map",
                DataType::Map(entry, true),
                false,
            )])),
            vec![Arc::new(map)],
        )
        .expect("map batch")
    }

    #[test]
    fn ontology_gate_checksum_canonical_kats() {
        let sorted = map_batch(vec!["a", "b"]);
        assert!(
            gate_result_checksum_v1(
                sorted.schema().as_ref(),
                std::slice::from_ref(&sorted),
                1_048_576,
            )
            .is_ok()
        );
        for keys in [vec!["b", "a"], vec!["a", "a"]] {
            let invalid = map_batch(keys);
            assert!(matches!(
                gate_result_checksum_v1(invalid.schema().as_ref(), &[invalid], 1_048_576),
                Err(ResultChecksumError::UnorderedMap)
            ));
        }
    }

    fn plan() -> (SessionContext, datafusion::logical_expr::LogicalPlan) {
        let context = SessionContext::new();
        let batch = RecordBatch::try_from_iter([(
            "value",
            Arc::new(Int64Array::from(vec![1, 2, 3])) as ArrayRef,
        )])
        .expect("fixture batch");
        let plan = context
            .read_batch(batch)
            .expect("fixture frame")
            .filter(col("value").gt(lit(1_i64)))
            .expect("fixture filter")
            .into_unoptimized_plan();
        (context, plan)
    }

    #[tokio::test]
    async fn ontology_gate_single_execution_metric_closure() {
        let (context, plan) = plan();
        let outcome = execute_ontology_gate_once(
            &context,
            &plan,
            "execution-1",
            "candidate-1",
            "gate-1",
            &GateResourceEnvelope::default(),
        )
        .await
        .expect("gate outcome");
        assert_eq!(outcome.artifact.terminal_action_count, 1);
        assert_eq!(outcome.receipt.gate_checksum.row_count, 2);
        assert!(outcome.artifact.metrics.contains_key("output_rows"));
    }

    #[tokio::test]
    async fn ontology_gate_artifact_identity_separation() {
        let (context, plan) = plan();
        let outcome = execute_ontology_gate_once(
            &context,
            &plan,
            "execution-2",
            "candidate-1",
            "gate-1",
            &GateResourceEnvelope::default(),
        )
        .await
        .expect("gate outcome");
        let receipt_identity = gate_receipt_identity(&outcome.receipt);
        let mut diagnostic = outcome.artifact.clone();
        diagnostic.metrics.insert("renamed_metric".into(), 99);
        assert_ne!(
            gate_artifact_identity(&diagnostic),
            outcome.artifact.artifact_identity
        );
        assert_eq!(receipt_identity, outcome.receipt.receipt_identity);
    }

    #[tokio::test]
    async fn ontology_candidate_resource_failure_no_mutation() {
        let limits = GateResourceEnvelope {
            max_output_rows: 1,
            ..GateResourceEnvelope::default()
        };
        let mut observed = Vec::new();
        for execution in ["limited-1", "limited-2"] {
            let (context, plan) = plan();
            let error = execute_ontology_gate_once(
                &context,
                &plan,
                execution,
                "candidate-1",
                "gate-1",
                &limits,
            )
            .await
            .expect_err("row limit");
            let OntologyGateError::Resource(resource) = error else {
                panic!("unexpected gate failure");
            };
            observed.push(resource);
        }
        assert_eq!(observed[0], observed[1]);
        assert_eq!(
            observed[0],
            GateResourceFailure::Rows {
                limit: 1,
                observed: 2
            }
        );
    }
}
