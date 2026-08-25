//! Phase-carrying public error envelope backed by the generated error registry.

use std::error::Error;
use std::fmt;

use crate::registries::{PHASE_VALUES, Phase, PublicErrorEntry, public_error, registry_state_name};

/// One boundary failure with structural phase, registry identity, and incremental trace.
#[derive(Debug)]
pub struct ErrorEnvelope<E, T = Phase> {
    pub phase: Phase,
    pub identity: &'static PublicErrorEntry,
    pub trace: Vec<T>,
    source: E,
}

impl<E, T> ErrorEnvelope<E, T> {
    /// Construct an envelope only from a generated public-error member.
    ///
    /// # Panics
    ///
    /// Panics when a caller attempts to publish an unregistered error identity. This is an
    /// internal invariant violation also prevented statically by `public-error-closure-check`.
    #[must_use]
    pub fn new(phase: Phase, code: &'static str, trace: Vec<T>, source: E) -> Self {
        let identity = public_error(code).expect("boundary error identity must be registered");
        Self {
            phase,
            identity,
            trace,
            source,
        }
    }

    #[must_use]
    pub const fn source_error(&self) -> &E {
        &self.source
    }

    #[must_use]
    pub fn into_source(self) -> E {
        self.source
    }
}

impl<E: fmt::Display, T: fmt::Debug> fmt::Display for ErrorEnvelope<E, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let phase = registry_state_name(PHASE_VALUES, self.phase as u16).unwrap_or("UNKNOWN_PHASE");
        write!(formatter, "{}:{phase}:{}", self.identity.name, self.source)
    }
}

impl<E, T> Error for ErrorEnvelope<E, T>
where
    E: Error + 'static,
    T: fmt::Debug,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wp61_behavioral_acceptance() {
        for registered in PHASE_VALUES {
            let phase = Phase::try_from(registered.code).expect("generated phase code");
            let error = ErrorEnvelope::new(
                phase,
                "INVALID_REQUEST_SCHEMA",
                vec![phase],
                std::io::Error::other(format!("injected at {}", registered.name)),
            );
            assert_eq!(error.phase, phase);
            assert_eq!(error.trace, [phase]);
            assert_eq!(error.identity.name, "INVALID_REQUEST_SCHEMA");
            assert_eq!(error.identity.grpc_status, "INVALID_ARGUMENT");
            assert_eq!(error.identity.retryability, "NEVER");
            assert!(error.to_string().contains(registered.name));
        }
    }
}
