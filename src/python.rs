//! PyO3 binding boundary.
//!
//! This module's *location* is a free choice — the specification explicitly declines to
//! require a `src/python.rs` or any other semantic production-code path (spec section
//! 93, step 2). What is architectural is the direction: Python-specific types and
//! exceptions stop here and never reach [`crate`]'s ordinary Rust API (spec sections 6.4
//! and 61.1).
//!
//! The module is compiled only under the `python` feature, so `cargo check` with no
//! features exercises the pure Rust core exactly as a non-Python consumer would see it.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// Translates a core [`crate::Error`] into the documented Python exception contract.
///
/// Centralizing the mapping here is a choice, not a requirement (spec section 6.5); the
/// invariant is only that equivalent Rust error classes map predictably, and that the
/// mapping is covered by Python-facing tests.
fn to_python_error(error: &crate::Error) -> PyErr {
    match error {
        crate::Error::EmptyField { .. } => PyValueError::new_err(error.to_string()),
    }
}

/// The private native extension, imported by Python as `codefabric._native`.
///
/// The declared name must match the final component of Maturin's
/// `module-name = "codefabric._native"` in `pyproject.toml` (spec section 6.3).
#[pymodule(name = "_native")]
mod native {
    use super::{PyResult, to_python_error};
    use pyo3::prelude::*;

    /// Returns the version of the compiled Rust core.
    #[pyfunction]
    fn version() -> &'static str {
        crate::version()
    }

    /// Normalizes a workspace identifier, raising `ValueError` when it is blank.
    #[pyfunction]
    fn normalize_workspace_id(raw: &str) -> PyResult<String> {
        crate::normalize_workspace_id(raw).map_err(|error| to_python_error(&error))
    }
}
