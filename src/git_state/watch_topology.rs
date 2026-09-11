//! Detached, finite Git input-metadata watches. No source or history is read here.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Compiled exact-path callback lookup for all repositories admitted by the source traversal.
/// Constructed once on the blocking installer; callback reads need no filesystem work or locks.
#[derive(Default)]
pub(crate) struct SelectedGitWatchInputs {
    relevant: BTreeSet<PathBuf>,
    topology: BTreeSet<PathBuf>,
    directories: BTreeSet<PathBuf>,
}

impl SelectedGitWatchInputs {
    pub(crate) fn from_topologies(topologies: &[GitWatchTopology]) -> Self {
        let mut selected = Self::default();
        for git in topologies {
            selected.relevant.insert(git.marker.clone());
            selected.topology.insert(git.marker.clone());
            selected.directories.extend(git.directories());
            for root in &git.roots {
                selected.relevant.insert(root.clone());
                selected.topology.insert(root.clone());
                for name in [
                    "index",
                    "config",
                    "config.worktree",
                    "commondir",
                    "gitdir",
                    "info",
                    "info/exclude",
                    "info/attributes",
                ] {
                    selected.relevant.insert(root.join(name));
                }
                for name in ["commondir", "gitdir", "config", "config.worktree", "info"] {
                    selected.topology.insert(root.join(name));
                }
            }
        }
        selected
    }

    pub(crate) fn relevant(&self, path: &Path) -> bool {
        self.relevant.contains(path)
    }
    pub(crate) fn topology_relevant(&self, path: &Path) -> bool {
        self.topology.contains(path)
    }
    pub(crate) fn directories(&self) -> &BTreeSet<PathBuf> {
        &self.directories
    }
    pub(crate) fn retained_bytes(&self) -> u64 {
        1024 + [&self.relevant, &self.topology, &self.directories]
            .into_iter()
            .flat_map(|paths| paths.iter())
            .map(|path| path.capacity() as u64 + 128)
            .sum::<u64>()
    }
}

/// Only the selected worktree's index/policy metadata affects input observation.
/// Other worktrees, object databases, refs, logs and hooks are not recursively watched.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GitWatchTopology {
    marker: PathBuf,
    roots: Vec<PathBuf>,
}

impl GitWatchTopology {
    pub(crate) fn resolve(root: &Path) -> Self {
        let marker = root.join(".git");
        let roots = match super::open_isolated(root) {
            Ok(repository) if !repository.is_bare() => {
                let mut roots = [repository.git_dir(), repository.common_dir()]
                    .into_iter()
                    .filter_map(|path| std::fs::canonicalize(path).ok())
                    .collect::<Vec<_>>();
                roots.sort();
                roots.dedup();
                roots
            }
            // Git is advisory to authoritative source capture. Keep observing the marker
            // while a repository is absent, malformed or being moved, then resolve again.
            _ => Vec::new(),
        };
        Self { marker, roots }
    }

    #[cfg(test)]
    pub(crate) fn relevant(&self, path: &Path) -> bool {
        path == self.marker
            || self.roots.iter().any(|root| {
                let Ok(relative) = path.strip_prefix(root) else {
                    return false;
                };
                relative.as_os_str().is_empty()
                    || [
                        "index",
                        "config",
                        "config.worktree",
                        "commondir",
                        "gitdir",
                        "info",
                    ]
                    .iter()
                    .any(|name| relative == Path::new(name))
                    || relative == Path::new("info/exclude")
                    || relative == Path::new("info/attributes")
            })
    }

    pub(crate) fn directories(&self) -> Vec<PathBuf> {
        let mut directories = Vec::with_capacity(6);
        for root in &self.roots {
            // Parent observation survives removal/replacement of an external administrative
            // directory. The callback accepts only explicitly selected metadata paths.
            if let Some(parent) = root.parent() {
                directories.push(parent.to_owned());
            }
            directories.push(root.clone());
            let info = root.join("info");
            if std::fs::symlink_metadata(&info).is_ok_and(|metadata| metadata.is_dir()) {
                directories.push(info);
            }
        }
        directories.sort();
        directories.dedup();
        directories
    }
}

#[cfg(test)]
pub(crate) fn linked_fixture(root: &Path, main: &Path) -> PathBuf {
    std::fs::create_dir_all(root).unwrap();
    let repository = gix::init(main).unwrap();
    let common = repository.git_dir();
    let administrative = common.join("worktrees/selected");
    std::fs::create_dir_all(&administrative).unwrap();
    std::fs::write(administrative.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::write(administrative.join("commondir"), "../..\n").unwrap();
    std::fs::write(
        administrative.join("gitdir"),
        format!("{}\n", root.join(".git").display()),
    )
    .unwrap();
    std::fs::write(
        root.join(".git"),
        format!("gitdir: {}\n", administrative.display()),
    )
    .unwrap();
    administrative
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_native_linked_worktree_metadata_without_walking_other_worktrees_or_objects() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("selected");
        let main = fixture.path().join("main");
        let administrative = linked_fixture(&root, &main);
        let topology = GitWatchTopology::resolve(&root);
        assert!(topology.roots.contains(&administrative));
        assert!(topology.roots.contains(&main.join(".git")));
        for path in [
            root.join(".git"),
            administrative.join("index"),
            main.join(".git/info/exclude"),
        ] {
            assert!(topology.relevant(&path));
        }
        for path in [
            main.join(".git/objects/pack/generated.pack"),
            main.join(".git/refs/heads/main"),
            main.join(".git/worktrees/other/index"),
        ] {
            assert!(!topology.relevant(&path));
        }
        assert!(topology.directories().len() <= 6);
        assert!(!topology.directories().contains(&main.join(".git/objects")));
    }

    #[test]
    fn absent_or_replaced_git_marker_retains_observation_without_source_authority() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("selected");
        std::fs::create_dir(&root).unwrap();
        let absent = GitWatchTopology::resolve(&root);
        assert!(absent.roots.is_empty());
        assert!(absent.relevant(&root.join(".git")));
        let first = linked_fixture(&root, &fixture.path().join("main"));
        let selected = GitWatchTopology::resolve(&root);
        assert!(selected.roots.contains(&first));
        let replacement = linked_fixture(&root, &fixture.path().join("other"));
        let replaced = GitWatchTopology::resolve(&root);
        assert!(replaced.roots.contains(&replacement));
        assert!(!replaced.roots.contains(&first));
        std::fs::write(root.join(".git"), "gitdir: missing\n").unwrap();
        assert!(GitWatchTopology::resolve(&root).roots.is_empty());
    }
}
