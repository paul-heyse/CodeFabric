//! Authority-neutral epoch identities, role isolation, and bounded DataFusion runtime inputs.
//!
//! These application-owned contracts are shared by programmatic assembly, exact activation, and
//! every admitted epoch. They contain no replayed semantic package, bootstrap schema, generated
//! registry, or catalog-authoring authority.

use std::num::NonZeroUsize;
use std::sync::Arc;

use datafusion::common::DataFusionError;
use datafusion::execution::memory_pool::{FairSpillPool, TrackConsumersPool};
use datafusion::execution::runtime_env::{RuntimeEnv, RuntimeEnvBuilder};
use datafusion::prelude::SessionConfig;
use thiserror::Error;

use super::datafusion_cache::DataFusionCachePolicy;

/// The sealed runtime and durable command/activation relations share one canonical epoch type.
pub use super::command::EpochId as FabricEpochId;

/// The single catalog owned by every sealed epoch.
pub const FABRIC_CATALOG: &str = "codefabric";
pub(super) const ARROW_RELEASE: &str = "59.2.0";
pub(super) const DATAFUSION_RELEASE: &str = "55.0.0";

/// Architectural role schemas shared by candidate and sealed sessions.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FabricSchemaRole {
    Source,
    RawTreeSitter,
    RawRuff,
    RawPyrefly,
    RawRustc,
    Fact,
    Derived,
    Proof,
    System,
    Public,
    Storage,
}

impl FabricSchemaRole {
    /// Sole role namespace admitted by programmatic assembly.
    pub const ALL: [Self; 11] = [
        Self::Source,
        Self::RawTreeSitter,
        Self::RawRuff,
        Self::RawPyrefly,
        Self::RawRustc,
        Self::Fact,
        Self::Derived,
        Self::Proof,
        Self::System,
        Self::Public,
        Self::Storage,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::RawTreeSitter => "raw_tree_sitter",
            Self::RawRuff => "raw_ruff",
            Self::RawPyrefly => "raw_pyrefly",
            Self::RawRustc => "raw_rustc",
            Self::Fact => "fact",
            Self::Derived => "derived",
            Self::Proof => "proof",
            Self::System => "system",
            Self::Public => "public",
            Self::Storage => "_storage",
        }
    }
}

/// Invalid bounded runtime input. No implicit or zero resource bound is accepted.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FabricEpochRuntimeConfigError {
    #[error("invalid fabric epoch runtime configuration: {0}")]
    Invalid(String),
}

/// Exact, release-bound execution settings used to create one fresh runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricEpochRuntimeConfig {
    memory_limit_bytes: usize,
    max_spill_bytes: u64,
    max_spill_merge_fan_in: usize,
    tracked_consumer_count: NonZeroUsize,
    batch_size: NonZeroUsize,
    target_partitions: NonZeroUsize,
    collect_statistics: bool,
    cache_policy: DataFusionCachePolicy,
}

impl FabricEpochRuntimeConfig {
    /// Construct an explicitly bounded DataFusion runtime configuration.
    ///
    /// # Errors
    ///
    /// Rejects any zero bound. A runtime with an implicit unbounded resource is invalid.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        memory_limit_bytes: usize,
        max_spill_bytes: u64,
        max_spill_merge_fan_in: usize,
        tracked_consumer_count: usize,
        batch_size: usize,
        target_partitions: usize,
        collect_statistics: bool,
    ) -> Result<Self, FabricEpochRuntimeConfigError> {
        let tracked_consumer_count = NonZeroUsize::new(tracked_consumer_count);
        let batch_size = NonZeroUsize::new(batch_size);
        let target_partitions = NonZeroUsize::new(target_partitions);
        if memory_limit_bytes == 0
            || max_spill_bytes == 0
            || max_spill_merge_fan_in == 0
            || tracked_consumer_count.is_none()
            || batch_size.is_none()
            || target_partitions.is_none()
        {
            return Err(FabricEpochRuntimeConfigError::Invalid(
                "memory, spill, merge fan-in, tracked-consumer, batch, and partition bounds must all be non-zero"
                    .into(),
            ));
        }
        Ok(Self {
            memory_limit_bytes,
            max_spill_bytes,
            max_spill_merge_fan_in,
            tracked_consumer_count: tracked_consumer_count.expect("validated non-zero"),
            batch_size: batch_size.expect("validated non-zero"),
            target_partitions: target_partitions.expect("validated non-zero"),
            collect_statistics,
            cache_policy: DataFusionCachePolicy::proportional_to(memory_limit_bytes),
        })
    }

    /// Replace the bounded cache profile that participates in epoch identity.
    #[must_use]
    pub fn with_cache_policy(mut self, cache_policy: DataFusionCachePolicy) -> Self {
        self.cache_policy = cache_policy;
        self
    }

    #[must_use]
    pub const fn cache_policy(&self) -> &DataFusionCachePolicy {
        &self.cache_policy
    }

    /// Canonical target-runtime identity.
    #[must_use]
    pub fn identity(&self) -> String {
        format!(
            "programmatic-fabric-runtime.v1:arrow={ARROW_RELEASE}:datafusion={DATAFUSION_RELEASE}:memory={}:spill={}:fan-in={}:consumers={}:batch={}:partitions={}:statistics={}:parquet-view-types=false:{}",
            self.memory_limit_bytes,
            self.max_spill_bytes,
            self.max_spill_merge_fan_in,
            self.tracked_consumer_count,
            self.batch_size,
            self.target_partitions,
            self.collect_statistics,
            self.cache_policy.identity_fragment(),
        )
    }

    pub(super) fn session_config(&self) -> SessionConfig {
        SessionConfig::new()
            .with_default_catalog_and_schema(FABRIC_CATALOG, FabricSchemaRole::Public.as_str())
            .with_create_default_catalog_and_schema(false)
            .with_information_schema(true)
            .with_batch_size(self.batch_size.get())
            .with_target_partitions(self.target_partitions.get())
            .set_bool(
                "datafusion.execution.collect_statistics",
                self.collect_statistics,
            )
            .set_bool(
                "datafusion.execution.parquet.schema_force_view_types",
                false,
            )
    }

    pub(super) fn runtime_env(&self) -> Result<Arc<RuntimeEnv>, DataFusionError> {
        self.cache_policy
            .configure_runtime(RuntimeEnvBuilder::new())
            .with_memory_pool(Arc::new(TrackConsumersPool::new(
                FairSpillPool::new(self.memory_limit_bytes),
                self.tracked_consumer_count,
            )))
            .with_max_temp_directory_size(self.max_spill_bytes)
            .with_max_spill_merge_fan_in(self.max_spill_merge_fan_in)
            .build_arc()
    }
}

impl Default for FabricEpochRuntimeConfig {
    fn default() -> Self {
        Self::try_new(
            256 * 1024 * 1024,
            2 * 1024 * 1024 * 1024,
            32,
            16,
            8_192,
            1,
            true,
        )
        .expect("the built-in epoch runtime profile is bounded")
    }
}

pub(super) fn epoch_identity_text(identity: FabricEpochId) -> String {
    hex(identity.as_bytes())
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn programmatic_role_namespace_excludes_predecessor_model_schema() {
        assert_eq!(FabricSchemaRole::ALL.len(), 11);
        let names = FabricSchemaRole::ALL
            .into_iter()
            .map(FabricSchemaRole::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), FabricSchemaRole::ALL.len());
        assert!(!names.contains("model"));
    }

    #[test]
    fn bounded_runtime_identity_is_target_owned_and_zero_bounds_fail_closed() {
        let identity = FabricEpochRuntimeConfig::default().identity();
        assert!(identity.starts_with("programmatic-fabric-runtime.v1:"));
        assert!(identity.contains("arrow=59.2.0:datafusion=55.0.0"));
        assert!(matches!(
            FabricEpochRuntimeConfig::try_new(0, 1, 1, 1, 1, 1, true),
            Err(FabricEpochRuntimeConfigError::Invalid(_))
        ));
    }
}
