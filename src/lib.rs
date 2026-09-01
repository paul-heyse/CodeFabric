//! Stable CodeFabric daemon and data-plane library.
//!
//! Wave 0 establishes the production dependency and build boundary. Domain behavior lives in the
//! production modules; [`compatibility`] is a compile- and test-only pinned-library probe tier,
//! never a runtime application contract.

#[cfg(any(
    feature = "canonical-json",
    feature = "daemon",
    feature = "data-fabric",
    feature = "fact-generation",
    feature = "repository-state"
))]
pub mod analysis_context;
pub mod cancellation;
#[cfg(feature = "daemon")]
pub mod common_derived_analysis;
#[cfg(feature = "compatibility-probes")]
pub mod compatibility;
#[cfg(any(feature = "canonical-json", feature = "contract-models"))]
pub mod contracts;
#[cfg(feature = "daemon")]
pub mod daemon;
#[cfg(any(
    feature = "canonical-json",
    feature = "contract-models",
    feature = "daemon",
    feature = "data-fabric",
    feature = "fact-generation",
    feature = "repository-state"
))]
pub mod error;
#[cfg(feature = "daemon")]
pub mod forward_cutover_controller;
#[cfg(feature = "daemon")]
pub mod freshness;
#[cfg(any(
    feature = "canonical-json",
    feature = "daemon",
    feature = "data-fabric",
    feature = "fact-generation",
    feature = "repository-state"
))]
pub mod identity;
/// Released application-owned identity recipe primitives.
#[cfg(any(
    feature = "canonical-json",
    feature = "daemon",
    feature = "data-fabric",
    feature = "fact-generation",
    feature = "repository-state"
))]
pub(crate) mod identity_recipes;
#[cfg(any(
    feature = "canonical-json",
    feature = "fact-generation",
    feature = "repository-state"
))]
pub mod integrity;
#[cfg(feature = "daemon")]
pub mod inventory;
#[cfg(feature = "data-fabric")]
pub mod operational_store;
#[cfg(feature = "daemon")]
pub mod production_provider_recipe;
#[cfg(feature = "daemon")]
pub mod production_query_recipe;
#[cfg(feature = "daemon")]
pub mod programmatic_derived_analysis;
#[cfg(feature = "daemon")]
pub mod provider_admission;
#[cfg(feature = "data-fabric")]
pub mod provider_boundary;
#[cfg(feature = "daemon")]
pub mod provider_capability;
#[cfg(all(feature = "data-fabric", feature = "fact-generation"))]
pub mod provider_native_syntax;
#[cfg(feature = "fact-generation")]
pub mod provider_raw_kinds;
#[cfg(feature = "daemon")]
pub mod provider_sandbox;
#[cfg(feature = "fact-generation")]
pub mod provider_types;
#[cfg(feature = "daemon")]
pub mod pyrefly_service;
#[cfg(feature = "daemon")]
pub mod python_context;
#[cfg(feature = "daemon")]
pub mod python_derived_analysis;
#[cfg(feature = "daemon")]
pub mod query_service;
/// Application-owned released categorical and lifecycle wire types.
#[cfg(any(
    feature = "canonical-json",
    feature = "contract-models",
    feature = "daemon",
    feature = "data-fabric",
    feature = "fact-generation",
    feature = "repository-state",
    feature = "rpc"
))]
pub mod registries;
#[cfg(feature = "data-fabric")]
pub mod relation_ipc;
#[cfg(feature = "data-fabric")]
pub(crate) mod relation_ipc_contract;
#[cfg(feature = "daemon")]
pub(crate) mod semantic_query_contract;
#[cfg(feature = "daemon")]
pub(crate) use rpc::generated::codefabric::provider::v1 as relation_ipc_proto_types;
#[cfg(feature = "daemon")]
pub(crate) mod relation_ipc_proto;
#[cfg(feature = "daemon")]
pub(crate) mod relation_ipc_wire;
#[cfg(feature = "data-fabric")]
pub mod relational_program;
#[cfg(feature = "daemon")]
pub mod relational_semantic_query;
#[cfg(feature = "rpc")]
pub mod rpc;
#[cfg(feature = "fact-generation")]
pub mod ruff_adapter;
#[cfg(feature = "daemon")]
pub mod rust_compilation_trust;
#[cfg(feature = "daemon")]
pub mod rust_mir_derived_analysis;
#[cfg(feature = "daemon")]
pub(crate) mod rustc_relation_schema;
#[cfg(feature = "daemon")]
pub mod rustc_service;
#[cfg(feature = "data-fabric")]
pub mod schema_contract;
#[cfg(feature = "data-fabric")]
pub mod schema_registry;
#[cfg(feature = "daemon")]
pub mod secure_path;
#[cfg(feature = "daemon")]
pub mod security;
#[cfg(any(
    feature = "canonical-json",
    feature = "daemon",
    feature = "data-fabric",
    feature = "fact-generation",
    feature = "repository-state"
))]
pub mod snapshot;
#[cfg(feature = "daemon")]
pub mod source_image;
#[cfg(feature = "data-fabric")]
pub mod workspace_registry;

#[cfg(feature = "data-fabric")]
pub mod fabric;
#[cfg(feature = "repository-state")]
pub mod git_state;
#[cfg(feature = "fact-generation")]
pub mod tree_sitter_adapter;

#[cfg(all(test, feature = "daemon"))]
mod production_evidence_core_tests;
#[cfg(all(test, feature = "daemon"))]
mod production_evidence_tests;
