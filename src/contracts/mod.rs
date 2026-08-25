//! Shared typed contract models and model-derived runtime provenance.

#[cfg(feature = "contract-models")]
pub mod catalog;
#[cfg(any(
    all(feature = "contract-models", not(feature = "model-compiler")),
    feature = "daemon",
    feature = "data-fabric",
    feature = "fact-generation",
    feature = "repository-state"
))]
pub mod index;
pub mod jcs;
#[cfg(feature = "contract-models")]
pub mod models;
#[cfg(feature = "contract-models")]
#[allow(dead_code)] // The shared generated-registry ingress surface exceeds current runtime use.
pub(crate) mod registry_models;
#[cfg(feature = "contract-models")]
pub use registry_models::replay_bounded_registry_ingress;
