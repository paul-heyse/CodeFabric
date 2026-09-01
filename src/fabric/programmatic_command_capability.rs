//! Explicit fail-closed command-family dispositions for one programmatic workspace.
//!
//! A programmatic runtime must still install the closed command router even when a family is not
//! part of the admitted application release. These adapters make that absence a typed,
//! diagnostic-backed known failure before any target transaction is prepared. They are production
//! effects, not test probes or default handlers: callers must select one concrete family type and
//! provide the identity of an already persisted capability-gap diagnostic.

use async_trait::async_trait;

use super::command::{
    CommandFailure, CommandKind, CommandRecord, DiagnosticRef, ExecutionOwner, FailureClass,
    FailureCode, ReconciliationObservation, ReductionContext, TransactionRef,
};
use super::command_actor::{CommandPortError, CommitEffectOutcome, PrepareEffectOutcome};
use super::command_effect_router::{
    AdministrationCommandEffectPort, CompactionCommandEffectPort,
    RelationPublicationCommandEffectPort, RetentionCommandEffectPort, RollbackCommandEffectPort,
    SourceWaveCommandEffectPort,
};

/// Why one statically known command family is unavailable in this application release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammaticCommandCapabilityDisposition {
    /// The family is intentionally outside the target programmatic authority.
    Removed,
    /// The family is valid but its required provider or durable capability is unavailable.
    Unavailable,
}

impl ProgrammaticCommandCapabilityDisposition {
    const fn failure(self, diagnostic: DiagnosticRef) -> CommandFailure {
        match self {
            Self::Removed => CommandFailure {
                code: FailureCode::InvalidInput,
                class: FailureClass::Permanent,
                diagnostic,
            },
            Self::Unavailable => CommandFailure {
                code: FailureCode::BackendUnavailable,
                class: FailureClass::RetryableBeforeCommit,
                diagnostic,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProgrammaticCommandCapabilityError {
    #[error("command capability-gap diagnostic identity is absent")]
    MissingDiagnostic,
}

#[derive(Clone, Copy, Debug)]
struct CapabilityGap {
    command_kind: CommandKind,
    failure: CommandFailure,
}

impl CapabilityGap {
    fn try_new(
        command_kind: CommandKind,
        disposition: ProgrammaticCommandCapabilityDisposition,
        diagnostic: DiagnosticRef,
    ) -> Result<Self, ProgrammaticCommandCapabilityError> {
        if diagnostic.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(ProgrammaticCommandCapabilityError::MissingDiagnostic);
        }
        Ok(Self {
            command_kind,
            failure: disposition.failure(diagnostic),
        })
    }

    fn prepare(
        self,
        executing: &CommandRecord,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> Result<PrepareEffectOutcome, CommandPortError> {
        let valid_execution = matches!(
            executing.state(),
            super::command::DurableCommandState::Executing {
                owner: executing_owner,
                ..
            } if executing_owner == owner
        );
        if executing.command().kind() != self.command_kind
            || !valid_execution
            || owner.fence != context.active_fence
        {
            return Err(CommandPortError::CorruptRecord);
        }
        Ok(PrepareEffectOutcome::KnownFailure {
            failure: self.failure,
        })
    }

    const fn command_kind(self) -> CommandKind {
        self.command_kind
    }
}

macro_rules! define_capability_gap_effect {
    ($name:ident, $trait_name:ident, $command_kind:expr, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug)]
        pub struct $name(CapabilityGap);

        impl $name {
            /// Bind this exact family disposition to a persisted diagnostic relation row.
            pub fn try_new(
                disposition: ProgrammaticCommandCapabilityDisposition,
                diagnostic: DiagnosticRef,
            ) -> Result<Self, ProgrammaticCommandCapabilityError> {
                Ok(Self(CapabilityGap::try_new(
                    $command_kind,
                    disposition,
                    diagnostic,
                )?))
            }

            #[must_use]
            pub const fn command_kind(self) -> CommandKind {
                self.0.command_kind()
            }
        }

        #[async_trait]
        impl $trait_name for $name {
            async fn prepare(
                &self,
                executing: &CommandRecord,
                owner: ExecutionOwner,
                context: ReductionContext,
            ) -> Result<PrepareEffectOutcome, CommandPortError> {
                self.0.prepare(executing, owner, context)
            }

            async fn commit(
                &self,
                _prepared: &CommandRecord,
                _owner: ExecutionOwner,
                _transaction: TransactionRef,
                _context: ReductionContext,
            ) -> Result<CommitEffectOutcome, CommandPortError> {
                // This adapter never returns `Prepared`, so commit is structurally unreachable.
                Err(CommandPortError::CorruptRecord)
            }

            async fn reconcile(
                &self,
                _awaiting: &CommandRecord,
                _owner: ExecutionOwner,
                _transaction: TransactionRef,
                _context: ReductionContext,
            ) -> Result<ReconciliationObservation, CommandPortError> {
                // No target transaction can exist for a capability-gap effect.
                Err(CommandPortError::CorruptRecord)
            }
        }
    };
}

define_capability_gap_effect!(
    ProgrammaticSourceWaveCapabilityGap,
    SourceWaveCommandEffectPort,
    CommandKind::PublishSourceWave,
    "Explicit source-wave capability gap for a release without source publication."
);
define_capability_gap_effect!(
    ProgrammaticRelationPublicationCapabilityGap,
    RelationPublicationCommandEffectPort,
    CommandKind::PublishRelations,
    "Explicit relation-publication capability gap for a release without a producer closure."
);
define_capability_gap_effect!(
    ProgrammaticRollbackCapabilityGap,
    RollbackCommandEffectPort,
    CommandKind::RollbackEpoch,
    "Explicit rollback capability gap for a release without retained-target proof."
);
define_capability_gap_effect!(
    ProgrammaticCompactionCapabilityGap,
    CompactionCommandEffectPort,
    CommandKind::CompactRelations,
    "Explicit compaction capability gap for a release without equivalence proof."
);
define_capability_gap_effect!(
    ProgrammaticRetentionCapabilityGap,
    RetentionCommandEffectPort,
    CommandKind::ApplyRetention,
    "Explicit retention capability gap for a release without protected-set closure."
);
define_capability_gap_effect!(
    ProgrammaticAdministrationCapabilityGap,
    AdministrationCommandEffectPort,
    CommandKind::Administer,
    "Explicit administration capability gap for a release without a typed operation adapter."
);

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn every_gap_is_explicitly_typed_and_requires_durable_diagnostic_identity() {
        assert_eq!(
            ProgrammaticSourceWaveCapabilityGap::try_new(
                ProgrammaticCommandCapabilityDisposition::Unavailable,
                DiagnosticRef::from_bytes([0; 32]),
            )
            .unwrap_err(),
            ProgrammaticCommandCapabilityError::MissingDiagnostic
        );

        let diagnostic = DiagnosticRef::from_bytes([7; 32]);
        let source = ProgrammaticSourceWaveCapabilityGap::try_new(
            ProgrammaticCommandCapabilityDisposition::Unavailable,
            diagnostic,
        )
        .unwrap();
        assert_eq!(source.command_kind(), CommandKind::PublishSourceWave);

        let _: Arc<dyn SourceWaveCommandEffectPort> = Arc::new(source);
        let _: Arc<dyn RelationPublicationCommandEffectPort> = Arc::new(
            ProgrammaticRelationPublicationCapabilityGap::try_new(
                ProgrammaticCommandCapabilityDisposition::Unavailable,
                diagnostic,
            )
            .unwrap(),
        );
        let _: Arc<dyn RollbackCommandEffectPort> = Arc::new(
            ProgrammaticRollbackCapabilityGap::try_new(
                ProgrammaticCommandCapabilityDisposition::Unavailable,
                diagnostic,
            )
            .unwrap(),
        );
        let _: Arc<dyn CompactionCommandEffectPort> = Arc::new(
            ProgrammaticCompactionCapabilityGap::try_new(
                ProgrammaticCommandCapabilityDisposition::Unavailable,
                diagnostic,
            )
            .unwrap(),
        );
        let _: Arc<dyn RetentionCommandEffectPort> = Arc::new(
            ProgrammaticRetentionCapabilityGap::try_new(
                ProgrammaticCommandCapabilityDisposition::Unavailable,
                diagnostic,
            )
            .unwrap(),
        );
        let _: Arc<dyn AdministrationCommandEffectPort> = Arc::new(
            ProgrammaticAdministrationCapabilityGap::try_new(
                ProgrammaticCommandCapabilityDisposition::Unavailable,
                diagnostic,
            )
            .unwrap(),
        );
    }
}
