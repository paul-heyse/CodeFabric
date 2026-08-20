//! The repository's single external integration-test target.
//!
//! Cargo compiles every top-level `tests/*.rs` file as its own crate, so additional
//! cases belong in a module under `tests/integration/` rather than in a new top-level
//! file (spec sections 4.2 and 61.3). A second top-level target is justified only by a
//! materially different feature set, process environment, harness, external service,
//! platform restriction, or resource group (spec section 4.3).
//!
//! The inline `mod integration` wrapper is load-bearing: a test target's crate root
//! resolves submodules against `tests/`, so without it `mod errors;` would look for
//! `tests/errors.rs` and each case would need its own top-level file — the exact crate
//! explosion section 4.2 exists to prevent. The wrapper shifts the search into
//! `tests/integration/`.
//!
//! These tests see only the crate's public API. Anything needing private state should be
//! a colocated `#[cfg(test)]` module beside the implementation instead of widening
//! visibility (spec sections 4.1 and 92).

mod integration {
    mod errors;
    mod happy_path;
}
