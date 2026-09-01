//! Exact Delta services bound to one admitted programmatic workspace epoch.
//!
//! This is the production composition seam joining the already implemented exact semantic read,
//! CDF checkpoint/replay, uncertain-commit reconciliation, and guarded maintenance capabilities.
//! Every operation first resolves a stable relation identity through the activation-selected
//! [`TableVersionSet`]. The service never accepts a free-standing root, discovers a latest
//! version, or lists raw table files.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use deltalake::{DeltaTable, DeltaTableBuilder, DeltaTableError};
use thiserror::Error;

use super::activation::{TableVersionSet, TableVersionSetRef};
use super::command::{EpochId, WorkspaceId};
use super::delta_cdf_checkpoint_sqlite::{
    DeltaCdfCheckpointInsert, DeltaCdfCheckpointStoreError, SqliteDeltaCdfCheckpointStore,
};
use super::delta_cdf_replay::{
    ExactDeltaCdfConsumptionOutcome, ExactDeltaCdfConsumptionRequest,
    ExactDeltaCdfCoordinatorError, ExactDeltaCdfDownstream,
};
use super::delta_commit_reconciliation::{
    UncertainDeltaCommitOutcome, UncertainDeltaCommitRequest, reconcile_uncertain_delta_commit,
};
use super::delta_exact::{DurableDeltaCdfCheckpoint, ExactDeltaPin};
use super::delta_guarded_maintenance::{
    DeltaMaintenanceError, DeltaMaintenanceOutcome, DeltaMaintenanceSafetyPort,
    GuardedDeltaMaintenance, GuardedDeltaMaintenanceRequest,
};
use super::delta_semantic_read::{
    ExactDeltaSemanticReadError, ExactDeltaSemanticReadRequest, PreparedExactDeltaSemanticRead,
    prepare_exact_delta_semantic_read,
};
use super::programmatic_epoch::ProgrammaticFabricEpoch;
use super::programmatic_schema::ProgrammaticRelationId;
use crate::schema_contract::SchemaContract;

/// Durable ports required by the exact Delta runtime.
///
/// Both are explicit production inputs. The CDF store is transport progress only; maintenance
/// admission is re-read from the supplied application authority on every request.
#[derive(Clone)]
pub struct ProgrammaticDeltaRuntimePorts {
    checkpoints: Arc<SqliteDeltaCdfCheckpointStore>,
    maintenance_authority: Arc<dyn DeltaMaintenanceSafetyPort>,
}

impl fmt::Debug for ProgrammaticDeltaRuntimePorts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgrammaticDeltaRuntimePorts")
            .field("checkpoint_database", &self.checkpoints.database_path())
            .field("maintenance_authority", &"installed")
            .finish()
    }
}

impl ProgrammaticDeltaRuntimePorts {
    #[must_use]
    pub fn new(
        checkpoints: Arc<SqliteDeltaCdfCheckpointStore>,
        maintenance_authority: Arc<dyn DeltaMaintenanceSafetyPort>,
    ) -> Self {
        Self {
            checkpoints,
            maintenance_authority,
        }
    }
}

/// One target-owned exact Delta service closure for an admitted epoch.
pub struct ProgrammaticDeltaRuntime {
    workspace_id: WorkspaceId,
    epoch_id: EpochId,
    table_versions: Arc<TableVersionSet>,
    session: Arc<datafusion::execution::SessionState>,
    contracts: BTreeMap<Arc<str>, Arc<SchemaContract>>,
    checkpoints: Arc<SqliteDeltaCdfCheckpointStore>,
    cdf: super::delta_cdf_replay::ExactDeltaCdfReplayCoordinator,
    maintenance: GuardedDeltaMaintenance,
}

impl fmt::Debug for ProgrammaticDeltaRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgrammaticDeltaRuntime")
            .field("workspace_id", &self.workspace_id)
            .field("epoch_id", &self.epoch_id)
            .field("table_version_set", &self.table_versions.reference())
            .field("relation_count", &self.table_versions.len())
            .finish_non_exhaustive()
    }
}

impl ProgrammaticDeltaRuntime {
    /// Bind exact Delta services to the same sealed epoch and reversible table vector used by
    /// query authority.
    pub(super) fn try_new(
        workspace_id: WorkspaceId,
        epoch: &Arc<ProgrammaticFabricEpoch>,
        table_versions: Arc<TableVersionSet>,
        ports: ProgrammaticDeltaRuntimePorts,
    ) -> Result<Self, ProgrammaticDeltaRuntimeError> {
        if table_versions.reference() != epoch.observation_publication().table_version_set_ref() {
            return Err(ProgrammaticDeltaRuntimeError::TableVersionSetMismatch);
        }
        let contracts = table_versions
            .components()
            .map(|(relation_id, _)| {
                let relation = ProgrammaticRelationId::new(relation_id);
                let contract = epoch
                    .observation_publication()
                    .semantic_history_read_contract(&relation)
                    .ok_or_else(|| {
                        ProgrammaticDeltaRuntimeError::MissingSchemaContract(Arc::from(relation_id))
                    })?;
                Ok((Arc::<str>::from(relation_id), Arc::clone(contract)))
            })
            .collect::<Result<BTreeMap<_, _>, ProgrammaticDeltaRuntimeError>>()?;
        let checkpoints = ports.checkpoints;
        Ok(Self {
            workspace_id,
            epoch_id: *epoch.identity(),
            table_versions,
            session: Arc::new(epoch.context().state()),
            contracts,
            cdf: super::delta_cdf_replay::ExactDeltaCdfReplayCoordinator::new(Arc::clone(
                &checkpoints,
            )),
            checkpoints,
            maintenance: GuardedDeltaMaintenance::new(ports.maintenance_authority),
        })
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn epoch_id(&self) -> EpochId {
        self.epoch_id
    }

    #[must_use]
    pub fn table_version_set_ref(&self) -> TableVersionSetRef {
        self.table_versions.reference()
    }

    /// Prepare a residual-correct semantic read against the relation's activation-selected pin.
    pub async fn prepare_semantic_read(
        &self,
        relation_id: &str,
        request: ExactDeltaSemanticReadRequest,
    ) -> Result<PreparedExactDeltaSemanticRead, ProgrammaticDeltaRuntimeError> {
        let selected = self.require_selected(relation_id, request.pin())?;
        let contract = self.contracts.get(relation_id).cloned().ok_or_else(|| {
            ProgrammaticDeltaRuntimeError::MissingSchemaContract(Arc::from(relation_id))
        })?;
        let table = load_exact(selected).await?;
        Ok(
            prepare_exact_delta_semantic_read(table, request, contract, Arc::clone(&self.session))
                .await?,
        )
    }

    /// Install one explicit durable transport checkpoint before the first CDF replay.
    pub async fn insert_cdf_checkpoint(
        &self,
        relation_id: &str,
        checkpoint: DurableDeltaCdfCheckpoint,
    ) -> Result<DeltaCdfCheckpointInsert, ProgrammaticDeltaRuntimeError> {
        self.require_selected_root(relation_id, checkpoint.consumed_through())?;
        Ok(self.checkpoints.insert_if_absent(checkpoint).await?)
    }

    /// Consume CDF through the activation-selected exact relation version.
    pub async fn consume_cdf<D: ExactDeltaCdfDownstream + ?Sized>(
        &self,
        relation_id: &str,
        request: &ExactDeltaCdfConsumptionRequest,
        downstream: &D,
    ) -> Result<ExactDeltaCdfConsumptionOutcome, ProgrammaticDeltaRuntimeError> {
        let selected = self.require_selected(relation_id, request.through_pin())?;
        let table = load_exact(selected).await?;
        Ok(self
            .cdf
            .consume(request, &table, Arc::clone(&self.session), downstream)
            .await?)
    }

    /// Reconcile one uncertain write whose predecessor was the selected relation state.
    pub async fn reconcile_uncertain_commit(
        &self,
        relation_id: &str,
        request: &UncertainDeltaCommitRequest,
    ) -> Result<UncertainDeltaCommitOutcome, ProgrammaticDeltaRuntimeError> {
        let selected = self.require_selected(relation_id, request.write().predecessor())?;
        let authority = load_exact(selected).await?;
        Ok(reconcile_uncertain_delta_commit(&authority, request).await)
    }

    /// Execute guarded native maintenance against an activation-selected exact relation state.
    pub async fn maintain(
        &self,
        relation_id: &str,
        request: &GuardedDeltaMaintenanceRequest,
    ) -> Result<DeltaMaintenanceOutcome, ProgrammaticDeltaRuntimeError> {
        let selected = self.require_selected(relation_id, request.target())?;
        let table = load_exact(selected).await?;
        Ok(self.maintenance.execute(request, table).await?)
    }

    fn require_selected<'a>(
        &'a self,
        relation_id: &str,
        requested: &ExactDeltaPin,
    ) -> Result<&'a ExactDeltaPin, ProgrammaticDeltaRuntimeError> {
        let selected = self.table_versions.pin(relation_id).ok_or_else(|| {
            ProgrammaticDeltaRuntimeError::UnknownRelation(Arc::from(relation_id))
        })?;
        if selected != requested {
            return Err(ProgrammaticDeltaRuntimeError::ExactPinMismatch {
                relation_id: Arc::from(relation_id),
                selected: selected.clone(),
                requested: requested.clone(),
            });
        }
        Ok(selected)
    }

    fn require_selected_root(
        &self,
        relation_id: &str,
        requested: &ExactDeltaPin,
    ) -> Result<(), ProgrammaticDeltaRuntimeError> {
        let selected = self.table_versions.pin(relation_id).ok_or_else(|| {
            ProgrammaticDeltaRuntimeError::UnknownRelation(Arc::from(relation_id))
        })?;
        if selected.canonical_root() != requested.canonical_root()
            || requested.version() > selected.version()
        {
            return Err(
                ProgrammaticDeltaRuntimeError::CheckpointOutsideSelectedHistory {
                    relation_id: Arc::from(relation_id),
                    selected: selected.clone(),
                    checkpoint: requested.clone(),
                },
            );
        }
        Ok(())
    }
}

async fn load_exact(pin: &ExactDeltaPin) -> Result<DeltaTable, DeltaTableError> {
    DeltaTableBuilder::from_url(pin.canonical_root().clone())?
        .with_version(pin.version())
        .load()
        .await
}

/// Fail-closed exact Delta runtime errors.
#[derive(Debug, Error)]
pub enum ProgrammaticDeltaRuntimeError {
    #[error("programmatic Delta runtime table vector differs from the sealed epoch")]
    TableVersionSetMismatch,
    #[error("relation {0} is absent from the activation-selected table vector")]
    UnknownRelation(Arc<str>),
    #[error("relation {0} has no executable schema contract in the sealed epoch")]
    MissingSchemaContract(Arc<str>),
    #[error("relation {relation_id} requested {requested:?}, but activation selected {selected:?}")]
    ExactPinMismatch {
        relation_id: Arc<str>,
        selected: ExactDeltaPin,
        requested: ExactDeltaPin,
    },
    #[error(
        "relation {relation_id} checkpoint {checkpoint:?} is outside selected history through {selected:?}"
    )]
    CheckpointOutsideSelectedHistory {
        relation_id: Arc<str>,
        selected: ExactDeltaPin,
        checkpoint: ExactDeltaPin,
    },
    #[error(transparent)]
    Delta(#[from] DeltaTableError),
    #[error(transparent)]
    SemanticRead(#[from] ExactDeltaSemanticReadError),
    #[error(transparent)]
    CdfCheckpoint(#[from] DeltaCdfCheckpointStoreError),
    #[error(transparent)]
    Cdf(#[from] ExactDeltaCdfCoordinatorError),
    #[error(transparent)]
    Maintenance(#[from] DeltaMaintenanceError),
}
