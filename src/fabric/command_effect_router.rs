//! Exhaustive static dispatch from the durable command wire contract to typed effect families.
//!
//! Command variants are genuinely static lifecycle protocol: unlike ontology or query semantics,
//! adding one requires a coordinated code/wire migration. Distinct trait objects therefore make
//! every production effect family an explicit constructor dependency while the command payload
//! remains the single discriminant. No string registry, optional handler, default branch, or
//! caller-selected dispatch key can create a mutation bypass.

use std::sync::Arc;

use async_trait::async_trait;

use super::command::{
    CommandRecord, ExecutionOwner, FabricCommandPayload, ReconciliationObservation,
    ReductionContext, TransactionRef,
};
use super::command_actor::{
    CommandPortError, CommitEffectOutcome, FabricCommandEffectPort, PrepareEffectOutcome,
};

macro_rules! define_command_effect_family {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[async_trait]
        pub trait $name: Send + Sync {
            async fn prepare(
                &self,
                executing: &CommandRecord,
                owner: ExecutionOwner,
                context: ReductionContext,
            ) -> Result<PrepareEffectOutcome, CommandPortError>;

            async fn commit(
                &self,
                prepared: &CommandRecord,
                owner: ExecutionOwner,
                transaction: TransactionRef,
                context: ReductionContext,
            ) -> Result<CommitEffectOutcome, CommandPortError>;

            async fn reconcile(
                &self,
                awaiting: &CommandRecord,
                owner: ExecutionOwner,
                transaction: TransactionRef,
                context: ReductionContext,
            ) -> Result<ReconciliationObservation, CommandPortError>;
        }
    };
}

define_command_effect_family!(
    ModelMigrationCommandEffectPort,
    "Effects for the `ApplyModelMigration` lifecycle command."
);
define_command_effect_family!(
    SourceWaveCommandEffectPort,
    "Effects for the `PublishSourceWave` lifecycle command."
);
define_command_effect_family!(
    RelationPublicationCommandEffectPort,
    "Effects for the `PublishRelations` lifecycle command."
);
define_command_effect_family!(
    ActivationCommandEffectPort,
    "Effects for the `ActivateEpoch` lifecycle command."
);
define_command_effect_family!(
    RollbackCommandEffectPort,
    "Effects for the `RollbackEpoch` lifecycle command."
);
define_command_effect_family!(
    CompactionCommandEffectPort,
    "Effects for the `CompactRelations` lifecycle command."
);
define_command_effect_family!(
    RetentionCommandEffectPort,
    "Effects for the `ApplyRetention` lifecycle command."
);
define_command_effect_family!(
    AdministrationCommandEffectPort,
    "Effects for the closed administrative lifecycle command family."
);

/// Complete effect closure required to start the production command actor.
///
/// Construction cannot omit or dynamically replace a command family. Semantic choices inside a
/// family remain data/model-driven behind its typed port; this router owns only the static command
/// protocol distinction.
pub struct FabricCommandEffectRouter {
    model_migration: Arc<dyn ModelMigrationCommandEffectPort>,
    source_wave: Arc<dyn SourceWaveCommandEffectPort>,
    relation_publication: Arc<dyn RelationPublicationCommandEffectPort>,
    activation: Arc<dyn ActivationCommandEffectPort>,
    rollback: Arc<dyn RollbackCommandEffectPort>,
    compaction: Arc<dyn CompactionCommandEffectPort>,
    retention: Arc<dyn RetentionCommandEffectPort>,
    administration: Arc<dyn AdministrationCommandEffectPort>,
}

impl std::fmt::Debug for FabricCommandEffectRouter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FabricCommandEffectRouter")
            .field("model_migration", &"installed")
            .field("source_wave", &"installed")
            .field("relation_publication", &"installed")
            .field("activation", &"installed")
            .field("rollback", &"installed")
            .field("compaction", &"installed")
            .field("retention", &"installed")
            .field("administration", &"installed")
            .finish()
    }
}

impl FabricCommandEffectRouter {
    /// Install the complete closed command-effect family set.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model_migration: Arc<dyn ModelMigrationCommandEffectPort>,
        source_wave: Arc<dyn SourceWaveCommandEffectPort>,
        relation_publication: Arc<dyn RelationPublicationCommandEffectPort>,
        activation: Arc<dyn ActivationCommandEffectPort>,
        rollback: Arc<dyn RollbackCommandEffectPort>,
        compaction: Arc<dyn CompactionCommandEffectPort>,
        retention: Arc<dyn RetentionCommandEffectPort>,
        administration: Arc<dyn AdministrationCommandEffectPort>,
    ) -> Self {
        Self {
            model_migration,
            source_wave,
            relation_publication,
            activation,
            rollback,
            compaction,
            retention,
            administration,
        }
    }
}

#[async_trait]
impl FabricCommandEffectPort for FabricCommandEffectRouter {
    async fn prepare(
        &self,
        executing: &CommandRecord,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> Result<PrepareEffectOutcome, CommandPortError> {
        match executing.command().payload {
            FabricCommandPayload::ApplyModelMigration { .. } => {
                self.model_migration
                    .prepare(executing, owner, context)
                    .await
            }
            FabricCommandPayload::PublishSourceWave { .. } => {
                self.source_wave.prepare(executing, owner, context).await
            }
            FabricCommandPayload::PublishRelations { .. } => {
                self.relation_publication
                    .prepare(executing, owner, context)
                    .await
            }
            FabricCommandPayload::ActivateEpoch { .. } => {
                self.activation.prepare(executing, owner, context).await
            }
            FabricCommandPayload::RollbackEpoch { .. } => {
                self.rollback.prepare(executing, owner, context).await
            }
            FabricCommandPayload::CompactRelations { .. } => {
                self.compaction.prepare(executing, owner, context).await
            }
            FabricCommandPayload::ApplyRetention { .. } => {
                self.retention.prepare(executing, owner, context).await
            }
            FabricCommandPayload::Administer { .. } => {
                self.administration.prepare(executing, owner, context).await
            }
        }
    }

    async fn commit(
        &self,
        prepared: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<CommitEffectOutcome, CommandPortError> {
        match prepared.command().payload {
            FabricCommandPayload::ApplyModelMigration { .. } => {
                self.model_migration
                    .commit(prepared, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::PublishSourceWave { .. } => {
                self.source_wave
                    .commit(prepared, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::PublishRelations { .. } => {
                self.relation_publication
                    .commit(prepared, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::ActivateEpoch { .. } => {
                self.activation
                    .commit(prepared, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::RollbackEpoch { .. } => {
                self.rollback
                    .commit(prepared, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::CompactRelations { .. } => {
                self.compaction
                    .commit(prepared, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::ApplyRetention { .. } => {
                self.retention
                    .commit(prepared, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::Administer { .. } => {
                self.administration
                    .commit(prepared, owner, transaction, context)
                    .await
            }
        }
    }

    async fn reconcile(
        &self,
        awaiting: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<ReconciliationObservation, CommandPortError> {
        match awaiting.command().payload {
            FabricCommandPayload::ApplyModelMigration { .. } => {
                self.model_migration
                    .reconcile(awaiting, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::PublishSourceWave { .. } => {
                self.source_wave
                    .reconcile(awaiting, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::PublishRelations { .. } => {
                self.relation_publication
                    .reconcile(awaiting, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::ActivateEpoch { .. } => {
                self.activation
                    .reconcile(awaiting, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::RollbackEpoch { .. } => {
                self.rollback
                    .reconcile(awaiting, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::CompactRelations { .. } => {
                self.compaction
                    .reconcile(awaiting, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::ApplyRetention { .. } => {
                self.retention
                    .reconcile(awaiting, owner, transaction, context)
                    .await
            }
            FabricCommandPayload::Administer { .. } => {
                self.administration
                    .reconcile(awaiting, owner, transaction, context)
                    .await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::fabric::command::{
        ActorId, AdmissionContext, AnalysisRunRef, AuthorizationDecision, AuthorizationRef,
        CommandIdentity, CommandKind, CommandOwnership, CommandPins, CommandReducer,
        CompilerReleaseRef, DiagnosticRef, EpochId, ExpectedHead, IdempotencyKey, LeaseId,
        ModelHeadRef, ModelMigrationRef, OperationId, OwnerSetRef, PrincipalId, ProofReceiptRef,
        ProtectedSetRef, ProviderSetRef, ReconciliationEvidenceRef, RelationPublication,
        RelationSetRef, ResourceEnvelopeRef, RetentionPolicyRef, RollbackAuthorizationRef,
        SourceGeneration, SourceImageSetRef, TransactionRef, UnknownCommit, UnknownCommitReason,
        WorkspaceId, WriterFence, WriterGeneration,
    };

    struct Probe {
        expected: CommandKind,
        prepare_calls: AtomicUsize,
        commit_calls: AtomicUsize,
        reconcile_calls: AtomicUsize,
    }

    impl Probe {
        fn new(expected: CommandKind) -> Self {
            Self {
                expected,
                prepare_calls: AtomicUsize::new(0),
                commit_calls: AtomicUsize::new(0),
                reconcile_calls: AtomicUsize::new(0),
            }
        }

        fn observe_prepare(&self, record: &CommandRecord) -> PrepareEffectOutcome {
            assert_eq!(record.command().kind(), self.expected);
            self.prepare_calls.fetch_add(1, Ordering::SeqCst);
            PrepareEffectOutcome::Prepared {
                transaction: transaction(),
            }
        }

        fn observe_commit(&self, record: &CommandRecord) -> CommitEffectOutcome {
            assert_eq!(record.command().kind(), self.expected);
            self.commit_calls.fetch_add(1, Ordering::SeqCst);
            CommitEffectOutcome::Unknown {
                unknown: UnknownCommit {
                    reason: UnknownCommitReason::ReadbackUnavailable,
                    diagnostic: DiagnosticRef::from_bytes([0x71; 32]),
                },
            }
        }

        fn observe_reconcile(&self, record: &CommandRecord) -> ReconciliationObservation {
            assert_eq!(record.command().kind(), self.expected);
            self.reconcile_calls.fetch_add(1, Ordering::SeqCst);
            ReconciliationObservation::Indeterminate {
                evidence: ReconciliationEvidenceRef::from_bytes([0x72; 32]),
            }
        }

        fn assert_called_once(&self) {
            assert_eq!(self.prepare_calls.load(Ordering::SeqCst), 1);
            assert_eq!(self.commit_calls.load(Ordering::SeqCst), 1);
            assert_eq!(self.reconcile_calls.load(Ordering::SeqCst), 1);
        }
    }

    macro_rules! impl_effect_family_for_probe {
        ($trait_name:ident) => {
            #[async_trait]
            impl $trait_name for Probe {
                async fn prepare(
                    &self,
                    executing: &CommandRecord,
                    _owner: ExecutionOwner,
                    _context: ReductionContext,
                ) -> Result<PrepareEffectOutcome, CommandPortError> {
                    Ok(self.observe_prepare(executing))
                }

                async fn commit(
                    &self,
                    prepared: &CommandRecord,
                    _owner: ExecutionOwner,
                    _transaction: TransactionRef,
                    _context: ReductionContext,
                ) -> Result<CommitEffectOutcome, CommandPortError> {
                    Ok(self.observe_commit(prepared))
                }

                async fn reconcile(
                    &self,
                    awaiting: &CommandRecord,
                    _owner: ExecutionOwner,
                    _transaction: TransactionRef,
                    _context: ReductionContext,
                ) -> Result<ReconciliationObservation, CommandPortError> {
                    Ok(self.observe_reconcile(awaiting))
                }
            }
        };
    }

    impl_effect_family_for_probe!(ModelMigrationCommandEffectPort);
    impl_effect_family_for_probe!(SourceWaveCommandEffectPort);
    impl_effect_family_for_probe!(RelationPublicationCommandEffectPort);
    impl_effect_family_for_probe!(ActivationCommandEffectPort);
    impl_effect_family_for_probe!(RollbackCommandEffectPort);
    impl_effect_family_for_probe!(CompactionCommandEffectPort);
    impl_effect_family_for_probe!(RetentionCommandEffectPort);
    impl_effect_family_for_probe!(AdministrationCommandEffectPort);

    #[tokio::test]
    async fn every_static_command_variant_routes_to_exactly_one_required_family() {
        let model = Arc::new(Probe::new(CommandKind::ApplyModelMigration));
        let source = Arc::new(Probe::new(CommandKind::PublishSourceWave));
        let publication = Arc::new(Probe::new(CommandKind::PublishRelations));
        let activation = Arc::new(Probe::new(CommandKind::ActivateEpoch));
        let rollback = Arc::new(Probe::new(CommandKind::RollbackEpoch));
        let compaction = Arc::new(Probe::new(CommandKind::CompactRelations));
        let retention = Arc::new(Probe::new(CommandKind::ApplyRetention));
        let administration = Arc::new(Probe::new(CommandKind::Administer));
        let router = FabricCommandEffectRouter::new(
            model.clone(),
            source.clone(),
            publication.clone(),
            activation.clone(),
            rollback.clone(),
            compaction.clone(),
            retention.clone(),
            administration.clone(),
        );
        let owner = execution_owner();
        let context = ReductionContext {
            current_head: ExpectedHead::Empty,
            active_fence: owner.fence,
        };

        for (seed, payload) in payloads().into_iter().enumerate() {
            let record = admitted_record(seed as u8 + 1, payload);
            router
                .prepare(&record, owner, context)
                .await
                .expect("static prepare dispatch succeeds");
            router
                .commit(&record, owner, transaction(), context)
                .await
                .expect("static commit dispatch succeeds");
            router
                .reconcile(&record, owner, transaction(), context)
                .await
                .expect("static reconciliation dispatch succeeds");
        }

        for probe in [
            model,
            source,
            publication,
            activation,
            rollback,
            compaction,
            retention,
            administration,
        ] {
            probe.assert_called_once();
        }
    }

    fn payloads() -> [FabricCommandPayload; 8] {
        [
            FabricCommandPayload::ApplyModelMigration {
                migration: ModelMigrationRef::from_bytes([0x01; 32]),
                target_model_head: ModelHeadRef::from_bytes([0x02; 32]),
            },
            FabricCommandPayload::PublishSourceWave {
                source_images: SourceImageSetRef::from_bytes([0x03; 32]),
                target_generation: SourceGeneration::new(1),
            },
            FabricCommandPayload::PublishRelations {
                publication: RelationPublication::Derived {
                    analysis_run: AnalysisRunRef::from_bytes([0x04; 32]),
                    owners: OwnerSetRef::from_bytes([0x05; 32]),
                    relations: RelationSetRef::from_bytes([0x06; 32]),
                },
            },
            FabricCommandPayload::ActivateEpoch {
                candidate_epoch: EpochId::from_bytes([0x07; 16]),
                proof_receipt: ProofReceiptRef::from_bytes([0x08; 32]),
            },
            FabricCommandPayload::RollbackEpoch {
                target_epoch: EpochId::from_bytes([0x09; 16]),
                authorization: RollbackAuthorizationRef::from_bytes([0x0a; 32]),
            },
            FabricCommandPayload::CompactRelations {
                relations: RelationSetRef::from_bytes([0x0b; 32]),
                equivalence_proof: ProofReceiptRef::from_bytes([0x0c; 32]),
            },
            FabricCommandPayload::ApplyRetention {
                policy: RetentionPolicyRef::from_bytes([0x0d; 32]),
                protected: ProtectedSetRef::from_bytes([0x0e; 32]),
            },
            FabricCommandPayload::Administer {
                action: crate::fabric::command::AdministrationAction::ReconcileOperation,
                request: crate::fabric::command::AdministrationRequestRef::from_bytes([0x0f; 32]),
            },
        ]
    }

    fn admitted_record(seed: u8, payload: FabricCommandPayload) -> CommandRecord {
        let command = crate::fabric::command::FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes([seed; 16]),
                idempotency_key: IdempotencyKey::from_bytes([seed; 32]),
            },
            ownership: CommandOwnership {
                workspace_id: WorkspaceId::from_bytes([0x21; 16]),
                principal_id: PrincipalId::from_bytes([0x22; 16]),
                authorization: AuthorizationRef::from_bytes([0x23; 32]),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence: execution_owner().fence,
            pins: CommandPins {
                compiler_release: CompilerReleaseRef::from_bytes([0x24; 32]),
                model_head: ModelHeadRef::from_bytes([0x25; 32]),
                source_generation: SourceGeneration::new(0),
                provider_set: ProviderSetRef::from_bytes([0x26; 32]),
            },
            resources: ResourceEnvelopeRef::from_bytes([0x27; 32]),
            payload,
        };
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
        .expect("test command is admitted")
        .record()
    }

    fn execution_owner() -> ExecutionOwner {
        ExecutionOwner {
            actor_id: ActorId::from_bytes([0x31; 16]),
            fence: WriterFence {
                lease_id: LeaseId::from_bytes([0x32; 16]),
                generation: WriterGeneration::new(1).expect("test generation is nonzero"),
            },
        }
    }

    fn transaction() -> TransactionRef {
        TransactionRef::from_bytes([0x41; 32])
    }
}
