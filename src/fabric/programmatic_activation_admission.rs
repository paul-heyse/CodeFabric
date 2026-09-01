//! Activation handoff from exact durable selection to one complete active workspace.
//!
//! The predecessor admission gate closes first. A release-owned builder then consumes the exact
//! `SelectedEpochRecord`, reconstructs every successor capability, and the kernel-owned
//! `WorkspaceSlot` swaps that complete value atomically. No query-authority registry or admission
//! epoch pointer participates.

use std::sync::{Arc, Mutex, Weak};

use async_trait::async_trait;
use thiserror::Error;

use super::activation::{ActivationChain, ActivationEvent};
use super::activation_transaction::{ActivationAdmissionPort, ActivationRecoveryAdmissionPort};
use super::admission::{
    ActivationBarrier, AdmissionError, FabricAdmissionRuntime, RecoverySelectionPublication,
};
use super::command::{ExpectedHead, WorkspaceId, WriterFence};
use super::production_kernel::{
    ActiveWorkspace, ActiveWorkspaceError, SelectedEpochRecord, WorkspaceSlot,
};
use super::programmatic_epoch::ProgrammaticFabricEpoch;

/// Release-owned reconstruction boundary for one complete successor workspace.
#[async_trait]
pub(crate) trait ReleaseOwnedActiveWorkspaceBuilder: Send + Sync {
    async fn build_activated(
        &self,
        selection: SelectedEpochRecord,
        chain_after_readback: &ActivationChain,
        candidate: Arc<ProgrammaticFabricEpoch>,
    ) -> Result<Arc<ActiveWorkspace>, ActiveWorkspaceBuildError>;

    async fn rebuild_selected(
        &self,
        selection: SelectedEpochRecord,
        chain_after_readback: &ActivationChain,
    ) -> Result<Arc<ActiveWorkspace>, ActiveWorkspaceBuildError>;
}

/// One bind-once indirection used to close the activation/runtime composition cycle.
///
/// The command router needs a successor builder, while the complete builder needs that same
/// command router. This handle carries only a weak reference, is bound exactly once after both
/// objects exist, and fails closed before binding or after owner teardown. It is not a registry
/// and cannot select a workspace or epoch.
#[derive(Default)]
pub(crate) struct ReleaseOwnedActiveWorkspaceBuilderHandle {
    target: Mutex<Option<Weak<dyn ReleaseOwnedActiveWorkspaceBuilder>>>,
}

impl ReleaseOwnedActiveWorkspaceBuilderHandle {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            target: Mutex::new(None),
        }
    }

    pub(crate) fn bind(
        &self,
        target: &Arc<dyn ReleaseOwnedActiveWorkspaceBuilder>,
    ) -> Result<(), ActiveWorkspaceBuildError> {
        let mut installed = self
            .target
            .lock()
            .map_err(|_| ActiveWorkspaceBuildError::Unavailable)?;
        if installed.is_some() {
            return Err(ActiveWorkspaceBuildError::Invalid);
        }
        *installed = Some(Arc::downgrade(target));
        Ok(())
    }

    fn upgrade(
        &self,
    ) -> Result<Arc<dyn ReleaseOwnedActiveWorkspaceBuilder>, ActiveWorkspaceBuildError> {
        self.target
            .lock()
            .map_err(|_| ActiveWorkspaceBuildError::Unavailable)?
            .as_ref()
            .and_then(Weak::upgrade)
            .ok_or(ActiveWorkspaceBuildError::Unavailable)
    }
}

#[async_trait]
impl ReleaseOwnedActiveWorkspaceBuilder for ReleaseOwnedActiveWorkspaceBuilderHandle {
    async fn build_activated(
        &self,
        selection: SelectedEpochRecord,
        chain_after_readback: &ActivationChain,
        candidate: Arc<ProgrammaticFabricEpoch>,
    ) -> Result<Arc<ActiveWorkspace>, ActiveWorkspaceBuildError> {
        self.upgrade()?
            .build_activated(selection, chain_after_readback, candidate)
            .await
    }

    async fn rebuild_selected(
        &self,
        selection: SelectedEpochRecord,
        chain_after_readback: &ActivationChain,
    ) -> Result<Arc<ActiveWorkspace>, ActiveWorkspaceBuildError> {
        self.upgrade()?
            .rebuild_selected(selection, chain_after_readback)
            .await
    }
}

/// Fail-closed complete-runtime reconstruction failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum ActiveWorkspaceBuildError {
    #[error("release-owned active-workspace reconstruction is unavailable")]
    Unavailable,
    #[error("release-owned active-workspace reconstruction differs from durable selection")]
    Invalid,
}

#[derive(Clone)]
struct InstalledSuccessor {
    barrier: ActivationBarrier,
    chain: ActivationChain,
    workspace: Arc<ActiveWorkspace>,
}

/// Admission wrapper enforcing successor-authority installation before atomic epoch swap.
pub(crate) struct ProgrammaticActivationAdmission {
    workspace_id: WorkspaceId,
    admission: Arc<FabricAdmissionRuntime>,
    slot: Weak<WorkspaceSlot>,
    builder: Arc<dyn ReleaseOwnedActiveWorkspaceBuilder>,
    pending: Mutex<Option<InstalledSuccessor>>,
}

impl std::fmt::Debug for ProgrammaticActivationAdmission {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProgrammaticActivationAdmission")
            .field("workspace_id", &self.workspace_id)
            .field("admission", &"exact-runtime")
            .field("workspace_slot", &"weak-kernel-owner")
            .field("active_workspace_builder", &"release-owned")
            .finish()
    }
}

impl ProgrammaticActivationAdmission {
    #[must_use]
    pub(crate) fn new(
        workspace_id: WorkspaceId,
        admission: Arc<FabricAdmissionRuntime>,
        slot: Weak<WorkspaceSlot>,
        builder: Arc<dyn ReleaseOwnedActiveWorkspaceBuilder>,
    ) -> Self {
        Self {
            workspace_id,
            admission,
            slot,
            builder,
            pending: Mutex::new(None),
        }
    }

    async fn install_successor(
        &self,
        barrier: ActivationBarrier,
        selection: SelectedEpochRecord,
        chain_after_readback: &ActivationChain,
        candidate: Arc<ProgrammaticFabricEpoch>,
    ) -> Result<(), AdmissionError> {
        if selection.workspace_id() != self.workspace_id
            || chain_after_readback.workspace_id() != self.workspace_id
            || chain_after_readback.head_event().copied() != Some(selection.event())
            || candidate.identity() != &selection.epoch_id()
            || candidate.table_version_set_ref() != selection.table_versions().reference()
        {
            return Err(AdmissionError::SuccessorQueryAuthorityMismatch(
                selection.epoch_id(),
            ));
        }
        let workspace = self
            .builder
            .build_activated(
                selection.clone(),
                chain_after_readback,
                Arc::clone(&candidate),
            )
            .await
            .map_err(|outcome| match outcome {
                ActiveWorkspaceBuildError::Unavailable => {
                    AdmissionError::SuccessorQueryAuthorityUnavailable(selection.epoch_id())
                }
                ActiveWorkspaceBuildError::Invalid => {
                    AdmissionError::SuccessorQueryAuthorityMismatch(selection.epoch_id())
                }
            })?;
        if workspace.selection() != &selection
            || !Arc::ptr_eq(workspace.runtime().epoch(), &candidate)
        {
            return Err(AdmissionError::SuccessorQueryAuthorityMismatch(
                selection.epoch_id(),
            ));
        }
        self.admission.publish_selected_epoch(
            barrier,
            chain_after_readback,
            Arc::clone(&candidate),
        )?;
        let slot =
            self.slot
                .upgrade()
                .ok_or(AdmissionError::SuccessorQueryAuthorityUnavailable(
                    selection.epoch_id(),
                ))?;
        slot.swap(Arc::clone(&workspace)).map_err(|_| {
            AdmissionError::SuccessorQueryAuthorityInstallFailed(selection.epoch_id())
        })?;
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| AdmissionError::StatePoisoned)?;
        *pending = Some(InstalledSuccessor {
            barrier,
            chain: chain_after_readback.clone(),
            workspace,
        });
        Ok(())
    }
}

#[async_trait]
impl ActivationAdmissionPort for ProgrammaticActivationAdmission {
    type Barrier = ActivationBarrier;

    async fn close_admission(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
    ) -> Result<Self::Barrier, AdmissionError> {
        self.admission
            .close_admission(expected_head, execution_fence)
    }

    async fn publish_selected_epoch(
        &self,
        barrier: Self::Barrier,
        selection: SelectedEpochRecord,
        chain_after_readback: &ActivationChain,
        candidate: Arc<ProgrammaticFabricEpoch>,
    ) -> Result<(), AdmissionError> {
        self.install_successor(barrier, selection, chain_after_readback, candidate)
            .await
    }

    async fn reconcile_and_reopen(
        &self,
        barrier: Self::Barrier,
        reconciled_head: ExpectedHead,
    ) -> Result<(), AdmissionError> {
        let installed = self
            .pending
            .lock()
            .map_err(|_| AdmissionError::StatePoisoned)?
            .clone()
            .ok_or(AdmissionError::SelectionNotPublished)?;
        if installed.barrier != barrier
            || reconciled_head != ExpectedHead::Epoch(installed.workspace.selection().epoch_id())
        {
            return Err(AdmissionError::StaleBarrier);
        }
        let slot =
            self.slot
                .upgrade()
                .ok_or(AdmissionError::SuccessorQueryAuthorityUnavailable(
                    installed.workspace.selection().epoch_id(),
                ))?;
        let leased = slot.lease().map_err(|_| {
            AdmissionError::SuccessorQueryAuthorityUnavailable(
                installed.workspace.selection().epoch_id(),
            )
        })?;
        if !Arc::ptr_eq(leased.workspace(), &installed.workspace) {
            return Err(AdmissionError::SuccessorQueryAuthorityMismatch(
                installed.workspace.selection().epoch_id(),
            ));
        }
        let selection = installed.workspace.selection();
        installed
            .workspace
            .runtime()
            .admission()
            .install_reconciled_selected_head(
                selection.event(),
                &installed.chain,
                Arc::clone(installed.workspace.runtime().epoch()),
                selection.control_horizon().active_recovery_fence(),
            )?;
        self.admission
            .finish_predecessor_handoff(barrier, selection.epoch_id())?;
        *self
            .pending
            .lock()
            .map_err(|_| AdmissionError::StatePoisoned)? = None;
        Ok(())
    }

    async fn abort_proved_no_selection(
        &self,
        barrier: Self::Barrier,
        unchanged_chain: &ActivationChain,
    ) -> Result<(), AdmissionError> {
        self.admission
            .abort_before_selection(barrier, unchanged_chain)
    }
}

#[async_trait]
impl ActivationRecoveryAdmissionPort for ProgrammaticActivationAdmission {
    async fn recover_selected_epoch(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
        active_recovery_fence: WriterFence,
        selection: SelectedEpochRecord,
        chain_after_readback: &ActivationChain,
        allow_already_reopened: bool,
    ) -> Result<RecoverySelectionPublication, AdmissionError> {
        let event = selection.event();
        let workspace = self
            .builder
            .rebuild_selected(selection.clone(), chain_after_readback)
            .await
            .map_err(|error| match error {
                ActiveWorkspaceBuildError::Unavailable => {
                    AdmissionError::SuccessorQueryAuthorityUnavailable(selection.epoch_id())
                }
                ActiveWorkspaceBuildError::Invalid => {
                    AdmissionError::SuccessorQueryAuthorityMismatch(selection.epoch_id())
                }
            })?;
        if workspace.selection() != &selection {
            return Err(AdmissionError::SuccessorQueryAuthorityMismatch(
                selection.epoch_id(),
            ));
        }
        let publication = self.admission.recover_selected_epoch(
            expected_head,
            execution_fence,
            active_recovery_fence,
            event,
            chain_after_readback,
            allow_already_reopened,
        )?;
        let slot =
            self.slot
                .upgrade()
                .ok_or(AdmissionError::SuccessorQueryAuthorityUnavailable(
                    selection.epoch_id(),
                ))?;
        match slot.lease() {
            Ok(lease) if lease.workspace().selection() == &selection => {}
            Ok(_) => {
                slot.swap(workspace).map_err(|_| {
                    AdmissionError::SuccessorQueryAuthorityInstallFailed(selection.epoch_id())
                })?;
            }
            Err(ActiveWorkspaceError::NotInstalled(_)) => {
                slot.install_initial(workspace).map_err(|_| {
                    AdmissionError::SuccessorQueryAuthorityInstallFailed(selection.epoch_id())
                })?;
            }
            Err(_) => {
                return Err(AdmissionError::SuccessorQueryAuthorityInstallFailed(
                    selection.epoch_id(),
                ));
            }
        }
        Ok(publication)
    }

    async fn reopen_recovered_selection(
        &self,
        event: ActivationEvent,
        chain_after_readback: &ActivationChain,
        active_recovery_fence: WriterFence,
    ) -> Result<(), AdmissionError> {
        let slot =
            self.slot
                .upgrade()
                .ok_or(AdmissionError::SuccessorQueryAuthorityUnavailable(
                    event.pins().epoch,
                ))?;
        let workspace = slot
            .lease()
            .map_err(|_| AdmissionError::SuccessorQueryAuthorityUnavailable(event.pins().epoch))?;
        if workspace.workspace().selection().event() != event {
            return Err(AdmissionError::RecoveryPublishedSelectionMismatch);
        }
        let successor = workspace.workspace().runtime().admission();
        successor.install_reconciled_selected_head(
            event,
            chain_after_readback,
            Arc::clone(workspace.workspace().runtime().epoch()),
            active_recovery_fence,
        )?;
        if !Arc::ptr_eq(successor, &self.admission) {
            self.admission.finish_recovered_predecessor(
                event,
                chain_after_readback,
                active_recovery_fence,
            )?;
        }
        Ok(())
    }

    async fn recover_proved_no_selection(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
        active_recovery_fence: WriterFence,
        unchanged_chain: &ActivationChain,
    ) -> Result<(), AdmissionError> {
        self.admission.recover_proved_no_selection(
            expected_head,
            execution_fence,
            active_recovery_fence,
            unchanged_chain,
        )
    }
}
