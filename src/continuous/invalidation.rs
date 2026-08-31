//! Dependency-closed dirty coverage for the continuous source actor.
//!
//! Watcher paths are urgency hints. This seam joins their last accepted owners to the persisted
//! operational dependency relation before source capture. A complete closure adds conservative
//! dependents (including reverse importers) as ordinary current-byte work. Any missing owner
//! coverage, explicit watcher loss, or rescan request broadens to a full authorized inventory;
//! absence in the dependency relation is never interpreted as proof that no dependent exists.

use std::collections::{BTreeMap, BTreeSet};

use crate::inventory::InclusionState;
use crate::lifecycle::{
    InvalidationPlan, LifecycleError, OperationalDependencyGraph, WatchHint, WatchHintBatch,
    WatchHintKind,
};
use crate::operational_store::OperationalStore;

pub(super) struct ExpandedDirtyCoverage {
    pub batch: WatchHintBatch,
    pub plan: InvalidationPlan,
}

/// Expand normalized watcher hints through the last accepted dependency relation.
///
/// New paths after a non-empty generation and owners absent from the dependency graph cannot
/// establish a closed reverse-dependency set. Both cases become a full inventory. The generic
/// inventory and source-image capture remain the byte authorities in the caller.
pub(super) fn expand_dirty_coverage(
    store: &OperationalStore,
    workspace_id: [u8; 16],
    current_source_generation: u64,
    mut batch: WatchHintBatch,
) -> Result<ExpandedDirtyCoverage, LifecycleError> {
    let owners_by_path = load_current_owner_paths(store, workspace_id, current_source_generation)?;
    let all_owners = owners_by_path.values().copied().collect::<BTreeSet<_>>();
    let changed_owners = batch
        .hints
        .iter()
        .filter_map(|hint| owners_by_path.get(&hint.path_bytes).copied())
        .collect::<BTreeSet<_>>();
    let new_or_uncovered_path = current_source_generation > 0
        && batch
            .hints
            .iter()
            .any(|hint| !owners_by_path.contains_key(&hint.path_bytes));
    let forced_full_inventory = batch.rescan_required || new_or_uncovered_path;

    let graph = OperationalDependencyGraph::load(&store.reader_factory().open()?, workspace_id)?;
    let seeds = if forced_full_inventory {
        &all_owners
    } else {
        &changed_owners
    };
    let mut plan = graph.plan_invalidation(seeds);
    if forced_full_inventory {
        plan.full_rebuild_required = true;
        plan.affected_owners.extend(all_owners.iter().copied());
    }

    if plan.full_rebuild_required {
        // Individual hints cannot express a proved complete set. Let the caller select every
        // currently authorized path through its ordinary generic-inventory path.
        batch.hints.clear();
        batch.rescan_required = true;
    } else {
        for (path_bytes, owner_id) in owners_by_path {
            if plan.affected_owners.contains(&owner_id)
                && !batch.hints.iter().any(|hint| hint.path_bytes == path_bytes)
            {
                batch.hints.push(WatchHint {
                    path_bytes,
                    kind: WatchHintKind::Unknown,
                });
            }
        }
        batch.hints.sort();
        batch.hints.dedup();
    }

    Ok(ExpandedDirtyCoverage { batch, plan })
}

fn load_current_owner_paths(
    store: &OperationalStore,
    workspace_id: [u8; 16],
    source_generation: u64,
) -> Result<BTreeMap<Vec<u8>, [u8; 16]>, LifecycleError> {
    let source_generation = i64::try_from(source_generation)
        .map_err(|_| LifecycleError::Configuration("source generation exceeds i64".into()))?;
    let rows = store
        .reader_factory()
        .open()?
        .with_connection_result(|connection| {
            let mut statement = connection.prepare(
                "SELECT path_bytes,COALESCE(current_file_owner,file_id)
                   FROM source_inventory
                  WHERE workspace_id=?1 AND source_generation=?2
                    AND inclusion_state_code=?3
                    AND COALESCE(current_file_owner,file_id) IS NOT NULL
                  ORDER BY path_bytes",
            )?;
            statement
                .query_map(
                    rusqlite::params![
                        workspace_id.as_slice(),
                        source_generation,
                        i64::from(InclusionState::Included as u16),
                    ],
                    |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
                )?
                .collect::<Result<Vec<_>, _>>()
                .map_err(LifecycleError::from)
        })?;
    rows.into_iter()
        .map(|(path, owner)| {
            let owner = owner
                .try_into()
                .map_err(|_| LifecycleError::Graph("current owner has invalid width".into()))?;
            Ok((path, owner))
        })
        .collect()
}
