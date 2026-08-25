//! Stable CodeFabric daemon and data-plane library.
//!
//! Wave 0 establishes the production dependency and build boundary. Domain behavior lives in the
//! production modules; [`compatibility`] is a compile- and test-only pinned-library probe tier,
//! never a runtime application contract.

#[cfg(feature = "canonical-json")]
pub mod analysis_context;
#[cfg(feature = "compatibility-probes")]
pub mod compatibility;
#[cfg(feature = "daemon")]
pub mod continuous;
#[cfg(any(feature = "canonical-json", feature = "contract-models"))]
pub mod contracts;
#[cfg(feature = "daemon")]
pub mod coordinator;
#[cfg(feature = "daemon")]
pub mod core_facts;
#[cfg(feature = "daemon")]
pub mod daemon;
#[cfg(feature = "daemon")]
pub mod derivation;
#[cfg(feature = "daemon")]
pub mod golden_corpus;
#[cfg(feature = "canonical-json")]
pub mod identity;
#[cfg(any(
    feature = "canonical-json",
    feature = "fact-generation",
    feature = "model-compiler",
    feature = "repository-state"
))]
pub mod integrity;
#[cfg(feature = "daemon")]
pub mod inventory;
#[cfg(feature = "daemon")]
pub mod lifecycle;
/// Model-generated exhaustive bindings. The generated aggregator owns its member list.
#[cfg(any(feature = "canonical-json", feature = "fact-generation"))]
#[path = "generated/model.rs"]
pub(crate) mod model_generated;
#[cfg(feature = "data-fabric")]
pub mod operational_store;
#[cfg(feature = "fact-generation")]
pub mod provider_raw_kinds;
#[cfg(feature = "daemon")]
pub mod provider_runtime;
#[cfg(feature = "fact-generation")]
pub mod provider_types;
#[cfg(feature = "daemon")]
pub mod query_service;
/// Generated categorical and lifecycle registry types.
#[cfg(any(
    feature = "contract-models",
    feature = "daemon",
    feature = "data-fabric",
    feature = "fact-generation",
    feature = "repository-state"
))]
pub mod registries;
#[cfg(feature = "rpc")]
pub mod rpc;
#[cfg(feature = "fact-generation")]
pub mod ruff_adapter;
#[cfg(feature = "daemon")]
pub mod rustc_service;
#[cfg(feature = "data-fabric")]
pub mod schema_registry;
#[cfg(feature = "daemon")]
pub mod secure_path;
#[cfg(feature = "daemon")]
pub mod security;
#[cfg(feature = "daemon")]
pub mod semantic_query;
#[cfg(feature = "canonical-json")]
pub mod snapshot;
#[cfg(feature = "daemon")]
pub mod snapshot_runtime;
#[cfg(feature = "daemon")]
pub mod source_image;
#[cfg(feature = "daemon")]
pub mod source_syntax;
#[cfg(feature = "data-fabric")]
pub mod workspace_registry;

#[cfg(feature = "data-fabric")]
pub mod fabric;
#[cfg(feature = "data-fabric")]
pub mod fact_ingest;
#[cfg(feature = "repository-state")]
pub mod git_state;
#[cfg(feature = "fact-generation")]
pub mod tree_sitter_adapter;
