//! Stable CodeFabric daemon and data-plane library.
//!
//! Wave 0 establishes the production dependency and build boundary. Domain behavior
//! arrives in later packets; [`compatibility`] is the executable contract that keeps
//! the selected library APIs and feature graph honest until those modules replace it.

#[cfg(feature = "canonical-json")]
pub mod analysis_context;
#[cfg(feature = "compatibility-probes")]
pub mod compatibility;
#[cfg(any(feature = "canonical-json", feature = "contracts-tooling"))]
pub mod contracts;
#[cfg(feature = "daemon")]
pub mod coordinator;
#[cfg(feature = "daemon")]
pub mod daemon;
#[cfg(feature = "canonical-json")]
pub mod identity;
#[cfg(feature = "daemon")]
pub mod inventory;
#[cfg(feature = "data-fabric")]
pub mod operational_store;
#[cfg(feature = "daemon")]
pub mod provider_runtime;
/// Generated categorical and lifecycle registry types.
pub mod registries;
#[cfg(feature = "rpc")]
pub mod rpc;
#[cfg(feature = "data-fabric")]
pub mod schema_registry;
#[cfg(feature = "daemon")]
pub mod secure_path;
#[cfg(feature = "canonical-json")]
pub mod snapshot;
#[cfg(feature = "daemon")]
pub mod snapshot_runtime;
#[cfg(feature = "daemon")]
pub mod source_image;
#[cfg(feature = "data-fabric")]
pub mod workspace_registry;

#[cfg(feature = "data-fabric")]
pub mod fabric;
#[cfg(feature = "data-fabric")]
pub mod fact_ingest;
#[cfg(feature = "repository-state")]
pub mod git_state;
