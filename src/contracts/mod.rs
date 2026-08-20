//! Machine-contract canonicalization, generation, and verification.

#[cfg(feature = "contracts-tooling")]
pub mod artifacts;
pub mod jcs;

/// Deterministically generated contract-index types and values.
#[cfg(feature = "contracts-tooling")]
pub mod generated {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/generated/contracts.rs"
    ));
}
