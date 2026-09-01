//! Programmatic relational data-fabric modules and shared Arrow contract helpers.

use std::sync::Arc;

use arrow_array::ArrayRef;
use arrow_array::builder::FixedSizeBinaryBuilder;
use arrow_schema::ArrowError;
use thiserror::Error;

pub mod activation;
#[cfg(feature = "daemon")]
pub mod activation_command_effect;
#[cfg(feature = "daemon")]
pub mod activation_control_delta;
#[cfg(feature = "daemon")]
pub mod activation_transaction;
#[cfg(feature = "daemon")]
pub mod administration_command_effect;
#[cfg(feature = "daemon")]
pub mod admission;
pub mod arrow_result_resource;
pub mod child_session;
pub mod command;
#[cfg(feature = "daemon")]
pub mod command_actor;
pub mod command_delta;
#[cfg(feature = "daemon")]
pub(crate) mod command_effect_contract;
#[cfg(feature = "daemon")]
pub mod command_effect_router;
#[cfg(feature = "daemon")]
pub mod command_record_sqlite;
#[cfg(feature = "daemon")]
pub mod command_runtime;
#[cfg(feature = "daemon")]
pub mod command_runtime_manager;
#[cfg(feature = "daemon")]
pub mod command_runtime_ports;
#[cfg(feature = "daemon")]
pub mod compaction_command_effect;
pub mod datafusion_cache;
#[cfg(feature = "daemon")]
pub mod delta_cdf_checkpoint_sqlite;
#[cfg(feature = "daemon")]
pub mod delta_cdf_replay;
pub mod delta_commit_reconciliation;
pub mod delta_exact;
pub mod delta_guarded_maintenance;
pub mod delta_semantic_read;
pub mod delta_write;
pub mod derived_producer_closure;
pub mod effective_view;
pub mod epoch_runtime;
#[cfg(feature = "daemon")]
pub mod explicit_unknown;
#[cfg(feature = "daemon")]
pub mod forward_cutover;
pub mod graph_program;
#[cfg(feature = "daemon")]
pub mod production_kernel;
#[cfg(feature = "daemon")]
pub mod programmatic_activation_admission;
#[cfg(feature = "daemon")]
pub mod programmatic_activation_command_ports;
#[cfg(feature = "daemon")]
pub mod programmatic_activation_command_sqlite;
#[cfg(feature = "daemon")]
pub mod programmatic_active_workspace_builder;
#[cfg(feature = "daemon")]
pub mod programmatic_command_capability;
#[cfg(feature = "daemon")]
pub mod programmatic_command_runtime_factory;
#[cfg(feature = "daemon")]
pub mod programmatic_delta_maintenance_command;
#[cfg(feature = "daemon")]
pub mod programmatic_delta_maintenance_relation;
#[cfg(feature = "daemon")]
pub mod programmatic_delta_runtime;
pub mod programmatic_epoch;
#[cfg(feature = "daemon")]
pub mod programmatic_ingress_port;
pub mod programmatic_observation_delta;
#[cfg(feature = "daemon")]
pub mod programmatic_query_backend;
pub mod programmatic_relation_delta;
pub mod programmatic_schema;
#[cfg(feature = "daemon")]
pub mod programmatic_workspace;
pub mod proof;
pub mod provider;
#[cfg(feature = "daemon")]
pub mod published_arrow_result;
#[cfg(feature = "daemon")]
pub mod query_artifact;
#[cfg(feature = "daemon")]
pub mod query_coordinator;
#[cfg(feature = "daemon")]
pub mod relation_publication_command_effect;
#[cfg(feature = "daemon")]
pub mod relational_query_runtime;
#[cfg(feature = "daemon")]
pub mod request_owned_relation;
mod result_checksum;
#[cfg(feature = "daemon")]
pub mod retention_command_effect;
#[cfg(feature = "daemon")]
pub mod rollback_command_effect;
#[cfg(feature = "daemon")]
pub mod source_context;
#[cfg(feature = "daemon")]
pub mod source_wave_command_effect;
#[cfg(feature = "daemon")]
pub mod streamed_result_package;
#[cfg(feature = "daemon")]
pub mod writer_generation_sqlite;
#[cfg(feature = "daemon")]
pub mod writer_lease;

#[cfg(feature = "daemon")]
pub use query_artifact::{
    QueryArtifactStage, QueryArtifactStageState, QueryExecutionArtifactAccumulator,
    QueryExecutionArtifactEvidence, QueryExecutionContext,
};
pub use result_checksum::{
    GATE_RESULT_CHECKSUM_VERSION, GateResultChecksumV1, ResultChecksumError, ResultChecksumV1,
    ResultChecksumV2, VersionedResultChecksum, batch_checksum, gate_result_checksum_v1,
    result_checksum_for_version, result_checksum_v1, result_checksum_v2,
};

pub(crate) fn id16_array<'a>(values: impl IntoIterator<Item = Option<&'a [u8; 16]>>) -> ArrayRef {
    let mut builder = FixedSizeBinaryBuilder::new(16);
    for value in values {
        if let Some(value) = value {
            builder
                .append_value(value)
                .expect("typed Id16 always has the governed storage width");
        } else {
            builder.append_null();
        }
    }
    Arc::new(builder.finish())
}

pub(crate) fn hash32_array<'a>(values: impl IntoIterator<Item = Option<&'a [u8; 32]>>) -> ArrayRef {
    let mut builder = FixedSizeBinaryBuilder::new(32);
    for value in values {
        if let Some(value) = value {
            builder
                .append_value(value)
                .expect("typed hash always has the governed storage width");
        } else {
            builder.append_null();
        }
    }
    Arc::new(builder.finish())
}

/// Shared failures for the remaining Arrow schema-mapping boundary.
#[derive(Debug, Error)]
pub enum FabricError {
    #[error(transparent)]
    Arrow(#[from] ArrowError),
}
