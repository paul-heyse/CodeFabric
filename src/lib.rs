//! Stable CodeFabric daemon and data-plane library.
//!
//! Wave 0 establishes the production dependency and build boundary. Domain behavior
//! arrives in later packets; [`compatibility`] is the executable contract that keeps
//! the selected library APIs and feature graph honest until those modules replace it.

#[cfg(feature = "compatibility-probes")]
pub mod compatibility;
#[cfg(any(feature = "canonical-json", feature = "contracts-tooling"))]
pub mod contracts;
#[cfg(feature = "rpc")]
pub mod rpc;

#[cfg(feature = "data-fabric")]
mod fabric;
#[cfg(feature = "repository-state")]
mod git_state;
