//! Read-only gix acceleration boundary for the generated-output-free model compiler.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::Path;

use super::repository_model::{GitInventory, GitPathState, RepositoryModelError, WorktreeTopology};

/// Resolve topology and detached path classifications without exposing gix values.
pub(super) fn inventory(root: &Path) -> Result<GitInventory, RepositoryModelError> {
    let repository = gix::open_opts(
        root,
        gix::open::Options::isolated()
            .strict_config(true)
            .bail_if_untrusted(true),
    )
    .map_err(|_| RepositoryModelError::GitUnavailable)?;
    let work_dir = repository
        .workdir()
        .map(|path| path.as_os_str().as_bytes().to_vec());
    let git_dir = repository.git_dir().as_os_str().as_bytes().to_vec();
    let common_dir = repository.common_dir().as_os_str().as_bytes().to_vec();
    let topology = WorktreeTopology {
        work_dir,
        linked_worktree: repository.git_dir() != repository.common_dir(),
        git_dir: Some(git_dir),
        common_dir: Some(common_dir),
        git_available: true,
    };
    let index = repository
        .index_or_empty()
        .map_err(|_| RepositoryModelError::GitUnavailable)?;
    let mut states = BTreeMap::<Vec<u8>, BTreeSet<GitPathState>>::new();
    for entry in index.entries() {
        let path = entry.path(&index)[..].to_vec();
        let path_states = states.entry(path.clone()).or_default();
        path_states.insert(GitPathState::Tracked);
        if entry.stage_raw() != 0 {
            path_states.insert(GitPathState::Conflicted);
        }
        if repository
            .workdir()
            .is_some_and(|workdir| !workdir.join(OsString::from_vec(path)).exists())
        {
            path_states.insert(GitPathState::Deleted);
        }
    }

    let options = repository
        .dirwalk_options()
        .map_err(|_| RepositoryModelError::GitUnavailable)?
        .emit_tracked(true)
        .emit_ignored(Some(gix::dir::walk::EmissionMode::Matching))
        .emit_untracked(gix::dir::walk::EmissionMode::Matching)
        .emit_empty_directories(false)
        .recurse_repositories(false);
    let mut collect = gix::dir::walk::delegate::Collect::default();
    repository
        .dirwalk(
            &index,
            [] as [gix::bstr::BString; 0],
            &std::sync::atomic::AtomicBool::new(false),
            options,
            &mut collect,
        )
        .map_err(|_| RepositoryModelError::GitUnavailable)?;
    for (entry, _) in collect.into_entries_by_path() {
        let path = entry.rela_path[..].to_vec();
        if path.is_empty() {
            continue;
        }
        let state = match entry.status {
            gix::dir::entry::Status::Tracked => GitPathState::Tracked,
            gix::dir::entry::Status::Ignored(_) => GitPathState::Ignored,
            gix::dir::entry::Status::Untracked => GitPathState::Untracked,
            gix::dir::entry::Status::Pruned => continue,
        };
        states.entry(path).or_default().insert(state);
    }

    let head_tree = repository
        .head_tree_id_or_empty()
        .map_err(|_| RepositoryModelError::GitUnavailable)?;
    repository
        .tree_index_status(
            &head_tree,
            &index,
            None,
            gix::status::tree_index::TrackRenames::Disabled,
            |change, _, _| {
                states
                    .entry(change.location()[..].to_vec())
                    .or_default()
                    .insert(GitPathState::Staged);
                Ok::<_, std::convert::Infallible>(std::ops::ControlFlow::Continue(()))
            },
        )
        .map_err(|_| RepositoryModelError::GitUnavailable)?;

    let status_items = repository
        .status(gix::progress::Discard)
        .map_err(|_| RepositoryModelError::GitUnavailable)?
        .untracked_files(gix::status::UntrackedFiles::Files)
        .into_iter(Vec::<gix::bstr::BString>::new())
        .map_err(|_| RepositoryModelError::GitUnavailable)?;
    for item in status_items {
        let item = item.map_err(|_| RepositoryModelError::GitUnavailable)?;
        states
            .entry(item.location()[..].to_vec())
            .or_default()
            .insert(GitPathState::WorktreeModified);
    }
    Ok(GitInventory { topology, states })
}

/// Read one immutable baseline blob without consulting the worktree or invoking Git.
pub(super) fn blob_at_revision(
    root: &Path,
    revision: &str,
    relative_path: &Path,
) -> Result<Option<Vec<u8>>, RepositoryModelError> {
    let repository = gix::open_opts(
        root,
        gix::open::Options::isolated()
            .strict_config(true)
            .bail_if_untrusted(true),
    )
    .map_err(|_| RepositoryModelError::GitUnavailable)?;
    let commit = repository
        .rev_parse_single(revision)
        .map_err(|_| RepositoryModelError::GitUnavailable)?
        .object()
        .map_err(|_| RepositoryModelError::GitUnavailable)?
        .try_into_commit()
        .map_err(|_| RepositoryModelError::GitUnavailable)?;
    let tree = commit
        .tree()
        .map_err(|_| RepositoryModelError::GitUnavailable)?;
    let Some(entry) = tree
        .lookup_entry_by_path(relative_path)
        .map_err(|_| RepositoryModelError::GitUnavailable)?
    else {
        return Ok(None);
    };
    let mut blob = entry
        .object()
        .map_err(|_| RepositoryModelError::GitUnavailable)?
        .try_into_blob()
        .map_err(|_| RepositoryModelError::GitUnavailable)?;
    Ok(Some(blob.take_data()))
}
