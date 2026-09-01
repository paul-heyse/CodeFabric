//! Target-owned query execution artifact identities and phase evidence.
//!
//! These values describe the programmatic query transaction itself. They do not select a
//! compiler, ontology package, serving session, or execution backend.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Identity allocated by the query boundary before any planning begins.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryExecutionContext {
    pub execution_id: String,
    pub semantic_request_id: String,
    pub mcp_call_id: String,
}

/// Availability of one phase artifact in an execution that may terminate early.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueryArtifactStageState {
    NotReached,
    Available,
    Partial,
    Complete,
    UnavailableWithReason,
}

/// Immutable evidence captured as soon as one execution stage is reached.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryArtifactStage {
    pub block_id: String,
    pub stage: String,
    pub state: QueryArtifactStageState,
    pub artifact: Option<String>,
    pub unavailable_reason: Option<String>,
    pub metrics: BTreeMap<String, u64>,
}

/// Phase-complete snapshot of every execution artifact available at a point in time.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryExecutionArtifactEvidence {
    pub execution: QueryExecutionContext,
    pub lifecycle_phase: String,
    pub failing_stage: Option<String>,
    pub stages: Vec<QueryArtifactStage>,
    pub snapshot_id: Option<String>,
    pub publication_id: Option<String>,
    pub source_table_versions: BTreeMap<u16, u64>,
    pub coverage_state: BTreeMap<String, u64>,
    pub partial_metrics: BTreeMap<String, u64>,
}

/// Shared append-only artifact accumulator allocated with execution identity before planning.
#[derive(Clone, Debug)]
pub struct QueryExecutionArtifactAccumulator {
    inner: Arc<std::sync::Mutex<QueryExecutionArtifactEvidence>>,
}

impl QueryExecutionArtifactAccumulator {
    #[must_use]
    pub fn new(execution: QueryExecutionContext) -> Self {
        let stages = [
            "binding",
            "logical_planning",
            "logical_optimization",
            "physical_planning",
            "physical_execution",
            "response_encoding",
        ]
        .into_iter()
        .map(|stage| QueryArtifactStage {
            block_id: "request".to_owned(),
            stage: stage.to_owned(),
            state: QueryArtifactStageState::NotReached,
            artifact: None,
            unavailable_reason: None,
            metrics: BTreeMap::new(),
        })
        .collect();
        Self {
            inner: Arc::new(std::sync::Mutex::new(QueryExecutionArtifactEvidence {
                execution,
                lifecycle_phase: "accepted".to_owned(),
                failing_stage: None,
                stages,
                snapshot_id: None,
                publication_id: None,
                source_table_versions: BTreeMap::new(),
                coverage_state: BTreeMap::new(),
                partial_metrics: BTreeMap::new(),
            })),
        }
    }

    pub fn set_phase(&self, phase: impl Into<String>) {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .lifecycle_phase = phase.into();
    }

    pub fn set_failure(&self, stage: impl Into<String>) {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .failing_stage = Some(stage.into());
    }

    pub fn record_stage(&self, stage: QueryArtifactStage) {
        let mut evidence = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (name, value) in &stage.metrics {
            evidence
                .partial_metrics
                .entry(name.clone())
                .and_modify(|current| *current = (*current).max(*value))
                .or_insert(*value);
        }
        if let Some(existing) = evidence
            .stages
            .iter_mut()
            .find(|existing| existing.block_id == stage.block_id && existing.stage == stage.stage)
        {
            *existing = stage;
        } else {
            evidence.stages.push(stage);
        }
    }

    pub fn record_coverage(&self, name: impl Into<String>, value: u64) {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .coverage_state
            .insert(name.into(), value);
    }

    #[must_use]
    pub fn snapshot(&self) -> QueryExecutionArtifactEvidence {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}
