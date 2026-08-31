//! Replayed relational model authority for the v2 fabric.
//!
//! The model in this module is reconstructed from immutable migrations.  The
//! bootstrap contains only the closed metamodel needed to describe model rows;
//! product facts, rules, queries, and policies enter through migrations.

mod data;
mod release;
mod replay;
mod schema;

pub use data::{ModelDataRow, ModelDataRowBuilder, ModelDataRowReference};
pub use release::{
    CompilerDependency, FabricCompilerRelease, FabricCompilerReleaseBuilder, InstalledIntrinsic,
    IntrinsicInstaller, IntrinsicPrimitive,
};
pub use replay::{
    CompilerReleaseMigration, ModelDecision, ModelEpoch, ModelMigration, ModelOperation,
    ModelRelations, ReplayEngine, RowReference,
};
pub use schema::{
    BootstrapMetamodel, FieldSpec, ModelRelation, ModelRow, ModelRowBuilder, ModelValue, ScalarType,
};

/// Failures are structural and explicit; replay never guesses around missing history.
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("invalid compiler release: {0}")]
    InvalidCompilerRelease(String),
    #[error("invalid intrinsic installation: {0}")]
    InvalidIntrinsicInstallation(String),
    #[error("invalid model row for {relation}: {message}")]
    InvalidRow { relation: String, message: String },
    #[error("invalid migration chain: {0}")]
    InvalidMigrationChain(String),
    #[error("model operation rejected: {0}")]
    OperationRejected(String),
    #[error("model reference closure failed: {0}")]
    ReferenceClosure(String),
    #[error("bootstrap self-description differs: {0}")]
    BootstrapClosure(String),
    #[error("compiler release migration rejected: {0}")]
    ReleaseMigration(String),
    #[error("Arrow model relation failure: {0}")]
    Arrow(#[from] arrow_schema::ArrowError),
}

pub(crate) fn require_identifier(value: &str, context: &str) -> Result<(), ModelError> {
    if value.is_empty()
        || value.len() > 240
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ModelError::InvalidMigrationChain(format!(
            "{context} is not a bounded canonical identifier: {value:?}"
        )));
    }
    Ok(())
}
