//! Production adapters for fresh command semantics and interruption diagnostics.
//!
//! These adapters deliberately do not own semantic state. Each command admission and recovery
//! read goes through an activation-chain authority and a policy authority supplied by the daemon.
//! SQLite command records remain temporal coordination only, while opaque diagnostic identities
//! must already exist in an application-owned diagnostic relation.

use std::sync::Arc;

use async_trait::async_trait;

use super::activation::ActivationChain;
use super::command::{
    AuthorizationDecision, CommandRecord, DiagnosticRef, DurableCommandState, ExpectedHead,
    FabricCommand, OperationId, TransactionRef, WorkspaceId, WriterFence,
};
use super::command_actor::CommandPortError;
use super::command_runtime::{
    CommandSemanticContextPort, InterruptedCommitDiagnosticPort, SemanticAdmissionContext,
};

/// Fresh durable activation-chain reader used by command admission and recovery.
#[async_trait]
pub trait CommandActivationChainPort: Send + Sync {
    /// Read and validate the complete activation chain for one workspace.
    async fn read_chain(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<ActivationChain, CommandPortError>;
}

/// Policy-relation evaluator for one command against the freshly read semantic head.
#[async_trait]
pub trait CommandAuthorizationPort: Send + Sync {
    /// Return an explicit authorized, denied, or unknown decision.
    async fn authorize(
        &self,
        command: &FabricCommand,
        current_head: ExpectedHead,
    ) -> Result<AuthorizationDecision, CommandPortError>;
}

/// Production semantic-context adapter over durable activation and policy relations.
pub struct RelationalCommandSemanticContext {
    workspace_id: WorkspaceId,
    activations: Arc<dyn CommandActivationChainPort>,
    authorization: Arc<dyn CommandAuthorizationPort>,
}

impl std::fmt::Debug for RelationalCommandSemanticContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RelationalCommandSemanticContext")
            .field("workspace_id", &self.workspace_id)
            .field("activations", &"installed")
            .field("authorization", &"installed")
            .finish()
    }
}

impl RelationalCommandSemanticContext {
    /// Bind one workspace to its durable activation and policy readers.
    #[must_use]
    pub fn new(
        workspace_id: WorkspaceId,
        activations: Arc<dyn CommandActivationChainPort>,
        authorization: Arc<dyn CommandAuthorizationPort>,
    ) -> Self {
        Self {
            workspace_id,
            activations,
            authorization,
        }
    }

    async fn read_head(&self) -> Result<ExpectedHead, CommandPortError> {
        let chain = self.activations.read_chain(self.workspace_id).await?;
        if chain.workspace_id() != self.workspace_id {
            return Err(CommandPortError::CorruptRecord);
        }
        Ok(chain.current_head())
    }
}

#[async_trait]
impl CommandSemanticContextPort for RelationalCommandSemanticContext {
    async fn read_admission_semantics(
        &self,
        command: &FabricCommand,
    ) -> Result<SemanticAdmissionContext, CommandPortError> {
        if command.ownership.workspace_id != self.workspace_id {
            return Err(CommandPortError::ContextUnavailable);
        }
        let current_head = self.read_head().await?;
        let authorization = self.authorization.authorize(command, current_head).await?;
        Ok(SemanticAdmissionContext {
            workspace_id: self.workspace_id,
            current_head,
            authorization,
        })
    }

    async fn read_current_head(
        &self,
        record: &CommandRecord,
    ) -> Result<ExpectedHead, CommandPortError> {
        if record.command().ownership.workspace_id != self.workspace_id {
            return Err(CommandPortError::ContextUnavailable);
        }
        self.read_head().await
    }
}

/// Exact lookup key for one already-persisted interruption diagnostic relation row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptedCommitDiagnosticQuery {
    pub workspace_id: WorkspaceId,
    pub operation_id: OperationId,
    pub transaction: TransactionRef,
    pub execution_fence: WriterFence,
    pub active_recovery_fence: WriterFence,
}

/// Read-only diagnostic-relation authority.
#[async_trait]
pub trait InterruptedCommitDiagnosticRelationPort: Send + Sync {
    /// Return the exact diagnostic identity if durable evidence exists.
    async fn read_interruption_diagnostic(
        &self,
        query: InterruptedCommitDiagnosticQuery,
    ) -> Result<Option<DiagnosticRef>, CommandPortError>;
}

/// Production interruption adapter which refuses to manufacture a diagnostic identity.
pub struct RelationalInterruptedCommitDiagnostics {
    workspace_id: WorkspaceId,
    diagnostics: Arc<dyn InterruptedCommitDiagnosticRelationPort>,
}

impl std::fmt::Debug for RelationalInterruptedCommitDiagnostics {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RelationalInterruptedCommitDiagnostics")
            .field("workspace_id", &self.workspace_id)
            .field("diagnostics", &"installed")
            .finish()
    }
}

impl RelationalInterruptedCommitDiagnostics {
    #[must_use]
    pub fn new(
        workspace_id: WorkspaceId,
        diagnostics: Arc<dyn InterruptedCommitDiagnosticRelationPort>,
    ) -> Self {
        Self {
            workspace_id,
            diagnostics,
        }
    }
}

#[async_trait]
impl InterruptedCommitDiagnosticPort for RelationalInterruptedCommitDiagnostics {
    async fn interruption_diagnostic(
        &self,
        prepared: &CommandRecord,
        transaction: TransactionRef,
        active_fence: WriterFence,
    ) -> Result<DiagnosticRef, CommandPortError> {
        let command = prepared.command();
        if command.ownership.workspace_id != self.workspace_id {
            return Err(CommandPortError::ContextUnavailable);
        }
        let DurableCommandState::CommitPrepared {
            owner,
            transaction: prepared_transaction,
            ..
        } = prepared.state()
        else {
            return Err(CommandPortError::CorruptRecord);
        };
        if prepared_transaction != transaction {
            return Err(CommandPortError::CorruptRecord);
        }
        self.diagnostics
            .read_interruption_diagnostic(InterruptedCommitDiagnosticQuery {
                workspace_id: self.workspace_id,
                operation_id: command.identity.operation_id,
                transaction,
                execution_fence: owner.fence,
                active_recovery_fence: active_fence,
            })
            .await?
            .ok_or(CommandPortError::ContextUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::fabric::command::{
        ActorId, AdmissionContext, AuthorizationRef, CommandEvent, CommandIdentity,
        CommandOwnership, CommandPins, CommandReducer, EpochId, ExecutionOwner,
        FabricCommandPayload, IdempotencyKey, InputReleaseRef, LeaseId, PrincipalId,
        ProgramReleaseRef, ProofReceiptRef, ProviderSetRef, ReductionContext, ResourceEnvelopeRef,
        SourceGeneration, WriterGeneration,
    };

    struct ChainReader {
        workspace_id: WorkspaceId,
    }

    #[async_trait]
    impl CommandActivationChainPort for ChainReader {
        async fn read_chain(
            &self,
            workspace_id: WorkspaceId,
        ) -> Result<ActivationChain, CommandPortError> {
            assert_eq!(workspace_id, self.workspace_id);
            ActivationChain::derive(workspace_id, []).map_err(|_| CommandPortError::CorruptRecord)
        }
    }

    struct AuthorizationReader;

    #[async_trait]
    impl CommandAuthorizationPort for AuthorizationReader {
        async fn authorize(
            &self,
            command: &FabricCommand,
            current_head: ExpectedHead,
        ) -> Result<AuthorizationDecision, CommandPortError> {
            assert_eq!(current_head, ExpectedHead::Empty);
            Ok(AuthorizationDecision::Authorized(
                command.ownership.authorization,
            ))
        }
    }

    struct DiagnosticReader {
        observed: Mutex<Option<InterruptedCommitDiagnosticQuery>>,
        diagnostic: Option<DiagnosticRef>,
    }

    #[async_trait]
    impl InterruptedCommitDiagnosticRelationPort for DiagnosticReader {
        async fn read_interruption_diagnostic(
            &self,
            query: InterruptedCommitDiagnosticQuery,
        ) -> Result<Option<DiagnosticRef>, CommandPortError> {
            *self.observed.lock().unwrap() = Some(query);
            Ok(self.diagnostic)
        }
    }

    fn workspace(seed: u8) -> WorkspaceId {
        WorkspaceId::from_bytes([seed; 16])
    }

    fn fence(seed: u8, generation: u64) -> WriterFence {
        WriterFence {
            lease_id: LeaseId::from_bytes([seed; 16]),
            generation: WriterGeneration::new(generation).unwrap(),
        }
    }

    fn command(workspace_id: WorkspaceId) -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes([0x20; 16]),
                idempotency_key: IdempotencyKey::from_bytes([0x21; 32]),
            },
            ownership: CommandOwnership {
                workspace_id,
                principal_id: PrincipalId::from_bytes([0x22; 16]),
                authorization: AuthorizationRef::from_bytes([0x23; 32]),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence: fence(0x24, 1),
            pins: CommandPins {
                input_release: InputReleaseRef::from_bytes([0x25; 32]),
                program_release: ProgramReleaseRef::from_bytes([0x26; 32]),
                application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(
                    [0x26; 32],
                ),
                source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(
                    [0x26; 32],
                ),
                provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(
                    [0x26; 32],
                ),
                source_generation: SourceGeneration::new(0),
                provider_set: ProviderSetRef::from_bytes([0x27; 32]),
            },
            resources: ResourceEnvelopeRef::from_bytes([0x28; 32]),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: EpochId::from_bytes([0x29; 16]),
                proof_receipt: ProofReceiptRef::from_bytes([0x2a; 32]),
            },
        }
    }

    fn admitted(command: FabricCommand) -> CommandRecord {
        CommandReducer::admit(
            None,
            &command,
            AdmissionContext {
                workspace_id: command.ownership.workspace_id,
                current_head: command.expected_head,
                active_fence: command.writer_fence,
                authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
            },
        )
        .unwrap()
        .record()
    }

    #[tokio::test]
    async fn semantic_context_reads_fresh_activation_and_policy_authorities() {
        let workspace_id = workspace(0x10);
        let context = RelationalCommandSemanticContext::new(
            workspace_id,
            Arc::new(ChainReader { workspace_id }),
            Arc::new(AuthorizationReader),
        );
        let command = command(workspace_id);

        assert_eq!(
            context.read_admission_semantics(&command).await.unwrap(),
            SemanticAdmissionContext {
                workspace_id,
                current_head: ExpectedHead::Empty,
                authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
            }
        );
        let record = admitted(command);
        assert_eq!(
            context.read_current_head(&record).await.unwrap(),
            ExpectedHead::Empty
        );
        assert!(matches!(
            RelationalCommandSemanticContext::new(
                workspace(0x11),
                Arc::new(ChainReader {
                    workspace_id: workspace(0x11)
                }),
                Arc::new(AuthorizationReader),
            )
            .read_admission_semantics(&command)
            .await,
            Err(CommandPortError::ContextUnavailable)
        ));
    }

    #[tokio::test]
    async fn interruption_diagnostic_requires_exact_durable_relation_evidence() {
        let workspace_id = workspace(0x30);
        let command = command(workspace_id);
        let execution_owner = ExecutionOwner {
            actor_id: ActorId::from_bytes([0x31; 16]),
            fence: command.writer_fence,
        };
        let transaction = TransactionRef::from_bytes([0x33; 32]);
        let admitted = admitted(command);
        let reduction_context = ReductionContext {
            current_head: ExpectedHead::Empty,
            active_fence: execution_owner.fence,
        };
        let executing = CommandReducer::reduce(
            &admitted,
            CommandEvent::Start {
                owner: execution_owner,
            },
            reduction_context,
        )
        .unwrap()
        .record;
        let prepared = CommandReducer::reduce(
            &executing,
            CommandEvent::PrepareCommit {
                owner: execution_owner,
                transaction,
            },
            reduction_context,
        )
        .unwrap()
        .record;
        let expected = DiagnosticRef::from_bytes([0x34; 32]);
        let reader = Arc::new(DiagnosticReader {
            observed: Mutex::new(None),
            diagnostic: Some(expected),
        });
        let diagnostics = RelationalInterruptedCommitDiagnostics::new(
            workspace_id,
            Arc::clone(&reader) as Arc<dyn InterruptedCommitDiagnosticRelationPort>,
        );
        let active_fence = fence(0x35, 5);

        assert_eq!(
            diagnostics
                .interruption_diagnostic(&prepared, transaction, active_fence)
                .await
                .unwrap(),
            expected
        );
        assert_eq!(
            *reader.observed.lock().unwrap(),
            Some(InterruptedCommitDiagnosticQuery {
                workspace_id,
                operation_id: command.identity.operation_id,
                transaction,
                execution_fence: execution_owner.fence,
                active_recovery_fence: active_fence,
            })
        );

        let absent = RelationalInterruptedCommitDiagnostics::new(
            workspace_id,
            Arc::new(DiagnosticReader {
                observed: Mutex::new(None),
                diagnostic: None,
            }),
        );
        assert!(matches!(
            absent
                .interruption_diagnostic(&prepared, transaction, active_fence)
                .await,
            Err(CommandPortError::ContextUnavailable)
        ));
    }
}
