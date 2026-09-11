//! Shared capture/watch inclusion. Git ignore rules describe inputs; they do not authorize them.

#[cfg(feature = "daemon")]
use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// Potential context roots inside the registered workspace. Context discovery still decides
/// precedence, validates conflicting settings and admits providers from captured bytes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SourceInclusionPolicy {
    selected_roots: BTreeSet<Vec<u8>>,
    #[cfg(feature = "daemon")]
    configuration_digests: BTreeMap<Vec<u8>, Option<[u8; 32]>>,
}

impl SourceInclusionPolicy {
    #[cfg(feature = "daemon")]
    pub(crate) const CONFIGURATIONS: [&'static str; 3] =
        ["pyrefly.toml", "pyproject.toml", ".gitmodules"];
    #[cfg(feature = "daemon")]
    pub(crate) const MAXIMUM_CONFIGURATION_BYTES: u64 = 1024 * 1024;

    /// Capture candidates conservatively from both configuration surfaces. Invalid or conflicting
    /// configuration remains a captured input for the native context's existing diagnostics.
    #[cfg(feature = "daemon")]
    pub(crate) fn capture(mut read: impl FnMut(&[u8]) -> Option<Vec<u8>>) -> Self {
        let mut policy = Self::default();
        policy.observe_directory(b"", &mut read);
        policy
    }

    /// Called before enumerating this already-admitted directory's children. Configuration
    /// can select descendants, never ancestors, so recursive inclusion needs no second walk.
    #[cfg(feature = "daemon")]
    fn observe_directory(&mut self, prefix: &[u8], mut read: impl FnMut(&[u8]) -> Option<Vec<u8>>) {
        for name in Self::CONFIGURATIONS {
            let path = below(prefix, name.as_bytes());
            if self.configuration_digests.contains_key(&path) {
                continue;
            }
            let bytes = read(&path);
            self.configuration_digests
                .insert(path, bytes.as_deref().map(crate::integrity::digest_bytes));
            let Some(bytes) = bytes else { continue };
            if name == ".gitmodules" {
                if let Ok(modules) = declared_submodules(&bytes) {
                    self.selected_roots.extend(
                        modules
                            .into_iter()
                            .filter_map(|module| module.path)
                            .map(|path| below(prefix, &path)),
                    );
                }
                continue;
            }
            let Some(document) = std::str::from_utf8(&bytes)
                .ok()
                .and_then(|text| toml::from_str::<toml::Value>(text).ok())
            else {
                continue;
            };
            let settings = if name == "pyproject.toml" {
                document.get("tool").and_then(|tool| tool.get("pyrefly"))
            } else {
                Some(&document)
            };
            let Some(settings) = settings else { continue };
            for key in [
                "search-path",
                "search_path",
                "site-package-path",
                "site_package_path",
            ] {
                for value in settings
                    .get(key)
                    .and_then(toml::Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(path) = value.as_str().and_then(relative_root) else {
                        continue;
                    };
                    self.selected_roots.insert(below(prefix, &path));
                }
            }
        }
    }

    #[cfg(feature = "daemon")]
    fn unchanged(&self, mut read: impl FnMut(&[u8]) -> Option<Vec<u8>>) -> bool {
        self.configuration_digests.iter().all(|(path, digest)| {
            read(path).as_deref().map(crate::integrity::digest_bytes) == *digest
        })
    }

    #[cfg(feature = "daemon")]
    pub(crate) fn configuration_path(&self, path: &[u8]) -> bool {
        self.configuration_digests.contains_key(path)
    }

    pub(crate) fn includes(&self, relative: &[u8], directory: bool) -> bool {
        if relative
            .split(|byte| *byte == b'/')
            .any(|part| part == b".git")
        {
            return false;
        }
        let parent_length = if directory {
            relative.len()
        } else {
            relative.iter().rposition(|byte| *byte == b'/').unwrap_or(0)
        };
        let parent = &relative[..parent_length];
        let mut end = 0;
        for part in parent.split(|byte| *byte == b'/') {
            end += part.len();
            if excluded_directory(part)
                && !self.selected_roots.iter().any(|selected| {
                    is_below(selected, &parent[..end])
                        && (is_below(parent, selected) || is_below(selected, parent))
                })
            {
                return false;
            }
            end += 1;
        }
        true
    }

    #[cfg(feature = "daemon")]
    pub(crate) fn capture_secure(root: &crate::secure_path::SecureRoot) -> Self {
        Self::capture(|path| Self::read_secure(root, path))
    }

    #[cfg(feature = "daemon")]
    fn read_secure(root: &crate::secure_path::SecureRoot, path: &[u8]) -> Option<Vec<u8>> {
        let path = crate::secure_path::PlatformPath::from_raw_relative_bytes(
            root.platform_code(),
            path.to_vec(),
        )
        .ok()?;
        root.read_stable_file(&path, Self::MAXIMUM_CONFIGURATION_BYTES)
            .ok()
            .map(|read| read.bytes)
    }

    #[cfg(feature = "daemon")]
    pub(crate) fn observe_secure_directory(
        &mut self,
        root: &crate::secure_path::SecureRoot,
        prefix: &[u8],
    ) {
        self.observe_directory(prefix, |path| Self::read_secure(root, path));
    }

    #[cfg(feature = "daemon")]
    pub(crate) fn unchanged_secure(&self, root: &crate::secure_path::SecureRoot) -> bool {
        self.unchanged(|path| Self::read_secure(root, path))
    }

    #[cfg(feature = "daemon")]
    pub(crate) fn capture_watch(root: &std::path::Path) -> Self {
        Self::capture(|path| Self::read_watch(root, path))
    }

    #[cfg(feature = "daemon")]
    fn read_watch(root: &std::path::Path, path: &[u8]) -> Option<Vec<u8>> {
        use std::os::unix::ffi::OsStrExt as _;
        crate::secure_path::read_control_artifact_nofollow(
            &root.join(std::ffi::OsStr::from_bytes(path)),
            Self::MAXIMUM_CONFIGURATION_BYTES,
        )
        .ok()
    }

    #[cfg(feature = "daemon")]
    pub(crate) fn observe_watch_directory(&mut self, root: &std::path::Path, prefix: &[u8]) {
        self.observe_directory(prefix, |path| Self::read_watch(root, path));
    }

    #[cfg(feature = "daemon")]
    pub(crate) fn unchanged_watch(&self, root: &std::path::Path) -> bool {
        self.unchanged(|path| Self::read_watch(root, path))
    }

    #[cfg(feature = "daemon")]
    pub(crate) fn retained_bytes(&self) -> u64 {
        1024 + self
            .selected_roots
            .iter()
            .map(|path| path.capacity() as u64 + 32)
            .sum::<u64>()
            + self
                .configuration_digests
                .keys()
                .map(|path| path.capacity() as u64 + 96)
                .sum::<u64>()
    }
}

#[cfg(feature = "daemon")]
fn below(prefix: &[u8], path: &[u8]) -> Vec<u8> {
    if prefix.is_empty() {
        path.to_vec()
    } else {
        [prefix, path].join(&b'/')
    }
}

fn is_below(path: &[u8], ancestor: &[u8]) -> bool {
    path == ancestor
        || path
            .strip_prefix(ancestor)
            .is_some_and(|rest| rest.first() == Some(&b'/'))
}

fn excluded_directory(name: &[u8]) -> bool {
    matches!(
        name,
        b"target" | b".venv" | b"node_modules" | b"__pycache__"
    )
}

#[cfg(feature = "daemon")]
fn relative_root(path: &str) -> Option<Vec<u8>> {
    relative_root_bytes(path.as_bytes())
}

#[cfg(feature = "daemon")]
pub(crate) fn relative_root_bytes(path: &[u8]) -> Option<Vec<u8>> {
    if path.starts_with(b"/") || path.contains(&0) {
        return None;
    }
    let mut parts = Vec::new();
    for component in path.split(|byte| *byte == b'/') {
        match component {
            b"" | b"." => {}
            b".." | b".git" => return None,
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join(&b'/'))
}

/// Declared paths only; no URL, fetch, update command, activation override or historical fallback.
#[cfg(feature = "daemon")]
pub(crate) struct DeclaredSubmodule {
    pub name: Vec<u8>,
    pub path: Option<Vec<u8>>,
}

#[cfg(feature = "daemon")]
pub(crate) fn declared_submodules(bytes: &[u8]) -> Result<Vec<DeclaredSubmodule>, String> {
    use gix::bstr::ByteSlice as _;
    let modules = gix::submodule::File::from_bytes(bytes, None, &gix::config::File::default())
        .map_err(|error| error.to_string())?;
    Ok(modules
        .names()
        .map(|name| DeclaredSubmodule {
            name: name.to_owned().into(),
            path: modules
                .path(name)
                .ok()
                .and_then(|path| relative_root_bytes(path.as_bstr().as_bytes())),
        })
        .collect())
}

#[cfg(all(test, feature = "daemon"))]
mod tests {
    use super::*;

    #[test]
    fn previously_missing_nested_configuration_invalidates_the_walk_policy() {
        let mut files = BTreeMap::<Vec<u8>, Vec<u8>>::new();
        let mut policy = SourceInclusionPolicy::capture(|path| files.get(path).cloned());
        policy.observe_directory(b"nested", |path| files.get(path).cloned());
        assert!(policy.unchanged(|path| files.get(path).cloned()));
        assert!(policy.configuration_path(b"nested/.gitmodules"));
        files.insert(
            b"nested/.gitmodules".to_vec(),
            b"[submodule \"x\"]\npath = target/x\n".to_vec(),
        );
        assert!(
            !policy.unchanged(|path| files.get(path).cloned()),
            "absence is a real lookup dependency"
        );
    }

    #[test]
    fn configured_roots_override_pruning_without_admitting_siblings_or_git() {
        let policy = SourceInclusionPolicy::capture(|name| {
            (name == b"pyproject.toml").then(||
            b"[tool.pyrefly]\nsite-package-path=['.venv/lib/python3.14/site-packages']\nsearch-path=['../outside','.git/objects']\n".to_vec())
        });
        for path in [
            b".venv".as_slice(),
            b".venv/lib",
            b".venv/lib/python3.14/site-packages/pkg",
        ] {
            assert!(policy.includes(path, true));
        }
        assert!(policy.includes(b".venv/lib/python3.14/site-packages/pkg/a.pyi", false));
        assert!(policy.includes(b".venv/pyvenv.cfg", false));
        for path in [
            b".venv/bin".as_slice(),
            b".venv/lib/unselected",
            b"target",
            b".git",
            b"src/.git",
        ] {
            assert!(!policy.includes(path, true));
        }
        assert!(!policy.includes(b".venv/bin/python", false));
        assert!(!policy.includes(
            b".venv/lib/python3.14/site-packages/pkg/__pycache__/cached.pyc",
            false
        ));
        assert!(policy.includes(b"target", false)); // a regular file is not a build directory
        assert!(policy.includes(b"src/ignored.py", false));
    }
}
