//! Shared capture/watch inclusion. Git ignore rules describe inputs; they do not authorize them.

/// Potential context roots inside the registered workspace. Context discovery still decides
/// precedence, validates conflicting settings and admits providers from captured bytes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SourceInclusionPolicy {
    selected_roots: Vec<Vec<u8>>,
    #[cfg(feature = "daemon")]
    configuration_digests: Vec<(&'static str, [u8; 32])>,
}

impl SourceInclusionPolicy {
    #[cfg(feature = "daemon")]
    pub(crate) const CONFIGURATIONS: [&'static str; 2] = ["pyrefly.toml", "pyproject.toml"];
    #[cfg(feature = "daemon")]
    pub(crate) const MAXIMUM_CONFIGURATION_BYTES: u64 = 1024 * 1024;

    /// Capture candidates conservatively from both configuration surfaces. Invalid or conflicting
    /// configuration remains a captured input for the native context's existing diagnostics.
    #[cfg(feature = "daemon")]
    pub(crate) fn capture(mut read: impl FnMut(&str) -> Option<Vec<u8>>) -> Self {
        let mut policy = Self::default();
        for name in Self::CONFIGURATIONS {
            let Some(bytes) = read(name) else { continue };
            policy
                .configuration_digests
                .push((name, crate::integrity::digest_bytes(&bytes)));
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
                    policy.selected_roots.push(path);
                }
            }
        }
        policy.selected_roots.sort();
        policy.selected_roots.dedup();
        policy
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
        Self::capture(|name| {
            let path = crate::secure_path::PlatformPath::from_raw_relative_bytes(
                root.platform_code(),
                name.as_bytes().to_vec(),
            )
            .ok()?;
            root.read_stable_file(&path, Self::MAXIMUM_CONFIGURATION_BYTES)
                .ok()
                .map(|read| read.bytes)
        })
    }

    #[cfg(feature = "daemon")]
    pub(crate) fn capture_watch(root: &std::path::Path) -> Self {
        Self::capture(|name| {
            crate::secure_path::read_control_artifact(
                &root.join(name),
                Self::MAXIMUM_CONFIGURATION_BYTES,
            )
            .ok()
        })
    }

    #[cfg(feature = "daemon")]
    pub(crate) fn retained_bytes(&self) -> u64 {
        1024 + self
            .selected_roots
            .iter()
            .map(|path| path.capacity() as u64 + 32)
            .sum::<u64>()
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
    if path.starts_with('/') {
        return None;
    }
    let mut parts = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." | ".git" => return None,
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/").into_bytes())
}

#[cfg(all(test, feature = "daemon"))]
mod tests {
    use super::*;

    #[test]
    fn configured_roots_override_pruning_without_admitting_siblings_or_git() {
        let policy = SourceInclusionPolicy::capture(|name| {
            (name == "pyproject.toml").then(||
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
