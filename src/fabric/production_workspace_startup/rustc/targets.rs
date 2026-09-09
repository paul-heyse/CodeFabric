//! Cargo target selection from captured manifests and source paths.

use std::collections::BTreeMap;

use super::{ProductionWorkspaceStartupError, step};
use crate::analysis_context::{ContextFileInput, RustTargetKind, RustTargetSettings};

#[derive(Clone, Debug)]
pub(super) struct CargoTarget {
    pub manifest: Vec<u8>,
    pub package: String,
    pub target: RustTargetSettings,
}

#[allow(clippy::too_many_lines)] // Explicit targets override automatic discovery in one collector.
pub(super) fn discover(
    files: &[ContextFileInput],
) -> Result<Vec<CargoTarget>, ProductionWorkspaceStartupError> {
    let mut targets = BTreeMap::new();
    for manifest in files.iter().filter(|file| {
        file.relative_path == b"Cargo.toml" || file.relative_path.ends_with(b"/Cargo.toml")
    }) {
        let document: toml::Value = toml::from_str(
            std::str::from_utf8(&manifest.contents)
                .map_err(|error| step("rust-target-manifest", error))?,
        )
        .map_err(|error| step("rust-target-manifest", error))?;
        let Some(package) = document.get("package") else {
            continue;
        };
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| step("rust-target-package", "package name missing"))?;
        let parent = manifest
            .relative_path
            .strip_suffix(b"Cargo.toml")
            .expect("manifest suffix");
        let mut add = |kind: RustTargetKind,
                       name: String,
                       relative: &str|
         -> Result<(), ProductionWorkspaceStartupError> {
            let path = join(parent, relative)?;
            if files.iter().any(|file| file.relative_path == path) {
                targets.insert(
                    (
                        manifest.relative_path.clone(),
                        format!("{kind:?}"),
                        name.clone(),
                    ),
                    CargoTarget {
                        manifest: manifest.relative_path.clone(),
                        package: package["name"].as_str().expect("package name").to_owned(),
                        target: RustTargetSettings {
                            name,
                            kind,
                            crate_root: path,
                        },
                    },
                );
            }
            Ok(())
        };
        let lib = document.get("lib");
        if lib.is_some() || package.get("autolib").and_then(toml::Value::as_bool) != Some(false) {
            let kind = if lib
                .and_then(|value| value.get("proc-macro"))
                .and_then(toml::Value::as_bool)
                == Some(true)
            {
                RustTargetKind::ProcMacro
            } else {
                RustTargetKind::Library
            };
            add(
                kind,
                lib.and_then(|value| value.get("name"))
                    .and_then(toml::Value::as_str)
                    .map_or_else(|| name.replace('-', "_"), str::to_owned),
                lib.and_then(|value| value.get("path"))
                    .and_then(toml::Value::as_str)
                    .unwrap_or("src/lib.rs"),
            )?;
        }
        if package.get("autobins").and_then(toml::Value::as_bool) != Some(false) {
            add(RustTargetKind::Binary, name.to_owned(), "src/main.rs")?;
        }
        for (table, directory, flag, kind) in [
            ("bin", "src/bin/", "autobins", RustTargetKind::Binary),
            (
                "example",
                "examples/",
                "autoexamples",
                RustTargetKind::Example,
            ),
            ("test", "tests/", "autotests", RustTargetKind::Test),
            (
                "bench",
                "benches/",
                "autobenches",
                RustTargetKind::Benchmark,
            ),
        ] {
            if package.get(flag).and_then(toml::Value::as_bool) != Some(false) {
                let prefix = join(parent, directory)?;
                for file in files {
                    let Some(tail) = file
                        .relative_path
                        .strip_prefix(prefix.as_slice())
                        .and_then(|tail| tail.strip_prefix(b"/"))
                    else {
                        continue;
                    };
                    let automatic = if let Some(stem) = tail.strip_suffix(b"/main.rs") {
                        (!stem.contains(&b'/')).then_some(stem)
                    } else {
                        tail.strip_suffix(b".rs")
                            .filter(|stem| !stem.contains(&b'/'))
                    };
                    if let Some(stem) = automatic {
                        let name = std::str::from_utf8(stem)
                            .map_err(|error| step("rust-target-name", error))?;
                        let relative = std::str::from_utf8(
                            file.relative_path
                                .strip_prefix(parent)
                                .expect("target parent"),
                        )
                        .map_err(|error| step("rust-target-path", error))?;
                        add(kind, name.to_owned(), relative)?;
                    }
                }
            }
            if let Some(explicit) = document.get(table).and_then(toml::Value::as_array) {
                for target in explicit {
                    let target_name = target
                        .get("name")
                        .and_then(toml::Value::as_str)
                        .unwrap_or(name);
                    let inferred = if table == "bin" && target_name == name {
                        "src/main.rs".to_owned()
                    } else {
                        format!("{directory}{target_name}.rs")
                    };
                    add(
                        kind,
                        target_name.to_owned(),
                        target
                            .get("path")
                            .and_then(toml::Value::as_str)
                            .unwrap_or(&inferred),
                    )?;
                }
            }
        }
    }
    if targets.is_empty() {
        return Err(step(
            "rust-targets",
            "no captured Cargo target source is available",
        ));
    }
    Ok(targets.into_values().collect())
}

fn join(parent: &[u8], relative: &str) -> Result<Vec<u8>, ProductionWorkspaceStartupError> {
    if relative.starts_with('/') || relative.as_bytes().contains(&0) {
        return Err(step(
            "rust-target-path",
            "target path must stay inside captured workspace",
        ));
    }
    let mut parts = parent
        .split(|byte| *byte == b'/')
        .filter(|part| !part.is_empty())
        .map(<[u8]>::to_vec)
        .collect::<Vec<_>>();
    for part in relative.as_bytes().split(|byte| *byte == b'/') {
        match part {
            b"" | b"." => {}
            b".." => {
                if parts.pop().is_none() {
                    return Err(step(
                        "rust-target-path",
                        "target path escapes captured workspace",
                    ));
                }
            }
            other => parts.push(other.to_vec()),
        }
    }
    Ok(parts.join(&b'/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file(path: &str, text: &str) -> ContextFileInput {
        ContextFileInput {
            file_id: path.to_owned(),
            relative_path: path.as_bytes().to_vec(),
            digest: crate::integrity::digest_bytes(text.as_bytes()),
            contents: text.as_bytes().to_vec(),
        }
    }
    #[test]
    fn captured_targets_include_directory_binaries_and_explicit_library() {
        let inputs = vec![
            file(
                "Cargo.toml",
                "[package]\nname = 'fixture'\nversion = '0.1.0'\n[lib]\nname = 'engine'\npath = 'engine.rs'\n",
            ),
            file("engine.rs", ""),
            file("src/main.rs", ""),
            file("src/bin/inspect/main.rs", ""),
            file("tests/api.rs", ""),
        ];
        let targets = discover(&inputs).unwrap();
        assert_eq!(targets.len(), 4);
        assert!(targets.iter().any(|target| target.target.name == "inspect"
            && target.target.crate_root == b"src/bin/inspect/main.rs"));
        assert!(targets.iter().any(|target| target.target.name == "engine"
            && target.target.kind == RustTargetKind::Library));
        assert!(targets.iter().any(
            |target| target.target.name == "api" && target.target.kind == RustTargetKind::Test
        ));
        assert!(join(b"member/", "../../outside.rs").is_err());
        assert_eq!(join(b"member/", "../shared.rs").unwrap(), b"shared.rs");
    }
    #[test]
    fn virtual_workspace_discovers_member_targets_and_respects_disabled_auto_targets() {
        let inputs = vec![
            file("Cargo.toml", "[workspace]\nmembers = ['member']\n"),
            file(
                "member/Cargo.toml",
                "[package]\nname = 'member'\nversion = '0.1.0'\nautobins = false\n",
            ),
            file("member/src/lib.rs", ""),
            file("member/src/bin/hidden.rs", ""),
        ];
        let targets = discover(&inputs).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].manifest, b"member/Cargo.toml");
        assert_eq!(targets[0].target.crate_root, b"member/src/lib.rs");
    }
}
