//! Admission barrier, epoch pinning, and atomic process-local epoch swap.
//!
//! Durable activation history remains authoritative in [`super::activation`].
//! This module is only the daemon's reconciled cache and concurrency boundary:
//! it closes admission before durable selection, swaps one already-sealed
//! [`ProgrammaticFabricEpoch`], and reopens only after the selected chain head and cache
//! agree.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use arc_swap::ArcSwapOption;

use super::activation::{ActivationChain, ActivationEvent, ActivationEventId};
use super::command::{EpochId, ExpectedHead, WorkspaceId, WriterFence};
use super::programmatic_epoch::ProgrammaticFabricEpoch;

static NEXT_RUNTIME_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// Opaque proof that one runtime instance has closed new-query admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationBarrier {
    runtime_instance: u64,
    barrier_id: u64,
    expected_head: ExpectedHead,
    authority_fence: WriterFence,
}

impl ActivationBarrier {
    #[must_use]
    pub const fn expected_head(self) -> ExpectedHead {
        self.expected_head
    }

    #[must_use]
    pub const fn authority_fence(self) -> WriterFence {
        self.authority_fence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmissionPhase {
    Open,
    Recovering {
        durable_head: ExpectedHead,
    },
    Closed {
        barrier: ActivationBarrier,
    },
    Swapped {
        barrier: ActivationBarrier,
        selected_event: ActivationEventId,
        selected_epoch: EpochId,
    },
}

/// Process-local publication state after durable recovery selected an epoch.
/// `PublishedClosed` requires cache reconciliation before reopening;
/// `AlreadyReopened` is accepted only for recovery of a transaction that had
/// already crossed the reopen boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoverySelectionPublication {
    PublishedClosed,
    AlreadyReopened,
}

#[derive(Debug)]
struct AdmissionState {
    phase: AdmissionPhase,
    admission_generation: u64,
    next_barrier_id: u64,
}

/// One admitted query's immutable epoch pin. Dropping the lease releases the
/// predecessor naturally when no query/result resource retains it.
#[derive(Clone, Debug)]
pub struct FabricQueryLease {
    epoch: Arc<ProgrammaticFabricEpoch>,
    admission_generation: u64,
}

impl FabricQueryLease {
    #[must_use]
    pub const fn epoch(&self) -> &Arc<ProgrammaticFabricEpoch> {
        &self.epoch
    }

    #[must_use]
    pub fn epoch_id(&self) -> EpochId {
        *self.epoch.identity()
    }

    #[must_use]
    pub const fn admission_generation(&self) -> u64 {
        self.admission_generation
    }
}

/// Process-local serving handle reconciled from an append-only activation
/// chain. The mutex serializes admission with close/swap/reopen; `ArcSwap`
/// makes the active epoch load atomic while leases retain prior epochs.
pub struct FabricAdmissionRuntime {
    workspace_id: WorkspaceId,
    runtime_instance: u64,
    active: ArcSwapOption<ProgrammaticFabricEpoch>,
    state: Mutex<AdmissionState>,
}

impl std::fmt::Debug for FabricAdmissionRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let active = self.active.load_full().map(|epoch| *epoch.identity());
        formatter
            .debug_struct("FabricAdmissionRuntime")
            .field("workspace_id", &self.workspace_id)
            .field("runtime_instance", &self.runtime_instance)
            .field("active_epoch", &active)
            .finish_non_exhaustive()
    }
}

impl FabricAdmissionRuntime {
    /// Reconstruct the process-local cache from the validated durable chain.
    ///
    /// # Errors
    ///
    /// Fails when the resolver cannot reconstruct the exact selected sealed
    /// epoch or returns an epoch with a different canonical identity.
    #[cfg(test)]
    pub fn recover(
        chain: &ActivationChain,
        resolver: impl FnOnce(EpochId) -> Option<Arc<ProgrammaticFabricEpoch>>,
    ) -> Result<Self, AdmissionError> {
        Self::recover_with_posture(chain, resolver, false)
    }

    /// Reconstruct the process-local epoch cache while keeping admission
    /// closed until durable operation-marker and temporal-cache recovery has
    /// completed. This is the production restart entry point for an
    /// interrupted activation.
    ///
    /// # Errors
    ///
    /// Fails when the exact durable head cannot be reconstructed.
    pub fn recover_for_reconciliation(
        chain: &ActivationChain,
        resolver: impl FnOnce(EpochId) -> Option<Arc<ProgrammaticFabricEpoch>>,
    ) -> Result<Self, AdmissionError> {
        Self::recover_with_posture(chain, resolver, true)
    }

    /// Start restart reconciliation with no process-local epoch cache.
    ///
    /// The durable chain fixes the selected identity while query admission
    /// remains closed. The marker-driven recovery coordinator must rebuild the
    /// exact selected epoch from its durable relation-version vector before
    /// this runtime can publish or reopen it.
    ///
    /// # Errors
    ///
    /// This constructor currently has no fallible local operation; the result
    /// preserves the recovery API's fail-closed construction boundary.
    pub fn recover_unmaterialized_for_reconciliation(
        chain: &ActivationChain,
    ) -> Result<Self, AdmissionError> {
        Ok(Self {
            workspace_id: chain.workspace_id(),
            runtime_instance: NEXT_RUNTIME_INSTANCE.fetch_add(1, Ordering::Relaxed),
            active: ArcSwapOption::empty(),
            state: Mutex::new(AdmissionState {
                phase: AdmissionPhase::Recovering {
                    durable_head: chain.current_head(),
                },
                admission_generation: 1,
                next_barrier_id: 1,
            }),
        })
    }

    fn recover_with_posture(
        chain: &ActivationChain,
        resolver: impl FnOnce(EpochId) -> Option<Arc<ProgrammaticFabricEpoch>>,
        recovery_closed: bool,
    ) -> Result<Self, AdmissionError> {
        let active = match chain.current_head() {
            ExpectedHead::Empty => None,
            ExpectedHead::Epoch(epoch_id) => {
                let epoch =
                    resolver(epoch_id).ok_or(AdmissionError::SelectedEpochUnavailable(epoch_id))?;
                if *epoch.identity() != epoch_id {
                    return Err(AdmissionError::ResolvedEpochIdentityMismatch {
                        selected: epoch_id,
                        resolved: *epoch.identity(),
                    });
                }
                Some(epoch)
            }
        };
        let phase = if recovery_closed {
            AdmissionPhase::Recovering {
                durable_head: chain.current_head(),
            }
        } else {
            AdmissionPhase::Open
        };
        Ok(Self {
            workspace_id: chain.workspace_id(),
            runtime_instance: NEXT_RUNTIME_INSTANCE.fetch_add(1, Ordering::Relaxed),
            active: ArcSwapOption::from(active),
            state: Mutex::new(AdmissionState {
                phase,
                admission_generation: 1,
                next_barrier_id: 1,
            }),
        })
    }

    /// Admit a query only while the gate is open and atomically pin the one
    /// active epoch under the same lock used by activation closure.
    ///
    /// # Errors
    ///
    /// Returns an explicit closed/no-head/poisoned error rather than falling
    /// back to a predecessor or discovering a later epoch.
    pub fn admit(&self) -> Result<FabricQueryLease, AdmissionError> {
        let state = self.lock_state()?;
        if state.phase != AdmissionPhase::Open {
            return Err(AdmissionError::AdmissionClosed);
        }
        let epoch = self
            .active
            .load_full()
            .ok_or(AdmissionError::NoActiveEpoch)?;
        Ok(FabricQueryLease {
            epoch,
            admission_generation: state.admission_generation,
        })
    }

    /// Close new-query admission against the exact active predecessor and
    /// writer fence before a durable activation append is attempted.
    ///
    /// # Errors
    ///
    /// Rejects an already closed gate or stale expected head.
    pub fn close_admission(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
    ) -> Result<ActivationBarrier, AdmissionError> {
        let mut state = self.lock_state()?;
        if state.phase != AdmissionPhase::Open {
            return Err(AdmissionError::AdmissionAlreadyClosed);
        }
        let actual = self.active_head();
        if actual != expected_head {
            return Err(AdmissionError::StalePredecessor {
                expected: expected_head,
                actual,
            });
        }
        let barrier = ActivationBarrier {
            runtime_instance: self.runtime_instance,
            barrier_id: state.next_barrier_id,
            expected_head,
            authority_fence: execution_fence,
        };
        state.next_barrier_id =
            state
                .next_barrier_id
                .checked_add(1)
                .ok_or(AdmissionError::InternalInvariant(
                    "activation barrier sequence exhausted",
                ))?;
        state.phase = AdmissionPhase::Closed { barrier };
        Ok(barrier)
    }

    /// After the activation event has been appended and read back, validate
    /// the new durable chain and atomically swap the exact selected epoch while
    /// admission remains closed.
    ///
    /// # Errors
    ///
    /// Rejects a foreign/stale barrier, chain mismatch, wrong predecessor or
    /// fence, unresolved candidate identity, or an unexpected process cache.
    pub fn publish_selected_epoch(
        &self,
        barrier: ActivationBarrier,
        chain_after_readback: &ActivationChain,
        candidate: Arc<ProgrammaticFabricEpoch>,
    ) -> Result<(), AdmissionError> {
        let mut state = self.lock_state()?;
        self.require_closed_barrier(&state, barrier)?;
        if chain_after_readback.workspace_id() != self.workspace_id {
            return Err(AdmissionError::WorkspaceMismatch);
        }
        if self.active_head() != barrier.expected_head {
            return Err(AdmissionError::ProcessCacheChangedWhileClosed);
        }
        let head = chain_after_readback
            .head_event()
            .ok_or(AdmissionError::MissingSelectedEvent)?;
        if head.predecessor_epoch() != barrier.expected_head {
            return Err(AdmissionError::SelectedEventPredecessorMismatch);
        }
        if head.execution_fence() != barrier.authority_fence {
            return Err(AdmissionError::SelectedEventFenceMismatch);
        }
        let selected_epoch = head.pins().epoch;
        if chain_after_readback.current_head() != ExpectedHead::Epoch(selected_epoch)
            || *candidate.identity() != selected_epoch
        {
            return Err(AdmissionError::SelectedCandidateMismatch {
                selected: selected_epoch,
                candidate: *candidate.identity(),
            });
        }
        self.active.store(Some(candidate));
        state.phase = AdmissionPhase::Swapped {
            barrier,
            selected_event: head.event_id(),
            selected_epoch,
        };
        Ok(())
    }

    /// Reopen only after the reconstructible receipt observer agrees with the
    /// swapped durable selection.
    ///
    /// # Errors
    ///
    /// Rejects reopening before swap, a stale barrier, or cache disagreement.
    pub fn reopen_after_reconciliation(
        &self,
        barrier: ActivationBarrier,
        reconciled_head: ExpectedHead,
    ) -> Result<(), AdmissionError> {
        let mut state = self.lock_state()?;
        let selected = match state.phase {
            AdmissionPhase::Swapped {
                barrier: active,
                selected_epoch,
                ..
            } if active == barrier => selected_epoch,
            AdmissionPhase::Swapped { .. } => return Err(AdmissionError::StaleBarrier),
            AdmissionPhase::Open
            | AdmissionPhase::Recovering { .. }
            | AdmissionPhase::Closed { .. } => {
                return Err(AdmissionError::SelectionNotPublished);
            }
        };
        if reconciled_head != ExpectedHead::Epoch(selected)
            || self.active_head() != ExpectedHead::Epoch(selected)
        {
            return Err(AdmissionError::ReconciliationMismatch {
                selected,
                reconciled: reconciled_head,
                active: self.active_head(),
            });
        }
        state.admission_generation =
            state
                .admission_generation
                .checked_add(1)
                .ok_or(AdmissionError::InternalInvariant(
                    "admission generation exhausted",
                ))?;
        state.phase = AdmissionPhase::Open;
        Ok(())
    }

    /// Abort a closure only when durable history proves no activation event was
    /// selected. Once a selection exists, recovery must publish it.
    ///
    /// # Errors
    ///
    /// Rejects a stale barrier, a post-swap abort, or durable head movement.
    pub fn abort_before_selection(
        &self,
        barrier: ActivationBarrier,
        durable_chain: &ActivationChain,
    ) -> Result<(), AdmissionError> {
        let mut state = self.lock_state()?;
        self.require_closed_barrier(&state, barrier)?;
        if durable_chain.workspace_id() != self.workspace_id
            || durable_chain.current_head() != barrier.expected_head
            || self.active_head() != barrier.expected_head
        {
            return Err(AdmissionError::CannotAbortAfterSelection);
        }
        state.phase = AdmissionPhase::Open;
        Ok(())
    }

    /// Reconcile a durable selected event into this process without ever
    /// publishing a second epoch or reopening before cache reconciliation.
    ///
    /// # Errors
    ///
    /// Rejects chain/event/candidate drift, an incompatible recovery posture,
    /// or any process cache that cannot be causally explained by the supplied
    /// durable chain.
    #[allow(clippy::too_many_arguments)]
    pub fn recover_selected_epoch(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
        active_recovery_fence: WriterFence,
        event: ActivationEvent,
        chain_after_readback: &ActivationChain,
        candidate: Arc<ProgrammaticFabricEpoch>,
        allow_already_reopened: bool,
    ) -> Result<RecoverySelectionPublication, AdmissionError> {
        Self::require_recovery_fence(execution_fence, active_recovery_fence)?;
        if chain_after_readback.workspace_id() != self.workspace_id {
            return Err(AdmissionError::WorkspaceMismatch);
        }
        if chain_after_readback.head_event().copied() != Some(event) {
            return Err(AdmissionError::RecoveryEventIsNotHead);
        }
        if event.predecessor_epoch() != expected_head {
            return Err(AdmissionError::SelectedEventPredecessorMismatch);
        }
        if event.execution_fence() != execution_fence {
            return Err(AdmissionError::SelectedEventFenceMismatch);
        }
        let selected_epoch = event.pins().epoch;
        if chain_after_readback.current_head() != ExpectedHead::Epoch(selected_epoch)
            || *candidate.identity() != selected_epoch
        {
            return Err(AdmissionError::SelectedCandidateMismatch {
                selected: selected_epoch,
                candidate: *candidate.identity(),
            });
        }

        let mut state = self.lock_state()?;
        match state.phase {
            AdmissionPhase::Recovering { durable_head } => {
                let active = self.active_head();
                if durable_head != ExpectedHead::Epoch(selected_epoch)
                    || (active != ExpectedHead::Empty
                        && active != ExpectedHead::Epoch(selected_epoch))
                {
                    return Err(AdmissionError::RecoveryHeadMismatch {
                        durable: ExpectedHead::Epoch(selected_epoch),
                        active,
                    });
                }
                if active == ExpectedHead::Empty {
                    self.active.store(Some(candidate));
                }
                let barrier =
                    self.next_recovery_barrier(&mut state, expected_head, active_recovery_fence)?;
                state.phase = AdmissionPhase::Swapped {
                    barrier,
                    selected_event: event.event_id(),
                    selected_epoch,
                };
                Ok(RecoverySelectionPublication::PublishedClosed)
            }
            AdmissionPhase::Closed { barrier } => {
                if barrier.expected_head != expected_head
                    || barrier.authority_fence != execution_fence
                {
                    return Err(AdmissionError::StaleBarrier);
                }
                if self.active_head() != expected_head {
                    return Err(AdmissionError::ProcessCacheChangedWhileClosed);
                }
                self.active.store(Some(candidate));
                let recovery_barrier =
                    self.next_recovery_barrier(&mut state, expected_head, active_recovery_fence)?;
                state.phase = AdmissionPhase::Swapped {
                    barrier: recovery_barrier,
                    selected_event: event.event_id(),
                    selected_epoch,
                };
                Ok(RecoverySelectionPublication::PublishedClosed)
            }
            AdmissionPhase::Swapped {
                barrier,
                selected_event,
                selected_epoch: active_epoch,
            } => {
                if selected_event != event.event_id()
                    || active_epoch != selected_epoch
                    || self.active_head() != ExpectedHead::Epoch(selected_epoch)
                    || !Self::recovery_fence_authorizes(
                        barrier.authority_fence,
                        active_recovery_fence,
                    )
                {
                    return Err(AdmissionError::RecoveryPublishedSelectionMismatch);
                }
                Ok(RecoverySelectionPublication::PublishedClosed)
            }
            AdmissionPhase::Open
                if allow_already_reopened
                    && self.active_head() == ExpectedHead::Epoch(selected_epoch) =>
            {
                Ok(RecoverySelectionPublication::AlreadyReopened)
            }
            AdmissionPhase::Open => Err(AdmissionError::RecoveryAdmissionUnexpectedlyOpen),
        }
    }

    /// Reopen a marker-recovered selected epoch only after its reconstructible
    /// cache has been reconciled.
    ///
    /// # Errors
    ///
    /// Rejects an event/head mismatch or a process phase other than the exact
    /// recovered selection.
    pub fn reopen_recovered_selection(
        &self,
        event: ActivationEvent,
        chain_after_readback: &ActivationChain,
        active_recovery_fence: WriterFence,
    ) -> Result<(), AdmissionError> {
        Self::require_recovery_fence(event.execution_fence(), active_recovery_fence)?;
        if chain_after_readback.workspace_id() != self.workspace_id
            || chain_after_readback.head_event().copied() != Some(event)
        {
            return Err(AdmissionError::RecoveryEventIsNotHead);
        }
        let selected_epoch = event.pins().epoch;
        let mut state = self.lock_state()?;
        match state.phase {
            AdmissionPhase::Swapped {
                barrier,
                selected_event,
                selected_epoch: active_epoch,
            } if selected_event == event.event_id()
                && active_epoch == selected_epoch
                && self.active_head() == ExpectedHead::Epoch(selected_epoch)
                && Self::recovery_fence_authorizes(
                    barrier.authority_fence,
                    active_recovery_fence,
                ) =>
            {
                Self::advance_admission_generation(&mut state)?;
                state.phase = AdmissionPhase::Open;
                Ok(())
            }
            AdmissionPhase::Open if self.active_head() == ExpectedHead::Epoch(selected_epoch) => {
                Ok(())
            }
            _ => Err(AdmissionError::RecoveryPublishedSelectionMismatch),
        }
    }

    /// Reopen after durable marker readback proves that no selection occurred.
    ///
    /// # Errors
    ///
    /// Rejects head movement, a swapped selection, or a closure belonging to a
    /// different predecessor/fence.
    pub fn recover_proved_no_selection(
        &self,
        expected_head: ExpectedHead,
        execution_fence: WriterFence,
        active_recovery_fence: WriterFence,
        unchanged_chain: &ActivationChain,
    ) -> Result<(), AdmissionError> {
        Self::require_recovery_fence(execution_fence, active_recovery_fence)?;
        if unchanged_chain.workspace_id() != self.workspace_id
            || unchanged_chain.current_head() != expected_head
        {
            return Err(AdmissionError::CannotAbortAfterSelection);
        }
        let mut state = self.lock_state()?;
        match state.phase {
            AdmissionPhase::Recovering { durable_head }
                if durable_head == expected_head && self.active_head() == expected_head =>
            {
                Self::advance_admission_generation(&mut state)?;
                state.phase = AdmissionPhase::Open;
                Ok(())
            }
            AdmissionPhase::Closed { barrier }
                if barrier.expected_head == expected_head
                    && barrier.authority_fence == execution_fence
                    && self.active_head() == expected_head =>
            {
                Self::advance_admission_generation(&mut state)?;
                state.phase = AdmissionPhase::Open;
                Ok(())
            }
            AdmissionPhase::Open if self.active_head() == expected_head => Ok(()),
            _ => Err(AdmissionError::CannotAbortAfterSelection),
        }
    }

    #[must_use]
    pub fn active_head(&self) -> ExpectedHead {
        self.active
            .load_full()
            .map_or(ExpectedHead::Empty, |epoch| {
                ExpectedHead::Epoch(*epoch.identity())
            })
    }

    fn require_closed_barrier(
        &self,
        state: &AdmissionState,
        barrier: ActivationBarrier,
    ) -> Result<(), AdmissionError> {
        if barrier.runtime_instance != self.runtime_instance {
            return Err(AdmissionError::ForeignBarrier);
        }
        match state.phase {
            AdmissionPhase::Closed { barrier: active } if active == barrier => Ok(()),
            AdmissionPhase::Closed { .. } => Err(AdmissionError::StaleBarrier),
            AdmissionPhase::Open | AdmissionPhase::Recovering { .. } => {
                Err(AdmissionError::AdmissionNotClosed)
            }
            AdmissionPhase::Swapped { .. } => Err(AdmissionError::SelectionAlreadyPublished),
        }
    }

    fn next_recovery_barrier(
        &self,
        state: &mut AdmissionState,
        expected_head: ExpectedHead,
        authority_fence: WriterFence,
    ) -> Result<ActivationBarrier, AdmissionError> {
        let barrier = ActivationBarrier {
            runtime_instance: self.runtime_instance,
            barrier_id: state.next_barrier_id,
            expected_head,
            authority_fence,
        };
        state.next_barrier_id =
            state
                .next_barrier_id
                .checked_add(1)
                .ok_or(AdmissionError::InternalInvariant(
                    "activation barrier sequence exhausted",
                ))?;
        Ok(barrier)
    }

    fn advance_admission_generation(state: &mut AdmissionState) -> Result<(), AdmissionError> {
        state.admission_generation =
            state
                .admission_generation
                .checked_add(1)
                .ok_or(AdmissionError::InternalInvariant(
                    "admission generation exhausted",
                ))?;
        Ok(())
    }

    fn recovery_fence_authorizes(execution: WriterFence, active: WriterFence) -> bool {
        active == execution || active.generation.get() > execution.generation.get()
    }

    fn require_recovery_fence(
        execution: WriterFence,
        active: WriterFence,
    ) -> Result<(), AdmissionError> {
        if Self::recovery_fence_authorizes(execution, active) {
            Ok(())
        } else {
            Err(AdmissionError::RecoveryFenceNotAuthorized { execution, active })
        }
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, AdmissionState>, AdmissionError> {
        self.state.lock().map_err(|_| AdmissionError::StatePoisoned)
    }
}

/// Fail-closed admission, swap, and recovery errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AdmissionError {
    #[error("selected epoch {0:?} cannot be reconstructed")]
    SelectedEpochUnavailable(EpochId),
    #[error("resolver returned {resolved:?} for selected epoch {selected:?}")]
    ResolvedEpochIdentityMismatch {
        selected: EpochId,
        resolved: EpochId,
    },
    #[error("new-query admission is closed")]
    AdmissionClosed,
    #[error("no fabric epoch is active")]
    NoActiveEpoch,
    #[error("new-query admission is already closed")]
    AdmissionAlreadyClosed,
    #[error("expected predecessor {expected:?}, active process cache {actual:?}")]
    StalePredecessor {
        expected: ExpectedHead,
        actual: ExpectedHead,
    },
    #[error("activation barrier belongs to another runtime instance")]
    ForeignBarrier,
    #[error("activation barrier is stale")]
    StaleBarrier,
    #[error("admission was not closed")]
    AdmissionNotClosed,
    #[error("the selected epoch was already published")]
    SelectionAlreadyPublished,
    #[error("durable activation chain belongs to another workspace")]
    WorkspaceMismatch,
    #[error("process epoch cache changed while admission was closed")]
    ProcessCacheChangedWhileClosed,
    #[error("durable chain has no selected activation event")]
    MissingSelectedEvent,
    #[error("selected activation event does not extend the closed predecessor")]
    SelectedEventPredecessorMismatch,
    #[error("selected activation event does not use the closed writer fence")]
    SelectedEventFenceMismatch,
    #[error("selected epoch {selected:?} differs from candidate {candidate:?}")]
    SelectedCandidateMismatch {
        selected: EpochId,
        candidate: EpochId,
    },
    #[error("selected epoch has not been published")]
    SelectionNotPublished,
    #[error(
        "reconciliation differs: selected {selected:?}, reconciled {reconciled:?}, active {active:?}"
    )]
    ReconciliationMismatch {
        selected: EpochId,
        reconciled: ExpectedHead,
        active: ExpectedHead,
    },
    #[error("durable history moved; closure cannot be aborted")]
    CannotAbortAfterSelection,
    #[error("marker-recovered event is not the unique durable head")]
    RecoveryEventIsNotHead,
    #[error("recovered durable head {durable:?} differs from process cache {active:?}")]
    RecoveryHeadMismatch {
        durable: ExpectedHead,
        active: ExpectedHead,
    },
    #[error("process selection differs from the marker-recovered event")]
    RecoveryPublishedSelectionMismatch,
    #[error("admission is open before marker/cache recovery authorizes reopening")]
    RecoveryAdmissionUnexpectedlyOpen,
    #[error(
        "active recovery fence {active:?} does not authorize convergence of execution fence {execution:?}"
    )]
    RecoveryFenceNotAuthorized {
        execution: WriterFence,
        active: WriterFence,
    },
    #[error("admission mutex is poisoned")]
    StatePoisoned,
    #[error("internal admission invariant failed: {0}")]
    InternalInvariant(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::activation::{
        ActivationAttempt, ActivationCommit, ActivationEvent, ActivationEventId, ActivationOrdinal,
        ActivationReadbackRef, BackendCommitRef, CompatibilityClassRef, FabricEpochPins,
        OverlaySegmentSetRef, PolicySetRef, TableVersionSetRef,
    };
    use crate::fabric::command::{
        ActorId, AuthorizationRef, CommandIdentity, CommandOwnership, CommandPins,
        CompilerReleaseRef, ExecutionOwner, FabricCommand, FabricCommandPayload, IdempotencyKey,
        LeaseId, ModelHeadRef, OperationId, OperationSelectionRef, PrincipalId, ProofReceiptRef,
        ProviderSetRef, ResourceEnvelopeRef, RetentionPolicyRef, SourceGeneration, TransactionRef,
        WriterGeneration,
    };
    use crate::fabric::epoch::FabricEpochRuntimeConfig;
    use crate::fabric::programmatic_epoch::{
        ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder,
    };

    const fn id16(seed: u8) -> [u8; 16] {
        [seed; 16]
    }

    const fn id32(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn command(
        operation_seed: u8,
        workspace: WorkspaceId,
        predecessor: ExpectedHead,
        target: EpochId,
        generation: u64,
    ) -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(id16(operation_seed)),
                idempotency_key: IdempotencyKey::from_bytes(id32(operation_seed)),
            },
            ownership: CommandOwnership {
                workspace_id: workspace,
                principal_id: PrincipalId::from_bytes(id16(2)),
                authorization: AuthorizationRef::from_bytes(id32(3)),
            },
            expected_head: predecessor,
            writer_fence: WriterFence {
                lease_id: LeaseId::from_bytes(id16(4)),
                generation: WriterGeneration::new(generation).unwrap(),
            },
            pins: CommandPins {
                compiler_release: CompilerReleaseRef::from_bytes(id32(5)),
                model_head: ModelHeadRef::from_bytes(id32(6)),
                source_generation: SourceGeneration::new(7),
                provider_set: ProviderSetRef::from_bytes(id32(8)),
            },
            resources: ResourceEnvelopeRef::from_bytes(id32(9)),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: target,
                proof_receipt: ProofReceiptRef::from_bytes(id32(10)),
            },
        }
    }

    fn activation_event(
        event_seed: u8,
        command: &FabricCommand,
        predecessor_event_id: Option<ActivationEventId>,
        ordinal: u64,
        target: EpochId,
    ) -> ActivationEvent {
        ActivationEvent::try_from_attempt(
            ActivationEventId::from_bytes(id32(event_seed)),
            ActivationAttempt::for_test(
                *command,
                1,
                ExecutionOwner {
                    actor_id: ActorId::from_bytes(id16(33)),
                    fence: command.writer_fence,
                },
            ),
            predecessor_event_id,
            ActivationOrdinal::new(ordinal).unwrap(),
            FabricEpochPins {
                epoch: target,
                compiler_release: command.pins.compiler_release,
                model_head: command.pins.model_head,
                source_generation: command.pins.source_generation,
                provider_set: command.pins.provider_set,
                table_versions: TableVersionSetRef::from_bytes(id32(11)),
                overlay_segments: OverlaySegmentSetRef::from_bytes(id32(12)),
                policy_set: PolicySetRef::from_bytes(id32(13)),
                resource_envelope: command.resources,
                proof_receipt: ProofReceiptRef::from_bytes(id32(10)),
            },
            CompatibilityClassRef::from_bytes(id32(14)),
            RetentionPolicyRef::from_bytes(id32(15)),
            ActivationCommit {
                operation_selection: OperationSelectionRef::from_bytes(id32(event_seed + 30)),
                transaction: TransactionRef::from_bytes(id32(event_seed + 60)),
                backend_commit: BackendCommitRef::from_bytes(id32(event_seed + 90)),
                readback: ActivationReadbackRef::from_bytes(id32(event_seed + 120)),
            },
        )
        .unwrap()
    }

    async fn epoch(epoch_id: EpochId) -> Arc<ProgrammaticFabricEpoch> {
        let config = FabricEpochRuntimeConfig::default();
        Arc::new(
            ProgrammaticFabricEpochBuilder::try_new(epoch_id, config)
                .unwrap()
                .seal_for_test()
                .await
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn leases_pin_predecessor_across_closed_atomic_swap() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let first_id = EpochId::from_bytes(id16(20));
        let second_id = EpochId::from_bytes(id16(21));
        let first = epoch(first_id).await;
        let second = epoch(second_id).await;
        let first_command = command(1, workspace, ExpectedHead::Empty, first_id, 1);
        let first_event = activation_event(1, &first_command, None, 1, first_id);
        let first_chain = ActivationChain::derive(workspace, [first_event]).unwrap();
        let runtime =
            FabricAdmissionRuntime::recover(&first_chain, |_| Some(Arc::clone(&first))).unwrap();
        let predecessor_lease = runtime.admit().unwrap();

        let second_command = command(2, workspace, ExpectedHead::Epoch(first_id), second_id, 1);
        let barrier = runtime
            .close_admission(second_command.expected_head, second_command.writer_fence)
            .unwrap();
        assert_eq!(
            runtime.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );
        let second_event = activation_event(
            2,
            &second_command,
            Some(first_event.event_id()),
            2,
            second_id,
        );
        let second_chain = ActivationChain::derive(workspace, [second_event, first_event]).unwrap();
        runtime
            .publish_selected_epoch(barrier, &second_chain, Arc::clone(&second))
            .unwrap();
        assert_eq!(predecessor_lease.epoch_id(), first_id);
        assert_eq!(
            runtime.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );
        runtime
            .reopen_after_reconciliation(barrier, ExpectedHead::Epoch(second_id))
            .unwrap();
        assert_eq!(runtime.admit().unwrap().epoch_id(), second_id);
        assert_eq!(predecessor_lease.epoch_id(), first_id);
    }

    #[tokio::test]
    async fn failed_preselection_can_abort_but_selected_history_cannot() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let first_id = EpochId::from_bytes(id16(20));
        let first = epoch(first_id).await;
        let first_command = command(1, workspace, ExpectedHead::Empty, first_id, 1);
        let first_event = activation_event(1, &first_command, None, 1, first_id);
        let chain = ActivationChain::derive(workspace, [first_event]).unwrap();
        let runtime = FabricAdmissionRuntime::recover(&chain, |_| Some(first)).unwrap();
        let barrier = runtime
            .close_admission(
                ExpectedHead::Epoch(first_id),
                WriterFence {
                    lease_id: LeaseId::from_bytes(id16(4)),
                    generation: WriterGeneration::new(1).unwrap(),
                },
            )
            .unwrap();
        runtime.abort_before_selection(barrier, &chain).unwrap();
        assert_eq!(runtime.admit().unwrap().epoch_id(), first_id);
    }

    #[tokio::test]
    async fn recovery_requires_the_exact_durable_head() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let selected = EpochId::from_bytes(id16(20));
        let wrong = EpochId::from_bytes(id16(21));
        let command = command(1, workspace, ExpectedHead::Empty, selected, 1);
        let event = activation_event(1, &command, None, 1, selected);
        let chain = ActivationChain::derive(workspace, [event]).unwrap();
        let wrong_epoch = epoch(wrong).await;
        assert_eq!(
            FabricAdmissionRuntime::recover(&chain, |_| Some(wrong_epoch)).unwrap_err(),
            AdmissionError::ResolvedEpochIdentityMismatch {
                selected,
                resolved: wrong,
            }
        );
        assert_eq!(
            FabricAdmissionRuntime::recover(&chain, |_| None).unwrap_err(),
            AdmissionError::SelectedEpochUnavailable(selected)
        );
    }

    #[tokio::test]
    async fn restart_recovery_keeps_admission_closed_until_marker_and_cache_reconciliation() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let selected = EpochId::from_bytes(id16(20));
        let selected_epoch = epoch(selected).await;
        let command = command(1, workspace, ExpectedHead::Empty, selected, 1);
        let event = activation_event(1, &command, None, 1, selected);
        let chain = ActivationChain::derive(workspace, [event]).unwrap();
        let runtime = FabricAdmissionRuntime::recover_for_reconciliation(&chain, |_| {
            Some(Arc::clone(&selected_epoch))
        })
        .unwrap();
        let active_recovery_fence = WriterFence {
            lease_id: LeaseId::from_bytes(id16(44)),
            generation: WriterGeneration::new(2).unwrap(),
        };

        assert_eq!(
            runtime.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );
        assert_eq!(
            runtime
                .recover_selected_epoch(
                    ExpectedHead::Empty,
                    command.writer_fence,
                    active_recovery_fence,
                    event,
                    &chain,
                    selected_epoch,
                    false,
                )
                .unwrap(),
            RecoverySelectionPublication::PublishedClosed
        );
        assert_eq!(
            runtime.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );
        runtime
            .reopen_recovered_selection(event, &chain, active_recovery_fence)
            .unwrap();
        assert_eq!(runtime.admit().unwrap().epoch_id(), selected);
    }

    #[test]
    fn restart_recovery_opens_empty_state_only_after_explicit_nonselection_proof() {
        let workspace = WorkspaceId::from_bytes(id16(1));
        let chain = ActivationChain::derive(workspace, []).unwrap();
        let runtime = FabricAdmissionRuntime::recover_for_reconciliation(&chain, |_| None).unwrap();
        let fence = WriterFence {
            lease_id: LeaseId::from_bytes(id16(4)),
            generation: WriterGeneration::new(1).unwrap(),
        };

        assert_eq!(
            runtime.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );
        runtime
            .recover_proved_no_selection(ExpectedHead::Empty, fence, fence, &chain)
            .unwrap();
        assert_eq!(runtime.admit().unwrap_err(), AdmissionError::NoActiveEpoch);
    }
}
