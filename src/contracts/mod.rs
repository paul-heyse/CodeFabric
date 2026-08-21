//! Machine-contract canonicalization, generation, and verification.

#[cfg(feature = "contracts-tooling")]
pub mod artifacts;
#[cfg(feature = "contracts-tooling")]
pub mod catalog;
#[cfg(feature = "contracts-tooling")]
pub mod compiler;
#[cfg(feature = "contracts-tooling")]
pub mod index;
pub mod jcs;
#[cfg(feature = "contracts-tooling")]
pub mod models;
#[cfg(feature = "contracts-tooling")]
pub(crate) mod registry_models;
#[cfg(feature = "contracts-tooling")]
pub(crate) mod schema_artifacts;
#[cfg(feature = "contracts-tooling")]
pub(crate) mod schema_models;
#[cfg(feature = "contracts-tooling")]
pub use registry_models::replay_bounded_registry_ingress;
