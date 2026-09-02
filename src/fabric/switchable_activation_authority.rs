//! Process-owned activation authority that follows exact Delta control-history readbacks.
//!
//! The command actor outlives any one selected epoch.  This owner lets that actor keep one
//! stable port identity while activation atomically installs a newly read, exact-version Delta
//! authority.  It is not a registry and cannot select by label, workspace, or latest version.

use std::sync::Arc;

use arc_swap::ArcSwap;
use async_trait::async_trait;
use thiserror::Error;

use super::activation::ActivationChain;
use super::activation_control_delta::{
    DeltaActivationRuntimeAuthority, DeltaActivationRuntimeAuthoritySnapshotError,
    ExactActivationControlSelection,
};
use super::activation_transaction::{
    ActivationAppendContract, ActivationAppendOutcome, ActivationAuthorityPort,
    ActivationAuthorityRequest, ActivationEventPort, ActivationOperationMarkerOutcome,
    ActivationOperationMarkerPort, ActivationOperationMarkerRequest, AuthorityRevalidationOutcome,
};
use super::command::WorkspaceId;
use super::command_actor::CommandPortError;
use super::command_runtime_ports::CommandActivationChainPort;

/// Stable process authority over one workspace's current exact activation-control snapshot.
pub(crate) struct SwitchableActivationAuthority {
    workspace_id: WorkspaceId,
    current: ArcSwap<DeltaActivationRuntimeAuthority>,
}

impl std::fmt::Debug for SwitchableActivationAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SwitchableActivationAuthority")
            .field("workspace_id", &self.workspace_id)
            .field("control_relation", self.current.load().control_relation())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum SwitchableActivationAuthorityError {
    #[error("activation authority successor belongs to another workspace")]
    WorkspaceMismatch,
}

impl SwitchableActivationAuthority {
    #[must_use]
    pub(crate) fn new(initial: Arc<DeltaActivationRuntimeAuthority>) -> Self {
        Self {
            workspace_id: initial.workspace_id(),
            current: ArcSwap::from(initial),
        }
    }

    #[must_use]
    pub(crate) const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub(crate) fn current(&self) -> Arc<DeltaActivationRuntimeAuthority> {
        self.current.load_full()
    }

    /// Install one already exact successor authority for the same workspace.
    pub(crate) fn install(
        &self,
        successor: Arc<DeltaActivationRuntimeAuthority>,
    ) -> Result<(), SwitchableActivationAuthorityError> {
        if successor.workspace_id() != self.workspace_id {
            return Err(SwitchableActivationAuthorityError::WorkspaceMismatch);
        }
        self.current.store(successor);
        Ok(())
    }

    pub(crate) async fn current_selection(
        &self,
    ) -> Result<ExactActivationControlSelection, DeltaActivationRuntimeAuthoritySnapshotError> {
        self.current().current_selection().await
    }
}

#[async_trait]
impl CommandActivationChainPort for SwitchableActivationAuthority {
    async fn read_chain(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<ActivationChain, CommandPortError> {
        self.current().read_chain(workspace_id).await
    }
}

#[async_trait]
impl ActivationAuthorityPort for SwitchableActivationAuthority {
    async fn revalidate(
        &self,
        request: ActivationAuthorityRequest,
    ) -> AuthorityRevalidationOutcome {
        self.current().revalidate(request).await
    }
}

#[async_trait]
impl ActivationEventPort for SwitchableActivationAuthority {
    async fn append_and_readback(
        &self,
        contract: ActivationAppendContract,
    ) -> ActivationAppendOutcome {
        self.current().append_and_readback(contract).await
    }
}

#[async_trait]
impl ActivationOperationMarkerPort for SwitchableActivationAuthority {
    async fn read_operation_marker(
        &self,
        request: ActivationOperationMarkerRequest,
    ) -> ActivationOperationMarkerOutcome {
        self.current().read_operation_marker(request).await
    }
}
