//! CodeFabric Rust core.
//!
//! # Source organization
//!
//! The repository/tooling specification governs package and crate boundaries; it
//! deliberately leaves the semantic decomposition of production source to the
//! application design (spec sections 2 and 93). The module layout in this file is
//! therefore a *free choice*, not a prescription — it may be reorganized freely
//! without touching the repository contract.
//!
//! # Architecture invariant
//!
//! This crate is Python-agnostic. It exposes ordinary Rust types and `Result`-style
//! errors. All PyO3 conversion lives behind the `python` feature (spec sections 6.4
//! and 61.1); the pure Rust core must never require the Python façade to function.
//!
//! # Status
//!
//! The items below are a deliberately minimal seed whose purpose is to prove the
//! toolchain end to end — Rust tests, doctests, the PyO3 boundary, error mapping, and
//! wheel packaging all have something real to exercise. Replace them with actual
//! CodeFabric functionality; nothing here is part of a design.

use std::fmt;

#[cfg(feature = "python")]
mod python;

/// Errors produced by the CodeFabric core.
///
/// Variants are ordinary Rust values. Their translation into Python exceptions is the
/// binding layer's responsibility, not this type's (spec section 6.5).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A required text field was empty, or contained only whitespace.
    EmptyField {
        /// Name of the field, as presented to the caller.
        field: &'static str,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "{field} must not be empty"),
        }
    }
}

impl std::error::Error for Error {}

/// Returns the crate version recorded in `Cargo.toml`.
///
/// ```
/// assert!(!codefabric::version().is_empty());
/// ```
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Normalizes a workspace identifier by trimming surrounding whitespace.
///
/// ```
/// # use codefabric::normalize_workspace_id;
/// assert_eq!(normalize_workspace_id("  my-workspace \n")?, "my-workspace");
/// # Ok::<(), codefabric::Error>(())
/// ```
///
/// # Errors
///
/// Returns [`Error::EmptyField`] when `raw` is empty or contains only whitespace.
pub fn normalize_workspace_id(raw: &str) -> Result<String, Error> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(Error::EmptyField {
            field: "workspace_id",
        });
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{Error, normalize_workspace_id, version};

    #[test]
    fn version_is_reported() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn workspace_id_is_trimmed() {
        assert_eq!(
            normalize_workspace_id("\t codefabric \n").as_deref(),
            Ok("codefabric")
        );
    }

    #[test]
    fn workspace_id_preserves_interior_whitespace() {
        assert_eq!(
            normalize_workspace_id("two words").as_deref(),
            Ok("two words")
        );
    }

    #[test]
    fn blank_workspace_id_is_rejected() {
        assert_eq!(
            normalize_workspace_id("   \t\n"),
            Err(Error::EmptyField {
                field: "workspace_id"
            })
        );
    }

    #[test]
    fn error_renders_the_field_name() {
        let rendered = Error::EmptyField {
            field: "workspace_id",
        }
        .to_string();
        assert_eq!(rendered, "workspace_id must not be empty");
    }
}
