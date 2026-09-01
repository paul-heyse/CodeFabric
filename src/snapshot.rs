//! Released analysis-context projection types used at snapshot-shaped public boundaries.

use serde::{Deserialize, Serialize};

/// One analysis-context identity in a public snapshot projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotContextRecord {
    pub analysis_context_id: String,
    pub context_manifest_digest: String,
    pub capability_partition_digest: String,
}

/// Context selection carried by a public snapshot projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotContexts {
    pub context_set_id: String,
    pub default_python_context_id: Option<String>,
    pub default_rust_context_id: Option<String>,
    pub records: Vec<SnapshotContextRecord>,
}
