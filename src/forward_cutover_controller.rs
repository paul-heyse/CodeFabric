//! Production deployment observation and durable forward-cutover admission.
//!
//! This adapter intentionally sits above both the daemon and the fabric. It translates actual
//! package/configuration layout, process identity, private Unix sockets, the OS-lease-backed
//! command handle, and the Delta activation readback into the typed cutover evidence consumed by
//! the append-only journal. Artifact digests below are identities only; semantic correctness is
//! supplied by the programmatic activation/proof authorities.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _};
use std::path::{Component, Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use thiserror::Error;

use crate::daemon::{DaemonConfig, DaemonDiscovery};
use crate::fabric::activation_control_delta::DeltaActivationRuntimeAuthority;
use crate::fabric::command::{
    AdministrationAction, CommandKind, CommandRecord, CommandResult, ExecutionOwner, ExpectedHead,
    FabricCommand, FabricCommandPayload, OperationSelectionRef, ReconciliationEvidenceRef,
    ReconciliationObservation, ReductionContext, TransactionRef, WorkspaceId, WriterFence,
};
use crate::fabric::command_actor::{CommandPortError, CommitEffectOutcome, PrepareEffectOutcome};
use crate::fabric::command_effect_contract::{
    ValidatedCommandAttempt, executing_attempt, prepared_attempt, reconciliation_attempt,
};
use crate::fabric::command_effect_router::AdministrationCommandEffectPort;
use crate::fabric::forward_cutover::{
    ActualSupervisorReadback, AuthorityOwner, CutoverActivationAuthority, CutoverAdmission,
    CutoverAppendOutcome, CutoverChainIdentity, CutoverCommandBinding, CutoverEvent,
    CutoverEventId, CutoverOperatorStatus, CutoverPlanId, CutoverTransitionEvidence,
    DaemonReleaseId, DurableCutoverState, DurableForwardCutoverJournal, HostBootId, HostIdentity,
    ObservedAvailability, PredecessorRevocationReadback, SupervisorConfigId, SupervisorFactId,
    SupervisorObservation, UdsEndpointId,
};
use crate::fabric::programmatic_workspace::{
    ProgrammaticCommandRuntimeContext, ProgrammaticDaemonComposition,
};

pub const FORWARD_CUTOVER_DEPLOYMENT_FILE: &str = "forward-cutover-v1.json";
pub const FORWARD_CUTOVER_JOURNAL_FILE: &str = "forward-cutover-v1.sqlite3";
const DEPLOYMENT_SCHEMA: &str = "codefabric.forward-cutover.deployment.v1";
const MAX_DEPLOYMENT_BYTES: u64 = 262_144;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct ForwardCutoverDeploymentManifest {
    schema: String,
    package_root: PathBuf,
    target_entrypoint: PathBuf,
    daemon_config_file: PathBuf,
    workspaces: Vec<ForwardCutoverWorkspaceManifest>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct ForwardCutoverWorkspaceManifest {
    plan_id: [u8; 32],
    workspace_id: [u8; 16],
    predecessor_release: [u8; 32],
}

#[derive(Clone, Debug)]
struct VerifiedDeployment {
    manifest: ForwardCutoverDeploymentManifest,
    target_release: DaemonReleaseId,
    supervisor_config: SupervisorConfigId,
    deployment_host: HostIdentity,
    current_boot: HostBootId,
    physical_zero_fact: SupervisorFactId,
}

#[derive(Clone, Copy, Debug)]
struct ProgrammaticAuthorityObservation {
    workspace_id: WorkspaceId,
    activation: CutoverActivationAuthority,
    writer_fence: WriterFence,
}

/// Exact live evidence paired with the reconstructed chain identity.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionCutoverEvidence {
    pub identity: CutoverChainIdentity,
    pub supervisor: SupervisorObservation,
    pub activation: CutoverActivationAuthority,
    pub current_writer_fence: WriterFence,
}

/// One durable operator status for one configured workspace.
#[derive(Clone, Debug)]
pub struct ProductionCutoverStatus {
    pub workspace_id: WorkspaceId,
    pub status: CutoverOperatorStatus,
}

/// Exact command intent observed from the deployment and live runtime authorities.
///
/// Bootstrap owns policy identity, semantic pins, resources, and command submission. This value
/// supplies only the cutover-owned fields that bootstrap must not duplicate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCutoverCommandIntent {
    pub workspace_id: WorkspaceId,
    pub plan_id: CutoverPlanId,
    pub expected_epoch: crate::fabric::command::EpochId,
    pub current_writer_fence: WriterFence,
}

/// Idempotent result of the production command-bound target convergence transaction.
#[derive(Clone, Debug)]
pub enum ProductionCutoverAdvanceOutcome {
    Advanced {
        append: CutoverAppendOutcome,
        status: ProductionCutoverStatus,
    },
    AlreadyConverged(ProductionCutoverStatus),
}

/// External-controller adapter activated only by the closed deployment manifest.
#[derive(Clone, Debug)]
pub struct ProductionForwardCutoverController {
    manifest_path: PathBuf,
    journal_path: PathBuf,
}

/// Immutable controller/configuration binding installed into each live command router.
///
/// A release without the closed cutover deployment manifest retains its existing administration
/// effect unchanged.
#[derive(Clone, Debug)]
pub struct ProductionForwardCutoverBinding {
    controller: ProductionForwardCutoverController,
    config: DaemonConfig,
}

impl ProductionForwardCutoverBinding {
    #[must_use]
    pub const fn new(controller: ProductionForwardCutoverController, config: DaemonConfig) -> Self {
        Self { controller, config }
    }

    pub(crate) fn wrap_administration(
        &self,
        context: &ProgrammaticCommandRuntimeContext,
        delegate: Arc<dyn AdministrationCommandEffectPort>,
    ) -> Arc<dyn AdministrationCommandEffectPort> {
        Arc::new(ForwardCutoverAdministrationEffect {
            binding: self.clone(),
            workspace_id: context.workspace_id(),
            activation_authority: Arc::clone(context.activation_authority()),
            delegate,
        })
    }
}

struct ForwardCutoverAdministrationEffect {
    binding: ProductionForwardCutoverBinding,
    workspace_id: WorkspaceId,
    activation_authority: Arc<DeltaActivationRuntimeAuthority>,
    delegate: Arc<dyn AdministrationCommandEffectPort>,
}

impl ProductionForwardCutoverController {
    /// Discover and validate the production cutover deployment, if explicitly configured.
    pub fn open_if_configured(
        config: &DaemonConfig,
    ) -> Result<Option<Self>, ProductionForwardCutoverError> {
        let manifest_path = config
            .static_config
            .config_root
            .join(FORWARD_CUTOVER_DEPLOYMENT_FILE);
        match fs::symlink_metadata(&manifest_path) {
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(ProductionForwardCutoverError::Io {
                    path: manifest_path,
                    source,
                });
            }
        }
        let controller = Self {
            manifest_path,
            journal_path: config
                .static_config
                .state_root
                .join(FORWARD_CUTOVER_JOURNAL_FILE),
        };
        controller.verify_deployment(config)?;
        DurableForwardCutoverJournal::open(&controller.journal_path)
            .map_err(|error| ProductionForwardCutoverError::Journal(error.to_string()))?;
        Ok(Some(controller))
    }

    /// Reobserve the deployment and programmatic authorities, reopen the journal, and derive
    /// current operator status. Nothing is accepted from an in-memory phase projection.
    pub async fn operator_statuses(
        &self,
        config: &DaemonConfig,
        composition: &ProgrammaticDaemonComposition,
    ) -> Result<Vec<ProductionCutoverStatus>, ProductionForwardCutoverError> {
        let authorities = observe_programmatic_authorities(composition).await?;
        self.operator_statuses_from_authorities(config, &authorities)
    }

    /// Read the exact cutover-owned command fields from the physical deployment and live
    /// programmatic composition.
    pub async fn command_intents(
        &self,
        config: &DaemonConfig,
        composition: &ProgrammaticDaemonComposition,
    ) -> Result<Vec<ProductionCutoverCommandIntent>, ProductionForwardCutoverError> {
        let authorities = observe_programmatic_authorities(composition).await?;
        self.command_intents_from_authorities(config, &authorities)
    }

    /// Require every configured workspace to be durably and physically observed as the target
    /// read/write authority before production ingress remains open.
    pub async fn require_target_read_write(
        &self,
        config: &DaemonConfig,
        composition: &ProgrammaticDaemonComposition,
    ) -> Result<Vec<ProductionCutoverStatus>, ProductionForwardCutoverError> {
        let statuses = self.operator_statuses(config, composition).await?;
        if let Some(closed) = statuses
            .iter()
            .find(|status| status.status.admission != CutoverAdmission::TargetReadWrite)
        {
            return Err(ProductionForwardCutoverError::AdmissionClosed {
                workspace_id: *closed.workspace_id.as_bytes(),
                code: closed.status.code,
            });
        }
        Ok(statuses)
    }

    /// Append or idempotently converge the target-only physical-zero transition at the exact
    /// durable `CommitPrepared` boundary of the sole programmatic command runtime.
    ///
    /// The command must be `Administer::ReconcileOperation`, name the cutover plan as its typed
    /// request identity, expect the activation-selected epoch, and carry the live OS-backed writer
    /// fence. The controller independently reobserves package/config/process/UDS authority and the
    /// Delta activation before constructing the event. It never manufactures a predecessor phase.
    #[allow(clippy::too_many_arguments)]
    pub async fn advance_from_prepared_command(
        &self,
        config: &DaemonConfig,
        composition: &ProgrammaticDaemonComposition,
        record: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<ProductionCutoverAdvanceOutcome, ProductionForwardCutoverError> {
        let workspace_id = record.command().ownership.workspace_id;
        let evidence = self
            .current_evidence_from_authorities(
                config,
                &observe_programmatic_authorities(composition).await?,
            )?
            .into_iter()
            .find(|candidate| candidate.identity.workspace_id() == workspace_id)
            .ok_or_else(|| {
                ProductionForwardCutoverError::Authority(
                    "prepared command workspace is not in the cutover deployment".to_owned(),
                )
            })?;
        self.advance_with_evidence(record, owner, transaction, context, &evidence)
    }

    async fn advance_from_runtime_authority(
        &self,
        config: &DaemonConfig,
        activation_authority: &DeltaActivationRuntimeAuthority,
        record: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<ProductionCutoverAdvanceOutcome, ProductionForwardCutoverError> {
        let snapshot = activation_authority
            .current_snapshot()
            .await
            .map_err(|error| ProductionForwardCutoverError::Authority(error.to_string()))?;
        let activation = CutoverActivationAuthority::try_from_chain(&snapshot.chain)
            .map_err(|error| ProductionForwardCutoverError::Authority(error.to_string()))?;
        if snapshot.active_fence != owner.fence {
            return Err(ProductionForwardCutoverError::Authority(
                "current durable writer authority differs from the prepared command owner"
                    .to_owned(),
            ));
        }
        require_strict_writer_successor(activation.writer_fence(), snapshot.active_fence)?;
        let evidence = self.current_evidence_for_authority(
            config,
            ProgrammaticAuthorityObservation {
                workspace_id: record.command().ownership.workspace_id,
                activation,
                writer_fence: snapshot.active_fence,
            },
        )?;
        self.advance_with_evidence(record, owner, transaction, context, &evidence)
    }

    fn advance_with_evidence(
        &self,
        record: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
        evidence: &ProductionCutoverEvidence,
    ) -> Result<ProductionCutoverAdvanceOutcome, ProductionForwardCutoverError> {
        let workspace_id = record.command().ownership.workspace_id;
        let command = record.command();
        let FabricCommandPayload::Administer { action, request } = command.payload else {
            return Err(ProductionForwardCutoverError::Command(
                "cutover convergence requires an administrative command".to_owned(),
            ));
        };
        if action != AdministrationAction::ReconcileOperation
            || request.as_bytes() != evidence.identity.plan_id().as_bytes()
            || command.expected_head != ExpectedHead::Epoch(evidence.activation.selected_epoch())
            || command.writer_fence != evidence.current_writer_fence
            || owner.fence != evidence.current_writer_fence
            || context.active_fence != evidence.current_writer_fence
        {
            return Err(ProductionForwardCutoverError::Command(
                "prepared command does not bind the exact plan, activation head, and live writer fence"
                    .to_owned(),
            ));
        }
        let journal = DurableForwardCutoverJournal::open(&self.journal_path)
            .map_err(|error| ProductionForwardCutoverError::Journal(error.to_string()))?;
        let chain = journal
            .events(workspace_id)
            .map_err(|error| ProductionForwardCutoverError::Journal(error.to_string()))?;
        let validated =
            prepared_attempt(record, owner, transaction, context, CommandKind::Administer)
                .map_err(|error| ProductionForwardCutoverError::Command(error.to_string()))?;
        let current_binding = CutoverCommandBinding::try_from_validated_attempt(validated)
            .map_err(|error| ProductionForwardCutoverError::Command(error.to_string()))?;
        if let Some(complete) = chain
            .last()
            .filter(|event| event.phase() == crate::fabric::forward_cutover::CutoverPhase::Complete)
        {
            if complete.command() != current_binding {
                return Err(ProductionForwardCutoverError::Command(
                    "durable completion belongs to another exact command transaction".to_owned(),
                ));
            }
            let status = journal
                .operator_status(
                    evidence.identity,
                    Some(evidence.supervisor),
                    Some(evidence.activation),
                )
                .map_err(|error| ProductionForwardCutoverError::Journal(error.to_string()))?;
            if status.admission != CutoverAdmission::TargetReadWrite {
                return Err(ProductionForwardCutoverError::AdmissionClosed {
                    workspace_id: *workspace_id.as_bytes(),
                    code: status.code,
                });
            }
            return Ok(ProductionCutoverAdvanceOutcome::AlreadyConverged(
                ProductionCutoverStatus {
                    workspace_id,
                    status,
                },
            ));
        }
        let event = CutoverEvent::try_next_from_prepared_command(
            chain.last(),
            evidence.identity,
            record,
            owner,
            transaction,
            context,
            CutoverTransitionEvidence::PhysicalZeroConverged {
                activation: evidence.activation,
                supervisor: evidence.supervisor,
            },
        )
        .map_err(|error| ProductionForwardCutoverError::Command(error.to_string()))?;
        let append = journal
            .append(&event)
            .map_err(|error| ProductionForwardCutoverError::Journal(error.to_string()))?;
        let status = journal
            .operator_status(
                evidence.identity,
                Some(evidence.supervisor),
                Some(evidence.activation),
            )
            .map_err(|error| ProductionForwardCutoverError::Journal(error.to_string()))?;
        if status.admission != CutoverAdmission::TargetReadWrite
            || !matches!(
                status.durable_state,
                DurableCutoverState::At {
                    phase: crate::fabric::forward_cutover::CutoverPhase::Complete,
                    ..
                }
            )
        {
            return Err(ProductionForwardCutoverError::AdmissionClosed {
                workspace_id: *workspace_id.as_bytes(),
                code: status.code,
            });
        }
        Ok(ProductionCutoverAdvanceOutcome::Advanced {
            append,
            status: ProductionCutoverStatus {
                workspace_id,
                status,
            },
        })
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn advance_from_prepared_command_for_test(
        &self,
        config: &DaemonConfig,
        workspace_id: WorkspaceId,
        activation: CutoverActivationAuthority,
        writer_fence: WriterFence,
        record: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<ProductionCutoverAdvanceOutcome, ProductionForwardCutoverError> {
        let evidence =
            self.current_evidence_for_test(config, workspace_id, activation, writer_fence)?;
        self.advance_with_evidence(record, owner, transaction, context, &evidence)
    }

    fn operator_statuses_from_authorities(
        &self,
        config: &DaemonConfig,
        authorities: &[ProgrammaticAuthorityObservation],
    ) -> Result<Vec<ProductionCutoverStatus>, ProductionForwardCutoverError> {
        let evidence = self.current_evidence_from_authorities(config, authorities)?;
        let journal = DurableForwardCutoverJournal::open(&self.journal_path)
            .map_err(|error| ProductionForwardCutoverError::Journal(error.to_string()))?;
        evidence
            .into_iter()
            .map(|evidence| {
                let status = journal
                    .operator_status(
                        evidence.identity,
                        Some(evidence.supervisor),
                        Some(evidence.activation),
                    )
                    .map_err(|error| ProductionForwardCutoverError::Journal(error.to_string()))?;
                Ok(ProductionCutoverStatus {
                    workspace_id: evidence.identity.workspace_id(),
                    status,
                })
            })
            .collect()
    }

    fn command_intents_from_authorities(
        &self,
        config: &DaemonConfig,
        authorities: &[ProgrammaticAuthorityObservation],
    ) -> Result<Vec<ProductionCutoverCommandIntent>, ProductionForwardCutoverError> {
        self.current_evidence_from_authorities(config, authorities)?
            .into_iter()
            .map(|evidence| {
                Ok(ProductionCutoverCommandIntent {
                    workspace_id: evidence.identity.workspace_id(),
                    plan_id: evidence.identity.plan_id(),
                    expected_epoch: evidence.activation.selected_epoch(),
                    current_writer_fence: evidence.current_writer_fence,
                })
            })
            .collect()
    }

    fn current_evidence_from_authorities(
        &self,
        config: &DaemonConfig,
        authorities: &[ProgrammaticAuthorityObservation],
    ) -> Result<Vec<ProductionCutoverEvidence>, ProductionForwardCutoverError> {
        let deployment = self.verify_deployment(config)?;
        let discovery = verify_live_process_and_sockets(config)?;
        if authorities.len() != deployment.manifest.workspaces.len() {
            return Err(ProductionForwardCutoverError::Authority(
                "configured and programmatic workspace censuses differ".to_owned(),
            ));
        }
        deployment
            .manifest
            .workspaces
            .iter()
            .map(|workspace| {
                let workspace_id = WorkspaceId::from_bytes(workspace.workspace_id);
                let authority = authorities
                    .iter()
                    .find(|candidate| candidate.workspace_id == workspace_id)
                    .copied()
                    .ok_or_else(|| {
                        ProductionForwardCutoverError::Authority(format!(
                            "configured workspace {:02x?} has no programmatic authority",
                            workspace.workspace_id
                        ))
                    })?;
                build_production_evidence(config, &deployment, &discovery, workspace, authority)
            })
            .collect()
    }

    fn current_evidence_for_authority(
        &self,
        config: &DaemonConfig,
        authority: ProgrammaticAuthorityObservation,
    ) -> Result<ProductionCutoverEvidence, ProductionForwardCutoverError> {
        let deployment = self.verify_deployment(config)?;
        let discovery = verify_live_process_and_sockets(config)?;
        let workspace = deployment
            .manifest
            .workspaces
            .iter()
            .find(|workspace| workspace.workspace_id == *authority.workspace_id.as_bytes())
            .ok_or_else(|| {
                ProductionForwardCutoverError::Authority(
                    "runtime workspace is not in the cutover deployment".to_owned(),
                )
            })?;
        build_production_evidence(config, &deployment, &discovery, workspace, authority)
    }

    fn targets_command(
        &self,
        config: &DaemonConfig,
        workspace_id: WorkspaceId,
        command: &FabricCommand,
    ) -> Result<bool, ProductionForwardCutoverError> {
        let FabricCommandPayload::Administer { action, request } = command.payload else {
            return Ok(false);
        };
        if action != AdministrationAction::ReconcileOperation
            || command.ownership.workspace_id != workspace_id
        {
            return Ok(false);
        }
        let deployment = self.verify_deployment(config)?;
        Ok(deployment.manifest.workspaces.iter().any(|workspace| {
            workspace.workspace_id == *workspace_id.as_bytes()
                && workspace.plan_id == *request.as_bytes()
        }))
    }

    fn read_exact_command(
        &self,
        workspace_id: WorkspaceId,
        binding: CutoverCommandBinding,
    ) -> Result<ExactCutoverJournalRead, ProductionForwardCutoverError> {
        let journal = DurableForwardCutoverJournal::open(&self.journal_path)
            .map_err(|error| ProductionForwardCutoverError::Journal(error.to_string()))?;
        let chain = journal
            .events(workspace_id)
            .map_err(|error| ProductionForwardCutoverError::Journal(error.to_string()))?;
        let matching = chain.iter().find(|event| {
            event.command().operation_id() == binding.operation_id()
                && event.command().transaction() == binding.transaction()
        });
        if let Some(event) = matching {
            if event.command() != binding {
                return Err(ProductionForwardCutoverError::Command(
                    "journal operation/transaction indexes resolve to another command binding"
                        .to_owned(),
                ));
            }
            if event.phase() == crate::fabric::forward_cutover::CutoverPhase::Complete {
                return Ok(ExactCutoverJournalRead::Committed(event.event_id()));
            }
        }
        Ok(ExactCutoverJournalRead::ProvedNotCommitted(
            reconciliation_evidence(
                workspace_id,
                binding,
                chain.last().map(CutoverEvent::event_id),
                b"not-committed",
            ),
        ))
    }

    fn verify_deployment(
        &self,
        config: &DaemonConfig,
    ) -> Result<VerifiedDeployment, ProductionForwardCutoverError> {
        let manifest_metadata = private_regular_file(&self.manifest_path)?;
        if manifest_metadata.len() > MAX_DEPLOYMENT_BYTES {
            return Err(ProductionForwardCutoverError::Deployment(
                "cutover deployment manifest exceeds its bound".to_owned(),
            ));
        }
        let manifest_bytes =
            fs::read(&self.manifest_path).map_err(|source| ProductionForwardCutoverError::Io {
                path: self.manifest_path.clone(),
                source,
            })?;
        let manifest: ForwardCutoverDeploymentManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| {
                ProductionForwardCutoverError::Deployment(format!(
                    "invalid closed deployment manifest: {error}"
                ))
            })?;
        if manifest.schema != DEPLOYMENT_SCHEMA || manifest.workspaces.is_empty() {
            return Err(ProductionForwardCutoverError::Deployment(
                "deployment schema or workspace census is invalid".to_owned(),
            ));
        }
        for relative in [
            &manifest.target_entrypoint,
            &manifest.daemon_config_file,
            &config.static_config.query_capability_token_file,
        ] {
            require_direct_relative_file(relative)?;
        }
        if !manifest.package_root.is_absolute()
            || manifest.package_root == config.static_config.config_root
        {
            return Err(ProductionForwardCutoverError::Deployment(
                "package root must be an independent absolute private root".to_owned(),
            ));
        }
        private_root(&manifest.package_root)?;
        private_root(&config.static_config.config_root)?;

        let package_names = direct_file_names(&manifest.package_root)?;
        let expected_package = BTreeSet::from([manifest
            .target_entrypoint
            .file_name()
            .expect("validated direct file")
            .to_os_string()]);
        if package_names != expected_package {
            return Err(ProductionForwardCutoverError::PhysicalZeroState(
                "package root contains an unapproved predecessor, bridge, or entrypoint".to_owned(),
            ));
        }
        let expected_config = BTreeSet::from([
            OsString::from(FORWARD_CUTOVER_DEPLOYMENT_FILE),
            manifest
                .daemon_config_file
                .file_name()
                .expect("validated direct file")
                .to_os_string(),
            config
                .static_config
                .query_capability_token_file
                .file_name()
                .expect("validated direct file")
                .to_os_string(),
        ]);
        if expected_config.len() != 3
            || direct_file_names(&config.static_config.config_root)? != expected_config
        {
            return Err(ProductionForwardCutoverError::PhysicalZeroState(
                "configuration root contains an unapproved predecessor, bridge, or entrypoint"
                    .to_owned(),
            ));
        }
        let target_entrypoint = manifest.package_root.join(&manifest.target_entrypoint);
        let target_metadata = private_package_entrypoint(&target_entrypoint)?;
        let current_executable =
            std::env::current_exe().map_err(|source| ProductionForwardCutoverError::Io {
                path: PathBuf::from("/proc/self/exe"),
                source,
            })?;
        let current_metadata = fs::metadata(&current_executable).map_err(|source| {
            ProductionForwardCutoverError::Io {
                path: current_executable,
                source,
            }
        })?;
        if target_metadata.dev() != current_metadata.dev()
            || target_metadata.ino() != current_metadata.ino()
        {
            return Err(ProductionForwardCutoverError::PhysicalZeroState(
                "running process is not the sole configured target entrypoint".to_owned(),
            ));
        }
        let daemon_config_path = config
            .static_config
            .config_root
            .join(&manifest.daemon_config_file);
        let decoded = DaemonConfig::load(&daemon_config_path).map_err(|error| {
            ProductionForwardCutoverError::Deployment(format!(
                "target daemon config readback failed: {error}"
            ))
        })?;
        if decoded != *config {
            return Err(ProductionForwardCutoverError::Deployment(
                "running daemon config differs from the sole configured target config".to_owned(),
            ));
        }
        let config_bytes =
            fs::read(&daemon_config_path).map_err(|source| ProductionForwardCutoverError::Io {
                path: daemon_config_path,
                source,
            })?;
        let target_bytes =
            fs::read(&target_entrypoint).map_err(|source| ProductionForwardCutoverError::Io {
                path: target_entrypoint,
                source,
            })?;
        let target_release = DaemonReleaseId::from_bytes(digest(
            b"codefabric.forward-cutover.target-artifact-identity.v1",
            &[&target_bytes],
        ));
        let supervisor_config = SupervisorConfigId::from_bytes(digest(
            b"codefabric.forward-cutover.supervisor-config-identity.v1",
            &[&config_bytes],
        ));
        let host_bytes = platform_host_identity_bytes()?;
        let boot_bytes = platform_boot_identity_bytes()?;
        let deployment_host = HostIdentity::from_bytes(digest(
            b"codefabric.forward-cutover.platform-host-identity.v1",
            &[&host_bytes],
        ));
        let boot_digest = digest(
            b"codefabric.forward-cutover.platform-boot-identity.v1",
            &[&boot_bytes],
        );
        let current_boot = HostBootId::from_bytes(
            boot_digest[..16]
                .try_into()
                .expect("fixed digest prefix has fixed width"),
        );
        let physical_zero_fact = fact_id(
            b"codefabric.forward-cutover.physical-zero-layout.v1",
            &[
                &manifest_bytes,
                &config_bytes,
                target_release.as_bytes(),
                deployment_host.as_bytes(),
                current_boot.as_bytes(),
            ],
        );
        let mut workspaces = BTreeSet::new();
        for workspace in &manifest.workspaces {
            if workspace.plan_id == [0; 32]
                || workspace.workspace_id == [0; 16]
                || workspace.predecessor_release == [0; 32]
                || !workspaces.insert(workspace.workspace_id)
            {
                return Err(ProductionForwardCutoverError::Deployment(
                    "workspace cutover identity is absent or duplicated".to_owned(),
                ));
            }
        }
        Ok(VerifiedDeployment {
            manifest,
            target_release,
            supervisor_config,
            deployment_host,
            current_boot,
            physical_zero_fact,
        })
    }

    #[cfg(test)]
    pub(crate) fn current_evidence_for_test(
        &self,
        config: &DaemonConfig,
        workspace_id: WorkspaceId,
        activation: CutoverActivationAuthority,
        writer_fence: WriterFence,
    ) -> Result<ProductionCutoverEvidence, ProductionForwardCutoverError> {
        self.current_evidence_for_authority(
            config,
            ProgrammaticAuthorityObservation {
                workspace_id,
                activation,
                writer_fence,
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn operator_statuses_for_test(
        &self,
        config: &DaemonConfig,
        workspace_id: WorkspaceId,
        activation: CutoverActivationAuthority,
        writer_fence: WriterFence,
    ) -> Result<Vec<ProductionCutoverStatus>, ProductionForwardCutoverError> {
        self.operator_statuses_from_authorities(
            config,
            &[ProgrammaticAuthorityObservation {
                workspace_id,
                activation,
                writer_fence,
            }],
        )
    }

    #[cfg(test)]
    pub(crate) fn command_intents_for_test(
        &self,
        config: &DaemonConfig,
        workspace_id: WorkspaceId,
        activation: CutoverActivationAuthority,
        writer_fence: WriterFence,
    ) -> Result<Vec<ProductionCutoverCommandIntent>, ProductionForwardCutoverError> {
        self.command_intents_from_authorities(
            config,
            &[ProgrammaticAuthorityObservation {
                workspace_id,
                activation,
                writer_fence,
            }],
        )
    }
}

enum ExactCutoverJournalRead {
    Committed(CutoverEventId),
    ProvedNotCommitted(ReconciliationEvidenceRef),
}

#[async_trait]
impl AdministrationCommandEffectPort for ForwardCutoverAdministrationEffect {
    async fn prepare(
        &self,
        executing: &CommandRecord,
        owner: ExecutionOwner,
        context: ReductionContext,
    ) -> Result<PrepareEffectOutcome, CommandPortError> {
        if !self
            .binding
            .controller
            .targets_command(&self.binding.config, self.workspace_id, executing.command())
            .map_err(cutover_port_error)?
        {
            return self.delegate.prepare(executing, owner, context).await;
        }
        let validated = executing_attempt(executing, owner, context, CommandKind::Administer)?;
        Ok(PrepareEffectOutcome::Prepared {
            transaction: cutover_transaction(validated)?,
        })
    }

    async fn commit(
        &self,
        prepared: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<CommitEffectOutcome, CommandPortError> {
        if !self
            .binding
            .controller
            .targets_command(&self.binding.config, self.workspace_id, prepared.command())
            .map_err(cutover_port_error)?
        {
            return self
                .delegate
                .commit(prepared, owner, transaction, context)
                .await;
        }
        let outcome = self
            .binding
            .controller
            .advance_from_runtime_authority(
                &self.binding.config,
                &self.activation_authority,
                prepared,
                owner,
                transaction,
                context,
            )
            .await
            .map_err(cutover_port_error)?;
        Ok(CommitEffectOutcome::Committed {
            result: cutover_result(prepared.command(), advance_event_id(&outcome)?),
        })
    }

    async fn reconcile(
        &self,
        awaiting: &CommandRecord,
        owner: ExecutionOwner,
        transaction: TransactionRef,
        context: ReductionContext,
    ) -> Result<ReconciliationObservation, CommandPortError> {
        if !self
            .binding
            .controller
            .targets_command(&self.binding.config, self.workspace_id, awaiting.command())
            .map_err(cutover_port_error)?
        {
            return self
                .delegate
                .reconcile(awaiting, owner, transaction, context)
                .await;
        }
        let recovery = reconciliation_attempt(
            awaiting,
            owner,
            transaction,
            context,
            CommandKind::Administer,
        )?;
        let binding = CutoverCommandBinding::try_from_validated_attempt(recovery.attempt())
            .map_err(|_| CommandPortError::CorruptRecord)?;
        match self
            .binding
            .controller
            .read_exact_command(self.workspace_id, binding)
            .map_err(cutover_port_error)?
        {
            ExactCutoverJournalRead::Committed(event_id) => {
                Ok(ReconciliationObservation::Committed {
                    evidence: reconciliation_evidence(
                        self.workspace_id,
                        binding,
                        Some(event_id),
                        b"committed",
                    ),
                    result: cutover_result(awaiting.command(), event_id),
                })
            }
            ExactCutoverJournalRead::ProvedNotCommitted(evidence) => {
                Ok(ReconciliationObservation::NotCommitted { evidence })
            }
        }
    }
}

fn cutover_transaction(
    attempt: ValidatedCommandAttempt,
) -> Result<TransactionRef, CommandPortError> {
    let command = attempt.command();
    let value = serde_json::to_value(command).map_err(|_| CommandPortError::CorruptRecord)?;
    let bytes = crate::contracts::jcs::canonicalize_value(&value)
        .map_err(|_| CommandPortError::CorruptRecord)?;
    let owner = attempt.execution_owner();
    Ok(TransactionRef::from_bytes(digest(
        b"codefabric.forward-cutover.command-transaction.v1",
        &[
            &bytes,
            &u64::from(attempt.attempt()).to_be_bytes(),
            owner.actor_id.as_bytes(),
            owner.fence.lease_id.as_bytes(),
            &owner.fence.generation.get().to_be_bytes(),
        ],
    )))
}

fn advance_event_id(
    outcome: &ProductionCutoverAdvanceOutcome,
) -> Result<CutoverEventId, CommandPortError> {
    let status = match outcome {
        ProductionCutoverAdvanceOutcome::Advanced { status, .. }
        | ProductionCutoverAdvanceOutcome::AlreadyConverged(status) => status,
    };
    match status.status.durable_state {
        DurableCutoverState::At {
            phase: crate::fabric::forward_cutover::CutoverPhase::Complete,
            event_id,
            ..
        } => Ok(event_id),
        _ => Err(CommandPortError::CorruptRecord),
    }
}

fn cutover_result(command: &FabricCommand, event_id: CutoverEventId) -> CommandResult {
    let FabricCommandPayload::Administer { request, .. } = command.payload else {
        unreachable!("cutover effect is selected only for administrative commands")
    };
    CommandResult::AdministrationApplied {
        request,
        resulting_head: command.expected_head,
        selection: OperationSelectionRef::from_bytes(*event_id.as_bytes()),
    }
}

fn reconciliation_evidence(
    workspace_id: WorkspaceId,
    binding: CutoverCommandBinding,
    observed_event: Option<CutoverEventId>,
    outcome: &[u8],
) -> ReconciliationEvidenceRef {
    ReconciliationEvidenceRef::from_bytes(digest(
        b"codefabric.forward-cutover.reconciliation-readback.v1",
        &[
            workspace_id.as_bytes(),
            binding.operation_id().as_bytes(),
            binding.transaction().as_bytes(),
            binding.writer_fence().lease_id.as_bytes(),
            &binding.writer_fence().generation.get().to_be_bytes(),
            observed_event
                .as_ref()
                .map_or(&[][..], |event| event.as_bytes().as_slice()),
            outcome,
        ],
    ))
}

fn cutover_port_error(error: ProductionForwardCutoverError) -> CommandPortError {
    match error {
        ProductionForwardCutoverError::Command(_) => CommandPortError::CorruptRecord,
        ProductionForwardCutoverError::Journal(_) | ProductionForwardCutoverError::Io { .. } => {
            CommandPortError::DurableStoreUnavailable
        }
        ProductionForwardCutoverError::Deployment(_)
        | ProductionForwardCutoverError::PhysicalZeroState(_)
        | ProductionForwardCutoverError::Authority(_)
        | ProductionForwardCutoverError::AdmissionClosed { .. } => {
            CommandPortError::ContextUnavailable
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        ProductionForwardCutoverError::UnsupportedPlatform => CommandPortError::ContextUnavailable,
    }
}

fn build_production_evidence(
    config: &DaemonConfig,
    deployment: &VerifiedDeployment,
    discovery: &DaemonDiscovery,
    workspace: &ForwardCutoverWorkspaceManifest,
    authority: ProgrammaticAuthorityObservation,
) -> Result<ProductionCutoverEvidence, ProductionForwardCutoverError> {
    require_strict_writer_successor(authority.activation.writer_fence(), authority.writer_fence)?;
    let workspace_id = WorkspaceId::from_bytes(workspace.workspace_id);
    let uds_endpoint =
        UdsEndpointId::derive(workspace_id, &config.static_config.query_socket_endpoint)
            .map_err(|error| ProductionForwardCutoverError::Authority(error.to_string()))?;
    let identity = CutoverChainIdentity::try_new(
        CutoverPlanId::from_bytes(workspace.plan_id),
        workspace_id,
        deployment.deployment_host,
        deployment.target_release,
        DaemonReleaseId::from_bytes(workspace.predecessor_release),
        deployment.supervisor_config,
        uds_endpoint,
    )
    .map_err(|error| ProductionForwardCutoverError::Authority(error.to_string()))?;
    let epoch = authority.activation.selected_epoch();
    let fence = authority.writer_fence;
    let uds_fact = fact_id(
        b"codefabric.forward-cutover.actual-uds.v1",
        &[
            &discovery.pid.to_be_bytes(),
            config
                .static_config
                .socket_endpoint
                .as_os_str()
                .as_encoded_bytes(),
            config
                .static_config
                .query_socket_endpoint
                .as_os_str()
                .as_encoded_bytes(),
        ],
    );
    let serving_fact = fact_id(
        b"codefabric.forward-cutover.actual-serving.v1",
        &[
            &discovery.process_start_token.to_be_bytes(),
            discovery.daemon_instance_id.as_bytes(),
            epoch.as_bytes(),
        ],
    );
    let writer_fact = fact_id(
        b"codefabric.forward-cutover.actual-writer.v1",
        &[
            fence.lease_id.as_bytes(),
            &fence.generation.get().to_be_bytes(),
            authority.activation.event_id().as_bytes(),
            epoch.as_bytes(),
        ],
    );
    let bind_denial = fact_id(
        b"codefabric.forward-cutover.physical-bind-zero.v1",
        &[
            deployment.physical_zero_fact.as_bytes(),
            uds_fact.as_bytes(),
        ],
    );
    let serve_denial = fact_id(
        b"codefabric.forward-cutover.physical-serve-zero.v1",
        &[
            deployment.physical_zero_fact.as_bytes(),
            serving_fact.as_bytes(),
        ],
    );
    let writer_denial = fact_id(
        b"codefabric.forward-cutover.physical-writer-zero.v1",
        &[
            deployment.physical_zero_fact.as_bytes(),
            writer_fact.as_bytes(),
        ],
    );
    let current_boot_fact = fact_id(
        b"codefabric.forward-cutover.physical-zero-current-boot.v1",
        &[
            deployment.current_boot.as_bytes(),
            deployment.physical_zero_fact.as_bytes(),
        ],
    );
    let supervisor = SupervisorObservation::try_from_actual_readback(
        identity,
        ActualSupervisorReadback {
            config_id: deployment.supervisor_config,
            host_identity: deployment.deployment_host,
            // There is no predecessor executable to reboot-test. The exact package and
            // configuration census below authorizes same-boot physical zero-state.
            previous_boot_id: deployment.current_boot,
            current_boot_id: deployment.current_boot,
            reboot_observation: current_boot_fact,
            target_release: deployment.target_release,
            predecessor_release: DaemonReleaseId::from_bytes(workspace.predecessor_release),
            uds_endpoint_id: uds_endpoint,
            uds_owner: AuthorityOwner::Target,
            uds_observation: uds_fact,
            serving_owner: AuthorityOwner::Target,
            serving_observation: serving_fact,
            writer_owner: AuthorityOwner::Target,
            writer_observation: writer_fact,
            activation_head: Some(epoch),
            programmatic_epoch: Some(epoch),
            predecessor_revocation: Some(PredecessorRevocationReadback {
                bind_denial,
                serve_denial,
                writer_denial,
            }),
            predecessor_package: ObservedAvailability::Absent,
            temporary_bridge: ObservedAvailability::Absent,
        },
    )
    .map_err(|error| ProductionForwardCutoverError::Authority(error.to_string()))?;
    Ok(ProductionCutoverEvidence {
        identity,
        supervisor,
        activation: authority.activation,
        current_writer_fence: fence,
    })
}

fn require_strict_writer_successor(
    activation_fence: WriterFence,
    current_fence: WriterFence,
) -> Result<(), ProductionForwardCutoverError> {
    if current_fence.generation.get() <= activation_fence.generation.get() {
        return Err(ProductionForwardCutoverError::Authority(
            "current OS-backed writer generation is not a strict successor of the activation-event fence"
                .to_owned(),
        ));
    }
    Ok(())
}

async fn observe_programmatic_authorities(
    composition: &ProgrammaticDaemonComposition,
) -> Result<Vec<ProgrammaticAuthorityObservation>, ProductionForwardCutoverError> {
    let mut observations = Vec::with_capacity(composition.workspaces().len());
    for (workspace_id, workspace) in composition.workspaces() {
        let handle = composition
            .command_runtime_handle(*workspace_id)
            .ok_or_else(|| {
                ProductionForwardCutoverError::Authority(
                    "programmatic workspace has no ready OS-lease-backed command handle".to_owned(),
                )
            })?;
        let snapshot = workspace
            .activation_authority()
            .current_snapshot()
            .await
            .map_err(|error| ProductionForwardCutoverError::Authority(error.to_string()))?;
        let activation = CutoverActivationAuthority::try_from_chain(&snapshot.chain)
            .map_err(|error| ProductionForwardCutoverError::Authority(error.to_string()))?;
        let startup = workspace.startup_observation();
        // The startup observation records the activation-event fence before command-runtime
        // ownership is acquired. The same durable generation store later supplies both the live
        // handle fence and `snapshot.active_fence`; those two are the strict successor pair.
        if handle.fence() != snapshot.active_fence
            || startup.active_fence != activation.writer_fence()
            || startup.epoch_id != activation.selected_epoch()
            || startup.activation_event_id != activation.event_id()
        {
            return Err(ProductionForwardCutoverError::Authority(
                "command lease, activation readback, and installed startup authority disagree"
                    .to_owned(),
            ));
        }
        require_strict_writer_successor(activation.writer_fence(), handle.fence())?;
        observations.push(ProgrammaticAuthorityObservation {
            workspace_id: *workspace_id,
            activation,
            writer_fence: handle.fence(),
        });
    }
    Ok(observations)
}

fn verify_live_process_and_sockets(
    config: &DaemonConfig,
) -> Result<DaemonDiscovery, ProductionForwardCutoverError> {
    let discovery_path = config.static_config.runtime_root.join("daemon.json");
    let metadata = private_regular_file(&discovery_path)?;
    if metadata.len() > MAX_DEPLOYMENT_BYTES {
        return Err(ProductionForwardCutoverError::Deployment(
            "daemon discovery readback exceeds its bound".to_owned(),
        ));
    }
    let bytes = fs::read(&discovery_path).map_err(|source| ProductionForwardCutoverError::Io {
        path: discovery_path,
        source,
    })?;
    let discovery: DaemonDiscovery = serde_json::from_slice(&bytes).map_err(|error| {
        ProductionForwardCutoverError::Deployment(format!(
            "invalid daemon discovery readback: {error}"
        ))
    })?;
    if discovery.pid != std::process::id()
        || discovery.socket_endpoint != config.static_config.socket_endpoint
        || discovery.query_socket_endpoint != config.static_config.query_socket_endpoint
        || discovery.public_bundle_versions.is_empty()
    {
        return Err(ProductionForwardCutoverError::Authority(
            "discovery does not identify the current target process and exact endpoints".to_owned(),
        ));
    }
    for socket in [
        &config.static_config.socket_endpoint,
        &config.static_config.query_socket_endpoint,
    ] {
        let metadata =
            fs::symlink_metadata(socket).map_err(|source| ProductionForwardCutoverError::Io {
                path: socket.clone(),
                source,
            })?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(ProductionForwardCutoverError::Authority(format!(
                "{} is not the current user's private Unix socket",
                socket.display()
            )));
        }
    }
    Ok(discovery)
}

fn private_root(path: &Path) -> Result<(), ProductionForwardCutoverError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| ProductionForwardCutoverError::Io {
            path: path.to_owned(),
            source,
        })?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(ProductionForwardCutoverError::PhysicalZeroState(format!(
            "{} is not an owner-private physical root",
            path.display()
        )));
    }
    Ok(())
}

fn private_regular_file(path: &Path) -> Result<fs::Metadata, ProductionForwardCutoverError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| ProductionForwardCutoverError::Io {
            path: path.to_owned(),
            source,
        })?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(ProductionForwardCutoverError::PhysicalZeroState(format!(
            "{} is not an owner-private regular file",
            path.display()
        )));
    }
    Ok(metadata)
}

fn private_package_entrypoint(path: &Path) -> Result<fs::Metadata, ProductionForwardCutoverError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| ProductionForwardCutoverError::Io {
            path: path.to_owned(),
            source,
        })?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.permissions().mode() & 0o100 == 0
    {
        return Err(ProductionForwardCutoverError::PhysicalZeroState(format!(
            "{} is not the owner-executable target entrypoint",
            path.display()
        )));
    }
    Ok(metadata)
}

fn direct_file_names(root: &Path) -> Result<BTreeSet<OsString>, ProductionForwardCutoverError> {
    fs::read_dir(root)
        .map_err(|source| ProductionForwardCutoverError::Io {
            path: root.to_owned(),
            source,
        })?
        .map(|entry| {
            let entry = entry.map_err(|source| ProductionForwardCutoverError::Io {
                path: root.to_owned(),
                source,
            })?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(|source| {
                ProductionForwardCutoverError::Io {
                    path: entry.path(),
                    source,
                }
            })?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(ProductionForwardCutoverError::PhysicalZeroState(format!(
                    "{} contains a non-file or symlink deployment entry",
                    root.display()
                )));
            }
            Ok(entry.file_name())
        })
        .collect()
}

fn require_direct_relative_file(path: &Path) -> Result<(), ProductionForwardCutoverError> {
    if path.is_absolute()
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(ProductionForwardCutoverError::Deployment(format!(
            "{} must be one direct relative filename",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn platform_host_identity_bytes() -> Result<Vec<u8>, ProductionForwardCutoverError> {
    read_platform_identity(Path::new("/etc/machine-id"), "host")
}

#[cfg(target_os = "linux")]
fn platform_boot_identity_bytes() -> Result<Vec<u8>, ProductionForwardCutoverError> {
    read_platform_identity(Path::new("/proc/sys/kernel/random/boot_id"), "boot")
}

#[cfg(target_os = "macos")]
fn platform_host_identity_bytes() -> Result<Vec<u8>, ProductionForwardCutoverError> {
    run_platform_identity("/usr/bin/uname", &["-n"], "host")
}

#[cfg(target_os = "macos")]
fn platform_boot_identity_bytes() -> Result<Vec<u8>, ProductionForwardCutoverError> {
    run_platform_identity("/usr/sbin/sysctl", &["-n", "kern.boottime"], "boot")
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn platform_host_identity_bytes() -> Result<Vec<u8>, ProductionForwardCutoverError> {
    Err(ProductionForwardCutoverError::UnsupportedPlatform)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn platform_boot_identity_bytes() -> Result<Vec<u8>, ProductionForwardCutoverError> {
    Err(ProductionForwardCutoverError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
fn read_platform_identity(
    path: &Path,
    kind: &str,
) -> Result<Vec<u8>, ProductionForwardCutoverError> {
    let bytes = fs::read(path).map_err(|source| ProductionForwardCutoverError::Io {
        path: path.to_owned(),
        source,
    })?;
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(ProductionForwardCutoverError::Authority(format!(
            "platform {kind} identity is empty"
        )));
    }
    Ok(bytes)
}

#[cfg(target_os = "macos")]
fn run_platform_identity(
    program: &str,
    args: &[&str],
    kind: &str,
) -> Result<Vec<u8>, ProductionForwardCutoverError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|source| ProductionForwardCutoverError::Io {
            path: PathBuf::from(program),
            source,
        })?;
    if !output.status.success() || output.stdout.iter().all(u8::is_ascii_whitespace) {
        return Err(ProductionForwardCutoverError::Authority(format!(
            "platform {kind} identity readback failed"
        )));
    }
    Ok(output.stdout)
}

fn digest(domain: &[u8], chunks: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for chunk in chunks {
        hasher.update(&(chunk.len() as u64).to_be_bytes());
        hasher.update(chunk);
    }
    *hasher.finalize().as_bytes()
}

fn fact_id(domain: &[u8], chunks: &[&[u8]]) -> SupervisorFactId {
    SupervisorFactId::from_bytes(digest(domain, chunks))
}

/// Fail-closed errors from the external deployment observation boundary.
#[derive(Debug, Error)]
pub enum ProductionForwardCutoverError {
    #[error("forward-cutover I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid forward-cutover deployment: {0}")]
    Deployment(String),
    #[error("physical predecessor zero-state failed: {0}")]
    PhysicalZeroState(String),
    #[error("programmatic cutover authority observation failed: {0}")]
    Authority(String),
    #[error("prepared cutover command admission failed: {0}")]
    Command(String),
    #[error("durable forward-cutover journal failed: {0}")]
    Journal(String),
    #[error("cutover admission is closed for workspace {workspace_id:02x?}: {code}")]
    AdmissionClosed {
        workspace_id: [u8; 16],
        code: &'static str,
    },
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    #[error("forward-cutover platform observation is unsupported on this operating system")]
    UnsupportedPlatform,
}
