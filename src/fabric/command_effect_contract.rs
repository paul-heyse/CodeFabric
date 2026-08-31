//! Shared execution/recovery validation for typed durable command effects.
//!
//! Command families remain distinct because the lifecycle wire variants are genuinely static.
//! Their reducer-state, transaction, and fencing rules are not distinct. Centralizing those rules
//! prevents one effect from weakening receipt authority or conflating the immutable transaction
//! executor with the actor performing a later recovery read.

use super::command::{
    CommandKind, CommandRecord, DurableCommandState, ExecutionOwner, FabricCommand,
    ReductionContext, TransactionRef, WriterGeneration,
};
use super::command_actor::CommandPortError;

/// Reducer-validated immutable transaction attempt shared by typed effect adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ValidatedCommandAttempt {
    command: FabricCommand,
    attempt: u32,
    execution_owner: ExecutionOwner,
}

impl ValidatedCommandAttempt {
    /// Exact admitted command.
    #[must_use]
    pub(crate) const fn command(self) -> FabricCommand {
        self.command
    }

    /// Borrow the exact admitted command without weakening token construction.
    #[must_use]
    pub(crate) const fn command_ref(&self) -> &FabricCommand {
        &self.command
    }

    /// Reducer-owned attempt number.
    #[must_use]
    pub(crate) const fn attempt(self) -> u32 {
        self.attempt
    }

    /// Immutable actor/fence that prepared and attempted the transaction.
    #[must_use]
    pub(crate) const fn execution_owner(self) -> ExecutionOwner {
        self.execution_owner
    }

    #[cfg(test)]
    pub(crate) const fn for_test(
        command: FabricCommand,
        attempt: u32,
        execution_owner: ExecutionOwner,
    ) -> Self {
        Self {
            command,
            attempt,
            execution_owner,
        }
    }

    /// Require an effect receipt to name the exact writer generation that attempted the
    /// transaction. Numeric ranges are never sufficient evidence.
    pub(crate) fn validate_receipt_generation(
        self,
        writer_generation: WriterGeneration,
    ) -> Result<(), CommandPortError> {
        if writer_generation == self.execution_owner.fence.generation {
            Ok(())
        } else {
            Err(CommandPortError::CorruptRecord)
        }
    }
}

/// Exact transaction attempt plus the independently advancing authority for recovery reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ValidatedCommandRecovery {
    attempt: ValidatedCommandAttempt,
    active_recovery_owner: ExecutionOwner,
}

impl ValidatedCommandRecovery {
    /// Original transaction attempt and execution authority.
    #[must_use]
    pub(crate) const fn attempt(self) -> ValidatedCommandAttempt {
        self.attempt
    }

    /// Current actor/fence authorized to query marker history and converge projections.
    #[must_use]
    pub(crate) const fn active_recovery_owner(self) -> ExecutionOwner {
        self.active_recovery_owner
    }

    #[cfg(test)]
    pub(crate) const fn for_test(
        attempt: ValidatedCommandAttempt,
        active_recovery_owner: ExecutionOwner,
    ) -> Self {
        Self {
            attempt,
            active_recovery_owner,
        }
    }
}

/// Validate an `Executing` record immediately before read-only preparation.
pub(crate) fn executing_attempt(
    record: &CommandRecord,
    owner: ExecutionOwner,
    context: ReductionContext,
    expected_kind: CommandKind,
) -> Result<ValidatedCommandAttempt, CommandPortError> {
    validate_context(record, owner, context, true)?;
    validate_kind(record, expected_kind)?;
    let DurableCommandState::Executing {
        attempt,
        owner: recorded,
    } = record.state()
    else {
        return Err(CommandPortError::CorruptRecord);
    };
    if recorded != owner {
        return Err(CommandPortError::ContextUnavailable);
    }
    validated_attempt(record, attempt, owner)
}

/// Validate a `CommitPrepared` record at the sole durable effect boundary.
pub(crate) fn prepared_attempt(
    record: &CommandRecord,
    owner: ExecutionOwner,
    transaction: TransactionRef,
    context: ReductionContext,
    expected_kind: CommandKind,
) -> Result<ValidatedCommandAttempt, CommandPortError> {
    validate_context(record, owner, context, true)?;
    validate_kind(record, expected_kind)?;
    let DurableCommandState::CommitPrepared {
        attempt,
        owner: recorded,
        transaction: prepared,
    } = record.state()
    else {
        return Err(CommandPortError::CorruptRecord);
    };
    if recorded != owner || prepared != transaction {
        return Err(CommandPortError::ContextUnavailable);
    }
    validated_attempt(record, attempt, owner)
}

/// Validate an exact marker/control-history read without changing execution authority.
pub(crate) fn reconciliation_attempt(
    record: &CommandRecord,
    active_recovery_owner: ExecutionOwner,
    transaction: TransactionRef,
    context: ReductionContext,
    expected_kind: CommandKind,
) -> Result<ValidatedCommandRecovery, CommandPortError> {
    validate_context(record, active_recovery_owner, context, false)?;
    validate_kind(record, expected_kind)?;
    let DurableCommandState::AwaitingReconciliation {
        attempt,
        execution_owner,
        recovery_owner,
        transaction: prepared,
        ..
    } = record.state()
    else {
        return Err(CommandPortError::CorruptRecord);
    };
    validate_owner_advance(execution_owner, recovery_owner, true)?;
    validate_owner_advance(recovery_owner, active_recovery_owner, false)?;
    if prepared != transaction {
        return Err(CommandPortError::ContextUnavailable);
    }
    Ok(ValidatedCommandRecovery {
        attempt: validated_attempt(record, attempt, execution_owner)?,
        active_recovery_owner,
    })
}

fn validate_context(
    record: &CommandRecord,
    owner: ExecutionOwner,
    context: ReductionContext,
    require_expected_head: bool,
) -> Result<(), CommandPortError> {
    if context.active_fence != owner.fence {
        return Err(CommandPortError::ContextUnavailable);
    }
    if require_expected_head && context.current_head != record.command().expected_head {
        return Err(CommandPortError::ContextUnavailable);
    }
    Ok(())
}

fn validate_kind(
    record: &CommandRecord,
    expected_kind: CommandKind,
) -> Result<(), CommandPortError> {
    if record.command().kind() == expected_kind {
        Ok(())
    } else {
        Err(CommandPortError::CorruptRecord)
    }
}

fn validated_attempt(
    record: &CommandRecord,
    attempt: u32,
    execution_owner: ExecutionOwner,
) -> Result<ValidatedCommandAttempt, CommandPortError> {
    let admitted = record.command().writer_fence;
    if execution_owner.fence != admitted
        && execution_owner.fence.generation.get() <= admitted.generation.get()
    {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(ValidatedCommandAttempt {
        command: *record.command(),
        attempt,
        execution_owner,
    })
}

fn validate_owner_advance(
    prior: ExecutionOwner,
    observed: ExecutionOwner,
    corrupt_on_failure: bool,
) -> Result<(), CommandPortError> {
    if prior == observed || observed.fence.generation.get() > prior.fence.generation.get() {
        return Ok(());
    }
    if corrupt_on_failure {
        Err(CommandPortError::CorruptRecord)
    } else {
        Err(CommandPortError::ContextUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::command::{
        ActorId, AdministrationAction, AdministrationRequestRef, AdmissionContext,
        AdmissionOutcome, AuthorizationDecision, AuthorizationRef, CommandEvent, CommandIdentity,
        CommandOwnership, CommandPins, CommandReducer, CompilerReleaseRef, ExpectedHead,
        FabricCommandPayload, IdempotencyKey, LeaseId, ModelHeadRef, OperationId, PrincipalId,
        ProviderSetRef, ReconciliationEvidenceRef, ReconciliationObservation, ResourceEnvelopeRef,
        SourceGeneration, UnknownCommit, UnknownCommitReason, WorkspaceId, WriterFence,
    };

    #[test]
    fn recovery_preserves_execution_authority_across_multiple_active_fences() {
        let execution = owner(1, 1, 1);
        let first_recovery = owner(2, 2, 2);
        let second_recovery = owner(3, 3, 3);
        let transaction = TransactionRef::from_bytes([0x31; 32]);
        let prepared = prepared(command(execution.fence), execution, transaction);
        let awaiting = CommandReducer::reduce(
            &prepared,
            CommandEvent::ReportUnknownCommit {
                owner: first_recovery,
                transaction,
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ProcessInterrupted,
                    diagnostic: crate::fabric::command::DiagnosticRef::from_bytes([0x32; 32]),
                },
            },
            context(&prepared, first_recovery.fence),
        )
        .expect("new writer records unknown transaction")
        .record;
        let indeterminate = CommandReducer::reduce(
            &awaiting,
            CommandEvent::ObserveReconciliation {
                owner: second_recovery,
                transaction,
                observation: ReconciliationObservation::Indeterminate {
                    evidence: ReconciliationEvidenceRef::from_bytes([0x33; 32]),
                },
            },
            context(&awaiting, second_recovery.fence),
        )
        .expect("second recovery probe remains unknown")
        .record;

        let recovery = reconciliation_attempt(
            &indeterminate,
            second_recovery,
            transaction,
            context(&indeterminate, second_recovery.fence),
            CommandKind::Administer,
        )
        .expect("shared contract validates exact recovery chain");
        assert_eq!(recovery.attempt().execution_owner(), execution);
        assert_eq!(recovery.active_recovery_owner(), second_recovery);
        assert!(
            recovery
                .attempt()
                .validate_receipt_generation(execution.fence.generation)
                .is_ok()
        );
        assert_eq!(
            recovery
                .attempt()
                .validate_receipt_generation(first_recovery.fence.generation),
            Err(CommandPortError::CorruptRecord),
            "a plausible intermediate generation is not the transaction executor"
        );
    }

    #[test]
    fn equal_generation_different_actor_is_not_recovery_authority() {
        let execution = owner(1, 1, 1);
        let transaction = TransactionRef::from_bytes([0x41; 32]);
        let prepared = prepared(command(execution.fence), execution, transaction);
        let impostor = owner(2, 2, 1);
        assert_eq!(
            CommandReducer::reduce(
                &prepared,
                CommandEvent::ReportUnknownCommit {
                    owner: impostor,
                    transaction,
                    unknown: UnknownCommit {
                        reason: UnknownCommitReason::ProcessInterrupted,
                        diagnostic: crate::fabric::command::DiagnosticRef::from_bytes([0x42; 32]),
                    },
                },
                context(&prepared, impostor.fence),
            ),
            Err(crate::fabric::command::CommandContractError::RecoveryFenceNotAdvanced)
        );
    }

    fn command(writer_fence: WriterFence) -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes([0x10; 16]),
                idempotency_key: IdempotencyKey::from_bytes([0x11; 32]),
            },
            ownership: CommandOwnership {
                workspace_id: WorkspaceId::from_bytes([0x12; 16]),
                principal_id: PrincipalId::from_bytes([0x13; 16]),
                authorization: AuthorizationRef::from_bytes([0x14; 32]),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence,
            pins: CommandPins {
                compiler_release: CompilerReleaseRef::from_bytes([0x15; 32]),
                model_head: ModelHeadRef::from_bytes([0x16; 32]),
                source_generation: SourceGeneration::new(0),
                provider_set: ProviderSetRef::from_bytes([0x17; 32]),
            },
            resources: ResourceEnvelopeRef::from_bytes([0x18; 32]),
            payload: FabricCommandPayload::Administer {
                action: AdministrationAction::ReconcileOperation,
                request: AdministrationRequestRef::from_bytes([0x19; 32]),
            },
        }
    }

    fn prepared(
        command: FabricCommand,
        execution_owner: ExecutionOwner,
        transaction: TransactionRef,
    ) -> CommandRecord {
        let admitted = CommandReducer::admit(
            None,
            &command,
            AdmissionContext {
                workspace_id: command.ownership.workspace_id,
                current_head: command.expected_head,
                active_fence: command.writer_fence,
                authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
            },
        )
        .expect("admit command");
        let AdmissionOutcome::New(admitted) = admitted else {
            panic!("fresh command is new")
        };
        let executing = CommandReducer::reduce(
            &admitted,
            CommandEvent::Start {
                owner: execution_owner,
            },
            context(&admitted, execution_owner.fence),
        )
        .expect("start command")
        .record;
        CommandReducer::reduce(
            &executing,
            CommandEvent::PrepareCommit {
                owner: execution_owner,
                transaction,
            },
            context(&executing, execution_owner.fence),
        )
        .expect("prepare command")
        .record
    }

    fn context(record: &CommandRecord, active_fence: WriterFence) -> ReductionContext {
        ReductionContext {
            current_head: record.command().expected_head,
            active_fence,
        }
    }

    fn owner(actor: u8, lease: u8, generation: u64) -> ExecutionOwner {
        ExecutionOwner {
            actor_id: ActorId::from_bytes([actor; 16]),
            fence: WriterFence {
                lease_id: LeaseId::from_bytes([lease; 16]),
                generation: WriterGeneration::new(generation).expect("nonzero generation"),
            },
        }
    }
}
